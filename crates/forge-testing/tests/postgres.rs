//! Database-per-test against a real PostgreSQL (`FORGE_TEST_PG_URL` or Docker).
//! None of these tests returns early without a server: they fail with the
//! `BLOCKED_INFRASTRUCTURE` message instead (CLAUDE.md rule 7).
#![allow(clippy::unwrap_used)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use forge_testing::{Seed, TestDatabase, server_url, sweep_stale};
use sqlx::{Connection, PgConnection};

async fn admin() -> PgConnection {
    PgConnection::connect(&server_url().await.unwrap())
        .await
        .unwrap()
}

async fn exists(name: &str) -> bool {
    let mut admin = admin().await;
    sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(name)
        .fetch_one(&mut admin)
        .await
        .unwrap()
}

/// A seed no other run has used, so the template is created by this test.
fn fresh_seed(body: &str) -> Seed {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    Seed::sql(format!("-- run {nanos} {}\n{body}", std::process::id()))
}

#[tokio::test]
async fn databases_are_isolated_and_seeded() {
    let seed = Seed::sql("CREATE TABLE t (id int)");
    let a = TestDatabase::create(&seed).await.unwrap();
    let b = TestDatabase::create(&seed).await.unwrap();
    assert_ne!(a.name(), b.name());
    for db in [&a, &b] {
        assert!(db.name().starts_with("forge_test_"));
        assert!(db.name().len() <= 63);
        assert!(
            db.name()
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        );
        assert!(db.url().contains(db.name()));
    }

    let pool_a = a.pool().await.unwrap();
    sqlx::query("INSERT INTO t VALUES (1)")
        .execute(&pool_a)
        .await
        .unwrap();
    let pool_b = b.pool().await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM t")
        .fetch_one(&pool_b)
        .await
        .unwrap();
    assert_eq!(rows, 0, "database B sees the seeded table but not A's row");
}

#[tokio::test]
async fn concurrent_creates_share_one_template() {
    let seed = fresh_seed("CREATE TABLE c (id int)");
    let template = format!("forge_tpl_{}", seed.hash());
    let creates = (0..8).map(|_| {
        let seed = seed.clone();
        async move { TestDatabase::create(&seed).await }
    });
    let databases: Vec<TestDatabase> = futures_join_all(creates).await;
    let mut names: Vec<&str> = databases.iter().map(TestDatabase::name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 8);

    let mut admin = admin().await;
    let (count, is_template, allow_conn): (i64, bool, bool) = sqlx::query_as(
        "SELECT count(*), bool_and(datistemplate), bool_and(datallowconn) FROM pg_database WHERE datname = $1",
    )
    .bind(&template)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!((count, is_template, allow_conn), (1, true, false));
    assert!(!exists(&format!("{template}_building")).await);
}

/// Awaits every create concurrently on this runtime and unwraps the results.
async fn futures_join_all<F>(futures: impl Iterator<Item = F>) -> Vec<TestDatabase>
where
    F: Future<Output = Result<TestDatabase, forge_testing::HarnessError>> + Send + 'static,
{
    let handles: Vec<_> = futures.map(tokio::spawn).collect();
    let mut out = Vec::new();
    for handle in handles {
        out.push(handle.await.unwrap().unwrap());
    }
    out
}

#[tokio::test]
async fn broken_seed_fails_and_leaves_no_template() {
    let seed = fresh_seed("THIS IS NOT SQL");
    let template = format!("forge_tpl_{}", seed.hash());
    let err = TestDatabase::create(&seed).await.err().unwrap();
    assert!(
        matches!(err, forge_testing::HarnessError::Database(_)),
        "{err:?}"
    );
    assert!(!exists(&template).await);
    // A retry after the failure starts from scratch and fails the same way.
    assert!(TestDatabase::create(&seed).await.is_err());
}

#[tokio::test]
async fn empty_seed_gives_an_empty_database() {
    let db = TestDatabase::create(&Seed::empty()).await.unwrap();
    let pool = db.pool().await.unwrap();
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 0);
}

#[tokio::test]
async fn drop_removes_database_even_on_panic() {
    let name = std::thread::spawn(|| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let db = runtime
            .block_on(TestDatabase::create(&Seed::empty()))
            .unwrap();
        let name = db.name().to_string();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _db = db;
            panic!("test body failed");
        }));
        assert!(outcome.is_err());
        name
    })
    .join()
    .unwrap();
    assert!(
        !exists(&name).await,
        "{name} must be dropped during unwinding"
    );
}

#[tokio::test]
async fn database_dropped_by_the_test_itself_does_not_panic_on_drop() {
    let db = TestDatabase::create(&Seed::empty()).await.unwrap();
    let mut admin = admin().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{}\" WITH (FORCE)",
        db.name()
    )))
    .execute(&mut admin)
    .await
    .unwrap();
    drop(db); // reports the failed drop on stderr instead of panicking
}

#[tokio::test]
async fn sweep_drops_stale_but_never_live_databases() {
    let url = server_url().await.unwrap();
    let pid = std::process::id();
    let stale_idle = format!("forge_test_100_{pid}_1");
    let stale_live = format!("forge_test_100_{pid}_2");
    let unparseable = [
        format!("forge_test_x_{pid}"),
        format!("forge_test_100{pid}"),
    ];
    let mut admin = admin().await;
    for name in [&stale_idle, &stale_live, &unparseable[0], &unparseable[1]] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&mut admin)
            .await
            .unwrap();
    }
    let live_url = url.rsplit_once('/').unwrap().0.to_string() + "/" + &stale_live;
    let live = PgConnection::connect(&live_url).await.unwrap();
    let fresh = TestDatabase::create(&Seed::empty()).await.unwrap();

    let dropped = sweep_stale(&url, Duration::from_hours(1)).await.unwrap();

    assert!(dropped.contains(&stale_idle), "{dropped:?}");
    assert!(!exists(&stale_idle).await);
    assert!(
        exists(&stale_live).await,
        "a database in use is never dropped"
    );
    assert!(
        exists(fresh.name()).await,
        "a fresh database is never dropped"
    );
    for name in &unparseable {
        assert!(
            exists(name).await,
            "{name} has no timestamp and is left alone"
        );
    }

    live.close().await.unwrap();
    for name in [&stale_live, &unparseable[0], &unparseable[1]] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&mut admin)
        .await
        .unwrap();
    }
}
