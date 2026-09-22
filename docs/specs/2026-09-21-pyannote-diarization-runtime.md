---
version: "0.1.0"
created_at: "2026-09-21T00:00:00+07:00,RWANG"
last_update: "2026-09-21T00:00:00+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "feature-design"
  scope: "FUNG Desktop — optional local pyannote speaker diarization runtime"
  language: "Thai"
---

# FUNG Desktop — pyannote Speaker-Diarization Runtime

## 1. Classification

| Field | Value |
| --- | --- |
| Complexity | C-2 — Documentation-Driven Implementation |
| Risk | HIGH — gated model, third-party runtime, audio privacy and license boundary |
| Parent architecture | `docs/Desktop/ARCHITECTURE.md` |
| Parent feature design | `docs/specs/2026-08-05-zoom-meeting-ingestion-design.md` §6 |
| Peer governance | `docs/ai-system/ethics-governance.md` speaker identity boundary |
| Approval state | Approved by user on 2026-09-21 |

## 2. Decision

เพิ่ม `pyannote/speaker-diarization-3.1` เป็น optional local runtime สำหรับ
แยกผู้พูดจากไฟล์เสียงรวม โดยใช้ `scripts/diarize.py` ที่มีอยู่แล้วและให้ Rust
เรียกผ่าน `run_diarization` เดิม ไม่สร้าง cloud diarization และไม่เปลี่ยน Path A
ที่ใช้ไฟล์เสียงแยกรายคนจาก Zoom

Dependency tree จะถูกติดตั้งเข้า `.venv-whisper` ผ่าน
`scripts/stage_diarization_runtime.ps1` และต้องถูกตรึงด้วย hash ใน
`scripts/diarization-runtime-requirements.txt` ที่สร้างจาก `pip download`
เท่านั้น ห้ามเขียน lockfile ด้วยมือ โดย dependency pure-Python ที่ PyPI มี
เฉพาะ source archive จะถูก build เป็น universal wheel ชั่วคราวเพื่อช่วย resolve
แต่ lock จะเก็บ hash ของ source archive ต้นฉบับ เพื่อไม่ผูกกับ timestamp ของ
wheel ที่ build ซ้ำบนแต่ละเครื่อง

## 3. Runtime contract

| Item | Contract |
| --- | --- |
| Pipeline | `pyannote/speaker-diarization-3.1` |
| Python runtime | FUNG `.venv-whisper` เดียวกับ Whisper |
| Default dependency variant | CPU wheels; ใช้ `-TorchVariant cu121` เมื่อ staging GPU ที่รองรับ |
| Torch compatibility pin | `torch==2.4.1`, `torchaudio==2.4.1`; pin คู่เพื่อคง `AudioMetaData` API ที่ pyannote 3.4.0 ใช้ |
| Staging interpreter | Host Python/uv ที่มี pip; embedded app Python ไม่มี pip โดยเจตนา |
| Worker | `scripts/diarize.py` — `PROGRESS` ทาง stderr, JSON เดียวทาง stdout |
| Output | anonymous `Speaker 1`, `Speaker 2`, … พร้อม `startMs`, `endMs`, `confidence` ที่ nullable |
| Cache | FUNG-owned Hugging Face cache; `FUNG_HF_HOME` override ได้เพื่อ share cache เดียว |
| First fetch | ต้อง accept license ของ pipeline และ `pyannote/segmentation-3.0` พร้อม `FUNG_HF_TOKEN` หรือ `HF_TOKEN` |
| Later runs | ใช้งาน offline จาก cache ที่มีอยู่ |
| Egress | เฉพาะการ download dependency/model ครั้งแรก; ไม่ส่ง audio ไป cloud |

## 4. Product and privacy invariants

- Diarization เป็นตัวช่วยแยกช่วงเสียง ไม่ใช่ voice recognition และห้ามอ้างว่า
  `Speaker 1` คือบุคคลจริงคนใด
- ผลลัพธ์เริ่มเป็น anonymous/model-proposed data ตาม provenance เดิมของ FUNG
- หาก dependency, model, token หรือ inference ไม่พร้อม transcript ต้องยังถูก
  persist และ job ต้องจบได้ โดยบันทึกเหตุผลว่า diarization unavailable
- Token อ่านเฉพาะใน process environment ไม่เขียนลง GenesisBlockDB, manifest,
  logs หรือ job event
- ไม่ bundle gated model weights ใน installer และไม่ redistribute น้ำหนักโมเดล
  ที่ผู้ใช้ดาวน์โหลดภายใต้บัญชีของตนเอง

## 5. Implementation scope

### In scope

- สร้างและ review hash-pinned dependency lockfile สำหรับ CPU runtime
- รองรับ optional CUDA dependency staging ผ่าน script เดิม
- stage dependency เข้า `.venv-whisper` โดยไม่สร้าง Python runtime ซ้ำ
- resolve/install wheels สำหรับ CPython 3.11 ด้วย host interpreter ที่มี pip
- ใช้ readiness probe เดิมเพื่อแยก `dependencies_missing` กับ `model_not_fetched`
- ปรับ setup documentation ให้ใช้ staging flow ที่ reproducible
- ตรวจ Python worker, Rust readiness/runner และ graceful transcript fallback

### Out of scope

- automatic biometric identity หรือ persistent voice profile
- cloud diarization
- การ bundle `pyannote`/Torch เข้า default installer
- เปลี่ยน speaker merge algorithm หรือ schema เดิม
- เปลี่ยน Whisper model profiles ที่ทำไว้ก่อนหน้านี้

## 6. Acceptance criteria

- `scripts/diarization-runtime-requirements.txt` มี dependency versions และ
  SHA-256 hashes ครบจากการ resolve จริง และติดตั้งด้วย `--require-hashes` ได้
- `stage_diarization_runtime.ps1` ผ่าน PowerShell parser และ dependency probe
  แสดง `diarization-deps-ready`
- `diarization_status` รายงานสถานะและ blocker ที่ถูกต้องโดยไม่เรียก network
- worker compile/import ได้ และ output schema parse ได้เมื่อใช้ model cache จริง
- model fetch ใช้ FUNG cache และ token ไม่ปรากฏใน output
- เมื่อ diarization ไม่พร้อมหรือ inference ล้มเหลว transcript ยังอยู่และ job
  event ระบุสาเหตุ
- `cargo test`, Python focused tests และ `git diff --check` ผ่านตามขอบเขต

## 7. Evidence boundary

การมี lockfile หรือ import probe เป็นหลักฐานว่า runtime ติดตั้งได้เท่านั้น
ยังไม่ใช่หลักฐานคุณภาพการแยกผู้พูดภาษาไทย การทดสอบกับเสียงจริงต้องใช้
labelled multi-speaker qualification set และรายงาน DER/JER แยกต่างหาก

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0 | Approved runtime contract for optional local pyannote diarization. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0 | 2026-09-21 | beta | Added approved runtime, privacy, staging and verification contract for `pyannote/speaker-diarization-3.1`. | pending | RWANG |
