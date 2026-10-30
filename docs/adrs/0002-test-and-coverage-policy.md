# ADR-0002 Test and coverage policy

**Status:** Accepted for the coverage mechanism (Phase 0 experiment done);
thresholds and mutation policy remain Proposed.  
**Date:** 2026-10-29  
**SPEC:** §9, §13, §17 Q8. Matrix: N-09, V-02, V-03, O-05.

## Context

SPEC demands 100% measurable line **and branch** coverage of application-owned
logic. Whether branch coverage is obtainable with the pinned stable toolchain
had to be proven, not assumed.

## Experiment and result (run 2026-10-29, verbatim log in
`docs/evidence/phase0/coverage-experiment.txt`)

| Toolchain | Command | Result |
|---|---|---|
| stable 1.95.0 | `cargo llvm-cov --branch --summary-only` | **FAIL**: rustc rejects `-Z coverage-options=branch` ("1 nightly option were parsed") |
| nightly 1.98.0 (2026-06-12) | `cargo +nightly llvm-cov --branch --summary-only` | **PASS**: report includes a `Branches` column |

Observation: Rust's branch instrumentation counts boolean conditions
(`if`/`while`/`&&`/`||`); `match` arms are covered as regions, not branches.
The Phase 0 code at experiment time had no `if`, hence `Branches 0`. Region
coverage therefore stays a reported metric alongside lines and branches.

## Decision

1. **Build and test on pinned stable** (`rust-toolchain.toml` = 1.95.0).
2. **Coverage runs on a pinned nightly** with `llvm-tools-preview`; the CI
   coverage job uses `cargo +nightly-2026-06-12 llvm-cov --branch`
   (date pinned in CI, bumped deliberately). A coverage job that cannot run
   reports `NOT_VERIFIED`, never a pass.
3. **Scope:** workspace crates under `crates/` are "framework-owned" and must
   hit 100% lines and 100% branches; `#[cfg(test)]` modules are **not**
   excluded by the tool (corrected by Amendment item 4); any exclusion needs
   an entry in `docs/coverage-exclusions.md`
   with reviewer, reason and expiry. An agent may not add entries for code it
   changed in the same change.
4. **Binaries** (`forge-cli`) are covered through integration tests that spawn
   the instrumented binary (`CARGO_BIN_EXE_forge`); cargo-llvm-cov merges those
   profiles.
5. **Tools:** `cargo-nextest` for the test runner, `cargo-llvm-cov` for
   coverage, `cargo-mutants` for mutation runs. Versions pinned in CI.

## Proposed (unvalidated) defaults

- Mutation: `cargo mutants` on changed crates per PR, full run nightly;
  initial "caught" threshold 80% to be replaced by measured data after Phase 1.
- Property tests with `proptest` for validation boundaries; fuzzing with
  `cargo-fuzz` for the query builder from Phase 2.

## Consequences

Two toolchains to install (stable + one nightly). Coverage numbers come from a
compiler one minor ahead of the one that ships; acceptable because coverage is
a gate on tests, not on codegen.

## Reversibility

High: if `-C instrument-coverage` branch support stabilizes, drop the nightly.

## Evidence

`docs/evidence/phase0/coverage-experiment.txt`

## Amendment 2026-10-29 (proposed, awaiting owner approval)

1. **Pinned coverage toolchain is `nightly-2026-06-13`.** The name
   `nightly-2026-06-12` in Decision 2 resolves to
   `rustc 1.98.0-nightly (b30f3df3b 2026-06-11)`; the build measured in the
   experiment is `rustc 1.98.0-nightly (654079540 2026-06-12)`, shipped as
   `nightly-2026-06-13` (rustup names a nightly by its release date, the day
   after the commit date). CI and `CONTRIBUTING.md` use
   `COVERAGE_TOOLCHAIN=nightly-2026-06-13`.
2. **Subprocess hygiene.** Tests that spawn `cargo` (the `forge check`
   integration tests) remove the variables a coverage run sets for its own
   build — `RUSTUP_TOOLCHAIN`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`,
   `RUSTDOCFLAGS`, `CARGO_ENCODED_RUSTDOCFLAGS`, `CARGO_TARGET_DIR`,
   `CARGO_BUILD_TARGET_DIR`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER` and
   every `CARGO_LLVM_COV*` — and pin fixture projects to the repository's
   `rust-toolchain.toml`. `LLVM_PROFILE_FILE` is kept so the instrumented
   binary still records coverage. Without this, 3 of 58 tests failed under
   `cargo +nightly-2026-06-13 llvm-cov nextest --branch` (fixture `cargo fmt`
   and `cargo clippy` ran on the nightly, which has neither component).
   Installing clippy into the nightly was rejected: test results would then
   depend on which clippy ran.
3. **The branch gate is Forge code.** `cargo llvm-cov` offers
   `--fail-under-lines`, `--fail-under-regions` and `--fail-under-functions`
   but no branch threshold (checked with `cargo llvm-cov --help`, 2026-10-29).
   `forge coverage` reads the `--json --summary-only` export and enforces each
   module's `[verification]` thresholds from `forge.module.toml`.
4. **What the tool excludes, and what `forge coverage` refuses** (from the
   Phase 0 final review). Decision 3's "`#[cfg(test)]` modules are excluded
   by the tool" was wrong: cargo-llvm-cov 0.8.7 measures them like any other
   code in `src/`. Its default `--ignore-filename-regex` drops, anywhere in
   the workspace, `tests/`, `examples/` and `benches/` directories and files
   named `tests.rs` or `<name>_tests.rs` / `<name>-tests.rs` (`src/report.rs`
   of v0.8.7), plus the target directory, cargo registry/git sources and
   rustup toolchains. Silent drops are not passes, so `forge coverage`:
   - reports `NOT_VERIFIED` (`FV006`) for every module when the export has
     no branch data at all (0 branches in every file: `--branch` missing or
     a stable toolchain);
   - reports `NOT_VERIFIED` (`FV007`) for each `.rs` file under a module's
     `src/` that the default filter would drop;
   - fails (`FV008`) a module whose threshold is below 100, because no
     owner-approved exclusion ledger exists before Phase 4 (D14); a value
     above 100 is a manifest error (`FM008`).
