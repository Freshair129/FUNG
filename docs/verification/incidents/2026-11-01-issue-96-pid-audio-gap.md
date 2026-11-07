---
id: FUNG-INC-000096
title: "PID audio gaps at source stream lifecycle transitions"
version: "0.1.3"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: candidate
superseded_by: null
attributes:
  domain: desktop-native-capture
  doc_type: incident-report
  scope: "Issue 96 UAT failure; canonical symptoms and response"
  language: Thai-first
aliases:
  - "Incident #96"
  - "PID audio gap"
  - "เสียงขาดหลัง Resume"
tags:
  - platform/windows
  - component/audio-capture
  - mode/pid-loopback
  - symptom/audio-gap
relations:
  issues:
    - https://github.com/Freshair129/FUNG/issues/96
  rcas:
    - FUNG-RCA-000096-01
ownership:
  owner: null
  reviewer: null
incident:
  status: investigating
  environment: installed-uat-and-owned-native-diagnostics
  severity: null
  started_at: null
  detected_at: null
  resolved_at: null
annotations:
  - id: FUNG-INC-000096-N01
    target: FUNG-INC-000096-S02
    kind: evidence-boundary
    text: "Resume in this fixture recreates a stream; not every application's pause/resume lifecycle."
    evidence_refs:
      - FUNG-RCA-000096-01-E01
  - id: FUNG-INC-000096-N02
    target: FUNG-INC-000096-S03
    kind: uncertainty
    text: "A timestamp gap is not an independently proven equal-duration loss of active audio."
    evidence_refs:
      - FUNG-RCA-000096-01-E01
---

# Incident FUNG-INC-000096 — PID audio gaps

## Summary and current state

สถานะ **investigating**: PID capture ยังไม่ผ่านความต่อเนื่องบริเวณเปลี่ยนอายุ source stream. ชุดเปรียบเทียบรูปแบบเสียง E01 ทั้ง 4 รอบล้มเหลว; การเปรียบเทียบ lifecycle E04 เพิ่มเติมพบว่า same-client Stop/Start มี measured edge parity และไม่มี QPC gaps ทั้งสองรอบ แต่ destruction/recreation ยังล้มเหลวทั้งสองรอบ. B ยังเป็น NOT_PROVEN สำหรับ full acceptance; มี installed PID failure แยกต่างหาก. ผล endpoint บางส่วนผ่าน แต่ไม่ใช่ full two-mode acceptance. ยังไม่มี repair ที่รับรองใช้กับโปรแกรมอื่นได้จริง และยังไม่ merge/tag จากหลักฐานนี้.

วันที่เริ่มเหตุ/ตรวจพบครั้งแรก, severity และผู้รับผิดชอบ/reviewer ยังไม่ได้ยืนยัน จึงเป็น `null`. วันที่ 2026-11-01 คือวันสร้างรายงานรวม ไม่ใช่วันเริ่มบั๊ก. ผู้เขียนเอกสาร: Codex; ไม่อ้างว่าเป็นเจ้าของ incident ที่ได้รับมอบหมาย.

## Symptoms — อาการที่พบ (canonical)

<a id="FUNG-INC-000096-S01"></a>
### FUNG-INC-000096-S01 — เสียงท้ายช่วงขาด

- **Observation / expected / actual:** การเทียบ PCM ควรมีขอบเสียงตรง endpoint reference ภายใน ±48 frames; native A/B/B/A ล่าสุด Stop delta อยู่ที่ −1984 frames ใน 7/8 cycles และ −1504 ใน 1/8 (ประมาณ 41.33 / 31.33 ms ที่ 48 kHz).
- **Trigger:** owned renderer Stop แล้วทำลาย audio client ทันที. ไม่ใช่ข้อพิสูจน์ของทุกแอปหรือทุกชนิด Stop.
- **Frequency:** 8/8 cycles ใน 4 runs วันที่ 2026-11-01 (`pcm16-b`, `float32-a`, `float32-b`, `pcm16-c`).
- **Scope / impact:** PID native receiver บน host นี้, explicit LM22 HDMI; เสี่ยงเก็บส่วนท้ายไม่ครบ. ไม่ได้วัดความเสียหายต่อคำพูดของผู้ใช้.
- **Reproduction:** ใช้ frozen renderer/supervisor + raw receiver และ endpoint reference ตาม [E01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E01); target PID/creation time และอุปกรณ์ต้องผ่าน guards. ไม่รันกับเสียงผู้ใช้โดยอัตโนมัติ.
- **Evidence / findings:** [E01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E01), [F01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-F01), [F02](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-F02).

<a id="FUNG-INC-000096-S02"></a>
### FUNG-INC-000096-S02 — เสียงต้นช่วงหลังสร้าง stream ใหม่ขาด

- **Observation / expected / actual:** ต้องตรง reference ภายใน ±48 frames; Resume edge delta +441 frames (~9.19 ms) ในทุก cycle ล่าสุด. PID onset maps to source frame 480; endpoint onset maps to frame 39.
- **Trigger:** renderer สร้าง stream ใหม่และเริ่ม marker ใหม่หลัง Stop/destruction; คำว่า Resume ในหลักฐานนี้ไม่ได้ยืนยัน position-preserving pause/resume.
- **Frequency:** 8/8 cycles ของ A/B/B/A ข้างต้น. Float32 ก่อน onset 441 samples เป็นศูนย์จริง; ไม่ใช่ silent-fill branch ของตัวรับ.
- **Scope / impact:** PCM16 และ Float32/no AUTO-SRC ของ raw receiver บน host นี้. เสี่ยงข้อมูลเสียงต้นช่วงสูญหาย; ผลต่อ transcription ยังไม่ได้วัด.
- **Reproduction:** frozen protocol เดียวกับ S01 ตรวจ raw onset และ independent reference; ห้ามสร้าง prefix ทดแทนเพื่อให้ผ่าน.
- **Evidence / findings:** [E01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E01), [F01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-F01).

