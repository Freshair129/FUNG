# RCA: Qwen worker cannot import its sibling alignment module

Date: 2026-09-30  
Risk: MEDIUM — isolated candidate worker execution  
Status: Root cause confirmed; fix pending verification

## Symptom

The real Rust Qwen worker exited before model inference with
`ModuleNotFoundError: No module named 'thai_ctc_alignment'`.

## Evidence

- The ignored Rust LOTUSDIS pilot test invoked the staged Python 3.12.10
  interpreter and failed while importing the worker's CTC helper.
- The CTC helper file exists beside `transcribe_qwen_detailed.py` in
  `scripts/`.
- The isolated interpreter's `sys.path` contains only its embedded zip,
  `Scripts`, and `Lib/site-packages`; the repository `scripts/` directory is
  absent.

## Root Cause

The embedded Python `._pth` isolation does not add the executed script's
directory to `sys.path`. The worker imported its sibling helper by module name
without explicitly adding that source directory.

## Why the issue escaped detection

The unit tests inserted `scripts/` into the test process's `sys.path`, so they
did not reproduce the staged embedded interpreter's isolated import path. The
first production Rust-worker run exposed the mismatch.

## Proposed prevention

Resolve the worker file's own parent directory and add only that directory to
`sys.path` before importing the sibling CTC helper. Verify through the staged
Rust worker again.
