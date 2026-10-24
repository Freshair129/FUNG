---
version: "0.2.0b"
created_at: "2026-08-09T00:00:00+07:00,ATHER"
last_update: "2026-10-25"
status: "draft"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "product-context"
  scope: "FUNG"
---

# Product

## Register

product

## Users

FUNG serves people who record and revisit long conversations, including meetings, interviews and personal working sessions. They work at a desktop with sensitive source audio, need to see capture health without hunting through the interface, correct transcript and speaker uncertainty, and produce exports that remain traceable to source evidence.

## Product Purpose

FUNG is a local-first meeting agent that listens, understands the current conversation, retrieves relevant evidence and helps people act while the meeting is taking place. It also preserves the conversation and produces useful summaries and follow-up work afterward.

Reliable capture, recovery, review and export are the foundation of this live assistance. Users choose the knowledge, tools and destinations available to the agent, including when material may leave their device.

## Key Concept — ผู้ช่วยที่ทำงานไปพร้อมกับการประชุม

ทิศทางผลิตภัณฑ์ที่ผู้ใช้ยืนยันเมื่อ **2026-10-25** คือให้ FUNG ช่วยให้การประชุม
เดินหน้าต่อได้ตั้งแต่ระหว่างสนทนา ผู้ใช้ไม่ต้องหยุดคุยเพื่อสลับไปค้นข้อมูลหรือ
รวบรวมบริบทเองทุกครั้ง

**ฟัง → เข้าใจ → ค้นข้อมูล → ลงมือทำ**

FUNG อยู่ในวงจรการทำงานระหว่างประชุม: ฟังบทสนทนาอย่างต่อเนื่อง รู้ว่าเรื่องใด
กำลังถูกพูดถึง ใช้ข้อมูลและเครื่องมือที่ได้รับสิทธิ์มาช่วยตอบ และรักษาที่มาของ
คำตอบไว้ให้ตรวจสอบได้ เมื่อประชุมจบจึงนำหลักฐานชุดเดียวกันมาสรุปการตัดสินใจ
เรื่องสำคัญ และงานที่ต้องติดตามต่อ

ส่วนนี้เป็น **canonical product direction** รายละเอียด implementation และ
ความพร้อมใช้งานจริงต้องตรวจจาก [Desktop progress](docs/Desktop/08-real-progress.md)
และรายงานตรวจรับที่อ้างถึง การบันทึกแนวคิดนี้ไม่ได้ปิด gate ของ feature หรือ release

### Core features

| ความสามารถหลัก | ผลลัพธ์ที่ต้องการให้ผู้ใช้ได้รับ | เอกสารเจ้าของรายละเอียด |
| --- | --- | --- |
| ฟังการประชุมแบบ real-time และแยกผู้พูด | เห็นบทสนทนาและ turn ของผู้พูดที่เชื่อมกับเวลา/ช่องเสียง แก้ชื่อหรือข้อมูลที่ยังไม่แน่ใจได้ | [Live transcription](docs/specs/2026-09-21-live-meeting-transcription-spec.md), [People and voice identity](docs/specs/2026-09-21-speaker-identity-domain-design.md) |
| เข้าใจบริบทระหว่างคุย | ติดตามหัวข้อ คำถาม การตัดสินใจ และข้อมูลที่ยังขาดจากบทสนทนาล่าสุด เพื่อเสนอความช่วยเหลือในจังหวะที่เหมาะสม | [Meeting Agent](docs/specs/2026-09-21-meeting-agent-participation-spec.md) |
| ค้นข้อมูลได้ทันทีเมื่อจำเป็น | ค้น Knowledge Base หรือแหล่งข้อมูลที่เชื่อมไว้ แล้วคืนคำตอบพร้อมแหล่งอ้างอิงและสถานะความสดของข้อมูล | [Knowledge and evidence](docs/specs/2026-09-21-meeting-knowledge-evidence-spec.md), [External retrieval](docs/specs/2026-08-11-live-meeting-external-retrieval-design.md) |
| เรียกเครื่องมือผ่าน MCP | ใช้เครื่องมือที่ผู้ใช้เปิดให้เพื่ออ่านข้อมูลหรือทำงานตามขอบเขต และแสดงผลลัพธ์/ความล้มเหลวของคำสั่งให้เห็น | [External MCP](docs/specs/2026-08-11-live-meeting-external-retrieval-design.md), [Product MCP roadmap](docs/plans/2026-10-20-product-mcp-feature-roadmap.md) |
| สรุปและติดตามงานหลังประชุม | สรุปประเด็นสำคัญ การตัดสินใจ และ action items ที่มีเจ้าของ/กำหนดเวลาหากมีข้อมูล รองรับการทบทวนแก้ไขและกลับไปยังหลักฐานต้นทาง | [Meeting intelligence domains](docs/architecture/MEETING_INTELLIGENCE_DOMAINS.md), [Desktop meeting workflow](docs/plans/2026-10-22-desktop-meeting-workflow-mvp.md) |

การแยก turn หรือช่องเสียงออกจากกันยังต้องแสดงความไม่แน่ใจของตัวบุคคลอย่างชัดเจน
การฟังแบบ real-time เป็นเป้าหมายด้านประสบการณ์และ latency ที่ต้องมีผลทดสอบรองรับ
ปัจจุบันมีโหมด CPU ที่ถอดเสียงหลัง Stop เพื่อรักษาความต่อเนื่องของการบันทึก
จึงต้องระบุ readiness ของ live transcript, speaker attribution และ agent แยกกัน

### ตัวอย่างประสบการณ์ที่เป็นแกนผลิตภัณฑ์

**“เอกสารตัวนี้อยู่ไหน?”**

