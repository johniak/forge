# Forge — engineering design and whole-project plan

**Date:** 2026-10-29  
**Status:** Approved by the owner 2026-10-29 ("rób co zaplanowane"); ADR-level items marked "needs ADR approval" remain pending  
**Companion documents:** `SPEC.md` (product requirements, authoritative), `docs/requirements-matrix.md` (traceability), `docs/adrs/` (decisions)

This document turns `SPEC.md` into an engineering design and a phase plan that a
builder can plan from without going back to the owner. Where the owner decided,
it says so. Where I decided, it is marked **(my call)** and the owner can veto.
Nothing in this document relaxes a `SPEC.md` requirement; where I propose a
deviation it is labelled **(deviation, needs ADR approval)**.

---

## 1. Intent

Forge is a full-stack web framework whose primary developer is an LLM coding
agent. It gives agents Django-class productivity (typed models, migrations,
admin, installable apps, auth, middleware, jobs) on a Rust/Axum/PostgreSQL
backend, and it refuses to accept unverified work: every feature is proven by
tests, real-browser screenshots and a release report bound to a commit.

The owner's reasons, in their words and ranked: broken UI that nobody noticed
before merge; agents getting lost in the code and making locally plausible,
globally wrong changes; database drift, migrations and security holes.

## 2. What the owner decided (this session)

| Topic | Owner's decision |
|---|---|
| Audience | Public open source from day one. Others will build on it. |
| First application | A new, deliberately simple demo app (the `orders`/`customers` demonstrator from SPEC §15), not a rewrite of an existing project. |
| Pain ranking | 1) UI broken unnoticed, 2) agent lost in code, 3) DB/migrations/security. |
| Frontend | "Take the whole fastest framework." Not a renderer-only approach, not a custom DSL. Fastest is to be confirmed by measurement, not intuition. |
| Agents | Claude Code **and** Codex as first-class, in parallel. |
| Production target | Single VPS with docker-compose first; architecture must not block cloud/k8s later. |
| Process | Owner approves every phase and every ADR. |
| Anti-goals | None. "It is a framework for LLMs": magic, generated code and non-human-friendly surfaces are acceptable when they serve agents. |
| Phase order | As in SPEC §15. |
| Everything else | "Make more decisions yourself." |

## 3. Goals, non-goals, anti-goals

**Goals** (SPEC §0, unchanged): Rust backend; Django-class batteries; first-class
typed models and migrations without a heavyweight ORM; PostgreSQL-first;
measured-fast frontend; single-source typed contracts; 100% measurable
line+branch coverage plus the full test matrix; mandatory screenshot
verification with independent baseline approval; secure by construction;
CLI+MCP tooling from one core with a project graph and bounded context;
deterministic runtime; modular monolith; native-crate extensibility.

**Owner addendum (SPEC §21):** the framework itself must be fast (overhead
budget vs plain Axum, enforced in CI); E2E testing is a built-in battery;
fixtures in the style of UNICEF HOPE.

**Non-goals for v1** (SPEC §0): microservice orchestration, multiple database
engines, arbitrary runtime code execution, a browser engine, a custom JSX
replacement before benchmarks, AI inference on HTTP requests, auto-deploying
destructive migrations, formal correctness, 100% coverage of third-party
binaries.

**Anti-goals:** none declared by the owner. Consequence **(my call)**: where
"agent ergonomics" and "human readability" conflict, agent ergonomics wins, but
every generated artifact must remain inspectable and diffable so that the
verification tooling (and humans, when they care) can audit it.

## 4. Constraints

- Rust stable pinned (`rust-toolchain.toml`, currently 1.95.0); a pinned
  nightly only for branch coverage (ADR-0002, proven 2026-10-29: stable rejects
  `-Z coverage-options=branch`).
- PostgreSQL 17 (`postgres:17-alpine` in dev/test/CI) **(my call)**.
- Node.js is always a **development** dependency (frontend build, Playwright).
  Whether a production container runs a JS runtime is decided by the Phase 3
  selection ADR from measurements (see D5), never assumed.
- Public OSS: license `MIT OR Apache-2.0` **(my call)**; semantic versioning with
  `0.x` until Phase 6 exit; `CHANGELOG.md` kept from the first release; all
  code, docs and ADRs in English; conversation with the owner in Polish.
