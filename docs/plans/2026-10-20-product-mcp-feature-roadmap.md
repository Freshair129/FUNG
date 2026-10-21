---
version: "0.1.6b"
created_at: "2026-10-20"
last_update: "2026-10-20"
status: "beta"
superseded_by: null
attributes:
  domain: "product-mcp"
  doc_type: "implementation-plan"
  scope: "FUNG product MCP feature roadmap, priorities, and execution dependencies"
  language: "Thai-first"
---

# FUNG Product MCP Feature Roadmap

เอกสารนี้จัดลำดับฟีเจอร์และเครื่องมือ MCP เป้าหมายของ FUNG เป็น wave และ dependency ที่มีเกณฑ์ตรวจรับได้จริง ผู้ใช้สั่งเริ่ม execution วันที่ 2026-10-20 ภายใต้ run `FUNG-PRODUCT-MCP-2026-10-20-01`; MCP-00 inventory และ MCP-01 contract v0.1.4b ผ่าน independent review แล้ว. MCP-02 implementation review พบ notification dispatch และ job-list scan ที่ไม่มีขอบเขต แล้วแก้ใน working tree. หลังแก้ full Rust library suite ผ่าน 649 tests (0 failed, 6 ignored), `npm run build`, workflow MCP 3/3, formatting และ whitespace checks. Independent re-review ยังไม่ทำ; MCP-02 จึงยังอยู่ REVIEW และ MCP-03 ยังไม่เริ่มตาม dependency gate. DAG เป็น checkpoint ledger ไม่ใช่ scheduler ทำงานเบื้องหลัง. เอกสารนี้ไม่ใช่หลักฐานว่าเครื่องมือทั้งหมดพร้อมใช้ และไม่อนุมัติ external write, data egress, destructive action หรือ release

## 1. สถานะจริงที่ใช้วางแผน

| Surface | สิ่งที่มีหลักฐาน | ขอบเขตที่ยังต้องตรวจ |
|---|---|---|
| Developer workflow MCP | fung.workflow.describe, fung.workflow.run_acceptance, fung.workflow.list_runs; รัน synthetic file-import → CPU transcription → edit/export → orderly Genesis reopen ได้ | เป็น acceptance harness; ไม่ได้เริ่มอัดไมค์หรือพิสูจน์ product MCP |
| Product MCP contract/runtime | [Product MCP v2 contract](../specs/2026-10-20-product-mcp-v2-contract.md) ผ่าน MCP-01 review; MCP-02 source candidate implements six discovery/read/job tools, durable job control and a loopback Streamable HTTP listener | independent implementation review, packaged app, actual MCP-client interoperability, microphone capture, and higher feature families remain unverified |
| Desktop capture | มีเส้นทางบันทึกไมค์/เสียงระบบและบันทึกไฟล์ในเครื่อง | หลักฐานไมค์จริงล่าสุดยังมีผลไม่สม่ำเสมอ; ต้อง gate ด้วยการมี audio output ที่บันทึกได้ ไม่ใช้คะแนนความแม่น ASR เป็น gate ในช่วงแรก |
| External MCP | มี code-level path สำหรับเชื่อมต่อ MCP ภายนอกและ approval แบบ default-off | เป็นคนละทิศทางกับ product MCP ที่เปิดเครื่องมือของ FUNG ให้ Codex/AI client; real-connector UAT ยังไม่ผ่าน |
| Mobile MCP | มี contract และ server ของ Mobile แยกตาม contracts/mobile-mcp-v1.yaml | ห้ามนับเป็น Desktop MCP acceptance โดยอัตโนมัติ |

แหล่งอ้างอิง: [workflow MCP runbook](../operations/fung-workflow-mcp.md), [workflow acceptance report](../verification/implementation-reports/2026-10-20-workflow-mcp-acceptance.md), [Desktop progress](../Desktop/08-real-progress.md), [Desktop MCP contract](../../contracts/local-mcp-v1.yaml), [Mobile MCP contract](../../contracts/mobile-mcp-v1.yaml), [meeting intelligence domain map](../architecture/MEETING_INTELLIGENCE_DOMAINS.md).

