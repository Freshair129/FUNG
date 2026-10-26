---
version: "0.1.2m"
created_at: "2026-10-19"
last_update: "2026-10-27"
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

- [Native client attribution and n03 — 2026-10-27](implementation-reports/2026-10-27-native-client-session-attribution.md) — actual-client diagnosis, strict native failure, portable CI fixtures and remaining runtime/installed gates.

- [Routed native seam and n01 diagnostic — 2026-10-26](implementation-reports/2026-10-26-routed-native-seam-n01-diagnostic.md) — failed owned probe and recovered first-cause evidence; source, native and installed claims remain separate.

- [Desktop implementation truth](../Desktop/08-real-progress.md) สรุปผล Desktop และ remaining gates.
- [Mobile implementation truth](../Mobile/IMPLEMENTATION_STATUS.md) สรุป build, automated checks และ physical-device gaps.
- [Core audio UAT — 2026-10-18](implementation-reports/2026-10-18-core-audio-uat.md)
- [Desktop core UAT follow-up — updated 2026-10-20](implementation-reports/2026-10-19-desktop-core-uat-followup.md) — Earlier recovery and close/reopen pass remain valid. A later isolated ONIKUMA M530 capture saved nonzero signal; all three chunk hashes match the database and SQLite integrity is `ok`. The 21.93-second sample produced inaccurate Thai transcript segments and exposed a timer that continues during stop finalization. The source fix passes focused tests/build; the user approved the unsigned native candidate, but its installation and runtime retest are not evidenced. History UI reopen for this sample remains unverified. Overall core acceptance is NOT PASSED.
- [Qwen3-ASR Thai candidate pilot — 2026-10-20](implementation-reports/2026-10-20-qwen-thai-asr-candidate-uat.md) — pinned hashes verified; full-waveform raw/denoise/normalize scores peak at 0.132, while full CTC alignment rejects U+0E46. Thai quality remains failed and Detailed routing stays disabled.
- [Live timer fix UAT — 2026-10-20](implementation-reports/2026-10-20-live-timer-fix-uat.md) — an earlier package verified active timer advancement; a later mic run found timer advancement during stopping. Source correction passes focused tests/build; the user approved the unsigned isolated native candidate, but installation and runtime retest remain unverified.
- [Workflow MCP acceptance — 2026-10-20](implementation-reports/2026-10-20-workflow-mcp-acceptance.md) — pinned synthetic file import → CPU transcription → edit/export → orderly Genesis reopen passed through MCP; transcript quality remains diagnostic only.
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
