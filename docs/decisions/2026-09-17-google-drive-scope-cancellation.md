---
version: "0.1.0b"
created_at: "2026-09-17T00:00:00+07:00,Agent: Codex,Commit: working-tree"
last_update: "2026-09-17T00:00:00+07:00,Agent: Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-storage"
  doc_type: "decision"
  scope: "FUNG Google Drive workstream"
  language: "Thai"
---

# Google Drive Workstream Cancellation

## Decision

Boss ยืนยันว่า FUNG จะไม่ใช้ Google Drive ต่อไป การตัดสินใจนี้มีผลกับงานที่ยัง
ค้างและงานใหม่ทันที:

- หยุด implementation, test expansion, Google OAuth/provider setup,
  deployment, UAT และ release acceptance ที่มี Google Drive เป็นเป้าหมาย
- นำ Google Drive ออกจาก active roadmap และ acceptance criteria
- เก็บ source, test และเอกสารเดิมไว้เป็น historical/deprecated evidence ก่อน
  ไม่ลบไฟล์หรือข้อมูลภายนอกโดยอัตโนมัติ

## Scope Boundary

ยังคงทำได้เฉพาะ local filesystem backup/restore ที่ระบุเป็น development/test
และงาน mobile account/device ที่ไม่ผูกกับ Google Drive การนำ cloud provider อื่น
กลับมาอยู่ใน scope ต้องมี product decision ใหม่

งาน Google Drive เดิมไม่ใช่ product recommendation อีกต่อไป แม้ source และ
focused tests จะยังคงอยู่เพื่อ provenance และ rollback ของเอกสาร

## Follow-up

1. Supersede แผน `docs/plans/2026-08-13-phase-4-google-drive-backup-mobile-account.md`
2. แก้ master plan ให้ Phase 4 ไม่ถือ Google Drive เป็น acceptance gate
3. หยุด delegated task ใด ๆ ที่ทำ Google Drive โดยไม่ลบ worktree ที่มีงานอื่น
   ซึ่งไม่เกี่ยวข้อง เช่น D-MVP-04-L1

## Non-goals

- ไม่ลบ Google Drive implementation จาก `src/` หรือ `src-tauri/` ใน decision นี้
- ไม่ revoke OAuth/token หรือ remote archive เพราะไม่มีคำสั่งให้ทำลาย external
  state และยังต้องตรวจ ownership ก่อน
- ไม่ประกาศว่า local filesystem backup เป็น production cloud backup

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | ยกเลิก Google Drive workstream และแยก historical source/evidence ออกจาก active product scope |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Boss-directed cancellation of Google Drive implementation, provider, deployment and UAT work; historical source is retained pending a separate removal decision. | working-tree | Codex |