## 2. หลักการจัด priority

1. ให้ความสำคัญกับ workflow ที่ทำงานครบและให้ผลลัพธ์จริง ก่อนปรับความแม่นหรือความเร็วของโมเดล
2. รักษาชื่อเดิมใน contract เช่น fung.recording.start, fung.recording.control, fung.transcript.get, fung.export.create เมื่อความหมายและ schema ยังเข้ากันได้; ใช้ชื่อใหม่แบบ fung.domain.verb สำหรับ capability ใหม่
3. MCP เป็น adapter เข้าสู่ API/command และ GenesisBlockDB invariant เดิม ห้ามเพิ่มทางเขียนฐานข้อมูลคู่หรือข้าม job lifecycle
4. ระบุสถานะ tool แต่ละตัวเป็น RUNTIME_VERIFIED, IMPLEMENTED_NOT_UAT, CONTRACT_ONLY, MISSING หรือ BLOCKED; อย่าอนุมานจากเอกสารอย่างเดียว
5. ทุกงานที่ใช้เวลานานคืน job_id ให้ติดตาม ยกเลิก และอ่านผลได้ โดยไม่บล็อกการบันทึกเสียง
6. แบ่ง permission ของการอ่านในเครื่อง, แก้ข้อมูลในเครื่อง, บันทึกเสียง, ใช้ network, ส่งข้อมูลออก, ลบข้อมูล และงาน admin ออกจากกัน

| Priority | ผลลัพธ์ที่ต้องได้ | เหตุผล |
|---|---|---|
| P0 — Contract and safety | รู้ว่า tool ไหนมีจริง, schema ตรงกับ runtime, สิทธิ์และ audit ชัด | ลดความเสี่ยงทำ MCP ซ้ำกับ API/contract เก่า และป้องกัน tool เกินสิทธิ์ |
| P1 — Capture workflow | เริ่ม → ดูสถานะ → หยุด → ได้ไฟล์และ job result → เปิดอ่านผลได้ | เป็นเส้นทางหลักที่ผู้ใช้ต้องการให้เรียกผ่าน MCP |
| P2 — Read and find | ค้นรายการประชุม/ไฟล์/transcript และย้อนกลับไปยังช่วงหลักฐานได้ | ทำให้ข้อมูลที่บันทึกนำกลับมาใช้ได้ |
| P3 — Review and produce | แก้ transcript, สร้างสรุป/งาน, export และอ่านผลหลัง restart | ปิดวงจรจากเสียงไปสู่ artifact |
| P4 — Competitive coverage | collections, contacts, decisions, clips, translation, analytics | เพิ่มความสามารถเทียบคู่แข่งหลัง core ใช้งานได้ |
| P5 — External actions and automation | share/publish, calendar, scheduled capture, connector writes, administration | มีผลต่อคนอื่นหรือส่งข้อมูลออก จึงต้องมาหลัง policy และ audit |

## 3. Roadmap ตาม wave

### W0 — Inventory และตรึง product contract (P0)

**เครื่องมือเป้าหมาย:** fung.health, fung.capabilities.get, fung.permissions.get, fung.job.list, fung.job.get, fung.job.control.

**งาน**

- เทียบ tool ใน contracts/local-mcp-v1.yaml กับ server registration, Tauri commands, local API, Genesis adapters และ tests ทีละตัว
- ทำ inventory ที่ระบุ runtime state, current input/output, side effect, permission, known limitation และ evidence
- กำหนด canonical identity ของ project/meeting/recording และการอ้าง revision เพื่อไม่สร้าง object ซ้ำ
- ตรึง MCP transport, local authentication, capability discovery, versioning, cancellation, idempotency, structured errors และ audit event
- กำหนด client-facing tool pack ตามงาน ไม่โหลด catalog ทั้งหมดทุก session

