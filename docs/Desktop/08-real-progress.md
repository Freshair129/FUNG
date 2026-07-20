---
version: "0.1.3b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-07-19T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "real-progress"
  scope: "FUNG"
---

# 08 - Real Progress

## Current Status

FUNG has a working desktop-first foundation, a Tauri/Rust backend skeleton, SQLite WAL initialization, local API health path, CLI smoke path, contract files, and a rendered Skeuomorphic Subtract HUD frontend.

This document separates implemented truth from planned capability.

## Implemented

| Area | Current Truth |
| --- | --- |
| Tauri v2 scaffold | Present with Rust backend and React/Vite frontend. |
| Frontend build | `npm run build` passed. |
| Rust check | `cargo check` passed. |
| SQLite WAL | Backend initializes SQLite with WAL, foreign keys, and busy timeout. |
| Local API | Basic loopback `/health` API path exists. |
| CLI | `fung-cli` smoke path exists and confirms WAL setting. |
| Contracts | API, MCP, CLI, GenesisBlockDB, job model, and SQLite schema files exist. |
| Layout RCA fix | Signals are inside the panel; `.fab-signals` removed. |
| Design direction | Skeuomorphic material implemented in UI and docs. |
| GPU runtime staging | `stage_gpu_runtime.ps1` stages FUNG-owned CUDA 12/cuDNN DLLs and writes a SHA-256 manifest. |
| GPU worker launch | The transcription subprocess resolves FUNG resources at runtime, selects an explicit CPU/GPU profile, and prepends FUNG's CUDA directory to its own `PATH`. |

## Partially Implemented

| Area | Current Truth | Gap |
| --- | --- | --- |
| Project CRUD | Backend commands exist for project creation/listing. | Needs full UI workflows and persistence QA. |
| Job model | Basic create/list commands exist. | Needs execution engine, retries, pause/resume, failure recovery. |
| Model providers | Seed local providers exist. | Needs provider diagnostics and real adapter execution. |
| Export UI | Signal card and queue affordance exists. | Needs real WAV/MP3/transcript export pipeline. |
| Summary/intent UI | Surface exists. | Needs evidence-cited model output pipeline. |
| GPU standalone release | DLL staging and child-process isolation are implemented. | Must build and run a copied packaged bundle with a speech fixture; NVIDIA redistribution approval remains a release gate. |

## Not Implemented Yet

- Long microphone recording.
- Chunked audio persistence and crash recovery scan.
- Audio import parser.
- Speaker diarization runtime integration.
- Noise reduction.
- Source separation/layer generation.
- Real transcript editor.
- Real summary and intent analysis output with evidence ranges.
- Full MCP server runtime.
- Full local API beyond initial health/project/job surfaces.

## Latest Verification Evidence

| Check | Result |
| --- | --- |
| `npm run build` | Passed |
| `cargo check` | Passed |
| `npm audit --audit-level=moderate` | Passed with 0 vulnerabilities |
| SVG XML validation | Passed |
| Browser layout check | Signals parent is `.panel-glass`; floating signal count is `0` |
| Compact viewport check | `1200 x 780` rendered without body scroll |
| FUNG-owned CUDA runtime manifest | Passed: 11 staged CUDA 12/cuDNN DLLs present and hash-recorded. |
| Isolated CUDA provider probe | Passed: with `G-Music`, Torch, and CUDA Toolkit paths excluded, FUNG Python/CTranslate2 reported `cuda_device_count=1`. |
| Clean-path GPU transcription | Passed: FUNG transcribed `C:\Windows\Media\Alarm01.wav` with `gpu` profile while its worker `PATH` began with `D:\FUNG\runtime\cuda12\bin` and excluded G-Music/Torch/CUDA Toolkit paths. |
| Release-layout GPU transcription | Passed: the Tauri release resource layout (`target\release\.venv-whisper`, `runtime`, and `scripts`) completed the same clean-path GPU transcription smoke. |
| Rust validation | `cargo check` passed after runtime-launcher changes. |
| Python worker syntax | `py_compile scripts/transcribe.py` passed. |

Screenshot artifacts from the latest UI validation:

- `output/playwright/fung-skeuo-1280x720.png`
- `output/playwright/fung-skeuo-1200x780.png`

## Next Milestones

| Milestone | Goal | Exit Criteria |
| --- | --- | --- |
| M1 Recording Core | Real long recording with chunks | Can record, stop, recover, and list chunk metadata locally. |
| M2 Audio Import/Export | File import and WAV export | Can import audio and export source/derived WAV. |
| M3 Transcription MVP | BYOM transcription job | Transcript segments saved with timestamps and provenance. |
| M4 Speaker Review | Diarization and label editing | Speakers are editable and linked to segments. |
| M5 Summary/Intent | Evidence-based AI outputs | Summary and intent cite transcript time ranges. |
| M6 MCP/CLI Completeness | Automation over same state | MCP and CLI can drive project/job/export workflows. |

## Known Risks

| Risk | Level | Mitigation |
| --- | --- | --- |
| Long recording data loss | High | Chunked writes, WAL state, recovery scan. |
| Model runtime instability | High | Adapter isolation, diagnostics, resumable jobs. |
| Legal/privacy misuse | High | Local-first defaults, opt-in cloud, inference labels. |
| Speaker misidentification | Medium | Editable labels, no definitive identity claims. |
| CPU/GPU overload | Medium | Queue control, pause/resume, runtime profile. |
| CUDA redistribution | High | Treat NVIDIA redistribution terms as a release gate; stage only from an approved, version-pinned source. |

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.3b | Added successful release-layout GPU transcription smoke evidence. |
| 0.1.2b | Added successful clean-path GPU transcription smoke evidence. |
| 0.1.1b | Added the implemented standalone GPU runtime staging/launch path and its current verification boundary. |
| 0.1.0b | Added implementation progress truth table and next milestones. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.3b | 2026-07-19 | beta | Recorded release-layout GPU smoke evidence. | N/A | ATHER |
| 0.1.2b | 2026-07-19 | beta | Recorded the clean-path GPU transcription smoke result. | N/A | ATHER |
| 0.1.1b | 2026-07-19 | beta | Recorded standalone GPU runtime implementation and validation evidence. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added real progress doc. | N/A | ATHER |
