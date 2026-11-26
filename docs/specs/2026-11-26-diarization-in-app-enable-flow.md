---
version: "0.1.0b"
created_at: "2026-11-26"
last_update: "2026-11-26"
status: "draft"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "specification"
  scope: "FUNG Desktop — in-app opt-in flow that installs the diarization runtime and fetches the gated pyannote model"
  language: "Thai-first"
---

# FUNG Desktop — เปิดใช้การแยกเสียงผู้พูดจากในแอป

## 1. Classification

| Field | Value |
| --- | --- |
| Complexity | C-2 — Documentation-Driven Implementation |
| Risk | HIGH — ดาวน์โหลดและรันโค้ดของบุคคลที่สาม, เก็บ token, เงื่อนไขอนุญาตของโมเดลและแพ็กเกจ |
| Parent architecture | `docs/Desktop/ARCHITECTURE.md` |
| Parent feature design | [`2026-09-21-pyannote-diarization-runtime.md`](2026-09-21-pyannote-diarization-runtime.md) (runtime contract, pins, privacy invariants) |
| Peer governance | `docs/ai-system/ethics-governance.md` speaker identity boundary |
| Approval state | **Draft — รอเจ้าของโปรเจกต์อนุมัติ ยังไม่มีโค้ด** |

## 2. ปัญหา

ฟีเจอร์แยกผู้พูดมีโค้ดครบแล้ว (`scripts/diarize.py`, `run_diarization`,
`diarization_status`) แต่ผู้ใช้ที่ติดตั้งจาก installer ใช้ไม่ได้ถ้าไม่ทำเองสามอย่าง:

1. รัน `scripts/stage_diarization_runtime.ps1` ซึ่งต้องมี Python ที่มี pip บนเครื่อง
   และต้องมี source checkout
2. ไปยอมรับเงื่อนไขของ `pyannote/speaker-diarization-3.1` และ
   `pyannote/segmentation-3.0` บน Hugging Face
3. ตั้ง environment variable `FUNG_HF_TOKEN` ก่อนเปิดแอป

ข้อ 1 และ 3 เป็นงานของนักพัฒนา ไม่ใช่ของผู้ใช้ทั่วไป ผลคือฟีเจอร์นี้ไม่มีใครใช้ได้จริง

## 3. Decision (เสนอ)

เพิ่มขั้นตอน "เปิดใช้การแยกเสียงผู้พูด" ในแอป โดยใช้ readiness probe เดิมเป็นตัวบอกว่าขั้นไหนยังขาด:

| Blocker เดิม | สิ่งที่แอปทำแทนผู้ใช้ |
| --- | --- |
| `dependencies_missing` | ดาวน์โหลด **runtime add-on archive** ที่ FUNG build และตรึง hash ไว้ แล้วแตกไฟล์ลงโฟลเดอร์ข้อมูลของแอป |
| `model_not_fetched` | พาผู้ใช้ไปหน้าเงื่อนไขบน Hugging Face (เปิดใน browser ของเครื่อง) รับ token ผ่านช่องในแอป แล้วดาวน์โหลดโมเดลทันทีเพื่อตรวจว่าใช้ได้ |

ผู้ใช้ที่ไม่เปิดฟีเจอร์นี้จะไม่ต้องโหลดอะไรเพิ่ม และ installer มีขนาดเท่าเดิม

### ทางเลือกที่ไม่เลือก

- **ใส่ pyannote/torch มากับ installer:** installer ใหญ่ขึ้นเกิน 1 GB สำหรับทุกคน
  และยังต้องทำขั้นตอนยอมรับเงื่อนไข/token อยู่ดี
- **รัน pip ในแอป:** embedded Python ไม่มี pip โดยเจตนา (ดู parent spec §3)
  และการ resolve จาก PyPI ตอนรันทำให้ได้ชุดแพ็กเกจที่ไม่ได้ review
- **ใส่โมเดลมากับ installer:** ทำไม่ได้ เป็นเงื่อนไขอนุญาตของโมเดล

## 4. Runtime add-on archive

