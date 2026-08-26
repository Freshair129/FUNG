---
version: "0.1.0b"
created_at: "2026-08-26T19:00:00+07:00,Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67"
last_update: "2026-08-26T19:00:00+07:00,Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "complexity-rule"
  document_kind: "technical-remediation-specification"
  scope: "D-MVP-01 packaged Whisper runtime"
  language: "Thai"
  risk: "MEDIUM"
---

# D-MVP-R1-01 — Packaged Whisper Model Path Remediation

## สถานะและ authorization boundary

เอกสารนี้เป็น **candidate** เท่านั้น ยังไม่อนุญาตให้แก้ source, tests,
package/CI, config, schema, artifact หรือ external system จนกว่า Boss จะ
อนุมัติ `D-MVP-R1-01` อย่างชัดเจน หลัง implementation ต้องผ่าน Terra review
และ main agent final gate อีกครั้ง

Amendment นี้ต่อยอดจาก D-MVP-01 ที่อนุมัติแล้ว แต่ไม่แก้ไขหรือเปลี่ยน hash ของ
candidate เดิม:

- Base/HEAD: `8a6406e6513943e09447daeb3c6572aa41468b67`
- Approved D-MVP-01 document SHA-256:
  `3F8C7121BD1399696360670A86B5E4E7CC167482E4E0D0D8F0A011DA8B0A14A9`
- New remediation ID: `D-MVP-R1-01`
- Risk: `MEDIUM` — narrow cross-boundary runtime fix and executable Rust
  coverage; no schema migration, dependency, or stored-data rewrite

## Confirmed blocker

Packaged FUNG created a valid MSI and contained the bundled Python runtime,
`scripts/transcribe.py`, `model.bin`, and `config.json`. In the live packaged
WebView, production `import_and_transcribe` for Session 2
(`678e147d-ba0a-48db-9435-23a283360e46`) created job
`9e8969fa-7ff5-42de-91de-b79630b03b2e` but failed at `1%`:

```text
RuntimeError: Unable to open file 'model.bin' in model '\\?\C:\Users\freshair\AppData\Local\Temp\fung-mvp-uat-20260826-183002\PFiles\FUNG\.venv-whisper\models\small'
```

