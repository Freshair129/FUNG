---
version: "0.1.1b"
created_at: "2026-08-26T00:00:00+07:00,Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67"
last_update: "2026-08-26T00:00:00+07:00,Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "technical-design"
  scope: "FUNG Desktop MVP critical path"
  language: "Thai"
---

# FUNG MVP Critical-Path Amendment

## สถานะและเจตนา

เอกสารนี้เป็น **candidate specification** สำหรับการทบทวนโดย Terra และการอนุมัติโดย Boss เท่านั้น ยังไม่มีการแก้ code, test, config, environment หรือ external state

Amendment นี้จัดลำดับ MVP ใหม่ตามผลลัพธ์ที่ผู้ใช้กำหนด:

`local audio input/capture → transcription → transcript review → Minute of Note → local export`

มันไม่ยกเลิก master plan แต่เลื่อนงาน pairing, FUNGWIRE, cloud และ connector ออกจาก critical path ของ local Desktop MVP จนกว่าจะมีหลักฐานว่าจำเป็นต่อ flow นี้

## Current repository truth

| ช่วงของ flow | สถานะจาก code | หลักฐาน/ข้อจำกัด |
|---|---|---|
| Local audio input/capture | Implemented; runtime/UAT-open | `src-tauri/src/live_meeting.rs` เปิดไมค์และ optional WASAPI loopback, ตัด WAV ทุก 8 วินาที, hash และ commit `audio_chunks` ก่อน transcription; real Live Meeting capture และ packaged interaction ยังไม่ผ่าน UAT ปัจจุบัน |
| Transcription | Implemented; locally prepared, runtime-open | `src-tauri/src/lib.rs` มี `import_and_transcribe`; `audio_custody` คัดลอกไฟล์เข้า project และ hash ก่อนใช้; `scripts/transcribe.py` รองรับ local faster-whisper และ progress; staged runtime/GPU smoke เป็น worker evidence ไม่ใช่ Live Meeting runtime proof |
| Transcript review | Partial | `list_transcript_segments` แสดง timestamps, confidence, speaker label และ cap warning; `LiveMeetingPanel` แสดง transcript สด; speaker rename มีอยู่ แต่ full transcript editor และ evidence marking ยังไม่ implemented |
| Minute of Note / summary | Implemented in code; runtime/UAT-open | `meeting_intel::summarize_and_export` สร้าง `whole_story`, `timeline`, `decisions_actions`, อ้างอิง transcript segments และเขียน Markdown; local LLM/Ollama execution และ summary/export review หลัง restart ยังเปิด |
| Local export | Partial/implemented by type | `transcript_export::render_subtitles` เขียน `.srt`/`.vtt` และ `export_artifacts`; summary path เขียน `.md`; audio WAV/MP3 export และ separate export queue ยังไม่ implemented; read ceiling 1000 rows ทำให้ long transcript ถูก refuse ไม่ใช่ truncate |

### จุดขัดข้องที่ต้องแก้ก่อน

`run_import_pipeline` สร้าง `recordings` และ `transcript_segments` แต่จาก source ที่ตรวจพบไม่ได้อัปเดต `projects.active_recording_id` ให้ชี้ไปยัง imported recording ขณะที่ `App.tsx` ใช้ค่านี้เป็น target ของ queued summary/export jobs จึงมีความเสี่ยงที่ import → transcript จะจบ แต่ผู้ใช้ไปต่อยัง Minute of Note/export ผ่าน shell ไม่ได้

นอกจากนี้ source เดียวกันสร้าง `audio_chunks` แบบ one-file โดยตั้ง `end_ms: 0` และใน success mutation ที่อ่านพบไม่ได้เติม `end_ms`/`transcribed_at` กลับลง chunk row ตาม comment ที่ระบุว่าจะเติม duration ภายหลัง เรื่องนี้ต้องยืนยันด้วย regression test ก่อนถือ imported recording ว่า review/retry/export-ready

## Proposed amendment

### D-MVP-01 — Close the local import handoff

**Goal:** ทำให้ imported local audio หนึ่งไฟล์เดินต่อจาก custody → transcription → recording-scoped review → Minute of Note → local export ได้โดยไม่พึ่ง login, cloud, pairing หรือ connector

**Smallest implementation slice:** แก้เฉพาะ handoff และ truth metadata ที่ทำให้ flow ปัจจุบันต่อกันไม่ได้

