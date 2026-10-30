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
    drop(databases);
    drop_template(&template).await;
}

/// Removes a template this test created, so repeated runs do not accumulate them.
async fn drop_template(template: &str) {
    let mut admin = admin().await;
    for sql in [
        format!("ALTER DATABASE \"{template}\" WITH IS_TEMPLATE false"),
        format!("DROP DATABASE \"{template}\""),
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&mut admin)
            .await
            .unwrap();
    }
}

/// Force-drops the named databases when it goes out of scope, also when the
/// test fails half-way, so hand-made fixture databases never outlive a run.
struct DropOnExit(Vec<String>);

impl Drop for DropOnExit {
    fn drop(&mut self) {
        let names = std::mem::take(&mut self.0);
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let mut admin = admin().await;
                for name in names {
                    let sql = format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)");
                    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                        .execute(&mut admin)
                        .await
                        .unwrap();
                }
            });
        })
        .join()
        .unwrap();
    }
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
    assert!(
        !exists(&format!("{template}_building")).await,
        "a failed build must not leave its scratch database behind"
    );
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
    // Half an hour old: stale for this test's one-minute sweep, but not for
    // the one-hour start-up sweep of other test processes running meanwhile.
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 1800;
    let stale_idle = format!("forge_test_{created}_{pid}_1");
    let stale_live = format!("forge_test_{created}_{pid}_2");
    let unparseable = [
        format!("forge_test_x_{pid}"),
        format!("forge_test_100{pid}"),
    ];
    let _cleanup = DropOnExit(vec![
        stale_idle.clone(),
        stale_live.clone(),
        unparseable[0].clone(),
        unparseable[1].clone(),
    ]);
    let mut admin = admin().await;
    for name in [&stale_idle, &stale_live, &unparseable[0], &unparseable[1]] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&mut admin)
            .await
            .unwrap();
    }
    let live = PgConnection::connect(&database_url(&url, &stale_live))
        .await
        .unwrap();
    let fresh = TestDatabase::create(&Seed::empty()).await.unwrap();

    let dropped = sweep_stale(&url, Duration::from_mins(1)).await.unwrap();

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
}

#[tokio::test]
async fn sweep_drops_abandoned_template_builds_but_not_active_ones() {
    let url = server_url().await.unwrap();
    // Unique per process; the advisory lock key is the first 15 hex digits.
    // Every fixture is created while its lock is held, as a real builder does,
    // so start-up sweeps of other test processes cannot take it early.
    let pid = format!("{:08x}", std::process::id());
    let abandoned = format!("forge_tpl_aaaa{pid}0000_building");
    let active_hash = format!("bbbb{pid}0000");
    let active = format!("forge_tpl_{active_hash}_building");
    let in_use_hash = format!("cccc{pid}0000");
    let in_use = format!("forge_tpl_{in_use_hash}_building");
    let junk = format!("forge_tpl_{pid}_building");
    let _cleanup = DropOnExit(vec![
        abandoned.clone(),
        active.clone(),
        in_use.clone(),
        junk.clone(),
    ]);
    // The builder of `active` keeps its lock for the whole test.
    let mut builder = PgConnection::connect(&url).await.unwrap();
    advisory_lock(&mut builder, &active_hash).await;
    // `in_use` gets a reader before its lock is released: no builder, but busy.
    let mut in_use_builder = PgConnection::connect(&url).await.unwrap();
    advisory_lock(&mut in_use_builder, &in_use_hash).await;
    let mut admin = admin().await;
    for name in [&abandoned, &active, &in_use, &junk] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&mut admin)
            .await
            .unwrap();
    }
    let reader = PgConnection::connect(&database_url(&url, &in_use))
        .await
        .unwrap();
    in_use_builder.close().await.unwrap();

    sweep_stale(&url, Duration::from_hours(1)).await.unwrap();

    assert!(!exists(&abandoned).await, "nobody builds it: dropped");
    assert!(exists(&active).await, "its builder holds the lock: kept");
    assert!(exists(&in_use).await, "a database in use is never dropped");
    assert!(
        exists(&junk).await,
        "no parseable hash, no lock key: left alone"
    );
    builder.close().await.unwrap();
    reader.close().await.unwrap();
}

/// Takes the template-build lock for `hash` (key: its first 15 hex digits).
async fn advisory_lock(conn: &mut PgConnection, hash: &str) {
    let key = i64::from_str_radix(&hash[..15], 16).unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(conn)
        .await
        .unwrap();
}

/// `url` pointing at database `name`.
fn database_url(url: &str, name: &str) -> String {
    url.rsplit_once('/').unwrap().0.to_string() + "/" + name
}

#[tokio::test]
async fn debug_output_never_shows_the_password() {
    let url = server_url().await.unwrap();
    let userinfo = url.split_once("://").unwrap().1.rsplit_once('@').unwrap().0;
    let password = userinfo.split_once(':').unwrap().1;
    assert!(
        !password.is_empty(),
        "the test servers always use a password"
    );
    let db = TestDatabase::create(&Seed::empty()).await.unwrap();
    let debug = format!("{db:?}");
    assert!(debug.contains(db.name()), "{debug}");
    assert!(!debug.contains(&format!(":{password}@")), "{debug}");
}
