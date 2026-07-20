---
version: "0.2.2b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-07-09T15:25:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "sitemap-ia"
  scope: "FUNG"
---

# 05 - Sitemap IA

## IA Principle

FUNG is a desktop command deck, not a route-heavy web app. The user should stay inside one fixed Subtract HUD while the P rail changes the active page domain. The center content, Agent card, log sector, and signal cards must all reflect the active P.

## Domain Taxonomy

| Domain | Purpose | Core Functions | Feature Set |
| --- | --- | --- | --- |
| Capture and Library | Bring audio into the local workspace safely | record, import, chunk, autosave, project select | long recording, WAV/MP3 import, capture queue, local chunk sealing |
| Transcript and Speakers | Turn audio into reviewable human text | transcribe, diarize, speaker labels, transcript review | speaker map, timestamp segments, layer-aware transcript, correction workflow |
| Summary and Intent | Convert reviewed material into useful understanding | summarize, infer intent, cite evidence, extract actions | full story summary, per-speaker intent, action list, glossary, evidence spans |
| Runtime and Governance | Control local-first AI, storage, privacy, and export | BYOM, provider health, policies, export, diagnostics | SQLite WAL, GenesisBlockDB surface, Ollama/vLLM providers, MCP/API/CLI, provenance |

## P Rail Pages

| Page | Domain | Center View | Agent View | Signals |
| --- | --- | --- | --- | --- |
| P1 | Capture and Library | capture queue, input map, capture memory | Capture console | input quality, capture safety, autosave, active source |
| P2 | Transcript and Speakers | project list, workspace map, output memory | Review workspace | transcript readiness, speaker lock, layer state, review focus |
| P3 | Summary and Intent | knowledge set, inference map, AI memory | Intelligence deck | summary status, intent confidence, citations, action queue |
| P4 | Runtime and Governance | provider stack, runtime map, governance memory | Runtime control | local runtime, privacy mode, export queue, policy focus |

P1-P4 are not decorative anchors. They are the top-level page state. When a P is active:

- The P chip is visually active.
- Topbar segmented state maps to the same page.
- The center battle zone changes labels and memory cards.
- Agent card changes to the domain-specific control surface.
- Sector C events describe the active domain.
- Signal cards keep fixed positions but retitle their focus to the active domain.

## Inner Content Simplification

The inner deck must not expose every subsystem at once. Competitive references cluster around a small number of repeated jobs: record/import, transcribe/edit, identify speakers, summarize/action items, clean speech, and export.

Each P center view should therefore show three sticky action tiles:

| Page | Tile 1 | Tile 2 | Tile 3 |
| --- | --- | --- | --- |
| P1 | Record long take | Add WAV / MP3 | Noise sample |
| P2 | Read and correct transcript | Lock speaker names | Mark useful audio |
| P3 | Story recap | Per-person intent | Next steps |
| P4 | BYOM status | Local policy | Render bundle |

Advanced details remain available through side actions, Agent card secondary actions, logs, or future detail panes. They should not crowd the first viewport.

## Feature Driver Strategy

The HUD content should be designed from recurring user jobs, not from subsystem inventory.

The first feature driver is:

- Meeting Mode -> long capture, transcript review, speaker confirmation, recap, action extraction, export

Feature drivers reshape copy, tile priority, signal wording, and Agent card guidance inside the existing shell. They do not create a new page framework. Detailed meeting-specific zone behavior lives in `07-meeting-mode.md`.

## Grouping Logic

| Keep Together | Reason |
| --- | --- |
| Capture + Library | Recording and import both create the same local project/chunk state. Splitting them makes long-recording safety harder to see. |
| Transcript + Speakers | Diarization is only useful when reviewed alongside timestamped text and layer context. |
| Summary + Intent | Intent is an AI inference over transcript evidence, so it must sit beside summary and citations. |
| Runtime + Governance | BYOM, privacy, export provenance, and local API state are one legal/operational control surface. |

## Command Surfaces

| Surface | Ownership |
| --- | --- |
| P rail | Top-level page selection only: P1, P2, P3, P4 |
| Topbar | Global search, page segment, light/dark mode, notifications, new project |
| Sidebar FAB | Fast task tools: record, import, playback, export, runtime, settings |
| Bottom-left power | Radial menu for close and minimize |
| Center deck | Active P domain content |
| Agent card | Active P control surface |
| Sector C | Activity and event evidence for the active P |
| Signal sector | Four persistent status cards with active-domain copy |

## Navigation Rules

- No route transition should move the deck layout. P changes swap content inside fixed sectors.
- P rail owns page identity; topbar segmented control mirrors it for fast access.
- Power actions must be behind a radial menu to avoid accidental close.
- Dark/light mode is a global UI preference in the topbar, not a P-specific setting.
- Deep settings stay within P4 unless a future architecture introduces separate modal pages.

## Core Workflows

### New Recording

1. P1 becomes active.
2. User records or imports audio.
3. App writes chunks into local storage and SQLite WAL.
4. Capture state appears in center deck and signal sector.

### Transcribe and Review

1. P2 becomes active.
2. User selects a project.
3. App creates transcription and diarization jobs.
4. User reviews text, speaker labels, and layer notes.

### Summarize and Analyze Intent

1. P3 becomes active.
2. User confirms transcript readiness.
3. BYOM runtime creates summary, intent, action, and evidence outputs.
4. UI labels intent as AI inference and keeps evidence spans visible.

### Runtime and Export

1. P4 becomes active.
2. User checks BYOM/runtime/privacy status.
3. User exports WAV, MP3, SRT, VTT, TXT, MD, or JSON.
4. Export provenance and local paths are retained.

## Content Priority

| Priority | Content |
| --- | --- |
| P0 | Recording safety, local data status, accidental-close protection |
| P1 | Transcript readiness, speaker state, active project |
| P2 | Summary, intent, citations, export queue |
| P3 | Runtime health, BYOM readiness, governance policy |

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.2b | Added feature-driver strategy and linked Meeting Mode as the first detailed content model. |
| 0.2.1b | Simplified active page content into three frequent-use action tiles per P. |
| 0.2.0b | Redesigned IA around domain taxonomy and P1-P4 page-state navigation. |
| 0.1.0b | Added sitemap and workflow IA. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.2b | 2026-07-09 | beta | Added feature-driver IA rule and connected Meeting Mode to the P1-P4 content model. | N/A | ATHER |
| 0.2.1b | 2026-07-06 | beta | Added inner content simplification rule for frequent-use tiles. | N/A | ATHER |
| 0.2.0b | 2026-07-06 | beta | Reworked sitemap into domain-first IA and P rail page rules. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added sitemap and IA. | N/A | ATHER |
