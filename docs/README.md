---
version: "0.2.0b"
created_at: "2026-08-20T00:00:00+07:00,ATHER"
last_update: "2026-10-16,Codex"
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
| [`../contracts/`](../contracts/) | Repository-root API, MCP และ data contracts |
| [`../schemas/`](../schemas/) | Repository-root SQL schema baselines |
| [`design/`](design/README.md) | Current wireframe/wireflow/userflow ownership and design history |
| [`FUNG_Design_System_v0.1.0/`](FUNG_Design_System_v0.1.0/README.md) | Shared design-system package and runtime-imported semantic tokens |
| [`brand-kit/`](brand-kit/README.md) | Brand assets and identity references |
| [`app/`](app/) | Web/landing product proposals and reference material |
| [`superpowers/`](superpowers/) | Supporting plan/spec workflow artifacts; read each file's status |
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
- Current UI journeys and wireframes: [Studio flow contract](design/2026-10-16-desktop-navigation-and-flows.md)
- Interface inventory: [UI_INTERFACE_INVENTORY.md](UI_INTERFACE_INVENTORY.md)
- Desktop status: [Desktop/08-real-progress.md](Desktop/08-real-progress.md)
- Mobile status: [Mobile/IMPLEMENTATION_STATUS.md](Mobile/IMPLEMENTATION_STATUS.md)
- Program roadmap: [master implementation plan](plans/2026-08-09-fung-master-implementation-plan.md)
- Current flow review: [2026-10-16 audit](verification/implementation-reports/2026-10-16-wireframe-wireflow-userflow-audit.md)

Internal tooling artifacts such as `.rwang-tasks/`, `.doc-graph.json` and
`.preflight-report.json` are not runtime or release acceptance. Their recorded
timestamps/content must be checked before reuse; this navigation repair does
not regenerate or certify their hashes.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-10-16 | candidate | Reconciled directory map and root contract/schema locations; linked current design/flow audit and restored status entry points. | working-tree | Codex |
| 0.1.1b | 2026-08-23 | candidate | Added document-control metadata, requirements, architecture, AI-system, and decision indexes. | pending | ATHER |