The identical model directory loaded successfully through the normal
`C:\...\.venv-whisper\models\small` path (`NORMAL_PATH_LOAD_OK`, exit `0`)
and failed with only the `\\?\` prefix added (exit `1`). The model is not
missing. The failed job is runtime evidence, not acceptance.

See [the RCA](../../.brain/rca/2026-08-26-packaged-whisper-verbatim-model-path.md)
for the complete evidence and escape analysis.

## Goal / เป้าหมาย

ทำให้ packaged Windows FUNG ส่ง bundled Whisper model path ที่ child
Python/faster-whisper/CTranslate2 เปิดได้อย่างแน่นอน โดยรักษา local-first
flow และ provenance ของ project/audio เดิมไว้ครบถ้วน และไม่ขยายขอบเขตไปยัง
Google Drive, cloud, auth หรือ release packaging อื่น

## Minimum surgical design

### D-MVP-R1-01.1 — Normalize only the child model path

แก้เฉพาะ `src-tauri/src/lib.rs` โดยเพิ่ม helper ขนาดเล็กที่ทำงานตรง
boundary ก่อน `FUNG_WHISPER_MODEL` ถูกส่งให้ child process:

| Input | Output |
|---|---|
| Local drive `\\?\C:\folder\model` | `C:\folder\model` |
| Verbatim UNC `\\?\UNC\server\share\model` | `\\server\share\model` |
| Ordinary `C:\folder\model`, relative/other ordinary path | ไม่เปลี่ยน |
| Non-Windows build | คง behavior เดิม หรือ compile เฉพาะ Windows ตาม implementation ที่เหมาะสม |

ข้อจำกัดของ helper:

- เป็น normalization สำหรับ **child-compatible model path เท่านั้น**
- ไม่ทำ general canonicalization, filesystem resolution, symlink traversal,
  case folding, separator rewrite หรือ existence-based rewrite
- ไม่แก้ `file_path`, `checksum`, `byte_size`, project storage path หรือ
  provenance ที่ persist ใน GenesisBlockDB
- ไม่แก้ `scripts/transcribe.py`
- ไม่เพิ่ม dependency

### D-MVP-R1-01.2 — Bind the helper to the actual worker path

จุดเรียกใช้ต้องเป็น path ที่มาจาก `bundled_whisper_model()` ก่อน
`command.env("FUNG_WHISPER_MODEL", ...)` ใน `run_python_worker()` เท่านั้น
เพื่อให้ทุก packaged Whisper child invocation ใช้ contract เดียวกัน โดยไม่
เปลี่ยน model selection, model size, GPU profile หรือ summary provider

### D-MVP-R1-01.3 — Executable Rust regression coverage

เพิ่ม tests ใน `src-tauri/src/lib.rs` เท่านั้น ครอบคลุม:

1. local-drive `\\?\C:\...` conversion
2. verbatim UNC `\\?\UNC\...` conversion
3. ordinary path unchanged
4. non-Windows conditional/unchanged behavior
5. bundled model path ที่ worker ใช้ต้องเป็น child-compatible path

Test ต้องตรวจ output ของ helper/worker-path contract โดยตรง ไม่ใช้เพียง
source regex หรือ file-presence assertion

## Exact proposed write set after approval

อนุญาตให้แก้ไขได้เฉพาะ:

- `D:\FUNG\src-tauri\src\lib.rs` — helper, worker binding, and adjacent Rust
  tests only

ห้ามแก้ไฟล์อื่น รวมถึง `scripts/transcribe.py`, `src-tauri` files อื่น,
`src/App.tsx`, `src/tauri.ts`, `tests/`, `package.json`, package/CI, config,
schema/migrations, candidate docs, release artifacts, Git state หรือ external
systems

## Acceptance Criteria (AC)

| ID | เกณฑ์ยอมรับ |
|---|---|
| AC-R1-01 | Windows local-drive verbatim model path ถูกแปลงเป็น drive path ปกติก่อนส่งให้ child |
| AC-R1-02 | Windows verbatim UNC model path ถูกแปลงเป็น UNC path ปกติก่อนส่งให้ child |
| AC-R1-03 | Ordinary path ไม่เปลี่ยน และ non-Windows behavior ไม่ถูกเปลี่ยนโดยไม่จำเป็น |
| AC-R1-04 | Rust executable tests ครบทั้ง 5 กลุ่มด้านบนและผ่าน |
| AC-R1-05 | `FUNG_WHISPER_MODEL` ที่ worker ได้รับมาจาก helper path นี้โดยตรง; ไม่มีการแก้ stored project/audio provenance |
| AC-R1-06 | ไม่มีการแก้ `scripts/transcribe.py`, dependency, schema, package/CI หรือ external state |
| AC-R1-07 | หลัง rebuild packaged artifact, model preflight ผ่าน และ import ไฟล์เดิมเข้า Session 2 หรือ clean test project เดินเกิน model-load threshold ที่ `>=5%` และจบสำเร็จ |
| AC-R1-08 | หลัง import สำเร็จมี `active_recording_id`, transcript view, summary/export และ restart identity ครบถ้วน |

## Success Criteria

ผู้ใช้สามารถ import ไฟล์ local เดิมใน packaged Windows FUNG ได้ โดย worker
เปิด bundled Whisper model, progress ผ่าน `>=5%`, transcription complete และ
ผลลัพธ์ยังผูกกับ recording เดิมอย่าง recording-scoped: project active pointer,
transcript, Minute of Note/summary, local export และ identity หลัง restart
ตรวจสอบย้อนกลับได้ตรงกัน

## Exit Criteria

งานนี้จึงจะปิดได้เมื่อครบทุกข้อ:

- Terra review ของ implementation เป็น PASS และไม่เหลือ P0–P3 finding ใน
  ขอบเขตนี้
- Rust tests ใหม่และ relevant regression ผ่าน พร้อม `cargo fmt --check`
  และ `git diff --check`
- `npm run build` และ existing D-MVP-01 focused tests ยังผ่าน
- Rebuild packaged artifact ได้ และระบุ artifact identity/hash ตามหลักฐานจริง
- Live packaged runtime ใช้ไฟล์เดิมหรือ fixture ที่เทียบเท่า: model preflight
  ผ่าน, progress `>=5%`, import complete, `active_recording_id` ถูกต้อง,
  transcript view แสดงผล, summary/export เปิดได้ และ restart ไม่เปลี่ยน
  identity
- รายงานแยกชัดเจนระหว่าง implementation/local tests, packaged runtime/UAT,
  และ release packaging; ห้ามใช้ failed job เป็น acceptance

ถ้าข้อ runtime ใดทำไม่ได้ ให้คงสถานะ `runtime/UAT-open` ไม่รายงานว่า PASS

## Dependencies

- Tauri packaged `resource_dir()` และ existing `bundled_whisper_model()` path
  contract
- Bundled `.venv-whisper` Python runtime, faster-whisper/CTranslate2,
  `model.bin`, `config.json`, and `scripts/transcribe.py`
- Existing `run_python_worker()` and `FUNG_WHISPER_MODEL` child environment
- Windows packaged runtime/test environment with access to the extracted MSI
- Existing GenesisBlockDB project/recording/transcript/summary/export contract
  from approved D-MVP-01

ไม่ต้องใช้ Google Drive OAuth, Supabase login, cloud credential, new crate,
schema migration หรือ package dependency

## Ownership and review gates

| Role | Responsibility |
|---|---|
| PIC — Luna | เขียน implementation และ tests ใน `src-tauri/src/lib.rs` หลัง approval; เก็บ RED → GREEN evidence |
| Terra | Review source, tests, scope, and runtime evidence; ต้อง PASS ก่อน final gate |
| Main agent | Final gate: ตรวจ diff/hash/scope, ผลทดสอบ, truth-status และไม่ claim UAT เกินหลักฐาน |
| Approver — Boss | อนุมัติ `D-MVP-R1-01` และ source/test write set เท่านั้น |

## Out of scope (explicit)

- แก้ `scripts/transcribe.py` — หากจำเป็นให้เสนอ candidate แยกและขอ approval
- NSIS `makensis Internal compiler error #12345`; เป็น packaging follow-up
  แยกจาก transcription root cause และไม่อยู่ใน D-MVP-R1-01
