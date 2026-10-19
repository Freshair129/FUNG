---
version: "0.3.0b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-10-16,Codex"
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
| 00 | [Current Studio flows](../design/2026-10-16-desktop-navigation-and-flows.md) | Current dashboard, bottom dock, five surfaces, wireframes, capture guard and user flows |
| 00a | `../FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md` | Approved Liquid Glass material, motion, profile and appearance-page addendum |
| 00b | `../FUNG_Design_System_v0.1.0/DESIGN_SYSTEM.md` | Shared token authority, theme contract and cross-platform component/state guidance |
| 01 | `01-foundations.md` | Product principles, goals, non-goals, acceptance criteria |
| 02 | `02-tokens.md` | Historical Skeuomorphic Subtract tokens; current values are in the shared DTCG source |
| 03 | `03_LAYOUT.md` | Legacy Subtract HUD layout and domain-coordinate reference; not current shell chrome |
| 04 | `04-components.md` | Legacy component inventory plus domain ownership rules; current shell components live under `src/components/desktop/` |
| 05 | `05-sitemap-ia.md` | Legacy P1-P4 domain IA; current shell navigation follows the Studio flow contract |
| 06 | `06-stack.md` | Tauri/Rust/API/MCP/SQLite/BYOM technical stack |
| 07 | `07-meeting-mode.md` | Feature-driver content model for meeting recording and review |
| 08 | `08-real-progress.md` | Current implementation truth, validation evidence, next gaps |

`07` still owns the meeting-content model. It does not override the current shell navigation. Future mode-specific specs should follow the same pattern and link the active-surface contract.

## Supporting Documents

| Document | Status | Notes |
| --- | --- | --- |
| [Studio flows](../design/2026-10-16-desktop-navigation-and-flows.md) | beta | Current shell/layout/navigation owner; 18 Desktop flows and platform handoffs |
| [September shell refresh](../design/2026-09-19-liquid-glass-desktop-shell-refresh.md) | historical chrome | Sidebar/header placement superseded; preserve feature and material history |
| `../FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md` | beta | Current Liquid Glass material and interaction addendum |
| `../FUNG_Design_System_v0.1.0/tokens/fung.tokens.json` | beta | Canonical shared tokens; generated CSS is consumed by the Desktop shell |
| `../design-system/index.html` | generated | Interactive cross-surface board and full token catalog |
| `PRODUCT_SPEC.md` | beta | Original product requirements |
| [ARCHITECTURE.md](ARCHITECTURE.md) | candidate | Parent architecture and three-layer document/knowledge ownership |
| `AUDIO_AI_PIPELINE.md` | beta | Audio/AI processing plan |
| `DESIGN_SYSTEM.md` | beta | Legacy plus current design direction |
| `03_LAYOUT.md` | legacy | Historical Subtract HUD shell and inner-grid coordinates; preserve as design history |
| `LEGAL_PRIVACY.md` | beta | Legal/privacy boundaries |
| `IMPLEMENTATION_SURFACES.md` | beta | Contracts and implementation handoff |
| `07-meeting-mode.md` | beta | Meeting-driven content behavior; shell placement follows the current active-surface contract |
| `wireframes/README.md` | draft/legacy | Page-level P1-P4 structural wireframes for domain content, not current shell chrome |
| `GPU_STANDALONE_RUNTIME_SPEC.md` | beta | CUDA 12/cuDNN runtime ownership, launcher, diagnostics, and clean-room GPU proof |
| `UAT_SITEMAP_2026-07-19.md` | superseded | Historical sitemap and Meeting Mode UI UAT for the former Subtract HUD/P rail shell |
| [CALLMD_ARCHITECTURE_COMPARISON.md](CALLMD_ARCHITECTURE_COMPARISON.md) | reference | Product/architecture comparison; not runtime acceptance |
| [CALLMD_FUNG_SCORECARD_TH.md](CALLMD_FUNG_SCORECARD_TH.md) | reference | Comparison scorecard; read its evidence limits |
| [LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md](LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md) | requirement source | Canonical existing live-meeting requirement IDs |
| [ZOOM_INTEGRATION_SETUP.md](ZOOM_INTEGRATION_SETUP.md) | setup reference | Provider setup and its independent credential/runtime gates |
| [Current flow audit](../verification/implementation-reports/2026-10-16-wireframe-wireflow-userflow-audit.md) | scoped evidence | Browser screenshots, source checks, fixes, CPU checks and named native/device blockers |

