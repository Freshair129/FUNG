---
version: "0.1.0b"
created_at: "2026-08-23T00:00:00+07:00,ATHER"
last_update: "2026-08-23T00:00:00+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "feature-design"
  scope: "FUNG Desktop/Mobile — assisted speaker identification and voice profiles"
  language: "Thai"
---

# FUNG — Assisted Speaker Identification & Voice Profile Specification

## 1. Classification

| Field | Value |
| --- | --- |
| Product owner | Boss (Founder) |
| Technical owner | ATHER |
| Complexity | C-3 — Architecture-Driven Implementation |
| Change risk | HIGH — voice identity and sensitive personal data |
| Parent architecture | `docs/Desktop/ARCHITECTURE.md` |
| Speaker contract | `docs/Mobile/SPEAKER_TIMELINE_PROPOSAL.md` |
| Related design | `docs/specs/2026-08-05-zoom-meeting-ingestion-design.md` |
| Approval state | Candidate — implementation is not authorized |

## 2. Problem

FUNG สามารถทำ speaker diarization เพื่อแบ่งช่วงเสียงเป็น `Speaker 1`, `Speaker 2`, … ได้ แต่ผลดังกล่าวยังไม่ใช่ตัวตนของบุคคล ปัจจุบัน FUNG ยังไม่มีขอบเขตข้อมูลสำหรับเก็บโปรไฟล์ผู้เข้าร่วมประชุมและจับคู่เสียงกับบุคคลอย่างตรวจสอบได้

ฟีเจอร์นี้เพิ่มการระบุผู้พูดแบบมีผู้ใช้กำกับ (assisted identification) โดยไม่เปลี่ยน anonymous speaker cluster ให้เป็นชื่อจริงโดยอัตโนมัติ

## 3. Goals

1. ให้ผู้ใช้สร้างโปรไฟล์ผู้เข้าร่วมประชุมและลงทะเบียนตัวอย่างเสียงได้
2. ใช้ diarization แบ่งเสียงเป็น anonymous speaker ก่อน
3. เปรียบเทียบ voice embedding เพื่อเสนอชื่อบุคคลที่อาจตรงกัน
4. ให้ผู้ใช้ยืนยัน ปฏิเสธ หรือยกเลิกการจับคู่ได้
5. เก็บ provenance, model run, confidence และประวัติการแก้ไขครบถ้วน
6. เก็บเสียงและ voice embedding ในเครื่องเป็นค่าเริ่มต้น
7. ให้ผลลัพธ์ที่ไม่มั่นใจกลับไปเป็น `ไม่ทราบชื่อ` หรือ `Speaker N` ได้เสมอ

## 4. Non-goals

- การระบุชื่อบุคคลจาก transcript, คำลงท้าย หรือบริบทภาษาเพียงอย่างเดียว
- การทำ biometric identification แบบเงียบ ๆ หรือเปิดเป็นค่าเริ่มต้น
- การส่งเสียง, transcript หรือ voice embedding ไป cloud โดยอัตโนมัติ
- การใช้ `voice_profiles` เดิมเพื่อระบุตัวบุคคล — ตารางนั้นเป็น TTS/agent voice
- การวิเคราะห์อารมณ์ บุคลิก หรือเจตนาจากเสียง
- การติดตามบุคคลข้ามการประชุมโดยไม่มี opt-in แยกต่างหาก

## 5. Terminology and identity boundary

| Term | ความหมาย | เป็นตัวตนจริงหรือไม่ |
| --- | --- | --- |
| `Account profile` | บัญชีผู้ใช้ FUNG จาก Supabase `profiles` | ใช่ เฉพาะเจ้าของบัญชี |
| `Participant profile` | โปรไฟล์บุคคลที่ผู้ใช้สร้าง เช่น คุณเจี๊ยบ | ใช่ แต่เป็นข้อมูลที่ผู้ใช้กำหนด |
| `Speaker cluster` | กลุ่มเสียงจาก diarization เช่น `Speaker 1` | ไม่ใช่ |
| `Speaker turn` | ช่วงเวลาที่ cluster หนึ่งพูด | ไม่ใช่ |
| `Voice identity profile` | ข้อมูลอ้างอิงสำหรับเทียบเสียงของ participant | sensitive identity data |
| `Speaker identity link` | ความสัมพันธ์ระหว่าง cluster กับ participant ที่มีสถานะและหลักฐาน | เป็นข้อเสนอจนกว่าจะยืนยัน |

