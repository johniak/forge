//! Server resolution, seeds and readiness — no PostgreSQL needed.
#![allow(clippy::unwrap_used)]

use std::cell::Cell;
use std::time::Duration;

use forge_testing::{ContainerStarter, HarnessError, Seed, resolve_server, wait_ready};

/// Answers with a fixed result and counts calls.
struct FakeStarter {
    answer: Result<&'static str, &'static str>,
    calls: Cell<u32>,
}

impl FakeStarter {
    fn new(answer: Result<&'static str, &'static str>) -> Self {
        Self {
            answer,
            calls: Cell::new(0),
        }
    }
}

// The trait requires a `Send` future; `Cell` makes the fake `!Sync`, so the
// counter is bumped before the future is built.
impl ContainerStarter for FakeStarter {
    fn start(&self) -> impl Future<Output = Result<String, HarnessError>> + Send {
        self.calls.set(self.calls.get() + 1);
        let answer = self
            .answer
            .map(ToString::to_string)
            .map_err(|e| HarnessError::Container(e.to_string()));
        async move { answer }
    }
}

#[tokio::test]
async fn resolve_prefers_env_url() {
    let starter = FakeStarter::new(Ok("postgres://from-docker"));
    let url = resolve_server(Some("postgres://from-env".into()), &starter)
        .await
        .unwrap();
    assert_eq!(url, "postgres://from-env");
    assert_eq!(
        starter.calls.get(),
        0,
        "Docker must not start when the env URL is set"
    );
}

#[tokio::test]
async fn resolve_uses_starter_without_env() {
    let starter = FakeStarter::new(Ok("postgres://from-docker"));
    let url = resolve_server(None, &starter).await.unwrap();
    assert_eq!(url, "postgres://from-docker");
    assert_eq!(starter.calls.get(), 1);
}

#[tokio::test]
async fn resolve_treats_an_empty_env_url_as_unset() {
    let starter = FakeStarter::new(Ok("postgres://from-docker"));
    let url = resolve_server(Some(String::new()), &starter).await.unwrap();
    assert_eq!(url, "postgres://from-docker");
}

#[tokio::test]
async fn resolve_reports_blocked_infrastructure() {
    let starter = FakeStarter::new(Err("Cannot connect to the Docker daemon"));
    let err = resolve_server(None, &starter).await.unwrap_err();
    let HarnessError::BlockedInfrastructure(message) = &err else {
        panic!("expected BlockedInfrastructure, got {err:?}");
    };
    assert!(message.starts_with("BLOCKED_INFRASTRUCTURE:"), "{message}");
    assert!(message.contains("FORGE_TEST_PG_URL"), "{message}");
    assert!(message.contains("Docker"), "{message}");
    assert!(
        message.contains("Cannot connect to the Docker daemon"),
        "{message}"
    );
    assert_eq!(err.to_string(), *message);
}

#[test]
fn seed_hash_is_stable_and_content_sensitive() {
    assert_eq!(
        Seed::sql("CREATE TABLE t (id int)").hash(),
        "65707b93dfd26a45"
    );
    assert_eq!(Seed::empty().hash(), "4faf5e582a209aac");
    assert_ne!(
        Seed::sql("CREATE TABLE u (id int)").hash(),
        Seed::sql("CREATE TABLE t (id int)").hash()
    );
}

#[tokio::test]
async fn wait_ready_gives_up_with_blocked_infrastructure_and_hides_the_password() {
    let err = wait_ready(
        "postgres://forge:s3cret@127.0.0.1:1/postgres",
        Duration::from_millis(300),
    )
    .await
    .unwrap_err();
    let message = err.to_string();
    assert!(
        matches!(err, HarnessError::BlockedInfrastructure(_)),
        "{err:?}"
    );
    assert!(message.starts_with("BLOCKED_INFRASTRUCTURE:"), "{message}");
    assert!(
        message.contains("postgres://forge:***@127.0.0.1:1/postgres"),
        "{message}"
    );
    assert!(!message.contains("s3cret"), "{message}");
}

#[tokio::test]
async fn wait_ready_rejects_a_malformed_url_immediately() {
    let err = wait_ready("not a url", Duration::from_secs(30))
        .await
        .unwrap_err();
    assert!(
        matches!(err, HarnessError::BlockedInfrastructure(_)),
        "{err:?}"
    );
    assert!(err.to_string().contains("not a url"), "{err}");
}

#[tokio::test]
async fn wait_ready_keeps_urls_without_a_password_intact() {
    for url in [
        "postgres://127.0.0.1:1/postgres",
        "postgres://forge@127.0.0.1:1/postgres",
    ] {
        let err = wait_ready(url, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(err.to_string().contains(url), "{err}");
        assert!(!err.to_string().contains("***"), "{err}");
    }
}

#[tokio::test]
async fn wait_ready_keeps_retrying_until_the_timeout() {
    let started = std::time::Instant::now();
    let err = wait_ready(
        "postgres://forge@127.0.0.1:1/postgres",
        Duration::from_millis(400),
    )
    .await
    .unwrap_err();
    let elapsed = started.elapsed();
    assert!(
        matches!(err, HarnessError::BlockedInfrastructure(_)),
        "{err:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(400),
        "gave up after {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "kept waiting for {elapsed:?}"
    );
}

#[tokio::test]
async fn wait_ready_hides_passwords_with_at_signs_and_in_the_query() {
    for (url, secret) in [
        ("postgres://forge:p@ss@127.0.0.1:1/postgres", "p@ss"),
        (
            "postgres://forge@127.0.0.1:1/postgres?sslmode=disable&password=s3cret",
            "s3cret",
        ),
    ] {
        let err = wait_ready(url, Duration::from_millis(100))
            .await
            .unwrap_err();
        let message = err.to_string();
        assert!(!message.contains(secret), "{message}");
        assert!(message.contains("127.0.0.1:1/postgres"), "{message}");
    }
}