1. หลัง import transcription สำเร็จ ให้ project ชี้ `active_recording_id` ไปยัง recording ที่เพิ่งสร้าง โดยไม่เปลี่ยนชื่อ project ที่ผู้ใช้ตั้งไว้
2. บันทึก metadata ของ imported audio chunk ให้สอดคล้องกับผล transcription (`end_ms`, และสถานะการ transcribe) โดย preserve `file_path`, `checksum`, `byte_size`, `sequence_no` และ provenance เดิม
3. ทำให้การดึง transcript และ UI bridge ของ active recording เป็น recording-scoped แยกจาก transcript ของ recording อื่นใน project และคง `capped`/`cap`/`cappedRecordingIds` เป็น truth signal
4. ใช้ existing `summary.generate` เป็น Minute of Note pass เดียวที่สร้างสามมุมมอง (`whole_story`, `timeline`, `decisions_actions`) และ existing local `.md`/`.srt`/`.vtt` writers; ไม่สร้าง job type หรือ export format ใหม่ใน slice นี้
5. แสดง terminal failure และ next step เมื่อไม่มี transcript, local model ใช้ไม่ได้ หรือ read ceiling ถูกชน; ห้ามรายงาน partial result เป็น complete

**Acceptance Criteria:**

- [ ] Import ไฟล์ local ที่อ่านได้หนึ่งไฟล์สร้าง project/recording/chunk ที่ project-owned พร้อม checksum และไม่พึ่ง Supabase
- [ ] เมื่อ transcription success, `active_recording_id` ชี้ recording เดียวกัน และ transcript review อ่านเฉพาะ recording นั้นพร้อม timestamp/confidence และ completeness state
- [ ] เมื่อ transcript ยังไม่มี, capped, หรือ model ไม่พร้อม ระบบ refuse/แสดงเหตุผลและไม่สร้าง Minute of Note หรือ export ที่ดูเหมือนสมบูรณ์
- [ ] เมื่อ transcript ครบและ local summary provider ตอบสำเร็จ, `summary.generate` สร้าง `whole_story`, `timeline`, `decisions_actions` พร้อม evidence refs และเขียน local Markdown
- [ ] `export.render` เขียน `.srt` และ `.vtt` ของ recording เดียวกัน, ลง `export_artifacts`, และ retry ไม่สะสมไฟล์ซ้ำ
- [ ] การปิด/เปิด process ไม่เปลี่ยน recording, transcript, summary หรือ export identity; หากยังไม่มี runtime evidence ให้สถานะเป็น open ไม่ใช่ PASS

**Success Criteria:** ผู้ใช้ที่มีไฟล์ audio local สามารถเห็น recording เดียวกันใน review, รับรู้ความครบถ้วนของ transcript, สร้าง Minute of Note และหาไฟล์ local export ได้ใน project เดียวกัน โดยทุกผลลัพธ์ trace กลับไปยัง recording และ transcript segments เดิม

**Exit Criteria:**

- Contract/implementation tests ของ imported recording pointer, chunk finalization, recording-scoped transcript, summary handoff และ export idempotence ผ่าน
- `npm run build` และ relevant Rust regression ผ่านบน working tree ที่สะอาดจาก change นี้
- มี local packaged-app click-through หรือ equivalent interaction evidence ครบตั้งแต่ import ถึงเปิดไฟล์ export; static/unit tests เพียงอย่างเดียวไม่ปิด runtime gate
- เอกสารสถานะระบุแยกชัดเจนว่า local proof, runtime/UAT proof, provider gate และ release proof อยู่ระดับใด

**PIC:** Agent-Rust (storage/import/job seam) และ Agent-Frontend (recording-scoped review/handoff)

**Approver:** Boss; Terra ทำ review ของ candidate/code package ก่อน Boss approval

**Risk:** MEDIUM — cross-module behavior ระหว่าง `lib.rs`, Genesis rows, `App.tsx`, summary/export jobs; ไม่มี schema migration ที่กำหนดไว้ใน slice นี้ แต่ wrong active recording หรือ stale chunk metadata อาจทำให้สรุปผิด meeting หรือ retry ซ้ำ

**Dependencies:** Existing GenesisBlockDB adapter/schema; existing local Whisper runtime and model; existing local summary provider (Ollama-compatible) สำหรับ actual Minute of Note; existing job engine; project storage path. Runtime/provider availability remains an external-to-code gate even when no cloud is used.

**Exact proposed write scope after approval:**

- `src-tauri/src/lib.rs` — imported recording activation and chunk finalization contract
- `src/App.tsx` — active-recording review/refresh and truthful handoff messaging
- `src/tauri.ts` — only if the recording-scoped bridge contract requires a signature change
- `src-tauri/src/meeting_intel.rs` — only tests/contract adjustment needed to consume the selected recording; do not redesign summary generation
- `tests/` and/or focused `#[cfg(test)]` modules adjacent to the changed behavior — regression coverage only

No other files, migrations, credentials, release artifacts, external systems, or generated outputs are in scope.

**Tests to add or update after approval:**

