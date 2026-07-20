---
version: "0.1.0b"
created_at: "2026-07-20T13:35:00+07:00,ATHER"
last_update: "2026-07-20T14:35:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "feature-proposal"
  scope: "FUNG Mobile dark appearance and speaker timeline"
  language: "Thai"
---

# FUNG Mobile — Dark Mode and Speaker Timeline Proposal

## 1. Classification

| Field | Value |
| --- | --- |
| Product owner | Boss (Founder) |
| Technical owner | ATHER |
| Complexity | C-3 — Architecture-Driven Implementation |
| Change risk | HIGH |
| Parent product | `docs/Mobile/PRODUCT_UX_SPEC.md` `0.1.0b` |
| Parent architecture | `docs/Mobile/TECHNICAL_DESIGN.md` `0.1.1b` |
| Desktop contract | `docs/Desktop/ARCHITECTURE.md` |
| Peer visual contract | `docs/Mobile/DARK_AND_CRITICAL_STATES_REVIEW.md` |
| Approval state | Approved by Boss on 2026-07-20; beta implementation authorized |

เหตุผลที่เป็น C-3/HIGH: ฟีเจอร์เชื่อม audio timeline, diarization model job, transcript/evidence provenance, schema, Desktop delegation, gesture-heavy rendering และ accessibility ใน Light/Dark appearance

## 2. Scope Decision

คำว่า “timeline แบบ DAW” ใน proposal นี้หมายถึง **speaker timeline viewer/editor** ที่ใช้ anatomy แบบ DAW เพื่ออ่านเวลา เล่นเสียง เลือก speaker turn และแก้ label ได้แม่นยำ

### In Scope

- แสดงผู้พูดเป็น lane ตามแกนเวลาร่วมกัน
- แสดง waveform clip ของแต่ละ speaker turn
- แสดงช่วงพูดซ้อนกันโดยไม่ merge ข้อมูลทิ้ง
- Play/pause, seek, drag playhead, horizontal pan และ pinch-to-zoom
- Rename anonymous speaker label
- Split speaker turn ที่ playhead
- Merge speaker labels แบบ reversible และต้องยืนยัน
- ผูก speaker turn กลับไปยัง source audio, transcript และ model run
- Light, Dark และ System appearance ที่ persist ข้าม session
- รับผล diarization จาก paired FUNG Desktop และรองรับ on-device adapter ในอนาคต

### Out of Scope

- Mixer, volume automation, pan, buses, EQ, plugins หรือ mastering
- การเลื่อน audio clip เพื่อเปลี่ยนเวลา source
- destructive audio trimming
- automatic biometric identity หรือการกล่าวอ้างว่า Speaker 1 คือบุคคลใด
- cloud diarization โดยปริยาย
- การบังคับให้มี Desktop เพื่อเล่นเสียงหรือแก้ label ด้วยมือ

การตัดสินใจนี้ขยาย visual timeline จาก product spec เดิมโดยไม่เปิด scope เป็น Full DAW ซึ่งยังคงเป็น out-of-scope

## 3. Product Behavior

### Standalone Truth

- Mobile ต้องเล่น source audio, แสดง waveform, seek, สร้าง anonymous speaker label และแก้ label ด้วยมือได้โดยไม่มี Desktop/Internet
- หากไม่มี compatible diarization model ให้แสดง `ยังไม่ได้แยกผู้พูด` พร้อมทางเลือก `ประมวลผลบน FUNG Desktop` เมื่อ paired device พร้อม
- การไม่มี diarization result ห้ามปิด playback, notes, transcript หรือ graph
- diarization result ทุกชิ้นเริ่มเป็น `ข้อเสนอ` จนผู้ใช้ยืนยันหรือแก้

### Speaker Identity Boundary

- ค่าเริ่มต้นใช้ `Speaker 1`, `Speaker 2`, `Speaker 3`
- confidence หมายถึงความมั่นใจในการจัดกลุ่มเสียง ไม่ใช่ความมั่นใจในตัวตนบุคคล
- การ rename เป็น user-authored label และเก็บ audit/revision
- ห้าม infer ชื่อจริงจากเสียงโดยไม่มี explicit biometric feature proposal และ consent แยกต่างหาก

## 4. Primary Flow

1. ผู้ใช้เปิด recording หรือ note detail แล้วเลือกแท็บ `ไทม์ไลน์`
2. ระบบแสดง waveform overview และ source duration ทันที
3. หากมี diarization result ระบบสร้าง speaker lanes ตาม time axis เดียวกัน
4. ผู้ใช้ drag playhead หรือแตะ clip เพื่อฟังช่วงนั้น
5. Selected clip แสดง speaker, time range, confidence, provenance และ transcript excerpt
6. ผู้ใช้ rename, split turn หรือ merge speaker label
7. ทุก mutation สร้าง revision/audit event โดยไม่เปลี่ยน source audio
8. Transcript และ GenesisBlockDB speaker relations อ้าง revision ล่าสุด แต่ยังย้อนกลับไปยัง evidence เดิมได้

