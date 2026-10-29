//! Forge test harness.
//!
//! Every test gets its own PostgreSQL database, created in milliseconds from a
//! template database that holds the test's seed (`CREATE DATABASE … TEMPLATE`).
//! The server comes from `FORGE_TEST_PG_URL` or, when that is unset, from a
//! reusable Docker container. Without either, tests fail with a
//! `BLOCKED_INFRASTRUCTURE` message; they are never skipped.
//!
//! Phase 1 adds model factories and an in-process `TestApp` on top of this.

mod database;
mod seed;
mod server;

pub use database::{TestDatabase, sweep_stale};
pub use seed::Seed;
pub use server::{ContainerStarter, DockerStarter, resolve_server, server_url, wait_ready};

/// Why the harness could not provide a database.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// No PostgreSQL is reachable; the message starts with
    /// `BLOCKED_INFRASTRUCTURE:` and names the remedies.
    #[error("{0}")]
    BlockedInfrastructure(String),
    /// The server rejected a statement or the connection broke.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Docker could not start or inspect the test container.
    #[error("container error: {0}")]
    Container(String),
    /// The harness could not set up its own process resources.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
