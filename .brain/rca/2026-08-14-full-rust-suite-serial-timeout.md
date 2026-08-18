---
version: "0.1.0b"
created_at: "2026-08-14T04:05:00+07:00,ATHER"
last_update: "2026-08-14T04:05:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "test-reliability"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: Full Rust library verification timed out under a serial override

## Symptom

The full command with an extra `--test-threads=1` override exceeded the
120-second shell budget. The captured tail then reported a broken pipe while
the test process was still writing output.

## Evidence

- Exact plan command `cargo test -j 1 --manifest-path src-tauri/Cargo.toml
  --lib` passed `212 passed; 0 failed` in 27.19 seconds.
- Explicit parallel run with `--test-threads=4` passed `212 passed; 0 failed`
  in 54.20 seconds.
- The timed-out serial run ended with `io error when listing tests:
  BrokenPipe`, which is a closed output pipe from the command timeout, not a
  Rust assertion or adapter failure.

## Root Cause

The verification invocation serialized 212 library tests with an unnecessary
`--test-threads=1` override. That execution exceeded the desktop shell budget;
the wrapper closed stdout and the still-running test binary observed
`BrokenPipe`.

## Why The Issue Escaped Detection

Focused Task 4 tests were intentionally serial, and that flag was copied to
the full-suite command. The plan itself only requires `cargo test -j 1` and
does not require serial test threads.

## Fix

Use the exact plan command (default test-thread scheduling) for full-suite
verification. Keep `--test-threads=1` only on bounded focused tests where
deterministic ordering is needed.

## Proposed Prevention

- Record the complete command and elapsed time for every closure run.
- Treat timeout/broken-pipe output as indeterminate until the exact command is
  rerun with a bounded but sufficient shell budget.
- Do not add serial test-thread overrides to the project-wide command without
  evidence that parallel scheduling is unsafe.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Recorded the serial full-suite timeout RCA and corrected verification command. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-08-14 | beta | Full 212-test Rust suite verified; serial timeout classified as harness procedure, not product failure. | working-tree | ATHER |
