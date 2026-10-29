# ADR-0006 Built-in E2E harness and HOPE-style fixtures

**Status:** Proposed — Phase 0 seeds the database harness; Phase 1 adds factories
and minted login; Phase 4 adds browser, page objects, settled signal and
screenshots. Each step has an experiment below.  
**Date:** 2026-10-29  
**SPEC:** §9.6, §10, §21.2–21.3. Matrix: V-06, X-02, X-03.

## Context

The product owner wants E2E testing to be a battery the framework ships, with
fixtures "like UNICEF HOPE", and explicitly does not know how to get there.
HOPE (`github.com/unicef/hope`, Django + Selenium, `tests/e2e/conftest.py`,
read on 2026-10-29) achieves its ergonomics through these mechanisms:

| HOPE mechanism (evidence) | Why it matters |
|---|---|
| `live_server` fixture: the Django app runs **inside the pytest process**, bound to the test DB | factories and the browser see the same database; no fixture files to ship to a separate server |
| factory-boy factories (`UserFactory`, `BeneficiaryGroupFactory`, …) called from tests and `conftest.py` | rows are created in-process with overridable defaults; PR #6502 shows the pitfall: a factory that defaults to a *new* business area puts rows in the wrong tenant |
| autouse seeds (`create_unicef_partner`, `create_role_with_all_permissions`, `seed_currencies`) | every test starts from a known baseline |
| `login` fixture: adds a server-minted session cookie instead of driving the login form (PR #6436) | removes the slowest, flakiest step from every test |
| page objects injected as fixtures (`page_programme_management(browser)` …) | tests read as business steps, selectors live in one place |
| one Chrome per xdist worker (`driver`, session scope) + `reset_browser` per test | ~2 s saved per test; isolation kept |
| per-worker `download_path`/`screenshot_path`, `cache_per_xdist_worker` | parallel workers never share mutable state |
| `pytest_runtest_makereport` attaches screenshot on failure to the HTML report | evidence without extra code in tests |
| `wait_for_page_ready` only checks `document.readyState`, which is insufficient for client-side routing (PR #6496) | the app must expose a real "settled" signal |

## Decision (design to be proven by experiments)

Forge ships `forge-testing`, used by framework and app tests alike:

1. **Isolated PostgreSQL per test.** A shared PG instance (testcontainers
   locally, `FORGE_TEST_PG_URL` in CI); one template database per schema
   version with migrations + baseline seeds applied once; each test gets
   `CREATE DATABASE … TEMPLATE forge_tpl_<hash>` (milliseconds), dropped on
   `Drop`. Per-worker naming makes `cargo nextest` parallelism safe.
   *Phase 0 experiment:* connect, create from template, query, drop.
2. **In-process application server.** `TestApp::spawn(config)` builds the Axum
   router on an ephemeral port in the test's Tokio runtime with the test DB
   pool, like `live_server`. The browser and the factories share state.
   *Phase 1 experiment:* request through `reqwest` to the spawned app.
3. **Factories derived from the model registry.** `#[derive(ForgeModel)]`
   also emits a `<Model>Factory` with defaults per field type, `.with_<field>()`
   overrides and relation auto-creation. Defaults for scoped fields (tenant,
   business area) come from the test's `Scope` fixture, which is the fix for
   HOPE's PR #6502 class of bugs. *Phase 1 experiment:* `CustomerFactory`.
4. **Login by minted session.** `TestApp::login_as(&user)` creates a session
   row and returns the cookie; the browser context receives it. The login form
   is tested exactly once, in its own test. *Phase 1.*
5. **Fixture injection.** `rstest` (`#[fixture]`, `#[rstest]`, async fixtures,
   `#[with(...)]` overrides, `#[once]` for process-wide values such as the PG
   container) is the candidate mechanism; verified on docs.rs 0.28: `#[once]`
   values are never dropped, so container cleanup relies on testcontainers'
   reaper, and async fixtures need `#[future]`. *Phase 1 experiment:* express
   "logged-in admin on a seeded tenant" as three composed fixtures. If rstest
   proves awkward for async teardown, fall back to a small `TestContext`
   builder; the ADR will record the result.
6. **Browser layer (Phase 4).** Playwright drives pinned browsers; Forge
   generates typed page objects from the screen registry (`screens::orders::Detail`)
   so selectors are data, not strings in tests; one browser per nextest worker,
   context reset per test; screenshot + DOM + console attached to the structured
   test report on failure (`report.rs` artifacts).
7. **Settled signal (Phase 4).** The frontend runtime exposes
   `window.__forge.settled()` resolving when no fetch is in flight, no route
   transition is pending and no scheduled render remains; the E2E driver waits
   on it instead of `readyState` or sleeps. This is the built-in E2E
   differentiator the owner asked for.
8. **Rust-native CDP vs Playwright-via-Node** stays open; both are evaluated in
   Phase 4 with the same page-object API on top.

## Consequences

Apps get E2E "for free" once they declare screens and models; the framework
takes on the burden of keeping the harness fast (per-test DB from template,
one browser per worker). Node.js becomes a *development* dependency if
Playwright is chosen; it never becomes a runtime one.

## Reversibility

High for items 1–4 (plain library code). Medium for 6–7 (depends on the
renderer chosen in Phase 3).

## Evidence

Phase 0: `crates/forge-testing` database harness test (see Phase 0 report).
Later phases append here.
