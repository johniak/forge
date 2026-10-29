//! `forge check`: manifests, architecture guard, formatting and lints, as one
//! structured report (SPEC §8.2 `forge.check` / `forge.architecture.check`).

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::Value;

use crate::architecture::check_architecture;
use crate::manifest::load_project;
use crate::process::{CommandOutput, CommandRunner};
use crate::report::{CheckReport, CheckResult, Diagnostic, Severity, Span};

/// The individual checks, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckId {
    /// `forge.toml` and `forge.module.toml` validity.
    Manifests,
    /// Real Cargo dependencies vs. module manifests.
    Architecture,
    /// `cargo fmt --check`.
    Fmt,
    /// `cargo clippy` with warnings denied.
    Clippy,
}

impl CheckId {
    /// Stable identifier used in reports and on the command line.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            CheckId::Manifests => "manifests",
            CheckId::Architecture => "architecture",
            CheckId::Fmt => "fmt",
            CheckId::Clippy => "clippy",
        }
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error for an unrecognised check name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown check `{0}` (expected one of: manifests, architecture, fmt, clippy)")]
pub struct UnknownCheck(pub String);

impl FromStr for CheckId {
    type Err = UnknownCheck;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manifests" => Ok(CheckId::Manifests),
            "architecture" => Ok(CheckId::Architecture),
            "fmt" => Ok(CheckId::Fmt),
            "clippy" => Ok(CheckId::Clippy),
            other => Err(UnknownCheck(other.to_string())),
        }
    }
}

/// What to check and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOptions {
    /// Project root (directory holding `forge.toml`).
    pub root: PathBuf,
    /// Checks to skip; each is reported as `NOT_VERIFIED`, never as a pass.
    pub skip: Vec<CheckId>,
}

/// Runs every check and folds the verdict.
#[must_use]
pub fn run_check(options: &CheckOptions, runner: &dyn CommandRunner) -> CheckReport {
    let skipped = |id: CheckId| options.skip.contains(&id);
    let skip_result =
        |id: CheckId| CheckResult::not_verified(id.as_str(), "FSKIP", "skipped by request");
    let mut checks = Vec::with_capacity(4);

    let project = if skipped(CheckId::Manifests) {
        checks.push(skip_result(CheckId::Manifests));
        None
    } else {
        match load_project(&options.root) {
            Ok(p) => {
                checks.push(CheckResult::pass(CheckId::Manifests.as_str()));
                Some(p)
            }
            Err(diags) => {
                checks.push(CheckResult::fail(CheckId::Manifests.as_str(), diags));
                None
            }
        }
    };

    checks.push(match (&project, skipped(CheckId::Architecture)) {
        (_, true) => skip_result(CheckId::Architecture),
        (None, false) => CheckResult::not_verified(
            CheckId::Architecture.as_str(),
            "FA000",
            "manifests not loaded; the architecture check cannot run",
        ),
        (Some(p), false) => {
            let diags = check_architecture(p);
            if diags.is_empty() {
                CheckResult::pass(CheckId::Architecture.as_str())
            } else {
                CheckResult::fail(CheckId::Architecture.as_str(), diags)
            }
        }
    });

    let cwd = project
        .as_ref()
        .map_or_else(|| options.root.clone(), |p| p.root.clone());
    checks.push(if skipped(CheckId::Fmt) {
        skip_result(CheckId::Fmt)
    } else {
        run_fmt(runner, &cwd)
    });
    checks.push(if skipped(CheckId::Clippy) {
        skip_result(CheckId::Clippy)
    } else {
        run_clippy(runner, &cwd)
    });

    CheckReport::new("forge.check", checks)
}

fn launch_failure(id: CheckId, what: &str, err: &std::io::Error) -> CheckResult {
    CheckResult::not_verified(
        id.as_str(),
        "FP001",
        format!("could not run `{what}`: {err}"),
    )
}