- MSI/NSIS packaging redesign, signing, publication, deployment, or release
  readiness claim
- Google Drive OAuth, Supabase/auth, IAM/handshake, cloud providers,
  credentials, pairing, FUNGWIRE, MCP/CLI, Android/device proof, Hyper-V/ISO
- เปลี่ยน model, model quality, diarization, speaker recognition, GPU policy,
  CUDA bundle, model download, or model storage format
- เปลี่ยน persisted project/audio paths, checksums, provenance, schema,
  migrations, GenesisBlockDB, summary/export semantics, or UI
- การลบ/แก้ dirty files อื่น หรือการ commit/push/PR/merge

## Rollback

Rollback คือ revert เฉพาะ change ของ `D-MVP-R1-01` ใน
`src-tauri/src/lib.rs` กลับไปยัง base/approved D-MVP-01 state
`8a6406e6513943e09447daeb3c6572aa41468b67` โดยไม่ใช้ destructive Git command
กับ dirty worktree อื่น หาก runtime regression เกิดขึ้นให้หยุดการ promotion,
เก็บ failed evidence และเปิด candidate ใหม่สำหรับทางเลือกที่แยกขอบเขต

## Approval command

เนื่องจาก SHA-256 ของไฟล์จะเปลี่ยนทันทีหากนำ hash ของไฟล์ตัวเองไปใส่ในเนื้อหา
จึงต้องใช้ค่า post-write จาก delivery receipt ด้านล่าง/ข้อความส่งมอบเป็น
integrity input ของคำสั่งอนุมัติ ห้ามเดา hash หรือใช้ hash ของ base commit:

```text
approve D-MVP-R1-01 only — base/HEAD 8a6406e6513943e09447daeb3c6572aa41468b67 — RCA SHA-256 <POST_WRITE_RCA_SHA256> — remediation SHA-256 <POST_WRITE_REMEDIATION_SHA256> — source/test write set src-tauri/src/lib.rs only
```

การอนุมัติคำสั่งนี้ไม่อนุญาตให้แก้ไฟล์อื่น และไม่ปิด packaged runtime/UAT,
NSIS packaging หรือ release gate โดยอัตโนมัติ

## Version Diff

- `new -> 0.1.0b`: Proposed the minimum Windows child model-path
  normalization and executable Rust coverage for the confirmed packaged
  Whisper blocker. Implementation awaits explicit Boss approval.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-26 | candidate | Candidate remediation for packaged Whisper Windows verbatim-path incompatibility; source/test write set is `src-tauri/src/lib.rs` only. | `8a6406e6513943e09447daeb3c6572aa41468b67` | Luna |
