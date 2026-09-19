---
version: "0.2.1b"
created_at: "2026-09-17T02:06:26+07:00,Codex,c378af9fac3c00db063948f49f9ee857ebad9126"
last_update: "2026-09-20T04:15:00+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-product"
  doc_type: "approval-package"
  scope: "Call.md-inspired desktop P1 feature selection; no code authorization yet"
  complexity: "C-3"
  risk: "MEDIUM UI; HIGH native playback and recording-scope authority"
---

# ชุดอนุมัติ Call.md → FUNG Desktop P1

## Execution approval — ปัจจุบัน

Boss ตอบ `approve` อนุมัติชุด v0.2.0b นี้แล้ว: B ครบ, รูปแบบ SVG/PNG,
baseline แบบคง native custody และแผน backend/UI ขนานหลังตรวจ interface
ข้อเลือก/คำว่า proposal ด้านล่างเป็นประวัติชุดที่อนุมัติ ไม่ใช่คำถามที่ต้องถามซ้ำ
กำลังทำ baseline ใน isolated worktree และปรับ coherence เอกสารก่อน review
งานถัดไปต้องผ่าน dependency/review แต่ไม่ต้องขออนุมัติขอบเขตเดิมอีก
ดู [บันทึกอนุมัติ](../verification/implementation-reports/2026-09-17-callmd-approval.md)
ยังไม่อนุมัติ commit/push/release, schema/cloud/CSP expansion หรือยกเว้น security tests

## สถานะและสิ่งที่อนุมัติแล้ว

Boss อนุมัติ **workflow** ด้วยข้อความ `ap[prove` แล้ว จึงเปิด Luna `max`
สามสายทำสัญญา แบบ UI และเกณฑ์ทดสอบขนานกันได้
ต่อมา Boss ระบุ `B: UI ใหม่ พร้อมประวัติบันทึก` จึงบันทึกการเลือก UI + history
แล้ว ต่อมา Boss ตอบ `ทำคู่กันแบบบขนาน` ยืนยันรวม player WAV และ recording Q&A
ตามข้อเสนอ B แล้ว ไม่ต้องถามเลือกฟีเจอร์ซ้ำ แต่การแก้ CI เดิมยังเป็นขอบเขตแยก
เอกสารนี้รวบรวมการตัดสินใจที่เหลือ ไม่สร้างสิทธิ์เพิ่มเอง

ฐานงาน: `c378af9fac3c00db063948f49f9ee857ebad9126`
บน `codex/callmd-ui-dag` เท่านั้น ไม่มี commit/push/deploy ในรอบเอกสาร
Codex เป็น orchestrator; งานโค้ดหรือแก้ข้อผิดพลาดจะส่งให้ Luna หลังได้รับอนุมัติ

## เอกสารและภาพสำหรับตรวจ

| รายการ | เอกสาร |
|---|---|
| สถาปัตยกรรม / สัญญา / เจ้าของไฟล์ | [Desktop contracts](../specs/2026-09-17-callmd-desktop-contracts.md) |
| หน้าจอ / การนำทาง / ขอบเขตงานออกแบบ | [Desktop UI](../design/2026-09-17-callmd-desktop-ui.md) |
| AC/SC / แผนทดสอบ / baseline RCA ที่เสนอ | [Acceptance plan](../specs/2026-09-17-callmd-desktop-acceptance.md) |
| โครงหน้าจอ | [Wireframes](../design/callmd-desktop/wireframes.rendered-mockup.png) |
| ภาพปกติ 3 หน้าจอ | [Light](../design/callmd-desktop/desktop-light.rendered-mockup.png), [Dark](../design/callmd-desktop/desktop-dark.rendered-mockup.png) |
| ว่าง / โหลด / ผิดพลาด | [Light states](../design/callmd-desktop/states-light.rendered-mockup.png), [Dark states](../design/callmd-desktop/states-dark.rendered-mockup.png) |

