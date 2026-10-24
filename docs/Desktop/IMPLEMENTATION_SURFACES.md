---
version: "0.1.2b"
created_at: "2026-07-05T10:35:00+07:00,ATHER"
last_update: "2026-10-20,Codex"
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
| `contracts/local-api-v1.yaml` | Loopback HTTP contract for same-machine clients; route registration and runtime coverage are tracked separately |
| `contracts/local-mcp-v1.yaml` | Legacy declared MCP tool contract; this file does not prove Desktop MCP registration or route/schema compatibility |
| `contracts/product-mcp-v2.yaml` | Accepted Desktop MCP v2 contract; MCP-02 source candidate is registered through the Rust/Tauri startup path, with independent review and package/client UAT pending |
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

- Rust owns the current local HTTP server, native Tauri commands, and job engine state transitions. MCP-02 adds a default-off loopback Streamable HTTP product MCP candidate; independent implementation review and package/client UAT are still pending.
- GenesisBlockDB remains the authoritative application persistence path. MCP/API adapters must call the same application services and must not open or mutate the SQLite store directly.
- UI code should consume the contracts, not redefine them.
- If Rust names differ from these contracts, add adapters rather than changing the meaning of fields.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.2b | Recorded the accepted Product MCP v2 contract and MCP-02 source candidate, while keeping independent review and packaged/client acceptance open. |
| 0.1.1b | Clarified that legacy MCP and HTTP files are declared contracts, not proof of a registered Desktop MCP server or exact route compatibility; recorded GenesisBlockDB as the persistence boundary. |
| 0.1.0b | Added implementation-facing contract map for API, MCP, CLI, SQLite WAL, GenesisBlockDB entities, and jobs. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.2b | 2026-10-20 | beta | Mapped the MCP-02 source candidate to the Rust/Tauri runtime and recorded remaining acceptance gates. | working-tree | Codex |
| 0.1.1b | 2026-10-20 | beta | Clarified Desktop MCP runtime and persistence boundaries against the current source inventory. | working-tree | Codex |
| 0.1.0b | 2026-07-05 | beta | Initial implementation surface handoff. | N/A | ATHER |
