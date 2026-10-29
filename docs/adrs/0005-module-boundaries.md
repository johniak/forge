# ADR-0005 Module boundaries and architecture guard

**Status:** Accepted for Phase 0 scope (manifest format + Cargo-graph enforcement);
symbol-level enforcement Proposed for Phase 5.  
**Date:** 2026-10-29  
**SPEC:** §3.2, §7.3, §8.2 (`forge.architecture.check`). Matrix: B-05, R-03.

## Context

Boundaries must be validated from real code, not from manifest declarations
alone, and waivers must be impossible for an agent to self-grant.

## Decision

### Manifests

- `forge.toml` at the project root lists modules: `[modules] <name> = "<path>"`.
- Every module directory holds `forge.module.toml`:

```toml
[module]
name = "orders"            # must equal the key in forge.toml
kind = "domain"            # domain | tooling | cli | runtime | app | shared
description = "…"

[dependencies]
allowed = ["customers"]    # other Forge modules this one may depend on
forbidden = ["payments"]   # explicit denials (documentation + guard)

[verification]
line_coverage = 100
branch_coverage = 100
```

### Enforcement (Phase 0)

`forge check` reads each module's **real** `Cargo.toml` (`[dependencies]`,
`[dev-dependencies]`, `[build-dependencies]`, including `workspace = true`
entries resolved through `[workspace.dependencies]`), maps path dependencies to
module names, and reports:

- `FA001` dependency on a module not in `allowed`;
- `FA002` dependency on a module listed in `forbidden`;
- `FA003` dependency cycle between modules.

Diagnostics carry the byte span of the offending dependency entry.

### Enforcement (Phase 5, proposed)

Symbol-level analysis of `use` paths and generated-code consumers through the
project graph, so that re-exports and `pub(crate)` leaks are caught.

### Waivers

Waivers live in `forge.waivers.toml`, each with `rule`, `module`, `reason`,
`approved_by`, `expires` (ISO date). The guard rejects expired waivers and
waivers without approver. The file is protected by CODEOWNERS; an agent may
propose a waiver but the check fails until a human-owned review merges it.
(Waiver parsing is not in Phase 0; the guard has no waiver path yet, which is
the fail-closed default.)

## Consequences

The framework repo dogfoods the format: `forge-tooling` may depend on nothing
Forge-internal; `forge-cli` may depend on `forge-tooling` only.

## Reversibility

High: manifests are data; enforcement strictness only grows.

## Evidence

Tests in `crates/forge-tooling/src/manifest.rs` and `architecture.rs`; the
repository's own `forge check` run (see Phase 0 report).
