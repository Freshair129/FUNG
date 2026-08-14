---
version: "0.2.8b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-08-14T11:44:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "real-progress"
  scope: "FUNG"
---

# 08 - Real Progress

## Current Status

FUNG has a working desktop-first foundation and a routed Live Meeting core. Sprint 4 adds an independently default-off connector and operator workflow for controlled read-only document and CRM lookup: local stdio registration, exact evidence/field preview, per-call approval, cancel/revoke, sanitized result provenance, and local history. A Windows relaunch smoke now proves the app window reopens and base Genesis project/recording/transcript rows remain readable; summary/export review after restart is still open. Streamable HTTP, vendor-specific production connectors, automated screenshot/keyboard UAT, real-device capture UAT, and real-connector UAT remain open.

This document separates implemented truth from planned capability.

## Public Desktop distribution overlay (2026-08-14)

`Freshair129/FUNG-Releases` is now the public binary-only Desktop channel. Its
v0.1.0 latest URL downloaded anonymously as 515,089,576 bytes with SHA-256
`f67a78e0b216628d19335646342eea20575d5c7b5a16cccf7daf32c6b780d414`,
matching the installer that passed local and real-hardware gates. Release
verification workflow run `31770231402` passed. The Landing cutover has a
source regression that rejects the private `Freshair129/FUNG/releases` path;
production deployment remains the final gate for this overlay.

## Desktop v0.1.0 release-candidate overlay (2026-08-14)

The isolated `codex/desktop-live-release` worktree now has a self-contained
Windows x64 CPU release candidate: portable CPython 3.11.9, pinned
`faster-whisper` 1.2.1 dependencies, pinned local `small` model, both worker
scripts, licenses, and a SHA-256 manifest. A 30.082-second real-hardware run on
the Fantech Leviosa microphone and Scarlett Solo system loopback produced 8
durable chunks and 10 transcript segments. The final NSIS candidate installed,
launched a visible `FUNG` window, and transcribed the speech fixture from its
installed runtime with 0.9988 confidence.

The final local asset is 515,089,576 bytes with SHA-256
`f67a78e0b216628d19335646342eea20575d5c7b5a16cccf7daf32c6b780d414`.
The production-style browser gate renders the version, approximate 491 MB
size, unsigned SmartScreen notice, and stable latest-release URL with zero
console errors. GitHub upload/hash equality and Vercel production promotion
remain publication gates and are not claimed by this pre-tag record.

## Phase 3 post-merge overlay (2026-08-12)

