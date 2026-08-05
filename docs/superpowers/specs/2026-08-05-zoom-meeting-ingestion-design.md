---
version: "0.1.0b"
created_at: "2026-08-05T00:00:00+07:00,ATHER"
last_update: "2026-08-05T00:00:00+07:00,ATHER"
status: "approved-design"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "feature-design"
  scope: "FUNG Desktop — Zoom meeting ingestion, speaker-attributed transcript, knowledge graph"
  language: "Thai"
---

# FUNG Desktop — Zoom Meeting Ingestion Design (Phase 1)

## 1. Classification

| Field | Value |
| --- | --- |
| Product owner | Boss (Founder) |
| Technical owner | ATHER |
| Complexity | C-3 — Architecture-Driven Implementation |
| Parent architecture | `docs/Desktop/ARCHITECTURE.md` `1.0.0b` |
| Parent pipeline | `docs/Desktop/AUDIO_AI_PIPELINE.md` `0.1.0b` |
| Speaker identity contract | `docs/Mobile/SPEAKER_TIMELINE_PROPOSAL.md` `0.1.0b` |
| Cloud boundary contract | `supabase/README.md` |
| Approval state | Approved by Boss on 2026-08-05 |

## 2. Goal

ผู้ใช้เชื่อมบัญชี Zoom กับ FUNG Desktop แล้ว import cloud recording เข้ามาได้ จากนั้นระบบสร้าง transcript ภาษาไทย/อังกฤษที่**แยกผู้พูดได้** และสร้าง **knowledge graph** (หัวข้อ, มติ, action item, สิ่งที่ถูกพูดถึง) ที่ค้นข้ามประชุมได้ — ทั้งหมดประมวลผลและเก็บในเครื่องตาม local-first contract

### Non-goals (Phase นี้)

- Webhook / real-time bot / RTMS
- Auto-sync ตามรอบเวลา (มีแค่ manual list + import)
- Entity resolution ข้ามประชุม และ vector search
- Mobile surface
- การเก็บ audio/transcript/graph บน cloud ใดๆ

## 3. Architecture Decision

**ทุกอย่างรันใน FUNG Desktop (Tauri/Rust) — ไม่มี server ใหม่**

ทางเลือกที่พิจารณาแล้วไม่เลือก:

| ทางเลือก | เหตุผลที่ไม่เลือก |
| --- | --- |
| Webhook + backend server | ขัด local-first contract; ต้องมี cloud surface รับข้อมูลประชุม |
| Webhook ผ่าน Supabase Edge Function | เพิ่ม cloud surface ที่ยังไม่มีและไม่จำเป็นสำหรับ manual import |
| Manual file import อย่างเดียว | UX ไม่อัตโนมัติ ไม่ได้ per-participant files และ metadata |

```
Zoom Cloud ──(user OAuth, HTTPS pull)──> FUNG Desktop
                                            │
   zoom.import job ── ดาวน์โหลด audio (แยกรายคนถ้ามี) + metadata
        │
   transcript.transcribe job (faster-whisper เดิม, ต่อไฟล์)
        │
   speaker merge ── ผูก segment กับ Speaker
        │            (ชื่อจาก Zoom หรือ pyannote fallback)
   transcript.diarize job (เฉพาะกรณีไฟล์รวม)
        │
   graph.build job ── LLM extraction (BYOM) → graph_nodes/graph_edges
```

ทุกขั้นเป็น stateful job ตาม job engine เดิม (resumable, มี ModelRun provenance)

## 4. Zoom Connect (OAuth)

- Zoom **General App แบบ user-authorized OAuth + PKCE** (ไม่ใช่ Server-to-Server เพราะรันบนเครื่องผู้ใช้)
- Redirect: loopback `http://127.0.0.1:<port>/zoom/callback` — Rust เปิด HTTP listener ชั่วคราวเฉพาะตอนกด Connect แล้วปิดทันทีที่ได้ code
- Scopes ขั้นต่ำ: อ่านรายการ cloud recordings ของตัวเอง + ดาวน์โหลดไฟล์ recording ของตัวเอง
- **Access/refresh token เก็บใน Windows Credential Manager** ผ่าน keyring crate เท่านั้น — ห้ามลง GenesisBlockDB, ห้ามลง log, ห้ามลง config file
- Local DB เก็บเฉพาะ redacted connection metadata: เชื่อมเมื่อไหร่, Zoom account email, สถานะการเชื่อมต่อ
- Token refresh อัตโนมัติ; refresh ล้มเหลว → สถานะ "ต้องเชื่อมต่อใหม่" โดยไม่ทำให้ job ที่ค้างอยู่ crash

