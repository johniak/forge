# Phase 0 — Reproducible Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish Phase 0 so that a fresh clone passes `cargo nextest run` and `forge check` with PostgreSQL from Docker or `FORGE_TEST_PG_URL`, coverage is gated at 100 % lines **and** branches per module by Forge's own tool, and CI encodes all of it.

**Architecture:** Keep the existing `forge-tooling` / `forge-cli` spike (design D12), fix it until it passes its own gates, add a `forge coverage` gate that consumes `cargo llvm-cov` JSON and the `[verification]` thresholds already declared in `forge.module.toml`, and add `forge-testing` (database-per-test from a template database). Documentation, threat model and ADR-0003 complete the phase.

**Tech Stack:** Rust 1.95.0 (pinned), nightly `2026-06-12` for coverage only, `cargo-nextest`, `cargo-llvm-cov`, `cargo-mutants`, `sqlx 0.9` (`runtime-tokio`, `postgres`), `tokio 1.54`, `testcontainers 0.27` (+ `testcontainers-modules 0.15`, `postgres`), `sha2 0.11`, PostgreSQL 17 (`postgres:17-alpine`), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-29-forge-design.md` (§7 Phase 0, D6, D10–D13) and `SPEC.md` (§9 testing, §11 security, §15 phases).

## Global Constraints

- No silent skips: absent infrastructure is `NOT_VERIFIED` / `BLOCKED_INFRASTRUCTURE`, never a pass (CLAUDE.md rule 7).
- Coverage thresholds are never lowered; no exclusion without an owner-approved ledger entry (none exist in Phase 0).
- `[workspace.lints]` stay as they are (`clippy::pedantic`, `unwrap_used`, `expect_used`, `missing_docs`, `unsafe_code = "forbid"`); tests may `#![allow(clippy::unwrap_used)]` only.
- Exit codes of `forge`: `0` PASS, `1` FAIL, `3` NOT_VERIFIED, `2` usage.
- Every report is `schema_version`-ed JSON (`report::SCHEMA_VERSION = 1`).
- Diagnostic codes are stable: existing `FM001–FM007`, `FA000–FA004`, `FF000–FF001`, `FC000–FC001`, `FP001`, `FSKIP`; new ones are defined in the task that introduces them.
- License `MIT OR Apache-2.0`; all code, docs and ADRs in English.
- **Commits (design D11):** none until the owner authorizes. Where a task would commit, the executor instead records the changed files in the task's status report.

## Review Focus

1. A coverage export whose file paths lie outside the project root (stale export, other checkout) must make every module `NOT_VERIFIED`, never `PASS` — Task 3.
2. A module with no measured files (new crate missing from the run) must be `NOT_VERIFIED` — Task 3.
3. A test process killed mid-run (nextest timeout) leaves a database behind; a later sweep must drop it, and must never drop a database a concurrently running test is connected to — Task 5.
4. The very first run with many parallel test processes and no template must create exactly one template and never fail with "source database is being accessed by other users" — Task 5.
5. Tests that spawn `cargo` must behave identically under `cargo nextest run` and under `cargo llvm-cov` (no inherited `RUSTUP_TOOLCHAIN`, `RUSTFLAGS`, `CARGO_TARGET_DIR`) — Task 1.

---

### Task 1: Pinned coverage toolchain and coverage-safe subprocess tests

Today `cargo +nightly llvm-cov nextest --branch` fails 3 `forge-cli` tests: the `forge` binary spawns `cargo fmt`/`cargo clippy` in fixture projects and inherits the coverage run's toolchain and flags.

**Files:**
- Modify: `crates/forge-cli/tests/cli.rs` (the `forge()` helper and `tiny_project()`)
- Modify: `docs/adrs/0002-test-and-coverage-policy.md` (amendment: exact pinned name and `rustc -V`)

**Interfaces:**
- Produces: the exact coverage toolchain name `COVERAGE_TOOLCHAIN` used by Tasks 3, 4, 6 (expected `nightly-2026-06-12`).