The Phase 3 follow-up is merged into `main` at `cea2d93` (PR #10), keeping the
routed Live Meeting and external-retrieval surfaces intact while adding the
scoped Python worker hardening. Automated evidence is Rust `195/195`, Python
concat `5/5`, mobile `4/4`, auth `5/5`, design-system `2/2`, and TypeScript/Vite
build passing. Post-merge GitHub CI run `31609642060` passed for frontend and
Rust. Real provider/device UAT and release/product gates remain open; this
overlay does not promote Phase 3 to fully release-ready.

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
| Live Meeting UI | `LiveMeetingPanel` renders source-aware transcript, current topic, local Knowledge Base question, and post-meeting summary surfaces. |
| Live capture/transcript route | Rust capture and long-lived whisper worker route microphone plus optional system audio into durable chunks and live transcript events. |
| Local meeting intelligence | Topic tracking and manual questions search stored transcript/knowledge graph while keeping source citations. |
| Post-meeting package | Overview, timeline/key points, decisions/actions, provenance rows, and Markdown export are implemented in `meeting_intel.rs`. |
| Live Meeting entry | Both the P1 card and fixed microphone rail open the real P1 `live-capture` panel; the rail path has a source regression test. |
| External retrieval contract | Rust DTOs define non-secret connectors, three read-only capabilities, suggestion/preview/run/sanitized-result states, and stable errors; the pure trust module stays separate from sibling transport/command modules. |
| External retrieval persistence | Genesis schema v9 extends connector metadata and registers grants, previews, runs, and sanitized results through the normal adapter path. |
| External retrieval trust foundation | Default-deny grant policy, canonical preview hash, exact field minimization, zeroized OS-keyring lifecycle, connector disconnect/revoke, typed Genesis audit payloads, and hostile-result sanitization are implemented and tested. |
| External retrieval backend | Allowlisted stdio MCP `2025-11-25` initialize/list/call, bounded process I/O, timeout/cancel/cleanup, durable one-time execution, all eight planned Tauri commands, and document/CRM fixture execution are implemented behind default-off `FUNG_EXTERNAL_MEETING_TOOLS=1`. |
| External retrieval operator UI | `ExternalMeetingToolsPanel` is embedded in Live Meeting behind default-off `VITE_FUNG_EXTERNAL_MEETING_TOOLS=1` with connector list/register/disconnect, exact field and transcript-evidence selection, preview/deny/approve, running/cancel, meeting-scope revoke, sanitized result, inert source references, policy/evidence/time provenance, and run history. |
| GPU runtime staging | `stage_gpu_runtime.ps1` stages FUNG-owned CUDA 12/cuDNN DLLs and writes a SHA-256 manifest. |
| GPU worker launch | The transcription subprocess resolves FUNG resources at runtime, selects an explicit CPU/GPU profile, and prepends FUNG's CUDA directory to its own `PATH`. |

## Partially Implemented

| Area | Current Truth | Gap |
| --- | --- | --- |
| Project CRUD | Backend commands exist for project creation/listing. | Needs full UI workflows and persistence QA. |
| Job model | Basic create/list commands exist. | Needs execution engine, retries, pause/resume, failure recovery. |
| Model providers | Seed local providers exist. | Needs provider diagnostics and real adapter execution. |
| Export UI | Signal card and queue affordance exists. | Needs real WAV/MP3/transcript export pipeline. |
| Summary/intent UI | Summary/action output pipeline and display surface exist. | Intent-specific UI and complete evidence-span review remain incomplete. |
| Live speaker attribution | Source channels map to editable `เรา`/`อีกฝ่าย` labels. | This is capture provenance, not arbitrary live multi-speaker diarization. |
| Live intelligence runtime | Topic and summary routes exist. | Requires current real-device/local-model UAT and explicit unavailable/degraded UI verification. |
| Live Meeting entry | The fixed microphone rail now opens the real panel and its regression passes. | Current packaged-app interaction/UAT remains to be rerun after the prior bootstrap incident. |
| External meeting retrieval | Backend plus operator workflow, stdio fixture transport, zero-process-before-approval, document/CRM reads, connector lifecycle, sanitized result rendering, recording-row isolation, and bounded relaunch persistence smoke are tested at unit/source/integration level. | Automated keyboard/1200×780 visual UAT, detailed connector health, artifact-wide secret scan, real-device capture-isolation UAT, summary/export review after restart, and real-connector UAT remain. |
| GPU standalone release | DLL staging and child-process isolation are implemented. | Must build and run a copied packaged bundle with a speech fixture; NVIDIA redistribution approval remains a release gate. |

## Not Implemented Yet

- Audio import parser.
- Speaker diarization runtime integration.
- Noise reduction.
- Source separation/layer generation.
- Real transcript editor.
- Full MCP server runtime.
- Full local API beyond initial health/project/job surfaces.
- Vendor-specific production connectors and all external write capabilities.

## Latest Verification Evidence

| Check | Result |
| --- | --- |
| `npm run build` | Passed |
| `cargo check` | Passed |
| `npm audit --audit-level=moderate` | Passed with 0 vulnerabilities |
| SVG XML validation | Passed |
| Browser layout check | Signals parent is `.panel-glass`; floating signal count is `0` |
| Compact viewport check | `1200 x 780` rendered without body scroll |
| Live Meeting code-route inspection | `LiveMeetingPanel.tsx`, `live_meeting.rs`, and `meeting_intel.rs` route capture, transcript, topic, local search, summary, actions, and Markdown export. This is implementation evidence, not a current real-device UAT. |
| FUNG-owned CUDA runtime manifest | Passed: 11 staged CUDA 12/cuDNN DLLs present and hash-recorded. |
| Isolated CUDA provider probe | Passed: with `G-Music`, Torch, and CUDA Toolkit paths excluded, FUNG Python/CTranslate2 reported `cuda_device_count=1`. |
| Clean-path GPU transcription | Passed: FUNG transcribed `C:\Windows\Media\Alarm01.wav` with `gpu` profile while its worker `PATH` began with `D:\FUNG\runtime\cuda12\bin` and excluded G-Music/Torch/CUDA Toolkit paths. |
| Release-layout GPU transcription | Passed: the Tauri release resource layout (`target\release\.venv-whisper`, `runtime`, and `scripts`) completed the same clean-path GPU transcription smoke. |
| Rust validation | `cargo check` passed after runtime-launcher changes. |
| Live Meeting rail regression | `npm run test:desktop-bootstrap` passed 5/5; the microphone control selects P1 `live-capture` and opens `LiveMeetingPanel`. |
| External MCP trust-foundation tests | Focused Rust tests passed 14/14: policy deny/allow matrix, canonical preview hash, minimizer, keyring lifecycle, Genesis disconnect/revoke, typed audit, secret-field rejection, hostile-result sanitizer, and resource limits. |
| External MCP Sprint 4 tests | Focused Rust cluster passed 23/23: trust contracts, stdio lifecycle/allowlist, timeout/cancel/cleanup, eight-command surface, connector/keyring/grant/disconnect lifecycle, document and CRM fixture calls, one-time execution, durable sanitized result/audit, failure terminalization, and active recording-row isolation. |
| External tool frontend tests | Passed 5/5: default-off flag, exact argument minimization, UI state transitions, Thai capture-safe errors, eight-command client surface, revoke control, and Live Meeting embedding. |
| Genesis v9 migration | Focused Rust integration test passed grant → preview → run → sanitized result round trip and reinstall idempotency. |
| Full Rust library regression | Passed **195/195** after `resolve_test_python()` added the Windows `py.exe` launcher fallback; the six prior FUNGWIRE failures are now green. Existing compiler warnings are non-failing and unrelated to this fix. |
| Auth/mobile/design regressions | Passed auth 5/5, mobile capture 4/4, and design-system publishing 2/2. |
| Diff hygiene | `git diff --check` passed. Repository-wide `cargo fmt --check` still reports broad pre-existing formatting debt outside this scoped change. |
| Rebuilt Desktop runtime | The debug `fung.exe` launched with title `FUNG`; a close/relaunch smoke observed PID 37720 then PID 9088 and non-zero window handles, with Genesis counts unchanged (`projects=1`, `recordings=1`, `transcript_segments=13`, `audit_events=1`). Windows Graphics Capture, browser screenshot, and keyboard automation remain unavailable, so visual/keyboard UAT is still open. |
| Real connector/device diagnostics | Claude Desktop MCP registry is empty, no approved vendor endpoint/credential is configured, and `adb`/`scrcpy` are absent. The real-connector and physical-device gates remain blocked, not waived. |
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
| M7 Controlled Meeting Retrieval | Read-only document/CRM lookup through MCP | Suggest → preview → per-call approve, minimization, keyring, audit, sanitized result, denial/revoke/failure gates pass. |

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
| 0.2.8b | Recorded the public binary-only Desktop channel, anonymous full-download equality, release workflow pass, and Landing private-repository regression. |
| 0.2.7b | Recorded the verified self-contained Desktop v0.1.0 release candidate, 30-second real-device transcript UAT, final installer hash, and remaining publication gates. |
| 0.2.5b | Recorded 195/195 full Rust regression after the Windows Python launcher fallback, expanded all-26 source-intent annotation coverage, bounded relaunch persistence evidence, and exact visual/device/real-connector blockers. |
| 0.2.6b | Recorded PR #10 merge at `cea2d93`, post-merge CI run `31609642060` passing, and the remaining real provider/device and release gates. |
| 0.2.4b | Recorded Sprint 4 connector/operator UI, eight-command and grant provenance boundary, focused frontend/backend evidence, successful dev launch, and remaining visual/restart/device/real-connector gates. |
| 0.2.3b | Recorded bounded Sprint 3 stdio execution, fixtures, isolation evidence, default-off flag, and unresolved full-suite environment failures. |
| 0.2.2b | Recorded the tested Sprint 2 policy, preview hash, minimization, keyring, disconnect/revoke, audit, and sanitizer foundation; MCP transport and UI remain explicitly unimplemented. |
| 0.2.1b | Recorded the real microphone-rail entry fix plus Sprint 1 typed external MCP and Genesis v9 foundations; runtime execution remains explicitly unimplemented. |
| 0.2.0b | Truth-synced the routed Live Meeting core and kept external MCP/CRM retrieval explicitly unimplemented. |
| 0.1.3b | Added successful release-layout GPU transcription smoke evidence. |
| 0.1.2b | Added successful clean-path GPU transcription smoke evidence. |
| 0.1.1b | Added the implemented standalone GPU runtime staging/launch path and its current verification boundary. |
| 0.1.0b | Added implementation progress truth table and next milestones. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.8b | 2026-08-14 | beta | Published and verified the public Desktop v0.1.0 channel; production Landing cutover remains the final gate. | pending | ATHER |
| 0.2.7b | 2026-08-14 | beta | Verified the Windows CPU release candidate, real-device Live Meeting transcript, installer runtime, and production-style web CTA before publication. | pending | ATHER |
| 0.2.6b | 2026-08-12 | beta | Phase 3 follow-up merged to `main`; post-merge frontend/Rust CI passed while provider/device and release gates remain open. | `cea2d93` | ATHER |
| 0.2.5b | 2026-08-12 | beta | Closed the prior full Rust regression, expanded annotation intent, and recorded bounded restart smoke plus environment-bounded UAT blockers. | pending | ATHER |
| 0.2.4b | 2026-08-11 | beta | Added verified Sprint 4 operator workflow and remaining UAT boundaries. | pending | ATHER |
| 0.2.3b | 2026-08-11 | beta | Added verified Sprint 3 bounded stdio backend evidence and remaining UI/UAT gaps. | pending | ATHER |
| 0.2.2b | 2026-08-11 | beta | Added verified Sprint 2 external-retrieval trust-foundation evidence and remaining runtime/UI gaps. | pending | ATHER |
| 0.2.1b | 2026-08-11 | beta | Added verified Live Meeting rail entry and Sprint 1 external MCP contract/schema evidence. | pending | ATHER |
| 0.2.0b | 2026-08-11 | beta | Recorded routed Live Meeting capability, verification boundaries, entry mismatch, and external retrieval gap. | pending | ATHER |
| 0.1.3b | 2026-07-19 | beta | Recorded release-layout GPU smoke evidence. | N/A | ATHER |
| 0.1.2b | 2026-07-19 | beta | Recorded the clean-path GPU transcription smoke result. | N/A | ATHER |
| 0.1.1b | 2026-07-19 | beta | Recorded standalone GPU runtime implementation and validation evidence. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added real progress doc. | N/A | ATHER |
