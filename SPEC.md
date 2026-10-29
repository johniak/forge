# FORGE — LLM-first full-stack web framework

**Product and engineering specification — 2026-10-29**  
**Status:** design baseline / implementation handoff  
**Audience:** Claude Code, Codex, Kimi, human maintainers  
**Language:** English for implementation clarity  
**Version:** 0.1 (initial specification, not an implemented product)

> **Mission:** Build a production-grade, extremely fast, strongly typed, secure-by-default full-stack web framework whose *primary developer is an LLM coding agent*, while preserving the batteries-included productivity, admin panel, installed apps, middleware, migrations and ecosystem extension patterns of Django. The framework must actively help agents understand code and MUST enforce evidence-based verification of their output.

This document is the **single source of product requirements**. API examples are intentionally *illustrative pseudocode* unless explicitly marked as executable. Do not claim speculative APIs exist. Implement iteratively, prove each feature with working tests, and record technical decisions as ADRs.

---

## 0. Decision register (read first)

### Agreed requirements — non-negotiable

1. **Rust backend** for performance, correctness and strong compiler feedback. Tokio + Axum + Tower are the proposed runtime foundation.
2. **Full-stack framework**: backend, frontend integration, contracts, UI tooling, app tooling, testing and deployment verification in one coherent product.
3. **Django-like productivity**: first-class typed models, relationships, migrations, generated/extensible admin panel, auth, permissions, installable third-party apps, middleware, forms, jobs, caching, storage, events and management CLI.
4. **No mandatory heavyweight ORM query DSL.** Keep a meaningful first-class model and schema system, safe SQL/typed query access, generated CRUD and a restricted query builder for dynamic admin queries.
5. **PostgreSQL-first** in v1; use its real semantics, constraints, indexes and transaction support. No artificial cross-database abstraction in v1.
6. **Very fast frontend**, with excellent browser runtime performance, size, startup and responsiveness; a Rust backend does NOT imply every DOM update must be Rust/WASM.
7. **Frontend choice is OPEN**: React + Compiler, Solid, Svelte, Leptos, Qwik and possibly custom compiler must be investigated empirically. Do not lock React in or categorically reject it. Rust/WASM is appropriate for computation-heavy client code when measurements justify it.
8. **Strict typing and single-source contracts** across UI, API, models, validation, routing and error states.
9. **Mandatory verification**: target **100% measurable application code line coverage and branch coverage**, plus unit, integration, E2E, property-based, security, migration, browser, accessibility and visual tests. An independent release gate must enforce requirements. Coverage alone is not proof of correctness.
10. **Screenshot analysis is mandatory** for UI-changing work: real-browser screenshots, visual diff against approved baselines, responsive states, and multimodal agent review. A coding agent must not silently bless its own baseline changes.
11. **Secure by construction**: no untrusted interpolation into SQL syntax via supported data APIs; prepared/bound parameters, allowlisted identifiers and operators, safe HTML, CSRF, XSS, object authorization, secrets and plugin containment. A literal mathematical guarantee of zero vulnerabilities is impossible; design to make insecure paths unavailable by default and verify diligently.
12. **LLM-native tooling, repo architecture and workflows**: CLI and MCP from one core, project graph, minimal task context, impact analysis, structured diagnostics, browser inspector, tests, release evidence, Claude Code integration, model-agnostic integration.
13. **Deterministic production runtime**: LLM is a developer, not a dependency of request processing. Generated applications must function without model API calls. Optional AI product features may be added explicitly by app authors.
14. **Modular monolith as default**; support background workers and optional scale-out without forcing microservices.
15. **Extensibility** by compile-time native Cargo apps; optional capability-limited WebAssembly component plugins in a later phase. First-party and third-party apps must obey verification/security rules.

### Decisions that MUST remain open until evidence exists

- **Frontend architecture:** investigate and benchmark before selection; do not implement a novel UI compiler first merely because it sounds LLM-friendly.
- **Schema authority:** choose an enforceable canonical representation (Rust model declarations or a dedicated typed schema IR), reconcile migrations/database, and avoid divergent hand-maintained sources of truth.
- **Model persistence APIs:** exact derive macro syntax, code generation and SQLx integration to be proven in a vertical slice.
- **App/plugin ABI, DI container, admin customization API and UI format:** design experimentally and document in ADRs.
- **Performance thresholds and mutation-testing policy:** propose sensible defaults and measure; don't invent validated numerical claims.

### Out of scope for first MVP

Microservice orchestration; five database engines; general-purpose arbitrary runtime code execution; a new browser engine; custom JSX replacement without comparative benchmarks; AI inference on ordinary HTTP requests; auto-deploying destructive production migrations; guaranteed formal correctness; 100% coverage of external third-party binaries as if source belonged to the app.

---

## 1. Product principles

### 1.1 Verification-driven development

Every application feature begins with **intended behavior and test obligations**, proceeds to implementation, then collects independently reproducible evidence. The coding agent cannot simply claim tests passed. It must provide artifacts produced by the test runner and linked to the exact Git commit and toolchain versions.

Workflow: **request → clarify/approve business behavior if needed → feature contract → affected architecture/schema/UI → implement → compile → tests → real browser → screenshot/diff + model critique → fix → full verify → reviewed release**.

### 1.2 Remove decisions an LLM need not make

Provide one canonical way to implement common needs: authentication, authorization, validation, forms, database parameter binding, errors, pagination, admin CRUD, background jobs, request tracing, test fixtures, dependencies. Models should write **business logic rather than infrastructure boilerplate**.

### 1.3 Compiler as teammate

Prefer explicit Rust types, exhaustive enums, typed errors, immutable contracts and fast machine-checkable feedback. Return compact, actionable diagnostics to agents, preserving detailed raw diagnostics on demand.

### 1.4 Humans own requirements and key tradeoffs

LLMs excel at feature implementation, refactoring and tool use but can make locally plausible, globally costly architecture choices. Enforce module boundaries, surface impact and rationale, require architecture review for risky changes, and do not let the implementing agent define away missing tests or business requirements.

### 1.5 Performance as a measurable outcome

Optimize total end-user experience and developer cycle time, not simply choice of implementation language:

- Server: throughput, memory, latency p50/p95/p99, DB queries, contention and startup.
- Browser: JS/WASM payload, startup, hydration/resumption costs, INP, LCP, CLS, CPU and memory across devices, large-list behavior.
- Agent development: `generate → compile → test → diagnose → repair` cycle duration, agent tool-call count, tokens, reproducibility and change quality.

### 1.6 Keep open standards and portability