## 5. Screen Contract — M8 Speaker Timeline

### Header and Transport

- Title: `ไทม์ไลน์ผู้พูด`
- Duration และ runtime location ต้องมองเห็นได้
- Transport แสดง play/pause, current time, total time และ overview waveform
- runtime copy ที่อนุญาต:
  - `บนมือถือ`
  - `แยกผู้พูดบน FUNG Desktop`
  - `ยังไม่ได้แยกผู้พูด`

### Timeline Canvas

- time ruler ติดด้านบนของ canvas
- playhead เส้นเดียวตัดทุก lane
- lane label ยึดด้านซ้ายขณะ horizontal scroll
- speaker turn เป็น waveform clip ที่ยึด start/end time จริง
- overlap แสดงคนละ lane ในตำแหน่งเวลาเดียวกัน
- selected clip ใช้ outline + contrast ไม่พึ่งสีอย่างเดียว
- zoom เปลี่ยน density ของ ruler และ waveform tile โดยไม่เปลี่ยนข้อมูล

### Selected Turn Inspector

- speaker label
- start/end timestamp
- confidence + epistemic label `ข้อเสนอ`, `ยืนยันแล้ว`, `ผู้ใช้แก้ไข`
- transcript excerpt และ evidence link
- actions: `เปลี่ยนชื่อ`, `แยกคลิป`, `รวมผู้พูด`

### Empty and Error States

| State | Required copy/behavior |
| --- | --- |
| No diarization | `ยังไม่ได้แยกผู้พูด` + manual labels + optional Desktop action |
| Processing | progress, runtime location, cancel/resume job |
| Desktop disconnected | keep source timeline; show last durable job checkpoint |
| Low confidence | mark turn as `ข้อเสนอ`; never hide it or present as fact |
| Overlap | show both turns with overlap indicator |
| Model failure | retain audio/transcript; show retry or choose another runtime |

## 6. Visual Direction

### Approved Concepts

| Mode | Artifact |
| --- | --- |
| Light | `docs/Mobile/concepts/09-speaker-timeline-light.png` |
| Dark | `docs/Mobile/concepts/dark/05-speaker-timeline-dark.png` |

### Dark Mode Contract

- Appearance setting: `System`, `Light`, `Dark`; default `System`
- User selection persist locally and applies before first painted frame
- Dark canvas uses charcoal porcelain `#171918`, not pure black
- Raised surfaces use `#202321` / `#272A27`
- Primary text `#F2EFE8`; secondary text `#A9AAA5`; quiet boundaries `#40423F`
- Speaker colors use accessible semantic variants:
  - Speaker 1 / focus: indigo `#6679AD`
  - Speaker 2 / local-safe: sage `#8FA88F`
  - Speaker 3 / proposal: warm metal `#C39A4A`
- color is never the only speaker cue; every lane has label, ordering and waveform geometry
- recording red `#E34338` appears only during active recording or destructive confirmation
- measured contrast must pass WCAG AA for text and essential controls

### Timeline Container Model

- open canvas with rails and a shared ruler
- transport and selected inspector may use porcelain wells
- no generic card grid
- bottom navigation remains owned by the Mobile shell
- minimum touch target 44×44 pt; lane height minimum 64 px at default zoom

## 7. Data Design Delta

Existing `speakers`, `transcript_segments`, `model_runs`, `jobs` and `audit_events` remain canonical. Add the following tables only after approval:

### `speaker_turns`

| Field | Contract |
| --- | --- |
| `id` | globally unique offline-capable ID |
| `recording_id` | source recording |
| `speaker_id` | anonymous/user-renamed speaker |
| `start_ms`, `end_ms` | immutable source time range for this revision |
| `confidence` | nullable clustering confidence |
| `status` | `proposed`, `confirmed`, `user_edited`, `superseded` |
| `model_run_id` | nullable provenance to diarization job |
| `overlap_group_id` | nullable group for simultaneous speech |
| `revision_of` | nullable prior speaker-turn revision |
| `created_at` | RFC3339 timestamp |

### `waveform_tiles`

| Field | Contract |
| --- | --- |
| `recording_id` | source recording |
| `zoom_level` | bounded integer resolution |
| `tile_index` | ordered tile number |
| `start_ms`, `end_ms` | time coverage |
| `peaks_blob` | compact min/max amplitude pairs |
| `source_checksum` | invalidates tile when source changes |

### Invariants

- `0 ≤ start_ms < end_ms ≤ recording.duration_ms`
- overlapping turns are legal
- speaker merge creates revisions; it does not rewrite source evidence silently
- deleting a speaker label never deletes audio, transcript or evidence spans
- waveform tiles are derived cache and can be regenerated

## 8. Runtime and Job Design

### Diarization Adapter

- job type remains `transcript.diarize`
- input manifest includes recording checksum, selected audio layer and optional transcript revision
- output includes anonymous clusters, turns, overlap, confidence and model metadata
- Desktop is the preferred initial runtime for higher-accuracy diarization
- on-device adapter is optional and must declare model size, language behavior, battery/thermal limits and license before enablement

