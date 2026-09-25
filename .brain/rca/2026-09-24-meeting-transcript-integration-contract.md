---
version: "0.1.0b"
created_at: "2026-09-24T04:01:35+07:00,RWANG,2c2559f"
last_update: "2026-09-24T04:01:35+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "root-cause-analysis"
  scope: "Static contract mismatch before multi-source production integration"
  complexity: "C-3"
  risk: "HIGH"
---

# Transcript integration: source consumption and recording cursors

## Symptom

The requested M1 production integration cannot safely call the existing atomic
adapter once per Whisper segment. One audio chunk can produce zero or multiple
segments, and later human corrections reuse the same audio. Mic and system
sources also need one combined replay order.

This is a source-derived integration finding, not a newly observed production
failure. No reproducer or test has been run in this turn: the user requested
consolidated testing after implementation. Existing accepted R3 security evidence
remains bounded and unchanged.

## Evidence

Source inspected at base `2c2559f` with the pre-existing approved routing diff:

- `src-tauri/src/meeting_intelligence_schema.rs:181`: `AtomicMeetingRequest`
  contains one revision and source coverage.
- `src-tauri/src/genesis_adapter.rs:3421`: commit checks the minimum supplied
  source sequence against the previous source sequence plus one; an already
  consumed chunk is not a new input sequence.
- `src-tauri/src/live_meeting.rs:1660`: the worker response is iterated as
  zero or more segments for one chunk. Its legacy segment commit and chunk
  completion stamp are separate operations.
- `src-tauri/src/genesis_adapter.rs:1256`: the event log has a unique
  `(recording_id, cursor)` index.
- `src-tauri/src/genesis_adapter.rs:3400`: the last event cursor used for
  progression comes from the source-specific cursor row. With neither source
  initialized, both mic and system require event cursor zero.
- The schema already contains v11 transcript/knowledge/agent aggregates;
  `genesis_adapter.rs:1731` registers the historical schema chain. Recreating
  these tables is unnecessary and would risk migration compatibility.

## Root cause

The bounded foundation combines consumption of new source coverage, one text
revision, and event advancement into one operation. That fits its single-source
fixture contract but does not model a zero/multiple-utterance batch or a
revision-only edit over existing coverage. Event sequencing is also validated
at source scope while persistence enforces recording scope.

These mismatched invariants are directly visible in source. Runtime reproduction,
final migration behavior and the proposed correction remain unverified.

## Why the issue escaped detection

The foundation was independently accepted for a bounded identity/custody and
AccountCommitFence scope. It is not the current production live writer, and
its acceptance did not claim multi-channel M1 replay, a complete correction
service or zero/multiple-segment batch integration. The broader integration
tests belong to the now-expanded implementation scope.

## Proposed prevention

Use the [expanded implementation plan](../../docs/plans/2026-09-24-meeting-intelligence-local-adapters.md):

1. Add typed batch-ingest and revision-only operations; retain verified coverage
   identity and raw ASR. Commit silence completion without inventing text.
2. Allocate one recording-wide cursor under the Genesis frontier CAS; retain
   source sequence tracking separately.
3. Add source-only/control events and immutable group identity additively;
   preserve historical schemas and the accepted broker/vault commit fences.
4. At the final campaign, execute two-source first-event ordering, multiple
   utterances per chunk, silence, correction with existing audio, crash/replay,
   idempotency, stale/manual revision and account-switch regressions.

No runtime fix is included in this RCA/documentation turn.

## Version Diff

No document → 0.1.0b: static integration evidence, bounded root cause,
verification limits and prevention plan.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-09-24 | candidate | Recorded source-derived M1 batch/cursor gaps before expanded implementation | base 2c2559f; working-tree | RWANG |