Rust/Cargo ecosystem, PostgreSQL, OpenAPI/JSON Schema where appropriate, MCP, Playwright, OpenTelemetry, Docker/OCI. No required vendor-specific LLM, cloud, IDE or paid hosting.

---

## 2. Macro architecture

```text
Developer / human reviewer / coding agents (Claude Code, Codex, Kimi, etc.)
                 │ CLI or MCP, same schemas and permission model
                 ▼
        Forge Tooling & Context Engine
  project graph • feature contracts • code intelligence • impact
  architecture guard • generation • compiler diagnostics
  unit/integration/E2E/visual/security/migration verification
                 │
                 ▼
            Application workspace
  apps/<domain>/    typed Rust models, actions, policies,
                    migrations, admin, UI, tests, fixtures
                 │
                 ▼
           Forge Runtime + Registry
  Rust/Axum/Tower • model/schema registry • auth/access
  typed operations • SQLx/PG • tasks • extensions
                 │                     │
                 ▼                     ▼
       Frontend renderer         PostgreSQL + workers
  chosen by benchmark,         optional Redis/cache,
  SSR + reactive UI             object storage, telemetry
  + browser test runtime
```

The framework's **canonical graph/IR** links modules, symbols, models/columns/relations, migrations, operations, permissions, routes, frontend screens/components, browser states, jobs, events, tests and their coverage obligations. It is a build artifact derived from authoritative declarations and static analysis, not an opaque AI-maintained index.

---

## 3. Backend runtime and architecture

### 3.1 Proposed platform

- Rust stable toolchain with pinned version and Cargo workspace.
- Tokio async runtime, Axum HTTP, Tower middleware and tower-http integrations.
- PostgreSQL and SQLx for connection pooling, transactions, static compile-checked SQL where supported; an explicitly constrained typed dynamic query builder for admin/filtering (e.g., evaluate SeaQuery).
- `tracing` + OpenTelemetry for request spans and linked browser/backend/database/job telemetry.
- Modular monolith as the default deployable unit; independently scalable workers for asynchronous work when required.
- Zero automatic LLM inference at runtime.

Do not reimplement mature HTTP networking, authentication cryptography, browser automation or SQL drivers unless there is a demonstrated functional gap.

### 3.2 Layering and boundaries

For each domain app:

1. **Models/schema**: typed data, constraints, relations, indexing metadata, migration references.
2. **Actions/use cases**: business operations, transactions, idempotency, typed inputs/outputs/errors.
3. **Policies**: authenticated and object/tenant-level permission checks, fail-closed.
4. **Queries**: bounded and parameterized read models/SQL, no uncontrolled dynamic SQL.
5. **Events/jobs**: domain events, durable async work and retries.
6. **Admin/UI**: generated defaults plus explicit extensions.
7. **Tests/fixtures**: colocated with owner domain.

Modules expose public contracts; forbidden cross-module imports are rejected by Architecture Guard. Cross-domain interactions through approved public actions/events. Shared code may hold truly cross-cutting types, not become a dumping ground.

### 3.3 Type-directed dependency injection

Use explicit, typed capability access, e.g. `ActionContext` with scoped `Db`, `Cache`, `Storage`, `CurrentUser`, `Clock`, `EventPublisher`, `HttpClient`. Dependencies declared by modules and checked at app startup/build. Allow mock/test replacements. Avoid uncontrolled global service locators and reflection magic. Per-request authorization context and transaction scope are explicit; prevent accidentally passing privileged DB handles to untrusted plugins.

### 3.4 Errors, validation and configuration

- Public `Result<Success, DomainError>` contract with predictable HTTP mapping and machine-readable errors.
- One validation contract on input schemas with independent server enforcement; client validation enhances UX but cannot authorize.
- Versioned, strongly typed configuration, environment/secret overlays and schema validation on startup.
- Logging with redaction and correlation IDs. Typed timeouts, retries and cancellation handling.
- Explicit startup/shutdown lifecycle, health/readiness probes.

---

## 4. Models, schema registry, SQL and migrations

### 4.1 First-class models are REQUIRED

Models are not optional even though a heavyweight ORM is. The agent and framework must know fields, types, nullability, enums, relationships, uniqueness, foreign keys, checks, indexes, access annotations, lifecycle and consumers. Schema metadata powers admin, generated forms, REST contracts, migration tooling, context extraction and required test generation.

Illustrative **non-compiling proposal**:

```rust
#[derive(ForgeModel, ForgeAdmin)]
#[forge(table = "orders")]
pub struct Order {
    #[primary_key]
    pub id: Uuid,
    #[index]
    pub customer: ForeignKey<Customer>,
    #[decimal_min("0")]
    pub amount: Decimal,
    pub status: OrderStatus,
    #[auto_now_add]
    pub created_at: DateTime<Utc>,
}

#[derive(ForgeEnum)]
pub enum OrderStatus { Pending, Paid, Cancelled }
```

Choose the source-of-truth mechanism in an ADR: either parse derive macro metadata into a canonical IR or define schema-first types generating Rust. Database schema must be continuously diffed against authoritative declarations. **Never** separately hand-maintain contradictory model, SQL schema, frontend type and admin definitions.

### 4.2 Data access model — not a heavyweight ORM

- **Business queries:** ordinary bound SQL (`sqlx::query!` / `query_as!` and equivalent), typechecked against an approved schema snapshot or live build DB.
- **Generated CRUD:** compile/build-time generation from models and policies, not LLM-generated at request time.
- **Dynamic filters in admin:** typed builder using allowlisted columns, joins, operators and sorting; bind all user-supplied values as parameters.
- **Relations:** known foreign-key graph and efficient explicit joins, **not** implicit lazy-loading; surface N+1 behavior through tracing/perf tools.
- **Transactions:** typed explicit scope for multi-step changes; clear retry strategy and isolation rules.
- **Reporting:** PostgreSQL-native CTEs, window functions, materialized views etc. through checked SQL.

The supported safe data interface must not expose arbitrary string concatenation into SQL syntax. Raw dynamic SQL is an isolated privileged API requiring explicit approval, additional lint rules and security review. Rust code in trusted native apps cannot be sandboxed simply by denying an API name, so define and verify what the framework can realistically enforce.

### 4.3 Safe query invariants

- All **values** parameterized; never compose untrusted input into values inside SQL strings.
- Dynamic **identifiers, order-by directions and operators** resolved via compile-time typed fields or runtime allowlists; cannot be parameterized like values.
- Enforce column/row selection and object policy before data is materialized. Consider PG Row-Level Security for tenant isolation as defense in depth.
- Minimal DB credentials; separate migration/admin runtime roles, read-only diagnostics role.
- Fuzz and security-test query builder and admin filters; test hostile parameters and Unicode/input corner cases.

