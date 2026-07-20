---
version: "0.1.0b"
created_at: "2026-07-05T10:35:00+07:00,ATHER"
last_update: "2026-07-05T10:35:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "implementation-surface"
  scope: "FUNG"
---

# Implementation Surfaces

## Purpose

This document turns the approved product and architecture docs into implementation-facing contracts for the Rust/Tauri runtime.

## Source Alignment

Parent docs reviewed:

- `docs/ARCHITECTURE.md`
- `docs/PRODUCT_SPEC.md`
- `docs/AUDIO_AI_PIPELINE.md`
- `docs/LEGAL_PRIVACY.md`

Peer docs reviewed:

- `docs/DESIGN_SYSTEM.md`
- `docs/03_LAYOUT.md`

## Contract Files

| File | Owns |
| --- | --- |
| `contracts/local-api-v1.yaml` | Loopback HTTP contract for UI, CLI, and MCP |
| `contracts/local-mcp-v1.yaml` | MCP tool surface mapped onto local API operations |
| `contracts/fung-cli-v1.yaml` | CLI verbs, output shape, and exit codes |
| `contracts/genesisblockdb-entities-v1.yaml` | Domain entities and provenance invariants |
| `contracts/stateful-job-model-v1.yaml` | Job states, transitions, and execution rules |
| `schemas/sqlite-wal-v1.sql` | Minimum SQLite WAL schema implied by current docs |

## Implementation Notes

- Keep the local API loopback-only by default.
- Keep the desktop runtime as the source of truth for project state.
- Record provenance for every AI-generated artifact.
- Treat summaries and intent as derived artifacts, never as source facts.
- Keep speaker labels editable and non-identity-asserting.

## Rust Ownership Notes

- Rust should own the HTTP server, MCP host, CLI wiring, SQLite migrations, and job engine state transitions.
- UI code should consume the contracts, not redefine them.
- If Rust names differ from these contracts, add adapters rather than changing the meaning of fields.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Added implementation-facing contract map for API, MCP, CLI, SQLite WAL, GenesisBlockDB entities, and jobs. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-05 | beta | Initial implementation surface handoff. | N/A | ATHER |
