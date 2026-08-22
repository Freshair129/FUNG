---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
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

The current Phase 4 archive-envelope record remains in this directory. Its
status and scope must be read from the file itself; this index does not promote
it or authorize a provider, filesystem write, restore, or release claim.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added decision-record index and status rules. | pending | ATHER |