- Imported recording sets `active_recording_id` only after the recording is registered and preserves project name/provenance.
- Imported chunk receives duration/transcribed state without losing path/checksum/byte metadata.
- A project with multiple recordings reviews and summarises only the selected recording.
- Missing transcript and `ROW_CAP` refusal prevent summary/export success claims.
- Summary retry ใช้ idempotent replacement ของผลลัพธ์ที่มี recording-scoped provenance และไม่กำหนดให้ LLM output ต้อง byte-identical; local export retry remains idempotent.
- Existing static/UI contracts: `tests/jobActions.test.mjs`, `tests/summaryScoping.test.mjs`, `tests/desktopBootstrap.test.mjs`; existing Rust coverage in `meeting_intel.rs`, `transcript_export.rs`, and relevant `lib.rs`/adapter tests.

## Dependency-ordered later MVP slices

| ID | Slice | Depends on | Status |
|---|---|---|---|
| D-MVP-02 | Minimal transcript correction/audit affordance; keep speaker labels non-biometric. Recording-scoped transcript retrieval/UI bridge is delivered by D-MVP-01 and explicitly excluded from this slice. | D-MVP-01 | Deferred |
| D-MVP-03 | Real Desktop capture → live/catch-up transcription UAT, including local runtime readiness and restart/recovery evidence | D-MVP-01 | Deferred; runtime/device evidence open |
| D-MVP-04 | Pagination/cursor support in GenesisBlockDB for transcripts, summaries and exports beyond the 1000-row ceiling; then enable long-session acceptance | D-MVP-01 | Deferred; upstream dependency |
| D-MVP-05 | Audio export (WAV/MP3) and a separate export queue, only if local MVP acceptance requires them | D-MVP-01 | Deferred |

## Explicit blocker assessment

| Capability | Blocks this MVP? | Decision |
|---|---:|---|
| Login/Supabase | No | Tauri shells intentionally allow local capture, transcription and review without Supabase config. |
| Google Drive OAuth | No | Deferred; local filesystem export is the defined output. |
| Mobile pairing/FUNGWIRE | No | Deferred; architecture is Desktop-first and the target flow is local. |
| Cloud credentials/providers | No for the defined local path | No cloud call is required. A local Whisper runtime and local summary provider are operational dependencies for actual transcription/Minute of Note and must be reported separately. |
| Diarization/voice recognition | No | Channel provenance labels (`เรา`/`อีกฝ่าย`) are sufficient for this slice; arbitrary speaker identity is deferred. |
| External connectors | No | Default-off retrieval is outside the critical path. |
| Production deployment/release signing | No | Release proof is outside this candidate and remains deferred. |

## Out of scope and concerns

- No Google Drive OAuth, Hyper-V/ISO, clean-install restore, Android/device proof, cloud/provider release claim, speaker recognition, or production deployment.
- No claim that the current repository has passed the complete local runtime flow. Existing records distinguish source/test evidence from real-capture, packaged-app, provider and restart UAT.
- Existing `08-real-progress.md` says summary/export code exists but summary/export review after restart remains open; Task 11 records the same boundary because the restart dataset had `summaries=0`.
- The master plan's pairing → FUNGWIRE → cloud critical path is not evidence that those features block the local Desktop MVP; this amendment keeps that plan as a later program path pending review.

## Cross-references

- `docs/plans/2026-08-09-fung-master-implementation-plan.md` — program roadmap and existing phase gates
- `docs/Desktop/ARCHITECTURE.md` — Desktop-first, local API, Genesis boundary, stateful jobs and local export surface
- `docs/Desktop/08-real-progress.md` — current Desktop implementation/evidence ledger and 1000-row refusal boundary
- `docs/Mobile/IMPLEMENTATION_STATUS.md` — mobile evidence boundary; not a dependency for this Desktop MVP
- `docs/.rwang-progress.md` — controller ledger; T12/T13 overlay; ledger is not edited by this task
- `docs/.rwang-tasks/task-11-report.md` — restart, visual, device and real-connector UAT boundaries
- `src-tauri/src/live_meeting.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/meeting_intel.rs`, `src-tauri/src/transcript_export.rs`
- `src/components/LiveMeetingPanel.tsx`, `src/App.tsx`, `src/tauri.ts`, `src/lib/jobActions.ts`, `src/lib/meetingSummaries.ts`

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Initial candidate amendment: reprioritises the local Desktop MVP critical path and defines D-MVP-01 without code or external-state changes. |
| 0.1.1b | Candidate patch: assigns recording-scoped transcript retrieval/UI bridge exclusively to D-MVP-01, narrows D-MVP-02 to correction/audit, aligns summary retry with idempotent replacement/provenance, and binds metadata to the current base commit. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-26 | candidate | Documentation-only local MVP critical-path amendment; implementation awaits Terra review and Boss approval. | not created | Luna |
| 0.1.1b | 2026-08-26 | candidate | Candidate documentation fix for Terra findings; implementation still awaits Terra re-review and Boss approval. | 8a6406e6513943e09447daeb3c6572aa41468b67 | Luna |
