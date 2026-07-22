---
status: resolved
date: 2026-07-21
scope: Android startup after on-device AI P1 integration
severity: high
---

# RCA — Genesis relational schema registration aborts Android startup

## Symptom

The Android application installed successfully but exited during startup before the UI became usable.

## Evidence

- Device: Samsung `SM-A075F`, Android 16, serial `R8YY91PX3ZT`.
- `output/uat-android-genesis/ai-profile-launch.log` records the exact Rust panic:
  `Failed to setup app: error encountered during setup hook: GenesisBlockDB error: relational schema version must increase`.
- The same log records `SIGABRT` immediately afterwards in the Rust app-start thread.
- `src-tauri/src/genesis_adapter.rs` previously registered schema versions 1, 2 and 3 on *every* application startup. A device that has already persisted version 3 rejects the next registration of version 1 by GenesisBlockDB's monotonic migration guard.

## Root cause

FUNG replayed its historical schema packages at every startup. GenesisBlockDB correctly disallows a downgrade, so reopening an existing version-3 namespace attempted `3 -> 1` and made the Tauri setup hook fail.

## Why it escaped detection

The automated Genesis tests opened fresh storage and installed the schema only once. Android UAT had not included an upgrade/relaunch path using a persisted schema-version-3 database.

## Fix and prevention

- Register only the current schema package during startup; GenesisBlockDB already upgrades older persisted schemas when the current package version is greater.
- Add a regression test that opens storage, advances it through v1, v2 and v3, and then calls FUNG's startup installation function again.
- Add persisted-database relaunch to the Android smoke/UAT gate before release.