- Every public tool schema is versioned (`schema_version`) and backwards
  compatible within a major version.
- No test may be skipped silently; absent infrastructure is reported as
  `NOT_VERIFIED`/`BLOCKED_INFRASTRUCTURE`.

## 5. Architecture overview

### 5.1 Runtime shape (what a Forge application is)

A Forge application is one Cargo workspace: a single `server` binary (Axum) and
a `worker` binary sharing the same crates, one PostgreSQL, optional Redis, local
or S3-compatible object storage. Apps are crates under `apps/<domain>/`; each
app owns its models, actions, policies, queries, events, jobs, admin config,
UI, migrations, fixtures, feature contracts and tests (SPEC §7.1). Cross-app
contracts live in `shared/contracts`.

```
browser (chosen JS framework, typed client)  ──HTTP──▶  server binary (Axum)
                                                         ├─ forge-http: router registry, middleware stages, error mapping, tracing
                                                         ├─ forge-auth: sessions, users, policies (default deny)
                                                         ├─ apps/*: actions ── forge-database (SQLx, constrained builder) ── PostgreSQL
                                                         ├─ forge-admin (SSR HTML app, same public APIs as any app)
                                                         └─ forge-jobs: PG-backed queue  ◀── worker binary
```

### 5.2 Framework monorepo (crates and when they appear)

Crates are extracted when they have real code and at least one consumer; no
empty crates (CLAUDE.md rule 9).

| Crate | Responsibility | Appears |
|---|---|---|
| `forge-tooling` | tooling core: `check` (exists), later `context`, `impact`, `verify`, `visual`, `release` | P0 (exists) |
| `forge-cli` | `forge` binary, thin adapter | P0 (exists) |
| `forge-testing` | PG-per-test harness (template DBs), factories, in-process `TestApp`, browser layer | P0 seed, P1, P4 |
| `forge-core` | `ActionContext` and capability traits (`Db`, `Clock`, `CurrentUser`, `EventPublisher`, `Storage`, `Cache`, `HttpClient`), `DomainError`, typed config, validation | P1 |
| `forge-schema` | schema IR (`ModelSchema`, fields, relations, constraints, indexes, access annotations), registry, JSON dump | P1 |
| `forge-macros` | `#[derive(ForgeModel)]`, `#[derive(ForgeEnum)]`, later `ForgeAdmin` | P1 |
| `forge-database` | pool, transactions, generated CRUD, safe-SQL policy, constrained dynamic query builder (P2) | P1 |
| `forge-migrations` | per-app versioned migrations, lineage, drift detection against IR, destructive-change detection; expansion/contraction planner (P6) | P1 |
| `forge-http` | Axum integration, route registry, middleware stage registry (P2), request IDs, OpenAPI export | P1 |
| `forge-auth` | users, groups/roles, password auth (argon2), PG-backed sessions, CSRF, policy evaluation; OIDC/MFA (P6) | P1 |
| `forge-apps` | app manifest, compile-time registry, dependency/version/capability validation | P2 |
| `forge-admin` | generated admin (SSR HTML) built only on public APIs | P2 |
| `forge-ui` | typed contract/client generation for the chosen frontend, screen registry, settled signal runtime | P3 |
| `forge-visual` | screenshot capture, baseline diff, deterministic checks, reviewer workflow, approvals | P4 |
| `forge-analysis` | project graph, symbol index, bounded context, change impact | P5 |
| `forge-mcp` | MCP stdio server over `forge-tooling` (candidate: `rmcp`, the official Rust SDK; verify at P5) | P5 |
| `forge-agent` | `forge agent init claude|codex`: CLAUDE.md/AGENTS.md, skills, hooks, `.mcp.json` | P5 |
| `forge-jobs`, `forge-cache`, `forge-storage`, `forge-events`, `forge-security` | production batteries | P6 |

Plus `examples/orders-demo` (the demonstrator app, grown from P1 onward),
`templates/web-app` (what `forge new` produces, P1), `benchmarks/overhead`
(P1) and `benchmarks/frontend` (P3).

## 6. Design decisions

Each decision maps to an ADR. "Accepted" means the owner approved the ADR;
until then the decision is a proposal the plan may rely on only in the phase
that validates it.

### D1 Schema authority — Rust declarations are the source of truth (ADR-0001, my call)

