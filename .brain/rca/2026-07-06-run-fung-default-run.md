---
version: "0.1.0b"
created_at: "2026-07-06T04:45:00+07:00,Agent: ATHER"
last_update: "2026-07-06T04:45:00+07:00,Agent: ATHER"
status: "beta"
attributes:
  domain: "desktop-runtime"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: RUN_FUNG.bat fails at Tauri dev launch

## Symptom

Running `D:\FUNG\RUN_FUNG.bat` exits with an error after Vite starts.

## Evidence

Observed command output:

```text
error: `cargo run` could not determine which binary to run. Use the `--bin` option to specify a binary, or the `default-run` manifest key.
available binaries: fung, fung-cli
```

`src-tauri/Cargo.toml` defines package `fung`, and Cargo discovers both the desktop binary `fung` and CLI binary `fung-cli`.

## Root Cause

The Tauri dev command calls `cargo run` without a `--bin` argument. Because the crate exposes two runnable binaries, Cargo requires an explicit default binary. `Cargo.toml` did not set `default-run`.

## Why The Issue Escaped Detection

Previous validation used `cargo check` and `RUN_FUNG.bat --check`. Those checks confirm tool availability and compilation, but they do not exercise the `tauri dev` runtime path that invokes `cargo run`.

## Fix

Set the Cargo package default runner:

```toml
default-run = "fung"
```

## Proposed Prevention

- Include a short `tauri dev` smoke test after adding additional binaries.
- Keep CLI binaries named explicitly, but set `default-run` for the GUI app.

## Version Diff

| Version | Change |
|---------|--------|
| 0.1.0b | Initial RCA for launcher failure caused by ambiguous Cargo binary selection. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-06 | beta | Added RCA and prevention for RUN_FUNG.bat cargo binary ambiguity. | N/A | ATHER |
