//! `forge check` orchestration with a scripted command runner.
#![allow(clippy::unwrap_used)]

mod common;

use std::cell::RefCell;
use std::io;
use std::path::Path;

use common::{TempProject, valid_project};
use forge_tooling::check::{CheckId, CheckOptions, run_check};
use forge_tooling::process::{CommandOutput, CommandRunner};
use forge_tooling::report::{SCHEMA_VERSION, Severity, Status};

/// Scripted runner: answers by program name, records every call.
struct FakeRunner {
    fmt: io::Result<CommandOutput>,
    clippy: io::Result<CommandOutput>,
    calls: RefCell<Vec<(String, Vec<String>, String)>>,
}

impl FakeRunner {
    fn new(fmt: io::Result<CommandOutput>, clippy: io::Result<CommandOutput>) -> Self {
        Self {
            fmt,
            clippy,
            calls: RefCell::new(Vec::new()),
        }
    }
}

fn ok(stdout: &str) -> io::Result<CommandOutput> {
    Ok(CommandOutput {
        status: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    })
}

fn failed(code: i32, stdout: &str, stderr: &str) -> io::Result<CommandOutput> {
    Ok(CommandOutput {
        status: Some(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    })
}

fn clone_result(r: &io::Result<CommandOutput>) -> io::Result<CommandOutput> {
    match r {
        Ok(o) => Ok(o.clone()),
        Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> io::Result<CommandOutput> {
        self.calls.borrow_mut().push((
            program.to_string(),
            args.iter().map(ToString::to_string).collect(),
            cwd.display().to_string(),
        ));
        match args.first().copied() {
            Some("fmt") => clone_result(&self.fmt),
            Some("clippy") => clone_result(&self.clippy),
            other => panic!("unexpected command {program} {other:?}"),
        }
    }
}

fn opts(tp: &TempProject) -> CheckOptions {
    CheckOptions {
        root: tp.root().to_path_buf(),
        skip: Vec::new(),
    }
}

fn by_id<'a>(
    report: &'a forge_tooling::report::CheckReport,
    id: &str,
) -> &'a forge_tooling::report::CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

#[test]
fn everything_green_is_pass_and_runs_cargo_in_the_project_root() {
    let tp = valid_project();
    let runner = FakeRunner::new(ok(""), ok(""));
    let report = run_check(&opts(&tp), &runner);
    assert_eq!(report.status, Status::Pass);
    assert_eq!(report.schema_version, SCHEMA_VERSION);
    assert_eq!(report.tool, "forge.check");
    let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["manifests", "architecture", "fmt", "clippy"]);
    assert!(report.checks.iter().all(|c| c.status == Status::Pass));
    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "cargo");
    assert_eq!(calls[0].1, vec!["fmt", "--all", "--", "--check"]);
    assert_eq!(
        calls[0].2,
        tp.root().canonicalize().unwrap().display().to_string()
    );
    assert_eq!(
        calls[1].1,
        vec![
            "clippy",
            "--workspace",
            "--all-targets",
            "--message-format=json",
            "--",
            "-D",
            "warnings"
        ]
    );
}

#[test]
fn manifest_failure_fails_and_leaves_architecture_not_verified() {
    let tp = TempProject::new();
    let runner = FakeRunner::new(ok(""), ok(""));
    let report = run_check(&opts(&tp), &runner);
    assert_eq!(report.status, Status::Fail);
    assert_eq!(by_id(&report, "manifests").status, Status::Fail);
    assert_eq!(by_id(&report, "manifests").diagnostics[0].code, "FM001");
    let arch = by_id(&report, "architecture");
    assert_eq!(arch.status, Status::NotVerified);
    assert!(arch.diagnostics[0].message.contains("manifests"));
    assert_eq!(by_id(&report, "fmt").status, Status::Pass, "fmt still runs");
}

#[test]
fn architecture_violation_fails() {
    let tp = valid_project();
    tp.module(
        "cli",
        "crates/cli",
        &[],
        &[],
        "core = { path = \"../core\" }",
    );
    let report = run_check(&opts(&tp), &FakeRunner::new(ok(""), ok("")));
    assert_eq!(report.status, Status::Fail);
    assert_eq!(by_id(&report, "architecture").diagnostics[0].code, "FA001");
}

#[test]
fn fmt_diff_lines_become_diagnostics_with_spans() {
    let tp = valid_project();
    let diff = "Diff in /abs/crates/core/src/lib.rs:3:\n fn main() {\n-    let x=1;\n+    let x = 1;\n }\nDiff in /abs/crates/cli/src/main.rs at line 10:\n-a\n+b\n";
    let report = run_check(&opts(&tp), &FakeRunner::new(failed(1, diff, ""), ok("")));
    assert_eq!(report.status, Status::Fail);
    let fmt = by_id(&report, "fmt");
    assert_eq!(fmt.status, Status::Fail);
    assert_eq!(fmt.diagnostics.len(), 2);
    assert_eq!(fmt.diagnostics[0].code, "FF001");
    let s0 = fmt.diagnostics[0].span.as_ref().unwrap();
    assert_eq!(s0.file.to_str().unwrap(), "/abs/crates/core/src/lib.rs");
    assert_eq!(s0.line, 3);
    let s1 = fmt.diagnostics[1].span.as_ref().unwrap();
    assert_eq!(s1.file.to_str().unwrap(), "/abs/crates/cli/src/main.rs");
    assert_eq!(s1.line, 10);
    assert_eq!(fmt.raw_log.as_deref(), Some(diff));
}

