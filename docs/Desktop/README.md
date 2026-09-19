---
version: "0.2.0b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-09-20T03:53:22+07:00,RWANG"
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
| 00 | `../design/2026-09-19-liquid-glass-desktop-shell-refresh.md` | Current DesktopShell chrome, active surfaces, sidebar and responsive shell contract |
| 00a | `../FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md` | Approved Liquid Glass material, motion, profile and appearance-page addendum |
| 01 | `01-foundations.md` | Product principles, goals, non-goals, acceptance criteria |
| 02 | `02-tokens.md` | Skeuomorphic design tokens and material rules |
| 03 | `03_LAYOUT.md` | Legacy Subtract HUD layout and domain-coordinate reference; not current shell chrome |
| 04 | `04-components.md` | Legacy component inventory plus domain ownership rules; current shell components live under `src/components/desktop/` |
| 05 | `05-sitemap-ia.md` | Legacy P1-P4 domain IA; current shell navigation is defined by the Liquid Glass refresh |
| 06 | `06-stack.md` | Tauri/Rust/API/MCP/SQLite/BYOM technical stack |
| 07 | `07-meeting-mode.md` | Feature-driver content model for meeting recording and review |
| 08 | `08-real-progress.md` | Current implementation truth, validation evidence, next gaps |

`07` still owns the meeting-content model. It does not override the current shell navigation. Future mode-specific specs should follow the same pattern and link the active-surface contract.

## Supporting Documents

| Document | Status | Notes |
| --- | --- | --- |
| `../design/2026-09-19-liquid-glass-desktop-shell-refresh.md` | beta | Current DesktopShell source-of-truth: Home/Live/Review/Appearance, hover sidebar, profile header and native-only window boundary |
| `../FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md` | beta | Current Liquid Glass material and interaction addendum |
| `PRODUCT_SPEC.md` | beta | Original product requirements |
| `ARCHITECTURE.md` | beta | Architecture-level design |
| `AUDIO_AI_PIPELINE.md` | beta | Audio/AI processing plan |
| `DESIGN_SYSTEM.md` | beta | Legacy plus current design direction |
| `03_LAYOUT.md` | legacy | Historical Subtract HUD shell and inner-grid coordinates; preserve as design history |
| `LEGAL_PRIVACY.md` | beta | Legal/privacy boundaries |
| `IMPLEMENTATION_SURFACES.md` | beta | Contracts and implementation handoff |
| `07-meeting-mode.md` | beta | Meeting-driven content behavior; shell placement follows the current active-surface contract |
| `wireframes/README.md` | draft/legacy | Page-level P1-P4 structural wireframes for domain content, not current shell chrome |
| `GPU_STANDALONE_RUNTIME_SPEC.md` | beta | CUDA 12/cuDNN runtime ownership, launcher, diagnostics, and clean-room GPU proof |
| `UAT_SITEMAP_2026-07-19.md` | superseded | Historical sitemap and Meeting Mode UI UAT for the former Subtract HUD/P rail shell |

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

The current Desktop UI direction is Liquid Glass: a translucent, motion-aware shell with FUNG / QUIET ARCHIVE lockup, a hover/focus-expandable sidebar, truthful profile state, and active surfaces `home`, `live`, `review`, and `appearance`. Recording actions belong to the active Home/Live surfaces; theme/material/transparency controls belong to the Appearance page; shell-level minimize/close buttons are not exposed. The older Skeuomorphic Subtract HUD documents remain as domain/layout history and are not the current shell source of truth.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | 2026-09-20 | Reconciled the Desktop documentation index with the Liquid Glass DesktopShell and marked the older HUD/rail documents as legacy where appropriate. |
| 0.1.5b | Added page-level P1-P4 Desktop wireframes and their index. |
| 0.1.4b | Added sitemap and Meeting Mode UAT record. |
| 0.1.3b | Added standalone GPU runtime packaging specification. |
| 0.1.2b | Added `07-meeting-mode.md` as the first feature-driver content spec. |
| 0.1.1b | Corrected the canonical layout filename to `03_LAYOUT.md` and removed stale mirror wording. |
| 0.1.0b | Added canonical documentation index matching numbered docs structure. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.0b | 2026-09-20 | beta | Added the current Liquid Glass shell references and reconciled legacy layout/index boundaries. | 285566b9515c5fc83b4fde64ce8e57389f7a565a | RWANG |
| 0.1.5b | 2026-08-22 | draft | Added the complete P1-P4 Desktop page-level wireframe set. | N/A — uncommitted | ATHER |
| 0.1.4b | 2026-07-19 | beta | Added sitemap UAT report to the documentation index. | N/A | ATHER |
| 0.1.3b | 2026-07-19 | beta | Added GPU standalone runtime spec to the documentation index. | N/A | ATHER |
| 0.1.2b | 2026-07-09 | beta | Added the Meeting Mode spec to the canonical document index. | N/A | ATHER |
| 0.1.1b | 2026-07-09 | beta | Fixed canonical layout doc reference and source-of-truth note. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added docs index. | N/A | ATHER |
