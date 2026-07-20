---
version: "0.1.0b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-07-05T13:15:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "foundations"
  scope: "FUNG"
---

# 01 - Foundations

## Product Thesis

FUNG is a local-first AI native desktop app for people who need to record long conversations, preserve source audio, turn speech into useful text, understand who said what, and produce exportable evidence without making cloud upload the default.

The product starts on desktop because long recording, local files, model runtimes, SQLite WAL state, and legal/privacy boundaries are easiest to control on the user's own machine.

## Platform Priority

1. Desktop app first.
2. Web app second.
3. Mobile third.

Desktop is the source of truth for recording, local state, model execution, and exports. Web and mobile are future companion surfaces.

## Core Principles

| Principle | Meaning |
| --- | --- |
| Local-first | Source audio, transcript, derived artifacts, and job state live locally by default. |
| BYOM-first | Users can bring local models or compatible endpoints such as Ollama, ollama.cpp, and vLLM. |
| Evidence-based AI | Summaries and intent analysis must cite transcript time ranges and show uncertainty. |
| Durable capture | Long recordings must be chunked and recoverable after app failure. |
| Legal caution | AI may infer intent, but must not present legal conclusions as facts. |
| Desktop-native | Native filesystem, window, recording, and runtime control are first-class product capabilities. |

## In Scope - V1

- Long recording with chunked autosave.
- Local project library backed by SQLite WAL.
- Import audio and record from microphone.
- Transcription through local/BYOM providers.
- Speaker diarization with editable speaker labels.
- Noise reduction and speech enhancement.
- Audio layer view for original, cleaned, separated, and selected clips.
- Export to `.wav`, `.mp3`, transcript `.txt`, `.srt`, `.vtt`, and structured `.json`.
- Whole-recording summary.
- Speaker-level summary.
- Speaker intent analysis with evidence and uncertainty.
- Local API, MCP, and CLI surfaces for automation.

## Out of Scope - V1

- Mobile as the primary recorder.
- Cloud sync by default.
- Realtime multiplayer editing.
- Full DAW editing such as MIDI, plugin chains, or advanced mixing.
- Definitive biometric speaker identity.
- Legal advice or automatic legal conclusion.

## User Promise

FUNG should feel like a quiet professional instrument: calm, tactile, private, and reliable. The first screen must be a usable workspace, not a marketing landing page.

## Acceptance Criteria

| Area | Criteria |
| --- | --- |
| Recording | Can record for long sessions with chunked persistence and recover existing chunks after interruption. |
| Storage | Project and job state are persisted in SQLite WAL from the start. |
| AI | Transcription, summary, and intent outputs record provider/model provenance. |
| Diarization | Speaker labels are editable and never treated as verified identity by default. |
| Export | Users can export source or derived audio as `.wav` and `.mp3`. |
| Privacy | Cloud upload is disabled by default and requires explicit opt-in. |
| Automation | API, MCP, and CLI refer to the same local state model. |

## Risk Level

Overall change class: C-3 Architecture-Driven Implementation.

Reason: the app combines desktop runtime, long audio recording, local database durability, AI model orchestration, local API, MCP, CLI, and privacy-sensitive workflows.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Added product foundation and V1 boundaries. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-05 | beta | Added foundations doc. | N/A | ATHER |
