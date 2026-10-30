# Forge requirement traceability matrix

Single place where every requirement category from `SPEC.md` is tied to a
phase, the crates/files expected to satisfy it, and the procedure that proves
it. **A requirement may be deferred to a later phase; it may never be dropped.**
Keep this file updated in the same change that moves a requirement.

Status vocabulary: `NOT_STARTED`, `IN_PROGRESS`, `VERIFIED (evidence path)`,
`BLOCKED_INFRASTRUCTURE (reason)`. "Verified" always points to an artifact
produced by a command that was actually run.

Phases (SPEC §15): P0 baseline, P1 first backend feature, P2 admin + apps,
P3 frontend evaluation, P4 verification engine, P5 agent tooling, P6 hardening,
P7 optional research.

## A. Non-negotiables (SPEC §0)

| ID | Requirement | Phase(s) | Expected crates / files | Verification procedure | Status |
|---|---|---|---|---|---|
| N-01 | Rust backend on Tokio + Axum + Tower | P0 scaffold, P1 first route | `crates/forge-http` (extract when ≥2 consumers), `Cargo.toml` workspace, `rust-toolchain.toml` | `cargo build` on fresh checkout; integration test hitting an Axum route | IN_PROGRESS (workspace + toolchain pin exist; no HTTP yet) |
| N-02 | Full-stack coherence (backend, frontend, contracts, tooling, testing, deploy verification) | P1–P6 | whole workspace | Phase demonstrators (§15) | NOT_STARTED |
| N-03 | Django-like productivity: typed models, relations, migrations, admin, auth, permissions, installable apps, middleware, forms, jobs, cache, storage, events, CLI | P1 models/migrations/auth/policy; P2 admin/apps/middleware; P6 jobs/cache/storage/events | `forge-schema`, `forge-migrations`, `forge-auth`, `forge-admin`, `forge-apps` (extract as proven) | per-battery integration tests; admin E2E; app install test | NOT_STARTED |
| N-04 | No mandatory heavyweight ORM; first-class models + safe SQL + generated CRUD + restricted builder | P1, P2 | `forge-database` (query builder), ADR-0003 | SQLi attempt test: hostile input cannot change SQL syntax via safe APIs | NOT_STARTED (ADR-0003 proposed) |
| N-05 | PostgreSQL-first, real PG semantics | P0 test infra, P1 | `crates/forge-testing` (PG harness), migrations | integration tests on real PG (testcontainers or `FORGE_TEST_PG_URL`) | IN_PROGRESS (P0 harness VERIFIED: `docs/evidence/phase0/forge-testing.txt`, `docs/evidence/phase0/exit.txt`; P1 migrations pending) |
| N-06 | Very fast frontend measured, not assumed | P3 | `benchmarks/frontend/*`, ADR-0004 | reproducible benchmark report | NOT_STARTED (ADR-0004 proposed) |
| N-07 | Frontend choice OPEN until benchmark | P3 | ADR-0004 → selection ADR | decision matrix with raw data | NOT_STARTED |
| N-08 | Strict typing, single-source contracts across UI/API/models/validation/routing/errors | P1 (API+model), P3 (UI) | `shared/contracts`, generated clients | contract drift test: change a type → dependent build fails | NOT_STARTED |
| N-09 | 100% measurable line + branch coverage; unit/integration/E2E/property/security/migration/browser/a11y/visual tests; independent release gate | P0 prototype, P4 engine, every phase for its code | ADR-0002, `cargo-llvm-cov` config, `forge verify` | coverage report with threshold; gate fails on decrease/unmeasured | IN_PROGRESS (P0 gate VERIFIED: `forge coverage` 100/100 lines+branches per module, `docs/evidence/phase0/coverage-gate.txt`, `docs/evidence/phase0/exit.txt`; P4 engine pending) |
| N-10 | Mandatory screenshot analysis for UI work, baselines independently approved | P4 | `forge-visual`, `forge visual verify` | injected visual regression blocks release | NOT_STARTED |
| N-11 | Secure by construction: parameterized SQL, allowlisted identifiers, safe HTML, CSRF, XSS, object authz, secrets, plugin containment | P1 (SQL, authz), P2 (admin), P6 (full) | `forge-security`, policies, ADR-0003 | attack regression tests; `forge security audit` | NOT_STARTED |
| N-12 | LLM-native tooling: CLI + MCP from one core, project graph, context, impact, diagnostics, browser inspector, release evidence, adapters | P0 (`forge check`), P5 (rest) | `crates/forge-tooling` (core), `crates/forge-cli`, later `forge-mcp` | CLI/MCP parity test: same JSON for same request | IN_PROGRESS (`forge check`, `forge coverage` in `forge-tooling` + `forge-cli`; MCP P5) |
| N-13 | Deterministic runtime: no LLM calls to serve requests | all | architecture guard rule | guard test: runtime crates must not depend on LLM client crates | NOT_STARTED |
| N-14 | Modular monolith default; workers; optional scale-out | P1 (monolith), P6 (workers) | app binary + worker binary | deploy check | NOT_STARTED |
| N-15 | Extensibility by native Cargo apps; optional WASM plugins later | P2 (native), P7 (WASM) | `forge-apps`, ADR on plugin ABI | install third-party sample app without editing unrelated source | NOT_STARTED |