กฎหลัก: `speaker_id` ห้ามทำหน้าที่เป็น `person_id` และ `confidence` ของ diarization ห้ามแสดงเป็นความมั่นใจว่าบุคคลนั้นคือใคร

## 6. Current state and compatibility

FUNG มีโครงสร้างที่ใช้ต่อได้แล้ว:

- `speakers`: `project_id`, `key`, `display_name`, `confidence`
- `speaker_turns`: recording, เวลา, speaker, confidence, status, model run, overlap และ revision
- `transcript_segments.speaker_id`: ความสัมพันธ์จาก transcript ไปยัง speaker cluster
- `model_runs`: provenance ของ diarization/model execution
- `speaker_timeline_revisions`: ประวัติ rename/split/merge/confirm
- `voice_profiles`: โปรไฟล์เสียงสำหรับ TTS พร้อมสิทธิ์และ provider

ยังไม่มีใน current schema:

- persistent participant profile สำหรับบุคคลในประชุม
- encrypted voice identity sample/embedding
- speaker-to-person identity link ที่แยกจากการ rename label

การเพิ่มฟีเจอร์ต้องไม่เพิ่ม `person_id` ลงใน `speakers` โดยตรง เพราะ `Speaker 1` เป็นผลเฉพาะ recording/project และอาจหมายถึงคนละคนในประชุมถัดไป

## 7. Proposed data model

ข้อมูลใหม่เป็น proposal เท่านั้น ต้องเพิ่มหลัง approval และ migration review

### 7.1 `participant_profiles` — local participant directory

| Field | Contract |
| --- | --- |
| `id` | globally unique local ID |
| `owner_scope` | local account/device scope; ไม่เปิด public โดย default |
| `display_name` | ชื่อที่ผู้ใช้กำหนด |
| `aliases_json` | ชื่อเรียก/ชื่อจาก Zoom ที่ผู้ใช้เพิ่มเอง |
| `role_label` | optional เช่น Product, Dev; ไม่ใช้เป็น identity evidence |
| `status` | `active`, `archived`, `deleted` |
| `created_at`, `updated_at` | RFC3339 |

### 7.2 `voice_identity_profiles` — encrypted voice reference

| Field | Contract |
| --- | --- |
| `id` | globally unique local ID |
| `participant_profile_id` | FK ไปยัง participant profile |
| `consent_state` | `pending`, `explicit_consent`, `revoked`, `expired` |
| `consent_evidence_ref` | อ้างอิง audit/consent record ห้ามเก็บข้อความเสียงใน log |
| `embedding_blob_ref` | reference ไป encrypted local blob; ไม่เก็บ raw vector ใน UI/log |
| `embedding_model` | ชื่อ/เวอร์ชัน model |
| `model_checksum` | checksum ของ model package |
| `sample_count` | จำนวนตัวอย่างที่ใช้สร้าง profile |
| `quality_json` | duration, SNR/quality flags และเงื่อนไขการเก็บ |
| `state` | `active`, `revoked`, `expired` |
| `created_at`, `updated_at` | RFC3339 |

Raw audio sample ให้คงอยู่ภายใต้ source recording ที่ผู้ใช้เลือก และต้องมี retention/deletion policy แยก ไม่คัดลอกเข้า profile โดยไม่จำเป็น

### 7.3 `speaker_identity_links` — meeting-scoped matching

