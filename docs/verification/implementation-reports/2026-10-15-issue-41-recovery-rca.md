---
status: validation
created_at: "2026-10-15,Codex"
related_issue: 41
scope: Existing FUNG desktop profile recovery and startup preservation
---

# RCA: FUNG #41 existing-profile second boot

## Status

This remediation is based on FUNG `main` at
`2bf21a06aad9b4be38e567ae6738897fa702e79f`. It pins the merged GenesisBlock
PR #233 fix at `2cf485573d48d0e5e2ae76db647ec4c4dee543c3`, makes startup
provider seeding insert-only for existing IDs, and adds a disposable-profile
reopen regression. The regression passed locally on Windows (1/1) against that
exact pin; locked `cargo check` and `cargo build` also passed. The first full
PR run found two stale restore-frontier equality assertions. GenesisBlock's
own restore contract allows a local generation-publication frame, so those
FUNG tests now assert the restored frontier is at least the source frontier
while continuing to verify restored data. Both focused restore tests pass.
The latest local full-suite run passed 611 tests, with one unrelated two-second
stdio timing assertion failing under parallel load (it passed in isolation in
0.17s); updated exact-head PR CI is pending. Keep FUNG issue #41 open until CI
passes and the PR is merged.

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

GenesisBlock's [Windows projection replay RCA](https://github.com/Freshair129/GenesisBlock/blob/2cf485573d48d0e5e2ae76db647ec4c4dee543c3/.brain/rca/RCA--ISSUE-230-WINDOWS-PROJECTION-REPLAY.md)
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

- `src-tauri/Cargo.toml` pins `genesis-block-native` to the exact merged
  GenesisBlock PR #233 commit `2cf485573d48d0e5e2ae76db647ec4c4dee543c3`.
  `src-tauri/Cargo.lock` carries the matching Git source revision.
- `app_state` calls `seed_default_model_providers`, which preserves an existing
  provider row and its configuration while still adding a missing default.
- `existing_profile_second_open_preserves_schema_data_and_provider_configuration`
  in `src-tauri/src/lib.rs` creates a disposable on-disk profile, applies the
  full FUNG schema, writes project/recording/Thai transcript and customized
  provider rows, checkpoints and drops the handle, then reopens the same path,
  reapplies the schema and default seeding, and compares persisted rows.

The regression passed on Windows against the exact pin:

```text
cargo test --manifest-path src-tauri/Cargo.toml --locked --jobs 1 --lib transcript_correction_tests::existing_profile_second_open_preserves_schema_data_and_provider_configuration -- --exact
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 614 filtered out
```

Both locked local build commands passed on Windows:

```text
cargo check --manifest-path src-tauri/Cargo.toml --locked --jobs 1
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8m 20s

cargo build --manifest-path src-tauri/Cargo.toml --locked --jobs 1
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 53s
```

PR #83 must receive fresh exact-head CI after this pin and report update are
pushed.

GenesisBlock's `backup_restore_u9_tests` contract says a restored frontier may
include a local generation-publication frame and asserts
`restored.stable_frontier() >= bundle.stable_frontier`. FUNG's two older
restore tests asserted equality after reopening the restored target. They now
use the same lower-bound contract and still check restored notes, relations,
and audio metadata. Each focused restore test passed locally (1/1).

## Evidence and limits

- GenesisBlock PR #233 was merged at
  `2cf485573d48d0e5e2ae76db647ec4c4dee543c3`. Its exact merge-head checks
  completed with 53 successes, 5 skips, and 0 failures, including the Windows
  projection rebuild regressions. See the [merged PR and checks](https://github.com/Freshair129/GenesisBlock/pull/233).
- The first FUNG full suite against the new pin reported 610 passed, 2 failed,
  and 3 ignored; both failures were equality checks against the documented
  restore frontier contract. After updating those assertions, the local full
  suite reported 611 passed, 1 failed, and 3 ignored. The remaining failure
  was `stdio_adapter_timeout_and_cancel_are_bounded_and_cleanup_the_child`,
  whose `<2s` timing assertion failed under suite load; an isolated rerun passed
  in 0.17s. Updated exact-head CI remains the full-suite gate.
- A separate open FUNG PR #80 contains a copied-profile and idle native
  lifecycle report for an earlier GenesisBlock candidate, `ddfc6e9`. That
  evidence is supporting history only; it does not validate this branch's
  `2cf4855` dependency revision. See the [PR #80 report at its recorded
  head](https://github.com/Freshair129/FUNG/blob/0faa78dc3aa4ae6b5c9ab2b0c31249122e8bb3db/docs/verification/implementation-reports/2026-10-15-existing-profile-recovery.md).
- The disposable-profile test exercises a second open and compares persisted
  schema data and provider configuration. It does not modify or inspect a real
  user profile.
- Packaged startup, a successful picker interaction, arbitrary crash recovery,
  and final issue closure remain unverified by this branch.
