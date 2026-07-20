---
version: "0.1.0b"
created_at: "2026-07-09T15:25:00+07:00,ATHER"
last_update: "2026-07-09T15:25:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "feature-driver-spec"
  scope: "FUNG"
---

# 07 - Meeting Mode

## Purpose

Meeting Mode is the first feature driver for the FUNG command deck. It defines what the fixed HUD should show when the user's main job is recording a meeting, reviewing who said what, and exporting a reliable recap.

This document does not change the shell layout. It only defines what content belongs inside the existing zones for a meeting-driven workflow.

## Risk Level

Change class: MEDIUM.

Reason:

- The shell layout stays fixed.
- Content hierarchy changes across P1-P4.
- The spec affects navigation, labels, and interaction priority across multiple zones.

## User Job

The user wants to:

1. Start a long meeting capture safely.
2. See whether recording is healthy without scanning the whole screen.
3. Turn the meeting into transcript plus speaker structure.
4. Extract recap, decisions, action items, and per-person intent.
5. Export evidence-backed artifacts without cloud-first behavior.

## Meeting Workflow Spine

```text
Prepare meeting -> Record meeting -> Review transcript -> Confirm speakers
-> Summarize and infer intent -> Export bundle
```

This workflow must be visible across the four P pages as one continuous story.

## Feature Driver Rules

- The content must prioritize the current step of the meeting workflow.
- Each P page gets one primary action, two secondary actions, and one dominant status.
- The center workbench must show only the three most frequent tasks for that stage.
- Agent card copy must answer "what should I do next?" for the active meeting.
- Sector C must show proof and recent events, not decorative system detail.
- Signal cards must summarize risk and readiness for meeting work.

## P1 - Meeting Capture

### Goal

Start and sustain a meeting recording safely.

### Center Workbench

| Tile | Purpose | Primary CTA |
| --- | --- | --- |
| Meeting setup | meeting title, source input, mic mode, recording preset | Start recording |
| Live capture | elapsed time, chunk seal cadence, clipping/noise watch | Pause or resume |
| Side notes | quick marker, agenda cue, important moment flag | Add marker |

### Agent Card

- Shows current meeting name.
- Shows input health meter.
- Shows storage safety: chunking, autosave, WAL write health.
- Secondary actions: change input, test mic, recover interrupted take.

### Sector C

Activity:

- recording started
- chunk sealed
- marker added
- device changed

Events:

- clipping warning
- silence stretch
- low disk risk
- recovery available

### Signals

| Card | Meeting meaning |
| --- | --- |
| D | Capture safety |
| E | Audio cleanliness |
| F | Note density |
| G | Session readiness for transcript |

## P2 - Transcript Review

### Goal

Turn the meeting audio into readable, correct, speaker-aware text.

### Center Workbench

| Tile | Purpose | Primary CTA |
| --- | --- | --- |
| Transcript pass | open latest transcript, read with timestamps | Review transcript |
| Speaker pass | merge, rename, or split speaker labels | Lock speakers |
| Highlight pass | mark decisions, questions, and useful quotes | Mark evidence |

### Agent Card

- Shows transcript readiness score.
- Shows current speaker map state.
- Shows pending corrections and unresolved uncertainty.
- Secondary actions: rerun transcript, rerun diarization, open layer preview.

### Sector C

Activity:

- transcript generated
- speaker labels edited
- quote marked

Events:

- low confidence span
- overlapping speakers
- untranslated phrase

### Signals

| Card | Meeting meaning |
| --- | --- |
| D | Transcript completeness |
| E | Speaker confidence |
| F | Evidence coverage |
| G | Ready for summary |

## P3 - Meeting Intelligence

### Goal

Produce a meeting recap that preserves evidence and separates fact from inference.

### Center Workbench

| Tile | Purpose | Primary CTA |
| --- | --- | --- |
| Meeting recap | full story, timeline, outcomes | Generate recap |
| People and intent | per-person goals, concerns, commitments, uncertainty | Analyze intent |
| Action register | next steps, owners, deadlines, follow-ups | Extract actions |

### Agent Card

- Shows whether transcript proof is sufficient.
- Shows current summary provider/model provenance.
- Shows confidence framing for intent output.
- Secondary actions: regenerate with another provider, compare summaries, pin evidence spans.

### Sector C

Activity:

- recap generated
- intent refreshed
- actions extracted

Events:

- weak evidence cluster
- conflicting speaker intent
- low confidence output

### Signals

| Card | Meeting meaning |
| --- | --- |
| D | Recap completeness |
| E | Intent confidence |
| F | Action extraction status |
| G | Export bundle readiness |

## P4 - Meeting Export and Governance

### Goal

Export a private, evidence-backed meeting package and confirm runtime/legal posture.

### Center Workbench

| Tile | Purpose | Primary CTA |
| --- | --- | --- |
| Export bundle | wav, mp3, txt, srt, vtt, json, md recap | Export bundle |
| Privacy and policy | local-only mode, provider policy, provenance | Review policy |
| Runtime and archive | model health, storage path, project retention | Archive project |

### Agent Card

- Shows BYOM runtime health.
- Shows privacy mode and export queue.
- Shows provenance completeness before export.
- Secondary actions: open export folder, verify local API, inspect job history.

### Sector C

Activity:

- export queued
- export completed
- archive created

Events:

- missing provenance
- runtime offline
- policy mismatch

### Signals

| Card | Meeting meaning |
| --- | --- |
| D | Export queue |
| E | Privacy mode |
| F | Runtime health |
| G | Provenance integrity |

## Cross-Zone Interaction Rules

- Clicking a focus tile updates the Agent card and Sector C to the same subtask.
- Signals never navigate away from the active P; they filter or emphasize the current meeting state.
- Topbar segmented control mirrors P1-P4 only; it must not add extra route depth.
- The meeting title is persistent in the score header across all P pages.
- The user must always see one clear primary action in the current page.

## Why This Layout Is Simpler

The meeting workflow collapses many low-level features into a small number of work surfaces:

- recording instead of generic capture options
- transcript pass instead of dense project tooling
- recap plus action extraction instead of many AI cards
- export plus policy instead of broad runtime clutter

This keeps advanced functions available, but outside the first decision layer.

## Acceptance Criteria

- Meeting Mode fits entirely inside the current HUD shell and zone map.
- Each P page exposes exactly 3 center tiles for first-view usage.
- Agent card always reflects the active meeting subtask.
- Sector C always shows evidence or operational events relevant to the current meeting step.
- Signals keep fixed positions and use meeting-specific copy.
- The meeting workflow remains understandable without opening extra panels.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Added feature-driver content spec for Meeting Mode across P1-P4 inside the fixed HUD layout. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-09 | beta | Added Meeting Mode feature-driver content spec. | N/A | ATHER |
