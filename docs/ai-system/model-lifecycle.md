---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG model lifecycle"
---

# Model Lifecycle

## Lifecycle states

| State | Meaning | Exit evidence |
|---|---|---|
| Proposed | Model or provider is being considered | owner, purpose, risk, license |
| Evaluating | Reproducible benchmark is running | versioned dataset and run manifest |
| Candidate | Results are reviewable but not production-authorized | model card, limitations, rollback |
| Approved | Owner has accepted scope and quality gates | signed/recorded approval and release gate |
| Monitored | Approved model is used with operational observation | quality/performance incident records |
| Deprecated | New work must not select the model | replacement and migration/rollback plan |
| Retired | Model is disabled and artifacts are handled per retention policy | disablement and archive evidence |

## Promotion gates

1. **Purpose and risk:** define the task, users, data class, human impact, and
   prohibited uses.
2. **Reproducibility:** pin model/provider version, runtime, prompt/config,
   hardware, dataset version, and randomisation where relevant.
3. **Quality:** record task metrics and representative Thai/audio cases; do not
   publish a quality claim without a baseline and limitations.
4. **Safety and privacy:** verify consent, retention, secret handling, egress,
   and speaker-identity boundaries.
5. **Operational readiness:** verify latency, resource usage, cancellation,
   retry, degradation, and rollback behavior.
6. **Approval:** record the decision and link the model card, evaluation report,
   implementation plan, and verification evidence.

## Change triggers

Re-evaluation is required when any of the following changes: model weights,
provider, quantization, runtime, prompt contract, preprocessing, language
coverage, hardware class, dataset mix, privacy/egress policy, or output schema.

## Incident response

When a model produces unsafe, materially inaccurate, privacy-violating, or
non-reproducible output: mark the run degraded, preserve the evidence, stop
promotion/delivery if needed, evaluate rollback, and record the root cause and
prevention. A model output must not be silently corrected into a false “truth”.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added lifecycle states and promotion gates. | pending | ATHER |
