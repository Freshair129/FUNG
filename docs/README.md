---
version: "0.1.1b"
created_at: "2026-08-20T00:00:00+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "documentation-governance"
  doc_type: "core-directive"
  scope: "FUNG documentation"
---

# FUNG Documentation

เอกสารของ FUNG แบ่งตามหน้าที่ของเอกสาร ไม่ผูกกับเครื่องมือหรือ workflow ใด workflow หนึ่ง

## Document map

| Directory | Purpose |
| --- | --- |
| `requirements/` | Canonical cross-domain functional, quality, data, security, and acceptance requirements |
| `architecture/` | Index of current, target, data, runtime, and deployment architecture views |
| `Desktop/` | Desktop product, architecture และ current-state evidence |
| `Mobile/` | Mobile product, UX และ implementation evidence |
| `specs/` | Feature requirements และ technical design proposals |
| `decisions/` | Architecture Decision Records และข้อเลือกที่อนุมัติแล้ว |
| `ai-system/` | AI/ML data lineage, model cards, evaluation, lifecycle, and governance |
| `plans/` | Implementation plans แบบ task-by-task |
| `verification/` | Test, review, audit และ delivery evidence |
| `contracts/` | API, MCP และ data contracts |
| `releases/` | Release notes และ release gates |
| `appendices/` | Traceability และ supporting material |

## Document lifecycle

```text
Requirement / problem
        ↓
requirements/ → specs/
        ↓ approval
decisions/ + architecture/ (เมื่อมี architectural impact)
        ↓
plans/
        ↓ implementation
verification/
```

กติกาหลัก:

- เอกสาร `candidate` หรือ `draft` เป็นข้อเสนอ ยังไม่ใช่ implementation truth
- เอกสาร architecture ต้องแยก current state ออกจาก target state
- ข้อกำหนดด้านคุณภาพ ความปลอดภัย และความเป็นส่วนตัวต้องระบุเป็นตัวชี้วัดหรือ acceptance criteria ที่ตรวจได้
- ห้ามคัดลอก requirement เดียวกันไปไว้หลายไฟล์โดยไม่มี source of truth
- ทุก plan ต้องอ้าง spec ที่อนุมัติแล้ว และทุก verification report ต้องอ้างผลตรวจที่ทำจริง
- ทุก model/provider change ต้องมี model card และ evaluation evidence ตาม
  `ai-system/` ก่อนใช้ถ้อยคำว่า high quality, approved, หรือ production-ready

## Primary entry points

- Parent architecture: `Desktop/ARCHITECTURE.md`
- Architecture index: `architecture/README.md`
- Requirements governance: `requirements/README.md`
- AI system governance: `ai-system/README.md`

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-08-23 | candidate | Added document-control metadata, requirements, architecture, AI-system, and decision indexes. | pending | ATHER |
- Desktop status: `Desktop/08-real-progress.md`
- Mobile status: `Mobile/IMPLEMENTATION_STATUS.md`
- Program roadmap: `plans/2026-08-09-fung-master-implementation-plan.md`