1. FUNG รับคำถามจากบทสนทนาหรือคำสั่งผู้ใช้ และใช้บริบทระบุว่าอ้างถึงเอกสารใด
2. ค้นใน Knowledge Base/collection ที่ได้รับสิทธิ์ ถ้ากำกวมให้ขอข้อมูลเพิ่ม
3. นำชื่อเอกสาร ลิงก์ และส่วนที่เกี่ยวข้องกลับมาในผู้ช่วยระหว่างประชุม พร้อมอ้างอิง
   ผู้ใช้สามารถคุยต่อโดยไม่ต้องออกไปค้นด้วยตนเอง

**“ลูกค้ารายนี้มีสถานะเป็นอย่างไร?”**

1. ระบุลูกค้าที่ถูกพูดถึง โดยตรวจตัวระบุให้ตรงและไม่เดาจากชื่อซ้ำ
2. อ่านสถานะผ่าน CRM connector หรือ MCP tool ที่ตั้งค่าและได้รับสิทธิ์แล้ว
3. แสดงสถานะ แหล่งข้อมูล และเวลาที่ดึงข้อมูล หากเชื่อมต่อไม่ได้หรือข้อมูลไม่พอ
   ให้บอกสถานะนั้นและให้การบันทึกดำเนินต่อได้

ตัวอย่าง CRM เป็น use case เป้าหมาย ความพร้อมของ connector ต้องมี implementation
และผลตรวจรับของตัวเอง คำตอบในแอป การส่งเข้าห้องประชุม และการเขียนกลับระบบภายนอก
เป็นคนละ action ที่มีขอบเขตสิทธิ์และผลยืนยันของตัวเอง การอ่านข้อมูลสำเร็จไม่ถือว่า
ส่งข้อความหรือแก้ข้อมูลให้แล้ว

### MCP มีสองบทบาท

- **FUNG เป็น MCP client:** agent ของ FUNG เรียกเครื่องมือภายนอก เช่น Knowledge Base
  หรือ CRM เพื่อช่วยงานระหว่างประชุม
- **FUNG เป็น MCP server:** agent ภายนอกเข้าถึงความสามารถของ FUNG ตาม contract
  เพื่อสั่งงานหรือทดสอบ workflow; รายการคำสั่งที่ใช้งานได้จริงต้องตรวจจาก
  [Product MCP contract](docs/specs/2026-10-20-product-mcp-v2-contract.md)

ความพร้อมของบทบาทหนึ่งไม่ได้รับรองอีกบทบาทหนึ่ง แผนพัฒนาต้องเชื่อมแต่ละ
ความสามารถกับ contract, implementation และ acceptance ของมัน

### หลักออกแบบ feature และ flow ต่อจากนี้

- รักษาวงจร **ประชุม → บันทึก → ทบทวน → สรุปและติดตามงาน** ที่อนุมัติไว้
  พร้อมให้บริบทและผู้ช่วยทำงานได้ระหว่างขั้นประชุม/บันทึก
- แสดง recording health, live transcript readiness, agent/tool status และผลการทำงาน
  อย่างชัดเจน ผู้ใช้ต้องรู้ว่าเสียงถูกบันทึกอยู่หรือกำลังรอข้อมูลส่วนใด
- เชื่อมคำตอบและ action items กับหลักฐานต้นทาง แสดงสิ่งที่ยังไม่ทราบ และให้แก้ไขได้
- วัดประโยชน์จากการช่วยให้คำถามได้รับคำตอบและงานเดินหน้าระหว่างประชุม
  ควบคู่กับความครบถ้วนของเสียงและผลลัพธ์หลังประชุม

## Brand Personality

Calm, precise, tactile.

FUNG is a quiet professional instrument. It uses clear operational language, visible system status and physical-feeling controls to reduce uncertainty during long-running work. Intelligence is helpful but never theatrical, coercive or more certain than its evidence.

## Anti-references

- A generic SaaS dashboard with floating cards, decorative metrics or needless visual noise.
- A marketing landing page as the first product screen.
- Purple gradients, dark blue or slate-heavy inactive UI, neon accents and decorative gradient blobs.
- A crowded DAW with dense control chrome before the user has a recording to review.
- UI patterns or branding copied from Call.md, including its logo, product copy, VideoDB identity and Electron-specific interaction model.
- Any interface that presents AI inference, speaker identity or meeting coaching as a verified fact.

## Design Principles

1. **Protect the recording first.** Capture state, storage safety and recovery status remain visible while recording.
2. **Keep the evidence legible.** Timestamps, source state and uncertainty are usable working material, not hidden metadata.
3. **Make local normal.** Offline/local processing is a supported state, never a degraded blank screen.
4. **Reveal intelligence by consent.** Suggestions, external tools and data egress are explicit, scoped and reversible.
5. **Use familiar controls with deliberate material.** The interface may feel tactile, but it must not invent unfamiliar navigation or obscure action consequences.

## Accessibility & Inclusion

FUNG targets WCAG 2.2 AA contrast and keyboard-operable desktop workflows. Transcript editing, speaker renaming and export actions must have visible focus states and text labels. `prefers-reduced-motion` disables non-essential motion. Audio-only system feedback must have a visible equivalent, and Thai text must retain readable line height with zero added letter spacing.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Recorded the user-confirmed live meeting agent concept, five core capabilities, knowledge/CRM examples, MCP client/server distinction and readiness boundaries. |
| 0.1.0b | Added approved product context for FUNG UI and design-system work. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.0b | 2026-10-25 | draft | Added canonical product direction: listen → understand → retrieve → act during the meeting, followed by evidence-backed summary and follow-up. Detailed feature/release acceptance remains separate. | pending | Codex |
| 0.1.0b | 2026-08-09 | draft | Captured FUNG product register, user context, personality, anti-references and accessibility baseline. | N/A — uncommitted | ATHER |