**Exit gate:** contract และ inventory ผ่าน independent review; tool แต่ละตัวมีสถานะจาก runtime evidence; ไม่มีการเปลี่ยน schema หรือ policy ที่ยังไม่ตัดสินใจ

### W1 — Capture จาก MCP ถึงไฟล์จริง (P1)

**เครื่องมือเป้าหมาย:** fung.project.create, fung.recording.sources.list, fung.recording.start, fung.recording.status, fung.recording.control, fung.job.get.

**งาน**

- เชื่อม MCP เข้ากับ capture controller ใน FUNG โดยใช้ source และ permission ที่ระบบรองรับจริง
- สร้าง project แบบระบุชื่อ/storage เมื่อไม่มี project ที่ผู้ใช้เลือก; ไม่เดา destination ของไฟล์
- รองรับ microphone, system audio แยก track, import; เปิด per-app audio เฉพาะเมื่อ capability probe ยืนยันว่ารองรับ
- ระบุ project, source, track, output policy และ idempotency key ใน start request
- ให้ control รองรับ pause/resume/stop/recover ผ่าน state machine เดียวกับ UI
- ส่ง job id และผลที่บันทึกแล้วกลับมา โดย MCP อ่านไฟล์เสียงหรือ transcript bytes ทั้งชุดเท่าที่จำเป็นเท่านั้น

**Exit gate:** ทดสอบ transport ด้วย fixture; จากนั้นทดสอบ packaged Desktop ใน UAT profile แยก: start → มีสัญญาณเสียงและ chunk ที่ไม่ว่าง → stop → ไฟล์/ledger hash ตรง → job จบ → ปิดเปิดแอปและอ่านผลเดิมได้. ASR ต้องสร้างผลลัพธ์ตาม workflow แต่ยังไม่ใช้คะแนนความแม่นเป็น gate ของ wave นี้. การเริ่มบันทึกต้องมี indicator และจุดหยุดที่ชัด; tool argument อย่าง confirmed: true ใช้แทนการอนุญาตใน UI ไม่ได้.

### W2 — Meeting library, search และ citations (P2)

**เครื่องมือเป้าหมาย:** fung.project.list, fung.project.get, fung.recording.list, fung.recording.get, fung.transcript.get, fung.transcript.search, fung.citation.resolve, fung.meeting.ask.

**งาน**

- คืนรายการและ metadata แบบมี pagination/cursor โดย default scope เป็น project ที่ผู้ใช้เลือกและมีสิทธิ์
- ค้น transcript, summary, task และ decision ตาม query/date/person/project
- คืน hit พร้อม meeting/recording ID, transcript revision, speaker, timestamp และ excerpt
- resolve citation ไปยัง source segment เดิมและ playback/deep link ที่ได้รับอนุญาต
- แยกคำพูดจากต้นฉบับ, summary ที่โมเดลสร้าง และ inference ของ Agent

**Exit gate:** ทุกคำตอบที่อ้างข้อมูลมี citation ที่ resolve กลับไปยัง revision/เวลาเดิมได้; permission filter ผ่าน cross-project negative cases; ไม่มีข้อความจาก transcript ที่ถูกตีความเป็นคำสั่ง MCP.

### W3 — Transcript review, summaries, tasks และ exports (P3)

**เครื่องมือเป้าหมาย:** fung.transcript.patch_segment, fung.transcript.rename_speaker, fung.summary.generate, fung.summary.list, fung.summary.get, fung.task.extract, fung.task.list, fung.task.update, fung.export.create.

**งาน**

- รองรับแก้คำพูด, speaker label, split/merge segment และ revision conflict อย่างไม่เขียนทับเงียบ ๆ
- สร้าง summary/minutes จาก source revision ที่ระบุ พร้อม citations
- เสนอ action item พร้อม quote หลักฐาน; เก็บ owner/due date เป็น unknown เมื่อหลักฐานไม่พอ
- export WAV/MP3/TXT/SRT/VTT/JSON และ artifact ที่อนุญาต พร้อมระบุ source revision
- เมื่อ transcript เปลี่ยน ให้ทำเครื่องหมาย summary/task draft ที่พึ่งพาข้อมูลเก่าว่า stale