#[test]
fn fmt_nonzero_exit_without_diff_is_still_a_failure_with_stderr() {
    let tp = valid_project();
    let report = run_check(
        &opts(&tp),
        &FakeRunner::new(failed(101, "", "error: no such subcommand"), ok("")),
    );
    let fmt = by_id(&report, "fmt");
    assert_eq!(fmt.status, Status::Fail);
    assert_eq!(fmt.diagnostics.len(), 1);
    assert_eq!(fmt.diagnostics[0].code, "FF000");
    assert!(fmt.diagnostics[0].message.contains("101"));
    assert!(
        fmt.raw_log
            .as_deref()
            .unwrap()
            .contains("no such subcommand")
    );
}

#[test]
fn io_error_launching_cargo_is_not_verified() {
    let tp = valid_project();
    let err = || Err(io::Error::new(io::ErrorKind::NotFound, "cargo not found"));
    let report = run_check(&opts(&tp), &FakeRunner::new(err(), err()));
    assert_eq!(report.status, Status::NotVerified);
    for id in ["fmt", "clippy"] {
        let c = by_id(&report, id);
        assert_eq!(c.status, Status::NotVerified);
        assert!(c.diagnostics[0].message.contains("cargo not found"));
    }
}

const CLIPPY_ERROR: &str = r#"{"reason":"compiler-artifact","package_id":"x","target":{"name":"core"},"fresh":true}
{"reason":"compiler-message","package_id":"path+file:///p/crates/core#0.0.0","manifest_path":"/p/crates/core/Cargo.toml","target":{"kind":["lib"],"name":"core","src_path":"/p/crates/core/src/lib.rs"},"message":{"$message_type":"diagnostic","message":"unneeded `return` statement","code":{"code":"clippy::needless_return","explanation":null},"level":"error","spans":[{"file_name":"crates/core/src/lib.rs","byte_start":5,"byte_end":10,"line_start":3,"line_end":3,"column_start":5,"column_end":7,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[{"message":"remove `return`","code":null,"level":"help","spans":[],"children":[],"rendered":null}],"rendered":"error: unneeded `return` statement\n"}}
{"reason":"compiler-message","package_id":"x","manifest_path":"/p/Cargo.toml","target":{"kind":["lib"],"name":"core","src_path":"/p/crates/core/src/lib.rs"},"message":{"$message_type":"diagnostic","message":"unused variable: `y`","code":{"code":"unused_variables","explanation":null},"level":"warning","spans":[{"file_name":"/p/crates/core/src/lib.rs","byte_start":1,"byte_end":2,"line_start":9,"line_end":9,"column_start":9,"column_end":10,"is_primary":false,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null},{"file_name":"/p/crates/core/src/lib.rs","byte_start":1,"byte_end":2,"line_start":8,"line_end":8,"column_start":9,"column_end":10,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":"warning: unused variable\n"}}
{"reason":"compiler-message","package_id":"x","manifest_path":"/p/Cargo.toml","target":{"kind":["lib"],"name":"core","src_path":"/p/crates/core/src/lib.rs"},"message":{"$message_type":"diagnostic","message":"aborting due to 1 previous error","code":null,"level":"error","spans":[],"children":[],"rendered":"error: aborting due to 1 previous error\n"}}
{"reason":"build-finished","success":false}
"#;

#[test]
fn clippy_json_messages_become_structured_diagnostics() {
    let tp = valid_project();
    let report = run_check(
        &opts(&tp),
        &FakeRunner::new(ok(""), failed(101, CLIPPY_ERROR, "")),
    );
    assert_eq!(report.status, Status::Fail);
    let clippy = by_id(&report, "clippy");
    assert_eq!(clippy.status, Status::Fail);
    assert_eq!(
        clippy.diagnostics.len(),
        2,
        "the summary line without spans is dropped: {:?}",
        clippy.diagnostics
    );

    let err = &clippy.diagnostics[0];
    assert_eq!(err.severity, Severity::Error);
    assert_eq!(err.code, "clippy::needless_return");
    assert_eq!(err.message, "unneeded `return` statement");
    assert_eq!(err.help.as_deref(), Some("remove `return`"));
    let span = err.span.as_ref().unwrap();
    assert_eq!(
        span.file,
        tp.root()
            .canonicalize()
            .unwrap()
            .join("crates/core/src/lib.rs")
    );
    assert_eq!(
        (span.line, span.column, span.start, span.end),
        (3, 5, 5, 10)
    );

    let warn = &clippy.diagnostics[1];
    assert_eq!(warn.severity, Severity::Warning);
    assert_eq!(warn.code, "unused_variables");
    assert_eq!(warn.span.as_ref().unwrap().line, 8, "primary span wins");
    assert_eq!(
        warn.span.as_ref().unwrap().file.to_str().unwrap(),
        "/p/crates/core/src/lib.rs"
    );
    assert!(
        clippy
            .raw_log
            .as_deref()
            .unwrap()
            .contains("needless_return")
    );
}

#[test]
fn clippy_warnings_only_with_zero_exit_pass_but_are_reported() {
    let tp = valid_project();
    let only_warning = CLIPPY_ERROR.lines().nth(2).unwrap().to_string() + "\n";
    let report = run_check(&opts(&tp), &FakeRunner::new(ok(""), ok(&only_warning)));
    let clippy = by_id(&report, "clippy");
    assert_eq!(clippy.status, Status::Pass);
    assert_eq!(clippy.diagnostics.len(), 1);
    assert_eq!(clippy.diagnostics[0].severity, Severity::Warning);
}

#[test]
fn clippy_nonzero_exit_without_messages_fails_with_generic_code() {
    let tp = valid_project();
    let report = run_check(
        &opts(&tp),
        &FakeRunner::new(ok(""), failed(101, "not json at all\n", "boom")),
    );
    let clippy = by_id(&report, "clippy");
    assert_eq!(clippy.status, Status::Fail);
    assert_eq!(clippy.diagnostics.len(), 1);
    assert_eq!(clippy.diagnostics[0].code, "FC000");
    assert!(clippy.raw_log.as_deref().unwrap().contains("boom"));
}

#[test]
fn clippy_message_without_code_uses_generic_code() {
    let tp = valid_project();
    let line = r#"{"reason":"compiler-message","package_id":"x","manifest_path":"/p/Cargo.toml","target":{"kind":["lib"],"name":"core","src_path":"/p/x.rs"},"message":{"$message_type":"diagnostic","message":"mismatched types","code":null,"level":"error","spans":[{"file_name":"/p/x.rs","byte_start":0,"byte_end":1,"line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":"x"}}
"#;
    let report = run_check(&opts(&tp), &FakeRunner::new(ok(""), failed(101, line, "")));
    let d = &by_id(&report, "clippy").diagnostics[0];
    assert_eq!(d.code, "FC001");
    assert_eq!(d.help, None);
}

#[test]
fn skipped_checks_are_not_verified_and_never_run() {
    let tp = valid_project();
    let runner = FakeRunner::new(ok(""), ok(""));
    let options = CheckOptions {
        root: tp.root().to_path_buf(),
        skip: vec![CheckId::Fmt, CheckId::Clippy],
    };
    let report = run_check(&options, &runner);
    assert_eq!(report.status, Status::NotVerified);
    assert_eq!(by_id(&report, "fmt").status, Status::NotVerified);
    assert!(
        by_id(&report, "fmt").diagnostics[0]
            .message
            .contains("skipped")
    );
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn skipping_manifests_also_blocks_architecture() {
    let tp = valid_project();
    let options = CheckOptions {
        root: tp.root().to_path_buf(),
        skip: vec![CheckId::Manifests],
    };
    let report = run_check(&options, &FakeRunner::new(ok(""), ok("")));
    assert_eq!(by_id(&report, "manifests").status, Status::NotVerified);
    assert_eq!(by_id(&report, "architecture").status, Status::NotVerified);
}

#[test]
fn check_ids_parse_from_strings() {
    assert_eq!("fmt".parse::<CheckId>().unwrap(), CheckId::Fmt);
    assert_eq!("clippy".parse::<CheckId>().unwrap(), CheckId::Clippy);
    assert_eq!("manifests".parse::<CheckId>().unwrap(), CheckId::Manifests);
    assert_eq!(
        "architecture".parse::<CheckId>().unwrap(),
        CheckId::Architecture
    );
    assert!("nope".parse::<CheckId>().is_err());
    assert_eq!(CheckId::Architecture.to_string(), "architecture");
}

#[test]
fn report_serializes_with_schema_version_and_statuses() {
    let tp = valid_project();
    let report = run_check(&opts(&tp), &FakeRunner::new(ok(""), ok("")));
    let json: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["status"], "PASS");
    assert_eq!(json["checks"][0]["id"], "manifests");
}

#[test]
fn nonexistent_root_is_reported_not_panicked() {
    let options = CheckOptions {
        root: Path::new("/definitely/not/here").to_path_buf(),
        skip: Vec::new(),
    };
    let report = run_check(&options, &FakeRunner::new(ok(""), ok("")));
    assert_eq!(report.status, Status::Fail);
    assert_eq!(by_id(&report, "manifests").diagnostics[0].code, "FM001");
}
