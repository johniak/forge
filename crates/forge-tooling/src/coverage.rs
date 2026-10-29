//! `forge coverage`: per-module line and branch coverage gate over the JSON
//! export of `cargo llvm-cov --json --summary-only` (ADR-0002 amendment).
//!
//! `cargo llvm-cov` has no branch threshold of its own; this gate enforces the
//! `[verification]` thresholds every module declares in `forge.module.toml`.
//! Code that did not run under coverage is never counted as covered: a module
//! without measured files is `NOT_VERIFIED`, and a measured file that belongs to
//! no module fails the report.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::manifest::{Module, load_project};
use crate::report::{CheckReport, CheckResult, Diagnostic, Span};

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
        .map(|(module, files)| module_result(module, files))
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

fn module_result(module: &Module, mut files: Vec<FileCoverage>) -> CheckResult {
    let id = result_id(module);
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
    let required = module.verification;
    let lines_ok = total(|f| f.lines).meets(required.line_coverage);
    let branches_ok = total(|f| f.branches).meets(required.branch_coverage);
    if lines_ok && branches_ok {
        return CheckResult::pass(id);
    }

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
    CheckResult::fail(id, diagnostics)
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
