---
version: "1.0.1b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-09-20T04:25:00+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "stack"
  scope: "FUNG"
---

# 06 - Stack

## Stack Summary

| Layer | Choice | Responsibility |
| --- | --- | --- |
| Desktop shell | Tauri v2 | Native desktop runtime, filesystem, window, secure commands |
| Backend | Rust | Genesis SDK adapter, local API, job state, CLI, MCP host path |
| Frontend | React + TypeScript + Vite | Liquid Glass `DesktopShell` and active-surface workspace UI |
| Database boundary | GenesisBlockDB embedded core | One open/write/query/backup/restore contract |
| Internal relational subsystem | SQLite | Genesis-owned relational projection, joins and migrations; no direct FUNG handle |
| Internal specialized projections | Native graph + native vector | Genesis-owned traversal, graph indexes, embeddings and ANN/HNSW |
| Local API | Loopback HTTP | Shared control plane for UI, CLI, and MCP |
| MCP | Local MCP surface | Agent/tool automation over approved local operations |
| CLI | `fung-cli` | Diagnostics, batch jobs, automation |
| AI runtime | BYOM adapters | Ollama/ollama.cpp, vLLM, local OpenAI-compatible endpoints |

## Rust Backend Decision

The current desktop presentation boundary is `src/components/desktop/DesktopShell.tsx`
with `home`, `live`, `review` and `appearance` active surfaces. This stack document
does not change the Rust ownership boundary; shell and responsive presentation details
are governed by [`docs/design/2026-09-19-liquid-glass-desktop-shell-refresh.md`](../design/2026-09-19-liquid-glass-desktop-shell-refresh.md).

Rust is a strong fit because FUNG application services integrate the Genesis embedded SDK, native files/audio, long-running jobs, packaging, CLI and resource control. The frontend must not own durable audio-processing state.

Rust owns:

- Genesis database-root lifecycle and SDK integration.
- FUNG relational schema package and typed Genesis transactions/queries.
- No direct SQLite, graph or vector handle; those stores and signed WAL are Genesis-owned.
- Project/job/model provider commands.
- Local API server.
- CLI diagnostics and automation.
- Future audio processing orchestration.

## Local Data Model

Minimum durable entities:

- Project.
- Recording.
- AudioChunk.
- AudioLayer.
- Speaker.
- TranscriptSegment.
- ModelProvider.
- ModelRun.
- Summary.
- IntentInference.
- ExportArtifact.
- AuditEvent.

## Stateful Jobs

All long operations become resumable jobs:

- Recording.
- Transcription.
- Diarization.
- Noise reduction.
- Source separation.
- Summary generation.
- Intent analysis.
- Export.

Job states follow:

```text
queued -> running -> paused -> completed
queued -> running -> failed -> retrying -> running
running -> cancelled
```

## BYOM Provider Classes

| Provider Class | Capabilities |
| --- | --- |
| Ollama / ollama.cpp | Local LLM summary, intent, reasoning |
| vLLM | Local or LAN OpenAI-compatible inference |
| Transcription runtime | Speech-to-text |
| Diarization runtime | Speaker segmentation |
| Audio processing runtime | Noise reduction and separation |

Cloud providers are not default. Any cloud provider must be explicit opt-in and visibly labelled.

## Developer Commands

| Command | Purpose |
| --- | --- |
| `npm run dev` | Run Vite frontend |
| `npm run build` | Type-check and build frontend |
| `npm run tauri` | Run Tauri commands |
| `cargo check` | Check Rust backend |

## Contract References

| Contract | Purpose |
| --- | --- |
| `contracts/local-api-v1.yaml` | Local HTTP API |
| `contracts/local-mcp-v1.yaml` | MCP operations |
| `contracts/fung-cli-v1.yaml` | CLI behavior |
| `contracts/stateful-job-model-v1.yaml` | Job model |
| `docs/Mobile/FULL_FEATURE_WIRING_SPEC.md` | FUNG-to-Genesis operational-boundary and migration contract |
| `contracts/genesisblockdb-entities-v1.yaml` | Deprecated historical entity inventory; not a Genesis SDK contract |
| `schemas/sqlite-wal-v1.sql` | Transitional FUNG-owned SQLite schema; migration input only after cutover |

## Version Diff

| Version | Change |
| --- | --- |
| 1.0.1b | Reconciled the frontend stack description with the current Liquid Glass DesktopShell; backend ownership remains unchanged. |
| 0.1.0b | Added stack reference for desktop-first implementation. |
| 1.0.0b | Replaced parallel SQLite/domain-layer framing with GenesisBlockDB as the single operational boundary. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 1.0.1b | 2026-09-20 | candidate | Reconciled the frontend shell description with the current DesktopShell and active surfaces; Rust/backend boundaries unchanged. | pending; docs reconciliation | RWANG |
| 0.1.0b | 2026-07-05 | beta | Added stack doc. | N/A | ATHER |
| 1.0.0b | 2026-07-20 | candidate | Corrected stack ownership to the GenesisBlockDB unified operational boundary | N/A — no commit created | ATHER |