**Exit gate:** edit และ artifact อยู่รอดหลัง Genesis close/reopen; export สอดคล้องกับ revision ที่เลือก; retry ไม่สร้างรายการหรือไฟล์ซ้ำโดยไม่มีเหตุผล.

### W4 — Competitive local productivity tools (P4)

**เครื่องมือเป้าหมาย:** fung.collection.list/update, fung.contact.search/update, fung.decision.list/update, fung.clip.create/list, fung.translation.create, fung.glossary.update, fung.analytics.query, fung.template.list/update.

**งาน**

- จัดระเบียบประชุมด้วย collections/tags และค้น contact ที่มีความสัมพันธ์กับ meeting
- ติดตาม decision, การแทนที่ decision เก่า และหลักฐานประกอบ
- ทำ soundbite/highlight แบบอ้างอิงต้นฉบับ; การสร้างไฟล์คลิปเป็นงาน export ที่แยกได้
- เพิ่ม translation/glossary และ analytics แบบระบุ coverage กับ missing data
- ทำ meeting templates สำหรับ stand-up, interview, sales และ research

**Exit gate:** ทุก projection คืน provenance และ source revision; analytics แสดง denominator/coverage; speaker/contact label ไม่เท่ากับการยืนยันตัวบุคคล.

### W5 — Connectors, sharing และ external writes (P5)

**เครื่องมือเป้าหมาย:** fung.connector.list, fung.connector.call.preview, fung.connector.call.commit, fung.connector.grant.revoke, fung.share.list/create/revoke, fung.publish.preview/commit.

**งาน**

- เริ่มจาก external read-only lookup ที่ใช้ grant แยกตาม project/meeting และหมดอายุได้
- preview ต้องแสดง connector, tool, query, fields ที่จะออกจากเครื่อง, ผู้รับ และผลลัพธ์ที่คาดไว้
- commit ผูกกับ immutable preview/hash เดิม; หาก payload หรือ recipient เปลี่ยนให้ preview ใหม่
- แยก local export, hosted share, provider send และการโพสต์ใน meeting room เป็นคนละ permission
- external write ต้องมี destination binding, idempotency key, receipt reconciliation และ revoke path

**Exit gate:** deny/expired/revoked/timeout ส่งข้อมูลออกเป็นศูนย์; send ที่ผลไม่แน่ชัดเข้า delivery_unknown และไม่ retry อัตโนมัติ; audit มี receipt โดยไม่เก็บ secret หรือ full transcript ใน log.

### W6 — Calendar, automation และ operations (P5)

**เครื่องมือเป้าหมาย:** fung.calendar.events.list/link, fung.automation.list/update/run, fung.diagnostics.get, fung.retention.preview/apply, fung.audit.list, fung.backup.create/restore.

**งาน**

- เชื่อม calendar แบบเลือกบัญชีและ event ด้วยตนเองก่อน auto-link
- scheduled capture เป็น standing grant ที่ระบุเวลา, source, event match และวันหมดอายุ
- automation ใช้ typed allowlist actions; ห้าม arbitrary shell หรือ arbitrary API tool
- เพิ่ม health, job diagnostics, audit, retention preview และ backup/restore runbook

**Exit gate:** revoke/disable หยุด action ใหม่ได้; schedule ไม่ขยาย source หรือ scope เอง; purge/restore ต้องมี preview, recovery evidence และ explicit authorization.

## 4. Execution DAG และวิธีแบ่งงาน

