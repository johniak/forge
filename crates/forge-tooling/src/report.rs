//! Structured, versioned report types shared by every tool (SPEC §8.1, §8.5, §13.1).

use std::fmt::Write as _;
use std::ops::Range;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Schema version of every JSON document produced by the tooling core.
pub const SCHEMA_VERSION: u32 = 1;

/// Outcome of a single check or of a whole report.
///
/// `NotVerified` is distinct from `Fail`: it means the check could not run
/// (missing tool, missing environment, skipped on request) and must never be
/// reported as a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    /// The check ran and found no problems.
    Pass,
    /// The check ran and found problems.
    Fail,
    /// The check could not run; treated as a failure by every gate.
    NotVerified,
}

impl Status {
    /// Folds a list of statuses into an overall verdict.
    ///
    /// Any `Fail` wins over `NotVerified`, which wins over `Pass`. An empty
    /// list is `NotVerified` because nothing was checked.
    #[must_use]
    pub fn overall<I: IntoIterator<Item = Status>>(statuses: I) -> Status {
        let mut overall = None;
        for status in statuses {
            overall = Some(match (overall, status) {
                (_, Status::Fail) | (Some(Status::Fail), _) => Status::Fail,
                (_, Status::NotVerified) | (Some(Status::NotVerified), _) => Status::NotVerified,
                (None | Some(Status::Pass), Status::Pass) => Status::Pass,
            });
        }
        overall.unwrap_or(Status::NotVerified)
    }

    /// `true` only for [`Status::Pass`].
    #[must_use]
    pub fn is_pass(self) -> bool {
        self == Status::Pass
    }

    /// Stable upper-case name, identical to the JSON encoding.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NotVerified => "NOT_VERIFIED",
        }
    }
}

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Makes the owning check fail.
    Error,
    /// Reported, does not fail the check on its own.
    Warning,
}

/// A location in a source file. `line` and `column` are 1-based; `column`
/// counts characters, `start`/`end` are byte offsets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Absolute path when known, otherwise the path as the producing tool printed it.
    pub file: PathBuf,
    /// 1-based line.
    pub line: u32,
    /// 1-based character column.
    pub column: u32,
    /// Byte offset of the start of the span.
    pub start: usize,
    /// Byte offset one past the end of the span.
    pub end: usize,
}

impl Span {
    /// Builds a span from a byte range inside `text`.
    #[must_use]
    pub fn from_byte_range(file: PathBuf, text: &str, range: Range<usize>) -> Span {
        let (line, column) = line_column(text, range.start);
        Span {
            file,
            line,
            column,
            start: range.start,
            end: range.end,
        }
    }

    /// A span that names a line but nothing more precise.
    #[must_use]
    pub fn line_only(file: PathBuf, line: u32) -> Span {
        Span {
            file,
            line,
            column: 1,
            start: 0,
            end: 0,
        }
    }
}

/// 1-based `(line, column)` of a byte offset in `text`; the column counts
/// characters. Offsets past the end or inside a multi-byte character are
/// clamped to the nearest valid boundary before them.
#[must_use]
pub fn line_column(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..text.floor_char_boundary(offset)];
    let line_start = before.rfind('\n').map_or(0, |p| p + 1);
    let line = before.matches('\n').count() + 1;
    let column = before[line_start..].chars().count() + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

/// One finding of a check, with a stable code and an optional source span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// Stable identifier such as `FM001` or a compiler lint name.
    pub code: String,
    /// Human-readable, one line.
    pub message: String,
    /// Where it happened, when known.
    pub span: Option<Span>,
    /// Suggested remediation, when known.
    pub help: Option<String>,
}

impl Diagnostic {
    /// An error-level diagnostic without span.
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            span: None,
            help: None,
        }
    }

    /// A warning-level diagnostic without span.
    #[must_use]
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(code, message)
        }
    }

    /// Attaches a span.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches a help text.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// Result of one named check inside a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    /// Stable check identifier (`manifests`, `fmt`, …).
    pub id: String,
    /// Verdict of this check alone.
    pub status: Status,
    /// Findings; may be non-empty even for `Pass` (warnings).
    pub diagnostics: Vec<Diagnostic>,
    /// Unmodified tool output, kept for debugging (SPEC §8.5).
    pub raw_log: Option<String>,
}

impl CheckResult {
    /// A passing check with no findings.
    #[must_use]
    pub fn pass(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Pass,
            diagnostics: Vec::new(),
            raw_log: None,
        }
    }

    /// A failing check carrying its findings.
    #[must_use]
    pub fn fail(id: impl Into<String>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            id: id.into(),
            status: Status::Fail,
            diagnostics,
            raw_log: None,
        }
    }

    /// A check that could not run, with the reason as a single diagnostic.
    #[must_use]
    pub fn not_verified(id: impl Into<String>, code: &str, reason: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::NotVerified,
            diagnostics: vec![Diagnostic::error(code, reason)],
            raw_log: None,
        }
    }

    /// Attaches the raw tool output.
    #[must_use]
    pub fn with_raw_log(mut self, raw: impl Into<String>) -> Self {
        self.raw_log = Some(raw.into());
        self
    }
}

/// A complete, versioned report of one tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    /// Always [`SCHEMA_VERSION`] for documents produced by this crate.
    pub schema_version: u32,
    /// Tool identifier, e.g. `forge.check`.
    pub tool: String,
    /// Overall verdict, folded with [`Status::overall`].
    pub status: Status,
    /// Individual checks in execution order.
    pub checks: Vec<CheckResult>,
}

