---
version: "0.1.0b"
created_at: "2026-09-21T03:36:16+07:00,RWANG,base-b336f33"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "live-transcription"
  doc_type: "feature-spec"
  scope: "FUNG Desktop live meeting transcript and API-attributed speakers"
  language: "Thai"
---

# FUNG — Live Meeting Transcription Specification

## 1. Outcome and authority

ผู้ใช้เห็นข้อความระหว่างประชุม มีเวลาและผู้พูด แยก “กำลังถอด” กับ “บันทึกแล้ว” ได้ และย้อนดูหลัง reconnect/crash โดยไม่มีประโยคซ้ำหรือข้อความหายเงียบ ๆ Meeting Agent ใช้เฉพาะ revision ที่ commit แล้วและผ่าน freshness/quality gate

C-3 / HIGH — เปลี่ยน event contract, persistence, scheduling และ input provider. เอกสารนี้เป็น candidate; ไม่ได้แก้ runtime, migration, flags หรือ default model ในรอบเขียนเอกสาร

Parents: [Domain map](../architecture/MEETING_INTELLIGENCE_DOMAINS.md), [Audio pipeline](../Desktop/AUDIO_AI_PIPELINE.md).
Peers: [Agent](2026-09-21-meeting-agent-participation-spec.md), [Google Meet decision](../decisions/2026-09-21-google-meet-agent-api-strategy.md), [Speaker identity](2026-08-23-speaker-identification-and-voice-profile-spec.md).

## 2. Current source vs target

| Evidence checked 2026-09-21 | Current behavior | Target gap |
| --- | --- | --- |
| `src-tauri/src/live_meeting.rs`: CHUNK_MS=8_000 | persistent Whisper worker receives chunks; UI gets completed chunk segments | not token streaming; no proven sub-second latency |
| `LiveSegmentEvent` | recordingId, segmentId, channel, speaker, startMs/endMs, text, confidence | no utterance revision/cursor/provisional contract |
| `persist_and_emit_segments` | persists transcript; labels mic=เรา, system=อีกฝ่าย | channel labels do not identify all remote people |
| `scripts/transcribe_live.py` | faster-whisper, VAD, no word timestamps | low-latency incremental windows and alignment need qualification |
| `LiveMeetingPanel.tsx` | recording-scoped subscriptions and deduplicated bounded display | target needs durable replay cursor and revision-aware updates |

Existing 8-second mode remains usable and is labelled chunked-live. None of the latency/revision/provider requirements below is claimed implemented.

## 3. Scope and input modes

| Mode | Audio / identity origin | Boundary |
| --- | --- | --- |
| Local capture | existing microphone + system channel | unknown people on mixed channels; pyannote optional |
| Google Meet API | participant-attributed frames through an approved adapter | provider participant != verified real person |
| Provider transcript | optional external transcript with provider provenance | separate consent/quality profile; not secretly enabled instead of local Whisper |
| Imported recording | post-meeting jobs | reuses final transcript model, not counted as live completion |

One selected primary source per logical audio path. Simultaneous local/Meet capture is allowed only as explicit independent evidence, with dedup policy; never concatenate both and double the meeting. Video capture, caption scraping, translation and concurrent multi-user transcript editing are outside MVP.

## 4. Functional requirements

