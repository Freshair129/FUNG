# RCA: new local model adapter omitted from egress register

**Date:** 2026-09-25
**Risk:** MEDIUM
**Status:** Remediated and locally verified; CI rerun pending

## Symptom

PR #67's frontend CI failed at `npm run test:egress`, although its build and
Meeting Intelligence contract tests passed. The same test failed locally with
`meeting_agent_model can reach the network but docs/appendices/E-egress-register.md
does not mention it`.

## Evidence

- `meeting_agent_model.rs` uses `reqwest` for local `/api/tags` and `/api/chat`.
- `tests/egressRegister.test.mjs` scans network-capable modules and requires
  each module name in Appendix E.
- Appendix E had the generic summary Ollama path and candidate external agent
  paths, but no entry for the implemented local Meeting Agent adapter.

## Root Cause

The implementation introduced a new network-capable module without updating
the repository's network egress register in the same change.

## Why the issue escaped detection

The pre-PR local suite targeted Meeting Intelligence tests and build, while
the broader `test:egress` gate ran only in CI. Those targeted checks do not
scan newly added Rust modules for network primitives.

## Remediation and prevention

Appendix E now records the exact payload, loopback destination, opt-in user
gate and call site. The candidate external paths remain separately labelled.
Run `npm run test:egress` whenever a new module creates a network client.

## Verification

`npm run test:egress` passed 8/8 locally after the register update. The PR CI
rerun is pending.