No honest design can guarantee the entire ecosystem has **zero SQL injection** or other bugs. Goal: make injection impossible via normal supported safe APIs, prohibit insecure escape hatches by policy, and test boundaries.

### 4.4 Migrations (mandatory)

- Versioned per-app migrations, topologically ordered dependency resolution, explicit locks and transaction semantics.
- Detect model→DB drift, generated migration completeness, destructive changes and incompatible changes.
- Verify on fresh DB and prior supported DB with representative seeded data, including concurrent-read scenarios where relevant.
- Support expansion/backfill/contraction plans, lock estimates, online index creation where appropriate, rollback/forward recovery plans, migration dry-runs and progress observability.
- For data migrations, require idempotency strategy, retries, limits, batching, failure recovery and validation queries.
- Never auto-run destructive migrations in production without independent approval and documented plan.
- Agents can generate migrations and data plans but cannot silently edit applied migration history.

### 4.5 Model awareness for agents

`forge model inspect`, `forge db schema`, `forge db explain`, `forge change impact` return machine-readable model relationships, consumers, policy surfaces, migration lineage, indexes and relevant query plans. DB diagnostics default to read-only/sanitized; never casually expose production records/secrets to agent tools.

---

## 5. Django-level batteries and installable apps

### 5.1 First-party batteries

- **Admin:** automatic list/search/filter/pagination/sorting, detail/edit/create, related objects, inline editing where safe, bulk actions, audited history, customized fields/actions/views, permissions.
- **Auth:** user model, groups/roles, sessions, secure cookies, password authentication, social OAuth/OIDC and MFA integration points, account lifecycle.
- **Authorization:** RBAC + contextual ABAC/object/tenant checks, centralized policies and default deny.
- **Forms:** model-bound typed validation, field components, display errors, files, accessibility.
- **Middleware:** order/stages, global/app/route, request+response, timeouts, rate limit, session, CSRF, security headers, traces.
- **Background jobs:** durable queue, retries/backoff, dead-letter, idempotency, schedule/cron, visibility, cancellation and admin monitoring; evaluate Redis + BullMQ-like semantics but select a Rust-compatible implementation based on ADR.
- **Cache:** local/Redis abstractions, typed cache keys, invalidation, TTL, tests.
- **Files:** local/S3-compatible storage, upload validation, object permissions, lifecycle cleanup.
- **Events/signals:** explicit domain events and hooks, transactional outbox when needed; avoid hidden side effects.
- **Search and list APIs:** pagination, filters, sorting, full-text as supported by PostgreSQL.
- **API:** typed operations, REST/OpenAPI/JSON Schema, optional streaming/SSE and explicit WebSocket support for realtime.
- **Management tools:** admin commands, migrations, seed, check, shell/REPL or safe inspect, test, deploy checks.
- **Observability:** structured logs, tracing, metrics, error reporting, query perf, correlation from UI to worker.

### 5.2 Apps mechanism akin to Django `INSTALLED_APPS`

Each app is a versioned Cargo crate + Forge manifest describing name, version, dependencies, capabilities, exports, models/migrations, routes, middleware, admin contributions, jobs, events, resources, UI and test obligations. Install app through a declarative manifest + consistent CLI, not manual cross-repo surgery.

Illustrative manifest:

```toml
[apps]
"forge.auth" = "builtin"
"forge.admin" = "builtin"
"community.audit_log" = "1.4"
"community.notifications" = "2.1"

[apps."community.notifications"]
provider = "smtp"
```

Requirements:

- Validate versions, cycles, capability conflicts, migration ordering and exported interfaces.
- A package installs its own DB schema changes, admin screens, API routes, UI fragments, middleware, tests and fixtures.
- App removal/upgrade requires explicit data and migration lifecycle; **no destructive automatic uninstall**.
- Plugins cannot hide failing tests or lower verification/security gates.
- First-party admin itself is an app built with the same APIs, proving extensibility.
- Compile-time native crates are trusted code in a shared process; permissions in a manifest do **not** sandbox arbitrary Rust behavior.
- Later: runtime **WASM Component Model** plugins (e.g. Wasmtime) with explicit host capability interfaces, memory/time quotas and isolation. A separate design is needed for who can write DB, register routes, run migrations and access secrets.

### 5.3 Middleware

Reuse Tower/Axum semantics; Forge provides a typed ordering/stage registry and conflict validation. Support global, per-app and per-route layers with explicit pre-routing and post-routing distinctions. Test middleware order and early-return behavior. Do not confuse request middleware with app lifecycle hooks or domain events. Expose a plugin-friendly middleware interface with clearly documented sequencing.

### 5.4 Admin design

Generated from schema registry plus `AdminConfig`; must be **extensible, permission-aware and testable**. Administrator privileges are not assumed for all admin users. Test row/object permissions, search/filter security, bulk actions, file upload restrictions and CSRF. Support user-defined admin views and package-provided admin panels, while preserving common design tokens and a11y. Auto admin is feature-complete enough to replace Django admin for common business operations.

---

## 6. Frontend: performance-first, strongly typed, under evaluation

### 6.1 Explicit goals

Frontend must be very fast on desktop **and weak mobile devices**, accessible, deterministic, typed and straightforward for LLMs to generate/verify. Do not presume React is slow or Rust/WASM is fast for DOM. Measure actual update work, payload and interaction latency.

Desired capabilities:

- SSR where valuable, streaming, efficient hydration/islands or resumability where it wins.
- Granular updates/signals or compiler-optimized rerenders; avoid unnecessary DOM work.
- Consistent runtime state with exhaustive discriminated unions / Rust enums.
- Typed routing and links, typed forms/API/action states and error contracts.
- Automatic loading/empty/error/permission/disabled/optimistic states.
- Accessible components, responsive layouts, theme/design tokens, animation and complex interactions.
- Lists/tables virtualization, filtering/sorting, query caching with invalidation.
- Minimal client assets, code-splitting and only load WASM for tasks where it pays.
- Possibility to deliver sophisticated editors, dashboards, IDE-like apps, graphs, drag/drop, canvas; not just CRUD.
- Solid web security defaults (escaping, sanitization for rich content, CSP, secure URL handling).

### 6.2 Candidate comparison — required research spike

Build equivalent apps with:

1. React + React Compiler (strong ecosystem baseline).
2. SolidJS (fine-grained direct DOM updates).
3. Svelte 5 (compiler output and reactive runes).
4. Leptos (Rust SSR + Rust/WASM client + signals).
5. Qwik (resumability/startup behavior).
6. Optional Dioxus (Rust cross-platform) if useful.

