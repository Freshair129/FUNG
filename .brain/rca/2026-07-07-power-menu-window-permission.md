---
version: "0.1.0b"
created_at: "2026-07-07T00:00:00+07:00,Agent: ATHER"
last_update: "2026-07-07T00:00:00+07:00,Agent: ATHER"
status: "beta"
attributes:
  domain: "desktop-runtime"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: Power menu close action does not close the Tauri window

## Symptom

The bottom-left power menu opens, but pressing `ปิด` does not close the app window.

## Evidence

- Frontend calls `getCurrentWindow().close()` through `closeWindow()`.
- Tauri capability file only grants `core:default`.
- Generated Tauri schema shows `core:window:default` does not include `allow-close` or `allow-minimize`.
- `core:window:allow-close` and `core:window:allow-minimize` are available explicit permissions.

## Root Cause

The window close/minimize commands were not granted to the `main` window capability. Tauri v2 blocks ungranted core window commands even when the frontend imports the API correctly.

## Why The Issue Escaped Detection

Previous validation used browser preview and build checks. Browser preview cannot exercise native Tauri window permissions, and `cargo check` does not validate runtime capability intent.

## Fix

Add explicit permissions:

- `core:window:allow-close`
- `core:window:allow-minimize`

## Proposed Prevention

- Any new Tauri window control must update `src-tauri/capabilities/default.json`.
- Native window actions must be smoke-tested in the desktop runtime, not only browser preview.

## Version Diff

| Version | Change |
|---------|--------|
| 0.1.0b | Initial RCA for missing Tauri window permissions. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-07 | beta | Added RCA for blocked power menu close/minimize actions. | N/A | ATHER |