## B. Decisions that must stay open until evidence (SPEC §0)

| ID | Decision | Owner ADR | Experiment | Status |
|---|---|---|---|---|
| O-01 | Frontend architecture | ADR-0004 | benchmark suite P3 | Proposed |
| O-02 | Schema authority (derive vs IR vs introspection) | ADR-0001 | P1 vertical slice | Proposed |
| O-03 | Model persistence API (derive syntax, codegen, SQLx) | ADR-0001 + ADR-0003 | P1 vertical slice | Proposed (ADR-0003 written 2026-10-30; closed by the P1 slice) |
| O-04 | App/plugin ABI, DI container, admin customization API, UI format | ADR TBD (P2) | P2 experiments | NOT_STARTED |
| O-05 | Performance thresholds and mutation-testing policy | ADR-0002, ADR-0007 | measure in P1/P4 | Proposed (defaults flagged as unvalidated; P0 mutation baseline: `docs/evidence/phase0/mutants.txt`) |

## C. Backend runtime (SPEC §3)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| B-01 | Pinned stable toolchain + Cargo workspace | P0 | `rust-toolchain.toml`, `Cargo.toml` | fresh clone builds | VERIFIED (`docs/evidence/phase0/exit.txt`: fresh clone builds and tests with `--locked`) |
| B-02 | Tokio/Axum/Tower/tower-http stack | P1 | `forge-http` | route test | NOT_STARTED |
| B-03 | SQLx pooling/transactions/checked SQL; constrained dynamic builder | P1/P2 | `forge-database` | tests | NOT_STARTED |
| B-04 | tracing + OpenTelemetry spans linked browser→backend→DB→job | P1 basic, P6 full | middleware | trace test | NOT_STARTED |
| B-05 | Layering: models/actions/policies/queries/events/admin/tests per app; guard rejects forbidden imports | P0 (Cargo-graph level), P5 (symbol level) | `forge check` architecture, ADR-0005 | guard test with injected forbidden dependency | IN_PROGRESS (P0 Cargo-graph level VERIFIED: `crates/forge-tooling/tests/architecture.rs`, `docs/evidence/phase0/exit.txt`; P5 symbol level pending) |
| B-06 | Typed capability injection (`ActionContext`), no global locator | P1 | `forge-core` | compile-time test | NOT_STARTED |
| B-07 | Errors/validation/config/logging/lifecycle/health | P1 basic, P6 full | `forge-core`, `forge-http` | tests | NOT_STARTED |
| B-08 | CI on every push and PR: `forge check`, nextest against PostgreSQL, coverage gate on nightly, mutation testing (informational) | P0 | `.github/workflows/ci.yml` | green GitHub Actions run on the final Phase 0 commit | VERIFIED (all 4 jobs green on 157c713, the last code commit of Phase 0: https://github.com/johniak/forge/actions/runs/40984870697; mutants in the pushed diff: 65 tested, 58 caught, 7 unviable, 0 missed) |
| B-09 | Open-source baseline: README, CONTRIBUTING, SECURITY, CHANGELOG, MIT OR Apache-2.0 | P0 | repository root | relative-link check; files present | VERIFIED (commit c26c07d; link check: `docs/evidence/phase0/linkcheck.sh`, result in `docs/evidence/phase0/execution-ledger.md`) |

## D. Models, schema, SQL, migrations (SPEC §4)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| M-01 | First-class typed models with full metadata feeding admin/forms/API/migrations/context | P1 | `forge-macros`, `forge-schema` | `forge schema inspect` JSON matches DB | NOT_STARTED |
| M-02 | Business queries via bound SQL; generated CRUD; dynamic admin filters via allowlisted builder; explicit joins; typed transactions | P1/P2 | `forge-database` | SQLi tests, N+1 tracing | NOT_STARTED |
| M-03 | Safe query invariants (values bound, identifiers allowlisted, policy before materialization, minimal roles, RLS option) | P1/P2/P6 | ADR-0003 | fuzz + security tests | NOT_STARTED |
| M-04 | Migrations: versioned per app, ordered, locks, drift detection, destructive-change detection, fresh+upgrade verification, expansion/contraction plans, dry-run, no silent history edits | P1 basic, P6 full | `forge-migrations` | migration tests on empty + prior DB | NOT_STARTED |
| M-05 | Model awareness tools (`forge model inspect`, `forge db schema/explain`, `forge change impact`) | P5 | `forge-tooling` | JSON schema tests | NOT_STARTED |

## E. Batteries and apps (SPEC §5)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| A-01 | Admin: list/search/filter/paginate/sort, detail/edit/create, relations, inline, bulk, audit, custom views, permissions | P2 | `forge-admin` (an app built on public APIs) | admin E2E + authz negative tests | NOT_STARTED |
| A-02 | Auth: users, groups, sessions, cookies, password, OAuth/OIDC + MFA hooks, lifecycle | P1 minimal, P6 full | `forge-auth` | tests | NOT_STARTED |
| A-03 | Authorization: RBAC + ABAC/object/tenant, central, default deny | P1 | policies | action × role × object matrix | NOT_STARTED |
| A-04 | Forms: model-bound typed validation, a11y | P2/P3 | | tests | NOT_STARTED |
| A-05 | Middleware stages, ordering registry, conflict validation, global/app/route | P2 | `forge-http` | order + early-return tests | NOT_STARTED |
| A-06 | Background jobs: durable, retries, DLQ, idempotency, cron, visibility, cancellation | P6 | ADR TBD | tests | NOT_STARTED |
| A-07 | Cache, Files/storage, Events/outbox, Search/list APIs, API (REST/OpenAPI/SSE/WS), management tools, observability | P2–P6 | per battery | tests | NOT_STARTED |
| A-08 | `INSTALLED_APPS`-like manifest; versions/cycles/capability conflicts/migration order validated; no destructive uninstall; admin is itself an app | P2 | `forge-apps`, `forge.toml` | install sample app test | NOT_STARTED |
| A-09 | WASM component plugins (later) | P7 | ADR | adversarial sandbox tests | NOT_STARTED |

## F. Frontend (SPEC §6)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| F-01 | Benchmark spike: React+Compiler, Solid, Svelte 5, Leptos, Qwik (+Dioxus) on 6 identical scenarios with pinned profiles | P3 | `benchmarks/frontend`, ADR-0004 | report | NOT_STARTED |
| F-02 | Typed routing/forms/API/action states; explicit data states; runtime decoding at boundaries | P3 | `shared/contracts` | contract tests | NOT_STARTED |
| F-03 | SSR/streaming/hydration/islands as measured; minimal assets; code splitting; WASM only where it pays | P3 | | metrics | NOT_STARTED |
| F-04 | Custom compiler/DSL only after evidence | P7 | ADR | | NOT_STARTED |

## G. Repository architecture (SPEC §7)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| R-01 | App-based generated repo layout (`apps/<domain>/...`) | P1 template | `templates/web-app` | `forge new` produces it | NOT_STARTED |
| R-02 | Framework monorepo partition, crates extracted as proven, no empty crates | P0+ | `crates/*` | review: each crate has code + tests | IN_PROGRESS (3 crates, each with code + tests: forge-tooling, forge-cli, forge-testing) |
| R-03 | Module manifests (`forge.module.toml`) + guard validating from real code; versioned, reviewed waivers an agent cannot self-grant | P0 (Cargo-level), P5 (symbol-level) | `forge check`, ADR-0005 | guard tests | IN_PROGRESS (P0 Cargo-level guard VERIFIED: `crates/forge-tooling/tests/architecture.rs`; waivers P4/P5) |

## H. Agent-native platform (SPEC §8)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| T-01 | One tooling core; CLI + MCP parity; versioned JSON; read-only defaults; elevated grants for writes | P0 core, P5 MCP | `forge-tooling`, `forge-cli`, `forge-mcp` | parity tests | IN_PROGRESS (core + CLI with versioned JSON; MCP P5) |
| T-02 | Required tools table (§8.2): project.map, module.inspect, symbol.find, context.for_task, change.impact, architecture.check, scaffold.feature, schema.inspect, db.explain, migration.plan, check, explain, test.affected, test.full, browser.inspect, visual.verify, trace, security.audit, perf, preview, release.verify, release.plan | P0 `check`+`architecture.check`; P4 test/visual/release; P5 rest; P6 perf/security | `forge-tooling` modules | each tool has schema + tests | IN_PROGRESS (`check` incl. `architecture.check`; `forge coverage` as the P0 seed of `release.verify`) |
| T-03 | Project Context Engine (graph from real declarations, budgeted, spans, freshness) | P5 | `forge-analysis` | context quality tests | NOT_STARTED |
| T-04 | Change Impact Engine | P5 | `forge-analysis` | tests | NOT_STARTED |
| T-05 | Structured diagnostics with spans, raw preserved | P0 (manifest spans), P5 (compiler/test) | `forge-tooling::report` | tests | IN_PROGRESS (P0: manifest, cargo fmt and clippy diagnostics with spans and raw logs; compiler/test P5) |
| T-06 | Agent adapters (`forge agent init claude/codex`), skills, hooks (not security controls) | P5 | `forge-agent` | generated files validated | NOT_STARTED |
| T-07 | Model neutrality, role routing, cost ceilings | P5 | | | NOT_STARTED |

## I. Testing and release gating (SPEC §9, §13)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| V-01 | Test obligations derive from approved feature contracts | P4 | `features/*.yaml`, `forge-testing` | obligation generation tests | NOT_STARTED |
| V-02 | 100% line + branch coverage, narrow human-approved exclusions, release fails if unmeasured | P0 prototype, P4 gate | ADR-0002 | coverage gate test | IN_PROGRESS (P0 gate VERIFIED: `forge coverage`, `docs/evidence/phase0/coverage-gate.txt`; exclusion ledger + approvals P4) |
| V-03 | Required test classes table (§9.3): unit, integration, E2E, authorization, migrations, property, mutation, fuzz, visual, a11y, security, performance, compatibility | P1–P6 progressively | | each class has a runner + example | IN_PROGRESS (unit + integration with real PG; mutation runner informational in CI, `docs/evidence/phase0/mutants.txt`; rest P1–P6) |
| V-04 | Fast loop vs full release loop | P4 | `forge test affected` / `forge test full` | | NOT_STARTED |
| V-05 | Independent test oracles; no self-approved baselines | P4 | approval workflow | | NOT_STARTED |
| V-06 | Deterministic test infra: isolated PG DBs, factories, fake time/UUID, fakes for HTTP/SMTP/storage, parallel workers | P0 PG DB isolation, P1 factories, P4 rest | `crates/forge-testing` | harness tests | IN_PROGRESS (P0 isolated PG databases VERIFIED: `docs/evidence/phase0/forge-testing.txt`; factories P1, fakes P4) |
| V-07 | `forge verify --full` report bound to commit/lockfile/toolchain; statuses VERIFIED / NOT_VERIFIED / BLOCKED_INFRASTRUCTURE | P4 | `forge-tooling::report` | report schema tests | IN_PROGRESS (status vocabulary exists) |
| V-08 | CI isolation, previews, attested artifacts, migration approval separated from deploy | P4/P6 | `.github/workflows` | | NOT_STARTED |

## J. E2E, screenshots, visual verification (SPEC §10)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| E-01 | Playwright real browsers, DOM/a11y/console/network/trace capture, source-element mapping | P4 | `forge-visual` | | NOT_STARTED |
| E-02 | Screen-state matrix per screen and viewports | P4 | screen declarations | | NOT_STARTED |
| E-03 | Visual verification process (seeded preview, pinned screenshots, baseline diff, deterministic checks, multimodal critique, independent approval) | P4 | `forge visual verify` | injected regression blocks release | NOT_STARTED |
| E-04 | `forge debug ui` contract | P4/P5 | | | NOT_STARTED |

## K. Security (SPEC §11)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| S-01 | Security invariants list (§11.1) | P1 core, P6 full | `forge-security` | attack regression suite | NOT_STARTED |
| S-02 | DB specifics: no raw user strings in public API; privileged escape hatch audited; minimal roles; RLS | P1/P6 | ADR-0003 | | NOT_STARTED |
| S-03 | Plugin trust boundaries honestly documented; WASM claims only after adversarial tests | P2/P7 | | | NOT_STARTED |
| S-04 | ASVS mapping, threat model per subsystem, exception register | P0 threat model skeleton, P6 | `docs/security/threat-model.md` | review per phase; exception register entries approved by the owner | IN_PROGRESS (P0 skeleton: `docs/security/threat-model.md`; ASVS mapping per subsystem from P1) |

## L. Observability, performance (SPEC §12, §14)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| P-01 | Causal tracing UI→HTTP→action→SQL→job; `forge trace` | P6 | | | NOT_STARTED |
| P-02 | Benchmarks vs plain Axum baseline and vs frontend baselines; budgets per profile | P1 baseline harness, P3 frontend, P6 gates | `benchmarks/`, ADR-0007 | regression failures | NOT_STARTED |

## M. Addendum requirements (product owner, 2026-10-29; SPEC §21)

| ID | Requirement | Phase | Expected | Verification | Status |
|---|---|---|---|---|---|
| X-01 | The framework itself must be fast: measured overhead over a plain Axum baseline is a standing budget, not a one-off | P1 harness, every phase thereafter | ADR-0007, `benchmarks/overhead` | benchmark in CI with threshold | NOT_STARTED |
| X-02 | E2E testing is a first-party battery shipped by the framework (in-process app server, browser driver, screenshot capture, settled-state signal) | P0 harness seed, P4 full | `crates/forge-testing`, ADR-0006 | an app gets working E2E with zero custom glue | IN_PROGRESS (P0 seed: `forge-testing` database per test; browser layer P4, ADR-0006) |
| X-03 | E2E fixtures in the style of UNICEF HOPE: factories with overridable defaults, in-process DB seeding visible to the browser, login by minted session, page objects as injected fixtures, per-worker isolation, screenshot on failure | P1 factories/login, P4 page objects/screenshots | ADR-0006 | fixture-driven E2E example passes | NOT_STARTED |
