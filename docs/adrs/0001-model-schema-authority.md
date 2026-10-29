# ADR-0001 Model schema authority

**Status:** Proposed — closed by the Phase 1 vertical slice.  
**Date:** 2026-10-29  
**SPEC:** §0 (open decision), §4.1, §17 Q1–Q2. Matrix: O-02, O-03, M-01.

## Context

Forge needs one authoritative description of every model (fields, types,
nullability, enums, relations, constraints, indexes, access annotations) that
feeds migrations, admin, forms, API contracts, the context engine and test
obligations. SPEC forbids hand-maintaining divergent copies.

## Options

1. **Rust declarations are authoritative.** A `#[derive(ForgeModel)]` macro
   emits, next to the struct, a `const`/`static` `ModelSchema` value; a
   registry collects them (explicit registration list per app, no link-time
   magic in v1). The IR is produced by running a generated binary/`forge`
   subcommand that serializes the registry. Migrations are diffed against the
   IR; the live DB is diffed against the IR (`forge db diff`).
2. **Schema-first IDL** (TOML/KDL) generates Rust types and SQL. Tooling can
   read the IDL without compiling, but authors write two artifacts and
   business code must live in generated-code-friendly places.
3. **Database introspection is authoritative.** Rust types are generated from
   PG. Fast to start, but loses intent (enums, access annotations, relations
   semantics) and makes migrations the only source of truth.

## Experiment (Phase 1)

Implement option 1 for a single `customers` model: derive macro → `ModelSchema`
→ `forge schema inspect --format json` → generated initial migration → apply on
empty PG → `forge db diff` reports zero drift. Then mutate the DB by hand and
verify drift is reported with the offending column. Measure how much macro
code is needed and whether tooling latency (needs a build) is acceptable.

Fallback trigger: if extracting metadata requires compiling the whole app for
every tooling call and that exceeds ~10 s on the demo app, revisit option 2
with the macro kept as a thin consumer of the IDL.

## Consequences

Option 1 keeps "business logic in Rust" and one place to look; it makes the
tooling depend on a build artifact, which is acceptable because `forge check`
already compiles.

## Reversibility

Medium: the IR schema is the stable contract; swapping the producer (macro vs
IDL) does not change consumers.

## Evidence

None yet. To be attached from the Phase 1 slice.
