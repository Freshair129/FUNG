# RCA: meeting agent model-run migration rejected by Genesis

**Date:** 2026-09-25
**Risk:** HIGH
**Status:** Remediated and locally verified

## Symptom

The first Rust regression run compiled but 126 tests failed while installing the
new relational schema. The focused migration test reported `relational schema
cannot change foreign keys on an existing table`.

## Evidence

- `schema_v11` introduced `meeting_agent_runs` without `model_run_id`.
- The first v13 proposal added a nullable `model_run_id` column and a foreign
  key to `model_runs` on that existing table.
- `n3_migrates_v10_additively_and_keeps_canonical_lane_aggregates` failed at
  `install(&storage)` with the quoted Genesis error, before any agent test ran.

## Root Cause

Genesis permits an additive nullable column in an existing table but rejects
changes to that table's foreign-key set. The first v13 package attempted both.

## Why the issue escaped detection

The schema code type-checked, and the new model-output unit tests did not
install a database that already contained `meeting_agent_runs`. The full
migration test exposed the engine-level restriction.

## Remediation and prevention

Keep historical schemas v1–v12 unchanged and add only the nullable
`model_run_id` column in v13. Native code creates `model_runs` and the linked
`meeting_agent_runs` row in one Genesis commit, so the link is written
atomically without changing the old table's FK set. The migration regression
asserts that v12 lacks the column and v13 has it; rerun it against an existing
database before accepting future schema additions.

## Verification

The corrected v10-to-current migration test passed. The Rust library suite
passed 583 tests with 1 ignored and the sandbox-only AppContainer probe
excluded. The `meeting_knowledge` integration suite passed 16/16 with that
same probe excluded; the direct probe returned `0x80070002` in the restricted
sandbox.
