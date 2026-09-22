---
version: "0.2.2b"
created_at: "2026-09-21T01:28:12+07:00,RWANG"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "architecture-adaptation"
  scope: "FUNG Desktop meeting transcript pipeline"
  language: "Thai"
  source_reference: "F:/lalin/docs/architecture/MEETING_TRANSCRIPT_PIPELINE.md"
---

# FUNG — Meeting Transcript Pipeline Adaptation (proposal)

## 1. Document boundary

`F:/lalin/docs/architecture/MEETING_TRANSCRIPT_PIPELINE.md` is treated as a
reference architecture and experiment record. Its prose, paths, model choices,
and decisions are not instructions for FUNG. Only the user's request to adapt
the useful pipeline behavior is in scope here.

This proposal maps that pipeline onto FUNG's existing Tauri/Rust,
GenesisBlockDB, stateful-job, local-runtime, transcript-review, and export
contracts. It does not copy LALIN's `apps/api` paths or create a second audio
pipeline.

## 2. Classification and risk

| Field | Decision |
|---|---|
| Complexity | **C-3 — Architecture-driven adaptation** |
| Risk | **HIGH** — crosses ingestion, worker profiles, diarization, provenance, summary, review, and export |
| Parent architecture | `docs/Desktop/ARCHITECTURE.md` |
| FUNG meeting contract | `docs/Desktop/07-meeting-mode.md` |
| Existing approved ingestion contract | `docs/specs/2026-08-05-zoom-meeting-ingestion-design.md` |
| Existing implementation seams | `src-tauri/src/zoom_sync.rs`, `src-tauri/src/local_diarization.rs`, `src-tauri/src/meeting_intel.rs` |
| Approval state | Approved by user; Slice B implemented locally, remaining slices stay open |

## 2.1 Candidate live/agent companion contracts — 2026-09-21

The existing approved Slice B and its implementation evidence are unchanged. New requested behavior is specified separately:

- [Live transcription](2026-09-21-live-meeting-transcription-spec.md): incremental revisions, replay and speaker-source mapping.
- [People/Voice domains](2026-09-21-speaker-identity-domain-design.md): optional recognition, reviewed Person links.
- [Knowledge evidence](2026-09-21-meeting-knowledge-evidence-spec.md): selected corpus, citations and metric context.
- [Meeting Agent](2026-09-21-meeting-agent-participation-spec.md): explicit participation and bounded same-channel publication.
- [Google Meet API strategy](../decisions/2026-09-21-google-meet-agent-api-strategy.md): participant-attributed audio first; pyannote for mixed/shared-mic gaps; managed-bot vs official-media capabilities.

These are **candidate additions**, not approval inherited from Slice B. The local-only/non-cloud constraints below still describe this adaptation's original lane. Managed meeting media, knowledge sharing and external publication require the new independent grants/deployment gates; no ring/glow detector or silent identity inference is adopted.

## 3. Target FUNG pipeline

```text
FUNG recording/import
    │  project-owned audio + Genesis custody/provenance
    ├──────────────► ASR: faster-whisper profile
    │                   large-v3-turbo default
    │                   medium low-resource fallback
    │                   large-v3 qualification-only reference
    │
    ├──────────────► speaker attribution
    │                   Path A: per-participant tracks / capture provenance
    │                   Path B: mixed or far-side audio + pyannote 3.1
    │
    └──────────────► evidence-bound review
                            proposed speaker turns → user edits/confirmation
                            optional local summary/intent with segment refs
                            SRT/VTT/Markdown/audio export
```

The pipeline remains local-first and resumable. A missing optional diarization
runtime must never delete or block a usable transcript.

## 4. Mapping from the reference pipeline to FUNG

