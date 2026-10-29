//! `forge coverage`: per-module line and branch gate over an llvm-cov export.
#![allow(clippy::unwrap_used)]

mod common;

use std::path::{Path, PathBuf};

use common::{TempProject, valid_project};
use forge_tooling::coverage::{CoverageOptions, run_coverage};
use forge_tooling::report::{CheckReport, CheckResult, Span, Status};
use serde_json::json;

/// One measured file: path, (lines covered, lines count), (branches covered, branches count).
type Measured = (PathBuf, (u64, u64), (u64, u64));

/// Writes an `llvm-cov export -format=json --summary-only` document and returns its path.
fn export(tp: &TempProject, files: &[Measured]) -> PathBuf {
    let files: Vec<_> = files
        .iter()
        .map(|(file, (lc, l), (bc, b))| {
            json!({
                "filename": file,
                "summary": {
                    "lines": {"count": l, "covered": lc, "percent": 0.0},
                    "branches": {"count": b, "covered": bc, "notcovered": b - bc, "percent": 0.0},
                    "functions": {"count": 1, "covered": 1, "percent": 100.0},
                    "regions": {"count": 1, "covered": 1, "notcovered": 0, "percent": 100.0}
                }
            })
        })
        .collect();
    let doc = json!({
        "type": "llvm.coverage.json.export",
        "version": "3.1.0",
        "data": [{"files": files, "totals": {}}]
    });
    tp.write("coverage.json", &doc.to_string())
}

fn src(tp: &TempProject, rel: &str) -> PathBuf {
    tp.root().canonicalize().unwrap().join(rel)
}

fn run(tp: &TempProject, input: &Path) -> CheckReport {
    run_coverage(&CoverageOptions {
        root: tp.root().to_path_buf(),
        input: input.to_path_buf(),
    })
}

fn result<'a>(report: &'a CheckReport, id: &str) -> &'a CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no result {id} in {report:?}"))
}

/// `valid_project` measured completely: both modules, all lines and branches.
fn full(tp: &TempProject) -> Vec<Measured> {
    vec![
        (src(tp, "crates/core/src/lib.rs"), (10, 10), (4, 4)),
        (src(tp, "crates/cli/src/lib.rs"), (5, 5), (0, 0)),
    ]
}

#[test]
fn full_coverage_passes() {
    let tp = valid_project();
    let report = run(&tp, &export(&tp, &full(&tp)));
    assert_eq!(report.tool, "forge.coverage");
    assert_eq!(report.status, Status::Pass, "{report:?}");
    let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["coverage:cli", "coverage:core"]);
}

#[test]
fn missing_branch_fails_with_file_span() {
    let tp = valid_project();
    let mut files = full(&tp);
    files[0].2 = (3, 4);
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(report.status, Status::Fail);
    let core = result(&report, "coverage:core");
    assert_eq!(core.status, Status::Fail);
    assert_eq!(core.diagnostics.len(), 1, "{:?}", core.diagnostics);
    let diag = &core.diagnostics[0];
    assert_eq!(diag.code, "FV002");
    assert!(diag.message.contains("3/4"), "{}", diag.message);
    assert!(
        diag.message.contains("crates/core/src/lib.rs"),
        "{}",
        diag.message
    );
    assert_eq!(
        diag.span,
        Some(Span::line_only(src(&tp, "crates/core/src/lib.rs"), 1))
    );
    assert_eq!(result(&report, "coverage:cli").status, Status::Pass);
}

#[test]
fn missing_line_fails() {
    let tp = valid_project();
    let mut files = full(&tp);
    files[1].1 = (4, 5);
    let report = run(&tp, &export(&tp, &files));
    let cli = result(&report, "coverage:cli");
    assert_eq!(cli.status, Status::Fail);
    assert_eq!(cli.diagnostics[0].code, "FV001");
    assert!(cli.diagnostics[0].message.contains("4/5"));
}

#[test]
fn files_without_branches_count_as_full() {
    let tp = valid_project();
    let files = vec![
        (src(&tp, "crates/core/src/lib.rs"), (1, 1), (0, 0)),
        (src(&tp, "crates/cli/src/lib.rs"), (1, 1), (0, 0)),
    ];
    assert_eq!(run(&tp, &export(&tp, &files)).status, Status::Pass);
}