ภาพทั้งหมดเป็น **mockup ข้อมูลตัวอย่าง** ไม่ใช่ภาพแอปที่พัฒนาและทดสอบแล้ว
ใช้ลำดับข้อมูลของ Call.md แต่คงแบรนด์ Quiet Archive / ภาษาไทย / ระบบ FUNG
ไม่คัดลอก Electron, VideoDB, cloud runtime หรือโมเดลเก็บ API key ของต้นทาง

## ทางเลือกที่เสนอ

| | A — ปรับ UI ด้วยสัญญาปัจจุบัน | B — พื้นที่ทบทวนบันทึกเต็มขึ้น (เสนอ) |
|---|---|---|
| Sidebar / Home / Live | ปรับใหม่และคงทางเข้าความสามารถเดิม | เช่นเดียวกับ A |
| ทบทวนบันทึก | บันทึกปัจจุบันที่ทราบ ID | เพิ่มรายการบันทึกจริงของโครงการและเลือกย้อนหลัง |
| ฟังเสียงบน desktop | ยังไม่รองรับ แสดงเหตุผล | เพิ่ม player native สำหรับ PCM16 WAV ที่อุปกรณ์รองรับ |
| ถาม AI | ค้นความรู้ในเครื่องตามสัญญาเดิม ไม่อ้างว่าแยก recording/project สมบูรณ์ | เพิ่มคำสั่งใหม่ที่ใช้เฉพาะ transcript ของ recording ที่เลือก |
| สรุป / ส่งออก | คงคู่ project + recording; รายการไฟล์ส่งออกยังเป็นระดับ project | คงความจริงเรื่องขอบเขตเช่นเดียวกับ A |
| ความเสี่ยง | UI/state ownership เป็นหลัก | เพิ่ม native audio, resource lifetime, capture/playback admission และคำสั่งอ่านข้อมูล |

**ข้อจำกัดของ B ที่ต้องรับทราบ:** player ที่เสนอไม่เพิ่ม decoder/resampler
รองรับ integer PCM16 WAV mono/stereo 8–96 kHz เมื่อ output device รองรับ rate
เดียวกันเท่านั้น MP3, float WAV, รูปแบบอื่น หรืออุปกรณ์ไม่รองรับ จะแสดง unavailable
การถอดเสียง/อ่าน transcript ของไฟล์เหล่านั้นไม่ได้แปลว่า player เล่นได้
ความพร้อมของอุปกรณ์จริงยังไม่ได้ทดสอบ และการเล่นเสียงต้องไม่ชนกับ capture

ไม่รวม P2: bookmarks, agenda/checklist แบบบันทึกถาวร, WPM/คะแนนบทสนทนา,
sentiment หรือ proactive assist ไม่เพิ่ม cloud/provider/OAuth/MCP automation,
schema/migration หรือ Google Drive ที่ยกเลิกไปแล้ว

## รายการขอบเขตที่ต้องเลือกและอนุมัติ

- **SCOPE:** Boss เลือก B โดยยืนยัน UI ใหม่ + ประวัติบันทึกแล้ว
  และยืนยันรวม player WAV + recording Q&A ด้วยคำตอบ `ทำคู่กันแบบบขนาน`
- **SEC-1 (B):** อนุมัติคำสั่ง local native/player และ ownership/limits ตาม
  contract รวม `recording_review.rs`, `desktop_playback.rs` และ `lib.rs`
  ไม่มี HTTP listener, media URL หรือ renderer fetch ใหม่
- **SEC-2 (B):** อนุมัติ guard ระดับ native ระหว่าง capture กับ playback ใน
  `live_meeting.rs` และ AppState; ไม่พึ่งปุ่ม UI ป้องกัน race เพียงอย่างเดียว
- **Q&A (B):** คำสั่งใหม่ไม่ส่ง graph/live tail ที่ไม่มี provenance ของ recording
  เข้า prompt; คำสั่งเดิมยังคงสัญญาเดิมและต้องติดป้ายขอบเขตตามจริง