| Field | Contract |
| --- | --- |
| `id` | globally unique local ID |
| `recording_id` | ขอบเขตการจับคู่ต้องเริ่มที่ recording |
| `speaker_id` | FK ไปยัง anonymous speaker |
| `participant_profile_id` | FK ไปยัง participant profile |
| `match_source` | `user_confirmed`, `zoom_participant`, `separate_audio_file`, `voice_match` |
| `status` | `proposed`, `confirmed`, `rejected`, `revoked` |
| `match_score` | score จาก embedding matcher; nullable เมื่อ user ตั้งชื่อเอง |
| `threshold_version` | version ของ policy ที่ใช้ตัดสิน candidate |
| `model_run_id` | FK ไปยัง matching run เมื่อเป็น model proposal |
| `evidence_ref` | speaker-turn/sample/time-span ที่ใช้ประกอบการตัดสิน |
| `reviewed_by` | `user`, `import_metadata`, หรือ local operator ID |
| `created_at`, `updated_at` | RFC3339 |

ข้อจำกัด: ผล `confirmed` ต้องมีผู้ใช้หรือ source metadata ที่ได้รับอนุญาตเป็นผู้ยืนยัน และการยืนยันหนึ่งครั้งต้องไม่ทำให้ทุก recording ในอนาคตถูกติดชื่อโดยอัตโนมัติ

## 8. Processing pipeline

```text
Enrollment consent
  → quality check + voice embedding
  → encrypted local voice identity profile

Recording
  → transcription
  → diarization: Speaker 1/2/3
  → speaker turns + model provenance
  → embedding per speaker turn/cluster
  → candidate matching against opted-in profiles
  → proposed identity links
  → user confirm/reject
  → transcript/export uses confirmed display name
```

### 8.1 Enrollment

1. ผู้ใช้สร้างหรือเลือก `participant_profile`
2. FUNG แสดง consent และขอบเขตการใช้เสียงก่อนบันทึก
3. รับตัวอย่างเสียงหลายช่วงที่ผู้ใช้เลือกเอง โดยต้องผ่าน quality gate
4. สร้าง embedding ด้วย local adapter
5. บันทึก model name, version, checksum, sample count และ quality evidence
6. ห้ามตั้ง `consent_state=explicit_consent` หากผู้ใช้ยังไม่ยืนยัน

ค่าเริ่มต้นที่เสนอสำหรับการทดสอบ: อย่างน้อย 3 ตัวอย่าง ความยาวประมาณ 10–30 วินาทีต่อช่วง; จำนวนและ threshold จริงต้องผ่าน benchmark กับเสียงภาษาไทยและสภาพแวดล้อมของ FUNG ก่อน lock

### 8.2 Matching

1. ใช้ diarization สร้าง anonymous turns ก่อนเสมอ
2. รวม embedding ของช่วงเสียงที่มีคุณภาพเพียงพอ
3. เปรียบเทียบกับ voice identity profiles ที่ `consent_state=explicit_consent` และ `state=active` เท่านั้น
4. คืน candidate ได้ไม่เกิน 3 ราย พร้อม score, threshold version และเหตุผลที่ใช้ได้
5. หาก score ต่ำกว่า threshold หรือ margin ระหว่างอันดับหนึ่ง/สองต่ำ ให้คืน `unknown`
6. สร้าง `speaker_identity_links.status=proposed` เท่านั้น

ห้ามใช้ชื่อใน transcript หรือ LLM inference เป็นหลักฐานแทน voice embedding และห้ามให้ LLM เป็นผู้ตัดสิน final identity

### 8.3 Review and confirmation

ผู้ใช้ต้องเห็น:

- ช่วงเสียงตัวอย่างและ timestamp
- ชื่อ candidate และ `match score` ที่อธิบายว่าเป็น score ของการเทียบเสียง ไม่ใช่ certainty ของตัวตน
- model/run provenance
- ปุ่ม `ยืนยัน`, `ปฏิเสธ`, `เลือก Speaker อื่น`, `ไม่ทราบชื่อ`

