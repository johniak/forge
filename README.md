# Forge

Forge is a full-stack web framework whose primary developer is an LLM coding
agent. It aims at Django-class productivity — typed models, migrations, a
generated admin, installable apps, auth, middleware, jobs — on a
Rust/Axum/PostgreSQL backend, and it refuses unverified work: every feature is
proven by tests, real-browser screenshots and a release report bound to a
commit.

> **Status: pre-alpha, Phase 0 (reproducible baseline).** Nothing here is
> usable for applications yet. What exists today is the tooling core
> (`forge check`, `forge coverage`) and the PostgreSQL test harness
> (`forge-testing`). See the [phase plan](docs/superpowers/specs/2026-10-29-forge-design.md#7-phase-plan-with-exit-criteria).

## Quickstart (contributors)

Requirements: `rustup`, Docker (or a PostgreSQL 17 server you can create
databases on), [`cargo-nextest`](https://nexte.st/), and for coverage
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov).

```sh
git clone https://github.com/johniak/forge && cd forge
cargo nextest run --workspace            # PostgreSQL from Docker, or set FORGE_TEST_PG_URL
cargo run -p forge-cli -- check          # fmt, clippy, manifests, architecture guard
```

Coverage (lines **and** branches, gated at 100 % per module):

```sh
rustup toolchain install nightly-2026-06-13 --profile minimal --component llvm-tools-preview
mkdir -p target/coverage
cargo +nightly-2026-06-13 llvm-cov nextest --branch --workspace \
  --json --summary-only --output-path target/coverage/coverage.json
cargo run -p forge-cli -- coverage --input target/coverage/coverage.json
```

`forge` exits `0` PASS, `1` FAIL, `3` NOT_VERIFIED (a check could not run),
`2` usage error; `--format json` gives a versioned report.

## Documents

- [`SPEC.md`](SPEC.md) — product requirements (authoritative)
- [Design and whole-project plan](docs/superpowers/specs/2026-10-29-forge-design.md)
- [Architecture decision records](docs/adrs/README.md)
- [Requirements traceability matrix](docs/requirements-matrix.md)
- [Threat model](docs/security/threat-model.md)
- [Contributing](CONTRIBUTING.md) · [Security policy](SECURITY.md) · [Changelog](CHANGELOG.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
