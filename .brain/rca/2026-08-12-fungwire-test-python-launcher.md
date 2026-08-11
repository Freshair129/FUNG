---
version: "0.1.0b"
created_at: "2026-08-12T03:05:00+07:00,Agent: ATHER"
last_update: "2026-08-12T03:05:00+07:00,Agent: ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-test-infrastructure"
  doc_type: "rca"
  scope: "FUNGWIRE Rust transcription tests"
---

# RCA — FUNGWIRE Rust Tests Could Not Resolve Windows Python Launcher

## Symptom

The full Rust library run passed 189/195 tests. Six FUNGWIRE client/server
transcription tests failed because the test worker reported:

`FUNG Python runtime is missing at D:\FUNG\.venv-whisper\Scripts\python.exe.`

## Evidence

- `src-tauri/src/lib.rs::resolve_test_python` probes only `where python` and
  `where python3`, then falls back to the empty `.venv-whisper` directory.
- `where.exe py` resolves `C:\Windows\py.exe` on this workstation.
- `C:\Windows\py.exe --version` returns Python 3.13.7 and can execute
  `src-tauri/tests/fixtures/fake_transcribe.py --help`.
- The six failing tests all use `WhisperRuntime::for_test` with the
  dependency-free fake transcriber, so they do not require faster-whisper or a
  model download.

## Root Cause

The Windows test-runtime resolver assumes a `python.exe` or `python3` command
is discoverable. This workstation exposes Python through the standard Windows
Python Launcher (`py.exe`) only, so the resolver selects the empty bundled
fallback and the real subprocess plumbing is rejected before the fixture runs.

## Why the Issue Escaped Detection

The earlier focused external-MCP and frontend gates do not exercise FUNGWIRE
delegated transcription. The release-layout GPU smoke uses an explicitly
populated runtime, while the unit tests relied on a launcher convention that
was not present on this Windows installation.

## Proposed Prevention

Keep the test-only resolver independent from a populated application bundle;
probe `py` after `python`/`python3` on Windows, retain the existing bundled
fallback, and keep the full Rust library run as the regression gate. Add a
focused resolver assertion if the launcher selection logic changes again.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Recorded the evidence-backed Windows launcher root cause and minimal prevention. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-12 | candidate | Recorded FUNGWIRE test Python launcher RCA. | pending | ATHER |
