---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG model card template"
---

# Model Card — [Model / Provider]

## Document control

| Field | Value |
|---|---|
| Model owner | [name/role] |
| Task | [STT / diarization / summarization / other] |
| Lifecycle status | `proposed` / `evaluating` / `candidate` / `approved` / `deprecated` |
| Approval record | [link or pending] |
| Related evaluation | [link] |
| Related requirement/decision | [IDs/links] |

## Model identity

| Field | Value |
|---|---|
| Exact name and version/digest | [value] |
| Provider / licence | [value and link] |
| Runtime and package versions | [value] |
| Quantization / precision | [value] |
| Prompt/template version | [hash/link or none] |
| Supported languages/domains | [value] |
| Input/output formats | [value] |

## Deployment profile

| Field | Value |
|---|---|
| Execution boundary | local / approved external / hybrid |
| CPU/GPU | [exact hardware] |
| VRAM | [value] |
| RAM | [value and configuration] |
| Driver/runtime | [CUDA/CPU/other] |
| Expected latency | [workload and measured value] |
| Cancellation/retry behavior | [value] |

## Intended use and limitations

### Intended use

[What the model is approved to do.]

### Prohibited or unsupported use

- [identity or consequential use not validated]
- [language, noise, speaker count, or duration limitations]

## Evaluation

| Metric | Dataset/version | Result | Threshold | Evidence |
|---|---|---:|---:|---|
| [WER/CER/DER/etc.] | [version/hash] | [value] | [approved threshold] | [link] |

Include representative examples, failure cases, confidence/degradation
behavior, and comparison with the current baseline.

## Data and privacy

- Training/fine-tuning data provenance: [value]
- Evaluation data consent/licence: [value]
- Raw input retention: [value]
- Derived artifact retention: [value]
- Egress and provider processing: [value]
- Secret handling: [value]
- Speaker identity/biometric risk: [value]

## Monitoring and rollback

- Runtime health signal: [value]
- Quality drift signal: [value]
- Incident threshold: [value]
- Rollback model/config: [value]
- Owner and escalation: [value]

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Initial model-card template. | pending | ATHER |
