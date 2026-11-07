---
version: "0.2.1"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-11-01"
status: "candidate"
superseded_by: null
attributes:
  domain: "decision-governance"
  doc_type: "core-directive"
  scope: "FUNG architecture and technical decisions"
---

# FUNG Decision Records

`docs/decisions/` contains architecture and technical decisions that explain
why a materially important choice was made. A decision record does not replace
requirements, a design specification, or verification evidence.

## Statuses

- `candidate`: proposed; implementation is not authorized by this file alone.
- `approved`: selected by the responsible owner; implementation may proceed
  only when the related requirement/specification is also approved.
- `superseded`: retained for history and linked to its replacement.
- `rejected`: retained when the decision history is useful.

Use [`TEMPLATE.md`](TEMPLATE.md) for new records.

## Existing records

- [Incident/RCA identity and evidence records](2026-11-01-incident-rca-records.md) — decision approved by owner; document beta; canonical Symptoms, metadata, annotations and templates. Does not approve #96 release.

| Record | Recorded status / scope |
| --- | --- |
| [Archive-envelope cryptography](2026-08-14-phase-4-archive-envelope-crypto-decision.md) | `beta`; approved bounded Task 3 implementation, not blanket restore/provider/release acceptance |
| [Google Drive cancellation](2026-09-17-google-drive-scope-cancellation.md) | `beta`; active Drive work removed; historical migration provenance retained |
| [Genesis schema-limit remediation](2026-09-21-genesis-local-schema-limit-remediation.md) | Front matter `need review`, body `candidate`; historical preparation selection, not current recovery evidence |
| [Google Meet API-first strategy](2026-09-21-google-meet-agent-api-strategy.md) | `candidate`; proposed strategy and approval boundaries |

Read the scope and approval inside each record. This index does not promote a
decision or replace newer implementation/recovery evidence in platform status.

- [CPU capture priority](2026-10-23-desktop-cpu-capture-priority.md) — CPU transcripts
  follow Stop; source-audio preservation and installed acceptance remain required.

- [Endpoint-first release; PID experimental](2026-11-07-endpoint-first-release-pid-experimental.md) — approved 2026-11-07;
  dedicated-endpoint mode is the per-app release gate, and PID capture stays available as experimental.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-10-16 | candidate | Indexed all four existing records with their recorded status and scope limits. | working-tree | Codex |
| 0.1.0b | 2026-08-23 | candidate | Added decision-record index and status rules. | pending | ATHER |
