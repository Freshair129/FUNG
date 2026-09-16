---
version: "0.1.0b"
created_at: "2026-09-17T00:00:00+07:00,Agent: Codex,Commit: 64ceb222d0f5cb98a0f2e9c7bc283f6a6e8f5c51"
last_update: "2026-09-17T00:00:00+07:00,Agent: Codex,Commit: 64ceb222d0f5cb98a0f2e9c7bc283f6a6e8f5c51"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "root-cause-analysis"
  document_kind: "root-cause-analysis"
  scope: "FUNG Desktop D-MVP-04-L1 export-artifact inventory"
  language: "Thai"
  risk: "LOW"
---

# RCA — Export-artifact inventory ถูกตัดที่ Genesis read ceiling

## สถานะความจริง (Truth status)

นี่คือ RCA candidate สำหรับ bounded local-only read-path defect ที่แก้ใน
D-MVP-04-L1 แล้ว หลักฐานนี้ไม่ใช่ packaged/runtime/UAT, provider, device หรือ
release acceptance

| ขอบเขต | สถานะ | หลักฐาน |
|---|---|---|
| Source read path | แก้แล้วใน working tree | `list_export_artifacts` ใช้ `genesis_adapter::query_all` และคง filter/sort/serialization เดิม |
| Regression | ผ่าน | Focused `transcript_export` Rust tests `16/16`; fixture มี `ROW_CAP + 5` artifacts และ project อื่นสำหรับ isolation |
| Runtime/release | ยังเปิด | Full Rust run มี 6 FUNGWIRE transcription tests ที่เริ่ม worker ไม่ได้เพราะ actual `.venv-whisper\\Scripts\\python.exe` ไม่มีใน worktree |

## Symptom / อาการ

เมื่อ project มี export artifacts มากกว่า Genesis single-read bound รายการไฟล์ที่
`list_export_artifacts` ส่งให้ Desktop ไม่ครบ โดยไม่มี error หรือ completeness
signal เพิ่มเติม แถวหลัง read ceiling จึงไม่ปรากฏใน inventory ที่ใช้เปิดไฟล์
ส่งออก

## Evidence / หลักฐาน

1. ก่อนแก้ `list_export_artifacts` เรียก `genesis_adapter::query` พร้อม
   `SEGMENT_READ_CAP` ซึ่งเท่ากับ `ROW_CAP`; จึงอ่านได้เพียงหน้าเดียว
2. `genesis_adapter::query_all` มีอยู่แล้วและเป็น contract ที่ reader อื่นใช้
   อ่านตารางที่มีขนาดโตตามอายุการใช้งาน
3. Regression ใหม่สร้าง artifacts ของ project เดียวจำนวน `ROW_CAP + 5`, เพิ่ม
   artifact ของ project อื่น, แล้วตรวจ count, newest-first tail และ project scope;
   focused `cargo test --manifest-path src-tauri/Cargo.toml --lib transcript_export`
   ผ่าน `16/16`
4. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`,
   `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `npm run build`,
   relevant Node suites และ `git diff --check` ผ่าน

## Root Cause / สาเหตุราก

Reader ของ export-artifact inventory ถูกคงไว้บน bounded single-page query จาก
implementation เดิม ขณะที่การปิด Genesis 1000-row ceiling ก่อนหน้านี้ครอบคลุม
reader ที่ถูกตรวจแล้วแต่ไม่ได้ encode invariant นี้ใน `list_export_artifacts`
และไม่มี regression ที่มีข้อมูลเกิน `ROW_CAP`

ดังนั้น root cause คือ **read-path contract mismatch**: ตารางที่มีจำนวนแถวโตได้
ถูกอ่านด้วย API ที่ตั้งใจให้คืนเพียงหน้าเดียว แทนที่จะใช้ whole-read paging
`query_all` ไม่ใช่ปัญหาของ export writer, audio transcoder, provider หรือ
persisted-data schema

## Why this escaped detection / เหตุใดจึงหลุดการตรวจ

1. Existing artifact tests ใช้ fixture ขนาดเล็กกว่า `ROW_CAP` จึงให้ผลเหมือน
   single-page และ whole-read
2. การตรวจ 1000-row ceiling มุ่งที่ transcript/summary และ readers ที่อยู่ใน
   critical-path evidence เดิม ไม่ได้มี artifact-inventory count assertion
3. Packaged click-through และ long-session acceptance ยังเปิดอยู่ จึงยังไม่มี
   interaction evidence ที่แสดงไฟล์สะสมเกิน ceiling ใน packaged app

## Proposed prevention / การป้องกัน

1. กำหนดให้ reader ของตารางที่โตตามอายุการใช้งานใช้ `query_all` หรือ cursor
   contract ที่เทียบเท่า และให้ bounded `query` ใช้เฉพาะ read ที่ตั้งใจจำกัดหน้า
2. เพิ่ม regression ที่เกิน `ROW_CAP` พร้อมตรวจ project scope, ordering และ
   terminal row ให้กับ inventory ที่ผู้ใช้เห็น
3. บันทึก local source/test evidence แยกจาก packaged/runtime/provider/device/
   release gates ใน amendment และ Desktop progress ledger

## Version Diff

- `new -> 0.1.0b`: documented the export-artifact inventory read-ceiling root
  cause, evidence, escaped-detection path, and prevention after the bounded fix.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | candidate | Documented the D-MVP-04-L1 export-artifact inventory read-ceiling RCA and prevention controls. | working-tree | Codex |