## 5. Import Pipeline — โมดูลใหม่ `src-tauri/src/zoom_sync.rs`

Tauri commands ใหม่:

| Command | หน้าที่ |
| --- | --- |
| `zoom_connect` | เริ่ม OAuth flow, เก็บ token, บันทึก connection metadata |
| `zoom_disconnect` | revoke token ฝั่ง Zoom, ลบ token จาก secure storage |
| `zoom_connection_status` | คืนสถานะการเชื่อมต่อ (redacted) |
| `zoom_list_recordings` | `GET /users/me/recordings` พร้อม date-range paging |
| `zoom_import_recording` | สร้าง job `zoom.import` สำหรับ recording ที่เลือก |

Job `zoom.import`:

1. ดึงรายละเอียด recording files ทั้งชุด: ไฟล์เสียงรวม (M4A), **ไฟล์เสียงแยกรายคน** (ถ้า host เปิด "Record a separate audio file of each participant"), participant list
2. ดาวน์โหลดลง project folder ผ่าน HTTPS (รองรับ resume ด้วย HTTP Range) → ลงทะเบียน Recording + blob manifest ตาม pattern เดิม
3. จบแล้ว chain เข้า pipeline transcribe อัตโนมัติ

การ import ซ้ำ recording เดิมต้อง idempotent — ตรวจจาก Zoom meeting UUID ก่อนสร้าง Recording ใหม่

## 6. Speaker Diarization (Hybrid)

### Path A — มีไฟล์เสียงแยกรายคน (แม่นสุด, ไม่ใช้ ML)

1. รัน faster-whisper ทีละไฟล์ (job `transcript.transcribe` ต่อไฟล์)
2. ทุก segment ผูกกับเจ้าของไฟล์โดยตรง
3. Merge ทุกไฟล์เป็น timeline เดียวเรียงตาม `startMs` — ช่วงพูดซ้อนกันเก็บทั้งคู่ ไม่ merge ทิ้ง
4. สร้าง Speaker record ใช้ **Zoom display name เป็น proposed label** — เป็น metadata จาก Zoom ไม่ใช่ biometric claim; ผู้ใช้แก้/ยืนยันได้เสมอตาม identity boundary ของ SPEAKER_TIMELINE_PROPOSAL

### Path B — มีแต่ไฟล์รวม (fallback + ใช้กับ recording ที่ไม่ใช่ Zoom ได้)

1. whisper ไฟล์รวมตามเดิม
2. Job `transcript.diarize` รัน script ใหม่ `scripts/diarize.py` (pyannote-audio ใน `.venv-whisper`) — I/O contract เดียวกับ `transcribe.py`: `PROGRESS` ทาง stderr, JSON เดียวทาง stdout
3. ได้ speaker turns → จับคู่กับ whisper segments ด้วย time-overlap มากสุด → label `Speaker 1/2/3` แบบ anonymous พร้อม confidence
4. ข้อจำกัดที่รับรู้แล้ว: pyannote model หลักเป็น gated บน HuggingFace — ต้องมี setup step (accept license + HF token ครั้งแรกตอนโหลด model ลงเครื่อง) หลังจากนั้นทำงาน offline ได้

**กติกาสำคัญ:** ถ้า diarization ไม่พร้อม/ล้มเหลว transcript ต้องยังใช้ได้ปกติ แค่ไม่มี speaker label — การไม่มี diarization ห้าม block playback, transcript, graph

## 7. Knowledge Graph — job ใหม่ `graph.build`

เขียนผ่าน genesis_adapter ลง `graph_nodes` / `graph_edges` เดิม สองชั้น:

### ชั้น Structural (deterministic — สร้างเสมอ)

