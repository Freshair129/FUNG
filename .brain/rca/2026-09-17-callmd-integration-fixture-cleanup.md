---
version: "0.1.0b"
created_at: "2026-09-17T08:16:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T08:16:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Windows test-fixture cleanup safety within existing integration-test lease"
  risk: "LOW generated test artifacts only"
---

# Fixture cleanup target must be verified

## Symptom / evidence

Pre-launch inspection of tests/callmdDesktopIntegration.test.mjs at08:14 finds
a task-specific mkdtemp under os.tmpdir(), then two recursive rm(fixtureRoot)
cleanup branches. The source does not explicitly verify/log the resolved exact
target before recursive cleanup. No unsafe deletion or data loss was observed.
The fixture has not been launched by the controller.

## Root cause / why it escaped

The new harness relies solely on its mkdtemp result. That limits normal scope,
but omits the explicit resolved-target check required by the Windows execution
policy. Existing Node behavior tests do not execute the separate fixture mode.

## Bounded prevention

Within the already approved integration-test path, integration Luna verifies
the canonical absolute generated directory is an immediate task-prefixed child
of the canonical temporary root before either recursive cleanup branch, logs the
exact generated path, and fails closed if containment is not verified. Preserve
the fixture behavior; no dependencies, production code, broad cleanup, user data
or new file lease. Do not execute an unverified recursive cleanup. This is a
pre-handoff safety correction, not a native/runtime failure or new feature.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record verified cleanup-target prerequisite for test harness | UNCOMMITTED;base376ef30 | Codex orchestrator |
