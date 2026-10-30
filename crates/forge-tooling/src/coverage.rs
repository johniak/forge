//! `forge coverage`: per-module line and branch coverage gate over the JSON
//! export of `cargo llvm-cov --json --summary-only` (ADR-0002 amendment).
//!
//! `cargo llvm-cov` has no branch threshold of its own; this gate enforces the
//! `[verification]` thresholds every module declares in `forge.module.toml`.
//! Code that did not run under coverage is never counted as covered: a module
//! without measured files is `NOT_VERIFIED`, an export without branch data is
//! `NOT_VERIFIED`, source files that `cargo llvm-cov` hides by default are
//! `NOT_VERIFIED`, and a measured file that belongs to no module fails the
//! report. Thresholds below 100 fail until owner-approved exclusions exist.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::manifest::{Module, load_project};
use crate::report::{CheckReport, CheckResult, Diagnostic, Span, Status};

const TOOL: &str = "forge.coverage";
const EXPORT_TYPE: &str = "llvm.coverage.json.export";

/// What to gate and where the coverage export is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageOptions {
    /// Project root (directory holding `forge.toml`).
    pub root: PathBuf,
    /// Path of the `llvm-cov export -format=json` document.
    pub input: PathBuf,
}

/// Covered and total counts of one kind (lines or branches).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Counts {
    /// Total measurable items.
    pub count: u64,
    /// Items executed at least once.
    pub covered: u64,
}

impl Counts {
    /// `true` when `covered / count` is at least `threshold_percent`; nothing to
    /// measure (`count == 0`) meets every threshold.
    #[must_use]
    pub fn meets(self, threshold_percent: u8) -> bool {
        u128::from(self.covered) * 100 >= u128::from(threshold_percent) * u128::from(self.count)
    }

    fn missed(self) -> u64 {
        self.count.saturating_sub(self.covered)
    }

    fn add(self, other: Counts) -> Counts {
        Counts {
            count: self.count + other.count,
            covered: self.covered + other.covered,
        }
    }
}

/// Coverage of one measured source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCoverage {
    /// Path as written in the export (canonicalized when attributed).
    pub file: PathBuf,
    /// Line counts.
    pub lines: Counts,
    /// Branch counts.
    pub branches: Counts,
}

#[derive(Deserialize)]
struct Export {
    #[serde(rename = "type")]
    kind: String,
    data: Vec<ExportData>,
}

#[derive(Deserialize)]
struct ExportData {
    files: Vec<ExportFile>,
}

#[derive(Deserialize)]
struct ExportFile {
    filename: PathBuf,
    summary: ExportSummary,
}

#[derive(Deserialize)]
struct ExportSummary {
    lines: Counts,
    branches: Counts,
}

/// Reads the per-file summaries of an `llvm-cov export -format=json` document.
///
/// # Errors
///
/// `FV004` when the text is not such a document.
pub fn parse_llvm_export(text: &str) -> Result<Vec<FileCoverage>, Box<Diagnostic>> {
    let export: Export = serde_json::from_str(text)
        .map_err(|err| Diagnostic::error("FV004", format!("not an llvm-cov JSON export: {err}")))?;
    if export.kind != EXPORT_TYPE {
        return Err(Box::new(Diagnostic::error(
            "FV004",
            format!(
                "not an llvm-cov JSON export: type is `{}`, expected `{EXPORT_TYPE}`",
                export.kind
            ),
        )));
    }
    Ok(export
        .data
        .into_iter()
        .flat_map(|data| data.files)
        .map(|f| FileCoverage {
            file: f.filename,
            lines: f.summary.lines,
            branches: f.summary.branches,
        })
        .collect())
}

/// Gates every module of the project against the coverage export.
#[must_use]
pub fn run_coverage(options: &CoverageOptions) -> CheckReport {
    let project = match load_project(&options.root) {
        Ok(p) => p,
        Err(diags) => return CheckReport::new(TOOL, vec![CheckResult::fail("manifests", diags)]),
    };
    let files = match read_export(&options.input) {
        Ok(files) => files,
        Err(reason) => {
            let checks = project
                .modules
                .iter()
                .map(|m| CheckResult::not_verified(result_id(m), "FV004", reason.clone()))
                .collect();
            return CheckReport::new(TOOL, checks);
        }
    };

    let has_branch_data = files.iter().any(|f| f.branches.count > 0);
    let mut owned: Vec<Vec<FileCoverage>> = vec![Vec::new(); project.modules.len()];
    let mut unowned = Vec::new();
    for mut file in files {
        file.file = file.file.canonicalize().unwrap_or(file.file);
        match owner(&project.modules, &file.file) {
            Some(i) => owned[i].push(file),
            None => unowned.push(file.file),
        }
    }

    let mut checks: Vec<CheckResult> = project
        .modules
        .iter()
        .zip(owned)
        .map(|(module, files)| module_result(module, files, has_branch_data))
        .collect();
    if !unowned.is_empty() {
        checks.push(CheckResult::fail(
            "coverage:unowned",
            unowned.into_iter().map(unowned_file).collect(),
        ));
    }
    CheckReport::new(TOOL, checks)
}

fn read_export(input: &Path) -> Result<Vec<FileCoverage>, String> {
    let text = fs::read_to_string(input)
        .map_err(|err| format!("cannot read {}: {err}", input.display()))?;
    parse_llvm_export(&text).map_err(|d| format!("{}: {}", input.display(), d.message))
}

fn result_id(module: &Module) -> String {
    format!("coverage:{}", module.name)
}

