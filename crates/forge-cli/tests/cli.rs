//! End-to-end tests of the `forge` binary.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// Variables a coverage run (`cargo +nightly llvm-cov`) sets for its own build;
/// the `cargo fmt`/`cargo clippy` that `forge` spawns in a fixture project must
/// not inherit them, or the fixture runs on the coverage toolchain and flags.
const COVERAGE_RUN_VARS: [&str; 9] = [
    "RUSTUP_TOOLCHAIN",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
];

/// Runs the `forge` binary with an environment that is identical under
/// `cargo nextest run` and under a coverage run. `LLVM_PROFILE_FILE` is kept so
/// the instrumented binary still records its own coverage.
fn forge(args: &[&str], cwd: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge"));
    command.args(args).current_dir(cwd);
    for var in COVERAGE_RUN_VARS {
        command.env_remove(var);
    }
    for (var, _) in std::env::vars_os() {
        if var.to_string_lossy().starts_with("CARGO_LLVM_COV") {
            command.env_remove(var);
        }
    }
    command.output().unwrap()
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A tiny but complete, formatted, lint-clean project with two modules.
fn tiny_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // Same pinned compiler as the repository, whichever toolchain runs the tests.
    write(
        root,
        "rust-toolchain.toml",
        include_str!("../../../rust-toolchain.toml"),
    );
    write(
        root,
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/core\", \"crates/cli\"]\n",
    );
    write(
        root,
        "forge.toml",
        "[project]\nname = \"tiny\"\nmanifest_version = 1\n\n[modules]\ncore = \"crates/core\"\ncli = \"crates/cli\"\n",
    );
    write(
        root,
        "crates/core/forge.module.toml",
        "[module]\nname = \"core\"\nkind = \"domain\"\n",
    );
    write(
        root,
        "crates/core/Cargo.toml",
        "[package]\nname = \"core\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    );
    write(
        root,
        "crates/core/src/lib.rs",
        "//! core\n\n/// Adds one.\n#[must_use]\npub fn inc(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    write(
        root,
        "crates/cli/forge.module.toml",
        "[module]\nname = \"cli\"\nkind = \"cli\"\n\n[dependencies]\nallowed = [\"core\"]\n",
    );
    write(
        root,
        "crates/cli/Cargo.toml",
        "[package]\nname = \"cli\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\ncore = { path = \"../core\" }\n",
    );
    write(
        root,
        "crates/cli/src/main.rs",
        "//! cli\n\nfn main() {\n    println!(\"{}\", core::inc(1));\n}\n",
    );
    dir
}

#[test]
fn json_report_with_skipped_checks_exits_3() {
    let dir = tiny_project();
    let out = forge(
        &[
            "check", "--format", "json", "--skip", "fmt", "--skip", "clippy",
        ],
        dir.path(),
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["tool"], "forge.check");
    assert_eq!(json["status"], "NOT_VERIFIED");
    assert_eq!(json["checks"][0]["id"], "manifests");
    assert_eq!(json["checks"][0]["status"], "PASS");
    assert_eq!(json["checks"][1]["status"], "PASS");
    assert_eq!(json["checks"][2]["status"], "NOT_VERIFIED");
    assert_eq!(json["checks"][3]["diagnostics"][0]["code"], "FSKIP");
}

#[test]
fn text_report_on_a_broken_project_exits_1_with_codes_and_spans() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "forge.toml",
        "[project]\nname = \"x\"\nmanifest_version = 1\n\n[modules]\nghost = \"nowhere\"\n",
    );
    let out = forge(
        &[
            "check",
            "--root",
            dir.path().to_str().unwrap(),
            "--skip",
            "fmt",
            "--skip",
            "clippy",
        ],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("forge.check: FAIL\n"), "{text}");
    assert!(text.contains("[FAIL] manifests"), "{text}");
    assert!(text.contains("error[FM002]"), "{text}");
    assert!(text.contains("forge.toml:6:"), "{text}");
    assert!(text.contains("[NOT_VERIFIED] architecture"), "{text}");
}

#[test]
fn full_run_on_a_clean_project_passes_with_exit_0() {
    let dir = tiny_project();
    let out = forge(&["check", "--format", "json"], dir.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "PASS");
    assert!(
        json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["status"] == "PASS"),
        "{stdout}"
    );
}

#[test]
fn unformatted_code_fails_fmt_with_a_span() {
    let dir = tiny_project();
    write(
        dir.path(),
        "crates/core/src/lib.rs",
        "//! core\n\n/// Adds one.\n#[must_use]\npub fn inc(x: u32) -> u32 {\n        x + 1\n}\n",
    );
    let out = forge(
        &["check", "--format", "json", "--skip", "clippy"],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let fmt = &json["checks"][2];
    assert_eq!(fmt["status"], "FAIL");
    assert_eq!(fmt["diagnostics"][0]["code"], "FF001");
    assert!(
        fmt["diagnostics"][0]["span"]["file"]
            .as_str()
            .unwrap()
            .ends_with("crates/core/src/lib.rs")
    );
}

#[test]
fn clippy_findings_are_structured() {
    let dir = tiny_project();
    write(
        dir.path(),
        "crates/core/src/lib.rs",
        "//! core\n\n/// Adds one.\n#[must_use]\npub fn inc(x: u32) -> u32 {\n    return x + 1;\n}\n",
    );
    let out = forge(&["check", "--format", "json", "--skip", "fmt"], dir.path());
    assert_eq!(out.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let clippy = &json["checks"][3];
    assert_eq!(clippy["status"], "FAIL", "{clippy}");
    let needless = clippy["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "clippy::needless_return")
        .unwrap_or_else(|| panic!("{clippy}"));
    assert_eq!(needless["span"]["line"], 6);
    assert!(
        needless["span"]["file"]
            .as_str()
            .unwrap()
            .ends_with("crates/core/src/lib.rs")
    );
}

#[test]
fn unknown_skip_value_is_a_usage_error() {
    let dir = tiny_project();
    let out = forge(&["check", "--skip", "vibes"], dir.path());
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown check `vibes`"));
}

#[test]
fn version_flag_works() {
    let out = forge(&["--version"], std::env::temp_dir().as_path());
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("forge "));
}
