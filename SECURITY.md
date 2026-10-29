# Security policy

## Supported versions

Forge is pre-alpha (Phase 0). No version is supported for production use
before `0.1.0`; fixes land on `main`.

## Reporting a vulnerability

Please report privately through GitHub's **"Report a vulnerability"** button
on the [Security tab](https://github.com/johniak/forge/security) of this
repository (private vulnerability reporting). Do not open a public issue for
a vulnerability.

Include what you found, how to reproduce it, and the impact you expect. We aim
to acknowledge reports within 7 days and to agree on a disclosure timeline
with you once the issue is understood.

## What we promise and what we do not

Forge is designed to be secure by construction — parameterized SQL,
allowlisted identifiers, default-deny authorization, central object
authorization, safe defaults — and its controls are tracked in the
[threat model](docs/security/threat-model.md). We do not claim that Forge, or
an application built with it, is provably secure; we document the guarantees
of the restricted APIs, the limits of their enforcement and the tests behind
them (SPEC §11.4).
