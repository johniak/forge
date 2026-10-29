# Claude Code project instructions — Forge

## Canonical specification

Read `SPEC.md` in full before making architecture decisions. It is the **single source of product requirements**. This file supplies operational instructions, not alternative requirements.

**Goal:** implement an LLM-first, batteries-included full-stack web framework with a Rust backend, Django-like typed models/migrations/admin/apps/middleware, extraordinarily strong automated testing and UI screenshot verification, a performance-first frontend selected through comparative evidence, and first-party CLI/MCP tooling for agents.

## Non-negotiable engineering rules

1. **Do not silently replace, narrow or omit SPEC.md requirements.** If scope cannot be implemented in one iteration, explicitly place it in a phase with an exit criterion, and keep the complete requirements in scope.
2. **Do not assume React or a custom UI DSL is the final frontend.** Run an honest documented spike comparing relevant candidates before selecting one. Rust/WASM is not automatically fastest for DOM work.
3. **Rust backend, PostgreSQL-first, strongly typed models and tested migrations are mandatory.** A full ORM query language is not mandatory; safe SQLx/typed queries and constrained dynamic admin querying are.
4. **Default-deny security, parameterized SQL, centrally enforced object authorization.** Do not bypass safety controls for convenience.
5. **Target 100% measurable line and branch coverage for application-owned logic, plus required integration/E2E/security/migration/browser/visual tests.** Never fake coverage, skip checks silently, weaken thresholds, or self-approve a test exclusion.
6. **UI changes require real browser runs and screenshot review.** A screenshot is not "verified" unless captured in a controlled environment, compared against baseline (if one exists), and semantically inspected; baseline updates need independent approval.
7. **No tests run ⇒ do not say tests passed.** For missing environment/tooling, report the exact blocker and mark state as NOT_VERIFIED / BLOCKED_INFRASTRUCTURE.
8. **No invented API names as executable code.** Examples in SPEC.md are concepts. Check actual crate docs and choose compileable designs.
9. **Do not create a forest of empty crates or generic abstractions.** Prefer the smallest end-to-end executable vertical slice and refactor after evidence.
10. **Use the framework's own tools as they become available.** Dogfood `forge check`, verify, context, inspect, browser and tooling core.
11. **Do not deploy or execute destructive production migrations on your own.** Require explicit permission and independent review.
12. **Implement deterministic application runtime.** LLM API is a development tool, not a requirement to service HTTP requests.

## Initial workflow

1. Inspect the current repository. Identify whether Forge is already implemented or only the handoff files exist.
2. Read `SPEC.md`, `README_PL.md` (optional Polish overview) and existing ADRs, if present.
3. Produce a **requirement traceability matrix** linking each requirement category to milestones, expected crates/files and verification procedures. Keep it maintained in `docs/requirements-matrix.md`.
4. Write a short, evidence-grounded Phase 0 + Phase 1 implementation plan. Create ADR proposals for unresolved choices, especially model schema authority, safe query strategy, frontend benchmark protocol and coverage instrumentation.
5. **Start implementing Phase 0 immediately** if the repository is empty. Establish reproducible Rust workspace + CI + minimal `forge check` + PostgreSQL integration test harness. Then implement the first complete vertical feature. Do not stop after merely drafting a plan unless blocked by lack of necessary tools.
6. After each meaningful step, run the available checks. Give a status including: changed files, commands actually run and results, verification gaps and next smallest task.
7. Before claiming a phase complete, demonstrate its exit criteria with real evidence.

## Architecture conventions

- Feature/app-based repo layout; keep model, action, policy, admin, UI, migration, fixtures and tests within the owning domain.
- Stable public contracts; forbid unapproved cross-module internal imports.
- Central schema/graph/IR derived from real declarations (do not hand-maintain drifting copies).
- Pure/isolated domain logic when possible; typed capability injection for IO, time, storage, DB, events, external HTTP.
- App registry must support installable third-party native crates, migrations, middleware and admin extension; optional Wasm plugins later.
- Common tooling core must back **both** CLI and MCP, returning structured versioned JSON and artifact references.
- Retain compiler source spans and test trace references to allow short, high-quality agent diagnostics.

## Quality bar and reporting

Never equate 100% coverage with 100% correctness or claim all vulnerabilities eliminated. Instead enforce strong APIs, tests and evidence, and honestly report limitations.

For every change report:

```text
Implemented:
- [what actually changed]

Verified (commands actually executed):
- command — PASS/FAIL

Evidence:
- [paths to artifacts; test/coverage/screenshot logs]

Not yet verified / known risks:
- [explicit list, or "none" only if justified]

Next concrete step:
- [smallest useful next action]
```

### Work allowed without asking

Read/analyze source, create new local files, run compilation/tests, start isolated local services/test DBs and take screenshots of local app. Use normal engineering judgment to make reversible implementation decisions. For unknown foundational architecture choices, use ADRs and measurements instead of arbitrary assumptions. Require review for security gate waivers, new baseline screenshots, destructive migration operations and production deployments.

## Phase discipline

- P0 reproducible workspace, static checks, coverage prototype, CI, architecture rules, Postgres test infra.
- P1 Rust typed model/migration/action/policy/API integrated end-to-end.
- P2 generated extensible admin, native app registry and middleware.
- P3 benchmark frontends and select renderer by evidence; integrate typed API.
- P4 contractual verification, E2E and visual-review engine.
- P5 context/dependency graph, CLI/MCP parity and Claude adapters.
- P6 production batteries/hardening.
- P7 optional WASM plugin isolation and novel frontend compiler if warranted.

Provide *working* demonstrators for each phase. Don't skip or mark a phase complete when exit criteria cannot be reproduced.
