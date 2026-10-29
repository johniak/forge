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
   hit 100% lines and 100% branches; `#[cfg(test)]` modules are excluded by the
   tool; any other exclusion needs an entry in `docs/coverage-exclusions.md`
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
