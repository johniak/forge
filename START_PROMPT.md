# Copy-paste this into Claude Code

I want you to begin implementing **Forge**, an LLM-first full-stack web framework. The complete, canonical requirements are in `SPEC.md` and your operating rules are in `CLAUDE.md` in this repository.

Before writing code, read both files completely and inspect the repository. Do not substitute a conventional React + TypeScript app for what is specified. The Rust backend, Django-class batteries (models, migrations, admin, installed apps, middleware), secure parameterized DB interface, rigorous enforced testing, real-browser screenshot verification, and developer tools exposed as both CLI and MCP are core requirements.

1. Create and maintain a requirement-to-milestone/test traceability matrix; do not lose deferred requirements.
2. Resolve uncertain design decisions using short ADRs and small measurable experiments. In particular, frontend choice must remain open pending comparative benchmarks; do **not** prematurely build a new UI DSL.
3. Plan Phase 0 and Phase 1 concisely, then **start implementing** the first working vertical slice now. Do not just write a theoretical architecture or empty placeholder crates.
4. Make tests, compilation and verification executable from the beginning. Do not claim anything is passing without actually running it; never lower coverage/security gates to obtain green results.
5. Report what you built, exact commands/results, files changed, evidence generated, outstanding risks and the next task. If blocked by missing tools, be explicit and still complete everything else feasible.

I want production-quality engineering, but incremental delivery. Aim for the smallest working step that demonstrably advances the entire specification.
