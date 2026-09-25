# RCA: meeting transcript IPC project scope

**Date:** 2026-09-25<br>
**Risk:** HIGH<br>
**Status:** Remediated and locally verified; final source review found no blocker

## Symptom

The meeting transcript snapshot and replay IPC commands can return transcript
content using only a caller-supplied `recording_id` and cursor. The native read
does not verify that the recording belongs to the active Desktop project.

## Evidence

- `src-tauri/src/lib.rs::meeting_transcript_snapshot` accepts only
  `recording_id` and passes it to the adapter.
- `src-tauri/src/lib.rs::replay_meeting_events` accepts only the recording ID
  embedded in the cursor and also passes it to the adapter.
- `genesis_adapter::meeting_transcript_snapshot` and
  `genesis_adapter::replay_meeting_events` query by recording ID.
- The existing `meeting_session_for_recording(storage, project_id,
  recording_id)` helper enforces the project-to-recording relationship for
  scoped knowledge operations.

## Root Cause

The M1 snapshot/replay IPC path was added as a recording-only API and did not
carry the Desktop project scope used by neighboring commands. The transcript
storage keys are recording-scoped, so possession or discovery of a recording
ID is enough to reach the rows without the project ownership check.

## Why the issue escaped detection

The local transcript tests exercise cursor ordering and replay integrity, while
the command-contract tests verify command names and payload shapes. Neither
asserts that a different project is denied for a known recording ID.

## Remediation

Require `project_id` alongside `recording_id` at the IPC boundary and in the
native snapshot/replay helpers. Validate the project-recording relationship
before querying transcript state. The Tauri commands and Desktop bridge now
carry project scope, and the regression denies snapshot and replay requests
from another project for the same recording. The focused and integrated test
results passed in the final campaign.

## Verification

The Rust library regression passed and denies both cross-project snapshot and
replay access. The final source/security review confirmed `project_id` is
required through bridge, IPC and adapter. This is local fixture evidence; no
installed Tauri-window or external acceptance is claimed.