ตัวติดตาม execution และหลักฐานอยู่ใน [product MCP execution DAG](2026-10-20-product-mcp-execution-dag.json). สถานะเปลี่ยนเมื่อ exit criteria และหลักฐานของ node ผ่าน review; dependency ปกติต้องเป็น `ACCEPTED` ส่วน optional lane จะนับว่าผ่านเมื่อ `SKIPPED` พร้อมเหตุผลและตัด scope ออกจาก release claims. MCP-01 accepted แล้ว; MCP-02 post-fix tests/build ผ่าน แต่ยังอยู่ `REVIEW` จนกว่า independent implementation review จะผ่านก่อนเปิด MCP-03. งานจะหยุดตรง gate ที่ต้องใช้ผู้ใช้หรือบริการจริง. ระบบยังไม่มี scheduler/agent dispatch อัตโนมัติ.

~~~mermaid
flowchart TD
  A[MCP-00 Inventory] --> B[MCP-01 Contract and policy]
  B --> C[MCP-02 Local bridge and jobs]
  C --> D[MCP-03 Capture source discovery]
  D --> E[MCP-04 Capture controls]
  C --> F[MCP-05 Library read]
  F --> G[MCP-06 Search and citations]
  F --> H[MCP-07 Transcript review]
  G --> I[MCP-08 Summaries and tasks]
  H --> I
  F --> J[MCP-09 Export]
  I --> J
  E --> K[MCP-10 Core acceptance]
  G --> K
  H --> K
  I --> K
  J --> K
  K --> L[MCP-11 Organization]
  K --> M[MCP-12 Media and clips]
  L --> N[MCP-13 Analytics]
  K --> O[MCP-14 External read]
  O --> P[MCP-15 Share and publish]
  Q[MCP-17 Operations and audit] --> P
  E --> R[MCP-16 Calendar and automation]
  Q --> R
  K --> Q
  L --> S[MCP-18 Release review]
  M --> S
  N --> S
  O --> S
  P --> S
  R --> S
  Q --> S
~~~

| Wave | งานขนาน | ต้องรอ |
|---|---|---|
| W0 | MCP-00 runtime/contract inventory; MCP-01 contract and policy freeze เป็นงานต่อเนื่องลำดับเดียว | เริ่มได้ทันที; ห้ามเปิด feature lane ก่อน inventory/contract accepted |
| W1 | Capture lane; Library lane; จากนั้น Search และ Transcript/revision ทำขนานกันได้ | MCP-01 accepted; tool lanes ใช้ schema และ permission model เดียวกัน |
| W2 | Summary/tasks; export/jobs; local end-to-end integration | W1 handoffs ถูก review และ accepted |
| W3 | Collections/contacts/decisions; clips/translations; analytics | Core workflow gate MCP-10 ผ่าน |
| W4 | External read connector; share/publish; calendar/automation; admin/retention | W0 policy, audit และ explicit approval gates พร้อม |
| W5 | Independent review, packaged Desktop UAT, MCP-client interoperability | MCP-10 ผ่านสำหรับ core flow; Codex/other client interoperability เป็นเกณฑ์ MCP-18; optional parity lanes ที่ไม่เลือกต้องระบุเหตุผล |

ผู้เขียนแต่ละ lane ต้องมี write set แยกกัน ส่วนไฟล์ shared เช่น Genesis adapter, Tauri command registry, shared TS bridge, schemas/lockfiles และ MCP registration ให้ integration owner คนเดียวแก้หลัง handoff เพื่อไม่ให้ merge conflict เปลี่ยน behavior โดยไม่ review.

## 5. Release gates

### Core MCP beta

- MCP-10 synthetic protocol client ค้น tools และเรียก tool ได้จาก FUNG Desktop แล้วอ่านผลกลับได้; การเชื่อม Codex จริงเป็นเกณฑ์ MCP-18
- Capture vertical workflow ใน W1 ผ่าน synthetic transport และ packaged mic UAT
- Audio artifact, transcript/job state, export และ restart readback ตรวจสอบย้อนกลับได้
- Read/search/citation/edit/summary/export ของ W2–W3 ทำงานกับ data ที่ผู้ใช้มีสิทธิ์
- ไม่มี assertion เรื่อง Thai ASR quality หรือ GPU speed ใน gate นี้; วัดและ optimize หลัง workflow ผ่าน