fn run_fmt(runner: &dyn CommandRunner, cwd: &Path) -> CheckResult {
    let output = match runner.run("cargo", &["fmt", "--all", "--", "--check"], cwd) {
        Ok(o) => o,
        Err(err) => return launch_failure(CheckId::Fmt, "cargo fmt", &err),
    };
    if output.status == Some(0) {
        return CheckResult::pass(CheckId::Fmt.as_str());
    }
    let mut diagnostics = parse_fmt_diff(&output.stdout);
    if diagnostics.is_empty() {
        diagnostics.push(Diagnostic::error(
            "FF000",
            format!("cargo fmt exited with status {}", status_text(&output)),
        ));
    }
    CheckResult::fail(CheckId::Fmt.as_str(), diagnostics).with_raw_log(raw_log(&output))
}

fn status_text(output: &CommandOutput) -> String {
    output
        .status
        .map_or_else(|| "signal".to_string(), |c| c.to_string())
}

fn raw_log(output: &CommandOutput) -> String {
    format!("{}{}", output.stdout, output.stderr)
}

/// Turns `rustfmt --check` output (`Diff in <file>:<line>:` or the older
/// `Diff in <file> at line <line>:`) into one diagnostic per hunk.
fn parse_fmt_diff(stdout: &str) -> Vec<Diagnostic> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("Diff in "))
        .filter_map(|rest| {
            let rest = rest.trim_end_matches(':');
            let (file, line) = rest
                .rsplit_once(" at line ")
                .or_else(|| rest.rsplit_once(':'))?;
            let line = line.trim().parse::<u32>().ok()?;
            Some(
                Diagnostic::error("FF001", format!("{file} is not formatted (line {line})"))
                    .with_span(Span::line_only(PathBuf::from(file), line))
                    .with_help("run `cargo fmt --all`"),
            )
        })
        .collect()
}

fn run_clippy(runner: &dyn CommandRunner, cwd: &Path) -> CheckResult {
    let args = [
        "clippy",
        "--workspace",
        "--all-targets",
        "--message-format=json",
        "--",
        "-D",
        "warnings",
    ];
    let output = match runner.run("cargo", &args, cwd) {
        Ok(o) => o,
        Err(err) => return launch_failure(CheckId::Clippy, "cargo clippy", &err),
    };
    let mut diagnostics = parse_cargo_messages(&output.stdout, cwd);
    let has_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    let id = CheckId::Clippy.as_str();
    if output.status == Some(0) {
        let mut result = CheckResult::pass(id);
        result.diagnostics = diagnostics;
        return result;
    }
    if !has_errors {
        diagnostics.push(Diagnostic::error(
            "FC000",
            format!("cargo clippy exited with status {}", status_text(&output)),
        ));
    }
    CheckResult::fail(id, diagnostics).with_raw_log(raw_log(&output))
}

/// Parses `cargo --message-format=json` lines; keeps error/warning compiler
/// messages that have a span (summary lines like "aborting due to…" have none).
fn parse_cargo_messages(stdout: &str, root: &Path) -> Vec<Diagnostic> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["reason"] == "compiler-message")
        .filter_map(|v| compiler_message(&v["message"], root))
        .collect()
}

fn compiler_message(message: &Value, root: &Path) -> Option<Diagnostic> {
    let severity = match message["level"].as_str()? {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        _ => return None,
    };
    let spans = message["spans"].as_array()?;
    let span = spans
        .iter()
        .find(|s| s["is_primary"] == true)
        .or_else(|| spans.first())?;
    let file = PathBuf::from(span["file_name"].as_str()?);
    let file = if file.is_absolute() {
        file
    } else {
        root.join(file)
    };
    let code = message["code"]["code"]
        .as_str()
        .unwrap_or("FC001")
        .to_string();
    let text = message["message"].as_str().unwrap_or_default().to_string();
    let help = message["children"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["level"] == "help")
        .and_then(|c| c["message"].as_str())
        .map(ToString::to_string);
    let mut diag = Diagnostic {
        severity,
        code,
        message: text,
        span: None,
        help,
    };
    diag.span = Some(Span {
        file,
        line: u32::try_from(span["line_start"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX),
        column: u32::try_from(span["column_start"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX),
        start: usize::try_from(span["byte_start"].as_u64().unwrap_or(0)).unwrap_or(usize::MAX),
        end: usize::try_from(span["byte_end"].as_u64().unwrap_or(0)).unwrap_or(usize::MAX),
    });
    Some(diag)
}
