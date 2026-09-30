# RCA: Local benchmark output collector blocks after inference

Date: 2026-09-30
Scope: one-off LOTUSDIS `large-v3-turbo` evaluation orchestration; FUNG worker code is unchanged.

## Symptom

The FUNG worker reported `PROGRESS 100`, but the local wrapper did not finish collecting its JSON result or write predictions and CER metrics.

## Evidence

- The worker log ends with `PROGRESS 100`.
- `worker-output.json`, predictions, and metrics were not written because the wrapper had not passed its output-read step.
- Process inspection confirmed the live child command was `scripts/transcribe.py --manifest ... --model ...large-v3-turbo --language th --profile cpu`, with the one-off collector as its parent.
- The collector iterated over `stderr` until EOF while keeping `stdout=PIPE`; only after the stderr loop did it read stdout. The worker emits its final JSON on stdout after reporting progress 100.

## Root Cause

The wrapper drained the worker's stderr synchronously but left its stdout pipe unread. The worker's final JSON write filled the anonymous pipe buffer and blocked before process exit; meanwhile, the parent waited for stderr EOF. This formed a pipe deadlock after transcription, rather than an ASR/model failure.

## Why the issue escaped detection

The one-off collector had not been exercised with a complete multi-clip result large enough to exceed the stdout pipe buffer. Progress output was visible, but successful inference completion was incorrectly treated as equivalent to result collection.

## Proposed prevention

Redirect worker stdout directly to a result file (or drain stdout and stderr concurrently), then parse and validate the JSON schema after process exit. The retry must assert a successful exit, one prediction record per manifest ID, and complete CER output before declaring the test finished.