#[test]
fn thresholds_come_from_module_manifest() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"domain\"\n\n[dependencies]\nforbidden = [\"cli\"]\n\n[verification]\nline_coverage = 90\nbranch_coverage = 75\n",
    );
    let mut files = full(&tp);
    files[0].1 = (9, 10);
    files[0].2 = (3, 4);
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(report.status, Status::Pass, "{report:?}");

    files[0].1 = (8, 10);
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(
        result(&report, "coverage:core").diagnostics[0].code,
        "FV001"
    );
}

#[test]
fn worst_file_is_reported_first() {
    let tp = valid_project();
    tp.write("crates/core/src/a.rs", "");
    let mut files = full(&tp);
    files[0].1 = (9, 10);
    files.push((src(&tp, "crates/core/src/a.rs"), (1, 4), (0, 0)));
    let report = run(&tp, &export(&tp, &files));
    let core = result(&report, "coverage:core");
    let messages: Vec<&str> = core
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(core.diagnostics.len(), 2, "{messages:?}");
    assert!(messages[0].contains("a.rs"), "{messages:?}");
}

#[test]
fn module_without_measured_files_is_not_verified() {
    let tp = valid_project();
    let files = vec![(src(&tp, "crates/core/src/lib.rs"), (10, 10), (4, 4))];
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(report.status, Status::NotVerified);
    let cli = result(&report, "coverage:cli");
    assert_eq!(cli.status, Status::NotVerified);
    assert_eq!(cli.diagnostics[0].code, "FV003");
}

#[test]
fn paths_outside_root_never_pass() {
    let tp = valid_project();
    let files = vec![
        (
            PathBuf::from("/elsewhere/crates/core/src/lib.rs"),
            (10, 10),
            (4, 4),
        ),
        (
            PathBuf::from("/elsewhere/crates/cli/src/lib.rs"),
            (5, 5),
            (0, 0),
        ),
    ];
    let report = run(&tp, &export(&tp, &files));
    assert_ne!(report.status, Status::Pass);
    for id in ["coverage:core", "coverage:cli"] {
        let module = result(&report, id);
        assert_eq!(module.status, Status::NotVerified);
        assert_eq!(module.diagnostics[0].code, "FV003");
    }
    let unowned = result(&report, "coverage:unowned");
    assert_eq!(unowned.status, Status::Fail);
    assert_eq!(unowned.diagnostics.len(), 2);
    assert!(unowned.diagnostics.iter().all(|d| d.code == "FV005"));
}

#[test]
fn invalid_input_is_not_verified() {
    let tp = valid_project();
    let not_an_export = tp.write("coverage.json", r#"{"type":"x"}"#);
    let missing = tp.root().join("missing.json");
    for input in [not_an_export, missing] {
        let report = run(&tp, &input);
        assert_eq!(report.status, Status::NotVerified, "{report:?}");
        for id in ["coverage:core", "coverage:cli"] {
            let module = result(&report, id);
            assert_eq!(module.status, Status::NotVerified);
            assert_eq!(module.diagnostics[0].code, "FV004");
        }
    }
}

#[test]
fn module_paths_do_not_match_by_string_prefix() {
    let tp = TempProject::new();
    tp.write(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/a\", \"crates/ab\"]\n",
    );
    tp.forge_toml(&[("a", "crates/a"), ("ab", "crates/ab")]);
    tp.module("a", "crates/a", &[], &[], "");
    tp.module("ab", "crates/ab", &[], &[], "");
    let files = vec![
        (src(&tp, "crates/a/src/lib.rs"), (1, 1), (0, 0)),
        (src(&tp, "crates/ab/src/lib.rs"), (1, 2), (0, 0)),
    ];
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(result(&report, "coverage:a").status, Status::Pass);
    assert_eq!(result(&report, "coverage:ab").status, Status::Fail);
}

#[test]
fn broken_manifest_fails_without_reading_coverage() {
    let tp = TempProject::new();
    tp.write("forge.toml", "not toml at all = = =");
    let report = run(&tp, &tp.root().join("missing.json"));
    assert_eq!(report.status, Status::Fail);
    let manifests = result(&report, "manifests");
    assert_eq!(manifests.diagnostics[0].code, "FM001");
}