Test **the same** scenarios and UX, not cherry-picked benchmarks:

- Administrative table with 10,000 records, virtualization, sorting/filtering/editing.
- Dynamic form with nested validation/relations, optimistic state and errors.
- Realtime dashboard with frequent updates and charts.
- Text editor/IDE-like interaction with autocomplete, long content and keyboard handling.
- SSR public page and slow mobile startup.
- User-facing route transitions and offline/error/loading states.

Measure: initial transferred bytes, JS+WASM split, parse/compile/startup, LCP, INP, CLS, p95 input delay, CPU, browser memory, server SSR throughput, fast/production build times, time-to-feedback for agent, coverage/testability, ecosystem compatibility, security surface, accessibility, complexity of forms/admin. Use pinned hardware/device/browser profiles and repeat tests. Prefer a documented decision matrix and ADR, not intuition.

### 6.3 Initial integration recommendation (not a locked decision)

Rust backend exposes typed generated operation contracts. Select frontend renderer after the spike; **Solid is a serious candidate for fastest normal DOM UI, Leptos for Rust end-to-end, React for ecosystem**, but none is declared winner. Avoid requiring Node.js in production backend if possible; if an SSR frontend option requires JS runtime, evaluate that cost explicitly.

### 6.4 Potential later custom frontend compiler / DSL

Only if benchmark and LLM ergonomics studies show clear value. Proposed direction:

- Declarative view/screen descriptions linked to models/actions/permissions.
- Compiler intermediate representation shared with Forge's context/test tools.
- Native Rust SSR, minimal generated DOM update instructions, possibly tiny JS runtime and lazy Rust/WASM islands.
- Fine-grained state dependencies, no general Virtual DOM unless benchmarks support it.
- Source maps mapping rendered elements to feature and source file, so agents can debug screenshots and DOM without guessing selectors.
- Generated code remains inspectable, testable and standards-based; escape hatches for canvas, rich editors, accessibility and advanced UI.

**Do not start MVP by inventing a new programming language or frontend compiler.** A library-integration prototype is safer, with an eventual DSL as research branch.

### 6.5 Source-of-truth API contracts

A change to model/action type, field nullability, endpoint shape, permission, route or event must invalidate/rebuild dependent clients, screens and tests. Data states should be explicit, e.g. `Loading | Empty | Ready<Data> | Failure<Error>`. On the client, nullability, inputs, outputs and errors are statically checked. Runtime decoding/validation occurs at untrusted boundaries.

---

## 7. Unified repository architecture

### 7.1 Generated application repo

```text
my-forge-app/
├── forge.toml                 # global manifest, apps, policies, verification
├── Cargo.toml                 # Rust workspace
├── Cargo.lock
├── apps/
│   ├── customers/
│   │   ├── Cargo.toml
│   │   ├── forge.module.toml  # boundaries, exports, declared capabilities
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── models.rs
│   │   │   ├── actions.rs
│   │   │   ├── queries.rs
│   │   │   ├── policies.rs
│   │   │   ├── admin.rs
│   │   │   ├── events.rs
│   │   │   └── jobs.rs
│   │   ├── ui/
│   │   │   ├── list/
│   │   │   ├── detail/
│   │   │   └── form/
│   │   ├── migrations/
│   │   ├── fixtures/
│   │   ├── features/          # approved behavior/test contracts
│   │   └── tests/
│   │       ├── unit/
│   │       ├── integration/
│   │       ├── e2e/
│   │       └── visual/
│   ├── orders/
│   └── payments/
├── shared/
│   ├── contracts/
│   └── ui/
├── tests/
│   └── cross_app/
├── .forge/
│   └── generated/             # reproducible, marked generated
├── .claude/
│   ├── skills/
│   ├── agents/
│   └── settings.json
├── .mcp.json
├── AGENTS.md
└── CLAUDE.md
```

Features reside within their owning app; do NOT scatter `models/`, `controllers/`, `components/` and `tests/` globally by technology. Cross-app contracts, shared UI and full integration tests remain in their own explicit locations. Avoid one Cargo crate per trivial function.

### 7.2 Framework's own monorepo

```text
forge/
├── Cargo.toml
├── crates/
│   ├── forge-core/
│   ├── forge-macros/
│   ├── forge-schema/
│   ├── forge-migrations/
│   ├── forge-database/
│   ├── forge-http/
│   ├── forge-auth/
│   ├── forge-admin/
│   ├── forge-apps/
│   ├── forge-ui/
│   ├── forge-testing/
│   ├── forge-visual/
│   ├── forge-analysis/
│   ├── forge-agent/
│   ├── forge-cli/
│   └── forge-security/
├── templates/{web-app,plugin}/
├── examples/{admin-demo,saas,realtime-dashboard}/
├── integration-tests/
├── benchmarks/
├── docs/
│   ├── adrs/
│   ├── architecture/
│   └── guides/
└── AGENTS.md
```

This is a target partition, **not** a requirement to create 15 empty crates on day one. Extract crates as implementation proves their interfaces.

### 7.3 Module manifests and architecture enforcement

Example:

```toml
[module]
name = "orders"
kind = "domain"
description = "Order lifecycle and fulfillment"

[dependencies]
allowed = ["customers", "inventory"]
forbidden = ["payments.internal"]

[exports]
models = ["Order", "OrderItem"]
actions = ["CreateOrder", "CancelOrder"]
events = ["OrderCreated", "OrderCancelled"]

[security]
default_policy = "deny"

[verification]
line_coverage = 100
branch_coverage = 100
e2e_required = true
visual_required = true
```

Guard must validate boundaries from actual code and manifests, not trust manifest declarations alone. Rules for layering and cross-app imports are checked with static analysis/build checks. Any waiver is versioned, narrowly scoped, reviewed and impossible for an agent to grant itself.

---

## 8. Agent-native development platform

### 8.1 Single tooling core, multiple interfaces

One **Forge Tooling Core** in Rust, exposed via CLI for humans/CI and MCP for Claude Code, Codex, Kimi and future agents. Both call the same underlying services and produce the same structured schemas, security checks, results and error identifiers.

- Prefer typed JSON with stable versioned schemas; concise default output and references to expandable raw logs.
- Deterministic idempotent operations when possible.
- Read-only/project inspection tools available with safe default privileges.
- Write operations, DB changes, screenshot baseline approvals and deployments require elevated explicit grants.
- Support workspace sandboxing, path allowlists and rate limits to avoid tool-assisted damage or exfiltration.
- No model-specific prompts should be required for correctness; adapters improve ergonomics only.

