---
version: "0.1.4b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-07-19T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "documentation-index"
  scope: "FUNG"
---

# FUNG Documentation

FUNG is a desktop-first, local-first, BYOM audio intelligence app for long recording, transcription, speaker diarization, audio cleanup, layered audio review, export, and evidence-based summaries.

This folder is the documentation source of truth for the current beta build.

## Canonical Reading Order

| Order | Document | Owns |
| --- | --- | --- |
| 01 | `01-foundations.md` | Product principles, goals, non-goals, acceptance criteria |
| 02 | `02-tokens.md` | Skeuomorphic design tokens and material rules |
| 03 | `03_LAYOUT.md` | RCA-corrected Subtract HUD layout and fixed coordinates |
| 04 | `04-components.md` | Component inventory, states, and ownership rules |
| 05 | `05-sitemap-ia.md` | Screens, information architecture, and workflow map |
| 06 | `06-stack.md` | Tauri/Rust/API/MCP/SQLite/BYOM technical stack |
| 07 | `07-meeting-mode.md` | Feature-driver content model for meeting recording and review |
| 08 | `08-real-progress.md` | Current implementation truth, validation evidence, next gaps |

`07` now owns the first feature-driver content spec. Future mode-specific specs should follow the same pattern.

## Supporting Documents

| Document | Status | Notes |
| --- | --- | --- |
| `PRODUCT_SPEC.md` | beta | Original product requirements |
| `ARCHITECTURE.md` | beta | Architecture-level design |
| `AUDIO_AI_PIPELINE.md` | beta | Audio/AI processing plan |
| `DESIGN_SYSTEM.md` | beta | Legacy plus current design direction |
| `03_LAYOUT.md` | beta | Current layout source of truth for the Subtract HUD shell and inner grid |
| `LEGAL_PRIVACY.md` | beta | Legal/privacy boundaries |
| `IMPLEMENTATION_SURFACES.md` | beta | Contracts and implementation handoff |
| `07-meeting-mode.md` | beta | Meeting-driven content behavior inside the fixed HUD shell |
| `GPU_STANDALONE_RUNTIME_SPEC.md` | beta | CUDA 12/cuDNN runtime ownership, launcher, diagnostics, and clean-room GPU proof |
| `UAT_SITEMAP_2026-07-19.md` | beta | Executed sitemap and Meeting Mode UI UAT, including runtime-scope findings |

## Contract Files

| File | Owns |
| --- | --- |
| `contracts/local-api-v1.yaml` | Loopback local API contract |
| `contracts/local-mcp-v1.yaml` | MCP tool contract |
| `contracts/fung-cli-v1.yaml` | CLI contract |
| `contracts/genesisblockdb-entities-v1.yaml` | GenesisBlockDB domain entities |
| `contracts/stateful-job-model-v1.yaml` | Stateful job model |
| `schemas/sqlite-wal-v1.sql` | SQLite WAL schema baseline |

## Current Design Decision

The UI direction is Skeuomorphic Subtract HUD: a tactile porcelain command deck with fixed zones. Signals D/E/F/G are panel-owned controls, not floating FABs. This follows the 2026-07-05 RCA that identified detached absolute signal cards as the root cause of the floating-sector layout bug.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.4b | Added sitemap and Meeting Mode UAT record. |
| 0.1.3b | Added standalone GPU runtime packaging specification. |
| 0.1.2b | Added `07-meeting-mode.md` as the first feature-driver content spec. |
| 0.1.1b | Corrected the canonical layout filename to `03_LAYOUT.md` and removed stale mirror wording. |
| 0.1.0b | Added canonical documentation index matching numbered docs structure. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.4b | 2026-07-19 | beta | Added sitemap UAT report to the documentation index. | N/A | ATHER |
| 0.1.3b | 2026-07-19 | beta | Added GPU standalone runtime spec to the documentation index. | N/A | ATHER |
| 0.1.2b | 2026-07-09 | beta | Added the Meeting Mode spec to the canonical document index. | N/A | ATHER |
| 0.1.1b | 2026-07-09 | beta | Fixed canonical layout doc reference and source-of-truth note. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added docs index. | N/A | ATHER |
