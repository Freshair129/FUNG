---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "requirements-governance"
  doc_type: "core-directive"
  scope: "FUNG"
---

# FUNG Requirements

This directory is the governed home for new cross-domain requirements and
requirements that are explicitly migrated from an existing product document.
It does not silently replace current-state evidence in `Desktop/`, `Mobile/`,
or historical verification reports.

## Source-of-truth rule

- A requirement has one canonical definition.
- Existing identifiers remain valid during migration. The current live-meeting
  source for `FR-101` through `FR-116`, `NFR-101` through `NFR-110`,
  `BR-101` through `BR-108`, and `AC-101` through `AC-112` remains
  `Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md`.
- A new requirement belongs here when it is reusable across Desktop, Mobile,
  API, CLI, MCP, or AI runtime boundaries.
- A specification may refine a requirement, but it must link back to the
  canonical ID rather than copy a competing statement.

## Requirement lifecycle

```text
problem / opportunity
        ↓
candidate requirement
        ↓ approval
approved requirement
        ↓ design + plan
implemented requirement
        ↓ evidence
verified requirement
        ↓ retirement decision
deprecated / superseded
```

`draft` and `candidate` documents are proposals. They do not authorize code,
schema, credential, deployment, or external-delivery changes.

## Identifier scheme

| Prefix | Meaning |
| --- | --- |
| `FR-<nnn>` | Functional requirement |
| `NFR-<nnn>` | Non-functional requirement |
| `BR-<nnn>` | Business rule |
| `AC-<nnn>` | Acceptance criterion |
| `DR-<nnn>` | Data requirement |
| `IR-<nnn>` | Infrastructure requirement |
| `SEC-<nnn>` | Security requirement |
| `SDD-<nnn>` | Software/design decision reference |
| `AI-AGT-<nnn>` | AI agent/runtime requirement |
| `AI-ETH-<nnn>` | AI ethics and governance requirement |

Use three digits, allocate monotonically, never reuse an identifier, and
record the allocation in [`REGISTRY.md`](REGISTRY.md). The registry is an
index, not a second definition of the requirement.

## Required requirement fields

Every approved requirement must state:

- actor and observable behavior;
- priority and rationale;
- scope and explicit non-goals;
- dependencies and affected contracts;
- verification method and acceptance evidence;
- privacy, security, accessibility, and operational impact where applicable;
- owner, approver, status, and revision history.

Use [`TEMPLATE.md`](TEMPLATE.md) for new documents.

## Review gates

1. Requirements review confirms intent, scope, IDs, and measurable acceptance.
2. Architecture/design review checks parent and peer documents before an
   implementation plan is written.
3. Implementation is allowed only after the requirement/specification is
   approved by the responsible owner.
4. Verification records the actual command, environment, result, and remaining
   gates; a passing unit test is not evidence of device, production, or human
   UAT unless it actually ran those checks.

## Related documents

- [`../README.md`](../README.md) — document map and lifecycle
- [`../architecture/README.md`](../architecture/README.md) — architecture views
- [`../ai-system/README.md`](../ai-system/README.md) — AI/ML-specific controls
- [`../Desktop/ARCHITECTURE.md`](../Desktop/ARCHITECTURE.md) — current parent architecture
- [`../appendices/D-traceability.md`](../appendices/D-traceability.md) — current traceability evidence

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added governed requirements directory and lifecycle rules. | pending | ATHER |
