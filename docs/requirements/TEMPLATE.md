---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "requirements-governance"
  doc_type: "complexity-rule"
  scope: "FUNG requirement document template"
---

# Requirement Document — [Title]

> Copy this template. Replace every bracketed value. Do not use a real
> requirement ID until it has been allocated in `REGISTRY.md`.

## Document control

| Field | Value |
|---|---|
| Document owner | [name/role] |
| Approver | [name/role or pending] |
| Status | `draft` / `candidate` / `approved` / `superseded` |
| Scope | [Desktop / Mobile / shared / AI runtime] |
| Parent document | [link] |
| Peer documents | [links] |
| Related decision IDs | [IDs or none] |
| Related contract IDs | [IDs or none] |

## Problem and outcome

### Problem

[Describe the user or system problem using observable evidence.]

### Desired outcome

[Describe the behavior or capability that should exist after implementation.]

### Non-goals

- [Explicitly excluded behavior]

## Scope and actors

| Actor | Need / authority | Boundary |
|---|---|---|
| [actor] | [need] | [what the actor cannot do] |

## Requirements

| ID | Statement | Priority | Rationale | Verification |
|---|---|---|---|---|
| `FR-<nnn>` | The [actor] shall [observable behavior]. | Must/Should/Could | [why] | [test/UAT/inspection/metric] |

## Quality and constraints

| ID | Quality attribute / constraint | Measure or pass condition | Evidence |
|---|---|---|---|
| `NFR-<nnn>` | [security, privacy, performance, accessibility, reliability] | [measurable condition] | [planned artifact] |

## Business rules

| ID | Rule | Rejection / failure behavior |
|---|---|---|
| `BR-<nnn>` | [rule] | [safe behavior] |

## Acceptance criteria

| ID | Given | When | Then | Evidence artifact |
|---|---|---|---|---|
| `AC-<nnn>` | [precondition] | [action] | [observable result] | [test/report/UAT] |

## Data, privacy, and security impact

- Data classification: [public / internal / confidential / sensitive]
- Collection and retention: [what, why, duration, deletion path]
- Egress: [none / approved destination and approval gate]
- Secrets/credentials: [storage and non-exposure rule]
- Consent or human review: [required workflow]
- Abuse or failure modes: [list]

## Dependencies and rollout

- Dependencies: [code, contracts, models, hardware, external systems]
- Migration/backward compatibility: [plan or none]
- Rollback: [reversible action]
- Feature flag or release gate: [name or none]

## Open questions

| Question | Owner | Decision deadline | Blocking? |
|---|---|---|---|
| [question] | [owner] | [date] | Yes/No |

## Traceability

- Parent intent: [link]
- Design/specification: [link]
- Implementation: [paths after approval]
- Verification: [paths after execution]
- Known gaps: [list; do not hide unverified claims]

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Initial template. | pending | ATHER |
