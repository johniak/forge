//! The real Docker starter, pointed at a Docker daemon that does not exist.
//!
//! `DOCKER_HOST` can only be changed for a whole process (setting it in-process
//! is `unsafe` in edition 2024), so the test re-runs its own binary as a probe
//! with the variable set and checks the probe's verdict.
#![allow(clippy::unwrap_used)]

use std::process::Command;

use forge_testing::{ContainerStarter, DockerStarter, HarnessError};

const PROBE: &str = "FORGE_TESTING_DOCKER_PROBE";
const TEST: &str = "docker_starter_reports_an_unreachable_daemon";

#[tokio::test]
async fn docker_starter_reports_an_unreachable_daemon() {
    if std::env::var_os(PROBE).is_some() {
        let err = DockerStarter.start().await.unwrap_err();
        assert!(matches!(err, HarnessError::Container(_)), "{err:?}");
        return;
    }
    let probe = Command::new(std::env::current_exe().unwrap())
        .args([TEST, "--exact", "--nocapture"])
        .env(PROBE, "1")
        .env(
            "DOCKER_HOST",
            "unix:///nonexistent/forge-testing/docker.sock",
        )
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&probe.stdout);
    assert!(
        probe.status.success(),
        "probe failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the probe must have run the test:\n{stdout}"
    );
}
