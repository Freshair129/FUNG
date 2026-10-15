# Existing-profile recovery — FUNG #41

Date: 2026-10-15. Scope: Windows original-profile database startup and orderly
close/reopen. No programme, installer or release gate is promoted.

## Cause and repair

The original `dev.fung.local` profile failed inside `Storage::open`, before
FUNG's schema installer, with `REL_SCHEMA_VERSION_CONFLICT`. Its projection
already contained schema v13 while the retained journal contained earlier
schema registrations. Genesis recovery attempted to delete an open SQLite
projection; Windows refused, and the ignored deletion error caused replay
against the same incompatible projection.

[GenesisBlock #230](https://github.com/Freshair129/GenesisBlock/issues/230) and
[draft PR #233](https://github.com/Freshair129/GenesisBlock/pull/233) rebuild
into a SQLite-managed temporary database, then publish using SQLite's backup
transaction into the original open connection. Replay or publication failure
preserves the original database. Schema ordering checks remain enforced.

FUNG pins the minimal repair commit
`ddfc6e9468b79d7d447fd9392aedcc3277dc2ddb`, based on the previous pinned
`77e07f7b127f65a1f92cfc688ef15f322115d5f8`; unrelated engine changes are not
included. Startup provider defaults now insert only missing providers, preserving
existing endpoints, enabled state, labels and timestamps.

## Preservation evidence

Local evidence root: `G:\AI-Studio\verification\2026-10-15-fung-issue41`.

- Before touching the original database, backed up its 12 files (1,508,777
  bytes) to `original-profile-backup-20261015-090654`; SHA-256 hashes matched.
- Original profile: `C:\Users\suanr\AppData\Roaming\dev.fung.local`.
  It contains two provider rows and no project, recording or transcript rows.
  This profile cannot by itself establish populated-transcript preservation.
- A disposable copy reproduced the old failure (`before-fix-probe.log`).
- A fresh disposable copy opened, installed schema and checkpointed with the
  repaired dependency (`after-fix-probe.log`). Offline integrity is `ok`;
  hashes, columns and counts match for all 71 application tables.
  See `copied-profile-comparison.json` and the before/after JSON snapshots.
- No user database reset/deletion, replacement profile, or fixture insertion
  into the original profile was used.

## Automated checks

- Upstream red regression reproduced the schema conflict before repair.
- Upstream green checks: 36 related integration cases and two failure-safety
  cases. Replay error and SQLite publication contention preserve original rows;
  retry after releasing the lock succeeds. `engine-*.log` contains the receipts.
- FUNG copied-profile probe: 1/1 passed, explicitly selected with `--ignored`
  against the disposable copy only.
- FUNG correction/recovery tests: 8 passed, 1 intentionally ignored diagnostic.
  A child process writes schema history and a Thai transcript correction then
  exits without its final checkpoint. Three successive reopens retain project
  paths, recording metadata, transcript text and timing. Another case proves
  startup defaults preserve custom provider settings on repeated initialization.
- The controlled child exit is not exhaustive arbitrary-crash or power-loss
  qualification. Full repository, microphone, GPU and installer tests are not
  claimed by these bounded checks.

## Native lifecycle

`tauri build --debug --no-bundle` passed, including TypeScript/Vite. Normal
identifier `dev.fung.local` was retained; bundled resource copying was omitted.
The prior preview executable was preserved as `before-repair-preview.exe`.
No installer was replaced.

Fixed executable: `.codex/desktop-preview/fung.exe`, SHA-256
`3b989b9a911e636c6a6af4349f95cd928f20cdcead644c37c846a3167618058d`.
The first preflight boot opened and closed successfully with an explicit
shutdown checkpoint. Its inspection guard detected a nonempty SQLite WAL,
so subsequent offline comparisons used a WAL-aware read-only connection;
an immutable main-file-only read was not accepted as current-state evidence.

The recorded repeat run (`native-lifecycle.json`):

| Cycle | PID | Result |
|---|---|---|
| 1 | 368 | Responding FUNG window; normal `CloseMainWindow`; exit 0; checkpoint completed; integrity ok; all 71 application table hashes/counts/columns unchanged. |
| 2 | 15348 | Same close/reopen checks passed; identity and recording-output settings also unchanged on both closed cycles. |
| 3 | 29440 | Responding FUNG window reopened with the same executable and original profile; left open for the user. |

`boot*.stdout.log` records the original profile path; `boot*.stderr.log`
records successful checkpoints for both closed cycles. The 12 original backup
file hashes were rechecked before launch. `after-native-cycle*.json` contains
the WAL-aware data snapshots. The earlier dashboard report's failure at
`52d5de5` remains historical evidence.

This verifies native startup and idle orderly close/reopen with the local API
disabled. It does not establish rendered UI acceptance, active-job/API-write
shutdown, arbitrary power-loss recovery, audio playback or packaged release.

## Changelog

| Version | Date | Change |
|---|---|---|
| 0.3.22b | 2026-10-15 | Pin safe Windows projection recovery; preserve provider defaults; add copied-profile and populated reopen coverage. |
