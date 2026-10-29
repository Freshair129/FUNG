---
version: "0.4.1b"
created_at: "2026-08-20"
last_update: "2026-10-25"
status: "beta"
superseded_by: null
attributes:
  domain: "documentation-governance"
  doc_type: "documentation-index"
  scope: "FUNG documentation"
  language: "Thai-first"
---

# FUNG documentation

หน้านี้เป็นจุดเริ่มต้นของเอกสาร FUNG ใช้หาเอกสารเจ้าของข้อมูลและแยกเอกสารปัจจุบันออกจากข้อเสนอ หลักฐานทดสอบ และประวัติการออกแบบ

## เริ่มจากตรงนี้

Product scope อยู่ใน [PRODUCT.md](../PRODUCT.md); RCA ย้อนหลังของ repository อยู่ใน [RCA archive](../.brain/README.md).

| ต้องการดู | เอกสารหลัก | ใช้ตอบคำถาม |
| --- | --- | --- |
| แนวคิดผลิตภัณฑ์และ core features | [Product concept](../PRODUCT.md#key-concept--ผู้ช่วยที่ทำงานไปพร้อมกับการประชุม) | ฟัง → เข้าใจ → ค้นข้อมูล → ลงมือทำระหว่างประชุม พร้อมขอบเขต readiness |
| ภาพรวมหน้าจอและสถานะฟีเจอร์ | [UI interface inventory](design/interface-inventory.md) | มี surface ใดบ้าง และสถานะ implementation ระบุไว้อย่างไร |
| หน้าจอ Desktop และ user flow ปัจจุบัน | [Studio flow contract](design/2026-10-16-desktop-navigation-and-flows.md) | sitemap, navigation, wireflow และ journey ปัจจุบัน |
| เอกสาร Desktop และสถานะล่าสุด | [Desktop index](Desktop/README.md), [Desktop real progress](Desktop/08-real-progress.md) | โครงสร้างเอกสาร Desktop และหลักฐาน/gate ล่าสุด |
| เอกสาร Mobile และสถานะ | [Mobile index](Mobile/README.md), [Mobile implementation status](Mobile/IMPLEMENTATION_STATUS.md) | เอกสาร Mobile, build, automated checks, device UAT และข้อจำกัด |
| ขอบเขตระบบ | [Desktop architecture](Desktop/ARCHITECTURE.md) และ [architecture index](architecture/README.md) | ส่วนประกอบ ข้อมูล runtime และขอบเขตระหว่างระบบ |
| ข้อกำหนด | [Requirements index](requirements/README.md) | แหล่ง requirement ที่เป็น canonical และวิธีเพิ่ม requirement |
| Design system | [Design system guide](FUNG_Design_System_v0.1.0/DESIGN_SYSTEM.md), [canonical DTCG tokens](FUNG_Design_System_v0.1.0/tokens/fung.tokens.json) และ [design board](design-system/index.html) | token และแนวทาง component; board เป็นไฟล์ generated |

## แผนผังเอกสาร

| ตำแหน่ง | หน้าที่และขอบเขต |
| --- | --- |
| `Desktop/` | [Desktop index](Desktop/README.md) รวม product UX, architecture และสถานะ; `08-real-progress.md` เป็นหลักฐานสถานะ ไม่ใช่ release approval |
| `Mobile/` | [Mobile index](Mobile/README.md) รวม UX, technical design และ implementation evidence |
| `design/` | Flow contract, UX decisions, interface inventory และ design briefs; ตรวจ `status` ก่อนใช้เอกสารที่มีวันที่ |
| `FUNG_Design_System_v0.1.0/` | แหล่ง token และ design-system package ที่ผูกกับ implementation |
| `design-system/` | บอร์ดอ้างอิงที่สร้างจาก token; แก้ที่ source และ regenerate แทนแก้ HTML generated |
| `architecture/` | สารบัญมุมมอง architecture ข้ามโดเมน; architecture ปัจจุบันของ Desktop อยู่ใน `Desktop/ARCHITECTURE.md` |
| `requirements/` | Requirement ที่กำกับข้าม Desktop, Mobile, API, CLI, MCP และ AI |
| `specs/` | Feature specification และ technical design ที่ refine requirement; proposal ไม่เท่ากับอนุมัติ |
| `decisions/` | Decision records และเหตุผลของทางเลือกที่ตัดสินแล้ว |
| `plans/` | Master plan และแผนลงมือทำแบบมีวันที่; อ่านสถานะปัจจุบันจากเอกสารสถานะของแต่ละ platform |
| `ai-system/` | Data pipeline, model lifecycle, evaluation และ governance ของ AI |
| `verification/` | รายงาน audit, UAT, preflight และ archive หลักฐานเก่าตามวันที่และ scope เดิม; รายงาน implementation อยู่ใน [รายงาน index](verification/implementation-reports/README.md) |
| `releases/` | Release notes และ release-specific gates |
| `operations/` | Runbook สำหรับ deploy, demo และการปฏิบัติงาน |
| `app/` | [Web index](app/README.md), proposal และ reference ของเว็บ/landing |
| `brand-kit/` | Brand assets และ identity references |
| `appendices/` | Traceability และทะเบียนประกอบ |
| `superpowers/` | [Supporting index](superpowers/README.md) รวม artifacts จาก workflow วางแผน; ไม่ใช่ source ของ implementation status |

API, MCP และ data contracts อยู่ใน [`../contracts/`](../contracts/) ส่วน schema baselines อยู่ใน [`../schemas/`](../schemas/)

## ลำดับความน่าเชื่อถือ

1. **Current behavior:** source code และ contract ที่ใช้อยู่
2. **Requirement และ approved decision:** `requirements/`, `decisions/` และ specification ที่ระบุสถานะอนุมัติชัดเจน
3. **Implementation status:** Desktop และ Mobile status documents พร้อมหลักฐานที่อ้างถึง
4. **Design proposal และ dated plan:** ใช้เป็นเจตนาหรือแผนตามสถานะเอกสาร ไม่ถือเป็นหลักฐานว่าฟีเจอร์ทำงานแล้ว
5. **Historical report:** ใช้ตรวจประวัติ ณ วันที่และ revision ที่ระบุเท่านั้น

เอกสาร `draft`, `candidate` หรือ `superseded` ไม่ได้ยืนยันพฤติกรรมปัจจุบัน การมี UI หรือ source code ก็ไม่เท่ากับผ่าน runtime, device, provider, native หรือ release acceptance

## วงจรเอกสาร

```text
ปัญหา/เป้าหมาย → requirement → specification → decision/architecture
                  → implementation plan → implementation → verification → release
```

กติกาการเขียนและ metadata อยู่ใน [Documentation contribution standard](CONTRIBUTING.md) สารบัญเฉพาะหมวดอยู่ที่ [plans](plans/README.md), [specs](specs/README.md), [verification](verification/README.md), [operations](operations/README.md) และ [releases](releases/README.md)

## Compatibility pointers และเครื่องมือเอกสาร

เอกสารที่ย้ายหมวดอาจมีหน้า pointer ที่ path เดิมเพื่อไม่ให้ลิงก์เก่าขาด ให้ใช้ path ปลายทางที่ pointer ระบุเมื่อเพิ่มลิงก์ใหม่

ไฟล์ที่ย้ายยังมี compatibility pointer ตามรายการนี้ เพื่อให้ path เดิมเปิดไปยังเอกสารหลักได้:

| Path เดิม | เอกสารหลัก |
| --- | --- |
| `UI_INTERFACE_INVENTORY.md` | [design/interface-inventory.md](design/interface-inventory.md) |
| `DOC_PREFLIGHT_2026-08-11.md` | [verification/preflight/2026-08-11-doc-preflight.md](verification/preflight/2026-08-11-doc-preflight.md) |
| `implementation-plan.md` | [plans/2026-08-11-live-meeting-external-retrieval-implementation.md](plans/2026-08-11-live-meeting-external-retrieval-implementation.md) |
| `PRESENTATION_RUNBOOK_2026-07-24.md` | [operations/demos/2026-07-24-presentation-runbook.md](operations/demos/2026-07-24-presentation-runbook.md) |
| `WEB_PRODUCTION_DEPLOYMENT.md` | [operations/web-production-deployment.md](operations/web-production-deployment.md) |
| `FUNG LOGO - Quiet Archive.png` | [brand-kit/logo/Quiet Archive.png](brand-kit/logo/Quiet%20Archive.png) — identical canonical copy; legacy path retained for historical references |
| `.preflight-report.json` | [verification/preflight/2026-08-26-doc-preflight-report.json](verification/preflight/2026-08-26-doc-preflight-report.json) |
| `.rwang-progress.md` | [verification/archive/documentation-maintenance/rwang-progress.md](verification/archive/documentation-maintenance/rwang-progress.md) |
| `.rwang-tasks/<file>.md` | [verification/archive/documentation-maintenance/rwang/](verification/archive/documentation-maintenance/rwang/README.md) — แต่ละ path เดิมมี pointer ของตัวเอง |

รายงาน preflight และคลัง documentation maintenance เป็นหลักฐานย้อนหลังตามวันที่และ scope เดิม ไม่ใช่ current product truth หรือหลักฐาน release โดยตัวมันเอง. ระบบ relationship snapshot แบบสร้างอัตโนมัติเดิมถูกถอนออกแล้ว การติดตามปัจจุบันให้ใช้ [Appendix D](appendices/D-traceability.md), annotation contract (`npm run test:traceability`) และ preflight ตามขอบเขตของแต่ละ gate

## CHANGELOG

| Version | Date | Status | Summary |
| --- | --- | --- | --- |
| 0.4.0b | 2026-10-19 | beta | ย้าย interface inventory และ audit/preflight archives เข้าหมวดเจ้าของ อัปเดต index และคง compatibility pointers ที่ path เดิม |
| 0.3.0b | 2026-10-19 | beta | จัด navigation ใหม่ ระบุ source-of-truth order และแยกเอกสารปฏิบัติงาน/หลักฐานออกจากข้อเสนอ |
| 0.2.0b | 2026-10-16 | candidate | Reconciled directory map and root contract/schema locations; linked current design/flow audit and restored status entry points. |
| 0.1.1b | 2026-08-23 | candidate | Added document-control metadata, requirements, architecture, AI-system, and decision indexes. |
