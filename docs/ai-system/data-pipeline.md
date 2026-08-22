---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG audio and AI data lineage"
---

# AI Data Pipeline and Lineage

## Logical flow

```text
source audio
  → capture metadata and durable chunks
  → normalised audio / preprocessing
  → speech-to-text segments
  → optional diarization / speaker hypotheses
  → reviewed transcript
  → meeting intelligence and artifacts
  → local persistence / export / approved external delivery
```

The flow describes the governed boundary. A line in this document is not
evidence that the corresponding worker or UI is complete.

## Data classes

| Data | Classification | Required controls |
|---|---|---|
| Raw audio | Sensitive | local-first storage, consent, retention, access control |
| Audio-derived features | Sensitive | minimise, document purpose, avoid unnecessary export |
| Transcript | Confidential/sensitive | provenance, review state, scoped access, redaction where needed |
| Speaker hypothesis/profile | Sensitive/biometric-risk | opt-in design, explicit purpose, no silent identity claim |
| Summary/action candidates | Confidential | model provenance, human review, distinguish candidate from fact |
| Run manifest | Internal | model/runtime/config/hash, no secrets or raw content by default |

## Lineage requirements

Each artifact should be traceable to:

- source asset identifier and capture timestamp;
- preprocessing and segmentation configuration;
- model name, version, runtime, device, and quantization;
- prompt/template version for generated text;
- parent job and retry/attempt identifier;
- human edits, review state, and export/delivery decision.

## Egress rules

- Raw audio and full transcript remain local unless a separately approved
  egress requirement exists.
- External payloads contain only approved fields and selected evidence refs.
- Credentials never appear in GenesisBlockDB records, logs, exports, prompts,
  model output, or test snapshots.
- If a worker fails, the pipeline records an unavailable/degraded state and
  preserves durable local capture; it must not fabricate completion.

## Retention and deletion

Retention is a product/privacy decision, not a model default. Each deployment
must record the retention period, deletion authority, backup implications, and
whether derived artifacts are deleted with the source audio.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added AI data lineage, classes, and egress rules. | pending | ATHER |