`#[derive(ForgeModel)]` on a plain struct emits a `ModelSchema` value next to
it. Each app lists its models in an explicit registry function (no link-time
collection magic in v1). The IR is produced by compiling the app and running
`forge schema dump` (a generated binary entry point), serialized as versioned
JSON. Migrations are generated from IR diffs (`forge migration plan`); the live
database is diffed against the IR (`forge db diff`). The macro's attribute
syntax (`#[forge(table = ...)]`, `#[primary_key]`, `#[index]`, relation
fields) is settled in the Phase 1 slice, not here. Fallback trigger recorded
in ADR-0001.

### D2 Persistence and safe SQL — three tiers (ADR-0003, my call)

1. **Static SQL** for business queries: `sqlx::query!`/`query_as!` checked
   against a committed `.sqlx/` offline snapshot (`SQLX_OFFLINE=true` in CI);
   `forge check` fails when the snapshot is stale. A fresh checkout builds
   without a database.
2. **Generated CRUD** from the IR: insert/get/update/delete/list with typed
   bindings, policy applied before materialization.
3. **Constrained dynamic builder** for admin/list filtering: column and
   operator allowlists derived from the IR, all values bound, sort directions
   typed. It is the only place outside migrations that assembles SQL text.

Evidence that this is enforceable: SQLx 0.9 implements `SqlSafeStr` only for
`&'static str`; any dynamic string must be wrapped in `AssertSqlSafe`
explicitly. Forge forbids `sqlx::AssertSqlSafe` and `sqlx::raw_sql` in app
crates via a lint configuration shipped by `forge new` (`clippy.toml`
disallowed types/methods) and the architecture guard; the privileged escape
hatch lives in `forge-database` behind a capability that requires an entry in
the security exception register.

### D3 App registration and dependency injection — explicit and compile-time (ADR for P2, my call)

An app is a crate exposing one `ForgeApp` descriptor (models, migrations,
routes, middleware, admin contributions, jobs, events, fixtures, test
obligations). `forge.toml` lists installed apps; `forge` generates the
registration module (`.forge/generated/apps.rs`, committed, marked generated).
Dependencies are injected as explicit capabilities on `ActionContext`; actions
are plain `async fn(ctx, input) -> Result<Output, DomainError>`. No service
locator, no reflection.

### D4 Admin rendering — server-rendered HTML in Phase 2 (my call; SPEC order kept)

SPEC orders admin (P2) before frontend selection (P3). To keep that order
without pre-empting the frontend decision, the generated admin is
server-rendered HTML from Rust (type-safe templates; candidate `maud`) with
minimal progressive-enhancement JS: lists, filters, pagination, forms and bulk
actions work without JavaScript. Rich widgets and app-defined admin views can
later use the chosen frontend (P6 "more comprehensive admin customization").
This keeps the admin fast, screenshot-testable in P4 and independent of the
JS build.

### D5 Frontend — the fastest full framework, chosen by a reduced benchmark (ADR-0004; candidate cut is a **deviation, needs ADR approval**)

Owner's path, for the record: they first said they wanted only a rendering
engine under a Forge-owned UI model, then settled on "take the whole fastest
framework". So Forge adopts a complete framework (its router, data layer and
meta-framework conventions included where they exist), and `forge-ui`
generates typed contracts, clients and screens *into* that framework rather
than wrapping a bare renderer.

The Phase 3 benchmark keeps all six SPEC scenarios and metrics but reduces
the built candidates from six to three: SolidJS (prior: fastest mainstream DOM
framework in public keyed benchmarks), Svelte 5, Leptos (the Rust/WASM
option). React+Compiler and Qwik are assessed on public data only and dropped
with a recorded reason. **This narrows SPEC §6.2/§15; ADR-0004 and matrix row
F-01 keep the six-candidate wording until the owner approves the cut.** The
ADR must contain the public numbers consulted. Expected but not assumed
outcome: Solid.

SSR and Node in production are **not** pre-decided. Each candidate is
benchmarked in the configuration its ecosystem considers production-grade
(SolidStart, SvelteKit, Leptos SSR), scenario 5 (SSR cold start on slow
mobile) measures the real thing, and the selection ADR records the
operational cost of any JS runtime in production next to the numbers (SPEC
§6.3). My only standing preference: if two configurations are within noise,
the one without a production Node process wins.

