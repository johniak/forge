# Contributing to Forge

Forge is built mostly by coding agents (Claude Code, Codex) under the rules in
[`CLAUDE.md`](CLAUDE.md); humans are welcome under the same rules. The short
version: no claim without evidence, no silently skipped check, no lowered gate.

## Toolchains

- **Stable, pinned** in [`rust-toolchain.toml`](rust-toolchain.toml) (1.95.0) —
  every build and test. rustup picks it up automatically.
- **Coverage toolchain** `nightly-2026-06-13` (rustc 1.98.0-nightly
  654079540) — only for branch coverage, which stable rustc cannot measure
  ([ADR-0002](docs/adrs/0002-test-and-coverage-policy.md)).

```sh
rustup toolchain install nightly-2026-06-13 --profile minimal --component llvm-tools-preview
cargo install cargo-nextest cargo-llvm-cov cargo-mutants --locked
```

## PostgreSQL for tests

`forge-testing` gives every test its own database cloned from a seeded
template. The server comes from:

1. `FORGE_TEST_PG_URL` — a URL whose user may create databases, e.g.
   `postgres://postgres:postgres@localhost:5432/postgres`; or
2. Docker — when the variable is unset, the harness starts (or reuses) the
   container `forge-test-postgres-17` (`postgres:17-alpine`). It keeps running
   between test runs; remove it with `docker rm -f forge-test-postgres-17`.

Without either, database tests **fail** with a `BLOCKED_INFRASTRUCTURE`
message; they are never skipped. For speed, run your own test server with
`-c fsync=off -c synchronous_commit=off -c full_page_writes=off` (the Docker
path does this for you; `DROP DATABASE` forces a checkpoint, so fsync
dominates test time otherwise).

## Before you push

```sh
cargo nextest run --workspace
cargo run -p forge-cli -- check
mkdir -p target/coverage
cargo +nightly-2026-06-13 llvm-cov nextest --branch --workspace \
  --json --summary-only --output-path target/coverage/coverage.json
cargo run -p forge-cli -- coverage --input target/coverage/coverage.json
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the same
commands; `check`, `test` and `coverage` gate every change, `mutants` is
informational until Phase 4.

## Rules that are not negotiable

- **No test is skipped silently.** Missing infrastructure is reported as
  `NOT_VERIFIED` / `BLOCKED_INFRASTRUCTURE`, never as a pass.
- **Coverage is 100 % lines and branches** for every module's own code. An
  exclusion needs an owner-approved ledger entry; nobody approves their own.
- **Write the failing test first**, then the code.
- **Architecture boundaries** live in each `forge.module.toml` and are checked
  by `forge check`; change the manifest deliberately, never to silence it.
- **Decisions** that shape the framework go into an ADR under
  [`docs/adrs/`](docs/adrs/README.md) and need the owner's approval before code
  depends on them.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`,
`test:`, `docs:`, `chore:`, …), one logical change per commit, with the
evidence (commands run, results) in the description when it matters.
