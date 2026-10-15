---
status: candidate
created_at: "2026-10-15,Codex"
related_issue: 41
scope: Existing FUNG desktop profile recovery and startup preservation
---

# RCA: FUNG #41 existing-profile second boot

## Status

This branch is a focused remediation candidate based on FUNG `main` at
`2bf21a06aad9b4be38e567ae6738897fa702e79f`. It pins GenesisBlock PR #233 head
`df33a989aeefde1507376941985c91caf01ff94c`, makes startup provider seeding
insert-only for existing IDs, and adds a disposable-profile reopen regression.
The Rust regression has **not been run in this preparation**. Keep FUNG issue
#41 open until GenesisBlock PR #233 is green and the owner completes the exact
local acceptance tests.

## Symptom

FUNG issue [#41](https://github.com/Freshair129/FUNG/issues/41) records a
successful first `npm run desktop` followed by a panic on the next launch:

```text
REL_SCHEMA_VERSION_CONFLICT: schema version must advance exactly once
```

The failure occurred while reopening the same persisted profile at
`%APPDATA%\dev.fung.local\genesisdb`, before the user could reach the app.
The issue report says the profile had no projects, so the symptom was not
limited to a populated project shape.

## Root cause

The previous FUNG diagnosis treated this as a schema-install/idempotency issue.
That conclusion was incomplete: the existing FUNG test re-registers schemas in
one open store and did not exercise a persisted profile whose relational
projection needed recovery during the next `Storage::open`.

GenesisBlock's [Windows projection replay RCA](https://github.com/Freshair129/GenesisBlock/blob/df33a989aeefde1507376941985c91caf01ff94c/.brain/rca/RCA--ISSUE-230-WINDOWS-PROJECTION-REPLAY.md)
traces the conflict to `projection_reopen_fresh`: it attempted to remove the
SQLite projection while its connection was still open, then ignored the
removal error. Windows kept the file locked, so the recovery path could reopen
the old projection and replay the retained journal against stale schema state.
GenesisBlock PR #233 stages replay in an engine-owned candidate and publishes
only a successfully rebuilt projection through SQLite's backup transaction;
schema-version validation remains strict.

There was a separate FUNG startup data-preservation issue on the same path:
`app_state` upserted the default model-provider rows on every launch. That
could replace a user's label, endpoint, enabled state, and timestamps even
when schema recovery succeeded. Startup now checks each default ID and commits
only missing rows.

## Remediation and regression

- `src-tauri/Cargo.toml` pins `genesis-block-native` to the exact GenesisBlock
  PR #233 candidate SHA. `src-tauri/Cargo.lock` carries the matching Git source
  revision; the package version and dependency list are unchanged.
- `app_state` calls `seed_default_model_providers`, which preserves an existing
  provider row and its configuration while still adding a missing default.
- `existing_profile_second_open_preserves_schema_data_and_provider_configuration`
  in `src-tauri/src/lib.rs` creates a disposable on-disk profile, applies the
  full FUNG schema, writes project/recording/Thai transcript and customized
  provider rows, checkpoints and drops the handle, then reopens the same path,
  reapplies the schema and default seeding, and compares persisted rows.

This is a test definition and source review, not a passing test result. It must
run against the exact pin after GenesisBlock PR #233 validation is complete.

## Evidence and limits

- The pinned upstream candidate is PR #233's exact head
  `df33a989aeefde1507376941985c91caf01ff94c`, which adds a wrapper fix for the
  non-exhaustive `StepResult` match. Its fresh CI matrix was queued/in progress
  when rechecked for this update; no green result is claimed. An earlier check
  set on the previous `1d2c2bbb` candidate had failed the scientific performance
  audit and MCP `package-contract`, so recheck the
  [live PR checks](https://github.com/Freshair129/GenesisBlock/pull/233) before
  accepting the pin.
- A separate open FUNG PR #80 contains a copied-profile and idle native
  lifecycle report for an earlier GenesisBlock candidate, `ddfc6e9`. That
  evidence is supporting history only; it does not validate this branch's
  `df33a989` dependency revision. See the [PR #80 report at its recorded
  head](https://github.com/Freshair129/FUNG/blob/0faa78dc3aa4ae6b5c9ab2b0c31249122e8bb3db/docs/verification/implementation-reports/2026-10-15-existing-profile-recovery.md).
- This preparation was limited to static review. No Cargo command, Rust build,
  test, package launch, or modification of a real user profile was performed.
- Packaged startup, a successful picker interaction, arbitrary crash recovery,
  and final issue closure remain unverified by this branch.
