---
version: "0.1.0b"
created_at: "2026-10-19"
last_update: "2026-10-19"
status: "beta"
superseded_by: null
attributes:
  domain: "documentation-governance"
  doc_type: "contributor-guide"
  scope: "FUNG documentation"
  language: "Thai-first"
---

# Documentation contribution standard

ใช้กติกานี้กับเอกสารใหม่และเอกสารที่แก้สาระสำคัญ การปรับเอกสารเก่าทั้ง repository ทำแบบมีขอบเขตตามงาน ไม่เปลี่ยนประวัติหรือหลักฐานเดิมเพียงเพื่อให้รูปแบบเหมือนกัน

## เลือกตำแหน่งก่อนสร้างไฟล์

| เนื้อหา | ตำแหน่ง |
| --- | --- |
| Cross-platform requirement | `requirements/` |
| Requirement เฉพาะ Desktop ที่ยังไม่ได้ migrate | `Desktop/` พร้อมระบุว่าเป็น canonical source |
| Feature contract หรือ technical design | `specs/` หรือ platform folder ที่เป็นเจ้าของ feature |
| ทางเลือกทางสถาปัตยกรรมที่ตัดสินแล้ว | `decisions/` |
| Architecture view | `architecture/` หรือ `Desktop/ARCHITECTURE.md` ตามขอบเขต |
| Implementation plan | `plans/` |
| Test, review, audit, UAT evidence | `verification/` |
| Release note และ release gate | `releases/` |
| Deploy, demo และ operational procedure | `operations/` |
| AI data/model/evaluation/governance | `ai-system/` |
| UX flow, wireframe และ visual direction | `design/`, `Desktop/`, `Mobile/` หรือ design-system source ตาม owner |

ถ้ามีเอกสารเจ้าของเดิม ให้เพิ่มรายละเอียดใน source of truth นั้นหรือ link กลับไป ห้ามสร้างสำเนา requirement, status หรือ contract ที่อาจ drift

## ชื่อไฟล์และลิงก์

- เอกสารที่เปลี่ยนต่อเนื่องใช้ชื่อคงที่และมี index เจ้าของ เช่น `README.md`, `ARCHITECTURE.md`, `IMPLEMENTATION_STATUS.md`.
- เอกสารที่บันทึกเหตุการณ์หรือ snapshot ใช้ `YYYY-MM-DD-topic-in-kebab-case.md`.
- หลีกเลี่ยงชื่อกำกวม เช่น `new.md`, `final2.md`, `implementation-plan.md`; ระบุ feature และวันที่ให้ค้นเจอ.
- ใช้ relative Markdown links สำหรับไฟล์ใน repo และอัปเดตทุก link เมื่อย้ายเอกสาร.
- เอกสารที่ย้ายแต่มีผู้ใช้งาน path เดิมให้เก็บ compatibility pointer สั้น ๆ ที่ path เดิม; เนื้อหาจริงมีอยู่แห่งเดียว.
- อย่าแก้ generated HTML/JSON ด้วยมือ ให้แก้ source และใช้ generator ที่กำหนด.

## Metadata

เอกสารใหม่ต้องมี YAML front matter ตามแบบนี้:

```yaml
---
version: "0.1.0b"
created_at: "YYYY-MM-DD"
last_update: "YYYY-MM-DD"
status: "draft"
superseded_by: null
attributes:
  domain: "<owning domain>"
  doc_type: "<requirement|specification|decision|architecture|plan|verification|runbook|index>"
  scope: "<bounded scope>"
  language: "Thai-first"
---
```

ใช้วันที่ ISO `YYYY-MM-DD` สำหรับเอกสารใหม่ และปรับ `last_update` เมื่อแก้สาระสำคัญ เอกสารเก่าที่มี metadata อีกรูปแบบไม่ต้องเปลี่ยนย้อนหลัง เว้นแต่งานนั้นต้อง reconcile เนื้อหาอยู่แล้ว

## สถานะเอกสารกับสถานะฟีเจอร์

สถานะเอกสารอธิบายความพร้อมของเอกสาร ส่วนสถานะฟีเจอร์อธิบาย implementation; ห้ามใช้แทนกัน

| Document status | ความหมาย |
| --- | --- |
| `draft` | กำลังร่างและต้องทบทวน |
| `candidate` | เสนอเพื่อพิจารณา ยังไม่ใช่ข้อกำหนดที่อนุมัติ |
| `beta` | ใช้อ้างอิงในขอบเขตที่ระบุ แต่ยังมี gate หรือข้อจำกัด |
| `stable` | เอกสารผ่านการทบทวนและเป็นแนวทางปัจจุบัน |
| `historical` | บันทึกสถานะ ณ เวลาหนึ่ง ห้ามอ้างเป็น current truth |
| `superseded` | มีเอกสารใหม่แทน; เติม `superseded_by` ให้ตามได้ |

คำว่า `IMPLEMENTED`, `PARTIAL`, `SPECIFIED`, `VERIFIED` และ `NOT PROVEN` เป็น implementation/evidence labels ในเอกสารที่กำหนด ไม่ใช่สถานะเอกสาร

## เนื้อหาขั้นต่ำตามชนิดเอกสาร

- **Requirement/specification:** actor, behavior, scope, non-goals, dependencies, measurable acceptance, privacy/security/accessibility impacts และ source requirement IDs.
- **Decision:** context, considered options, decision, consequences, owner และวันที่.
- **Plan:** prerequisite, bounded tasks, dependencies, verification command/evidence และ exit gates.
- **Verification report:** revision/commit, environment, exact command or interaction, result, artifacts, scope limits และ remaining gates. แยก source/CPU/browser evidence จาก native/device/provider/production acceptance.
- **Runbook:** prerequisite, ordered steps, expected result, recovery/rollback และเจ้าของระบบ.

## การเปลี่ยนเอกสาร

1. อัปเดตเอกสารเจ้าของและ index ใน commit เดียวกัน.
2. ระบุ current-state truth กับ target/proposal ให้แยกกัน.
3. เมื่อล้าสมัย ให้ทำเครื่องหมาย `historical` หรือ `superseded` และชี้ไปยัง source ใหม่ แทนลบเอกสารที่ยังถูกอ้างอิง.
4. ตรวจ link ที่ได้รับผลจากการย้าย และดู `git diff --check`; รายงานสถานะการตรวจตามที่รันจริง.