- **UI-1:** อนุมัติการจัด owner สำหรับ live component/CSS และการต่อ
  `DesktopShell.tsx` โดยคง settings/pairing/recovery/import/export เดิม
- **DOC-FMT:** ยอมรับ SVG/PNG ของสามหน้าหลักเป็นชุดออกแบบ P1 นี้ หรือขอให้
  ส่ง Figma/Penpot + 2× และหน้าทั้งหมดก่อน ไม่มีการอ้างว่าชุดปัจจุบันครบ
  redesign brief ทุก surface/ทุก settings screen
- **BASELINE (แยกจาก UI):** อนุมัติให้ตรวจ/แก้ CI script inventory และเพิ่ม
  regression guard สองทิศทางตาม RCA ที่เสนอ ไม่รื้อฟื้น Drive ไม่ลบ native
  custody gate โดยไม่มีหลักฐานทดแทน หากขาด coverage ต้องเสนอขอบเขตเพิ่มก่อน

รายการไฟล์ที่ลงรายละเอียดใน contract เป็น **candidate leases** เท่านั้น
ต้องปรับ manifest ให้ตรงชุดที่อนุมัติก่อน code dispatch ไม่ให้ worker อาศัย
wildcard หรือข้อเสนอในเอกสารเป็นสิทธิ์แก้ไฟล์โดยอัตโนมัติ

## Review และหลักฐาน

Independent Terra documentation review ให้ **PASS** ทั้งสามสายและตรวจ
SHA-256 ของ frozen author artifacts ครบ 16 ไฟล์ตรงกัน อ่านผลพร้อม digests
ใน [รายงานตรวจอิสระ](../verification/implementation-reports/2026-09-17-callmd-doc-review.md)
ผลนี้รับรองความพร้อมของข้อเสนอ A/B สำหรับตัดสินใจ ไม่ใช่ผลทดสอบผลิตภัณฑ์
Boss ยืนยัน B ครบแล้วตาม [บันทึกการเลือก](../verification/implementation-reports/2026-09-17-callmd-approval.md)
ก่อน dispatch โค้ดเหลือการยอมรับรูปแบบ SVG/PNG, อำนาจแก้ baseline แยก
และการปรับ dependency ด้านล่างเพื่อให้ backend/UI ทำคู่กันได้จริง

| Gate | สถานะ |
|---|---|
| Luna documentation wave | COMPLETED; fresh explicit Luna max dispatches; leases released |
| Orchestrator structural / visual review | PASS (static); 25 nodes / 28 edges; 5 SVG + 5 PNG 2× |
| Independent Terra DOC_REVIEW | PASS; actual Terra/high request recorded; reviewer closed |
| Boss feature selection / code approval | Full B confirmed; scoped format and separate baseline approval pending |
| Product tests / build / native audio / hosted CI | NOT_RUN |

## ขั้นหลังอนุมัติ

แผนเดิมรอ backend เสร็จก่อน shared bridge และ UI จึงยังไม่ทำ backend/UI
คู่กันจริง ผลตรวจอยู่ใน [parallel readiness](../verification/implementation-reports/2026-09-17-callmd-parallel-readiness.md)
เพื่อทำตามคำสั่งล่าสุด เสนอปรับโดยไม่ตัด review gate ดังนี้:

1. Baseline ที่อนุมัติ → contract tests/review → เพิ่ม read-only
   `BACKEND_INTERFACE_REVIEW` ตรึง command/DTO/error/ownership จากสเปกที่ตรวจแล้ว
2. เปิดคู่กัน: `BACKEND_RECORDING → BACKEND_REVIEW` กับ
   `SHARED_CONTRACT → SHARED_REVIEW → UI_SHELL || UI_LIVE || UI_HISTORY`
   จำกัด Luna รวมไม่เกิน 3 ตัว จึงรอคิว UI บางตัวหาก backend ยังทำอยู่
3. `INTEGRATE` ต้องรอทั้ง `BACKEND_REVIEW` และ `UI_TASK_REVIEW` จากนั้น
   verification + independent integration review ตามเดิม

