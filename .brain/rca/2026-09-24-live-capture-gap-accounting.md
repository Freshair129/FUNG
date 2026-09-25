---
version: "0.1.0b"
created_at: "2026-09-24T10:30:00+07:00,RWANG,working-tree"
last_update: "2026-09-24T10:30:00+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "root-cause-analysis"
  scope: "Static review of bounded live capture and media-gap persistence"
  complexity: "C-2"
  risk: "HIGH"
---

# Live capture gaps and catch-up memory

## Symptom

The live capture callback used a lossy handoff without a capacity bound or a
sample timeline. A failed WAV write reported only an error, so revisioned
transcripts could not persist the exact missing source interval. The degraded
transcription path also retained every unprocessed chunk descriptor in memory
until recording ended.

This finding comes from source inspection during the approved local M1 work. No
runtime failure was reproduced in this turn; execution is deferred to the
consolidated campaign requested by the user.

## Evidence

- `src-tauri/src/live_meeting.rs::build_stream` forwards callback samples to a
  consumer; the callback and its queue pressure were not represented in
  revisioned source coverage.
- `cut_capture_chunk` knew the interval before a WAV write, but the
  `ChunkWriteFailed` event carried only channel and error.
- The coordinator persisted source gaps only for callback queue losses, not
  failed chunk writes.
- `pending_transcription` accumulated `RawChunk` values for the full recording,
  even though the durable `audio_chunks` ledger and `transcribed_at` stamps can
  identify unfinished work.
- Before this change, capture tests covered job outcome wording but not sample
  queue holes, exact gap intervals or bounded catch-up state.

## Root Cause

Capture had no explicit bounded media handoff contract. Audio time was derived
only from successfully written chunks, so dropped callback batches and failed
file writes could not always become durable, ordered gaps. Catch-up state was
also owned by the session loop instead of reconstructed from the durable
recording ledger.

## Why the issue escaped detection

Existing checks covered chunk outcome classification and successful native
recording paths. They did not force a callback queue hole or a file-write
failure, and they did not bound the metadata retained for degraded sessions.

## Proposed prevention

Use a bounded non-blocking sample queue with absolute sample positions. Convert
queue drops, too-short stop tails and failed chunk writes to explicit source
gaps; flush and reset revision windows at each gap. Reconstruct legacy catch-up
work from the durable audio ledger after capture closes instead of retaining a
session-length vector. Add focused regression cases and keep final behavior
unaccepted until the consolidated test campaign passes.

## Verification boundary

The implementation and regression cases are present in the working tree but
have not been executed. This RCA records an evidence-based static finding; it
does not claim a native audio-device run or production acceptance.