### 8.2 Required CLI / MCP tools

| Tool namespace / example | Required behavior |
|---|---|
| `forge.project.map` | apps, modules, routes, exports, ownership, dependencies |
| `forge.module.inspect` | model/actions/policies/UI/tests for selected module |
| `forge.symbol.find` | declarations, usages, source spans, consumers |
| `forge.context.for_task` | ranked, bounded context for goal; explanations/citations |
| `forge.change.impact` | impacted types, screens, SQL, migrations, tests, permissions |
| `forge.architecture.check` | forbidden imports, layering, dependency cycles |
| `forge.scaffold.feature` | feature scaffold and contractual test obligations |
| `forge.schema.inspect` | typed models, relationships, constraints and migrations |
| `forge.db.explain` | safe read-only query plans, sanitize parameters |
| `forge.migration.plan` | draft migration, impact and destructive-change warnings |
| `forge.check` | formatting, type check, contracts, architecture, lint |
| `forge.explain` | short structured compiler/test error explanation |
| `forge.test.affected` | quick dependency-based test selection; never sufficient for release |
| `forge.test.full` | complete verified test suite with evidence artifacts |
| `forge.browser.inspect` | browser DOM/accessibility/network/console/trace/screenshot |
| `forge.visual.verify` | baseline diff + screenshot assessment + viewport checks |
| `forge.trace` | frontend → HTTP → domain action → SQL → job span correlation |
| `forge.security.audit` | policy, static checks, dependency auditing, targeted attack tests |
| `forge.perf` | backend/frontend profiling and regression evidence |
| `forge.preview` | isolated ephemeral app with seeded data |
| `forge.release.verify` | independent signed/attested commit-level release verdict |
| `forge.release.plan` | deploy + rollback planning, no unilateral destructive steps |

The exact command names are part of the proposed public tool contract; stabilize them as an ADR before external integrations.

### 8.3 Project Context Engine

Rather than ask an LLM to grep 400k lines of source, derive the **dependency and ownership graph** at build time from Rust symbols/Cargo, models and SQL query references, routes/actions, UI, tests and feature contracts. Use semantic retrieval only as a supplementary ranking technique.

Example usage:

```bash
forge context --task "Add order cancellation" --budget 6000 --format json
```

Example shape (not executed):

```json
{
  "task": "Add order cancellation",
  "primary_module": "orders",
  "relevant_files": ["apps/orders/src/models.rs", "apps/orders/src/actions.rs", "apps/orders/ui/detail"],
  "dependencies": ["payments", "inventory"],
  "affected_contracts": ["OrderStatus", "OrderCancelled"],
  "required_tests": ["order_cancellation", "order_detail_e2e"],
  "constraints": ["No direct payments.internal access", "Permission orders.cancel required"],
  "source_spans": []
}
```

Properties: budgeted context, rationale, explicit source spans/hashes, freshness and invalidation, follow-up expansion, no silent omission of high-impact dependencies. Context should be progressively disclosed: map → contracts → exact code spans → full files when necessary.

### 8.4 Change Impact Engine

For a changed column, model, event, public method or UI state identify related source, DB queries, API contracts, screens, migrations, test obligations, plugins and consumers. Output a graph and a suggested *minimal* test set; full release suite is still mandatory. Identify uncertainty explicitly, especially reflection/runtime plugin usage.

### 8.5 Structured diagnostics

Instead of dumping long compiler stdout, tools return: severity, error code, file and exact span, relevant model/feature, expected vs actual type, affected contract, suggested remediation and link to raw evidence. Preserve unmodified raw error messages for debugging. Distinguish test failures from infrastructure failures.

### 8.6 Agent adapters

`forge agent init claude` generates up-to-date `CLAUDE.md`, `.claude/skills/*`, `.claude/agents/*`, `.claude/settings.json` hooks, `.mcp.json`. Equivalent `forge agent init codex` etc. Shared tool protocol means agents use the same safety gates.

Suggested skills: implement-feature, design-review, debug-backend, debug-frontend, migration-review, visual-review, security-review, verify-release. Suggested subagents: planner/architect, implementer, independent test reviewer, security reviewer, visual reviewer, performance analyst. Agent teams or worktrees are **optional**; not every small change needs multi-agent orchestration.

Use hooks after edits to trigger fast checks, on task completion to flag missing verification. Hooks are **not** security controls: they may be bypassed. CI and release verification must not trust them.

### 8.7 Multi-agent model neutrality

Different model families may be good at implementation, architecture, math, terminal usage or visual assessment. Select models via measured task success and budget, not hardcoded brand rankings or unverified benchmark claims. Provide optional role routing and independent critique, with cost ceilings, traceability, worktree isolation and collision management.

---

## 9. Mandatory testing and release gating

### 9.1 Principle: test obligations derive from contracts

Adding a model, action, screen, policy, event, migration or job creates machine-readable **verification obligations**. The framework can generate technical negative cases; the expected **business behavior must come from approved specifications**, not reverse-engineered from code written by the same agent.

Illustrative feature contract:

```yaml
feature: orders.cancel
owner: orders
action: Order.cancel
requirements:
  - owner_can_cancel_pending_order
  - cannot_cancel_paid_order
  - cancellation_is_idempotent
  - other_users_are_denied
ui:
  screen: orders.detail
  states: [pending, cancelling, cancelled, error]
verification:
  unit: required
  integration: required
  e2e: required
  visual: required
  security: required
```

### 9.2 Hard coverage target

- **100% line and branch coverage of measurable project-owned application logic**, including backend and frontend where their instrumentation supports it.
- Scope rules: generated framework runtime is tested in framework CI, app-owned generated customizations are tested within app scope, vendored third-party libraries follow dependency assurance rather than impossible blanket app coverage.
- Instrumentation exclusions must be narrow, auditable and **human-approved**; agent cannot lower threshold or exclude its changed code.
- Release fails if instrumentation is unavailable, coverage decreases, or affected code is unmeasured (explicitly report unsupported coverage constructs).
- Coverage = execution, **not correctness**; test oracles, boundary cases, mutation tests, security tests and E2E are mandatory alongside it.

Suggested tooling: `cargo-llvm-cov` (verify branch coverage/toolchain support), `cargo-nextest`, Vitest/coverage if JS UI chosen, Playwright coverage/instrumentation as applicable; `cargo-mutants` for Rust mutation analysis. Pin versions and validate coverage reports are real, not synthetic.

### 9.3 Required test classes

