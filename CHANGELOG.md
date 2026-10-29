# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
semantic versioning with `0.x` until Phase 6.

## [Unreleased]

### Added

- `forge check`: manifests, Cargo-graph architecture guard, `cargo fmt` and
  `cargo clippy` as one structured, versioned report.
- `forge coverage`: per-module line and branch coverage gate over a
  `cargo llvm-cov` JSON export, using each module's `[verification]`
  thresholds.
- `forge-testing`: a fresh PostgreSQL database per test, cloned from a seeded
  template; `FORGE_TEST_PG_URL` or a reusable Docker container; stale-database
  sweep; `BLOCKED_INFRASTRUCTURE` instead of skipped tests.
- CI: check, test (PostgreSQL service), coverage (nightly, 100 % gate),
  mutation testing (informational).
- Specification, design, ADRs 0001–0007, requirements matrix, threat model.

### Fixed

- The architecture guard no longer passes when the workspace `Cargo.toml` is
  missing or unparseable (it reports `FA004`).
- Clippy findings reported once per target are deduplicated.
