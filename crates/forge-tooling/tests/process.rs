//! The real command runner.
#![allow(clippy::unwrap_used)]

use forge_tooling::process::{CommandRunner, SystemRunner};

#[test]
fn runs_a_real_program_and_captures_output() {
    let out = SystemRunner
        .run("rustc", &["--version"], std::env::temp_dir().as_path())
        .unwrap();
    assert_eq!(out.status, Some(0));
    assert!(out.stdout.starts_with("rustc "), "{}", out.stdout);
}

#[test]
fn nonzero_exit_and_stderr_are_captured() {
    let out = SystemRunner
        .run(
            "rustc",
            &["--definitely-not-a-flag"],
            std::env::temp_dir().as_path(),
        )
        .unwrap();
    assert_ne!(out.status, Some(0));
    assert!(!out.stderr.is_empty());
}

#[test]
fn missing_program_is_an_io_error() {
    let err = SystemRunner
        .run(
            "forge-no-such-program-xyz",
            &[],
            std::env::temp_dir().as_path(),
        )
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
