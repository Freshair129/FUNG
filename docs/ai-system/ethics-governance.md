---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG AI ethics and governance"
---

# AI Ethics and Governance

## Principles

- Local-first by default; minimise collection and external transfer.
- A transcript, diarization result, summary, or action candidate is evidence
  produced by a system, not automatically verified truth.
- Human review is required for sensitive identity claims, consequential
  decisions, external delivery, and corrections that alter meaning.
- The UI must expose model origin, timestamp, review state, and degraded or
  unavailable conditions.
- Consent, purpose limitation, retention, deletion, and access must be
  explicit for meeting audio and derived artifacts.

## Speaker identity and voice profiles

Voice recognition or persistent speaker profiles are a separate high-risk
capability. They require a dedicated approved requirement and design covering
consent, enrolment, false-match handling, access, deletion, retention, and
prohibited uses. Diarization may label an anonymous speaker segment without
claiming a real-world identity.

## Risk register

| Risk | Preventive control | Detection | Response |
|---|---|---|---|
| Hallucinated summary/action | evidence refs, review state, structured output | human review and contradiction checks | mark degraded, correct with provenance |
| Wrong speaker attribution | anonymous labels by default, confidence/overlap handling | labelled evaluation set | remove identity claim, re-review |
| Unauthorised egress | default-deny capability policy and minimisation | audit/secret scans | deny, revoke, investigate |
| Sensitive data over-retention | explicit retention and deletion path | periodic inventory | delete/contain, record incident |
| Model/provider drift | pinned versions and lifecycle gates | regression evaluation | hold promotion or rollback |

## Human review record

For reviewed artifacts, record reviewer role, review time, decision, changed
fields, evidence refs, and unresolved uncertainty. Do not overwrite the raw
model result without retaining the provenance link.

## Incident handling

Privacy, identity, security, or materially misleading-output incidents follow
the repository RCA rule: symptom, evidence, root cause, why detection failed,
and prevention. External delivery is paused when approval or provenance cannot
be established.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added AI ethics, speaker identity, and risk controls. | pending | ATHER |