| ID | Required behavior | Acceptance |
| --- | --- | --- |
| LT-01 | Preflight source, device/API permission, consent, disk, model/profile and current source capability | Missing requirement yields named blocker; no fake “recording” |
| LT-02 | Start/stop/pause local capture and API subscription through explicit session state | Pause semantics say whether audio capture, transcription or both stopped |
| LT-03 | Render provisional text in a replaceable utterance card; commit only after source audio custody and canonical write succeed | UI style and event state differ; partial never exported as final |
| LT-04 | Record monotonic per-utterance revision and stable IDs; content edit creates a new revision | duplicate/out-of-order messages cannot append duplicate sentences |
| LT-05 | Carry track/participant session attribution independently from text | name change updates label evidence, not audio source history |
| LT-06 | Support unknown, overlap and shared-room participants | never force one named person onto ambiguous mixed speech |
| LT-07 | Reconnect using committed event cursor and snapshot recovery | missed final events replay; cursor expiry requires snapshot, not empty success |
| LT-08 | Source gaps, codec errors, network loss, model lag and disk failure are explicit intervals/states | missing audio never labelled silence |
| LT-09 | Capture survives transcription/agent/model failure when local storage remains healthy | pending audio is replayable; source outage shown separately |
| LT-10 | Stop seals final audio, processes bounded tail and exposes pending catch-up | recording stopped != transcript complete |
| LT-11 | Thai, English and code-switch test corpus; user language auto/override is recorded | no global forced Thai or fabricated confidence |
| LT-12 | Manual correction pins reviewed text; late ASR/refinement cannot overwrite it | stale worker result rejected with expectedRevision |
| LT-13 | Share model weights/cache and schedule fairly across active speakers | no per-participant model download or unbounded workers |
| LT-14 | Emit committed revision/freshness events to Agent; exclude self-generated output | partial/corrected-away/self-echo cannot trigger public facts |
| LT-15 | Retain raw ASR alongside effective reviewed projection and provenance | timestamp exports reproducible from named snapshot |
| LT-16 | Local-only mode produces no new media/transcript network egress | API route requires explicit online mode and independent grant |

## 5. States and durability semantics

Session: `idle → preflight → starting → live ↔ paused → stopping → stopped`.
Health is separate: `healthy | catching_up | degraded | unavailable`; a single enum must not hide “audio safe, transcript delayed”.

Utterance: `provisional → committed → superseded`; manual correction creates another committed revision with `origin=human`. “Committed” means durably stored, **not** human-verified or semantically true. A provisional can be `discarded` (noise/overlap/resegmentation) with an explicit remove event.

For the proposed low-latency path:

1. Receive audio with source sequence and clock mapping; validate dimensions/rate/duration.
2. Keep bounded per-source VAD/context buffers. Trial defaults: 1-second hypothesis cadence, 4-second decode window, up to 1-second overlap; all versioned and benchmark-adjustable.
3. Seal durable source fragments at no more than 2-second intervals or utterance boundary. This is a proposed change from the current 8-second chunks; do not claim the old path meets new latency targets.
4. A final candidate must cite complete durable audio coverage. Decoder completion/endpointing selects a commit; repeated same text alone does not prove correct attribution.
5. In one serialized mutation boundary, write raw text revision + canonical projection + committed event + processed input cursor. Emit after commit only. If Genesis cannot atomically apply this set, use a recoverable write-intent journal and idempotent reconciliation; split unguarded upserts are not acceptable.
6. Provisional updates stay in memory and can vanish after a crash. Committed text and source coverage survive. Legacy consumers receive committed v1 projection only.

Every ASR run records model ID/revision, compute type, runtime build, hardware, language, VAD/window/preprocessing config, source hashes and timing. Explicitly retain `confidence=null` when not supplied/calibrated.

## 6. Proposed event contract

Transport name: `live-transcript-v2`; schemaVersion=2. Existing `live-segment` remains a backward-compatible committed-only adapter during migration.

| Field | Required meaning |
| --- | --- |
| schemaVersion, eventId, eventType | version; unique ID; upsert/discard/gap/status |
| projectId, recordingId, meetingSessionId | validated owned scope |
| sourceSessionId, trackId, sourceGeneration | source/rejoin continuity boundary |
| utteranceId, revision, supersedesRevision | stable logical utterance, monotonic revision, optional parent |
| state, origin | provisional/committed/superseded; local_asr/provider_asr/human/refinement |
| startMs, endMs | recording-relative monotonic media time; 0 <= start < end |
| text, language, confidence | bounded text; nullable confidence, not identity certainty |
| attribution | kind, participantSessionId or speakerClusterId, labelSnapshot, evidenceRevision |
| audioRefs, modelRunId | durable source spans/run; required on local committed ASR |
| committedCursor | monotonic recording event-log cursor; absent for provisional |
| emittedAt, receivedAt, sourceClockUncertaintyMs | diagnostics; wall time is not segment ordering |
| reviewState, qualityFlags | unreviewed/reviewed; overlap/low_quality/missing_mapping/etc. |

