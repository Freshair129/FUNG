---
version: "0.1.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-08-23T01:15:22+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "requirements-governance"
  doc_type: "core-directive"
  scope: "FUNG requirement ID registry"
---

# Requirement ID Registry

This is the initial registry snapshot after the documentation architecture
approval. It was derived from identifiers found under `docs/` on 2026-08-23.
No new product requirement was allocated by this scaffold.

## Current allocation snapshot

| Prefix | Existing IDs found | Next candidate | Notes |
|---|---:|---:|---|
| `FR` | `101–116` | `FR-117` | Existing live-meeting source remains canonical. |
| `NFR` | `101–110` | `NFR-111` | Existing live-meeting source remains canonical. |
| `BR` | `101–108` | `BR-109` | Existing live-meeting source remains canonical. |
| `AC` | `101–112` | `AC-113` | Existing live-meeting source remains canonical. |
| `DR` | none found in `docs/` scan | `DR-101` | Allocate only when an approved data requirement exists. |
| `IR` | none found in `docs/` scan | `IR-101` | Allocate only when an approved infrastructure requirement exists. |
| `SEC` | none found in `docs/` scan | `SEC-101` | Allocate only with security review. |
| `SDD` | none found in `docs/` scan | `SDD-101` | Decisions live in `docs/decisions/`. |
| `AI-AGT` | none found in `docs/` scan | `AI-AGT-101` | Use for agent/runtime behavior. |
| `AI-ETH` | none found in `docs/` scan | `AI-ETH-101` | Use for AI ethics/governance controls. |

“None found” is a scan result, not proof that an identifier does not exist
outside `docs/`. Before allocating a new ID, search the repository and review
the current graph.

## Allocation rules

1. Reserve the next number only when a requirement is accepted for review.
2. Move the ID to `approved` only after the statement and acceptance evidence
   are reviewable.
3. Never reuse an ID, even if the requirement is rejected or retired.
4. Keep a stable ID when wording is clarified; create a new ID when behavior
   or scope changes materially.
5. Link implementation and verification evidence from the canonical document
   and graph; do not infer coverage from a filename.

## Audit command

From the repository root, inspect current identifiers with:

```powershell
rg -o -i --glob '*.md' --glob '*.yaml' --glob '*.yml' --glob '*.json' `
  '(FR|NFR|SDD|SEC|AI-AGT|AI-ETH|BR|AC|DR|IR)-[0-9]{3}' . |
  Sort-Object -Unique
```

Update this file and the document graph together when an ID is allocated.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Initialized counters from the repository docs scan. | pending | ATHER |