| Item | Contract |
| --- | --- |
| เนื้อหา | `site-packages` ของชุดแพ็กเกจใน `scripts/diarization-runtime-requirements.txt` เท่านั้น (CPU variant), พร้อมโฟลเดอร์ `LICENSES/` ของทุกแพ็กเกจ |
| วิธี build | CI job ใหม่ที่ติดตั้งจาก lockfile ด้วย `--require-hashes` ลง venv ชั่วคราว แล้วบีบ `site-packages` เป็น archive เดียว |
| ที่เก็บ | GitHub Release asset ของ FUNG แยกตาม lock version, ชื่อเช่น `fung-diarization-runtime-cpu-win-x64-<lockhash>.zip` |
| การตรวจสอบ | SHA-256 และขนาดของ archive ถูก compile เข้า binary ของแอป (ไม่อ่านจาก server); hash ไม่ตรง = ลบไฟล์และหยุด |
| ตำแหน่งติดตั้ง | `<data_root>/diarization-runtime/<lockhash>/site-packages` — ไม่เขียนลงโฟลเดอร์ติดตั้งของแอป เพราะอาจเขียนไม่ได้และจะถูกลบตอนอัปเดต |
| การโหลด | Rust ตั้ง `PYTHONPATH` ชี้โฟลเดอร์นี้ตอน spawn `diarize.py` เท่านั้น; worker อื่นไม่เห็น |
| ความเข้ากันได้ | archive ผูกกับ CPython minor version ของ `.venv-whisper` ที่มากับ installer; แอปตรวจ tag นี้ก่อนใช้ |
| การติดตั้งแบบ atomic | ดาวน์โหลดเป็น `.partial` → ตรวจ hash → แตกลงโฟลเดอร์ชั่วคราว → rename เป็นโฟลเดอร์จริง; ล้มกลางทางไม่ทิ้งสถานะครึ่งๆ |
| Resume/ยกเลิก | ยกเลิกได้ระหว่างดาวน์โหลด; เริ่มใหม่ได้โดยไม่ต้องโหลดซ้ำส่วนที่ hash แล้ว (ถ้า server รองรับ Range) |
| พื้นที่ดิสก์ | ตรวจพื้นที่ว่างก่อนเริ่ม (ประมาณ archive + ขนาดหลังแตก) ด้วย `GetDiskFreeSpaceExW` ที่มีอยู่แล้ว |
| ถอนการติดตั้ง | ปุ่ม "ลบ" ในหน้าเดียวกัน ลบทั้ง runtime add-on และ (ถามแยก) model cache |

`dependencies_present()` เปลี่ยนจากดู `.venv-whisper/Lib/site-packages` อย่างเดียว
เป็นดูโฟลเดอร์ add-on ที่ตรงกับ lockhash ปัจจุบันด้วย (dev checkout ที่ stage ด้วย script เดิมยังใช้ได้)

## 5. Token และโมเดล

| Item | Contract |
| --- | --- |
| การรับ token | ช่องกรอกในแอป (type password); ผู้ใช้กรอกเอง |
| ที่เก็บ token | Windows Credential Manager ผ่าน crate `keyring` ที่มีอยู่แล้ว; ไม่เขียนลง GenesisBlockDB, ไฟล์ config, log หรือ job event |
| ลำดับความสำคัญ | `FUNG_HF_TOKEN` / `HF_TOKEN` จาก environment ยังใช้ได้และมาก่อน (คงพฤติกรรมเดิม) |
| การดาวน์โหลดโมเดล | หลังรับ token แอปสั่ง worker ดาวน์โหลดโมเดลทันที (โหมด `--fetch-only`) ไม่รอจนถึงงานแรก |
| ข้อความเมื่อพลาด | HTTP 401 → "token ไม่ถูกต้อง"; HTTP 403/gated → "ยังไม่ได้ยอมรับเงื่อนไข" พร้อมลิงก์ทั้งสองโมเดล; network → "เชื่อมต่อไม่ได้ ลองใหม่" |
| หลังโหลดสำเร็จ | เสนอให้ลบ token ออกจาก Credential Manager เพราะรอบต่อไปใช้ cache แบบ offline (ตาม `a_cached_model_needs_no_token`) |
| ลิงก์ภายนอก | เปิดหน้า Hugging Face ด้วย `tauri-plugin-opener` ใน browser ของระบบ; ไม่ฝังหน้า login ในแอป |

## 6. UX (ร่าง)

แสดงในที่ที่ `RecordingReview.tsx` ใช้ `diarizationStatus` อยู่แล้ว และในหน้าตั้งค่า:

1. สถานะ "ยังไม่ได้เปิดใช้" + ปุ่ม **เปิดใช้การแยกเสียงผู้พูด** + บอกขนาดดาวน์โหลดจริง
2. แถบความคืบหน้าดาวน์โหลด/ตรวจสอบ/ติดตั้ง พร้อมปุ่มยกเลิก
3. ขั้นยอมรับเงื่อนไข: ปุ่มเปิดหน้าโมเดลทั้งสอง, ช่องกรอก token, ปุ่ม **ตรวจสอบและดาวน์โหลดโมเดล**
4. สถานะ "พร้อมใช้" + ตำแหน่งเก็บไฟล์ + ปุ่มลบ

