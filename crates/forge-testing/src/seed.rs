//! What a template database is built from.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// Domain separator, bumped when the template layout changes.
const HASH_DOMAIN: &str = "forge-testing/1\n";

/// The SQL a template database is built from. Phase 1 replaces the hand-written
/// SQL with the project's migrations; the hash stays the template's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    sql: String,
}

impl Seed {
    /// A template with no objects.
    #[must_use]
    pub fn empty() -> Seed {
        Seed::sql("")
    }

    /// A template built by running `sql` (several statements allowed).
    #[must_use]
    pub fn sql(sql: impl Into<String>) -> Seed {
        Seed { sql: sql.into() }
    }

    /// First 16 hex digits of SHA-256 over the domain separator and the SQL.
    #[must_use]
    pub fn hash(&self) -> String {
        let digest = Sha256::new()
            .chain_update(HASH_DOMAIN)
            .chain_update(&self.sql)
            .finalize();
        digest[..8]
            .iter()
            .fold(String::with_capacity(16), |mut hex, byte| {
                // Writing into a String cannot fail.
                let _ = write!(hex, "{byte:02x}");
                hex
            })
    }

    pub(crate) fn text(&self) -> &str {
        &self.sql
    }
}