| Class | Required scope / acceptance |
|---|---|
| Unit | all application logic paths; 100% measurable lines/branches |
| Integration | every API action, database operation, relation, transaction, job and plugin contract |
| E2E | every declared critical user flow and screen-state contract in real browser |
| Authorization | action × role × tenant/object ownership × forbidden cases; default-deny |
| Database/migrations | initial and upgrade schemas, data migrations, constraints, rollback/forward recovery as appropriate |
| Property-based | validation boundaries, numeric ranges, enums, serialization, state transitions |
| Mutation | tests must detect representative mutations; define thresholds/waivers via ADR; no meaningless tests |
| Fuzzing | parser/SQL builder/untrusted data and security-critical interfaces |
| Visual regression | real screenshots for configured states/viewports; approved golden baselines |
| Accessibility | semantics, keyboard, contrast/interaction tooling and manual/agent audits where needed |
| Security | static analysis, dependency checks, API attack tests, CSRF/XSS/SQLi/IDOR/SSRF/mass assignment |
| Performance | repeatable benchmark suites; threshold-based regression failures on critical flows |
| Compatibility | plugin API, middleware sequencing, SSR/client contracts, supported browsers |

### 9.4 Fast loop vs full release loop

- **After file edit:** formatter, incremental compiler/type checker, manifest/architecture check, lightweight lint.
- **After logic change:** affected unit+integration tests, selected browser states, SQL/model checks.
- **After UI change:** browser E2E, screenshot suite for affected screens, semantic/a11y checks.
- **Before merge/release:** all tests, 100% coverage gates, policy/security, migration checks, visual review and approved baselines, report provenance.
- **Deep verification:** mutation/fuzz/load as required by schedule, risk and release policy; security-critical changes get mandatory targeted mutation/fuzz runs before release.

Fast tests accelerate development but do NOT substitute for complete required release gates. Results tied to code hash, toolchain and deterministic fixture versions.

### 9.5 Independent test oracles

The implementing agent must not invent the definition of success after implementation. Business contracts are approved before implementation; a separate reviewer/test agent may critique tests. Golden screenshots cannot be updated by the authoring agent alone. Tests must assert observable behavior, not just internal implementation details. Testing cannot guarantee zero defects, but systematic independently evaluated evidence lowers risk.

### 9.6 Deterministic testing infrastructure

Every app can create isolated PostgreSQL test DBs, seed builders/factories, fake time, deterministic UUID/randomness, controllable external HTTP/SMTP/storage, repeatable queues, test login/auth roles, transactional fixture boundaries and cleanup. Real integration DB tests are not replaced by mocks. Support parallel tests, per-worker DB namespaces, reproducible failures, network isolation for untrusted agent runs.

---

## 10. E2E, screenshot and visual design verification

### 10.1 Browser tooling

Use real Playwright-driven browsers (Chromium/Firefox/WebKit). Capture DOM, accessibility tree, console errors, network traces, screenshots and browser traces. Expose source-element mapping from UI build metadata so an agent can identify which owner component/code line produced a visible element.

### 10.2 Required screen-state matrix

Each screen declares testable visual/functional states: success, loading, empty, error, forbidden, validation error, responsive breakpoints, long text, missing images, large data sets and relevant dark/light/theme modes. Test across configured desktop, tablet and mobile profiles; ensure keyboard and touch paths where relevant.

### 10.3 Visual verification process

1. Start isolated seeded preview at exact commit.
2. Navigate real browser to required screens/states and execute key interactions.
3. Take standardized screenshots with pinned browser/OS/fonts/device scale/animations/time/network fixtures.
4. Compare images against **human-approved** baseline; record pixel/diff threshold and masked dynamic areas with justification.
5. Run deterministic checks for overflow, clipped content, hidden controls, contrast/a11y and console/network errors.
6. Send current screenshot(s), baseline/diff and UX requirements to multimodal reviewer for semantic layout critique (alignment, visual hierarchy, legibility, responsive behavior). The reviewer must mark uncertainty.
7. Create actionable findings mapped to elements/source. Agent fixes code and repeats.
8. Any baseline change requires human or explicitly independent authorized review; never automatically mark changed screenshot as correct.

LLM screenshot evaluation is a supplementary signal, **not** a mathematical visual correctness proof. CI must make lack of reviewer or required screenshot an explicit **unverified** result rather than silently pass.

### 10.4 Example debugging contract

`forge debug ui orders.detail --state=error --viewport=mobile --format=json` returns screenshot artifact pointer, DOM/a11y tree, overflow checks, console/network errors, component owner/source span, API trace IDs, baseline difference and agent-review findings. Never fabricate an image assessment when no browser was run.

---

## 11. Security architecture (defense in depth)

### 11.1 Security invariants

- Secure by default, **default deny** for authorization and plugin capabilities.
- All untrusted inputs validated at boundaries; DB values bound, identifiers allowlisted.
- Escape HTML by default, no raw HTML without controlled sanitization/trusted types.
- CSRF for cookie-auth state-changing actions; SameSite/HttpOnly/Secure cookie defaults, reasonable CSP/security headers and robust CORS settings.
- Central object/tenant authorization on every model/API/admin/job access; server-side enforcement even if UI hides controls.
- No privilege escalation through dynamic admin filters, mass assignment, bulk actions, uploads or plugin APIs.
- SSRF-restricted outbound HTTP connector, allowlists/DNS/IP checks, redirects controls, timeouts and sandboxing.
- No shell string interpolation; subprocess API takes argument arrays, controlled executable choices and cwd.
- Storage paths must be bounded to intended namespace with safe object IDs, not direct user-controlled filesystem paths.
- Secret management/rotation, log redaction, least-privileged service credentials, dependency integrity, SBOM and supply-chain controls.
- Request rate limiting, resource limits, structured audit logs, incident diagnostics, permissions test matrix.

### 11.2 Database specifics

Static typed SQLx queries and restricted dynamic query-builder in normal app code. No ad hoc `execute(raw_user_string)` in public safe API. Escape hatch only under separately audited privileged capability and review. Minimize DB roles, keep migrations privileged and runtime least-privilege; use Row-Level Security when applicable, with isolation tests that try cross-tenant access.

### 11.3 Plugins and trust boundaries

Native Rust crates run with process privileges; signed package/manifests and declared capabilities **alone cannot sandbox them**. Treat third-party native apps as trusted code requiring review, dependency scan and release verification. Optional Wasmtime runtime components may isolate capabilities using an explicit host ABI. No sandbox claim without adversarial testing of filesystem, network, DB, memory/fuel/time and escape behavior.

### 11.4 Verification standard

