---
version: "0.1.0b"
created_at: "2026-09-17T09:10:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T09:10:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Integration-owned History fixture exact-count assertion only"
  risk: "LOW"
---

# History fixture refresh count oracle

## Symptom

Six visible PASS rows were observed by main, but the current-pair assertion proves
at least one refresh, not exactly one. No duplicate production refresh is asserted.

## Evidence

Pauli independently inspected root tests/callmdDesktopIntegration.test.mjs at
SHA2562da26221cbbcba6805377edba8ad2eb4a826a9afab740e70f460b8be32758ddd.
Lines500-502 use list > baseline. Other five case booleans use captured baselines
before later display aggregation; their later displayed aggregate deltas are not
the oracles. No flaw in those five production/callback results was found.

## Root Cause

The positive current-pair oracle is weaker than the claimed exact-one expectation.
The prior browser observation is valid for its actual at-least-one assertion only.

## Why the issue escaped detection

The green row and displayed delta1 looked exact, but the boolean allowed larger
counts. Independent source-oracle inspection exposed that distinction.

## Proposed prevention

Within Socrates's already active IntegrationFIX1 test lease, change only this
boolean to equality with baseline+1, retain the real production React fixture,
and rerun it under a new test hash. No production code change or new dependency.
This is the existing approved one-refresh acceptance assertion, not a new feature.
Prior main proof is not retroactively relabeled; new evidence must be recorded.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Exact-count oracle correction inside existing integration test lease | UNCOMMITTED; base376ef30 | Codex orchestrator |