<a id="FUNG-INC-000096-S03"></a>
### FUNG-INC-000096-S03 — Packet timestamps มีช่องว่าง

- **Observation / expected / actual:** timeline ต้องอธิบายความต่อเนื่องและ uncertainty ได้; พบ 4 QPC gaps ต่อ run ทั้ง 4 runs ล่าสุด ขณะที่ native packet flags เป็นศูนย์ทั้งหมด.
- **Trigger:** พบใน fixture ที่เปลี่ยนอายุ stream; ไม่สรุปว่าทุก gap มี causal trigger เดียวกัน.
- **Frequency:** 4/4 runs; เป็นคนละ measurement กับ 8 cycle edge checks ของ S01/S02.
- **Scope / impact:** timestamp discontinuity ทำให้ยังรับรอง whole-run continuity ไม่ได้. ยังไม่มี matched active-marker windows ครบทั้งสองข้างสำหรับทุก gap จึงห้ามแปลงเป็นจำนวนเสียงที่หายแน่นอนหรือ safe silence.
- **Reproduction:** วิเคราะห์ QPC และ read-entry cadence พร้อม raw packets ตาม [E01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E01); เก็บ fault flags/unknown coverage ทั้งหมด.
- **Related finding:** [F03](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-F03).

## Impact and boundaries

ขอบเขตเพิ่มเติมของ S01–S03: [E04 / F04](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-F04) แสดงความต่างระหว่างคง client/queue/cursor กับทำลายแล้วสร้างใหม่. อัตราเกิดและตัวเลขใน Symptoms ข้างต้นเป็นผลชุด E01 ตามชื่อ run; ไม่เหมารวมเป็นทุกวิธี Pause/Resume. Matching endpoint reference ไม่รับรองว่า source ไม่เล่นข้อมูลซ้ำ.

ปิด release gate ของ PID #96 และ two-mode installed acceptance. ไม่พบหลักฐานยืนยัน production outage, จำนวนผู้ใช้ที่ได้รับผล หรือข้อมูลประชุมจริงสูญหายจากชุดทดลองนี้. Endpoint control ที่ใช้เปรียบเทียบไม่ใช่การรับรอง endpoint workflow ทั้งหมด.

E06's matched 15.5 ms Reset-to-prime timing did not reproduce the recreation arm's first-Resume deficit in the controlled fixture. Its edges still do not establish whole-source continuity: queued source PCM was intentionally discarded, source coordinates skipped, and final receiver tails were censored. Keep the incident and both installed release gates open.

## Detection and timeline

| Date / precision | Event | Evidence |
| --- | --- | --- |
| Unknown | First occurrence/detection not established in this consolidated report | Preserve null metadata |
| 2026-11-01, date-only | Installed candidate PID fails; bounded endpoint/export/recovery observations collected | [E03](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E03) |
| 2026-11-01, date-only | Six release-delay runs fail whole-run; 50 ms helps measured Stop component only | [E02](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E02) |
| 2026-11-01, date-only | First new receiver attempt refused because HDMI unavailable; no capture process launched | [E01](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E01) |
| 2026-11-01, date-only | User reconnects displays; four actual A/B/B/A captures complete, all FAIL | E01 |
| 2026-11-01T13:23:25.248350+00:00 | Completed-study manifest creation (not capture start time) | E01 |
| 2026-11-01, Asia/Bangkok date | User authorizes ADR/templates and consolidated records | Chat instruction; documentation approval only |
| 2026-11-01, date-only | Documentation committed; fresh lifecycle A/B/B/A narrows trigger without product repair | [E04](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E04) |
| 2026-11-01, date-only | Matched Reset/recreation C/D/D/C narrows lifecycle sensitivity; no product remedy | [E05](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E05) |
| 2026-11-01, date-only | Matched Reset-to-prime timing discriminator does not reproduce first-Resume deficit; diagnostic remains NOT_PROVEN | [E06](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#FUNG-RCA-000096-01-E06) |

## Containment and workaround

Keep failed/uncertain evidence and release gate open. No silent endpoint fallback, synthesized missing PCM, parent PID substitution or threshold relaxation. Retaining the source-owned client ~50 ms is a diagnostic observation, not an implemented foreign-app workaround. Endpoint routing has bounded positive results, but user requires both modes; it does not resolve this incident.

## Recovery, follow-up and closure

Missing PID audio has no demonstrated recovery. Endpoint committed-prefix recovery belongs to E03 and does not restore PID samples that never reached capture. Follow-up is owned by [RCA action register](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md#actions); no duplicate action status here.

Closure requires a supported repair, regression checks, actual installed PID and endpoint workflow acceptance on one traceable candidate, and review of remaining gates. Unassigned owner/reviewer and absent closure evidence remain explicit; document creation does not close #96.

## Related records and history

[RCA](../rca/2026-11-01-issue-96-pid-audio-gap-rca.md), [incident index](README.md), [ADR](../../decisions/2026-11-01-incident-rca-records.md).

- 2026-11-01 — 0.1.0: consolidated existing evidence, allocated S01–S03; no new native or installed test.
- 2026-11-01 — 0.1.1: add scope qualification from fresh same-client lifecycle study; incident stays investigating.

- 2026-11-01 — 0.1.2: reference Reset/recreation comparison and retain ongoing incident status; no installed gate promoted.
- 2026-11-01 — 0.1.3: reference matched timing discriminator; retain unknown cause and dual-mode release gate.
