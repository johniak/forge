# ADR-0007 Framework overhead budget

**Status:** Proposed — numbers below are starting proposals, not validated claims.  
**Date:** 2026-10-29  
**SPEC:** §1.5, §14, §21.1. Matrix: X-01, P-02.

## Context

The product owner requires the framework itself to be fast. SPEC §14 asks for
comparison with an equivalent plain Axum baseline; this ADR turns it into a
standing, CI-enforced budget.

## Decision (method)

`benchmarks/overhead/` contains two binaries built from the same handler code:

- `baseline`: plain Axum + SQLx, hand-written router, no Forge.
- `forge`: identical endpoints through the Forge runtime (registry, middleware
  stack, policy check, typed action, error mapping).

Endpoints: `GET /ping` (JSON), `GET /customers/{id}` (one PG row),
`POST /customers` (insert in a transaction), `GET /customers?limit=50`
(paginated list through the constrained query builder once it exists).

Load generator: `oha` (pinned version) at fixed concurrency 64, 30 s, 3 runs,
against a local PG seeded with 100 000 customers; report req/s, p50/p95/p99,
RSS after warm-up, cold start to first healthy response, binary size.

## Proposed initial budgets (to be replaced by measured values)

| Metric | Budget vs baseline |
|---|---|
| Throughput | ≥ 90% of baseline |
| p50 / p99 latency | ≤ baseline + 100 µs / + 1 ms |
| RSS | ≤ baseline + 20 MB |
| Cold start | ≤ baseline + 50 ms |

The first measurement (Phase 1) replaces these with observed numbers plus an
explicit margin; CI fails on regression beyond the margin on the pinned runner.

## Consequences

Every runtime feature pays for itself or is opt-in. Benchmarks run on a fixed
machine profile; cloud CI noise is handled by comparing against the baseline
built in the same job, never against historic absolute numbers.

## Reversibility

High.