### Import and Reconciliation

1. verify result manifest and recording checksum
2. reject out-of-range or malformed turns
3. store model output as proposed revisions
4. preserve competing user edits as conflicts; never overwrite silently
5. rebuild transcript speaker references and graph relations transactionally
6. retain original model output for audit

## 9. Command/API Delta

| Command | Purpose |
| --- | --- |
| `mobile_timeline_query` | bounded viewport query for ruler, waveform tiles, speakers and turns |
| `mobile_diarization_start` | create local or delegated `transcript.diarize` job |
| `mobile_diarization_import` | validate and commit proposed turns from model result |
| `mobile_speaker_rename` | create user-authored speaker label revision |
| `mobile_speaker_turn_split` | split one turn at a validated playhead position |
| `mobile_speaker_merge` | merge labels after explicit confirmation, preserving revisions |
| `mobile_speaker_turn_confirm` | promote proposed turn to confirmed/user-reviewed state |

All commands must return provenance and revision identifiers where relevant. MCP may expose read-only timeline query first; mutation tools require a later capability/security review.

## 10. Performance Design

- timeline query is viewport-bounded; never return the whole recording by default
- render lanes through virtualized HTML/SVG or Canvas with accessible DOM mirrors
- waveform tiles load by zoom and visible time window
- default maximum visible speaker lanes before vertical virtualization: 8
- target interaction latency: seek/selection acknowledgement ≤100 ms on reference device
- target pan/zoom: no sustained frame drop below 50 fps on reference device
- diarization never competes with active capture priority; delegated/local jobs pause when resource policy requires

Targets are candidate gates until measured on reference Android/iPhone hardware.

## 11. Accessibility

- speaker lane has accessible name, order, start/end and state
- playhead time is announced without flooding screen readers during continuous playback
- keyboard/switch access can seek by fixed increments and move between turns
- patterns, labels and focus outline supplement color
- reduced motion disables inertial/animated zoom transitions
- Dynamic Type must not collapse the time axis; inspector becomes scrollable before labels truncate critically

## 12. Acceptance Criteria

- Light/Dark/System selection persists and does not flash the wrong theme at startup
- timeline shows at least three speakers and overlapping turns on a shared time axis
- seek, play/pause, clip selection, horizontal pan and pinch zoom update real state
- selected turn exposes timestamp, confidence, epistemic status, transcript and provenance
- rename, split and confirmed merge create revisions without modifying source audio
- no-model and disconnected states retain playback and manual labeling
- low-confidence diarization is visibly proposed, never verified identity
- 60-minute recording with ≥1,000 turns remains responsive under viewport-bounded loading
- Light and Dark implementations pass measured contrast and concept fidelity review

## 13. Test Strategy

- schema constraints for ranges, overlap and revisions
- model-result parser tests for malformed/out-of-range turns
- conflict tests for Desktop result arriving after user edits
- waveform tile checksum invalidation
- timeline viewport/zoom query tests
- React interaction tests for selection, seek, split and merge confirmation
- browser visual regression at 393×852 for Light/Dark
- Android/iPhone real-device pan/zoom and 60-minute recording performance
- screen reader and reduced-motion checks

## 14. Rollout

1. Theme persistence and full-screen Dark audit
2. Static timeline query + waveform tiles
3. Speaker lanes, transport and selection
4. Manual rename/split/merge revisions
5. Desktop delegated diarization import
6. Optional on-device adapter proposal after model/license benchmark
7. Real-device performance and accessibility gate

## 15. Open Decisions

| ID | Decision | Default proposal |
| --- | --- | --- |
| ST-01 | Initial diarization runtime | Paired FUNG Desktop first |
| ST-02 | On-device model | Not selected; separate benchmark/license gate |
| ST-03 | Speaker palette beyond 3 lanes | deterministic accessible palette + label/index |
| ST-04 | Merge confirmation | required because transcript/graph references change |
| ST-05 | Timeline editing boundary | speaker metadata only; source audio immutable |

## 16. Definition of Done

- approved Light/Dark concepts implemented faithfully
- schema/API contracts reviewed against Desktop and Mobile architecture
- automated tests and browser visual regression pass
- Android/iPhone real-device performance evidence passes candidate targets
- diarization provenance, confidence and user revisions remain auditable
- no biometric identity, Full DAW or cloud behavior is introduced implicitly
- implementation status and version diff are updated

## Version Diff

### `0.0.0` → `0.1.0b`

- Added bounded DAW-style speaker timeline without opening Full DAW scope.
- Added Light/Dark concept contracts and persistent appearance behavior.
- Added speaker-turn and waveform-tile data proposal.
- Added diarization runtime, provenance, revision, conflict and performance contracts.
- Preserved standalone playback/manual-label behavior when Desktop or model is unavailable.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-20 | candidate | Initial dark appearance and speaker timeline feature proposal | N/A — workspace is not an initialized Git repository | ATHER |
| 0.1.0b | 2026-07-20 | beta | Approved for implementation; scope and architecture retained without expansion | N/A — workspace is not an initialized Git repository | ATHER |
