# ADR-0004 Frontend benchmark protocol

**Status:** Proposed — executed in Phase 3; selection recorded in a follow-up ADR.  
**Date:** 2026-10-29  
**SPEC:** §0 (open), §6.2, §14, §17 Q6. Matrix: N-06, N-07, F-01, O-01.

## Context

The renderer is chosen by evidence. This ADR fixes the protocol *before* any
candidate is built so the comparison cannot be tuned toward a favourite.

## Candidates

React 19 + React Compiler · SolidJS · Svelte 5 · Leptos · Qwik · (optional)
Dioxus. Each candidate gets the **same** typed API contract generated from the
Rust backend and the same seeded dataset.

## Scenarios (identical UX per candidate)

1. Admin table, 10 000 rows, virtualization, sort/filter/inline edit.
2. Nested dynamic form with relations, optimistic state, server errors.
3. Realtime dashboard, 20 updates/s, 4 charts.
4. Editor-like screen: autocomplete, 50 kB document, keyboard handling.
5. SSR public page cold start on throttled mobile.
6. Route transitions with loading/error/offline states.

## Measurements

Transferred bytes (JS/WASM split), parse+compile, startup, LCP, INP, CLS, p95
input delay, main-thread CPU, heap, SSR req/s, dev and prod build time, test
run time, coverage tooling availability, a11y audit score, lines of code and
"agent cycle" (generate → compile → test → fix) duration for one scripted change.

## Procedure

- Hardware: the development Mac (recorded model/OS) with Chrome DevTools CPU
  throttling 4× and 6× and network "Slow 4G"; browsers pinned by Playwright
  version. Repeat 5×, report median and p95; no single-run numbers.
- Driver: Playwright scripts under `benchmarks/frontend/<candidate>/`, one
  shared runner, results as JSON next to a `manifest.json` with commit hashes.
- Decision matrix weights fixed here: user-perceived performance 35%, agent
  ergonomics/testability 25%, ecosystem/complexity 20%, security surface 10%,
  SSR/ops cost 10%. Weights may be changed only by a superseding ADR before
  results exist.

## Consequences

Phase 3 is expensive (six apps). Mitigation: scenarios are small and share
backend and fixtures; candidates may be dropped early only with a recorded
reason.

## Reversibility

High until Phase 3 ends; the selection ADR must state the exit cost.