- [ ] **Step 1:** `rustup toolchain install nightly-2026-06-12 --profile minimal --component llvm-tools-preview` and run `rustup run nightly-2026-06-12 rustc -V`. Expected: `rustc 1.98.0-nightly (654079540 2026-06-12)` (the build measured in `docs/evidence/phase0/coverage-experiment.txt`). If the commit differs, try `nightly-2026-06-13`; use the name whose `rustc -V` matches and record it in the ADR-0002 amendment.
- [ ] **Step 2:** Reproduce the failure: `cargo +<COVERAGE_TOOLCHAIN> llvm-cov nextest --branch --workspace --summary-only`. Expected: 3 failures in `forge-cli::cli` with "'cargo-clippy' is not installed for the toolchain".
- [ ] **Step 3:** In `cli.rs`, make `forge()` remove these variables from the child environment: `RUSTUP_TOOLCHAIN`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `RUSTDOCFLAGS`, `CARGO_ENCODED_RUSTDOCFLAGS`, `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, and every variable starting with `CARGO_LLVM_COV`. Keep `LLVM_PROFILE_FILE` (the instrumented `forge` binary must still write its profile). Make `tiny_project()` write a `rust-toolchain.toml` with the repository's content (read it via `env!("CARGO_MANIFEST_DIR")/../../rust-toolchain.toml`) so fixture projects use 1.95.0 in both runs. Decision: sanitize instead of installing clippy into the nightly, so that test semantics do not depend on which clippy runs.
- [ ] **Step 4:** Run `cargo nextest run --workspace` → `58 tests run: 58 passed`; run the Step 2 command → `58 passed` and a coverage table that includes `forge-cli/src/main.rs`. If any further variable leaks (symptom: a fixture build lands under `target/llvm-cov-target`), add it to the list and say so in the report.
- [ ] **Step 5:** Append an "Amendment 2026-10-29 (proposed)" section to ADR-0002: pinned name, `rustc -V` output, the sanitized variable list, and that the 100 % branch gate is implemented by `forge coverage` (Task 3) because `cargo llvm-cov` has `--fail-under-lines/regions/functions` but no branch threshold (verified with `cargo llvm-cov --help`).

### Task 2: `forge check` passes on its own repository

**Files:**
- Modify: `crates/forge-tooling/src/manifest.rs` (`parse_toml`, `load_project`)
- Modify: `crates/forge-tooling/src/architecture.rs` (`module_edges`)
- Modify: `crates/forge-tooling/src/check.rs` (`parse_cargo_messages`)
- Test: `crates/forge-tooling/tests/check.rs`

**Interfaces:**
- Produces: `parse_toml(...) -> Result<T, Box<Diagnostic>>`, `module_edges(...) -> Result<Vec<Edge>, Box<Diagnostic>>` (crate-private). Public API of `forge-tooling` is unchanged.

- [ ] **Step 1: Write the failing test** `clippy_duplicate_messages_are_reported_once` in `tests/check.rs`: the fake runner returns the same `compiler-message` JSON line twice (clippy emits one per target: lib and lib-test) and exits 101; assert the clippy result has exactly one diagnostic with that code, file and line.
- [ ] **Step 2:** `cargo nextest run -p forge-tooling clippy_duplicate` → FAIL (2 diagnostics).
- [ ] **Step 3:** Deduplicate in `parse_cargo_messages` on `(severity, code, message, file, line, column)`, keeping first-seen order. Fix `result_large_err` by returning `Box<Diagnostic>` from `parse_toml` and `module_edges`; fix `too_many_lines` by extracting two private helpers from `load_project`: `load_module` (reads and validates one module manifest) and `validate_dependencies` (cross-module `allowed`/`forbidden` checks). Behaviour and diagnostic codes unchanged.
- [ ] **Step 4:** `cargo nextest run --workspace` → all pass (59). `cargo run -q -p forge-cli -- check` → `forge.check: PASS`, exit 0.

### Task 3: `forge coverage` — per-module line and branch gate

**Files:**
- Create: `crates/forge-tooling/src/coverage.rs`
- Modify: `crates/forge-tooling/src/lib.rs` (`pub mod coverage;`)
- Modify: `crates/forge-cli/src/main.rs` (subcommand `coverage`)
- Test: `crates/forge-tooling/tests/coverage.rs`, `crates/forge-cli/tests/cli.rs`

**Interfaces:**
- Consumes: `manifest::load_project`, `manifest::Module { name, path, verification, .. }`, `report::{CheckReport, CheckResult, Diagnostic, Span}`.
- Produces:
  - `pub struct CoverageOptions { pub root: PathBuf, pub input: PathBuf }`
  - `pub fn run_coverage(options: &CoverageOptions) -> CheckReport` — tool name `"forge.coverage"`, one `CheckResult` per module with id `coverage:<module>`, in `forge.toml` order.
  - `pub struct Counts { pub count: u64, pub covered: u64 }` with `pub fn percent(self) -> f64` (`count == 0` → `100.0`).
  - `pub struct FileCoverage { pub file: PathBuf, pub lines: Counts, pub branches: Counts }`
  - `pub fn parse_llvm_export(text: &str) -> Result<Vec<FileCoverage>, Diagnostic>` — reads `data[0].files[].{filename, summary.lines, summary.branches}` of `llvm-cov export -format=json` (as written by `cargo llvm-cov --json --summary-only`).
  - CLI: `forge coverage --input <PATH> [--root .] [--format text|json]`, same exit codes as `check`.
- New codes: `FV001` line coverage below threshold, `FV002` branch coverage below threshold, `FV003` module has no measured files (NOT_VERIFIED), `FV004` input unreadable or not an llvm-cov export (NOT_VERIFIED for every module), `FV005` measured file outside every module, reported (error severity) in an extra result `coverage:unowned` that FAILs — unowned measured code is code without a gate.

- [ ] **Step 1: Write the failing tests** in `tests/coverage.rs`, building inputs as JSON strings with absolute paths inside a `TempProject` (reuse `tests/common`):
  - `full_coverage_passes`: one module, one file, lines 10/10, branches 4/4 → report PASS.
  - `missing_branch_fails_with_file_span`: branches 3/4 → module FAIL with one `FV002` whose message names the file and contains `3/4` and whose span is `Span::line_only(file, 1)` (summary exports carry no line numbers).
  - `missing_line_fails`: lines 9/10 → `FV001`.
  - `files_without_branches_count_as_full`: branches 0/0 → PASS.
  - `thresholds_come_from_module_manifest`: module declares `line_coverage = 90`, lines 9/10 → PASS.
  - `module_without_measured_files_is_not_verified` (Review Focus 2) → `FV003`, report NOT_VERIFIED.
  - `paths_outside_root_never_pass` (Review Focus 1): every filename under another directory → every module `FV003`, plus `coverage:unowned` FAIL with `FV005`; overall never PASS.
  - `invalid_input_is_not_verified`: input `{"type":"x"}` and a missing file → `FV004`, report NOT_VERIFIED.
  - `module_paths_do_not_match_by_string_prefix`: modules `a` → `crates/a` and `ab` → `crates/ab`; a file in `crates/ab/src/lib.rs` is attributed to `ab` only.
  - In `cli.rs`: `coverage_command_exit_codes` — PASS input → exit 0, failing input → exit 1, missing input → exit 3, `--format json` output has `"tool": "forge.coverage"` and `"schema_version": 1`.
- [ ] **Step 2:** `cargo nextest run --workspace coverage` → FAIL (module missing).
- [ ] **Step 3:** Implement `coverage.rs` and the subcommand. Attribution: canonicalize module paths and filenames, attribute a file to the module whose path is the longest `Path::starts_with` match (component-wise, which also satisfies the prefix test). Aggregate per module (sum of covered / sum of count) and compare with `verification.line_coverage` / `branch_coverage`; when a module fails, emit one diagnostic per file with missed lines or branches, worst first.
- [ ] **Step 4:** `cargo nextest run --workspace` → all pass. Inspect a real export once to confirm the field names: `cargo +<COVERAGE_TOOLCHAIN> llvm-cov nextest --branch --workspace --json --summary-only --output-path target/coverage/coverage.json` then `jq '.data[0].files[0].summary | keys' target/coverage/coverage.json` → contains `"branches"` and `"lines"`.
- [ ] **Step 5:** `cargo run -q -p forge-cli -- coverage --input target/coverage/coverage.json` → FAIL listing the current gaps (expected: today's ~97.8 % lines / ~90.3 % branches). This is the gate's first real run; keep the output for Task 4.

### Task 4: 100/100 on `forge-tooling` and `forge-cli`

**Files:**
- Modify: whichever `crates/forge-tooling/src/*.rs`, `crates/forge-cli/src/main.rs` and tests the Task 3 gate names.
- Create: `docs/evidence/phase0/coverage-gate.txt`

**Interfaces:**
- Consumes: `forge coverage` (Task 3). Produces: no API change visible to later tasks, except possibly `report::CheckReport::render_json(&self) -> String` if the CLI's JSON rendering moves into the library (see Step 2).

- [ ] **Step 1:** For each file the gate reports, list each missed line/branch with `cargo +<COVERAGE_TOOLCHAIN> llvm-cov report --branch --show-missing-lines` (or the HTML report) and classify it: (a) behaviour without a test → write the test first, then confirm it covers the gap; (b) unreachable by construction → remove it by type design (e.g. infallible `Serialize` paths: render through a function whose error case a test can trigger with a type that fails to serialize, such as a map with non-string keys, or make the case unrepresentable). No `#[coverage(off)]`, no `--ignore-filename-regex`, no ledger entry.
- [ ] **Step 2:** Re-run the Task 3 Step 4 export and `forge coverage`. Expected: `forge.coverage: PASS`, exit 0, `coverage:forge-tooling` and `coverage:forge-cli` both PASS.
- [ ] **Step 3:** Save the command lines and the gate output (text format) to `docs/evidence/phase0/coverage-gate.txt`.

### Task 5: `forge-testing` — database per test from a template

**Files:**
- Create: `crates/forge-testing/Cargo.toml`, `crates/forge-testing/forge.module.toml` (`kind = "testing"`, `allowed = []`, `forbidden = ["forge-cli"]`, 100/100), `crates/forge-testing/src/lib.rs`, `src/server.rs`, `src/seed.rs`, `src/database.rs`
- Modify: `Cargo.toml` (members, workspace deps), `forge.toml` (`forge-testing = "crates/forge-testing"`), `crates/forge-tooling/src/manifest.rs` (`ModuleKind::Testing`, serialized `"testing"`)
- Test: `crates/forge-testing/tests/postgres.rs`, unit tests in `src/server.rs`, `src/seed.rs`, `src/database.rs`; `crates/forge-tooling/tests/manifest.rs` (`testing_kind_is_accepted`)

**Interfaces:**
- Produces (P1 builds factories and `TestApp` on these):
  - `pub enum HarnessError { BlockedInfrastructure(String), Database(sqlx::Error), Container(String) }` (`thiserror`); `BlockedInfrastructure`'s message starts with `BLOCKED_INFRASTRUCTURE:` and names both remedies (set `FORGE_TEST_PG_URL`, or start Docker).
  - `pub trait ContainerStarter { fn start(&self) -> impl Future<Output = Result<String, HarnessError>> + Send; }` and `pub struct DockerStarter;` (the real one).
  - `pub async fn resolve_server(env_url: Option<String>, starter: &impl ContainerStarter) -> Result<String, HarnessError>` — env URL wins; otherwise the starter; a starter error becomes `BlockedInfrastructure`.
  - `pub async fn server_url() -> Result<String, HarnessError>` — process-wide `tokio::sync::OnceCell<String>` over `resolve_server(std::env::var("FORGE_TEST_PG_URL").ok(), &DockerStarter)`. Only the URL is cached, never a pool (pools are bound to the runtime that made them; each `#[tokio::test]` has its own runtime).
  - `pub struct Seed` with `pub fn empty() -> Seed`, `pub fn sql(sql: impl Into<String>) -> Seed`, `pub fn hash(&self) -> String` (first 16 hex chars of SHA-256 over `"forge-testing/1\n"` + SQL). Phase 1 replaces the SQL input with migrations/IR; the hash input is the only contract.
  - `pub struct TestDatabase { url: String, name: String, admin_url: String }` with `pub async fn create(seed: &Seed) -> Result<TestDatabase, HarnessError>`, `pub fn url(&self) -> &str`, `pub fn name(&self) -> &str`, `pub async fn pool(&self) -> Result<sqlx::PgPool, HarnessError>`, and `impl Drop`.
  - `pub async fn sweep_stale(admin_url: &str, older_than: std::time::Duration) -> Result<Vec<String>, HarnessError>` — returns dropped names.
  - crate-private `fn stale_timestamp(name: &str) -> Option<u64>` — the `unix_secs` embedded in a `forge_test_…` name.
- Mechanism (decided here):
  - Names: template `forge_tpl_<hash>`; test databases `forge_test_<unix_secs>_<pid>_<counter>` (process-local atomic counter), always ≤ 63 bytes, `[a-z0-9_]` only.
  - Template creation under `pg_advisory_lock` keyed by the hash, on a maintenance connection to the `postgres` database: if `forge_tpl_<hash>` is absent, create `forge_tpl_<hash>_building`, apply the seed over a connection that is closed afterwards, `ALTER DATABASE … RENAME TO forge_tpl_<hash>`, then `ALTER DATABASE … WITH IS_TEMPLATE true ALLOW_CONNECTIONS false` (no session can ever block `CREATE DATABASE … TEMPLATE`). A leftover `_building` database from a crashed run is dropped before retrying.
  - `create`: `CREATE DATABASE "forge_test_…" TEMPLATE "forge_tpl_<hash>"`; identifiers come only from the generated names above, quoted, never from input.
  - `Drop`: on a separate `std::thread` with its own current-thread runtime, `DROP DATABASE IF EXISTS "<name>" WITH (FORCE)`, then join. Runs during unwinding, so panicking tests clean up.
  - `server_url()` runs `sweep_stale(…, 1 hour)` once per process. The sweep drops only `forge_test_%` databases whose embedded `unix_secs` is older than the threshold **and** have no row in `pg_stat_activity`, using plain `DROP DATABASE` (no `FORCE`) and ignoring "being accessed" errors — so a live test's database is never dropped.
  - `DockerStarter`: `testcontainers_modules::postgres::Postgres::default().with_tag("17-alpine")`, `.with_container_name("forge-test-postgres-17")`, `.with_reuse(ReuseDirective::Always)` (testcontainers feature `reusable-containers`, experimental), guarded by an exclusive `std::fs::File::lock` on `std::env::temp_dir()/forge-test-postgres.lock` so parallel test processes start one container. Returns `postgres://postgres:postgres@<host>:<port>/postgres`. Verify each of these names against the 0.27.3 sources before use (CLAUDE.md rule 8). The container is left running for reuse and documented in CONTRIBUTING (`docker rm -f forge-test-postgres-17`).

- [ ] **Step 1: Write the failing unit tests:** `resolve_prefers_env_url` (fake starter must not be called), `resolve_uses_starter_without_env`, `resolve_reports_blocked_infrastructure` (starter fails → `BlockedInfrastructure`, message starts with `BLOCKED_INFRASTRUCTURE:` and contains `FORGE_TEST_PG_URL` and `Docker`), `seed_hash_is_stable_and_content_sensitive` (pin one literal expected hash), `test_database_names_are_safe_identifiers` (1000 generated names: unique, ≤ 63 bytes, `[a-z0-9_]+`), `stale_name_parsing` (`forge_test_100_7_3` → `Some(100)`, `forge_test_x` → `None`, `other_db` → `None`). Add `testing_kind_is_accepted` to `forge-tooling/tests/manifest.rs`.
- [ ] **Step 2: Write the failing integration tests** in `tests/postgres.rs` (all against `server_url()`; none returns early when PG is missing — they fail with the `BLOCKED_INFRASTRUCTURE` message):
  - `databases_are_isolated_and_seeded`: seed `CREATE TABLE t (id int)`; insert into db A; db B sees the table and zero rows.
  - `concurrent_creates_share_one_template` (Review Focus 4): a seed unique to this run (`SELECT <random literal>` comment), 8 concurrent `create` → all `Ok`; exactly one `pg_database` row named `forge_tpl_<hash>`, with `datistemplate = true` and `datallowconn = false`.
  - `drop_removes_database_even_on_panic`: create inside a thread that panics; after join, the name is absent from `pg_database`.
  - `sweep_drops_stale_but_never_live_databases` (Review Focus 3): raw-create `forge_test_100_1_1` (stale, idle) and `forge_test_100_1_2` (stale, held open by a live connection); `sweep_stale(…, 1 h)` returns exactly the first; the second still exists; a fresh `TestDatabase` is untouched.
- [ ] **Step 3:** `cargo nextest run -p forge-testing` → FAIL (crate does not compile yet).
- [ ] **Step 4:** Implement the crate per the Interfaces and Mechanism above.
- [ ] **Step 5:** With Docker running and `FORGE_TEST_PG_URL` unset: `cargo nextest run --workspace` → all pass (the container `forge-test-postgres-17` appears once in `docker ps`). With a separately started `docker run -d -e POSTGRES_PASSWORD=postgres -p 55432:5432 postgres:17-alpine` and `FORGE_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`: all pass again. With `FORGE_TEST_PG_URL` pointing at a closed port: integration tests FAIL with `BLOCKED_INFRASTRUCTURE` (record this output as evidence).
- [ ] **Step 6:** `forge check` → PASS (architecture guard sees the new module); coverage export + `forge coverage` → PASS for all three modules. Append both outputs to `docs/evidence/phase0/coverage-gate.txt`.

### Task 6: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `forge check`, `forge coverage` (Tasks 2–3), `COVERAGE_TOOLCHAIN` (Task 1), `FORGE_TEST_PG_URL` (Task 5).

- [ ] **Step 1:** Write the workflow (triggers: `push` to `main`, `pull_request`; `permissions: contents: read`; `concurrency` cancels superseded runs; `env: CARGO_TERM_COLOR: always`, `COVERAGE_TOOLCHAIN: <from Task 1>`). Every third-party action pinned to a full commit SHA with a `# vX.Y.Z` comment (resolve with `git ls-remote https://github.com/<owner>/<repo> refs/tags/<tag>`). Jobs:
  - `check` (ubuntu-latest): checkout; toolchain from `rust-toolchain.toml` (`rustup show`); `Swatinem/rust-cache`; `cargo run --locked -p forge-cli -- check --format json` (fmt + clippy + manifests + architecture).
  - `test`: service `postgres:17-alpine` (`POSTGRES_PASSWORD: postgres`, port 5432, `pg_isready` health check); `FORGE_TEST_PG_URL: postgres://postgres:postgres@localhost:5432/postgres`; nextest via `taiki-e/install-action`; `cargo nextest run --locked --workspace`.
  - `coverage`: no `FORGE_TEST_PG_URL` (exercises the Docker path that fresh clones use); install `COVERAGE_TOOLCHAIN` with `llvm-tools-preview`; `cargo-llvm-cov` + nextest via `taiki-e/install-action`; the Task 3 Step 4 export command with `--locked`; `cargo run --locked -p forge-cli -- coverage --input target/coverage/coverage.json`; upload `target/coverage/` as an artifact (`if: always()`).
  - `mutants` (informational in Phase 0): `continue-on-error: true`, `timeout-minutes: 30`, PG service as in `test`; `cargo mutants --workspace --in-place` — on PRs restricted to the diff (`git diff origin/${{ github.base_ref }}...HEAD > pr.diff` then `--in-diff pr.diff`, checkout with `fetch-depth: 0`), full run on `main`; upload `mutants.out/` (`if: always()`). The job name says `(informational)` so its non-gating status is visible, not silent; Phase 4 makes it gating with a threshold (matrix row T-mutation).
- [ ] **Step 2:** Validate syntax with `actionlint` if installable (`brew install actionlint`); otherwise mark syntax `NOT_VERIFIED`.
- [ ] **Step 3:** Run each job's commands locally in the same order (with the Task 5 PG container for `test`) and record results. Run `cargo mutants --package forge-tooling --timeout 120` once locally; record caught/missed/timeout counts in `docs/evidence/phase0/mutants.txt`, add tests for missed mutants in `report.rs`/`manifest.rs`/`coverage.rs` where the mutant reveals untested behaviour, and list the rest with a one-line reason each.
- [ ] **Step 4:** The workflow itself stays `BLOCKED_INFRASTRUCTURE` until the owner creates the GitHub repository and pushes (design D16); the final report says so.

### Task 7: Open-source documents and threat model skeleton

**Files:**
- Create: `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `docs/security/threat-model.md`

- [ ] **Step 1:** `LICENSE-APACHE`: copy verbatim from a crate in `~/.cargo/registry/src` that ships the unmodified Apache-2.0 text (e.g. `serde-1.0.229/LICENSE-APACHE`; confirm with `diff` against a second crate's copy). `LICENSE-MIT`: standard MIT text, `Copyright (c) 2026 The Forge contributors`.
- [ ] **Step 2:** `README.md` (English): what Forge is (design §1), status "pre-alpha, Phase 0", a quickstart that is exactly the fresh-clone commands of the exit criterion, links to SPEC, design, ADRs, matrix. `CONTRIBUTING.md`: toolchains (stable pin + `COVERAGE_TOOLCHAIN`), `cargo nextest run`, `forge check`, the coverage commands, PG via Docker or `FORGE_TEST_PG_URL`, removing the reusable container, Conventional Commits, "no test may be skipped silently", ADR process. `SECURITY.md`: supported versions (none before 0.1.0), private reporting through GitHub Security Advisories once the repository exists, response targets, no promise of absolute security (SPEC §11). `CHANGELOG.md`: Keep a Changelog format, `## [Unreleased]` listing Phase 0 deliverables.
- [ ] **Step 3:** `docs/security/threat-model.md`: assets, trust boundaries and STRIDE table per SPEC §11 areas (SQL, authz, sessions/CSRF, uploads, admin, plugins/apps, tooling/MCP grants, CI/supply chain, secrets), each row with mitigation owner phase and status `PLANNED` (Phase 0 implements none beyond CI permissions and pinned actions — say so).
- [ ] **Step 4:** Check every relative link in the new files resolves: `grep -o '](\./[^)]*\|](docs/[^)]*\|](SPEC[^)]*' README.md CONTRIBUTING.md` and `test -e` each target.

### Task 8: ADR-0003, matrix, phase evidence and exit demonstration

**Files:**
- Create: `docs/adrs/0003-safe-query-strategy.md`, `docs/evidence/phase0/exit.txt`
- Modify: `docs/adrs/README.md`, `docs/requirements-matrix.md`

- [ ] **Step 1:** Write ADR-0003 (Status: Proposed) from design D2: the three tiers, the `SqlSafeStr`/`AssertSqlSafe`/`raw_sql` evidence (sqlx 0.9.0), the clippy `disallowed-methods`/`disallowed-types` plus architecture-guard enforcement, the privileged escape hatch and its exception register, the Phase 1 experiment that closes it (`.sqlx` offline snapshot staleness detected by `forge check`; a deliberate `AssertSqlSafe` in an app crate rejected), reversibility. Update the ADR index.
- [ ] **Step 2:** Exit demonstration on a fresh copy: `rsync -a --exclude target --exclude .git ./ <scratchpad>/forge-fresh/` then in that copy, with Docker running and `FORGE_TEST_PG_URL` unset: `cargo nextest run --locked --workspace`, `cargo run --locked -p forge-cli -- check`, the coverage export + `forge coverage`. Record command lines, exit codes and summaries in `docs/evidence/phase0/exit.txt`.
- [ ] **Step 3:** Update `docs/requirements-matrix.md`: Phase 0 rows to `VERIFIED` only where Step 2 or earlier evidence files prove them (with the evidence path), CI rows to `BLOCKED_INFRASTRUCTURE` (repository not created), mutation to `IN_PROGRESS (informational)`, add rows for `forge coverage` and `forge-testing`.
- [ ] **Step 4:** Final report in the CLAUDE.md format. Phase 0 is reported complete except "CI green on the first commit", which remains `BLOCKED_INFRASTRUCTURE` until the owner creates the repository and authorizes the first commit and push.