Map controls to current OWASP ASVS and use threat modeling per core subsystem; maintain explicit attack regression tests and a security exception register. **Never describe the system as provably 100% secure**. Instead document guarantees of restricted APIs, limits of enforcement and test results.

---

## 12. Browser, API, jobs and tracing contract

All operations have typed identities, inputs/outputs, error types, policy requirements and owners. A frontend action calls a generated client function with request/trace ID. Observability connects:

`UI element/event → typed client operation → HTTP middleware → auth policy → domain action → SQL span → event/outbox → background job → UI refresh`

Agents can ask `forge trace <id>` and receive relevant causal evidence, not raw log floods. Privacy controls redact secrets and PII before tool results. Error states are visible in frontend and integration tests. Jobs support retry/backoff, idempotency, DLQ, cancellation and safe replay. SSE and WebSocket interfaces are typed and tested when used.

---

## 13. Verification artifacts, CI and release policy

### 13.1 Report structure

`forge verify --full --format json` produces an independently generated, immutable-ish report bound to Git commit hash, lockfile hashes, tool versions, OS/browser image, test DB seed and relevant build environment. Include:

- Contract/architecture checks.
- Rust/frontend compile/typecheck.
- Coverage line/branch by module and exclusion ledger.
- Unit/integration/E2E case identifiers and pass/fail artifacts.
- Mutation/fuzz results and approved risk-based waivers.
- Model/schema drift and migrations data checks.
- Authorization/security checks and dependency audit.
- Browser traces, screenshots, golden diffs, screenshot review approval status.
- Performance results vs baselines.
- Detailed fail reasons and suggested reproductions.

Statuses: `VERIFIED`, `NOT_VERIFIED`, `BLOCKED_INFRASTRUCTURE`; **no missing data == pass**. UI displays results honestly. CI will only ship when all policy-required checks pass and human approvals are attached. Agent cannot sign off own changes or override coverage.

### 13.2 Build and rollout

- CI checks PRs in isolated envs; deploy previews for reviewers.
- Signed/attested OCI artifacts where practical; fixed dependencies; provenance linked to exact commit.
- Production DB migration approval/plan separate from ordinary deployment, especially destructive steps.
- Health checks and safe rollout/canary/rollback planning; ensure DB backward compatibility during rolling upgrades.
- No secret leakage through reports/artifacts/logs.

---

## 14. Performance engineering and benchmarks

Benchmarks must include user workloads (admin lists, permission checks, SQL joins, uploads, jobs, realtime updates) and synthetic stress cases. Compare framework overhead with equivalent Axum baseline; browser baseline with React Compiler/Solid/Svelte/Leptos/Qwik.

Measure: HTTP throughput/latency, DB p95, memory, startup, build time, cache behavior, client bundle compressed size, first paint, LCP, INP, CLS, layout/repaint CPU, browser heap, SSR cost, screenshot test run time, agent iteration latency. Define configurable budgets per product profile (`admin`, `public_site`, `realtime_app`, `editor`). Optimize only after profiling and regressions; no language-based performance guarantees.

---

## 15. Implementation plan, milestones and exit criteria

**General implementation discipline:** build a vertical slice that runs, tests and proves itself. Do not create empty stubs for everything in the map. Keep a running decision log and verified change history. Each phase is complete only if its demonstration and objective gates pass.

### Phase 0 — repository, ADRs and reproducible baseline

Deliver: Rust workspace skeleton, toolchain pin, CI, formatter/clippy, coverage verification prototype (confirm branch coverage works), architecture conventions, testcontainers/dev PG or equivalent, pre-commit optional, docs, threat model. ADRs: model-source-of-truth; test policy; frontend benchmark protocol; module boundaries.

Exit: clean fresh checkout builds/tests deterministically; `forge check` works on a minimal crate; no unjustified boilerplate.

### Phase 1 — first real Rust backend feature

Deliver: one app (`customers` or `orders`) with typed model, PG migration, constraint, SQLx query, typed action, authentication/policy, Axum route, `forge check`, basic test obligations and 100% scope instrumentation.

Exit: create/read/update with safe parameters, failed unauthorized requests, migration applied on empty/upgraded DB, integration tests pass. Demonstrate an attempted SQLi input cannot change SQL syntax through safe APIs.

### Phase 2 — Django-like admin and app installation

Deliver: schema registry, generated list/detail/create/edit UI, pagination/filter/search using constrained query builder, first-party admin app, installable external sample app with own models/migration/admin/routes, middleware registration, DI/mock services.

Exit: install third-party example and see admin integration without editing unrelated app source; test ordering/cycles/capability conflicts; authorization negative tests.

### Phase 3 — frontend evaluation and typed contract integration

Deliver: consistent benchmark apps for React Compiler, Solid, Svelte, Leptos and Qwik; measure startup/updates/bundle/SSR/complexity; ADR selects initial renderer. Wire generated contracts and state types to Rust actions, UI errors and validation.

Exit: reproducible benchmark report plus working typed end-to-end app; no unsupported blanket claims of superiority.

### Phase 4 — Verification Engine and visual E2E

Deliver: feature contracts, derived obligations, full release report, Playwright E2E with pinned browsers, screenshot comparisons, multimodal visual critique workflow, controlled baseline approvals, unit/integration/migration/authorization/security/coverage gates.

Exit: intentionally injected code, authorization, visual and migration regressions each block release and generate actionable evidence; app-owned coverage at required threshold.

### Phase 5 — Agent tooling and repository intelligence

Deliver: source/dependency graph, `forge project map`, module inspector, bounded context, impact analysis, structured compile/test diagnostics, CLI/MCP parity, Claude Code adapter with skills/hooks/agents, agent sandbox/test fixtures.

Exit: a coding agent can implement a modest business feature using Forge tooling and independently produced verification without being given the whole repo manually; evaluate cost/time/errors against basic terminal-only workflow.

### Phase 6 — production batteries and hardening

Deliver: advanced auth, jobs, cache, files, events/outbox, realtime, telemetry, plugin marketplace/compatibility rules, migration expansion/recovery, security threat tests, performance thresholds, more comprehensive admin customization.

Exit: representative SaaS demo and public app pass deployable release gates; documented operational runbooks.

### Phase 7 (optional research) — runtime sandbox and new UI compiler

WASM component plugins, visual DSL, Rust SSR + tiny JS runtime + lazy WASM islands **only if** benchmarks and real users justify complexity. Keep backward-compatible integration with selected frontend.

### Demonstrator: end-to-end order cancellation

Request: "Add order cancellation. Owner may cancel pending order; paid order cannot be cancelled; operation is idempotent; other users denied; mobile UI reflects states."