| Reference stage | FUNG adaptation | Current FUNG seam | Boundary |
|---|---|---|---|
| 1. Ingest | Use a `Recording` and its project-owned `canonical_audio_path`/`audio_chunks`; preserve hashes, chunk timing, and Genesis rows. Accept imported audio/video only through the existing custody path. | `lib.rs`, `live_meeting.rs`, `zoom_sync.rs`, `audio_chunks` | No `apps/api/runtime`; no real meeting media in Git. |
| 2. ASR | Reuse `run_transcription` and the staged Whisper runtime. Keep `large-v3-turbo` as default, `medium` as low-resource/CPU option, and `large-v3` as qualification-only. Language is a recording/profile input; the reference experiment's forced `th` is not a global FUNG rule. | `scripts/transcribe.py`, `lib.rs`, transcription profile contract | `initial_prompt`/glossary is a future contract change (D15), not silently added to `AsrInput`. |
| 3. Diarization | Reuse `scripts/diarize.py`, `run_diarization`, and the optional staged `pyannote/speaker-diarization-3.1` runtime/cache. | `diarization.rs`, `local_diarization.rs`, `zoom_sync.rs` | Anonymous labels or proposed metadata only; no biometric identity claim. |
| 4a. Meet ring gate | Keep as an optional video-evidence adapter, not part of the FUNG audio core. FUNG has no approved Meet tile/frame custody contract that would justify importing the pixel rule. | None in current FUNG core | Do not infer a person's name from a glow or from an untracked video frame. |
| 4b. Fuse | Use time-overlap attribution and capture provenance. Per-participant files and the microphone channel remain stronger facts than a diarizer guess. | `speaker_merge.rs`, `local_diarization.rs` | Preserve overlap and uncertainty; do not clear a certain channel label because diarization heard no turn. |
| 5. Narrative agent | Map the evidence-bound part to existing `summary.generate`, `meeting_ask`, and `meeting_intel` output. Any future transcript correction pass must be a separate, auditable local job that produces a diff and never overwrites raw ASR text. | `meeting_intel.rs`, `summaries`, `model_runs` | No hard-coded `qwen3.5:9b`; provider/model stays BYOM and must be recorded. |
| 6. Review/export | Use the existing recording-scoped review, manual correction/audit, speaker review, and `export.render` paths. Export SRT/VTT and existing local summary/audio artifacts; add a JSON evidence/diff export only as a separately approved slice. | `RecordingReview.tsx`, `transcript_export.rs`, `meeting_intel.rs` | Review remains human-confirmed; proposed speaker turns are not final identity. |

## 5. FUNG execution paths

### 5.1 Imported Zoom recording

1. `zoom.import` downloads project-owned mixed audio/video and any separate
   participant tracks.
2. Separate tracks use faster-whisper per file and `speaker_merge` Path A;
   Zoom display names remain proposed metadata.
3. Mixed-only recordings use faster-whisper, then pyannote Path B.
4. The transcript is persisted even when pyannote is unavailable. The job event
   records the blocker without turning a valid transcript into a failed import.
5. Structural graph build and the existing meeting-intelligence jobs remain
   downstream, with evidence references back to the same transcript rows.

### 5.2 Local Live Meeting recording

1. Capture keeps writing durable mic/system chunks before any model sees them.
2. The microphone channel keeps its capture-provenance speaker label.
3. `speakers.diarize` runs only on the far-side/system channel, projects turns
   back across real chunk timestamps, and updates existing segment IDs in
   place.
4. Missing or lossy chunks cause the projection to decline rather than create
   confidently wrong speaker times.
5. Summary and export consume the recording-scoped transcript after the user
   reviews the proposed speaker lanes.

### 5.3 Manual local import

The first adaptation reuses the existing import/transcribe route. A future
general `meeting.pipeline` orchestration job may compose the same handlers, but
it must not introduce a second persistence or job vocabulary before the current
recording-scoped contracts are proven.

## 6. Provenance and evidence contract

The adapted pipeline must keep these identities stable:

| Artifact | FUNG record | Required provenance |
|---|---|---|
| Source audio/chunks | `recordings`, `audio_chunks` | recording id, project id, path/hash, timing/custody |
| ASR segments | `transcript_segments` | recording id, model/profile, language, confidence |
| Speaker labels | `speakers` | project scope, proposed/edited display label, no biometric claim |
| Diarization turns | `speaker_turns` | time range, overlap, status, `model_run_id` |
| Model execution | `model_runs` / `model_providers` | provider, model, task kind, runtime location, input/output refs |
| Summary/intent | `summaries` | recording-scoped evidence segment ids and model run |
| Export | `export_artifacts` | local path, kind, source recording/layer |

The raw transcript remains the source of truth. A narrative or correction pass
may propose `keep`, `fix`, or `flag` records only after an approved data
contract exists; it may not silently replace text, speaker ids, timestamps, or
evidence references.

## 7. Context pack and Thai-language policy

The reference pipeline's context pack is useful, but FUNG does not currently
have an approved persisted context-pack entity or an `initial_prompt` field in
the transcription contract. The adaptation therefore uses this sequence:

1. Keep the current ASR output and profile behavior unchanged.
2. Define an optional local context-pack contract containing a content hash,
   source path/identity, language, and scope before passing any text to a
   worker or LLM.
