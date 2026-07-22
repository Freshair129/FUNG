# RCA: Genesis relational schema adapter API drift

**Date:** 2026-07-22
**Scope:** `src-tauri/src/genesis_adapter.rs` only
**Risk:** Medium — local database schema registration and upgrades

## Symptom

`fung` no longer compiles against the current local `genesis-block-native` dependency.

## Evidence

- `G:\GenesisBlock_Dev\GenesisBlock\src\lib.rs` defines `RelationalColumn.default: Option<Value>` as a required Rust struct field.
- The same API defines required fields on `RelationalSchemaPackage`: `previous_version`, `package_id`, `schema_hash`, and `named_queries`.
- `validate_schema_upgrade` requires each upgrade to increment `schema_version` by one and set `previous_version` to the immediately preceding version.
- FUNG's adapter constructed `RelationalColumn` and `RelationalSchemaPackage` with the earlier, smaller field sets.

## Root Cause

The FUNG adapter encoded an older Genesis relational-schema struct shape and migration contract. Updating the path dependency exposed the incompatible literals at compile time; version 2 and version 3 schema packages also lacked the explicit migration ancestry the current runtime validates.

## Why the issue escaped detection

The adapter did not have a compatibility check against the current local Genesis path dependency after its relational schema API was expanded. Its schema-upgrade test existed but could not compile to exercise the current validation path.

## Proposed Prevention

Keep schema literals complete for the pinned Genesis API and run `cargo check --manifest-path src-tauri/Cargo.toml` whenever the local Genesis path dependency changes. Preserve explicit `previous_version` values for every sequential FUNG schema package.