การแก้ edge ที่เสนอ: เพิ่ม interface review หลัง contract-test review;
ย้าย dependency ของ SHARED_CONTRACT จาก BACKEND_REVIEW ไป interface review;
เพิ่ม BACKEND_REVIEW เป็น dependency ของ INTEGRATE โดยไม่ลบ node รีวิวเดิม
ต้องปรับ contract/test acceptance ให้แยกการตรวจ interface ล่วงหน้าออกจาก
ผล native implementation; mock/expected-red ไม่ใช่หลักฐาน runtime ผ่าน
นี่เป็น **ข้อเสนอ** ยังไม่เปลี่ยน executable DAG 25 nodes/28 edges
ใช้ isolated worktree และ exact file leases เหมือนเดิม; orchestrator ไม่แก้โค้ด

ขอบเขต baseline ที่เสนอให้อนุมัติแยก:
`.github/workflows/ci.yml`, `tests/ciCoverage.test.mjs`,
`tests/nativeSessionCustody.test.mjs`, `package.json` เฉพาะ script/gate ที่เกี่ยวข้อง
นำแนวแก้ stale CI ที่มีใน main `d10bbf8` มาใช้เฉพาะส่วนที่ตรวจแล้ว ไม่รวม main ทั้งก้อน
คง/คืนการตรวจ native custody แบบไม่มี Drive และเพิ่มการตรวจ CI → script ย้อนกลับ
ตาม [RCA](../../.brain/rca/2026-09-17-callmd-baseline-gates.md)
ห้ามอ้างว่า coverage เดิมทดแทนครบเมื่อยังไม่มีหลักฐาน ไม่ลด security assertions

การ commit/push/merge/release ต้องมีอำนาจแยก ไม่อนุมานจาก approval ชุดนี้

## Version Diff

- `0.2.0b → 0.2.1b`: reconcile the approved UI-1 owner from the historical
  InstrumentRail path to the current `DesktopShell.tsx` shell boundary; feature
  authority and implementation gates are unchanged.
- `0.1.2b → 0.2.0b`: เสนอ interface-first fork/join สำหรับ backend/UI ขนานจริง
  และ exact baseline repair scope; รออนุมัติก่อนใช้ dependency/lease ที่เปลี่ยน
- `0.1.1b → 0.1.2b`: บันทึก B ครบทั้ง UI/history และ WAV/Q&A พร้อมคำสั่งทำขนาน
  ไม่ถามเลือกฟีเจอร์ซ้ำ; baseline และรูปแบบส่งมอบยังแยกชัดเจน

- `0.1.0b → 0.1.1b`: บันทึกผล independent review และการเลือก UI + history
  ของ Boss โดยไม่อนุมาน player/Q&A/CI authority
- `new → 0.1.0b`: รวมทางเลือก A/B, ขอบเขต native/UI/baseline และข้อจำกัด
  mockup เพื่อขออนุมัติหลัง independent documentation review
- Application code/version: ไม่เปลี่ยน

## CHANGELOG

| Version | Date | Status | Summary | Commit | Agent |
|---|---|---|---|---|---|
| 0.2.1b | 2026-09-20 | candidate | Reconciled approved UI-1 owner with the current DesktopShell boundary; no new implementation authority | pending; docs reconciliation | RWANG |
| 0.2.0b | 2026-09-17 | candidate | Propose safe backend/UI fork-join and exact baseline-only scope | UNCOMMITTED; base 376ef30 | Codex orchestrator |
| 0.1.2b | 2026-09-17 | candidate | Record full B selection and parallel execution direction; preserve independent gates | UNCOMMITTED; base 376ef30 | Codex orchestrator |
| 0.1.1b | 2026-09-17 | candidate | Record documentation PASS and Boss UI/history selection; remaining scope pending | UNCOMMITTED; base c378af9 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | candidate | Assemble feature approval packet; product code still unapproved | UNCOMMITTED; base c378af9 | Codex orchestrator |