3. Decide separately whether the pack can influence ASR decoding (`D15`) or
   only the post-ASR summary/correction pass.
4. Keep `th` as an explicit per-recording/profile choice for Thai meetings;
   do not force it for all FUNG recordings because FUNG's contract supports
   both Thai and English.

## 8. Delivery slices after approval

| Slice | Deliverable | Proof required |
|---|---|---|
| A | Adapted docs and source-to-FUNG contract map | parent/peer doc review; no code drift |
| B | Canonical pipeline orchestration over existing jobs/handlers, if still needed after the map | idempotent job tests and recovery tests |
| C | Context-pack and evidence-diff contract | schema/bridge tests; raw transcript immutability test |
| D | Optional video evidence adapter | fixture video proof; explicit privacy/custody review |
| E | End-to-end fixture and real local meeting qualification | mixed-audio fixture, model provenance, review/export evidence; real-device/runtime gates reported separately |

The first implementation slice should be the smallest missing FUNG contract,
not a wholesale copy of the six-stage LALIN scripts.

### Slice B — local-capture stage order (implemented)

The local-capture stop path now queues the existing `speakers.diarize` job
before `summary.generate`, using the existing job vocabulary and one serial
worker. This gives a successful speaker pass a chance to enrich the transcript
before summary evidence is read. If diarization cannot be queued or later
declines because its optional runtime/model/audio is unavailable, the summary
still queues and runs against the unchanged transcript; the blocker remains
visible as its own job/event. The existing manual summary-retry command queues
only `summary.generate`, so it does not rerun a completed speaker pass.

No new persistence table, job type, video detector, identity claim, or raw
transcript rewrite was introduced in this slice.

## 9. Acceptance criteria for the adaptation

- One FUNG recording remains the sole scope for ASR, diarization, summary, and
  export; no cross-recording evidence leakage.
- Path A and Path B both converge on the existing `speakers`,
  `transcript_segments`, `speaker_turns`, and `model_runs` records.
- A diarization failure leaves the transcript readable and records a truthful
  blocker/event.
- Capture-provenance labels are never overruled by anonymous diarization.
- Proposed speaker labels remain editable and distinguish proposed inference
  from user-confirmed naming.
- Summary/intent outputs cite real transcript segment ids and are not presented
  as verified facts when evidence is insufficient.
- Retry/restart does not duplicate transcript rows, speaker turns, summaries,
  or export artifacts.
- All media, context text, model cache, and intermediate files remain local;
  no token, raw meeting media, or context content enters source control or
  logs.
- Static/unit/fixture/runtime/real-meeting evidence is reported separately;
  no local probe is described as production or real-meeting acceptance.

## 10. Explicit non-goals in this proposal

- Implementing the Google Meet ring/glow detector in FUNG without a video
  custody and privacy contract.
- Adding biometric face/voice identity or claiming that `Speaker 1` is a named
  person automatically.
- Hard-coding LALIN's `qwen3.5:9b`, CUDA version, or exact benchmark timings
  into FUNG's provider contract.
- Moving diarization, video gating, or narrative correction into the live
  capture worker.
- Adding cloud upload, cross-meeting entity resolution, or remote transcript
  retrieval.

## Version Diff

| Version | Change |
|---|---|
| 0.2.2b | Cross-linked candidate live transcript, knowledge and Meet-agent companions without changing the approved local Slice B scope. |
| 0.2.1b | Clarified that only a new local-capture stop queues diarization; manual summary retry remains summary-only. |
| 0.2.0b | Implemented the approved local-capture order: optional speaker diarization is queued before the existing recording-scoped summary job, while diarization failure remains non-blocking. |
| 0.1.0d | Initial candidate adaptation of the LALIN meeting pipeline onto FUNG's existing recording, job, provenance, diarization, summary, and export contracts. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.2b | 2026-09-21 | beta | Cross-linked candidate live transcript, knowledge and Meet-agent companions without changing the approved local Slice B scope. | working-tree | RWANG |
| 0.2.1b | 2026-09-21 | beta | Clarified the local-capture-only diarization enqueue and preserved summary-only manual retry behavior. | working-tree | RWANG |
| 0.2.0b | 2026-09-21 | beta | Implemented the approved local-capture stage order using existing jobs; optional diarization remains non-blocking and no new job vocabulary was added. | working-tree | RWANG |
| 0.1.0d | 2026-09-21 | candidate | Proposed FUNG adaptation; no implementation performed from this proposal. | working-tree | RWANG |