impl CheckReport {
    /// Builds a report, computing the overall status from the checks.
    #[must_use]
    pub fn new(tool: impl Into<String>, checks: Vec<CheckResult>) -> Self {
        let status = Status::overall(checks.iter().map(|c| c.status));
        Self {
            schema_version: SCHEMA_VERSION,
            tool: tool.into(),
            status,
            checks,
        }
    }

    /// Compact human-readable rendering (the CLI's default output).
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut out = format!("{}: {}\n", self.tool, self.status.as_str());
        for check in &self.checks {
            let _ = writeln!(out, "  [{}] {}", check.status.as_str(), check.id);
            for d in &check.diagnostics {
                let severity = match d.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                };
                let _ = writeln!(out, "      {severity}[{}]: {}", d.code, d.message);
                if let Some(span) = &d.span {
                    let _ = writeln!(
                        out,
                        "        --> {}:{}:{}",
                        span.file.display(),
                        span.line,
                        span.column
                    );
                }
                if let Some(help) = &d.help {
                    let _ = writeln!(out, "        help: {help}");
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn empty_is_not_verified() {
        assert_eq!(Status::overall([]), Status::NotVerified);
    }

    #[test]
    fn all_pass_is_pass() {
        assert_eq!(Status::overall([Status::Pass, Status::Pass]), Status::Pass);
    }

    #[test]
    fn fail_wins_over_everything() {
        assert_eq!(
            Status::overall([
                Status::Pass,
                Status::NotVerified,
                Status::Fail,
                Status::Pass
            ]),
            Status::Fail
        );
        assert_eq!(
            Status::overall([Status::Fail, Status::NotVerified]),
            Status::Fail
        );
    }

    #[test]
    fn not_verified_wins_over_pass() {
        assert_eq!(
            Status::overall([Status::Pass, Status::NotVerified, Status::Pass]),
            Status::NotVerified
        );
        assert_eq!(
            Status::overall([Status::NotVerified, Status::Pass]),
            Status::NotVerified
        );
    }

    #[test]
    fn is_pass_only_for_pass() {
        assert!(Status::Pass.is_pass());
        assert!(!Status::Fail.is_pass());
        assert!(!Status::NotVerified.is_pass());
    }

    #[test]
    fn status_names_match_json_encoding() {
        for status in [Status::Pass, Status::Fail, Status::NotVerified] {
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{}\"", status.as_str())
            );
        }
        let back: Status = serde_json::from_str("\"FAIL\"").unwrap();
        assert_eq!(back, Status::Fail);
    }

    #[test]
    fn line_column_counts_characters_and_clamps() {
        let text = "ab\nżółw = 1\n";
        assert_eq!(line_column(text, 0), (1, 1));
        assert_eq!(line_column(text, 3), (2, 1));
        // "żółw" is 4 chars but 7 bytes; offset of " = 1" is 3 + 7 = 10.
        assert_eq!(line_column(text, 10), (2, 5));
        // inside the two-byte "ż" (bytes 3..5): clamps back to its start
        assert_eq!(line_column(text, 4), (2, 1));
        // past the end: clamps to the end
        assert_eq!(line_column(text, 999), (3, 1));
    }

    #[test]
    fn span_from_byte_range_and_line_only() {
        let span = Span::from_byte_range(PathBuf::from("/f"), "a\nbc", 2..4);
        assert_eq!(
            span,
            Span {
                file: PathBuf::from("/f"),
                line: 2,
                column: 1,
                start: 2,
                end: 4
            }
        );
        let span = Span::line_only(PathBuf::from("/g"), 9);
        assert_eq!((span.line, span.column, span.start, span.end), (9, 1, 0, 0));
    }

    #[test]
    fn diagnostic_builders() {
        let d = Diagnostic::warning("W1", "m")
            .with_help("h")
            .with_span(Span::line_only("/f".into(), 1));
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.help.as_deref(), Some("h"));
        assert_eq!(d.span.unwrap().line, 1);
        assert_eq!(Diagnostic::error("E1", "x").severity, Severity::Error);
    }

    #[test]
    fn check_result_builders() {
        assert_eq!(CheckResult::pass("a").status, Status::Pass);
        let f = CheckResult::fail("b", vec![Diagnostic::error("E", "m")]).with_raw_log("raw");
        assert_eq!(f.status, Status::Fail);
        assert_eq!(f.raw_log.as_deref(), Some("raw"));
        let nv = CheckResult::not_verified("c", "FSKIP", "skipped");
        assert_eq!(nv.status, Status::NotVerified);
        assert_eq!(nv.diagnostics[0].code, "FSKIP");
    }

    #[test]
    fn report_text_rendering_lists_every_detail() {
        let report = CheckReport::new(
            "forge.check",
            vec![
                CheckResult::pass("manifests"),
                CheckResult::fail(
                    "architecture",
                    vec![
                        Diagnostic::error("FA001", "bad dep")
                            .with_span(Span::line_only("/p/Cargo.toml".into(), 7))
                            .with_help("add it to allowed"),
                        Diagnostic::warning("W", "meh"),
                    ],
                ),
                CheckResult::not_verified("fmt", "FSKIP", "skipped by request"),
            ],
        );
        assert_eq!(report.status, Status::Fail);
        let text = report.render_text();
        let expected = "forge.check: FAIL\n  [PASS] manifests\n  [FAIL] architecture\n      error[FA001]: bad dep\n        --> /p/Cargo.toml:7:1\n        help: add it to allowed\n      warning[W]: meh\n  [NOT_VERIFIED] fmt\n      error[FSKIP]: skipped by request\n";
        assert_eq!(text, expected);
    }
}