## Contract Files

| File | Owns |
| --- | --- |
| [local-api-v1.yaml](../../contracts/local-api-v1.yaml) | Loopback local API contract |
| [local-mcp-v1.yaml](../../contracts/local-mcp-v1.yaml) | MCP tool contract |
| [fung-cli-v1.yaml](../../contracts/fung-cli-v1.yaml) | CLI contract |
| [genesisblockdb-entities-v1.yaml](../../contracts/genesisblockdb-entities-v1.yaml) | GenesisBlockDB domain entities |
| [stateful-job-model-v1.yaml](../../contracts/stateful-job-model-v1.yaml) | Stateful job model |
| [sqlite-wal-v1.sql](../../schemas/sqlite-wal-v1.sql) | SQLite WAL schema baseline |

## Current Design Decision

The current Desktop UI is the Studio dashboard with a bottom navigation dock,
project/utility disclosures and `home`, `live`, `review`, `output`, `appearance`
surfaces. Home enters capture setup; Live owns recording controls; Review owns
selected-recording playback, Q&A and jobs; Output owns new-file destinations;
Appearance owns local display choices. The Liquid Glass addendum owns materials.
Older HUD and hover-sidebar placement remains design history. Native window
controls remain platform-owned; browser preview does not qualify native behavior.

## Version Diff

| Version | Change |
| --- | --- |
| 0.3.0b | Reconciled current Studio flows, legacy pointers, supporting docs and root contract/schema links. |
| 0.2.0b | Reconciled the Desktop documentation index with the September Liquid Glass shell and marked the older HUD/rail documents as legacy. |
| 0.1.5b | Added page-level P1-P4 Desktop wireframes and their index. |
| 0.1.4b | Added sitemap and Meeting Mode UAT record. |
| 0.1.3b | Added standalone GPU runtime packaging specification. |
| 0.1.2b | Added `07-meeting-mode.md` as the first feature-driver content spec. |
| 0.1.1b | Corrected the canonical layout filename to `03_LAYOUT.md` and removed stale mirror wording. |
| 0.1.0b | Added canonical documentation index matching numbered docs structure. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.3.0b | 2026-10-16 | beta | Indexed current Studio wireframes/flows and audit; corrected supporting document coverage and relative contract/schema paths. | working-tree | Codex |
| 0.2.0b | 2026-09-20 | beta | Added the current Liquid Glass shell references and reconciled legacy layout/index boundaries. | 285566b9515c5fc83b4fde64ce8e57389f7a565a | RWANG |
| 0.1.5b | 2026-08-22 | draft | Added the complete P1-P4 Desktop page-level wireframe set. | N/A — uncommitted | ATHER |
| 0.1.4b | 2026-07-19 | beta | Added sitemap UAT report to the documentation index. | N/A | ATHER |
| 0.1.3b | 2026-07-19 | beta | Added GPU standalone runtime spec to the documentation index. | N/A | ATHER |
| 0.1.2b | 2026-07-09 | beta | Added the Meeting Mode spec to the canonical document index. | N/A | ATHER |
| 0.1.1b | 2026-07-09 | beta | Fixed canonical layout doc reference and source-of-truth note. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added docs index. | N/A | ATHER |

## Additional Desktop documents

เอกสารเสริมและประวัติที่ไม่ใช่จุดเริ่มต้นหลักอยู่ในรายการนี้ ให้ตรวจสถานะของแต่ละไฟล์ก่อนใช้อ้างอิง

- [01-foundations](01-foundations.md) — `beta`
- [02-tokens](02-tokens.md) — `legacy`
- [06-stack](06-stack.md) — `candidate`
- [GPU_STANDALONE_RUNTIME_SPEC](GPU_STANDALONE_RUNTIME_SPEC.md) — `beta`
- [IMPLEMENTATION_SURFACES](IMPLEMENTATION_SURFACES.md) — `beta`
- [PRODUCT_SPEC](PRODUCT_SPEC.md) — `candidate`
- [UAT_SITEMAP_2026-07-19](UAT_SITEMAP_2026-07-19.md) — `superseded`
