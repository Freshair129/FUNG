---
id: FUNG-RCA-000096-01
title: "PID stream lifecycle gaps — causal boundary investigation"
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: candidate
superseded_by: null
attributes:
  domain: desktop-native-capture
  doc_type: rca
  scope: "Issue 96 Stop/recreation loss and QPC gaps; no confirmed repair"
  language: Thai-first
aliases:
  - "RCA #96"
  - "PID upstream boundary RCA"
  - "วิเคราะห์เสียงขาดหลัง Resume"
tags:
  - platform/windows
  - component/audio-capture
  - mode/pid-loopback
  - symptom/audio-gap
relations:
  incident: FUNG-INC-000096
  issues:
    - https://github.com/Freshair129/FUNG/issues/96
ownership:
  owner: null
  reviewer: null
investigation:
  status: investigating
  root_cause_status: unconfirmed
annotations:
  - id: FUNG-RCA-000096-01-N01
    target: FUNG-RCA-000096-01-F01
    kind: evidence-boundary
    text: "Independent native reproduction weakens FUNG-downstream-only causation; it is not installed acceptance."
    evidence_refs:
      - FUNG-RCA-000096-01-E01
  - id: FUNG-RCA-000096-01-N02
    target: FUNG-RCA-000096-01-F02
    kind: uncertainty
    text: "Source-client retention is controlled by the source app; no demonstrated arbitrary-PID repair."
    evidence_refs:
      - FUNG-RCA-000096-01-E02
---

# RCA FUNG-RCA-000096-01 — PID stream lifecycle gaps

## Summary and causal status

**Root cause: unconfirmed.** หลักฐานระบุ failure boundary ได้ว่าข้อมูลที่ต้องการไม่ครบเมื่อมาถึง PID packet receiver ในชุดทดลองนี้. C++ receiver แยกจาก FUNG ทำให้เกิดซ้ำได้ จึงไม่จำเป็นต้องมี Rust downmix/writer/timeline ของ FUNG จึงจะเกิดอาการ. ยังระบุ Windows internal component, queue หรือกลไกที่เป็นเหตุแน่นอนไม่ได้.

เอกสารนี้รวมผลเดิม ไม่ใช่ผล capture ใหม่หรือ patch. Owner/reviewer ยังไม่ถูกแต่งตั้ง (`null`); วันที่สร้างไม่ใช่วันเริ่มเหตุ.

## Symptoms — references to canonical Incident