เมื่อยืนยันแล้วจึงค่อยแสดงชื่อใน transcript, Minute of Note, graph และ export การ rerun ต้องรักษา link ที่ยืนยันไว้และสร้างผลใหม่เป็น revision/proposal แยกต่างหาก

## 9. Model and runtime contract

### 9.1 Diarization baseline

- ใช้ adapter เดิม `pyannote/speaker-diarization-3.1` ตาม current FUNG design
- รัน local บน FUNG Desktop เป็นหลัก
- model เป็น gated dependency; ต้องให้ผู้ใช้ยอมรับ license และดาวน์โหลดด้วย token ของตนเองครั้งแรก
- หลัง cache แล้วต้องรองรับ offline run
- diarization ล้มเหลวต้องไม่ block playback, transcript, notes หรือ graph

### 9.2 Speaker embedding adapter

เริ่มด้วย adapter แบบ ECAPA-TDNN หรือโมเดล speaker embedding ที่มี license เหมาะสม โดย exact model/version ต้องถูก pin หลัง benchmark และบันทึกใน `model_packages`/`model_runs` ไม่ hardcode ใน UI

Adapter ต้องรายงาน:

- model name/version/checksum/license
- embedding dimension และ preprocessing
- runtime/device/compute type
- language/domain limitations
- enrollment และ matching latency
- false accept/false reject result จาก benchmark

เครื่องที่ตรวจพบในรอบปัจจุบันมี RTX 3060 12GB แต่ต้องเก็บ hardware snapshot จริงทุก model run และห้ามสรุป performance จากสเปกที่ผู้ใช้กรอกเอง

## 10. Privacy and security

- local-first: audio, transcript, identity link และ embedding ไม่ขึ้น Supabase โดย default
- Supabase `profiles` ใช้เฉพาะ FUNG account profile ไม่ใช่ voice identity store
- voice embedding ต้องเก็บเป็น encrypted local blob และ access ผ่าน local authorization boundary
- token, raw audio excerpt และ embedding ห้ามเขียนลง logs, error messages, MCP output หรือ Flex message
- revoke ต้องหยุดการ match ในอนาคต และลบ/ทำลาย encrypted embedding ตาม retention policy
- export ภายนอกต้องใช้ชื่อที่ `confirmed` เท่านั้น; `proposed` ต้องแสดงเป็น `Speaker N`
- การแชร์ project/backup ต้องระบุว่าจะรวม participant profile และ voice identity data หรือไม่ ค่าเริ่มต้นคือไม่รวม
- ต้องมี audit event สำหรับ enroll, match proposal, confirm, reject, revoke และ delete

## 11. UX surface

### Settings — People & Voice Identity

- รายการ participant profiles
- สถานะ `ยังไม่มีเสียง`, `รอ consent`, `พร้อมใช้งาน`, `ถูก revoke`
- เพิ่ม/ลบตัวอย่างเสียง
- revoke voice recognition
- แสดง model และ local storage location แบบไม่เปิดเผยข้อมูลลับ

### Recording — Speaker Timeline

- lane เริ่มต้นเป็น `Speaker 1`, `Speaker 2`, …
- badge แยก `Diarization proposal` กับ `Identity proposal`
- แสดง candidate name เฉพาะเมื่อมี match proposal
- ต้องมี manual rename และ `ไม่ทราบชื่อ`
- transcript source audio และ evidence ต้องย้อนกลับได้

## 12. Local command/API proposal

ชื่อคำสั่งต่อไปนี้เป็น contract proposal ยังไม่ implement:

| Command | หน้าที่ |
| --- | --- |
| `participant_profiles_query` | อ่าน participant profiles แบบ local |
| `participant_profile_create` | สร้าง/แก้ profile ที่ผู้ใช้กำหนด |
| `voice_identity_enrollment_start` | เริ่ม consent และ quality-gated enrollment |
| `voice_identity_enrollment_commit` | สร้าง encrypted embedding profile |
| `voice_identity_revoke` | revoke profile และหยุด matching |
| `speaker_identity_candidates_query` | อ่าน identity proposals ต่อ recording |
| `speaker_identity_confirm` | ยืนยัน mapping พร้อม audit |
| `speaker_identity_reject` | ปฏิเสธ mapping |
| `speaker_identity_unlink` | ยกเลิก mapping เดิมแบบ reversible |

