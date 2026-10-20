---
version: "0.1.0b"
created_at: "2026-10-20"
last_update: "2026-10-20"
status: "beta"
superseded_by: null
attributes:
  domain: "developer-tooling"
  doc_type: "runbook"
  scope: "Local FUNG functional audio workflow acceptance through MCP"
  language: "Thai-first"
---

# FUNG workflow MCP

เครื่องมือนี้ให้ Codex เรียก acceptance ของสายงานเสียงผ่าน MCP แบบ stdio โดยไม่ต้องเปิดหรือควบคุมหน้าต่าง FUNG เป็นเครื่องมือสำหรับทดสอบในเครื่อง ไม่ใช่ full MCP server ของผลิตภัณฑ์ FUNG

เซิร์ฟเวอร์รองรับ MCP stdio แบบ newline-delimited JSON-RPC, discovery ของ revision `2026-07-28` และ initialize handshake สำหรับ client รุ่นก่อน ตาม [ข้อกำหนด MCP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports#stdio).

## ขอบเขตที่รัน

`fung.workflow.run_acceptance` ใช้ไฟล์เสียงทดสอบสังเคราะห์ที่ pin hash ไว้ แล้วตรวจสายงานนี้:

1. ส่งไฟล์เข้า production loopback import API
2. รอ transcription job จบและได้ transcript อย่างน้อยหนึ่ง segment
3. อ่าน playback กลับมาแล้วเทียบ byte กับไฟล์ต้นทาง
4. แก้ transcript segment ผ่าน production storage path
5. สร้าง SRT, VTT และ WAV พร้อมตรวจเนื้อหาที่แก้และ byte ของเสียง
6. ปิด listener/store เปิด Genesis store ใหม่ แล้วตรวจ job, transcript, recording และ export inventory

รอบนี้ผ่านเมื่อการทำงานและผลลัพธ์เหล่านี้ครบ คะแนน keyword เป็นข้อมูลวินิจฉัยเท่านั้น ไม่ทำให้ workflow ตก และ MCP จะไม่คืนข้อความ transcript หรือข้อมูลเสียง

## เครื่องมือ

| Tool | ใช้ทำอะไร |
| --- | --- |
| `fung.workflow.describe` | ตรวจไฟล์ทดสอบ, hash, runtime และข้อจำกัดทรัพยากร |
| `fung.workflow.run_acceptance` | รัน workflow จริงด้วย CPU/int8/large-v3-turbo และสร้างหลักฐานใหม่ |
| `fung.workflow.list_runs` | อ่านผลย้อนหลังและเทียบเวลาแต่ละ stage; จำกัด 1–20 รายการ |

`run_acceptance` ไม่รับ path, shell command, model หรือ profile จากผู้เรียก จึงไม่มีตัวเลือกให้ชี้ไปที่ไฟล์เสียงของผู้ใช้โดยไม่ตั้งใจ แต่ละรอบใช้โฟลเดอร์ใหม่ใน `G:\AI-Studio\verification\fung-workflow-mcp` และมี file lock กันการชนกับ Cargo target ที่ใช้ร่วมกัน

## การเรียกจาก Codex

MCP server ลงทะเบียนใน `C:\Users\suanr\.codex\config.toml` ด้วยชื่อ `fung_workflow` แล้ว หากย้าย repo ให้แก้ path ของ `command` และ `cwd` ให้ตรงกับเครื่อง:

```toml
[mcp_servers.fung_workflow]
command = "node"
args = ['G:\AI-Studio\development\FUNG\scripts\fung-workflow-mcp.mjs']
cwd = 'G:\AI-Studio\development\FUNG'
startup_timeout_sec = 20
tool_timeout_sec = 900
```

เปิด session Codex ที่โหลด MCP config แล้วเรียก `fung.workflow.describe` ก่อน หากพร้อมจึงเรียก `fung.workflow.run_acceptance`; หลังจบใช้ `fung.workflow.list_runs` เพื่อดูผลและ timings รอบก่อนหน้า รายงาน acceptance ที่ผ่านแล้วอยู่ใน [verification report](../verification/implementation-reports/2026-10-20-workflow-mcp-acceptance.md)

การตรวจ transport ใน repo รันได้ด้วย:

```powershell
npm run test:fung-workflow-mcp
```

## อ่านผลและปรับ performance

ผลแต่ละรอบมี `workflow-result.json`, `run-receipt.json` และ `run.log` ใน evidence folder เดียวกัน MCP แสดงเวลา `initialDatabaseOpen`, `loopbackUpload`, `transcriptionJob`, `transcriptRead`, `playbackRead`, `transcriptEdit`, `subtitleExport`, `audioExport`, `databaseCloseReopenAndReadback` และ `total` หน่วยเป็นมิลลิวินาที

`transcriptionJob` รวมเวลาเริ่ม worker โหลดโมเดล ถอดเสียง และช่วง polling ของ job จึงยังแยก model-load กับ inference ไม่ได้ ค่า `durationMs` ใน receipt ยังรวม Cargo startup/build ด้วย ให้ใช้ timings ใน `workflowResult` เทียบการรันจริง และเทียบรอบที่ใช้ fixture, model revision, compute profile และเครื่องเดียวกัน

## ข้อจำกัดของหลักฐาน

- นี่คือ import workflow จากไฟล์ทดสอบ ไม่ใช่การกดอัดไมค์หรือการทดสอบอุปกรณ์จริง
- ทดสอบด้วย CPU/int8 และ orderly store reopen; ไม่ได้ทดสอบ GPU, crash recovery, packaged UI หรือการถอดเสียงภาษาไทย
- การได้ transcript ที่ไม่ว่างพิสูจน์ว่า pipeline ให้ผลลัพธ์ ไม่ได้ยืนยันความแม่นยำของ transcript
- MCP นี้ไม่เปลี่ยน product MCP contract ใน `contracts/local-mcp-v1.yaml` และไม่ได้ปิด full product MCP gap

ดู schema ของ server ที่ [`contracts/workflow-acceptance-mcp-v1.yaml`](../../contracts/workflow-acceptance-mcp-v1.yaml). `list_runs` จะแสดง run ที่จบแล้วตามเวลาเริ่ม และระบุ `STALE` สำหรับ receipt ที่ค้างสถานะ `RUNNING` เกิน timeout 14 นาทีบวกช่วงเผื่อ 1 นาที; ไฟล์ receipt เดิมยังเก็บสถานะต้นฉบับไว้
