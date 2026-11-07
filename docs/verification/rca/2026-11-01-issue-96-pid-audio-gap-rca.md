---
id: FUNG-RCA-000096-01
title: "PID stream lifecycle gaps — causal boundary investigation"
version: "0.1.3"
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
  - id: FUNG-RCA-000096-01-N04
    target: FUNG-RCA-000096-01-F05
    kind: evidence-boundary
    text: "Reset/recreation comparison intentionally discards queued source data; no foreign-app remedy or full source-continuity proof."
    evidence_refs:
      - FUNG-RCA-000096-01-E05
  - id: FUNG-RCA-000096-01-N05
    target: FUNG-RCA-000096-01-H07
    kind: evidence-boundary
    text: "A matched setup wait did not reproduce the first-Resume deficit in this owned fixture; no arbitrary-app repair or full continuity proof follows."
    evidence_refs:
      - FUNG-RCA-000096-01-E06
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
  - id: FUNG-RCA-000096-01-N03
    target: FUNG-RCA-000096-01-F04
    kind: evidence-boundary
    text: "B matches its endpoint reference but both share source-coordinate overlap; no nonreplayed-source or full-mode PASS."
    evidence_refs:
      - FUNG-RCA-000096-01-E04
---

# RCA FUNG-RCA-000096-01 — PID stream lifecycle gaps

## Summary and causal status

**Root cause: unconfirmed.** หลักฐานระบุ failure boundary ได้ว่าข้อมูลที่ต้องการไม่ครบเมื่อมาถึง PID packet receiver ในชุดทดลองนี้. C++ receiver แยกจาก FUNG ทำให้เกิดซ้ำได้ จึงไม่จำเป็นต้องมี Rust downmix/writer/timeline ของ FUNG จึงจะเกิดอาการ. E06 ไม่สนับสนุนว่าเวลารอ 15.5 ms เพียงอย่างเดียวทำให้เกิด deficit ใน fixture; ยังระบุ Windows internal component, queue หรือกลไกที่เป็นเหตุแน่นอนไม่ได้.

เอกสารนี้เชื่อมหลักฐานแต่ละรอบตาม evidence register; การแก้เอกสารไม่ใช่ product patch หรือ installed acceptance. Owner/reviewer ของ incident ยังไม่ถูกแต่งตั้ง (`null`); วันที่สร้างไม่ใช่วันเริ่มเหตุ.

## Symptoms — references to canonical Incident