- Node: `Meeting` (จาก recording), `Speaker` (ต่อ speaker label)
- Edge: `Speaker --spoke_in--> Meeting`, `Meeting --part_of--> Project`

### ชั้น LLM Extraction (BYOM adapter — best-effort, retry ได้)

ส่ง transcript เป็น chunk เข้า local LLM (Ollama / OpenAI-compatible endpoint ตาม BYOM adapter เดิม) สกัด:

| Entity type | ความหมาย |
| --- | --- |
| `Topic` | หัวข้อที่คุยกัน |
| `Decision` | มติ/ข้อสรุปที่ตกลงกัน |
| `ActionItem` | ใคร-ทำอะไร-เมื่อไหร่ |
| `Mention` | คน/โปรเจกต์/องค์กรที่ถูกพูดถึง |

กติกาตาม AUDIO_AI_PIPELINE:

- ทุก extracted node ต้องมี **evidence edge ชี้กลับ transcript segment** + confidence
- ทุก node ติด label ว่าเป็น AI inference
- ModelRun บันทึก provider, model, parameters
- extraction ล้มเหลว → structural graph ยังอยู่ครบ, job retry ได้
- `graph.build` ซ้ำบน meeting เดิมต้อง idempotent (แทนที่ผล extraction เก่าของ meeting นั้น ไม่สร้าง node ซ้ำ)

ผลลัพธ์ที่ต้องตอบได้: "โปรเจกต์ X ถูกพูดถึงในประชุมไหนบ้าง" / "ใครรับ action item อะไรไปจากประชุมนี้"

## 8. UI (Phase 1 — minimal)

- Settings → **Connections → Zoom**: Connect / Disconnect / สถานะ (email ที่เชื่อม)
- หน้า **Import from Zoom**: รายการ cloud recordings (ชื่อประชุม, วันเวลา, ความยาว, มี per-participant files ไหม) + ปุ่ม import + job progress
- Transcript view เดิมแสดง speaker label ต่อ segment (rename ได้)
- Graph query ใช้ surface pattern เดิม (`mobile_graph_query`)

## 9. Error Handling

| กรณี | พฤติกรรม |
| --- | --- |
| Token หมดอายุ | refresh อัตโนมัติ; ล้มเหลว → สถานะ "ต้องเชื่อมต่อใหม่", job pause ไม่ crash |
| ดาวน์โหลดขาด | resume ด้วย HTTP Range; retry ตาม job state machine เดิม |
| Zoom rate limit (429) | backoff ตาม `Retry-After` แล้ว retry |
| ไม่มีไฟล์แยกรายคน | ตกไป Path B อัตโนมัติ พร้อมแจ้งผู้ใช้ว่าความแม่น speaker ลดลง |
| pyannote model ไม่พร้อม | transcript ใช้ได้ปกติ ไม่มี speaker; แสดงทางแก้ (setup step) |
| LLM extraction ล้มเหลว | structural graph ครบ; retry extraction ได้ |

## 10. Security & Privacy

- Token อยู่ใน OS secure storage เท่านั้น; log ทุกจุด redact token/URL ที่มี access token ฝัง
- ไฟล์เสียง/transcript/graph อยู่ในเครื่องเท่านั้น — ไม่มีการ upload
- Download URL ของ Zoom มี access token ใน query — ห้าม log URL เต็ม
- Loopback listener เปิดเฉพาะระหว่าง OAuth flow และ bind เฉพาะ 127.0.0.1

## 11. Testing

- **Unit:** multi-file timeline merge (รวม overlap), time-overlap speaker assignment, graph upsert idempotency, VTT/paging parsing
- **Integration:** mock Zoom API fixtures ทดสอบ job chain ทั้งเส้น (import → transcribe → diarize/merge → graph.build) รวม failure paths
- **UAT:** บัญชี Zoom จริง 1 ประชุม ทั้งแบบเปิดและไม่เปิด separate audio files

## 12. Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Initial approved design: desktop-pull Zoom ingestion, hybrid diarization, structural + LLM knowledge graph. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-08-05 | approved-design | Initial Zoom meeting ingestion design approved by Boss. | N/A | ATHER |
