---
version: "0.1.1b"
created_at: "2026-09-24T00:19:02+07:00,RWANG,2c2559f"
last_update: "2026-09-24T00:31:51+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "transcription"
  doc_type: "remediation-spec"
  scope: "GAP-02 native live-worker routing only"
  complexity: "C-2"
  risk: "MEDIUM - shared batch/live resolver"
---

# Restore operational live worker routing

## Problem and governing contracts

The native live path sends JSONL requests to a persistent worker. On current
base `2c2559f`, turbo/medium incorrectly resolve the batch entry point and
fail before readiness. See the [confirmed RCA](../../.brain/rca/2026-09-24-operational-live-worker-routing.md).

Parent intent is unchanged: Desktop local-first capture and independent
worker execution under the existing architecture. Peer contracts are
[Whisper profiles v0.3.0b](2026-09-21-whisper-model-profiles.md), the persistent
JSONL contract in `scripts/transcribe_live.py`, the batch argv/JSON contract
in `scripts/transcribe.py`, and the opt-in candidate backend boundary.

## Approved bounded change

Change only `whisper_worker_script_for_profile` in `src-tauri/src/lib.rs`
and its focused unit tests. Ordinary live calls derive the sibling
`transcribe_live.py` from the descriptor's script directory. Ordinary batch
calls retain the descriptor's exact script path, including injected fixtures.
Candidate batch/live paths retain their existing separate Transformers scripts.
Do not repair just the smoke harness: production uses the same resolver.

| Profile | Batch result | Live result |
| --- | --- | --- |
| turbo | exact `runtime.script` | sibling `transcribe_live.py` |
| medium | exact `runtime.script` | sibling `transcribe_live.py` |
| thai-large-candidate | sibling `transcribe_transformers.py` | sibling `transcribe_transformers_live.py` |

No change to default turbo selection, compute types, model directories,
dependency versions, CUDA validation/fallback, offline policy, capture device
selection, schema, credentials, candidate model qualification or R3 security.
No timeout increase and no new worker abstraction.

## Verification and exit criteria

1. Add turbo and medium live-path regression cases and show failure on the
   current resolver; assert batch retains an injected custom script path.
2. Implement the minimal resolver correction; new cases and existing
   candidate/profile tests pass.
3. Run scoped formatting, clippy and relevant packaging/release contracts.
4. Rerun the native `live_smoke` binary with capture seconds `0`, a fresh
   isolated Genesis directory and the approved repository silence fixture.
   Assert real worker ready, one persisted chunk and 1000 ms capture duration;
   zero transcript segments is expected for this fixture. Inspect summary
   failure lines explicitly rather than accepting only process exit code.
5. Repeat the native resolver/worker smoke for medium CPU if feasible; retain
   turbo GPU, medium CPU/GPU standalone results as separate worker evidence.
6. Update RCA/closure evidence. Real speech, native GUI/device selection,
   output destination, restart/installer and release gates stay open.

Acceptance means the shared native resolver launches the proper protocol and
the bounded fixture flow completes. It does not establish live-room accuracy
or a complete summary/export flow without speech input and a qualified LLM.

## Rollback

The change has no data migration. Revert only its resolver/test diff if a
regression appears; keep staged model artifacts and evidence. Do not delete
or modify existing user projects or restore over their databases.

## Approval

The user explicitly approved this bounded remediation with “approve” after
reviewing version 0.1.0b. Implementation and verification are authorized for
the resolver and focused tests above. Provider/device/release gates remain
separate; the earlier “ลุยตามนั้น” approval covers the surrounding gap-closure sequence.

Implemented and verified locally: regression red/green, worker/profile 13/13,
packaging/release contracts 16/16, fmt/clippy, and native turbo GPU/medium CPU
fixture probes passed. Summary/export remained unavailable on silence as
expected. Exact outcomes and evidence hashes are in the
[closure ledger](../verification/implementation-reports/2026-09-24-gap-closure-runtime.md).

## Version Diff

No document → 0.1.0b: bounded routing behavior, regression matrix and rollback.

0.1.0b → 0.1.1b: recorded explicit user approval; scope and exit criteria unchanged.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.1b | 2026-09-24 | beta | User approved bounded resolver remediation and regression verification | base 2c2559f; working-tree | RWANG |
| 0.1.0b | 2026-09-24 | candidate | Proposed operational live worker routing correction after confirmed native failure | base 2c2559f; working-tree | RWANG |
