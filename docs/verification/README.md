---
version: "0.1.0b"
created_at: "2026-10-19"
last_update: "2026-10-19"
status: "beta"
superseded_by: null
attributes:
  domain: "verification-governance"
  doc_type: "documentation-index"
  scope: "FUNG test, review, audit and UAT evidence"
  language: "Thai-first"
---

# FUNG verification evidence

เอกสารในหมวดนี้บันทึกผลตรวจตามวันที่ revision และขอบเขตที่ระบุ รายงานหนึ่งฉบับไม่รับรอง feature อื่นหรือ revision ใหม่โดยอัตโนมัติ

## เริ่มจากหลักฐานปัจจุบัน

- [Desktop implementation truth](../Desktop/08-real-progress.md) สรุปผล Desktop และ remaining gates.
- [Mobile implementation truth](../Mobile/IMPLEMENTATION_STATUS.md) สรุป build, automated checks และ physical-device gaps.
- [Core audio UAT — 2026-10-18](implementation-reports/2026-10-18-core-audio-uat.md)
- [Wireframe/wireflow/userflow audit — 2026-10-16](implementation-reports/2026-10-16-wireframe-wireflow-userflow-audit.md)
- [Issue #41 recovery RCA — 2026-10-15](implementation-reports/2026-10-15-issue-41-recovery-rca.md)
- [Documentation path and orphan-link audit — 2026-10-19](2026-10-19-documentation-path-and-orphan-audit.md)
- [Preflight reports](preflight/README.md) เป็น snapshot การตรวจเอกสาร; ใช้วันที่และข้อจำกัดในรายงาน ไม่ถือเป็น current runtime acceptance.

## รายงานตามชนิดหลักฐาน

| Evidence type | สิ่งที่พิสูจน์ได้ | สิ่งที่ยังต้องรายงานแยก |
| --- | --- | --- |
| Source review | โครงสร้างและ logic ที่อ่านจาก revision ที่ระบุ | พฤติกรรมขณะรัน |
| Unit/CPU test | เงื่อนไขที่ test นั้นรันและ assertions ที่ผ่าน | GPU, native UI, device, provider หรือ production |
| Browser/UI | route, viewport, DOM และ interaction ที่บันทึกไว้ | native shell, OS permissions, real audio/device |
| Native/device UAT | interaction จริงบน app/device ที่ระบุ | provider, clean-install, recovery หรือ release gates ที่ไม่ได้รัน |
| Provider/production | integration กับ service/environment ที่ระบุ | platform/feature อื่นนอก scope |

## ที่เก็บรายงาน

- `implementation-reports/` — dated implementation, test, review, layout และ UAT reports; `progress.md` เป็น ledger ตามเวลาที่บันทึก ไม่ใช่ source of truth ปัจจุบัน.
- `preflight/` — documentation preflight snapshots.
- `archive/` — historical audit records and maintenance ledgers; use the index in that directory to follow moved records and compatibility pointers.
- `2026-09-23-fung-gap-analysis.md` — cross-domain gap analysis snapshot.

## รูปแบบรายงานใหม่

ใส่วันที่และหัวข้อในชื่อไฟล์ `YYYY-MM-DD-<scope>-<evidence>.md` และบันทึก commit/revision, environment, คำสั่งหรือขั้นตอนจริง, ผล, artifact path, ขอบเขตที่ไม่ได้ตรวจ และ remaining gates ตาม [contribution standard](../CONTRIBUTING.md)

## Catalogs

- [Implementation and review report index](implementation-reports/README.md) lists dated reports by filename and their recorded status.
