# Threat model (skeleton)

**Status:** skeleton, Phase 0. Rows are filled in by the phase that builds each
subsystem; a row moves from `PLANNED` to `IMPLEMENTED` only with the attack
regression tests named in it. SPEC §11 is the source of the controls; the
mapping to OWASP ASVS is added per subsystem (SPEC §11.4).

## Assets

- Application data in PostgreSQL, including tenant-scoped data (D15).
- User credentials, sessions, API tokens; secrets in configuration.
- Admin capabilities (bulk actions, dynamic filters, uploads).
- Approval records and grants (D14): baseline approvals, coverage-exclusion
  ledger entries, architecture waivers, tool grants.
- The release evidence chain: CI results, coverage, screenshots, reports.
- Developer machines and CI runners executing Forge tooling and agents.

## Trust boundaries

1. Browser ↔ server (HTTP, cookies, CSRF, CORS).
2. Server ↔ PostgreSQL (runtime role vs migration role).
3. Application code ↔ third-party native apps (same process; review, not sandbox — SPEC §11.3).
4. Agent ↔ tooling (CLI/MCP tools with read vs write/DB/baseline/deploy grants).
5. Repository ↔ CI ↔ package registries (supply chain).
6. Server ↔ outbound HTTP (SSRF).

## STRIDE by area

| Area | Main threats | Planned controls (SPEC §11) | Phase | Status |
|---|---|---|---|---|
| SQL | Injection via dynamic SQL; identifier injection in filters/sorts | Static `query!` + committed `.sqlx`; generated CRUD; constrained builder with IR allowlists; `AssertSqlSafe`/`raw_sql` forbidden in app crates (ADR-0003) | P1–P2 | PLANNED |
| Authorization | Missing object/tenant checks; IDOR; mass assignment; privilege escalation via admin filters/bulk actions | Default deny; central policy on every model/API/admin/job access; permission test matrix; RLS for scoped tables | P1–P2, P6 | PLANNED |
| Sessions, CSRF, cookies | Session fixation/theft; CSRF on state changes | PG-backed sessions; SameSite/HttpOnly/Secure defaults; CSRF tokens for cookie auth | P1 | PLANNED |
| Output encoding | XSS in admin and app UI | Escape by default; no raw HTML without sanitization; CSP and security headers | P2–P3 | PLANNED |
| Uploads and storage | Path traversal; malicious files; quota abuse | Bounded storage namespaces; safe object IDs; size/type limits | P6 | PLANNED |
| Outbound HTTP | SSRF to metadata/internal services | Allowlisted connector with DNS/IP checks, redirect control, timeouts | P6 | PLANNED |
| Subprocesses | Shell injection | Argument arrays only, fixed executables, explicit cwd (already true for `forge check`) | P0+ | PARTIAL (tooling spawns `cargo` with argument arrays) |
| Third-party apps | Malicious or vulnerable native crates | Review, dependency scan, release verification; declared capabilities are not a sandbox; optional Wasm isolation later | P2, P7 | PLANNED |
| Tooling, MCP, agents | Agent self-approves baselines/exclusions/waivers; prompt injection through repository content; secrets in tool output | Signed approval records with keys the agent does not hold (D14); short-lived grants; redaction in tool results | P4–P5 | PLANNED |
| Test infrastructure | Credentials in logs; test harness dropping non-test databases | URLs in harness messages hide passwords; harness drops only `forge_test_*` databases it can date, never one in use | P0 | IMPLEMENTED (`forge-testing`, tests `wait_ready_gives_up_with_blocked_infrastructure_and_hides_the_password`, `sweep_drops_stale_but_never_live_databases`) |
| CI and supply chain | Compromised actions or dependencies; token abuse | Actions pinned to commit SHAs; `permissions: contents: read`; `--locked` builds; SBOM, `cargo-deny`/audit later | P0, P6 | PARTIAL (pinning, read-only token, `--locked`) |
| Secrets and logs | Secret leakage in logs, reports, screenshots | Typed secret config; log/trace redaction; screenshot masking | P1, P4, P6 | PLANNED |
| Availability | Resource exhaustion; brute force | Rate limits, request/body limits, timeouts, audit logs | P6 | PLANNED |

## Security exception register

Empty. Any accepted exception (escape-hatch SQL, waived control) is recorded
here with owner approval, reason, scope and expiry.
