# Architecture Decision Records

One file per decision, numbered, never rewritten after acceptance (supersede
instead). Template: Status · Date · Context · Options · Decision or Experiment ·
Consequences · Reversibility · Evidence. "Proposed" means the choice is still
open and the ADR names the experiment that closes it; agents must not build on a
proposed decision as if it were accepted (SPEC §17).

| # | Title | Status |
|---|---|---|
| 0001 | [Model schema authority](0001-model-schema-authority.md) | Proposed |
| 0002 | [Test and coverage policy](0002-test-and-coverage-policy.md) | Accepted for Phase 0 (branch coverage mechanism); thresholds proposed; amendment 2026-10-29 (pinned nightly, `forge coverage` gate) proposed |
| 0003 | [Safe query strategy](0003-safe-query-strategy.md) | Proposed |
| 0004 | [Frontend benchmark protocol](0004-frontend-benchmark-protocol.md) | Proposed |
| 0005 | [Module boundaries and architecture guard](0005-module-boundaries.md) | Accepted for Phase 0 scope (Cargo-graph level) |
| 0006 | [Built-in E2E harness and HOPE-style fixtures](0006-e2e-harness-and-fixtures.md) | Proposed |
| 0007 | [Framework overhead budget](0007-framework-overhead-budget.md) | Proposed |