Expected:
1. Approved feature contract and test obligations exist before implementation.
2. Agent pulls task context and impact graph, changes action/schema/UI only as needed.
3. All public API and typed UI compile.
4. Unauthorized, missing, already-cancelled and paid cases tested; transaction/idempotency verified.
5. Real browser E2E executed; mobile+desktop screenshots compared/reviewed.
6. Coverage and policy gates satisfied; test+visual artifacts linked to commit.
7. PR can be reviewed with **evidence**, not prose claims.

---

## 16. Nonfunctional acceptance criteria and definition of done

A Forge feature is **done** only if:

- It meets an independently approved behavior/acceptance contract.
- It follows allowed domain boundaries and code ownership.
- Models/relations/constraints/migrations reflect real database state; no drift.
- API and UI share strongly typed validated contracts.
- AuthN/AuthZ and tenant/object access are enforced and tested negatively.
- Safe query APIs prevent untrusted text from entering SQL structure.
- App code meets 100% measurable line/branch coverage gate, with reviewed exclusions only.
- Required unit/integration/E2E/property/mutation/security/migration tests pass as per policy.
- Changed screens are verified in real browser, screenshots analyzed, baseline changes independently approved; no hidden a11y/overflow/errors.
- Performance budgets relevant to the feature pass or have approved documented deviations.
- Full verification artifact is bound to exact commit and is reproducible.
- Public extension/API changes are documented and compatibility tested.
- Release cannot proceed if a required check is missing or failed.

Framework itself must dogfood these principles on first-party apps and example projects.

---

## 17. Technical design questions for ADRs (do not silently guess)

1. Schema authority and migrations: derive macro vs typed IDL vs DB introspection; which allows best structural indexing and migration diff?
2. Can SQLx macro checking run conveniently in CI/offline with migration snapshots? How to report schema drift?
3. What minimum safe query API can render all admin filters without accidental injection or excessively dynamic Rust generic types?
4. How should compile-time app registration (Cargo crates) work across platforms, versioning and plugin-owned migrations?
5. Which boundaries need native trust vs WASM sandbox? Define explicit ABI and capabilities.
6. Which frontend wins *measured* requirements; does using JS SSR hurt or help overall operations?
7. How to connect frontend ownership/source maps to Rust model/action graph without fragile conventions?
8. How do we define and instrument 100% branch coverage for async, macros and frontend generated code consistently?
9. How does the model/CI identify required behavioral E2E scenarios without falsely claiming 100% of all possible paths?
10. How are visual baselines initially approved and how do we prevent reviewer self-approval?
11. What specific audit evidence constitutes a verified release? How do we attest artifacts and protect secrets?
12. What are viable performant dev reload strategies for Rust server + selected UI and safe migrational previews?
13. Which performance budgets suit CRUD/admin vs IDE/editor vs public landing pages?
14. What API/manifest compatibility guarantees apply to third-party apps across upgrades?
15. How to expose context/query tools safely on sensitive repositories or production-connected environments?

Where unknown, add an ADR with alternatives, concrete experiment, risks, conclusion and reversibility. Do **not** make implementation silently depend on an unsupported answer.

---

## 18. Coding standards for agents and human contributors

- Keep domain code small, typed, idiomatic Rust, explicit and testable.
- Prefer mature, maintained dependencies with clear licenses, security posture and lockfile versions; verify real library APIs before using them.
- No fabricated test results, tool outputs, benchmark numbers or screenshot reviews.
- Do not mass-generate empty files or speculative layers; build thin vertical slices.
- Avoid broad automatic refactors unrelated to the task.
- Keep compile-test cycles short using affected checks, then independently run full gates.
- Every new public symbol needs purpose, owner, contract and relevant tests.
- Changes to permissions, migrations, plugin privileges, baselines or verification configuration require explicit review.
- Version API/tool schemas and keep backwards compatibility or migration plan.
- Separate LLM suggestions from deterministic compiler/runtime outcomes.

---

## 19. External technical references (background, not product requirements)

Official/reference material for evaluating components:

- Axum middleware/Tower integration: https://docs.rs/axum/latest/axum/middleware/
- SQLx design and checked queries: https://github.com/launchbadge/sqlx and https://docs.rs/sqlx/latest/sqlx/
- Playwright visual comparison: https://playwright.dev/docs/test-snapshots
- Playwright browser automation: https://playwright.dev/docs/intro
- Wasmtime components: https://docs.rs/wasmtime/latest/wasmtime/component/
- OWASP ASVS: https://owasp.org/www-project-application-security-verification-standard/
- OWASP SQL injection prevention: https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html
- JS frameworks performance benchmark: https://krausest.github.io/js-framework-benchmark/
- Rust coverage: https://github.com/taiki-e/cargo-llvm-cov
- Rust mutation tests: https://github.com/sourcefrog/cargo-mutants
- MCP specification: https://modelcontextprotocol.io/specification

Research should verify compatibility/current version numbers before installation, rather than freeze versions from this document.

---

## 20. Bottom line

Forge is **not** a new chatbot framework, not React copied in Rust, not a thin wrapper around Axum, and not Django ORM recreated by hand. It is a **Django-productive, Rust-fast, LLM-native, verification-enforcing full-stack web framework** that gives coding agents structured, scoped information and forces externally checkable proof of correctness, security and UI quality. The production app itself stays deterministic and deployable without an LLM. All unresolved architecture decisions must be settled through tracked evidence and reversible ADRs.

---

## 21. Addendum 2026-10-29 — requirements added by the product owner

Stated by the product owner during the first implementation session; they carry
the same weight as §0 and are traced in `docs/requirements-matrix.md` (X-01…X-03).

1. **The framework itself must be fast.** §14 already requires comparing Forge
   overhead with an equivalent plain Axum baseline; this is now a standing
   budget enforced in CI, not a one-off measurement. Initial numeric budgets are
   proposals in ADR-0007 and must be replaced by measured values.
2. **E2E testing is a built-in battery.** The framework ships its own E2E harness
   (in-process app server bound to an isolated test database, real browser
   driver, screenshot capture, deterministic "page settled" signal) so that an
   application gets working E2E tests without custom glue. Design: ADR-0006.
3. **E2E fixtures in the style of UNICEF HOPE** (`github.com/unicef/hope`,
   `tests/e2e/conftest.py`): factories with overridable defaults that seed the
   test database from inside the test process, login by a server-minted session
   instead of the login form, page objects injected as fixtures, per-worker
   isolation, and screenshots attached on failure. How each mechanism maps to
   Rust is the subject of ADR-0006; the product owner explicitly does not yet
   know how to achieve this, so the ADR must carry experiments, not assertions.