An utterance crossing participant/source generation must split; the old speaker label cannot bleed across track-slot reuse. Re-segmentation emits tombstones and replacement IDs as a committed revision group, preventing two visible copies of overlapping text.

## 7. Persistence and commands

All tables are proposals under Genesis, not current schema additions.

| Aggregate | Minimum fields / invariant |
| --- | --- |
| meeting_sessions | recording, provider occurrence, source mode, state, owner, policy version |
| meeting_participant_sessions | provider participant/session, join generation, label snapshots, source kind, optional person link |
| transcript_revisions | utterance, revision, raw/effective text refs, source range, origin, attribution revision, model run, parent revision |
| transcript_event_log | recording cursor, unique event ID, mutation group, committed revision refs, event time |
| source coverage | existing audio_chunks/custody extended with track, generation, seq/time coverage and explicit gaps |

Use unique identity/idempotency checks under a serialized per-recording writer. Never deduplicate by text alone: identical repeated speech may be genuine.

Proposed native command boundary:

| Command | Input | Result / rejection |
| --- | --- | --- |
| live_transcript_snapshot | owned recording, page size, optional cursor | current committed projection + highWatermarkCursor |
| live_transcript_replay | recording, afterCursor, bounded page | ordered committed events or CURSOR_EXPIRED |
| live_transcript_set_profile | session, target profile, expectedRevision | scheduled safe-boundary switch or PROFILE_UNAVAILABLE |
| live_transcript_correct | utterance, expectedRevision, replacement | new reviewed revision or REVISION_CONFLICT |
| live_transcript_catch_up | recording, missing coverage range | durable job ID; no duplicated covered revisions |

Actor comes from trusted session, not UI fields. Start/stop integration should extend existing lifecycle commands where safe rather than create competing capture controllers.

Snapshot contract: subscribe/buffer → read snapshot with cursor → replay strictly after cursor → apply buffered newer events → follow stream. Server cap and pagination prevent lost events during snapshot/subscription races.

## 8. Speaker attribution policy

Priority is about **source evidence**, not certainty of a human's identity:

1. Exact API participant-session/track mapping → label “ชื่อจาก Meet”.
2. Imported participant track → label “ชื่อจากแหล่งไฟล์”.
3. Local capture channel → “ไมค์เครื่องนี้/เสียงระบบ”.
4. Diarization cluster → “ผู้พูด 1/2”; optional identity proposal.
5. Confirmed D6 person overlay → human-reviewed name, preserving original source.

Google Meet virtual streams may change contributing participant; mapping is per packet/span/generation, never cached forever against stream slot. Managed APIs have their own participant IDs and source limits; normalize without inventing cross-provider equality.

For room devices/shared mics, retain a parent participant endpoint and anonymous child speakers. Unknown mapping buffers briefly (candidate cap 2 seconds), then commits unattributed speech; delayed metadata may add an attribution revision without rewriting text. Never use the “active speaker” UI highlight as proof of a whole utterance.

## 9. Runtime budgets and performance targets

These are **proposed acceptance targets**, not benchmarks or promises.

| Metric / scenario | Candidate target and measurement |
| --- | --- |
| Primary profile | large-v3-turbo on qualified GPU; first provisional p95 <=3s from first voiced frame received |
| Committed latency | primary p95 <=6s from utterance end at source to durable UI commit; report source clock uncertainty/network separately |
| Low-resource profile | medium / CPU int8; p95 <=15s committed on qualified hardware or explicit “delayed” mode |
| UI interaction | transcript controls p95 <=250ms; bounded last 200 utterances, history paginated |
| Backlog | warn >15s; suspend proactive publication >30s; no unbounded memory |
| Capture safety | 3-hour soak; zero unexplained loss in accepted source coverage; memory bounded after warm-up |
| Concurrency | test 2/4/8 participant sources and overlapping speech; do not infer capacity from number of connected attendees |

