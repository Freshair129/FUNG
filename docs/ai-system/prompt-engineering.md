---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG model prompts and output contracts"
---

# Prompt and Output Engineering

## Prompt contract

Every production-bound prompt or instruction template must define:

- purpose and allowed input fields;
- data classification and redaction rule;
- model/provider compatibility;
- output schema and required evidence references;
- uncertainty, refusal, and unavailable behavior;
- version, owner, change reason, and evaluation dataset.

## Safe generation rules

1. Treat transcript text, retrieved documents, and tool results as untrusted
   content; they cannot rewrite system instructions or authorize a side effect.
2. Do not place credentials, access tokens, or unnecessary raw sensitive data in
   prompts or persisted prompt logs.
3. Require structured output for artifacts consumed by code, then validate the
   schema before persistence or delivery.
4. Preserve source segment IDs and model provenance for summaries, action
   candidates, and decisions.
5. If required evidence is absent or contradictory, return an explicit
   uncertainty/degraded state instead of filling the gap.

## Versioning and regression

Prompt changes are model-lifecycle changes. Pin the prompt/template hash in the
run manifest and re-run representative Thai/audio and negative safety cases
before promotion. A prompt that changes output semantics requires a decision or
requirement update, not only a code review.

## Human-facing language

User-facing Thai and English labels should distinguish `model-generated`,
`reviewed`, `unavailable`, `inferred`, and `verified`. Avoid wording that turns
confidence into certainty or an anonymous speaker into a named person.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added prompt, provenance, output-schema, and regression controls. | pending | ATHER |