### Competitive product release

- W4 optional lanes ที่เปิดใช้มี acceptance ของตนเอง รวม negative permission cases; MCP-18 บันทึกว่าจะปล่อยหรือคงปิดแต่ละ lane
- External tools แสดง destination/egress และบังคับ approval ตาม operation
- Packaged desktop, fresh profile, restart, revoke, crash/cancel และ MCP-client interoperability ผ่านตาม matrix
- Release evidence แยก local tests, synthetic fixture, real mic, provider UAT, packaged artifact และ release signing

## 6. Decisions closed and implementation gates

MCP-01 contract กำหนด transport เป็น Streamable HTTP แบบ stateless บน loopback,
local-only Bearer grant (พร้อมบันทึกข้อเบี่ยงเบนจาก MCP OAuth profile),
workspace/project binding, capability scopes, opaque Genesis IDs, revision
preconditions, idempotency, job/error/audit rules และ schemas ของ tool pack
MCP-01 ผ่าน independent re-review แล้ว จึงเริ่ม MCP-02 ได้และ implementation ได้เริ่มขึ้นแล้ว; ขอบเขตที่ยังไม่ผ่านคือ review ของ implementation และ packaged/client acceptance.

สิ่งที่ยังเป็น gate ของ implementation:

1. Legacy `local-mcp-v1` จะเป็น historical contract; tool ใหม่ใช้ schema v2
   และจะเพิ่ม compatibility alias เฉพาะเมื่อ input/output ตรงกันและมี contract test.
2. Meeting ID จะยังไม่ถูกสร้างจน Genesis มี canonical entity; ทุก phase ต้องระบุ
   project/recording IDs จริงที่ใช้แทน.
3. MCP-10 ใช้ synthetic transport และ packaged mic UAT เพื่อยืนยัน workflow
   capture-to-result; MCP-18 เป็น gate สำหรับ Codex/third-party client interop.
4. เลือก/ตัด optional lanes ใน MCP-18 พร้อมเหตุผลก่อนระบุขอบเขต release.
5. External write, provider egress, deletion, admin และ release ต้องมี gate
   เฉพาะและการอนุมัติของผู้ใช้; การอนุมัติ execution roadmap นี้ไม่อนุมัติ action เหล่านั้น.

## 7. Version Diff

| Version | Change |
|---|---|
| 0.1.6b | บันทึก full Rust library tests, frontend build และ workflow MCP acceptance ที่ผ่านหลังแก้ findings โดยคง MCP-02 ใน REVIEW จนกว่าจะผ่าน independent re-review |
| 0.1.5b | บันทึก post-remediation compile/format/whitespace checks ที่ผ่าน โดยคง MCP-02 ใน REVIEW จนกว่าชุดทดสอบและ independent re-review จะผ่าน |
| 0.1.4b | บันทึก review findings ที่แก้ใน working tree และแยกผลตรวจเดิมก่อนแก้จาก post-fix verification ที่ยังไม่รัน |
| 0.1.3b | บันทึก MCP-02 full Rust/frontend/workflow test evidence, คงสถานะ REVIEW จน independent implementation review และ packaged/client UAT ผ่าน |
| 0.1.2b | ปรับ MCP-01 เป็น accepted, บันทึก MCP-02 source implementation และแยก source/synthetic evidence ออกจาก packaged/client UAT |
| 0.1.1b | บันทึก MCP-01 decisions ที่ปิดแล้ว แยก synthetic/packaged core UAT ออกจาก client interoperability gate และแก้ลำดับ review ก่อนปล่อย |
| 0.1.0b | เสนอ P0–P5 roadmap, priority, MCP tool mapping, release gates และ DAG โดยแยก developer acceptance MCP ออกจาก product MCP |
