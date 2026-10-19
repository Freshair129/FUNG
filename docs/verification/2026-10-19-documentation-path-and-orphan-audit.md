---
version: "0.1.0b"
created_at: "2026-10-19"
last_update: "2026-10-19"
status: "beta"
superseded_by: null
attributes:
  domain: "documentation-governance"
  doc_type: "verification-report"
  scope: "FUNG Markdown path integrity and orphan-link inventory"
  language: "Thai-first"
---

# Documentation path and orphan-link audit — 2026-10-19

## Scope and method

This audit checks Markdown navigation and repository-relative references in the FUNG project working tree. The file inventory comes from Git-tracked and non-ignored files, so ignored dependency and virtual-environment content is excluded. A **link-orphan candidate** is a Markdown file with zero inbound inline local Markdown links, excluding `README.md`/`AGENTS.md` entry points and compatibility-pointer files. Fenced code examples are ignored; directory links resolve to `README.md` or `index.md`. This measures discoverability; it does not prove that a file is unused or safe to delete.

Compatibility pointers resolve to their canonical targets. Binary assets and runtime-generated paths are outside this count.

## Results

| Measure | Count | Interpretation |
| --- | ---: | --- |
| Markdown files in the project working tree | 440 | Git-tracked and non-ignored Markdown files, including source guidance and documentation outside `docs/`. |
| Markdown files under `docs/` | 344 | Documentation corpus including the new indexes and this audit. |
| Inline local Markdown links resolved to Markdown files | 799 | Fenced code is ignored; directory links resolve to `README.md` or `index.md`. |
| Link-orphan candidates, repository-wide | 0 | Every non-index, non-pointer Markdown file has an inbound local link. |
| Link-orphan candidates under `docs/` | 0 | All documentation files are linked from their owner indexes. |
| Link-orphan candidates outside `docs/` | 0 | Product scope and RCA records are linked from repository indexes. |
| Broken relative Markdown links under `docs/` | 0 | Checked against files and directories present in the working tree. |

The corrected pre-index inventory contained 236 link-orphan candidates: 161 under `docs/` and 75 elsewhere in the project. The earlier preliminary total of 257 counted 21 ignored Markdown files from `.venv-whisper`; those third-party environment files are excluded from this final project inventory. The index updates now provide inbound links for all 236 candidates without deleting the historical records.

## Path reconciliation

- Canonical links now point to the organized locations listed in [`docs/README.md`](../README.md); former Markdown paths remain short compatibility pointers.
- Owner indexes now link Desktop, Mobile, app, plans, specs, superpowers, implementation reports and the tracked RCA archive; the root README links the product scope and RCA index.
- Repository-relative path mentions in the 23 archived documentation-maintenance records were updated to the canonical locations. Machine-specific absolute paths in those historical records remain as provenance.
- The Desktop scorecard and dated preflight now point to the canonical preflight and plan locations.
- The design-system source was updated to the canonical operations runbook path, and generated board files were regenerated from the token source.
- The retired document relationship artifact and its references were not found in the repository working tree.

## Limits

The orphan counts measure Markdown-link reachability. They do not inspect binary assets, runtime imports, database content, dynamically assembled paths or references that use only a basename. The 257 candidates need owner/status review before any move or deletion; this audit does not remove them.
