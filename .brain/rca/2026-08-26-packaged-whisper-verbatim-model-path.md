---
version: "0.1.0b"
created_at: "2026-08-26T19:00:00+07:00,Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67"
last_update: "2026-08-26T19:00:00+07:00,Agent: Luna,Commit: 8a6406e6513943e09447daeb3c6572aa41468b67"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "complexity-rule"
  document_kind: "root-cause-analysis"
  scope: "D-MVP-01 packaged Whisper runtime"
  language: "Thai"
  risk: "MEDIUM"
---

# RCA — Packaged Whisper ใช้ Windows verbatim model path ไม่ได้

## สถานะความจริง (Truth status)

เอกสารนี้เป็น RCA candidate สำหรับ runtime blocker ที่พบหลัง D-MVP-01 ผ่าน
source/test review แล้ว ยังไม่มีการแก้ code หรืออนุมัติ remediation

| ขอบเขต | สถานะ | หลักฐาน |
|---|---|---|
| D-MVP-01 source/contract | ผ่าน review | Terra source/test review PASS; Node `15/15` และ focused `31/31`; `npm run build` ผ่าน; Rust `409/409`; `cargo fmt --check` และ `git diff --check` ผ่าน |
| Packaged artifact | สร้างได้บางส่วน | MSI ถูกสร้างและ extract ตรวจสอบได้; NSIS ล้มเหลวด้วย `makensis Internal compiler error #12345` ซึ่งเป็น packaging follow-up แยกต่างหาก |
| Packaged transcription runtime | **BLOCKED** | Import job ล้มเหลวที่ progress `1%` ขณะเปิด bundled model |
| D-MVP-01 acceptance/UAT | **ยังไม่ผ่าน/ยังเปิด** | หลักฐานนี้เป็น failed runtime evidence ไม่ใช่ completion หรือ UAT acceptance |

## Symptom / อาการ

เมื่อนำ packaged FUNG เปิดที่ `http://tauri.localhost/` แล้วเลือก project
Session 2 (`678e147d-ba0a-48db-9435-23a283360e46`) ซึ่งยังไม่มี active recording
และเรียก production Tauri IPC `import_and_transcribe` ด้วยไฟล์ทดสอบ import job
`9e8969fa-7ff5-42de-91de-b79630b03b2e` เริ่มทำงานแต่หยุดที่ progress `1%`

ข้อความ terminal ที่เกิดขึ้นตรง ๆ คือ:

```text
RuntimeError: Unable to open file 'model.bin' in model '\\?\C:\Users\freshair\AppData\Local\Temp\fung-mvp-uat-20260826-183002\PFiles\FUNG\.venv-whisper\models\small'
```

ผลคือ packaged import ยังไม่สามารถเดินต่อไปถึง transcript completion,
`active_recording_id`, transcript view, summary หรือ export ได้

## Evidence / หลักฐาน

### 1. Runtime artifact identity

- Approved D-MVP-01 base commit: `8a6406e6513943e09447daeb3c6572aa41468b67`
- Approved D-MVP-01 document SHA-256:
  `3F8C7121BD1399696360670A86B5E4E7CC167482E4E0D0D8F0A011DA8B0A14A9`
- Valid MSI: `D:\FUNG\src-tauri\target\release\bundle\msi\FUNG_0.1.0_x64_en-US.msi`
- MSI size: `1,984,553,088` bytes
- Administrative extraction root:
  `C:\Users\freshair\AppData\Local\Temp\fung-mvp-uat-20260826-183002\PFiles\FUNG`
- Extracted bundle contained all required model/runtime inputs:
  `fung.exe`, `.venv-whisper\Scripts\python.exe`, `scripts\transcribe.py`,
  `.venv-whisper\models\small\model.bin`, and `config.json`
- Packaged app opened without a login or cloud gate.

### 2. Reproduction and differential result

The bundled Python/faster-whisper runtime was invoked against the same model
directory in two path forms:

| Input path form | Result | Exit |
|---|---|---:|
| `C:\...\.venv-whisper\models\small` | `NORMAL_PATH_LOAD_OK` | `0` |
| `\\?\C:\...\.venv-whisper\models\small` | Same `RuntimeError` opening `model.bin` | `1` |

The model files therefore existed and were readable through the normal local
drive path. The failure is deterministic when only the Windows verbatim prefix
is added.

### 3. Code path evidence

- `src-tauri/src/lib.rs` `whisper_runtime()` around lines `144–163` obtains
  `app.path().resource_dir()` and builds the bundled runtime/script paths.
- `bundled_whisper_model()` around lines `188–191` derives the model path from
  that runtime root.
