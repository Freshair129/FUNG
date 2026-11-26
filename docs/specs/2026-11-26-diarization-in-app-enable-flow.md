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
| `dependencies_missing` | ดาวน์โหลด **wheel ทุกตัวใน lockfile จาก PyPI โดยตรง** ตรวจ SHA-256 กับค่าที่ตรึงไว้ แล้วแตกไฟล์ลงโฟลเดอร์ข้อมูลของแอป |
| `model_not_fetched` | พาผู้ใช้ไปหน้าเงื่อนไขบน Hugging Face (เปิดใน browser ของเครื่อง) รับ token ผ่านช่องในแอป แล้วดาวน์โหลดโมเดลทันทีเพื่อตรวจว่าใช้ได้ |

ผู้ใช้ที่ไม่เปิดฟีเจอร์นี้จะไม่ต้องโหลดอะไรเพิ่ม และ installer มีขนาดเท่าเดิม

### ทางเลือกที่ไม่เลือก

- **ใส่ pyannote/torch มากับ installer:** installer ใหญ่ขึ้นเกิน 1 GB สำหรับทุกคน
  และยังต้องทำขั้นตอนยอมรับเงื่อนไข/token อยู่ดี
- **รัน pip ในแอป:** embedded Python ไม่มี pip โดยเจตนา (ดู parent spec §3)
  และการ resolve จาก PyPI ตอนรันทำให้ได้ชุดแพ็กเกจที่ไม่ได้ review
- **ใส่โมเดลมากับ installer:** ทำไม่ได้ เป็นเงื่อนไขอนุญาตของโมเดล
- **FUNG build archive รวมแล้วแจกเองผ่าน GitHub Release (ร่างแรก):** ทำให้ FUNG เป็นผู้แจกต่อ binary
  ทั้งหมด รวม Intel OpenMP (`libiomp5md.dll` เงื่อนไข proprietary, ไม่มี notice ใน wheel) และ GCC runtime
  (GPL-3.0 + exception) ตาม [licence audit](../verification/2026-11-26-diarization-runtime-licence-audit.md) §3
  และต้องมี CI สำหรับ build/host ไฟล์ขนาดใหญ่

## 4. Runtime add-on (ดาวน์โหลด wheel จาก PyPI)

| Item | Contract |
| --- | --- |
| แหล่งไฟล์ | wheel ตามชื่อไฟล์และ SHA-256 ใน `scripts/diarization-runtime-requirements.txt` (CPU variant) จาก `files.pythonhosted.org` — ตรวจแล้วเมื่อ 2026-11-26 ว่า hash ของ 96 wheel ตรงกับไฟล์บน PyPI ทุกตัว |
| ข้อยกเว้น sdist | `antlr4-python3-runtime` (BSD-3-Clause) และ `docopt` (MIT) มีแต่ source archive; CI build เป็น `py3-none-any` wheel แล้วแนบใน GitHub Release ของ FUNG (pure Python, permissive) |
| Manifest | build step สร้าง manifest (ชื่อไฟล์, URL, SHA-256, ขนาด) จาก lockfile แล้ว compile เข้า binary ของแอป; แอปไม่ resolve dependency เองและไม่อ่าน URL/hash จาก server |
| การตรวจสอบ | ทุกไฟล์ต้องตรง SHA-256 และขนาดใน manifest; ไม่ตรง = ลบไฟล์และหยุด |
| การติดตั้ง | แตก wheel (zip) ลง `site-packages` ตามรูปแบบ wheel spec; ไม่รันสคริปต์ใดๆ จากแพ็กเกจ; เก็บ `*.dist-info/licenses/` ไว้ครบ |
| ตำแหน่งติดตั้ง | `<data_root>/diarization-runtime/<lockhash>/site-packages` — ไม่เขียนลงโฟลเดอร์ติดตั้งของแอป เพราะอาจเขียนไม่ได้และจะถูกลบตอนอัปเดต |
| การโหลด | Rust ตั้ง `PYTHONPATH` ชี้โฟลเดอร์นี้ตอน spawn `diarize.py` เท่านั้น; worker อื่นไม่เห็น |
| ความเข้ากันได้ | wheel ผูกกับ `cp311-win_amd64`; แอปตรวจว่า `.venv-whisper` เป็น CPython 3.11 ก่อนเริ่ม |
| การติดตั้งแบบ atomic | ดาวน์โหลดเป็น `.partial` → ตรวจ hash → แตกลงโฟลเดอร์ชั่วคราว → rename เป็นโฟลเดอร์จริง; ล้มกลางทางไม่ทิ้งสถานะครึ่งๆ |
| Resume/ยกเลิก | ยกเลิกได้ระหว่างดาวน์โหลด; เริ่มใหม่โดยไม่โหลดซ้ำไฟล์ที่ hash ผ่านแล้ว |
| พื้นที่ดิสก์ | ตรวจพื้นที่ว่างก่อนเริ่ม (รวมขนาด wheel + ขนาดหลังแตก) ด้วย `GetDiskFreeSpaceExW` ที่มีอยู่แล้ว |
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
- egress ใหม่มีสามปลายทางเท่านั้น: `files.pythonhosted.org` (wheel), GitHub Release asset ของ FUNG (wheel สองตัวที่ build เอง)
  และ Hugging Face; ทั้งหมดเกิดเมื่อผู้ใช้กดเองเท่านั้น และ request ไม่มีข้อมูลผู้ใช้นอกจาก token ที่ส่งไป Hugging Face