| Symptom | Investigation question | Findings |
| --- | --- | --- |
| [FUNG-INC-000096-S01](../incidents/2026-11-01-issue-96-pid-audio-gap.md#FUNG-INC-000096-S01) | Stream lifetime changes delivery at which boundary? | [F01](#FUNG-RCA-000096-01-F01), [F02](#FUNG-RCA-000096-01-F02) |
| [FUNG-INC-000096-S02](../incidents/2026-11-01-issue-96-pid-audio-gap.md#FUNG-INC-000096-S02) | Is restart prefix absent before FUNG processing? | [F01](#FUNG-RCA-000096-01-F01) |
| [FUNG-INC-000096-S03](../incidents/2026-11-01-issue-96-pid-audio-gap.md#FUNG-INC-000096-S03) | Which QPC gaps contain provable missing signal? | [F03](#FUNG-RCA-000096-01-F03) |

Descriptions, frequency and reproduction prerequisites are maintained only in Incident. Results below are experiment observations supporting the analysis.

## Environment and provenance

- One Windows host; no second host comparison available. Exact OS/device inventory belongs to each immutable run receipt; this document does not re-query or imply a new device run.
- LM22 endpoint `{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}`; Mi Monitor `{0.0.0.00000000}.{febdb10f-94d3-4af8-af69-b3ac1e887122}`. Admission checks required Active, explicit nondefault muted route, owned renderer and sampled foreign-session guards.
- Latest diagnostic: 48 kHz stereo, actual 480-frame buffer; PCM16 flags `0x88060000`, Float32 flags `0x60000` without AUTO/SRC. Owned renderer destroys/recreates streams. Tests ran serially A/B/B/A without this investigation's builds/FFT running concurrently.
- Source context HEAD `4950696c607c5b77eaba3840b2413ab4702beee2`, dirty branch `fix/issue-96-app-loopback-transition`; HEAD alone does not identify product bytes. 258 current product inputs matched `E:\verification\fung-source-workflow3-20261101\postformat-inputs.json` at completion of E01. That does not retroactively qualify the formatted tree as installed.
- Raw receiver SHA-256 `5BADEFA043865723B8931837E505B07CCEBDF51A59576514B7406C8BE0F3C4B2`; source `C0E272D1336161A9F4C0FE6EC242051C846EC0B9BAF1CC2D8DB8FB48E6662E13`; controller `E3D94F7CB879268460B4B29E9B85800410B9EB9A4A87B226080A0128367C5F27`.
- E03 installed exe SHA-256 `0CEB57CF6500E2CC4DC99A9FDF831348885FFE617EC8C885818F1836F4303CFD`; installer `65932D86B55B7771F0A706133B87258A8AF1D22CE16A4A043BF614F2595BA12A`. Isolated app ID `dev.fung.uat.issue9620261101`; later formatting is not a new installed candidate.

## Data path and observed boundary

```text
Owned source render submission → Windows audio path → PID GetBuffer bytes
                                      └──────────→ endpoint reference
PID bytes → FUNG conversion/downmix → writer → timeline/export/playback
```

The separate C++ receiver copies native bytes before ReleaseBuffer and bypasses the FUNG processing chain. The independent endpoint reference is a comparison control, not physical playback proof or full endpoint acceptance. Missing expected marker content at the receiver boundary is established within matching/coverage limits; the exact internal upstream failure location is not.

## Findings

<a id="FUNG-RCA-000096-01-F01"></a>
### FUNG-RCA-000096-01-F01 — Independent receiver reproduces failure

[E01](#FUNG-RCA-000096-01-E01): all four A/B/B/A runs FAIL; all eight cycle alignments credible. Removing AUTO/SRC and keeping raw Float32 does not remove measured loss. All packet flags are zero; Float32 preceding zeros are native data, not the diagnostic SILENT-fill branch. Each first active packet has 64 leading near-zero frames (exact zeros in Float32; PCM16 magnitude at most 1), then 416 frames matching source 480–895 (both Float32 channels correlation 1.0). This is strong evidence against a FUNG-downstream-only or AUTO/SRC-only explanation in these conditions, not proof that FUNG has no other defects.

<a id="FUNG-RCA-000096-01-F02"></a>
### FUNG-RCA-000096-01-F02 — Stop component is source-lifetime sensitive

[E02](#FUNG-RCA-000096-01-E02): preserving the stopped source-owned client roughly 50–56 ms brings the measured Stop edge within ±48 frames across four cycles, but Resume remains ~441 frames different and whole-run gaps remain. Successful source write receipts are not proof of physical output. The intervention supports lifetime-sensitive delivery/history upstream of the receiver; it neither identifies a particular OS queue nor supplies a foreign-app workaround.

<a id="FUNG-RCA-000096-01-F03"></a>
### FUNG-RCA-000096-01-F03 — QPC gap content remains partly unknown

[E01](#FUNG-RCA-000096-01-E01) also audits 25 gaps from the earlier six recordings. No gap has independently matched active-marker windows on both immediately adjacent 480-frame packets. In 24/25 events read-entry spacing tracks QPC spacing within 1.85 ms; read-entry is before GetBuffer, so native production and scheduling are not isolated. This does not justify either zero-filling presumed silence or equating every QPC leap with exact lost active PCM.

## Hypothesis / experiment matrix

| ID | Hypothesis | Experiment / result | State and limit |
| --- | --- | --- | --- |
| <a id="FUNG-RCA-000096-01-H01"></a>FUNG-RCA-000096-01-H01 | FUNG processing alone causes the observed loss | Independent C++ raw receiver reproduces; E01 | not_supported_in_tested_scope; other product defects not excluded |
| <a id="FUNG-RCA-000096-01-H02"></a>FUNG-RCA-000096-01-H02 | AUTO/SRC conversion alone causes restart loss | PCM16 versus no-AUTO Float32 both fail; E01 | not_supported_in_tested_scope; this changes representation and flags together |
| <a id="FUNG-RCA-000096-01-H03"></a>FUNG-RCA-000096-01-H03 | Source-client lifetime affects upstream retention/delivery | 0/10/50 ms delayed destruction; E02 | supported for measured Stop component; restart mechanism unconfirmed |
| <a id="FUNG-RCA-000096-01-H04"></a>FUNG-RCA-000096-01-H04 | One specific Windows queue or scheduling mechanism explains all gaps | ETW schema query exists, StartTrace returns access denied; no attributable runtime trace; E01 | open; no unique internal cause established |

## Causal analysis and unknowns

Established trigger in the controlled fixture: source Stop/destruction followed by recreation. Established boundary: missing expected marker content is already observed in received raw PID samples. Supported contributing condition: source client lifetime affects the Stop tail. **Confirmed root cause: none yet.**

Do not infer a universal Windows defect, a guaranteed 50 ms repair, or equivalence to every application's pause/resume. Endpoint reference success and successful reader exit codes do not prove PID continuity. Capability-only 0/1/5-second buffer requests all return 480 frames; that is not evidence that changing requested duration repairs capture. No new remedy is claimed here.

## Evidence register

All paths beginning `E:\` below are **machine-local**, not portable repository downloads. Reports are the readable repository evidence; pinned originals stay outside Git. Collection dates are 2026-11-01 unless the linked report explicitly distinguishes historical analysis inputs.

<a id="FUNG-RCA-000096-01-E01"></a>
### FUNG-RCA-000096-01-E01 — Raw receiver A/B/B/A and upstream localization

- Report/protocol: [upstream boundary](../implementation-reports/2026-11-01-pid-upstream-boundary.md), [investigation plan](../../plans/2026-11-01-pid-upstream-boundary.md).
- Root: `E:\verification\fung-pid96-resume-localization-20261101`; executable/controller/source pins above; `run-receiver-comparison.py`, `analyze-receiver.py`, `review/independent-abba-v3.json`, `root-abba-replay-receipt.json`.
- Manifest: `completed-abba-study-manifest.json`, creation `2026-11-01T13:23:25.248350+00:00`; SHA-256 verified for this document: `F79E204B6ACEC6653DB24FF768A5A50351E3FAFBD5A557ACACA6129E50030334`.
- Actual runs: `native/pcm16-b`, `native/float32-a`, `native/float32-b`, `native/pcm16-c`: all FAIL, 4 QPC gaps each; Stop −1984/−1504 frames; Resume +441 throughout. Process exits 0 and clean cleanup mean collection completion only.
- Prior `native/pcm16-a` REFUSED before capture because HDMI unavailable; not a fifth audio failure. Five analyzer unit tests and static review are separate checks. Independent audit and root replay agree.
- Limits: owned fixture, one host, sampled guards, final receiver tails censored because no post-Stop drain; not installed acceptance. Historical onset/cadence and ETW limits are detailed in the linked report. This report does not change immutable study hashes when new docs are added.

<a id="FUNG-RCA-000096-01-E02"></a>
### FUNG-RCA-000096-01-E02 — Source destruction delay study

- Report/protocol: [upstream report](../implementation-reports/2026-11-01-pid-upstream-boundary.md).
- Root: `E:\verification\fung-pid96-release-boundary-20261101`; `completed-native-study-manifest.json` and frozen `run-release-boundary.py`. Digest not independently recomputed for this consolidation; use original manifest pins.
- Six runs in order 0/10/50/50/10/0 ms, all whole-run FAIL. 50 ms arm meets Stop component only; Resume still fails. `delay10-a` cycle 2 retains NOT_PROVEN alignment and extra fifth gap, not dropped as an outlier.
- Limits: controlling one's own source client does not establish control of arbitrary external clients; endpoint reference is not installed endpoint UAT.

<a id="FUNG-RCA-000096-01-E03"></a>
### FUNG-RCA-000096-01-E03 — Installed PID and bounded endpoint/export/recovery

- Report/protocol: [installed workflow evidence](../implementation-reports/2026-11-01-endpoint-export-recovery.md).
- Roots: `E:\verification\fung-source-workflow3-20261101`, `E:\verification\fung-source-workflow3-longmarker-retry-20261101`; package identity above, source-input and preformat snapshots retained there.
- Installed PID recording `c47e1fb7-c35c-4849-8a36-a1c9f347d9e5` FAIL. Endpoint recording `ffda653e-21d4-47f8-8c7c-6ae9f62a929a`: measured source PCM/separation/transition components pass; exported PCM correspondence/reuse and bounded committed-prefix crash recovery have separate positive results.
- Overall endpoint analyzer remains NOT_PROVEN for full workflow. Audible/rendered UI playback, transcription/enrichment and independent journal replay are not qualified by these checks; committed-prefix recovery is not power-loss/volatile-tail recovery. Subsequent formatting did not receive a new installed retest.

<a id="actions"></a>
## Corrective and preventive actions

Owners/deadlines remain unassigned/null; no execution is implied by registration.

| ID | Action and trace | Status | Verification / exit |
| --- | --- | --- | --- |
| <a id="FUNG-RCA-000096-01-A01"></a>FUNG-RCA-000096-01-A01 | Audit prior lifecycle tests and define a genuinely new discriminating experiment for S01/S02/H03 | proposed | Reuse existing evidence first; predeclare same-stream versus recreation hypothesis, immutable inputs and unchanged ±48-frame gate; no redundant capture |
| <a id="FUNG-RCA-000096-01-A02"></a>FUNG-RCA-000096-01-A02 | Repair only a demonstrated mechanism; retain S03 uncertainty in timeline handling | pending-root-cause | Regression proves actual PCM custody and timeline behavior; no fabricated samples or hidden fallback |
| <a id="FUNG-RCA-000096-01-A03"></a>FUNG-RCA-000096-01-A03 | Qualify both actual installed modes and representative external apps | blocked-by-pid-failure | Traceable package/source, PID plus endpoint capture/playback/export/recovery, independent review; no merge/tag until all required gates pass |
| <a id="FUNG-RCA-000096-01-A04"></a>FUNG-RCA-000096-01-A04 | Maintain symptom/evidence IDs and failed-run provenance | ongoing | ADR/templates/indexes and link checks; does not close audio defect |

## Acceptance and review

Keep ±48-frame edge and 0.001 leakage gates unchanged for their applicable protocols; retain whole-run faults and NOT_PROVEN coverage. No format variant, endpoint substitution, parent PID substitution, native helper pass or exit code can qualify an installed PID repair. Required two-mode acceptance remains open. Review of this documentation checks faithful evidence interpretation only.

## Changelog and navigation

- 2026-11-01 — 0.1.0: existing evidence consolidated with stable IDs; root cause stays unconfirmed. No new audio, source fix, package build, merge or tag.

[Incident](../incidents/2026-11-01-issue-96-pid-audio-gap.md) · [RCA index](README.md) · [ADR](../../decisions/2026-11-01-incident-rca-records.md).