- `run_python_worker()` around lines `2490–2492` sets `FUNG_WHISPER_MODEL`
  without converting Windows verbatim syntax first.
- `scripts/transcribe.py` around lines `64–66` consumes `FUNG_WHISPER_MODEL`
  as the model default, and around line `191` passes `args.model` to
  `WhisperModel`.

### 4. What the run did and did not prove

- The live packaged WebView selected Session 2 and the production import IPC
  was invoked; this is stronger than a source-only check.
- Native file-dialog selection was blocked by the Computer Use automation
  environment, so the equivalent production IPC invocation was used. This is
  an interaction limitation, not evidence that the dialog or import command is
  absent.
- The failed job is evidence of a packaged model-path blocker. It is not an
  acceptance result, not a completed transcription, and not a runtime/UAT
  completion claim.

## Root Cause / สาเหตุราก

On the packaged Windows path, Tauri's `resource_dir()` can return a Windows
verbatim path beginning with `\\?\`. FUNG derives the bundled Whisper model
directory from that value and passes it unchanged through `FUNG_WHISPER_MODEL`
to the Python child process. The CTranslate2 model loader used by
`faster-whisper` does not accept this verbatim path form for the model
directory, even though `model.bin` and `config.json` are present.

ดังนั้น root cause คือ **path representation mismatch at the Tauri → child
process → CTranslate2 boundary**. ไม่ใช่ model หาย, ไม่ใช่ model quality,
ไม่ใช่ cloud/auth/login, และไม่ใช่ D-MVP-01 import-handoff finalizer

## Why this escaped detection / เหตุใดจึงหลุดการตรวจ

1. Existing worker preparation and release-layout GPU smoke proved that the
   staged runtime, model, CUDA files, and normal worker launch were available;
   they did not exercise the exact packaged `resource_dir()` Windows verbatim
   path through the CTranslate2 loader.
2. D-MVP-01 source and contract tests focused on persistence, recording scope,
   failure gating, build, and Rust regressions. Those checks cannot establish
   that a packaged child process can open a model from the installed bundle.
3. The model presence check saw the expected files, but file existence is not
   equivalent to loader compatibility for every Windows path namespace.
4. The native file dialog could not be automated in this environment, so the
   first live packaged run used an equivalent IPC invocation; this preserved
   the runtime evidence but did not provide a normal operator click-through.

## Proposed prevention

The candidate remediation `D-MVP-R1-01` should add one narrow Windows
child-process/model-path normalization boundary in `src-tauri/src/lib.rs`:

1. Convert local-drive `\\?\C:\...` to `C:\...` before setting
   `FUNG_WHISPER_MODEL`.
2. Convert verbatim UNC `\\?\UNC\server\share\...` to
   `\\server\share\...`.
3. Leave ordinary paths unchanged and keep non-Windows behavior unchanged or
   conditionally compiled as appropriate.
4. Normalize only the child-compatible model path. Never rewrite stored
   project, audio, checksum, or provenance paths.
5. Add executable Rust coverage for both conversions, ordinary-path identity,
   non-Windows conditional behavior, and the path actually selected for the
   bundled worker.

After implementation, rebuild the packaged artifact and repeat the same-file
runtime flow. The acceptance sequence must observe model preflight success,
progress of at least `5%`, completion, recording activation, transcript view,
summary/export, and restart identity. Until that evidence exists, status stays
runtime/UAT-open.

## Scope boundary

This RCA covers only the packaged Whisper model-path failure. The following are
explicitly separate:

- NSIS `makensis Internal compiler error #12345` is an independent packaging
  follow-up and is not this transcription root cause or part of D-MVP-R1-01.
- Google Drive OAuth, login, cloud credentials, pairing, FUNGWIRE, Android,
  Hyper-V/ISO, speaker recognition, model-quality tuning, and production
  release claims are out of scope.
- The already-approved D-MVP-01 document and its hash must not be altered.

## Proposed status decision

`D-MVP-R1-01 = candidate / MEDIUM risk / implementation not authorized`.
Approval must be explicit and limited to the source/test write set described in
the remediation spec. Terra review and the main agent final gate remain
required after implementation.

## Version Diff

- `new -> 0.1.0b`: Documented the confirmed packaged Whisper verbatim-path
  blocker, evidence boundary, root cause, and minimum prevention proposal.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-26 | candidate | RCA for packaged CTranslate2 failure caused by an unnormalized Windows verbatim model path; remediation remains approval-gated. | `8a6406e6513943e09447daeb3c6572aa41468b67` | Luna |
