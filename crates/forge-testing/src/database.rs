//! Template databases, per-test databases and the stale sweep.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{AssertSqlSafe, Connection, PgConnection};

use crate::server::{database_url, server_url};
use crate::{HarnessError, Seed};

const TEST_PREFIX: &str = "forge_test_";
const TEMPLATE_PREFIX: &str = "forge_tpl_";
const POOL_SIZE: u32 = 5;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A database that exists for one test and is dropped when this value drops,
/// including while a failing test unwinds.
#[derive(Debug)]
pub struct TestDatabase {
    url: String,
    name: String,
    admin_url: String,
}

impl TestDatabase {
    /// Creates a fresh database holding `seed`, building the seed's template
    /// first if no process has built it yet.
    ///
    /// # Errors
    ///
    /// [`HarnessError::BlockedInfrastructure`] without a server; a
    /// [`HarnessError::Database`] when the seed SQL or a statement fails.
    pub async fn create(seed: &Seed) -> Result<TestDatabase, HarnessError> {
        let admin_url = server_url().await?;
        let template = ensure_template(&admin_url, seed).await?;
        let name = next_name();
        let mut admin = PgConnection::connect(&admin_url).await?;
        execute(
            &mut admin,
            format!(
                "CREATE DATABASE {} TEMPLATE {}",
                quote(&name),
                quote(&template)
            ),
        )
        .await?;
        admin.close().await?;
        Ok(TestDatabase {
            url: database_url(&admin_url, &name)?,
            name,
            admin_url,
        })
    }

    /// Connection URL of this database.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Database name (`forge_test_<unix secs>_<pid>_<counter>`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A small connection pool for this database, bound to the calling runtime.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Database`] when the database cannot be reached.
    pub async fn pool(&self) -> Result<PgPool, HarnessError> {
        Ok(PgPoolOptions::new()
            .max_connections(POOL_SIZE)
            .connect(&self.url)
            .await?)
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // `Drop` may run inside the test's runtime, which cannot block on
        // another future, so the drop runs on its own thread and runtime.
        let mut outcome = Ok(());
        std::thread::scope(|scope| {
            scope.spawn(|| outcome = drop_database(&self.admin_url, &self.name));
        });
        if let Err(err) = outcome {
            eprintln!(
                "forge-testing: could not drop test database {}: {err}; the next run's sweep removes it",
                self.name
            );
        }
    }
}

fn drop_database(admin_url: &str, name: &str) -> Result<(), HarnessError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut admin = PgConnection::connect(admin_url).await?;
        execute(
            &mut admin,
            format!("DROP DATABASE {} WITH (FORCE)", quote(name)),
        )
        .await?;
        Ok(admin.close().await?)
    })
}

/// Builds `forge_tpl_<hash>` once across all processes and returns its name.
///
/// Under a session advisory lock keyed by the hash: build into `…_building`,
/// rename, then mark it a template that accepts no connections, so no session
/// can ever block `CREATE DATABASE … TEMPLATE`.
async fn ensure_template(admin_url: &str, seed: &Seed) -> Result<String, HarnessError> {
    let hash = seed.hash();
    let template = format!("{TEMPLATE_PREFIX}{hash}");
    let key = i64::from_str_radix(&hash[..15], 16).unwrap_or_default();
    let mut admin = PgConnection::connect(admin_url).await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut admin)
        .await?;
    let built = build_template(&mut admin, admin_url, &template, seed).await;
    // Closing the session releases the advisory lock even when the build failed.
    admin.close().await?;
    built.map(|()| template)
}

async fn build_template(
    admin: &mut PgConnection,
    admin_url: &str,
    template: &str,
    seed: &Seed,
) -> Result<(), HarnessError> {
    let present: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(template)
            .fetch_one(&mut *admin)
            .await?;
    if present {
        return Ok(());
    }
    let building = format!("{template}_building");
    execute(
        admin,
        format!("DROP DATABASE IF EXISTS {}", quote(&building)),
    )
    .await?;
    execute(admin, format!("CREATE DATABASE {}", quote(&building))).await?;
    let mut conn = PgConnection::connect(&database_url(admin_url, &building)?).await?;
    let seeded = sqlx::raw_sql(AssertSqlSafe(seed.text().to_string()))
        .execute(&mut conn)
        .await;
    conn.close().await?;
    seeded?;
    execute(
        admin,
        format!(
            "ALTER DATABASE {} RENAME TO {}",
            quote(&building),
            quote(template)
        ),
    )
    .await?;
    execute(
        admin,
        format!(
            "ALTER DATABASE {} WITH IS_TEMPLATE true ALLOW_CONNECTIONS false",
            quote(template)
        ),
    )
    .await
}

/// Drops `forge_test_*` databases created more than `older_than` ago. A
/// database someone is connected to is left alone: plain `DROP DATABASE`
/// (without `FORCE`) refuses it, and that refusal is not an error here.
///
/// # Errors
///
/// [`HarnessError::Database`] when the server cannot be reached or listed.
pub async fn sweep_stale(
    admin_url: &str,
    older_than: Duration,
) -> Result<Vec<String>, HarnessError> {
    let mut admin = PgConnection::connect(admin_url).await?;
    let names: Vec<String> =
        sqlx::query_scalar("SELECT datname FROM pg_database WHERE starts_with(datname, $1)")
            .bind(TEST_PREFIX)
            .fetch_all(&mut admin)
            .await?;
    let cutoff = now_secs().saturating_sub(older_than.as_secs());
    let mut dropped = Vec::new();
    for name in names {
        let stale = stale_timestamp(&name).is_some_and(|created| created < cutoff);
        if stale
            && execute(&mut admin, format!("DROP DATABASE {}", quote(&name)))
                .await
                .is_ok()
        {
            dropped.push(name);
        }
    }
    admin.close().await?;
    Ok(dropped)
}

/// Creation time embedded in a test database name.
fn stale_timestamp(name: &str) -> Option<u64> {
    let rest = name.strip_prefix(TEST_PREFIX)?;
    let (secs, _) = rest.split_once('_')?;
    secs.parse().ok()
}

fn next_name() -> String {
    format!(
        "{TEST_PREFIX}{}_{}_{}",
        now_secs(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Quotes a generated identifier. Names come only from this module, never
/// from test input.
fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Runs one generated DDL statement (DDL cannot take bind parameters).
async fn execute(conn: &mut PgConnection, sql: String) -> Result<(), HarnessError> {
    sqlx::raw_sql(AssertSqlSafe(sql)).execute(conn).await?;
    Ok(())
}