### D6 Verification engine (ADR-0002 accepted mechanism; ADR-0006 proposed)

- **Coverage:** `cargo-llvm-cov` on pinned nightly with `--branch`; 100% lines
  and branches for `crates/*` and for app code in `examples/*`; exclusions only
  via a reviewed ledger; `forge verify` fails on unmeasured code.
- **Runner:** `cargo-nextest`; mutation runs with `cargo-mutants` on changed
  crates per PR; property tests with `proptest`; fuzzing (`cargo-fuzz`) for the
  query builder and parsers.
- **Database harness:** shared PG instance (testcontainers locally,
  `FORGE_TEST_PG_URL` in CI), template database per schema hash, one database
  per test created from the template and dropped afterwards; parallel-safe.
- **In-process app:** `TestApp` boots the Axum router on an ephemeral port in
  the test's runtime with the test pool (HOPE's `live_server`).
- **Factories:** generated from the IR per model, overridable defaults,
  scoped defaults from a `Scope` fixture (fixes the HOPE PR #6502 class of
  bug); `rstest` fixtures for composition, `#[once]` for process-wide values.
- **Login by minted session** (HOPE PR #6436): `TestApp::login_as(user)`
  returns the cookie; the login form is tested once.
- **E2E browser layer (my call, deviation from the literal SPEC wording, needs ADR-0006 approval):**
  tests are written in TypeScript with the Playwright Test runner for the
  release gate (Chromium/Firefox/WebKit, pinned), using **generated** page
  objects and **generated** typed fixture clients that call a test-only control
  channel on the in-process app (`create via factory`, `login as`, `settle`).
  The factories therefore stay Rust and IR-derived; the browser test gets the
  HOPE ergonomics over a channel instead of in-process. A Rust CDP driver for
  the fast loop is evaluated in the same ADR; if it proves adequate the plan
  may add it, never replace the cross-browser gate.
- **Settled signal:** the frontend runtime exposes a deterministic "settled"
  promise (no in-flight requests, no pending route transition, no scheduled
  render) that every E2E step waits on. No sleeps, no `readyState`.
- **Visual pipeline:** pinned browser/fonts/viewport/time/network fixtures;
  screenshot per declared screen state; pixel diff with masks; deterministic
  overflow/contrast/console checks; multimodal review via the agent adapter
  with uncertainty marking; baseline approval only by a human or a separately
  authorized reviewer identity, never by the authoring agent.
- **Release report:** `forge verify --full --format json` bound to commit,
  lockfiles, toolchains, browser image, DB seed; statuses
  `VERIFIED`/`NOT_VERIFIED`/`BLOCKED_INFRASTRUCTURE`.

### D7 Tooling core, CLI and MCP (SPEC §8; my call on mechanics)

One Rust core (`forge-tooling`), every tool returns `schema_version`-ed JSON
with diagnostics carrying spans and a `raw_log` reference. CLI exit codes:
0 PASS, 1 FAIL, 3 NOT_VERIFIED, 2 usage. MCP server in Rust over stdio
(candidate `rmcp`); tool names follow SPEC §8.2 (`forge.check`,
`forge.context.for_task`, …) and are frozen by an ADR before Phase 5 exit.
Read-only tools run with default grants; write/DB/baseline/deploy tools
require an explicit grant token in the request. Parity test: the same request
through CLI and MCP yields byte-identical JSON.

Agent adapters: `forge agent init claude` and `forge agent init codex` both
produced from one template set; skills and hooks are ergonomics, never
security controls.

### D8 Project context and impact (ADR for P5, my call on approach)

Graph built from authoritative sources: `cargo metadata`, the schema IR, route
and action registries, SQLx query references (`.sqlx/` snapshot), the screen
registry, feature contracts and test obligations; symbols and spans from a
`syn`-based pass over app crates (rust-analyzer integration is a later
upgrade, not a dependency). Context is budgeted, ranked, cited with spans and
hashes, and progressively disclosed (map → contracts → spans → files).

### D9 Batteries for VPS-first (my call)

Jobs: PostgreSQL queue (`FOR UPDATE SKIP LOCKED`, retries/backoff, DLQ,
cron, cancellation) as the default; Redis adapter optional later. Cache:
in-process (`moka`) with a Redis adapter. Storage: namespaced local filesystem
plus S3-compatible via `object_store`. Email: SMTP with a mailpit fixture in
tests. Observability: `tracing` with OTLP export. Deploy: `forge release plan`
emits a docker-compose bundle (server, worker, postgres, optional redis, caddy)
with health checks and a migration step gated by approval; the same images run
on k8s later.

### D10 Public open source from day one (my call on mechanics)

GitHub repository, `MIT OR Apache-2.0`, Conventional Commits, CI on every PR,
`CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md` with a disclosure policy,
docs site generated from `docs/` (mdBook) from Phase 2. Public API stability
is "best effort with a migration note" until `1.0`.

### D11 Process (owner's decision + my call on mechanics)

- Each phase gets a written implementation plan (`docs/superpowers/plans/`)
  that the owner approves before work starts.
- Each ADR is approved by the owner before code depends on it; experiments
  that an ADR names may run before approval because they produce evidence.
- Every change reports in the CLAUDE.md format (Implemented / Verified with
  commands / Evidence / Not verified / Next step).
- Commits are made only when the owner asks; the first commit will be the
  approved Phase 0 state.
- Work happens in this repository on `main` until the first release, then on
  branches with PRs. "Claude Code and Codex in parallel" means both adapters
  are first-class; it does not mean two agents implement Forge concurrently.
  If the owner ever runs two agents at once, each works in its own git
  worktree and branch (my call).

### D12 Existing Phase 0 spike (my call)

The workspace, `forge-tooling` (`forge check`: manifests, Cargo-graph
architecture guard, fmt/clippy with structured diagnostics; 58 tests) and
`forge-cli` created on 2026-10-29 are kept as Phase 0 output, but they are
re-reviewed under the Phase 0 plan: coverage measured, pedantic lints fixed,
documented. They are not evidence of Phase 0 completion.

### D13 Coverage scope for generated code, macros and the frontend (my call; amends ADR-0002, needs ADR approval)

- **Generated code is thin.** Every derive and generator emits delegates into
  library code (`forge-schema`, `forge-database`, `forge-testing`) that the
  framework covers 100/100 in its own CI. An uncovered generated region in an
  app therefore means a real untested app path, not framework noise.
- **Proc-macro crate split.** `forge-macros` is a shim that only forwards
  `TokenStream`s; the real work lives in `forge-macros-impl`, an ordinary
  library tested with `syn` inputs and snapshot tests, covered 100/100. The
  shim is the one pre-proposed ledger exclusion (reason: proc-macro crates run
  inside rustc and cannot be instrumented), to be approved by the owner with
  ADR-0002's amendment.
- **Attribution experiment (Phase 1, first week).** Measure how
  `cargo-llvm-cov` attributes derive-expanded regions (expansion site vs
  elided). The Phase 1 exit criterion reads "100/100 on all new crates and on
  the demo app's own code, with derive expansions either covered by app tests
  or excluded by an owner-approved ledger entry backed by this experiment".
- **Frontend.** App-owned TypeScript is gated 100/100 with the selected
  framework's standard instrumentation (V8/istanbul via Vitest or Playwright
  coverage) once Phase 3 selects it; constructs the instrumentation cannot
  measure are reported by name in `forge verify`, never counted as covered
  (SPEC §9.2).

