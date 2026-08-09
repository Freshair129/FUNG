---
status: resolved
date: 2026-08-10
scope: Desktop startup — legacy `fung.db` one-time import into GenesisBlockDB
severity: critical
---

# RCA — Legacy SQLite import aborts desktop startup in a permanent crash loop

## Symptom

`npm run desktop` built successfully, then the process exited with code 101 before
any window appeared. Relaunching reproduced the failure identically, every time,
with no way to reach the UI. The desktop app was unusable on any machine holding
a pre-Genesis `fung.db`.

## Evidence

- Launch log (`scratchpad/tauri-dev.log`, 2026-08-10 00:25):

  ```
  thread 'main' panicked at tauri-2.11.5/src/app.rs:1425:11:
  Failed to setup app: error encountered during setup hook:
  GenesisBlockDB error: REL_TYPE_MISMATCH: relational value does not match column type
  error: process didn't exit successfully: `target\debug\fung.exe` (exit code: 101)
  ```

- The failing call is inside `app_state()` (`src-tauri/src/lib.rs`), which runs
  `genesis_adapter::import_legacy_sqlite` whenever `fung.db` exists and the marker
  `genesisdb/legacy-fung-sqlite-import-v1.complete` does not.

- The user's real legacy database (`%APPDATA%/dev.fung.local/fung.db`, last written
  2026-07-12) holds 21 rows across 7 tables:
  `transcript_segments` 13, `job_events` 2, `model_providers` 2,
  `projects` 1, `recordings` 1, `jobs` 1, `audit_events` 1.

- A read-only probe binary (`src-tauri/src/bin/dbcheck.rs`) run against a **copy** of
  the live Genesis directory reproduced the failure and then confirmed the fix:

  ```
  install OK
  importing legacy sqlite .../fung.db ...
  legacy import OK: 21 rows          # after the fix; REL_TYPE_MISMATCH before it
  seed OK
  ```

- Type evidence: SQLite has no JSON or BOOLEAN storage class. The legacy schema
  stores `jobs.input_refs_json` as `TEXT '[]'` and `model_providers.enabled` as
  `INTEGER 1`, while the Genesis packages declare those same columns
  `RelationalColumnType::Json` and `::Boolean` respectively.

## Root cause

`import_legacy_sqlite` mapped SQLite *storage classes* straight onto Genesis
*column types*: `ValueRef::Text` became `Value::String`, `ValueRef::Integer` became
a JSON number, and the result was handed to `commit_rows` unchanged. For every
Json-typed column the engine received a string, and for every Boolean-typed column
it received a number, so the first `jobs` or `model_providers` row aborted the whole
transaction with `REL_TYPE_MISMATCH`.

The crash was made **permanent** rather than transient by the ordering in
`app_state()`: the completion marker is written only *after* a successful import.
A failed import therefore leaves no marker, so the next launch retries the same
doomed import and fails identically — a self-perpetuating loop with no user-visible
escape short of deleting the app data directory.

## Why it escaped detection

- The only regression test for this path,
  `legacy_sqlite_is_read_once_into_signed_genesis_rows`, builds a fixture containing
  a single `projects` row. Every column of `projects` is `Text` in both SQLite and
  Genesis, so the test never exercised a Json or Boolean column and passed against
  the broken mapper.
- No CI or test fixture contained a `jobs`, `model_providers`, or any other row with
  a `_json` / `enabled` column, which are precisely the shapes that fail.
- Recent development had been concentrated on mobile capture and the FUNGWIRE LAN
  tunnel. The desktop shell had not been launched against a populated legacy
  database in the interim, so a crash-on-every-launch defect sat undetected in the
  startup path.

## Fix

`src-tauri/src/genesis_adapter.rs`:

- New `coerce_legacy_value(value, column_type)` converts each imported value to the
  target column's declared type before it reaches `commit_rows`:
  Json ← parse the TEXT payload (falling back to the raw string if it is not valid
  JSON), Boolean ← `n != 0` / `"true"` / `"1"`, Real ← widen an integer,
  Integer ← narrow a whole-valued float. Every other pair passes through untouched.
- The import loop now calls it per column instead of inserting the raw storage-class
  value.
- New regression test `legacy_import_coerces_text_json_and_integer_booleans` builds a
  fixture with `projects` + `jobs` (TEXT `input_refs_json`) + `model_providers`
  (INTEGER `enabled`, TEXT `config_json`) and asserts all rows import.

## Prevention

- Any future legacy/import mapper must be tested against at least one Json column and
  one Boolean column; all-Text fixtures do not exercise the type boundary.
- Consider decoupling startup liveness from one-time migrations: a failed import
  should record the failure and let the app start degraded rather than abort the
  setup hook, so a data-shape defect can never become an unrecoverable boot loop.
  (Not implemented here — flagged for the integration owner, as it changes startup
  policy beyond the Live Meeting scope.)
