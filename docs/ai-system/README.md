---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "core-directive"
  scope: "FUNG AI system"
---

# FUNG AI System Documentation

This directory documents model-backed behavior as a governed system: data,
runtime, model, prompt, evaluation, human review, and operational controls.
It is not a claim that every target capability is implemented.

## System boundary

```text
audio / meeting input
        ↓
capture and durable job boundary
        ↓
STT / diarization / enrichment models
        ↓
transcript and evidence-bearing meeting artifacts
        ↓
local persistence, review, export, or approved external action
```

FUNG remains local-first. AI workers must preserve provenance, degradation
state, and the distinction between a model suggestion and verified truth.
GenesisBlockDB remains the persistence boundary; model workers and agents do
not open an independent application database.

## Documentation map

| Document | Purpose | Status |
|---|---|---|
| [`agent-architecture.md`](agent-architecture.md) | Agent, tool, API, job, and model boundaries | candidate |
| [`data-pipeline.md`](data-pipeline.md) | Audio/data lineage and egress controls | candidate |
| [`model-lifecycle.md`](model-lifecycle.md) | Intake, evaluation, promotion, monitoring, retirement | candidate |
| [`evaluation-plan.md`](evaluation-plan.md) | Quality, performance, safety, and reproducibility evidence | candidate |
| [`ethics-governance.md`](ethics-governance.md) | Consent, privacy, speaker identity, and human oversight | candidate |
| [`prompt-engineering.md`](prompt-engineering.md) | Versioned prompt and structured-output controls | candidate |
| [`model-cards/TEMPLATE.md`](model-cards/TEMPLATE.md) | Per-model deployment and limitation record | active template |

## Current versus target truth

- `Desktop/ARCHITECTURE.md` is the current parent architecture and records
  BYOM, stateful jobs, Tauri, local API, MCP, CLI, and Genesis boundaries.
- This directory defines the control plane for AI quality and governance; it
  does not promote a model, claim GPU readiness, or authorize cloud egress.
- A model-specific card must identify the exact model, runtime, quantization,
  hardware, dataset/evaluation version, and evidence before a quality claim is
  published.

## Minimum quality dimensions

Every production-bound audio/AI change should address the dimensions relevant
to it: transcription accuracy, diarization quality, summary fidelity,
latency, memory/VRAM, failure isolation, privacy, reproducibility, and human
reviewability.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added AI system documentation map and control boundary. | pending | ATHER |
