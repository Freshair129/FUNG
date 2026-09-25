# RCA: stale meeting-agent drafts could reach native preview and approval

**Date:** 2026-09-25
**Risk:** MEDIUM
**Status:** Remediated and locally verified

## Symptom

The Desktop panel marked a private draft stale and removed the visible delivery
preview after a newer transcript cursor arrived. The native preview and
approval commands only checked draft state and expiry, so a concurrent or
direct command could still create or approve a preview based on an older
transcript.

## Evidence

- `MeetingIntelligencePanel.tsx` compares transcript event cursors to
  `basedOnTranscriptCursor` and clears local preview state.
- `meeting_agent_preview_delivery` in `src-tauri/src/lib.rs` only required a
  `private` draft and valid expiry before calling the persistence boundary.
- `meeting_agent_approve_delivery` checked the runtime draft and payload hash,
  but did not compare the draft's transcript cursor to the durable high-water
  mark.
- `persist_private_meeting_agent_draft` stored `transcript_cursor` in the run
  row but did not check that it was current at the Genesis commit boundary.
- Native preview and approval already use `begin_meeting_commit` and commit
  against the captured Genesis frontier; a transcript-cursor comparison in
  those operations can therefore reject stale state and concurrent changes.

## Root Cause

Freshness was enforced only as a frontend presentation rule. Native persistence
trusted the earlier ask-time check, leaving a race between that check and
draft persistence, preview creation, or approval.

## Why the issue escaped detection

The UI fixture verified that a newer event visually staled the draft. Existing
adapter tests exercised knowledge citation freshness and payload approval, but
did not advance the transcript cursor between draft creation, preview, and
approval.

## Remediation and prevention

Native draft persistence, local preview creation, and approval now re-read the
durable transcript high-water mark and compare it to the run's stored cursor.
Each write uses the Genesis expected-frontier commit to reject a transcript
commit racing after that read. A regression verifies both matching and stale
cursor outcomes, and the runtime regression verifies a committed cursor
invalidates in-memory drafts and previews.

## Verification

The cursor-guard regression passed. The consolidated native suite passed 577
tests with 1 ignored, and the `meeting_knowledge` integration suite passed
17/17. All 32 registered Node suites, the TypeScript/Vite build, Rust
formatting, all-target check/clippy, and native build passed. The first
restricted-sandbox AppContainer attempt failed to create a per-user profile
(`0x80070002`); the complete native campaign passed when rerun with host
profile access.