MCP/CLI จะเปิดเฉพาะ read-only query และ review action ที่ผ่าน local authorization; ห้ามเปิด raw embedding retrieval เป็น public tool

## 13. Acceptance criteria

### Functional

- [ ] ผู้ใช้สร้าง participant profile และยืนยัน consent ได้
- [ ] ระบบสร้าง anonymous speaker turns ได้โดยไม่มีชื่อบุคคล
- [ ] ระบบเสนอ candidate จาก voice profile ที่ opt-in เท่านั้น
- [ ] ผู้ใช้ confirm/reject/unknown ได้ และผลถูกบันทึกเป็น audit/revision
- [ ] transcript/export แสดงชื่อเฉพาะ confirmed link
- [ ] rerun model ไม่ลบ confirmed mapping และไม่เขียนทับ manual edits
- [ ] revoke profile ปิดการ match รอบถัดไป

### Privacy/security

- [ ] ไม่มี network egress ของ audio/transcript/embedding ใน local-only mode
- [ ] ไม่มี raw embedding หรือเสียงตัวอย่างใน logs/errors/MCP results
- [ ] ลบหรือ revoke แล้วไม่สามารถใช้ profile นั้น match ต่อได้
- [ ] profile deletion ไม่ลบ source audio หรือ transcript โดยอัตโนมัติ
- [ ] unknown/low-confidence result ไม่ถูกแปลงเป็นชื่อโดย LLM หรือ heuristic จากข้อความ

### Quality/verification

- [ ] มี benchmark ภาษาไทยในสภาพเสียงใกล้เคียงการใช้งานจริง
- [ ] วัด false accept, false reject, unknown rate และ latency แยกตามจำนวนผู้พูด
- [ ] ทดสอบเสียงซ้อน, เสียงโทรศัพท์, เสียงสะท้อน, microphone เดียว และไฟล์แยกรายคน
- [ ] มี model/run/hardware provenance ต่อผล matching ทุกครั้ง
- [ ] FUNG ยัง playback/transcribe/export ได้เมื่อ diarization หรือ voice matching ใช้งานไม่ได้

## 14. Rollout gates

| Stage | ขอบเขต | Gate |
| --- | --- | --- |
| A | anonymous diarization + manual rename | ผ่าน regression ของ speaker timeline |
| B | enrollment + assisted candidate matching ภายในเครื่อง | privacy review + benchmark ผ่านเกณฑ์ |
| C | cross-meeting matching แบบ opt-in | consent/retention review และผู้ใช้เปิดเอง |
| D | production default | UAT, audit review, rollback และ delete/revoke proof |

Stage B เป็น scope ที่แนะนำให้ทำก่อน Stage C/D และยังไม่อนุญาตให้ auto-label แบบเงียบ

## 15. Open decisions requiring approval

1. อนุมัติให้มี `participant_profiles`, `voice_identity_profiles` และ `speaker_identity_links` เป็น schema ใหม่หรือไม่
2. เลือก embedding model/license หลัง benchmark ภาษาไทย
3. กำหนด retention ของ sample audio และ encrypted embedding
4. กำหนดว่า participant profile แชร์ข้าม project ได้หรือไม่
5. อนุมัติให้มี cross-meeting matching หรือจำกัดเฉพาะ recording เดียว
6. อนุมัติ privacy/consent wording ก่อนเปิด enrollment UI

## 16. Version diff

| Version | Change |
| --- | --- |
| 0.1.0b | Candidate spec: assisted speaker identification, local voice profiles, identity links, consent, provenance and rollout gates. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-08-23 | candidate | Initial specification for local-first assisted speaker identification and voice profiles. | N/A | ATHER |

Please review and approve this documentation. I will generate the implementation plan and code only after approval.