### D14 Independent approval and grants — signed records, keys the agent does not hold (my call)

Baseline approvals, coverage-exclusion ledger entries, architecture waivers
and grants for write/DB/baseline/deploy tools are all the same primitive: a
small file under `.forge/approvals/<kind>/<id>.toml` containing the subject
hash, reason, approver and expiry, **signed** with a key listed in
`forge.toml` `[approvers]`. Tooling verifies the signature; the signing key
lives in the owner's agent-less environment (their SSH/GPG signing key), so an
agent cannot mint approvals. A second agent counts as an "explicitly
independent authorized reviewer" (SPEC §10.3) only if the owner lists a
separate key for it; a solo owner approves with one command (`forge approve
<kind> <id>`) that prompts for their key. Session grants for privileged tools
are short-lived signed tokens minted the same way. GitHub-side protections
(CODEOWNERS, branch protection) are added when the repository goes public, as
a second layer, not the primitive. Phase 4's exit test "a baseline change by
the authoring agent is rejected" is a signature check.

### D15 Multi-tenancy is a first-class optional concept from Phase 1 (my call)

The schema IR carries a `scope` annotation on models (e.g. tenant); when a
model declares one, generated CRUD, factories, admin filters and policies
apply the scope from `ActionContext` automatically, and PostgreSQL Row-Level
Security can be enabled per scoped table in Phase 6. The `orders` demo stays
single-tenant in Phase 1; the framework tests the machinery on a tiny
two-tenant fixture model so that the Phase 6 SaaS demo switches tenancy on
without retrofitting `forge-auth`, policies or CRUD paths.

