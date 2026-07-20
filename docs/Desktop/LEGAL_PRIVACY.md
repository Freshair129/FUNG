---
version: "0.1.0b"
created_at: "2026-07-05T00:00:00+07:00,ATHER"
last_update: "2026-07-05T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "privacy-legal"
  scope: "FUNG"
---

# Legal and Privacy Model

## Core Principle

FUNG must be local-first by default. Audio, transcript, speaker labels, summaries, and intent analysis remain on the user's machine unless the user explicitly enables an external provider.

## BYOM Policy

BYOM is the default model strategy.

Allowed provider types:

- Local process.
- Local network endpoint.
- OpenAI-compatible endpoint explicitly configured by the user.
- Cloud endpoint only after opt-in and clear warning.

The app must show:

- Provider name.
- Runtime location: local, LAN, or cloud.
- Data sent to provider.
- Retention warning if provider is external.

## Consent and Recording Notice

The product must not imply that recording is legal in every context.

Required UX:

- New recording flow must remind user to follow applicable consent laws.
- Exported reports should optionally include a recording-consent note.
- User is responsible for confirming permission to record.

## Speaker Handling

Rules:

- Default labels must be generic: Speaker 1, Speaker 2.
- User may rename speakers.
- AI must not claim verified identity from voice alone.
- Diarization confidence should be visible when available.

## Intent Analysis Handling

Intent analysis is sensitive because it infers internal state.

Rules:

- Always label as inferred.
- Include timestamp evidence.
- Include uncertainty.
- Avoid presenting intent as fact.
- Avoid protected-class, medical, legal, or financial conclusions.

Example phrasing:

```text
Likely intent: seeking clarification.
Evidence: 00:13:20-00:13:48.
Confidence: medium.
Note: This is an AI inference, not a verified fact.
```

## Local Data Storage

Data stored locally:

- Project metadata.
- Audio chunks.
- Derived layers.
- Transcript.
- Speaker labels.
- Model run metadata.
- Summaries and intent inferences.
- Export artifacts.
- Audit events.

Sensitive fields should be minimized and never sent externally by default.

## Audit Trail

Each AI-generated artifact must record:

- Source artifact.
- Model provider.
- Model name.
- Runtime location.
- Parameters where practical.
- Created timestamp.
- User edits after generation.

## Risk Controls

| Risk | Control |
| --- | --- |
| Accidental cloud upload | Local-only default and explicit provider warnings |
| Misuse of intent inference | Evidence, uncertainty, and inference labels |
| Speaker identity confusion | Generic default labels and editable speakers |
| Data loss | SQLite WAL, chunked files, recovery scan |
| Legal misunderstanding | Consent reminders and export notes |

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Initial legal/privacy model for local-first recording, BYOM, diarization, and intent inference. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-05 | beta | Initial legal and privacy spec. | N/A | ATHER |