/// The module whose directory contains `file`, comparing whole path components
/// so that `crates/ab` never matches module `crates/a`; the deepest wins.
fn owner(modules: &[Module], file: &Path) -> Option<usize> {
    modules
        .iter()
        .enumerate()
        .filter(|(_, m)| file.starts_with(&m.path))
        .max_by_key(|(_, m)| m.path.components().count())
        .map(|(i, _)| i)
}

fn module_result(
    module: &Module,
    mut files: Vec<FileCoverage>,
    has_branch_data: bool,
) -> CheckResult {
    let id = result_id(module);
    let required = module.verification;
    let lowered: Vec<Diagnostic> = [
        ("line_coverage", required.line_coverage),
        ("branch_coverage", required.branch_coverage),
    ]
    .into_iter()
    .filter(|(_, percent)| *percent < 100)
    .map(|(field, percent)| lowered_threshold(module, field, percent))
    .collect();
    if !lowered.is_empty() {
        return CheckResult::fail(id, lowered);
    }
    if !has_branch_data {
        return CheckResult::not_verified(
            id,
            "FV006",
            "the coverage export has no branch data (0 branches in every file); measure with \
             `cargo +nightly-2026-06-13 llvm-cov --branch` (ADR-0002)",
        );
    }
    if files.is_empty() {
        return CheckResult::not_verified(
            id,
            "FV003",
            format!(
                "module `{}` has no measured files; its code did not run under coverage",
                module.name
            ),
        );
    }

    let total = |pick: fn(&FileCoverage) -> Counts| {
        files.iter().map(pick).fold(Counts::default(), Counts::add)
    };
    let lines_ok = total(|f| f.lines).meets(required.line_coverage);
    let branches_ok = total(|f| f.branches).meets(required.branch_coverage);
    files.sort_by_key(|f| std::cmp::Reverse(f.lines.missed() + f.branches.missed()));
    let mut diagnostics = Vec::new();
    for f in &files {
        if !lines_ok && f.lines.missed() > 0 {
            diagnostics.push(shortfall(
                "FV001",
                "lines",
                f,
                f.lines,
                required.line_coverage,
                module,
            ));
        }
        if !branches_ok && f.branches.missed() > 0 {
            diagnostics.push(shortfall(
                "FV002",
                "branches",
                f,
                f.branches,
                required.branch_coverage,
                module,
            ));
        }
    }
    let failed = !diagnostics.is_empty();
    let mut hidden = Vec::new();
    hidden_sources(&module.path.join("src"), Path::new(""), &mut hidden);
    diagnostics.extend(hidden.into_iter().map(hidden_file));
    let status = if failed {
        Status::Fail
    } else if diagnostics.is_empty() {
        Status::Pass
    } else {
        Status::NotVerified
    };
    CheckResult {
        id,
        status,
        diagnostics,
        raw_log: None,
    }
}

/// Collects `.rs` files below `dir` that `cargo llvm-cov`'s default filename
/// filter drops: anything in a `tests`, `examples` or `benches` directory and
/// files named `tests.rs`, `*_tests.rs` or `*-tests.rs` (cargo-llvm-cov 0.8.7,
/// `src/report.rs`). `rel` is the path of `dir` below the module's `src/`.
fn hidden_sources(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let rel = rel.join(&name);
        if path.is_dir() {
            hidden_sources(&path, &rel, out);
        } else if is_hidden_by_default(&rel) {
            out.push(path);
        }
    }
}

fn is_hidden_by_default(rel: &Path) -> bool {
    let Some(stem) = rel
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".rs"))
    else {
        return false;
    };
    let in_hidden_dir = rel.parent().is_some_and(|dir| {
        dir.iter()
            .any(|c| matches!(c.to_str(), Some("tests" | "examples" | "benches")))
    });
    // `tests\.rs|[0-9a-zA-Z_-]+[_-]tests\.rs`
    let tests_file = stem == "tests"
        || stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            && stem.len() > "_tests".len()
            && (stem.ends_with("_tests") || stem.ends_with("-tests"));
    in_hidden_dir || tests_file
}

fn hidden_file(file: PathBuf) -> Diagnostic {
    Diagnostic::error(
        "FV007",
        format!(
            "{} is never measured: cargo llvm-cov drops tests/, examples/, benches/ and *tests.rs files by default",
            file.display()
        ),
    )
    .with_span(Span::line_only(file, 1))
    .with_help("rename it, or move test-only code to the crate's top-level tests/ directory")
}

fn lowered_threshold(module: &Module, field: &str, percent: u8) -> Diagnostic {
    Diagnostic::error(
        "FV008",
        format!(
            "module `{}` declares {field} = {percent}; thresholds below 100 need an owner-approved \
             exclusion ledger entry (ADR-0002), which does not exist before Phase 4",
            module.name
        ),
    )
    .with_help("restore 100 in forge.module.toml and cover the code")
}

fn shortfall(
    code: &str,
    what: &str,
    file: &FileCoverage,
    counts: Counts,
    required: u8,
    module: &Module,
) -> Diagnostic {
    Diagnostic::error(
        code,
        format!(
            "{}: {}/{} {what} covered; module `{}` requires {required}%",
            file.file.display(),
            counts.covered,
            counts.count,
            module.name
        ),
    )
    .with_span(Span::line_only(file.file.clone(), 1))
    .with_help("add tests that execute the missing code; exclusions need an owner-approved ledger entry (ADR-0002)")
}

fn unowned_file(file: PathBuf) -> Diagnostic {
    Diagnostic::error(
        "FV005",
        format!(
            "{} was measured but belongs to no module in forge.toml",
            file.display()
        ),
    )
    .with_span(Span::line_only(file, 1))
    .with_help("register the crate under [modules] in forge.toml, or check that the export comes from this checkout")
}
