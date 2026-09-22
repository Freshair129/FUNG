---
version: "0.2.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG AI evaluation"
---

# AI Evaluation Plan

This plan defines what must be measured before an AI change is described as
high quality. Thresholds are model/task-specific and must be approved in the
model card; this scaffold does not invent a passing score.

## Evaluation matrix

| Dimension | Example measure | Required comparison | Evidence |
|---|---|---|---|
| Transcription | WER/CER, Thai word/character error | baseline and representative audio | immutable run report |
| Speaker separation | DER/JER and attribution accuracy | labelled multi-speaker set | diarization report |
| Summary fidelity | omission, contradiction, unsupported claim rate | human-reviewed reference | review sheet/report |
| Structured output | schema validity and field completeness | required output contract | machine-readable test |
| Latency | end-to-end and stage p50/p95 | target hardware and workload | timed run manifest |
| Resource use | RAM, VRAM, CPU/GPU utilisation | declared hardware profile | hardware snapshot |
| Robustness | noise, overlap, accents, long files, interruption | failure fixture set | failure matrix |
| Privacy/safety | secret leakage, unauthorised egress, unsafe identity claim | negative tests | scan and audit evidence |

## Dataset controls

Every evaluation set must record source, consent/licence, language/domain,
speaker balance, audio conditions, annotation method, version/hash, and known
limitations. Sensitive audio must remain in its approved boundary.

## Run manifest

Record at minimum:

- model/provider and exact version or digest;
- runtime and package versions;
- prompt/template and preprocessing versions;
- hardware model, RAM, VRAM, driver/runtime details;
- dataset version/hash and sample count;
- start/end time, retries, failures, and output artifact hashes.

## Acceptance gate

A model may be promoted only when the owner can answer:

1. Is it better than or acceptably equivalent to the current baseline for the
   declared task and language?
2. Are failure modes and uncertainty visible to the user?
3. Does it meet resource and latency constraints on the declared hardware?
4. Are privacy, consent, egress, and retention controls verified?
5. Is rollback possible without losing durable local data?

If any answer is unknown, status remains `evaluating` or `candidate`.

## Meeting intelligence qualification — candidate extension

Use the requirement/test matrices in [Live transcript](../specs/2026-09-21-live-meeting-transcription-spec.md), [Knowledge evidence](../specs/2026-09-21-meeting-knowledge-evidence-spec.md), [Meeting Agent](../specs/2026-09-21-meeting-agent-participation-spec.md) and [Google Meet strategy](../decisions/2026-09-21-google-meet-agent-api-strategy.md).

| Track | Measure separately |
| --- | --- |
| Live ASR | WER/CER, first-partial and durable-final latency, RTF, churn, coverage/gaps, 3-hour CPU/GPU soak |
| API source attribution | misattributed duration, unmapped duration, shared-room ambiguity, rejoin/track-slot changes; not biometric accuracy |
| Voice recognition | false accept/reject/unknown, calibrated thresholds and consent-scoped gallery; not required for API-attributed MVP |
| Knowledge | relevant evidence retrieval, claim support/citation correctness, company/fiscal year/unit consistency, numeric exactness |
| Proactive agent | relevant/irrelevant triggers, interruptions/hour, stale/self-echo suppression, response latency and cost |
| Publication | wrong-room/unauthorized-source exposure, duplicate sends, unknown-send reconciliation, revoke races and real recipient access |
| Operations | actual admission/chat/media permissions, provider/gateway outage, lease cleanup and deletion receipts |

Release safety fixtures require zero unauthorized/wrong-room sends and zero silent duplicate publications in the defined suite. This is a required test result, not a measured result or statistical guarantee. Quality scores and numeric latency budgets in new specs are candidate targets requiring approved datasets/hardware.

Evidence layers must stay separate: document validation, unit/contract fixtures, local integration, real vendor API, real Google Meet room, packaged Desktop and deployed gateway. No mocked event stream, Google Chat message or private local draft closes same-Meet delivery acceptance.

## Evidence naming

Use a stable, reviewable pattern such as:

```text
verification/ai/<model-or-run>/<yyyy-mm-dd>-<dataset-version>-<run-id>/
```

Keep raw sensitive inputs out of source control; store only approved references,
metrics, manifests, and redacted examples.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Added live/API-attribution, grounded-knowledge, proactive-agent and same-room delivery qualification tracks. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-09-21 | candidate | Added live/API-attribution, grounded-knowledge, proactive-agent and same-room delivery qualification tracks. | working-tree | RWANG |
| 0.1.0b | 2026-08-23 | candidate | Added AI quality, performance, safety, and reproducibility gates. | pending | ATHER |
