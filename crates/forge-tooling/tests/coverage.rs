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
    // A file with no branches is fully covered as long as the export carries
    // branch data at all (see export_without_branch_instrumentation_is_not_verified).
    let tp = valid_project();
    let files = vec![
        (src(&tp, "crates/core/src/lib.rs"), (1, 1), (2, 2)),
        (src(&tp, "crates/cli/src/lib.rs"), (1, 1), (0, 0)),
    ];
    assert_eq!(run(&tp, &export(&tp, &files)).status, Status::Pass);
}

#[test]
fn module_with_its_library_outside_src_is_gated_on_its_files() {
    // `[lib] path = "lib.rs"`: there is no src/ directory to scan for hidden files.
    let tp = valid_project();
    std::fs::remove_file(tp.root().join("crates/cli/src/lib.rs")).unwrap();
    std::fs::remove_dir(tp.root().join("crates/cli/src")).unwrap();
    tp.write("crates/cli/lib.rs", "pub fn f() {}\n");
    let files = vec![
        (src(&tp, "crates/core/src/lib.rs"), (1, 1), (2, 2)),
        (src(&tp, "crates/cli/lib.rs"), (1, 1), (0, 0)),
    ];
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(
        result(&report, "coverage:cli").status,
        Status::Pass,
        "{report:?}"
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
        (src(&tp, "crates/a/src/lib.rs"), (1, 1), (2, 2)),
        (src(&tp, "crates/ab/src/lib.rs"), (1, 2), (2, 2)),
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

#[test]
fn export_of_another_type_is_not_verified() {
    let tp = valid_project();
    let input = tp.write("coverage.json", r#"{"type":"lcov","data":[]}"#);
    let report = run(&tp, &input);
    assert_eq!(report.status, Status::NotVerified);
    let core = result(&report, "coverage:core");
    assert_eq!(core.diagnostics[0].code, "FV004");
    assert!(
        core.diagnostics[0].message.contains("`lcov`"),
        "{:?}",
        core.diagnostics
    );
}

#[test]
fn each_file_is_reported_only_for_its_own_shortfall() {
    let tp = valid_project();
    tp.write("crates/core/src/a.rs", "");
    let mut files = full(&tp);
    files[0].2 = (3, 4);
    files.push((src(&tp, "crates/core/src/a.rs"), (1, 2), (2, 2)));
    let report = run(&tp, &export(&tp, &files));
    let core = result(&report, "coverage:core");
    let found: Vec<(&str, bool)> = core
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.message.contains("a.rs")))
        .collect();
    assert_eq!(found.len(), 2, "{:?}", core.diagnostics);
    assert!(found.contains(&("FV001", true)), "{found:?}");
    assert!(found.contains(&("FV002", false)), "{found:?}");
}

#[test]
fn export_without_branch_instrumentation_is_not_verified() {
    // `cargo llvm-cov` without `--branch` (or on stable) writes 0/0 branches
    // for every file; that is missing data, not full branch coverage.
    let tp = valid_project();
    let files = vec![
        (src(&tp, "crates/core/src/lib.rs"), (10, 10), (0, 0)),
        (src(&tp, "crates/cli/src/lib.rs"), (5, 5), (0, 0)),
    ];
    let report = run(&tp, &export(&tp, &files));
    assert_eq!(report.status, Status::NotVerified, "{report:?}");
    for id in ["coverage:core", "coverage:cli"] {
        let module = result(&report, id);
        assert_eq!(module.status, Status::NotVerified);
        assert_eq!(module.diagnostics[0].code, "FV006");
        assert!(module.diagnostics[0].message.contains("--branch"));
    }
}

#[test]
fn files_hidden_by_the_llvm_cov_default_filter_are_not_verified() {
    // cargo-llvm-cov 0.8.7 drops `tests/`, `examples/` and `benches/`
    // directories and `tests.rs` / `<name>_tests.rs` / `<name>-tests.rs` files
    // anywhere in the workspace by default.
    let tp = valid_project();
    let hidden = [
        "crates/core/src/tests/mod.rs",
        "crates/core/src/examples/demo.rs",
        "crates/core/src/benches/bench.rs",
        "crates/core/src/tests.rs",
        "crates/core/src/model_tests.rs",
        "crates/core/src/model-tests.rs",
        "crates/core/src/__tests.rs",
    ];
    let measured = [
        "crates/core/src/_tests.rs",          // needs a name before `_tests`
        "crates/core/src/modèl_tests.rs",     // the name must be [0-9a-zA-Z_-]
        "crates/core/src/latests.rs",         // no separator before `tests`
        "crates/core/src/tests/fixture.json", // not Rust
        "crates/core/tests/integration.rs",   // the crate's own test target
    ];
    for rel in hidden.iter().chain(&measured) {
        tp.write(rel, "pub fn f() {}\n");
    }
    let report = run(&tp, &export(&tp, &full(&tp)));
    assert_eq!(report.status, Status::NotVerified, "{report:?}");
    let core = result(&report, "coverage:core");
    assert_eq!(core.status, Status::NotVerified);
    assert!(core.diagnostics.iter().all(|d| d.code == "FV007"));
    let mut reported: Vec<PathBuf> = core
        .diagnostics
        .iter()
        .map(|d| d.span.as_ref().unwrap().file.clone())
        .collect();
    reported.sort();
    let mut expected: Vec<PathBuf> = hidden.iter().map(|rel| src(&tp, rel)).collect();
    expected.sort();
    assert_eq!(reported, expected);
    assert_eq!(result(&report, "coverage:cli").status, Status::Pass);
}

#[test]
fn lowered_threshold_fails_without_approval() {
    let tp = valid_project();
    tp.write(
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"domain\"\n\n[dependencies]\nforbidden = [\"cli\"]\n\n[verification]\nline_coverage = 90\n",
    );
    let report = run(&tp, &export(&tp, &full(&tp)));
    assert_eq!(report.status, Status::Fail, "{report:?}");
    let core = result(&report, "coverage:core");
    assert_eq!(core.diagnostics[0].code, "FV008");
    assert!(core.diagnostics[0].message.contains("90"));
}
