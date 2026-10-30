# ADR-0003 Safe query strategy

**Status:** Proposed — closed by the Phase 1 vertical slice (experiment below).  
**Date:** 2026-10-30  
**SPEC:** §4.2, §11.1, §11.2, §17 Q3. Matrix: O-03, M-02, M-03, S-02.

## Context

SPEC requires a strongly typed, PostgreSQL-first persistence layer without a
heavyweight ORM: values always bound, identifiers allowlisted, policy applied
before data is materialized, no `execute(raw_user_string)` in the public safe
API, and an audited privileged escape hatch. Agents write most of the code, so
the safe path must also be the easy path and the unsafe path must be visible
to tooling, not only to reviewers.

## Options

1. **Three tiers on SQLx** (design D2):
   1. *Static SQL* for business queries — `sqlx::query!`/`query_as!` checked
      at compile time against a committed `.sqlx/` offline snapshot
      (`SQLX_OFFLINE=true` in CI); a fresh checkout builds without a database.
   2. *Generated CRUD* from the schema IR (ADR-0001) — insert/get/update/
      delete/list with typed bindings, policy applied before materialization.
   3. *Constrained dynamic builder* for admin lists and filters — columns and
      operators allowlisted from the IR, every value bound, sort direction a
      typed enum. It is the only place outside migrations that assembles SQL
      text.
2. **A full ORM query DSL** (SeaORM/Diesel style). Rejected as the default:
   SPEC does not require one, it adds a second query language agents must
   learn, and dynamic admin querying still needs allowlisting on top.
3. **Raw SQLx with review only.** Rejected: nothing mechanical stops an agent
   from formatting user input into SQL.

## Decision (proposed)

Option 1. Enforcement is mechanical, in three layers:

- **The type system.** In sqlx 0.9.0, `query`, `query_as`, `query_scalar` and
  `raw_sql` take `impl SqlSafeStr`, implemented only for `&'static str`,
  `AssertSqlSafe<…>` and `SqlStr` (verified in `sqlx-core-0.9.0/src/sql_str.rs`).
  A runtime string therefore cannot reach the database without the visible
  wrapper `sqlx::AssertSqlSafe(...)`.
- **Lints shipped by `forge new`.** App crates get a `clippy.toml` with
  `disallowed-types = ["sqlx::AssertSqlSafe"]` and
  `disallowed-methods = ["sqlx::raw_sql"]`; `forge check` runs clippy with
  warnings denied.
- **The architecture guard.** `forge check` also fails when an app module
  depends on the privileged escape-hatch API of `forge-database` without an
  entry in the security exception register (`docs/security/threat-model.md`).

The escape hatch lives in `forge-database` behind an explicit capability
(name settled in Phase 1), for migrations and audited cases only.

Framework crates that must emit generated DDL (today: `forge-testing`'s
`CREATE/DROP DATABASE` on names it generates and quotes itself) use
`AssertSqlSafe` directly; they are not app crates, and each use names where
its identifiers come from.

## Experiment (Phase 1)

1. The `orders` slice uses only tiers 1–2; a stale `.sqlx/` snapshot makes
   `forge check` fail with a structured diagnostic.
2. A deliberate `sqlx::AssertSqlSafe(format!(…))` in an app crate is rejected
   by `forge check` with a span on the call.
3. Tier 3 is built in Phase 2 with property tests and `cargo-fuzz` showing that
   no generated filter/sort combination produces SQL outside the allowlist and
   that every value is a bind parameter.

## Consequences

- Business SQL is reviewed as SQL, checked against the real schema at compile
  time, and visible to the context engine through `.sqlx/`.
- Contributors must refresh `.sqlx/` (`cargo sqlx prepare`) when queries
  change; `forge check` tells them.
- Dynamic querying is limited to what the IR allows; anything else needs a
  static query or a reviewed exception.

## Reversibility

Medium. Tiers 2–3 sit behind Forge APIs and can change implementation; tier 1
ties business code to SQLx macros, which is the cost of compile-time checked
SQL.

## Evidence

- sqlx 0.9.0 `SqlSafeStr` implementations: `sqlx-core-0.9.0/src/sql_str.rs`
  (checked 2026-10-29/30).
- `forge-testing` uses the pattern for generated DDL:
  `crates/forge-testing/src/database.rs` (`execute`, `quote`).