ทุกข้อความเป็นภาษาไทยและบอก "ขั้นต่อไป" ตามหลักเดียวกับ `DiarizationBlocker::detail`

## 7. Privacy และ security invariants

- คงทุกข้อใน parent spec §4 (ไม่ส่งเสียงขึ้น cloud, ผลเป็น anonymous speaker, ไม่ redistribute น้ำหนักโมเดล)
- egress ใหม่มีสองปลายทางเท่านั้น: GitHub Release asset ของ FUNG และ Hugging Face; ทั้งสองเกิดเมื่อผู้ใช้กดเองเท่านั้น
- โค้ดที่ดาวน์โหลดมาจะรันได้ก็ต่อเมื่อ hash ตรงกับค่าที่ compile ไว้ใน binary ที่ลงนามแล้ว
- token ไม่ปรากฏใน stdout/stderr ของ worker, log, telemetry หรือ error message

## 8. Open questions (ต้องตอบก่อนอนุมัติ)

1. **License audit ของทั้ง ~100 แพ็กเกจใน lockfile** — การแจก archive ถือเป็นการ redistribute;
   ต้องยืนยันว่าทุกแพ็กเกจอนุญาตให้แจก binary และรวมไฟล์ license ครบ (ตอนนี้ script เก็บแค่ของ pyannote.audio และ PyTorch)
2. ขนาด archive จริงของ CPU variant หลังบีบอัด (ต้องวัด) และจะรองรับ CUDA variant ด้วยไหม
3. ที่เก็บ archive: GitHub Release พอไหม หรือต้องมี mirror
4. ทำ code signing / Authenticode กับไฟล์ `.pyd`/`.dll` ใน archive ด้วยหรือพอแค่ hash pin
5. นโยบายเมื่อ lockfile เปลี่ยน (security update ของ torch): แอปเวอร์ชันใหม่ชี้ archive ใหม่ — จะลบของเก่าอัตโนมัติไหม
6. `pyannote.audio` 4.x มี pipeline ใหม่ที่อาจไม่ gated — ควรประเมินแยกว่าเปลี่ยนโมเดลแล้วตัดขั้นตอน token ได้ไหม (นอกขอบเขตเอกสารนี้)

## 9. Scope

### In scope

- CI job build + publish runtime add-on archive จาก lockfile เดิม
- Rust: download/verify/extract/remove add-on, ขยาย `probe()`, ตั้ง `PYTHONPATH`, เก็บ/อ่าน/ลบ token ผ่าน `keyring`
- `diarize.py`: โหมด `--fetch-only` และ error code แยก 401/403/network
- UI ตาม §6
- เอกสาร setup สำหรับผู้ใช้

### Out of scope

- เปลี่ยนโมเดลหรือ algorithm การแยกผู้พูด
- GPU/CUDA add-on (รอคำตอบ open question 2)
- macOS/Linux
- voice recognition หรือ voice profile

## 10. Acceptance criteria

- บนเครื่อง Windows ที่ติดตั้งจาก installer อย่างเดียว (ไม่มี Python บนเครื่อง ไม่มี source checkout)
  ผู้ใช้เปิดฟีเจอร์ได้ครบจากในแอป และงานถอดเสียงถัดไปได้ `Speaker 1`, `Speaker 2`
- archive ที่ hash ไม่ตรง, ดาวน์โหลดไม่ครบ หรือแตกไฟล์ล้ม ไม่ทำให้ probe รายงานว่าพร้อม และไม่ทิ้งไฟล์ครึ่งๆ
- token ผิด / ยังไม่ยอมรับเงื่อนไข / ไม่มีเน็ต แสดงข้อความต่างกันตาม §5
- ค้นหา token ใน log, GenesisBlockDB, `<data_root>` และ job event แล้วไม่พบ
- หลังโหลดโมเดลแล้ว ตัดเน็ตแล้วงานแยกผู้พูดยังทำงานได้
- ลบ add-on แล้ว probe กลับเป็น `dependencies_missing` และการถอดเสียงปกติยังทำงาน
- `cargo test`, Python worker tests, frontend tests และ `git diff --check` ผ่าน
- installer มีขนาดไม่เพิ่มเกิน 1 MB เมื่อเทียบกับก่อนทำ

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | First draft of the in-app opt-in flow for the diarization runtime and gated model. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-11-26 | draft | Proposed add-on archive, keyring token storage, fetch-only model check and UX; awaiting owner approval. | pending | Claude |