- โค้ดที่ดาวน์โหลดมาจะรันได้ก็ต่อเมื่อ hash ตรงกับค่าที่ compile ไว้ใน binary ที่ลงนามแล้ว
- token ไม่ปรากฏใน stdout/stderr ของ worker, log, telemetry หรือ error message

## 8. Open questions (ต้องตอบก่อนอนุมัติ)

1. **License audit ของทั้ง ~100 แพ็กเกจใน lockfile** — การแจก archive ถือเป็นการ redistribute;
   ต้องยืนยันว่าทุกแพ็กเกจอนุญาตให้แจก binary และรวมไฟล์ license ครบ (ตอนนี้ script เก็บแค่ของ pyannote.audio และ PyTorch)
   — **ตรวจแล้ว:** [`2026-11-26-diarization-runtime-licence-audit.md`](../verification/2026-11-26-diarization-runtime-licence-audit.md)
   ไม่พบแพ็กเกจที่ห้ามแจก; Intel OpenMP และ GCC runtime ที่ต้องตัดสินใจหมดไปเมื่อเปลี่ยน §4 เป็นการดาวน์โหลดจาก PyPI
   โดยตรง (FUNG แจกเองแค่ wheel ของ `antlr4-python3-runtime` และ `docopt`) — **ให้เจ้าของโปรเจกต์ยืนยันข้อสรุปนี้**
2. ขนาดดาวน์โหลดรวมจริงของ CPU variant (torch wheel อย่างเดียว ~118 MB) และจะรองรับ CUDA variant ด้วยไหม
   (cu130 มาจาก `download.pytorch.org` ไม่ใช่ PyPI)
3. ถ้า PyPI ล่มหรือถูกบล็อกในเครือข่ายของผู้ใช้ จะมี fallback ไหม
4. พอแค่ hash pin หรือต้องตรวจลายเซ็น Authenticode ของ `.pyd`/`.dll` เพิ่ม (wheel ส่วนใหญ่ไม่ได้ลงนาม)
5. นโยบายเมื่อ lockfile เปลี่ยน (security update ของ torch): แอปเวอร์ชันใหม่ใช้ lockhash ใหม่ — จะลบโฟลเดอร์เก่าอัตโนมัติไหม
6. `pyannote.audio` 4.x มี pipeline ใหม่ที่อาจไม่ gated — ควรประเมินแยกว่าเปลี่ยนโมเดลแล้วตัดขั้นตอน token ได้ไหม (นอกขอบเขตเอกสารนี้)

## 9. Scope

### In scope

- build step ที่สร้าง manifest จาก lockfile และ CI job ที่ build/publish wheel ของ `antlr4-python3-runtime` กับ `docopt`
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
- wheel ที่ hash ไม่ตรง, ดาวน์โหลดไม่ครบ หรือแตกไฟล์ล้ม ไม่ทำให้ probe รายงานว่าพร้อม และไม่ทิ้งไฟล์ครึ่งๆ
- token ผิด / ยังไม่ยอมรับเงื่อนไข / ไม่มีเน็ต แสดงข้อความต่างกันตาม §5
- ค้นหา token ใน log, GenesisBlockDB, `<data_root>` และ job event แล้วไม่พบ
- หลังโหลดโมเดลแล้ว ตัดเน็ตแล้วงานแยกผู้พูดยังทำงานได้
- ลบ add-on แล้ว probe กลับเป็น `dependencies_missing` และการถอดเสียงปกติยังทำงาน
- `cargo test`, Python worker tests, frontend tests และ `git diff --check` ผ่าน
- installer มีขนาดไม่เพิ่มเกิน 1 MB เมื่อเทียบกับก่อนทำ

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | First draft of the in-app opt-in flow for the diarization runtime and gated model. Revised the same day after the licence audit: the runtime comes from hash-pinned PyPI wheels instead of a FUNG-hosted archive. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-11-26 | draft | Proposed direct PyPI wheel download (hash-pinned), keyring token storage, fetch-only model check and UX; awaiting owner approval. | pending | Claude |
