# RCA: meeting-agent observe mode did not consume committed transcript events

**Date:** 2026-09-25
**Risk:** MEDIUM
**Status:** Remediated with automatic-trigger fail-closed gate

## Symptom

Starting the local meeting agent as `observe` changed its persisted mode and
reported `observing`, but no native path consumed newly committed transcript
events. The pure `MeetingAgentCoordinator` was not connected to production
event handling, so the state could imply activity that was not happening.

## Evidence

- `meeting_agent_start` in `src-tauri/src/lib.rs` committed the policy and
  called `update_meeting_agent_runtime`, without registering or advancing an
  event observer.
- Durable transcript events are emitted after commit in
  `src-tauri/src/live_meeting.rs::commit_revisioned_sources` and
  `src-tauri/src/lib.rs::correct_meeting_utterance`.
- `commit_meeting_ingest_batch` assigns `kind: "unknown_source"` and
  `speaker_cluster_id: "unknown"` to local revisioned ASR input. The current
  local stream therefore cannot prove that a candidate came from a non-self,
  non-agent participant.
- The approved local adapter plan requires current committed non-self/non-bot
  evidence and says edits cancel stale triggers.

## Root Cause

Agent start and transcript ingestion were implemented as separate paths. The
runtime stored only mode/state and had no last-observed cursor or native
post-commit callback. Separately, there is no trusted participant attribution
for the current local ASR path, so automatic drafting cannot safely infer who
spoke from audio track labels.

## Why the issue escaped detection

The fixture campaign covered policy changes and manual private drafts, while
the final source review did not require a committed-event-to-agent observer
integration case. The pure coordinator tests do not prove that production
transcript commits reach the coordinator.

## Remediation and prevention

Post-commit transcript events now advance the process-local agent cursor. A
newer cursor cancels an older active run, marks private drafts stale, and drops
their in-memory previews; when a stale draft was the current runtime state, the
status returns to observing. The panel exposes automatic triggering as blocked
with `TRUSTED_PARTICIPANT_ATTRIBUTION_UNAVAILABLE`; microphone/loopback labels
are not treated as participant identity. Automatic drafting remains
unavailable for local-ASR events until a trusted attribution source and
production trigger runner are implemented.

## Verification

The runtime regression verifies cursor advancement is idempotent, cancels an
older run, stales its draft, drops its preview, and resets status to observing.
The consolidated native
suite passed 577 tests with 1 ignored; all 32 registered Node suites and the
TypeScript/Vite build passed. The native-window acceptance lane remains
separate.
