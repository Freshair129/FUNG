---
version: "0.1.0b"
created_at: "2026-08-12T07:00:00+07:00,ATHER"
last_update: "2026-08-12T07:00:00+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "test-runtime"
  scope: "D:\\FUNG codex/phase3-integration sync candidate"
  doc_type: "rca"
---

# RCA: Rust worker tests missed the Windows Python launcher

## Symptom

The Phase 3 candidate's standard `cargo test -j 1 --manifest-path
src-tauri/Cargo.toml` run initially failed six FUNGWIRE worker tests with
`FUNG Python runtime is missing at ...\\.venv-whisper\\Scripts\\python.exe`.
The same candidate suite passed 186/186 when the bundled debug-venv `Scripts`
directory was prepended to `PATH`; the post-merge-base sync candidate passes
the complete Rust suite at 195/195.

## Evidence

- The failing tests call the test-only `WhisperRuntime::for_test` helper and
  use `tests/fixtures/fake_transcribe.py`; they do not require a model or GPU.
- `resolve_test_python` queried only `where python` and `where python3`, then
  returned the source-tree `.venv-whisper` fallback when both names resolved
  only to the Windows Store alias.
- This workstation has a real Python 3.13 interpreter available through
  `C:\\Windows\\py.exe`, while the worktree `.venv-whisper` junction targets
  an empty `D:\\FUNG\\.venv-whisper` directory.
- Running the unchanged suite with
  `src-tauri\\target\\debug\\.venv-whisper\\Scripts` on `PATH` produced
  `186 passed; 0 failed`.

## Root Cause

The test-only interpreter discovery treated the `py.exe` launcher as absent.
Because it did not query `where py`, it selected an invalid source-tree venv
fallback even though a usable Windows Python launcher was installed.

## Why The Issue Escaped Detection

The prior CI-oriented fix covered machines exposing `python`/`python3` on
`PATH`, and the earlier local run had a bundled interpreter available. The
Windows launcher-only installation shape was not represented in the resolver's
candidate list or regression matrix.

## Fix

Include `py` in the Windows test interpreter candidates. `run_python_worker`
can invoke the launcher directly with the fixture script, so no production
runtime path or secret/provider boundary changes are required.

## Proposed Prevention

- Keep the full Rust regression as a required gate and record the interpreter
  discovery mode in the evidence report.
- Keep the fake-worker tests independent of model downloads and GPU hardware.
- If Python discovery changes again, exercise both `python`/`python3` and
  launcher-only Windows installations in the test matrix.

## Version Diff

| Version | Change |
|---------|--------|
| 0.1.0b | Recorded the Windows launcher discovery gap and its test-only fix. |
| 0.1.1b | Added post-merge-base 195/195 regression evidence without widening the fix scope. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.1b | 2026-08-12 | candidate | Added post-merge-base 195/195 regression evidence without widening the fix scope. | pending sync commit | ATHER |
| 0.1.0b | 2026-08-12 | candidate | Documented missing `py.exe` discovery and the 186-test regression evidence. | same commit | ATHER |