One shared runtime/model cache; bounded inference pool. Live ASR gets priority over diarization, enrollment, graph and summary. On GPU OOM, preserve capture, report failure, unload or switch only to a preauthorized staged profile; record boundary/provenance. Do not auto-download medium or spawn a model per participant. ASR may serialize active turns if needed; backlog stays visible.

## 10. Stop, failure and recovery

| Event | Required response |
| --- | --- |
| Decoder timeout/crash | isolate worker; preserve pending durable audio; bounded retry and explicit catch-up |
| Source reconnect | new generation + gap evidence; reconcile IDs, never overlap duplicate ranges silently |
| Provider unavailable | stop receiving remote audio, show real gap; local fallback only if user enabled it beforehand or explicitly starts it |
| Disk full | stop new persistence/capture safely with visible alert; do not promise retained audio that was not written |
| Stop with >30s tail | stop capture promptly; show catch-up job; user can cancel text processing without deleting audio |
| App restart | recover committed state and pending coverage; never auto-rejoin external meeting or regrant publication |
| Late result after correction/revoke | reject stale write; retain safe run outcome without restoring revoked content |
| Participant leaves/renames/rejoins | preserve session history; do not merge solely by display name |

Published Agent outputs are not silently rewritten by transcript correction; D13 handles correction notices with independent authority.

## 11. UX and observability

Live surface has source badge, capture health, transcript lag, provisional styling, speaker-source badge, follow-live toggle, pause-follow while reading, keyboard navigation, timestamps and gap markers. Screen reader receives throttled committed announcements, not every partial token. No mandatory People/voice enrollment wizard to transcribe.

Metrics: input coverage, lost/unmapped durations, inference RTF, pending seconds, provisional churn, commit latency p50/p95/p99, revision conflicts, duplicates rejected, replay gaps, worker restarts, model profile. Logs contain IDs/counts/error classes, not audio/text/embeddings/OAuth tokens.

## 12. Qualification matrix and Definition of Done

| Test | Required proof |
| --- | --- |
| LT-T01 normal Thai/English/code-switch | timestamps/raw text/model provenance; WER/CER reported with fixed corpus, no invented score |
| LT-T02 duplicates/out-of-order/resegment | one current projection; repeated genuine phrase retained |
| LT-T03 crash between audio/write/emit | no lost committed text; pending audio recoverable; emission never ahead of commit |
| LT-T04 reconnect during snapshot | full coverage after cursor; no silent missing final event |
| LT-T05 overlapping remote people + shared room | accurate source labels or explicit unknown; no named-person overclaim |
| LT-T06 manual correction vs late worker | human revision preserved; stale agent draft invalidated |
| LT-T07 3-hour CPU/GPU soak | measured latency/RTF/memory/disk and profile/degraded status |
| LT-T08 model/API/agent failure | local capture unaffected where input exists; remote gaps honest |
| LT-T09 source switch/rejoin/renamed participant | generation isolation; no wrong-person carry-over |
| LT-T10 privacy/revoke | zero unauthorized egress; stopped consumers cannot commit new output |
| LT-T11 optional pyannote absent | live transcript still works; post-meeting diarization truthful unavailable |
| LT-T12 backend/frontend compatibility | v1 receives committed-only output; old recordings readable |

DoD: reviewed spec + migration plan, contract/unit/integration tests, hardware/real-room UAT, accessibility, privacy and rollback evidence. Documentation checks alone close none of these implementation gates.

## Version Diff

| Version | Change |
| --- | --- |
| 0.0.0 → 0.1.0b | Added live revision, durability, API speaker attribution, replay, resource budgets and qualification contract. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-09-21 | candidate | Detailed live-transcription requirements; no runtime changes or performance claims. | working-tree; base b336f33 | RWANG |
