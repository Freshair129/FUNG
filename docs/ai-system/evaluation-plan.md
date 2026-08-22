---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
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

## Evidence naming

Use a stable, reviewable pattern such as:

```text
verification/ai/<model-or-run>/<yyyy-mm-dd>-<dataset-version>-<run-id>/
```

Keep raw sensitive inputs out of source control; store only approved references,
metrics, manifests, and redacted examples.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added AI quality, performance, safety, and reproducibility gates. | pending | ATHER |