| Symptom | Investigation question | Findings |
| --- | --- | --- |
| [FUNG-INC-000096-S01](../incidents/2026-11-01-issue-96-pid-audio-gap.md#FUNG-INC-000096-S01) | Stream lifetime changes delivery at which boundary? | [F01](#FUNG-RCA-000096-01-F01), [F02](#FUNG-RCA-000096-01-F02) |
| [FUNG-INC-000096-S02](../incidents/2026-11-01-issue-96-pid-audio-gap.md#FUNG-INC-000096-S02) | Is restart prefix absent before FUNG processing? | [F01](#FUNG-RCA-000096-01-F01), [F05](#FUNG-RCA-000096-01-F05) |
| [FUNG-INC-000096-S03](../incidents/2026-11-01-issue-96-pid-audio-gap.md#FUNG-INC-000096-S03) | Which QPC gaps contain provable missing signal? | [F03](#FUNG-RCA-000096-01-F03) |

Descriptions, frequency and reproduction prerequisites are maintained only in Incident. Results below are experiment observations supporting the analysis.

## Environment and provenance

- One Windows host; no second host comparison available. Exact OS/device inventory belongs to each immutable run receipt; this document does not re-query or imply a new device run.
- LM22 endpoint `{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}`; Mi Monitor `{0.0.0.00000000}.{febdb10f-94d3-4af8-af69-b3ac1e887122}`. Admission checks required Active, explicit nondefault muted route, owned renderer and sampled foreign-session guards.
- E01 format comparison: 48 kHz stereo, actual 480-frame capture buffer; PCM16 flags `0x88060000`, Float32 flags `0x60000` without AUTO/SRC. Owned renderer destroys/recreates streams. E04 and E05 add lifecycle controls below. Each study runs serially without this investigation's builds/FFT running concurrently with capture.
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

<a id="FUNG-RCA-000096-01-F04"></a>
### FUNG-RCA-000096-01-F04 — Same-client state preservation changes the measured outcome

[E04](#FUNG-RCA-000096-01-E04) compares frozen destruction/recreation against same-client Stop/Start with queued PCM/cursor preserved. Both B runs have credible PID/reference edges within 1 frame and zero QPC gaps; both A runs still fail. B remains NOT_PROVEN for full mode acceptance. Source-coordinate overlap can be present in both reference and PID, so parity does not establish nonreplayed source continuity. In E04, client identity, queue retention and source-position continuation change together. E05 subsequently controls queue discard/cursor/priming while comparing Reset versus recreation, with remaining timing/resource differences explicit.

<a id="FUNG-RCA-000096-01-F05"></a>
### FUNG-RCA-000096-01-F05 — Reset and recreation differ under matched reprime

[E05](#FUNG-RCA-000096-01-E05) controls submitted-end cursor, pending-queue discard, full-capacity priming and stopped-client retention until Resume. Both same-client Reset runs have zero recorded QPC gaps and edges within approximately 1 frame; both recreation runs have gaps and first-Resume deficits of 449/441 frames. Second recreation Resume agrees within 1 frame. Reset/queue clearing alone is therefore not sufficient for this observed deficit under the tested protocol. Creation duration and identity/resource changes remain coupled; root cause is still unconfirmed. Both arms intentionally skip queued source content, so no uninterrupted source-delivery claim follows.

<a id="FUNG-RCA-000096-01-F06"></a>
### FUNG-RCA-000096-01-F06 — Matched setup wait does not reproduce the deficit

[E06](#FUNG-RCA-000096-01-E06) adds a same-client Reset arm whose complete Reset-return→before-prime interval is bounded to 15.500–15.590 ms, matching the upper part of E05's measured 12.37–15.59 ms drop/open duration. Both delayed cycles remain within the unchanged ±48-frame edge bound with zero QPC gaps, like E05's immediate Reset controls. The added wait did not reproduce the recreation arm's first-Resume +441/+449-frame deficit in this owned fixture. This does not prove that delay can never matter, identify an OS component, establish source continuity, or provide a FUNG-controlled arbitrary-app repair.

## Hypothesis / experiment matrix

| ID | Hypothesis | Experiment / result | State and limit |
| --- | --- | --- | --- |
| <a id="FUNG-RCA-000096-01-H01"></a>FUNG-RCA-000096-01-H01 | FUNG processing alone causes the observed loss | Independent C++ raw receiver reproduces; E01 | not_supported_in_tested_scope; other product defects not excluded |
| <a id="FUNG-RCA-000096-01-H02"></a>FUNG-RCA-000096-01-H02 | AUTO/SRC conversion alone causes restart loss | PCM16 versus no-AUTO Float32 both fail; E01 | not_supported_in_tested_scope; this changes representation and flags together |
| <a id="FUNG-RCA-000096-01-H03"></a>FUNG-RCA-000096-01-H03 | Source-client lifetime affects upstream retention/delivery | 0/10/50 ms delayed destruction; E02 | supported for measured Stop component; restart mechanism unconfirmed |
| <a id="FUNG-RCA-000096-01-H04"></a>FUNG-RCA-000096-01-H04 | One specific Windows queue or scheduling mechanism explains all gaps | ETW schema query exists, StartTrace returns access denied; no attributable runtime trace; E01 | open; no unique internal cause established |
| <a id="FUNG-RCA-000096-01-H05"></a>FUNG-RCA-000096-01-H05 | Same-client state-preserving Stop/Start differs from destruction/recreation at the capture boundary | Four serial A/B/B/A runs; B edge parity/no QPC gaps, A loss/gaps; E04 | supported for this combined intervention; E05 controls queue/cursor/prime, with identity/duration still coupled |
| <a id="FUNG-RCA-000096-01-H06"></a>FUNG-RCA-000096-01-H06 | Reset/clearing queued PCM alone explains the PID deficit | Same-client Reset versus recreation, equal cursor/prime; E05 | not_supported_in_tested_scope; recreation duration/identity remain coupled |
| <a id="FUNG-RCA-000096-01-H07"></a>FUNG-RCA-000096-01-H07 | Elapsed setup delay alone explains the first-Resume deficit | Same-client Reset with complete 15.500–15.590 ms Reset-return→before-prime interval; E06 | not_supported_in_tested_scope; one owned fixture/host, not a foreign-app intervention |

## Causal analysis and unknowns

Established trigger in the controlled fixture: source Stop/destruction followed by recreation. Established boundary: missing expected marker content is already observed in received raw PID samples. Supported contributing condition: source client lifetime affects the Stop tail. **Confirmed root cause: none yet.**

Do not infer a universal Windows defect, a guaranteed 50 ms repair, or equivalence to every application's pause/resume. Endpoint reference success and successful reader exit codes do not prove PID continuity. Capability-only 0/1/5-second buffer requests all return 480 frames; that is not evidence that changing requested duration repairs capture. No new remedy is claimed here.

## Evidence register

All paths beginning `E:\` below are **machine-local**, not portable repository downloads. Reports are the readable repository evidence; pinned originals stay outside Git. Collection dates are 2026-11-01 unless the linked report explicitly distinguishes historical analysis inputs.

<a id="FUNG-RCA-000096-01-E01"></a>
### FUNG-RCA-000096-01-E01 — Raw receiver A/B/B/A and upstream localization

- Report/protocol: [upstream boundary](../implementation-reports/2026-11-01-pid-upstream-boundary.md), [investigation plan](../../plans/2026-11-01-pid-upstream-boundary.md).
- Root: `E:\verification\fung-pid96-resume-localization-20261101`; executable/controller/source pins above; `run-receiver-comparison.py`, `analyze-receiver.py`, `review/independent-abba-v3.json`, `root-abba-replay-receipt.json`.
- Manifest: `completed-abba-study-manifest.json`, creation `2026-11-01T13:23:25.248350+00:00`; the RCA v0.1.2 hash recorded at that consolidation was `F79E204B6ACEC6653DB24FF768A5A50351E3FAFBD5A557ACACA6129E50030334`. Later E06 documentation is not part of immutable E01.
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

<a id="FUNG-RCA-000096-01-E04"></a>
### FUNG-RCA-000096-01-E04 — Same-client lifecycle A/B/B/A

- Report: [same-client follow-up](../implementation-reports/2026-11-01-pid-upstream-boundary.md#same-client-stopstart-discriminator--follow-up-after-documentation-commit); collected 2026-11-01 after documentation commit `17a01fb`.
- Machine-local root `E:\verification\fung-pid96-same-client-20261101`; prospective `PLAN.md`, `build-receipt.json`, `study-summary.json`, four `*-analysis.json` and immutable native run directories. Commands/protocol are `run-lifecycle.py` and `analyze-lifecycle.py` under that root.
- B renderer SHA-256 `EDE740C19B766BD757902C7900E66D0BDE7356ACF7F122C384C094AD1DE1BD20`; source `30B1E948146443824F6E4032692612156DBB4130A8BDAA4796BD854B748A8C9A`. Frozen A and Float32 receiver retain E01 identities. Root verified all 358 recorded per-run input hashes.
- `recreate-b` and `recreate-c`: FAIL; Stop −1504 or −1984, Resume +441, 4 gaps each. `same-client-a` and `same-client-b`: measured edges within 1 frame, 0 gaps, overall NOT_PROVEN. All four actual collections exit cleanly. Initial `recreate-a` pre-GO tree-guard refusal is not an audio run.
- No Reset/rewind/paused feed in B; valid frozen clock and counters verified, event order and binary/mode binding checked. Nine analyzer tests pass. Native lifecycle discrimination only; not an installed repair, nonreplayed source proof or release gate closure.
- Independent audit `review/independent-audit.json` verifies 117 inventory snapshots, native byte custody and local source-anchor correlations; root replay is identical. B shares approximately 400–428 frames of source overlap with the endpoint across its four cycles. `completed-study-manifest.json` hashes 388 artifact files (excluding build target/cache); SHA-256 `546C9964C312795D96ECCF9ECF229B094F5B4021FE1B45D827C3F0655EDECDE5`.

<a id="FUNG-RCA-000096-01-E05"></a>
### FUNG-RCA-000096-01-E05 — Reset versus recreation C/D/D/C

- Report: [matched reprime follow-up](../implementation-reports/2026-11-01-pid-upstream-boundary.md#reset-versus-recreation-with-matched-reprime--cddc); machine-local root `E:\verification\fung-pid96-reset-recreate-20261101`.
- Completed order `reset-b`, `recreate-a`, `recreate-b`, `reset-c`: NOT_PROVEN / FAIL / FAIL / NOT_PROVEN. Thirteen analyzer tests pass; both compiled renderers build without warnings. Protocol/source/controller/analyzer/binary hashes are recorded in the report and analysis receipts.
- Eight credible cycles; Reset has no recorded QPC gaps and all edges within 1 frame. Recreation has 4/3 gaps, first Resume +449/+441 frames, second −1/0 frames. Intentional endpoint source skips ~3914–4406 frames and operation-time differences remain explicit.
- Independent audit verifies 113 inventory snapshots, packet custody, clearing/priming/cursor/epoch/write timing and source anchors. Root replay JSON is identical, SHA-256 `8BA50A8A08D385A6069B565D67875FCAF0E860C3CD8CBBB922C58E3B17B491B0`.
- Completed-study manifest seals 468 artifacts, SHA-256 `D24765A0B9B3A1A111BAD56996AFD11663CCF0A7F7EDFCEC6C0FB8FA626E08C3`; 258 product inputs and 360 analysis/input pins verify unchanged. Strict current-tree all-target clippy exits 0 after capture; no installed acceptance follows.
- `reset-a` is guard-interrupted after capture began, subject exit 0/reference forced. Snapshot retains the subject's Active session; exact process-exit time relative to enumeration is not established by these receipts. It is excluded from the completed comparison and retained separately.
- Limits: owned source intervention, one host, API duration differences, deliberate queue discard, sampled guards, censored final receiver tails. No arbitrary-app repair or new installed qualification.

<a id="FUNG-RCA-000096-01-E06"></a>
### FUNG-RCA-000096-01-E06 — Same-client Reset with matched setup delay

- Protocol and machine-local evidence: `E:\verification\fung-pid96-setup-delay-20261101-v3`; run `native/delay-v3-a`; analysis `delay-v3-a-analysis.json`.
- Preparation manifest SHA-256 `AD03D38407A5966F952BD20752438B54D02255A5308C37B39F24FCB03FF82C6B`; all 10 manifest files, 21 receiver pins and 87 input hashes were independently verified. E05, v1 and v2 sealed/preparation inputs remained unchanged.
- Renderer SHA-256 `EB00B1D86F7C25159F1E45E15385AD3BEDDE009712681A31BB1D75D5B4049D25`; source `A4F6E0AEA847F70F7326F483DB1033BBB2A28240C44B620A6F4D21865AB98677`; controller `2C837B24577D5DD4405DCD993572A936B4DE894FF6BDF73EB142D103293A83EC`; analyzer `886B734B712434914C35D7A33A9BD9B79E96DAD1F3BE623E566FA9520C635E5F`.
- Analyzer result SHA-256 `0C38387053D02FDE63EFA69DF3409D84BCCABC50D82701EE56F7261A2CC9C71B`; raw Float32 stereo SHA-256 `73C98091880C76C427893EC78AAD9076329ADF6F58DDA387DF36F839FE849997`; metadata `C4E29603D3B2E5CA4896BC1B0EA90557652346E09C75523A60E82C044510FEAB`.
- Python 3.12.10 with NumPy 2.5.4; command: `uv run --no-project --python 3.12.10 --with numpy==2.5.4 python analyze-lifecycle.py --run <absolute-run-path> --out <fresh-absolute-output-path>`.
- Actual Reset-return→prime: 15.5000/15.5001 ms. PID raw receiver: 959,040 Float32 stereo frames, 480-frame packets, flags `0x60000`, zero native QPC gaps and native problems. Stop deltas effectively 0/0 frames; Resume 0/−1 frame, both cycles within ±48. Independent endpoint reference marker correlations are at least 0.9999879.
- Controller, owned renderer/supervisor, PID receiver and endpoint reference exited 0 without forced termination or cleanup/reference faults. Exact LM22-B200S nondefault endpoint was active and muted; 28 inventory snapshots and 29 mute receipts passed. Guards are sampled, not continuous.
- Independent post-capture reviews rechecked the 87 input hashes and 21 receiver pins, 4,800-sample stereo marker windows per epoch (channel correlations approximately 1.0), target PID/creation identity, reference endpoint/faults and process exits. The endpoint review confirmed the 28 sampled inventory and 29 mute receipts; these checks do not prove continuous isolation outside observation windows.
- Whole diagnostic verdict **NOT_PROVEN**, not PASS. The fixture intentionally discards queued source PCM; source-coordinate skips are about 4,389/4,395 frames and final receiver tails are censored. This is not installed FUNG acceptance. No product source changed and no package was rebuilt.

<a id="actions"></a>
## Corrective and preventive actions

Owners/deadlines remain unassigned/null; no execution is implied by registration.

| ID | Action and trace | Status | Verification / exit |
| --- | --- | --- | --- |
| <a id="FUNG-RCA-000096-01-A01"></a>FUNG-RCA-000096-01-A01 | Audit prior lifecycle tests and run discriminators for S01/S02/H03/H05/H06/H07 | diagnostic-complete | E04/E05/E06 narrow lifecycle sensitivity; product root cause and general remedy remain open |
| <a id="FUNG-RCA-000096-01-A02"></a>FUNG-RCA-000096-01-A02 | Repair only a demonstrated mechanism; retain S03 uncertainty in timeline handling | pending-root-cause | Regression proves actual PCM custody and timeline behavior; no fabricated samples or hidden fallback |
| <a id="FUNG-RCA-000096-01-A03"></a>FUNG-RCA-000096-01-A03 | Qualify both actual installed modes and representative external apps | blocked-by-pid-failure | Traceable package/source, PID plus endpoint capture/playback/export/recovery, independent review; no merge/tag until all required gates pass |
| <a id="FUNG-RCA-000096-01-A04"></a>FUNG-RCA-000096-01-A04 | Maintain symptom/evidence IDs and failed-run provenance | ongoing | ADR/templates/indexes and link checks; does not close audio defect |

## Acceptance and review

Keep ±48-frame edge and 0.001 leakage gates unchanged for their applicable protocols; retain whole-run faults and NOT_PROVEN coverage. No format variant, endpoint substitution, parent PID substitution, native helper pass or exit code can qualify an installed PID repair. Required two-mode acceptance remains open. Review of this documentation checks faithful evidence interpretation only.

## Owner architecture decision requested

The current supported arbitrary-process path is [Windows Process Loopback](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/), which is the same facility FUNG already uses. [Endpoint loopback](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording) captures an endpoint mix, while [IAudioSessionManager](https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nn-audiopolicy-iaudiosessionmanager) enumerates/control sessions rather than expose another process's PCM. Research found no documented generic Windows API that lets FUNG take control of a foreign application's renderer/client lifetime; this is an investigation limit, not proof of a universal Windows defect.

Keep the existing dual-mode installed acceptance and release block unless the owner explicitly changes product scope. The owner decision is whether to (A) continue PID research only when a new supported mechanism or reproducible discriminator is available, or (B) authorize a separate opt-in endpoint-routed product scope with explicit routing/mix limitations and its own completed installed acceptance. Option B would not qualify the currently incomplete endpoint package and would require an ADR that explicitly changes the existing release policy. No gate is changed by this RCA.

## Changelog and navigation

- 2026-11-01 — 0.1.3: add F06/H07/E06 matched timing discriminator and owner architecture decision request; root cause unconfirmed, dual-mode gate unchanged.
- 2026-11-01 — 0.1.2: add F05/H06/E05 from matched Reset/recreation study; preserve interrupted evidence and installed release gates.

- 2026-11-01 — 0.1.1: add F04/H05/E04 after same-client native comparison; keep root cause unconfirmed and release blocked.

- 2026-11-01 — 0.1.0: existing evidence consolidated with stable IDs; root cause stays unconfirmed. No new audio, source fix, package build, merge or tag.

[Incident](../incidents/2026-11-01-issue-96-pid-audio-gap.md) · [RCA index](README.md) · [ADR](../../decisions/2026-11-01-incident-rca-records.md).
