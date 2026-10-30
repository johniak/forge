//! Where the test PostgreSQL server comes from.

use std::fs::File;
use std::str::FromStr;
use std::time::{Duration, Instant};

use sqlx::postgres::PgConnectOptions;
use sqlx::{ConnectOptions, Connection};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ImageExt, ReuseDirective};
use tokio::sync::OnceCell;

use crate::HarnessError;
use crate::database::sweep_stale;

/// Environment variable holding a server URL with rights to create databases.
pub const ENV_URL: &str = "FORGE_TEST_PG_URL";
/// Name of the reusable local container.
pub const CONTAINER_NAME: &str = "forge-test-postgres-17";
const IMAGE_TAG: &str = "17-alpine";
/// Durability is irrelevant for throw-away test data, and `DROP DATABASE`
/// forces a checkpoint: with fsync on, a test run spends most of its time in
/// fsync (measured: 48 s with defaults vs 6 s with these settings).
const SERVER_SETTINGS: [&str; 6] = [
    "-c",
    "fsync=off",
    "-c",
    "synchronous_commit=off",
    "-c",
    "full_page_writes=off",
];
/// How long a server may take to accept connections (a reused, stopped
/// container is restarted without a readiness wait).
const READY_TIMEOUT: Duration = Duration::from_secs(30);
/// Test databases older than this, and idle, are dropped at harness start.
const STALE_AFTER: Duration = Duration::from_hours(1);

/// Starts (or finds) a PostgreSQL server and returns its admin URL.
pub trait ContainerStarter {
    /// Starts the server.
    fn start(&self) -> impl Future<Output = Result<String, HarnessError>> + Send;
}

/// Runs `postgres:17-alpine` as the reusable container `forge-test-postgres-17`.
///
/// Uses testcontainers' experimental `reusable-containers` feature: the
/// container outlives the test process and later runs reuse it. Remove it with
/// `docker rm -f forge-test-postgres-17`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DockerStarter;

impl ContainerStarter for DockerStarter {
    async fn start(&self) -> Result<String, HarnessError> {
        // Parallel test processes would otherwise race to create the same
        // named container; the lock is released when `_lock` drops.
        let _lock = lock_file()?;
        let container = Postgres::default()
            .with_tag(IMAGE_TAG)
            .with_cmd(SERVER_SETTINGS)
            .with_container_name(CONTAINER_NAME)
            .with_reuse(ReuseDirective::Always)
            .start()
            .await
            .map_err(container_error)?;
        let host = container.get_host().await.map_err(container_error)?;
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .map_err(container_error)?;
        Ok(format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
    }
}

fn lock_file() -> Result<File, HarnessError> {
    let file = File::create(std::env::temp_dir().join("forge-test-postgres.lock"))?;
    file.lock()?;
    Ok(file)
}

fn container_error(err: impl std::fmt::Display) -> HarnessError {
    HarnessError::Container(err.to_string())
}

/// The server URL: `env_url` when set and non-empty, otherwise whatever
/// `starter` provides.
///
/// # Errors
///
/// [`HarnessError::BlockedInfrastructure`] when there is no URL and the starter
/// fails.
pub async fn resolve_server(
    env_url: Option<String>,
    starter: &impl ContainerStarter,
) -> Result<String, HarnessError> {
    if let Some(url) = env_url.filter(|url| !url.is_empty()) {
        return Ok(url);
    }
    starter.start().await.map_err(|err| {
        HarnessError::BlockedInfrastructure(format!(
            "BLOCKED_INFRASTRUCTURE: no PostgreSQL for tests; set {ENV_URL} to a server URL \
             or start Docker so that forge-testing can run postgres:{IMAGE_TAG} ({err})"
        ))
    })
}

/// Waits until `url` accepts connections.
///
/// # Errors
///
/// [`HarnessError::BlockedInfrastructure`] when the URL is malformed or the
/// server is not ready within `timeout`; the message never shows the password.
pub async fn wait_ready(url: &str, timeout: Duration) -> Result<(), HarnessError> {
    let blocked = |why: String| {
        HarnessError::BlockedInfrastructure(format!(
            "BLOCKED_INFRASTRUCTURE: PostgreSQL at {} {why}",
            redact(url)
        ))
    };
    let options = PgConnectOptions::from_str(url)
        .map_err(|err| blocked(format!("is not a valid URL: {err}")))?;
    let deadline = Instant::now() + timeout;
    loop {
        let err = match options.connect().await {
            Ok(conn) => return Ok(conn.close().await?),
            Err(err) => err,
        };
        if Instant::now() >= deadline {
            return Err(blocked(format!(
                "did not accept connections within {timeout:?}: {err}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// `url` with the password replaced by `***`, both in the userinfo (which may
/// itself contain `@`) and in a `password=` query parameter.
pub(crate) fn redact(url: &str) -> String {
    let url = redact_query_password(url);
    let Some((scheme, rest)) = url.split_once("://") else {
        return url;
    };
    let Some((userinfo, host)) = rest.rsplit_once('@') else {
        return url;
    };
    let Some((user, _password)) = userinfo.split_once(':') else {
        return url;
    };
    format!("{scheme}://{user}:***@{host}")
}

fn redact_query_password(url: &str) -> String {
    let Some((head, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let params: Vec<&str> = query
        .split('&')
        .map(|param| {
            if param.starts_with("password=") {
                "password=***"
            } else {
                param
            }
        })
        .collect();
    format!("{head}?{}", params.join("&"))
}

static SERVER: OnceCell<String> = OnceCell::const_new();

/// The process-wide server URL: resolved once (env or Docker), waited for, and
/// swept of stale test databases. Only the URL is shared, never a pool: pools
/// belong to the runtime that created them and every `#[tokio::test]` has its own.
///
/// # Errors
///
/// [`HarnessError::BlockedInfrastructure`] without a reachable server; a
/// [`HarnessError::Database`] from the sweep.
pub async fn server_url() -> Result<String, HarnessError> {
    SERVER
        .get_or_try_init(|| async {
            let url = resolve_server(std::env::var(ENV_URL).ok(), &DockerStarter).await?;
            wait_ready(&url, READY_TIMEOUT).await?;
            sweep_stale(&url, STALE_AFTER).await?;
            Ok(url)
        })
        .await
        .cloned()
}

/// `admin_url` pointing at database `name` instead.
pub(crate) fn database_url(admin_url: &str, name: &str) -> Result<String, HarnessError> {
    Ok(PgConnectOptions::from_str(admin_url)?
        .database(name)
        .to_url_lossy()
        .to_string())
}
