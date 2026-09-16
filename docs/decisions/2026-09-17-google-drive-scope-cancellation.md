---
version: "0.2.0b"
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
- เก็บ migration/schema history และ verification reports ไว้เป็น
  historical/deprecated evidence; active source, commands, provider functions
  และ contract tests จะถูกลบตาม follow-up execution ด้านล่าง

## Scope Boundary

ยังคงทำได้เฉพาะ local filesystem backup/restore ที่ระบุเป็น development/test
และงาน mobile account/device ที่ไม่ผูกกับ Google Drive การนำ cloud provider อื่น
กลับมาอยู่ใน scope ต้องมี product decision ใหม่

งาน Google Drive เดิมไม่ใช่ product recommendation อีกต่อไป แม้ migration,
schema history และรายงาน verification จะยังคงอยู่เพื่อ provenance ของฐานข้อมูล
และเอกสาร

## Follow-up

1. Supersede แผน `docs/plans/2026-08-13-phase-4-google-drive-backup-mobile-account.md`
2. แก้ master plan ให้ Phase 4 ไม่ถือ Google Drive เป็น acceptance gate
3. หยุด delegated task ใด ๆ ที่ทำ Google Drive โดยไม่ลบ worktree ที่มีงานอื่น
   ซึ่งไม่เกี่ยวข้อง เช่น D-MVP-04-L1
4. ลบ active Google Drive adapter/UI/commands/Edge functions/contract tests แล้ว
   และตรวจให้ local backup/mobile account เป็น scope ที่เหลืออยู่

## Non-goals

- ข้อความเดิมที่ไม่ลบ implementation เป็น initial-decision boundary; follow-up
  ที่ผู้ใช้สั่งภายหลังได้ลบ active implementation แล้ว
- ไม่ revoke OAuth/token หรือ remote archive เพราะไม่มีคำสั่งให้ทำลาย external
  state และยังต้องตรวจ ownership ก่อน
- ไม่ประกาศว่า local filesystem backup เป็น production cloud backup

## Version Diff

| Version | Change |
|---|---|
| 0.2.0b | ลบ active implementation หลังการยกเลิก; เก็บเฉพาะ migration/schema/report history |
| 0.1.0b | ยกเลิก Google Drive workstream และแยก historical source/evidence ออกจาก active product scope |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-09-17 | beta | Removed active Google Drive adapter, UI, commands, Edge functions, and contract tests after the product cancellation; retained migration/schema/report history and did not change external state. | pending | Codex |
| 0.1.0b | 2026-09-17 | beta | Boss-directed cancellation of Google Drive implementation, provider, deployment and UAT work; active source was retained before the follow-up removal instruction. | working-tree | Codex |