### D16 Name, namespace and distribution (my call, easy to reverse before the first public commit)

`forge`, `forge-cli`, `forge-core`, `forge-macros` and `rustforge` are taken
on crates.io (checked 2026-10-29); `forgeweb*`, `forgekit`, `forge-web` are
free. The binary and MCP namespace stay `forge`/`forge.*` as the owner named
the project (Foundry's `forge` collides; the docs will mention aliasing).
Published crates use the prefix `forgeweb-` (`forgeweb-core`,
`forgeweb-tooling`, …); the workspace directories keep the short names.
crates.io publication starts at `0.1.0` (Phase 6 exit); until then generated
apps and third-party apps depend on the framework through git tags, and the
app registry resolves apps by Cargo (path/git/registry) exactly like any
dependency. The GitHub repository does not exist yet; the owner creates it (or
asks me to) at the first commit, and Phase 0's "CI green on the first commit"
is evaluated on that push.

## 7. Phase plan with exit criteria

Phase order is the owner's (SPEC §15). Each phase ends with a demonstrator and
objective gates; "done" means the exit criteria reproduced by command, not
described. First visible milestone **(my call)**: the Phase 2 admin generated
from declared models. The milestone that proves the thesis: the Phase 5
demonstrator, an agent adding order cancellation with a `VERIFIED` report.
Detailed implementation plans are written per phase (D11); this document is
the whole-project design, not seven plans.

### Phase 0 — reproducible baseline (partly done)

Deliver: workspace + toolchain pin (done), `forge check` (done; lints to fix),
branch-coverage prototype (done: nightly required), `forge-testing` PG harness
(template DB, per-test DB), CI (fmt, `forge check`, nextest with PG service,
coverage on nightly with 100/100 gates, mutation smoke), threat model
skeleton, README/CONTRIBUTING/SECURITY/LICENSE, ADR-0003, matrix updated.  
Exit: fresh clone → `cargo nextest run` and `forge check` pass with PG
provided by Docker or env; coverage report shows lines and branches for every
crate; CI green on the first commit.

### Phase 1 — first real backend feature

Deliver: `forge-core`, `forge-schema`, `forge-macros`, `forge-database`,
`forge-migrations`, `forge-http`, `forge-auth` (minimal), `templates/web-app`,
`forge new`, `examples/orders-demo` with app `customers` (model, migration,
constraint, static SQLx query, typed action, policy, Axum route), `.sqlx`
offline workflow, factories and `TestApp`, overhead benchmark harness with
first measured numbers replacing ADR-0007's placeholders, ADR-0001 proven.  
Exit (SPEC): create/read/update with safe parameters; unauthorized requests
fail closed; migration applied on empty and upgraded DB; integration tests
pass; a hostile SQLi input demonstrably cannot change SQL syntax through the
safe APIs (test with the exact payload in the report); coverage 100/100 on all
new crates and on the demo app's own code per D13; the derive-attribution
experiment is recorded in ADR-0002's amendment.

### Phase 2 — admin and installable apps

Deliver: `forge-apps` (manifest, registry generation, version/cycle/capability
validation, migration ordering), `forge-admin` (SSR HTML list/detail/create/
edit, search/filter/sort/pagination via the constrained builder, bulk actions,
audit log, permissions), middleware stage registry, DI mocks, a sample
third-party app (`community.audit_log`-like) installed through `forge.toml`.  
Exit (SPEC): install the sample app and see its models, migrations, admin
screens and routes without editing unrelated app source; ordering/cycle/
capability-conflict tests; authorization negative matrix for the admin;
screenshot baselines for admin screens captured (approval workflow arrives in
P4, so these are "candidate" baselines).

### Phase 3 — frontend selection and typed contracts

Deliver: `benchmarks/frontend` with the three candidates on all six scenarios,
pinned profiles, repeated runs, raw JSON; selection ADR; `forge-ui`: typed
client and state types generated from the IR and route/action registries,
explicit data states (`Loading | Empty | Ready | Failure`), screen registry,
settled signal; `orders` app screens (list, detail, create) in the demo.  
Exit (SPEC): reproducible benchmark report; working typed end-to-end app; a
contract change breaks the dependent client build; no blanket superiority
claims.

### Phase 4 — verification engine and visual E2E

Deliver: feature contracts (`features/*.yaml`) and derived obligations,
`forge test affected`/`forge test full`, Playwright-based E2E with generated
page objects and fixture clients, `forge-visual` pipeline with baseline
approval workflow, `forge verify --full` report, authorization/security/
migration gates wired into CI, ADR-0006 experiments concluded.  
Exit (SPEC): injected regressions of each kind (code, authorization, visual,
migration) block release with actionable evidence; demo app coverage at
threshold; a baseline change by the authoring agent is rejected.

### Phase 5 — agent tooling and repository intelligence

Deliver: `forge-analysis` (graph, `project map`, `module inspect`, `symbol
find`, `context for_task`, `change impact`), structured compiler/test
diagnostics (`forge explain`), `forge-mcp`, CLI/MCP parity tests, `forge agent
init claude|codex`, agent sandbox and fixtures, tool-name freeze ADR.  
Exit (SPEC demonstrator): an agent, given only the task "add order
cancellation" and Forge tools, delivers contract, code, migration if needed,
tests, E2E, screenshots and a `VERIFIED` report; compared against a
terminal-only run on cost, time and errors, with numbers.

### Phase 6 — production batteries and hardening

Deliver: `forge-jobs`, `forge-cache`, `forge-storage`, `forge-events`
(outbox), realtime (SSE/WebSocket typed), OIDC/MFA, telemetry, migration
expansion/contraction planner and dry-run, security threat tests mapped to
ASVS, performance budgets enforced, admin customization with the chosen
frontend, docker-compose release bundle, runbooks, `0.1.0` release.  
Exit (SPEC): a representative SaaS demo and a public site pass the release
gates; deployable to a VPS by the documented procedure.

### Phase 7 — optional research

WASM component plugins and a custom UI compiler only with evidence from real
users; not planned in detail.

## 8. Left to the builder

- Exact derive attribute syntax, generated code layout and naming (P1).
- Admin template engine choice and design tokens (P2).
- Benchmark harness implementation details within the ADR-0004 protocol (P3).
- Internal module layout inside each crate; test organization beyond "colocated
  with owner".
- Which Rust crates implement each battery, subject to SPEC §18 (mature,
  maintained, verified APIs) and ADR notes.
- Whether `.forge/generated` is committed per artifact type (default: yes, marked generated).

## 9. Risks and how the plan treats them

| Risk | Treatment |
|---|---|
| Proc-macro/IR approach makes tooling depend on a compile (latency) | ADR-0001 fallback trigger; measure in P1 |
| Branch coverage needs nightly; nightly breaks | pinned date; coverage job isolated; `NOT_VERIFIED` instead of green |
| Playwright-over-channel loses HOPE's in-process feel | ADR-0006 experiment compares ergonomics with a Rust CDP driver before P4 exit |
| Three-candidate benchmark misses a faster framework | ADR records public data for dropped candidates; the owner can reopen |
| Rust compile times hurt agent loops | affected-test selection, nextest, crate split by change frequency; overhead and loop latency measured from P1 |
| Public OSS exposes unstable APIs | `0.x`, changelog, migration notes; no promise before 1.0 |
| Owner approval per phase/ADR slows delivery | plans are small and phases have mid-phase checkpoints with evidence |
| Two UI stacks (SSR HTML admin in P2, JS framework from P3) | both are generated from the same schema registry and screen registry, so single-source contracts hold; admin screens enter the same visual pipeline; rich admin widgets only after P3; the P3 selection ADR decides whether the admin migrates |
| Derive-expanded code may be attributed to app files and sink the 100/100 gate | D13 experiment in the first week of P1; thin generated delegates; owner-approved ledger entry only with evidence |
| Approval primitive depends on key management by a solo owner | one-command signing; keys never in agent sessions; GitHub protections as a second layer |
| Crate names and binary collide with existing projects | D16 prefix decided before the first public commit; reversible until then |
