---
version: "0.1.1b"
created_at: "2026-09-24T00:19:02+07:00,RWANG,2c2559f"
last_update: "2026-09-24T00:29:52+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "transcription"
  doc_type: "rca"
  scope: "Operational turbo/medium live worker routing regression"
  risk: "MEDIUM"
---

# Operational live worker selects the batch entry point

## Symptom

After successful staging and standalone worker qualification, the native
`live_smoke` harness in zero-second inject mode exits 1 before live readiness:
`live worker exited or stalled before ready`. Its stderr contains
`transcribe.py: error: either --manifest (non-empty) or at least one positional audio path is required`.
No microphone was opened. The isolated fixture database was preserved.

## Evidence

- Base source: `2c2559fdd907cf94a6f38b34bbb2c6650e68a0e1`.
- `whisper_runtime()` constructs `WhisperRuntime.script` as `scripts/transcribe.py`.
  The headless harness constructs the same batch script path.
- `LiveWorker::spawn()` calls `whisper_worker_script(runtime, true)` and
  starts the result without audio argv, using persistent JSONL stdin/stdout.
- `whisper_worker_script_for_profile()` checks `live` only inside the
  `THAI_CANDIDATE_PROFILE` branch. All ordinary profiles return
  `runtime.script.clone()`, including when `live=true`.
- `git show 021d816 -- src-tauri/src/live_meeting.rs` shows replacement of the
  former explicit sibling `transcribe_live.py` path with this shared helper.
- Standalone `transcribe_live.py` passed with the pinned turbo GPU and medium
  CPU/GPU models. This isolates routing from missing model/dependency causes.
- Actual native log: `.runtime-cache/gap-closure-20260924/native-inject.log`.
  Build succeeded; execution failed at readiness. The harness and production
  coordinator share this worker boundary; native GUI interaction was not tested.

## Root Cause

Commit `021d816f090e058068c581bd65228bd348761d72` introduced candidate routing
without preserving the non-candidate live entry-point selection. A batch
runtime descriptor is valid input to the shared resolver, but the resolver
does not translate it to the live sibling script for turbo/medium. The batch
parser rejects the persistent worker invocation before it can emit `ready`.

## Why the issue escaped detection

`thai_candidate_uses_separate_model_root_and_workers` covers candidate batch
and live paths and ordinary turbo batch only; it omits ordinary live paths.
Packaging tests prove the scripts exist, not that the native caller chooses
the correct protocol. CI fixtures and successful standalone worker runs do
not close this native routing path. The prior missing operational models also
prevented this workspace's real-model readiness check from reaching the bug.

## Proposed prevention

Restore live selection centrally for ordinary profiles while preserving
custom batch test paths and candidate paths. Add ordinary turbo/medium live
and batch regression cases, retain candidate regressions, and rerun the native
zero-second fixture harness. Check report content as well as exit code:
the harness can report summary/export failure while returning `Ok(report)`.
No timeout increase, fallback model change or worker-protocol workaround is needed.

The user approved the bounded
[remediation spec](../../docs/specs/2026-09-24-operational-live-worker-routing-remediation.md).
The resolver now selects the persistent sibling script for ordinary live calls;
batch and candidate behavior are unchanged. Two focused regressions were
added. Before the fix, the live-path test failed with both actual paths equal
to `scripts/transcribe.py` while the injected batch-path test passed. After the
fix, all 13 worker/profile tests and 16 packaging/release contracts passed.
The [closure ledger](../../docs/verification/implementation-reports/2026-09-24-gap-closure-runtime.md)
records native verification separately from these unit/contract results.
Both native zero-second inject runs (turbo GPU and medium CPU) now reach
readiness and persist one 1000 ms chunk. Summary/export has no transcript to
consume on the approved silence fixture and is not counted as accepted.

## Version Diff

No document → 0.1.0b: evidence-backed routing RCA and scoped prevention.

0.1.0b → 0.1.1b: recorded approved minimal correction and red/green regression evidence.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.1b | 2026-09-24 | beta | Implemented approved resolver correction; red/green evidence recorded | base 2c2559f; working-tree | RWANG |
| 0.1.0b | 2026-09-24 | beta | Confirmed ordinary-profile live/batch selection regression with native fixture evidence | base 2c2559f; working-tree | RWANG |
