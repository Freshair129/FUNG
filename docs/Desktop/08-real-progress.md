---
version: "0.2.13b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-08-23T03:47:57+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "real-progress"
  scope: "FUNG"
---

# 08 - Real Progress

## Current Status

FUNG has a working desktop-first foundation and a routed Live Meeting core. Sprint 4 adds an independently default-off connector and operator workflow for controlled read-only document and CRM lookup: local stdio registration, exact evidence/field preview, per-call approval, cancel/revoke, sanitized result provenance, and local history. A Windows relaunch smoke proves the app window can reopen and base Genesis project/recording/transcript rows remain readable; summary/export review after restart is still open. The host `py -3` interpreter cannot import `faster_whisper`, but FUNG's staged `.venv-whisper` runtime imports `faster-whisper` 1.2.1, has the pinned `small` model, and uses the staged CUDA 12/cuDNN 9 bundle. The standalone GPU worker smoke passed; Live Meeting real-capture, device, connector, and visual UAT remain open. Streamable HTTP, vendor-specific production connectors, automated screenshot/keyboard UAT, real-device capture UAT, and real-connector UAT remain open.

This document separates implemented truth from planned capability.

## Current truth sync (2026-08-23)

PR #16 is merged into `origin/main` at merge commit `26da78466364e479085d0aa5d7f06e24a08bd12c`. Its routing, backup-payload, and connector-timeout changes are therefore part of the current mainline. The physical Android, clean-install restore, real connector, and release gates remain open as recorded below.

The local staged runtime check is split into separate facts: `D:\FUNG\.venv-whisper\Scripts\python.exe` imports `faster-whisper` 1.2.1 and exposes the worker CLI; `D:\FUNG\.venv-whisper\models\small` contains the pinned `Systran/faster-whisper-small` revision `536b0662742c02347bc0e980a01041f333bce120`; `D:\FUNG\runtime\manifest.json` records 11 staged CUDA 12/cuDNN 9 DLLs. `scripts/smoke_gpu_standalone.ps1` passed with `C:\Windows\Media\Alarm01.wav` and `--profile gpu`; this proves the packaged worker path, not Live Meeting real-capture or device UAT.

## Routing and backup-payload overlay (historical snapshot, 2026-08-19; superseded)

This section preserves the pre-merge state. PR #16 was subsequently merged as recorded in the current truth sync above.

`fix/routing-predicate-and-backup-audio` is pushed and open as PR #16 with
both CI jobs green (`frontend` 27s, `rust` 6m8s). It is **not merged**, so
`main` does not yet carry any of the following.

Two reachability defects and one timeout defect are closed at source level:

- **Mobile surface on Android.** `resolveRootRoute` returned `"desktop"` for
  every Tauri runtime with no platform branch, so the APK loaded the fixed
  1280x780 desktop shell and the whole `src/mobile/` tree was unreachable on
  the device it was written for. Routing now takes an `isMobilePlatform` input
  derived from the webview user agent, and neither Tauri shell is gated on
  Supabase any more. **This is code and unit-test evidence only — no physical
  Android device has rendered the mobile surface since the change.**
- **Backup UI reachability.** All seven backup commands were called only from
  `src/web/AccountSettings.tsx`, a surface the Tauri runtime can never route
  to and where `invoke` does not exist. The controls moved to a shared
  `src/components/BackupPanel.tsx` mounted by both shells. This supersedes the
  0.2.10b claim that "Desktop AccountSettings has a labelled development/test
  UI" — no desktop AccountSettings existed when that was written.
- **Backup contents.** `run_backup_job` encrypted the Genesis bundle and
  nothing else, while every audio chunk lives as a loose file referenced by an
  absolute path; there is no Genesis blob API call anywhere in the tree. A
  "verified" archive was therefore a database-only backup. `backup_payload.rs`
  now carries the bundle plus every audio file the ledger references under the
  same authenticated envelope, records files that are unreadable or no longer
  match their capture-time digest as explicitly omitted rather than dropping
  them, and reports both counts through `BackupRunReport` / `RestoreResult`.
  Pre-container archives still restore.
- **Connector tool timeout.** `execute_stdio_tool` started its deadline before
  `spawn_stdio`, so `limits.timeout` covered process launch and handshake.
  Measured on this host, spawn was 1.34s against 48ms for the whole
  initialize/list/call exchange. Startup now has its own budget; the caller's
  timeout bounds the requested work; total stays capped by
  `MAX_MCP_EXECUTION`. This also removed a long-standing suite flake, which
  reproduced on `main` in 2 of 3 full runs.

Two bounds are now named in errors rather than hit silently: the archive
payload is capped at 2 GiB because the envelope encrypts from one in-memory
buffer, and audio chunks are enumerated per recording because GenesisBlockDB
rejects any query limit outside `1..1000` and offers no offset — a saturated
read fails closed instead of truncating.

Automated evidence on this host: Rust **238/238** across six consecutive full
runs, all eight Node suites **46/46**, `npm run build` passing. Still open and
not claimed here: physical Android render, real clean-install restore into the
approved Phase 4 roots, U9 closure, and every release gate.

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
| Phase 4 filesystem test backup | Genesis full export → XChaCha20-Poly1305/Argon2id encryption → atomic bounded-root write is wired end-to-end with clean-target restore, post-restore digest identity, and deep fixture verification. The backup controls live in a shared `BackupPanel` mounted by both shells (root picker, one-time 24-word recovery-phrase display, restore confirmation); before PR #16 they were reachable from no surface that could invoke them. Archives carry source audio with per-file digests and explicit omission accounting. Google Drive production transport remains TODO, and no clean-install restore has been run. |
| Mobile device reconciliation | The Android `devices` row is always resolved by (current user, fingerprint); the cached `fung.device.id` is only a mirror, replaced when stale and cleared on sign-out/revocation. Supabase RLS ownership policies were rechecked and required no migration. |
| GPU runtime staging | `stage_gpu_runtime.ps1` stages FUNG-owned CUDA 12/cuDNN DLLs and writes a SHA-256 manifest. |
| GPU worker launch | The transcription subprocess resolves FUNG resources at runtime, selects an explicit CPU/GPU profile, and prepends FUNG's CUDA directory to its own `PATH`. |

## Partially Implemented

| Area | Current Truth | Gap |
| --- | --- | --- |
| Project CRUD | Backend commands exist for project creation/listing. | Needs full UI workflows and persistence QA. |
| Job model | Basic create/list commands exist. | Needs execution engine, retries, pause/resume, failure recovery. |
| Model providers | Seed local providers exist. | Needs provider diagnostics and real adapter execution. |
| Transcript read completeness | The storage engine caps one relational query at 1000 rows with no cursor, and six differently-named copies of that constant had grown up around it. There is now one — `genesis_adapter::ROW_CAP` — and a `query_capped` helper that returns the rows together with whether the ceiling was reached. `list_transcript_segments` reads per recording instead of per project (a project of five 400-segment recordings used to return 1000 of its 2000 segments) and reports which recordings are still short; `meeting_intel::load_segments` refuses rather than summarising a transcript missing its tail. | The refusal is a stopgap, not the fix: a recording past 1000 segments still cannot be summarised or exported at all, and a 3-hour session is roughly 1500-2500 segments. The real fix is a cursor or offset in GenesisBlockDB, which is a change in a different repository and a dependency bump. Every reader of a length-driven table now either reads whole or says it could not: `fungwire_client::gather_segments` refuses instead of sending a renumbered, gap-free-looking partial manifest to a paired device, and `AudioIntegrityReport` carries `unread_recordings` so `is_clean()` cannot answer "nothing is wrong" when it means "nothing wrong was found". |
| Export UI | Subtitle export is real: `export.render` is a job the engine runs, `transcript_export` writes `.srt` and `.vtt` beside the recording, both are recorded in `export_artifacts`, and `list_export_artifacts` lets the shell tell the user where they landed. Formatting is unit-tested against the ways transcript text corrupts each format (blank lines, `<`, `-->`, zero-length cues). | Capped at one recording per run and at the storage engine's 1000-row single-read ceiling — past that the export refuses rather than writing a file that stops early. Audio export (WAV/MP3) and the separate export queue are still unimplemented. The write path is tested against a real GenesisBlockDB store (files on disk, both artifact rows, retry idempotence, refusal past the ceiling); what is untested is the packaged app's own click-to-file round trip. |
| Summary/intent UI | Summary/action output pipeline and display surface exist. | Intent-specific UI and complete evidence-span review remain incomplete. |
| Live speaker attribution | Source channels map to editable `เรา`/`อีกฝ่าย` labels. | This is capture provenance, not arbitrary live multi-speaker diarization. |
| Live intelligence runtime | Topic and summary routes exist; capture can degrade without the worker. | Current machine has no bundled Whisper interpreter/model and `faster_whisper` is unavailable, so live transcription requires runtime installation plus UAT. |
| Live Meeting entry | The fixed microphone rail now opens the real panel and its regression passes. | Current packaged-app interaction/UAT remains to be rerun after the prior bootstrap incident. |
| Audio import | `import_and_transcribe` is implemented, registered, and reachable from the desktop UI; it transcribes a picked file and persists segments. Recorded as "not implemented" through 0.2.10b, which was wrong. | The source file is referenced in place — no copy into project storage, no chunking, no checksum — so moving or deleting it invalidates a recording the ledger still reports `completed`. No `model_runs` row is written, so imported transcripts carry no model provenance. |
| External meeting retrieval | Backend plus operator workflow, stdio fixture transport, zero-process-before-approval, document/CRM reads, connector lifecycle, sanitized result rendering, recording-row isolation, and bounded relaunch persistence smoke are tested at unit/source/integration level. | Automated keyboard/1200×780 visual UAT, detailed connector health, artifact-wide secret scan, real-device capture-isolation UAT, summary/export review after restart, and real-connector UAT remain. |
| URL ingest | `fetch_and_transcribe`, `media_fetch_status` and `media_fetch_consent_set` are implemented, registered, and reachable from the desktop UI. A pasted http(s) link is fetched audio-only by a pinned yt-dlp worker, taken into custody, and transcribed through the same pipeline as a file import; the path is declared in the egress register §1.6 and guarded by `tests/egressRegister.test.mjs`. | Proven against a local HTTP server (`Generic` extractor) only. No real site — including YouTube — has been fetched on a device, and without the opt-in `deno` runtime YouTube is expected to fail outright. The staged runtime has never been installed on a machine here, so the readiness probe is unit-tested rather than run. yt-dlp is pinned to 2026.7.4 and the app has no update path, so extractor rot is a standing maintenance cost. |
| GPU standalone release | DLL staging and child-process isolation are implemented. | Must build and run a copied packaged bundle with a speech fixture; NVIDIA redistribution approval remains a release gate. |

## Not Implemented Yet

- Speaker diarization *reach*. `diarize.py` now ships in `bundle.resources`,
  `scripts/stage_diarization_runtime.ps1` installs the `torch`/`pyannote.audio`
  tree into the staged runtime, and `diarization_status` reports which
  prerequisite is missing before a worker is spawned — so an installed build
  can run diarization once the operator opts in. The dependencies stay out of
  the default bundle deliberately: the tree is hundreds of megabytes and the
  `pyannote/speaker-diarization-3.1` weights are gated per user on Hugging
  Face, so they cannot be redistributed in an installer.
  What remains is reach: the only route into diarization is still Zoom
  mixed-audio import. A locally captured meeting cannot be diarized, because
  its audio is per-channel chunks rather than one file, and deciding what to
  feed the model (system channel alone, or a mixdown) changes what the speaker
  timings mean. Neither the dependency install nor a diarization run has been
  executed on a device.
- URL ingest *at scale*. The path works, but two things are unproven and one
  is structural. Unproven: a real extractor run on a device, and the `deno`
  JS-runtime install that YouTube's signature challenges need. Structural:
  yt-dlp tracks sites that change deliberately, so a pinned version decays —
  a build six months old will fail on links a current yt-dlp handles, and
  nothing in the app tells the user that is why. Re-pinning is a lockfile
  regeneration (`stage_media_fetch_runtime.ps1 -GenerateLock`) and a release.
- A cursor for reads past 1000 rows. `Storage::query_relational` rejects any
  limit outside `1..1000`, `RelationalFilter` is equality-only, and
  `RelationalQuery` has no offset, so no read path in FUNG can page. Every
  reader now either refuses or reports when it hits the ceiling — silence was
  the defect, and that is fixed — but "reports" is not "reads". Until the
  engine grows a cursor, a recording longer than roughly 90 minutes cannot be
  summarised or exported at all. That work is in the GenesisBlock repository,
  not this one, and lands here as a dependency bump.
- Noise reduction.
- Source separation/layer generation.
- Real transcript editor.
- Full MCP server runtime.
- Full local API beyond initial health/project/job surfaces.
- Vendor-specific production connectors and all external write capabilities.

## Latest Verification Evidence

| Check | Result |
| --- | --- |
| `npm run build` | Passed on 2026-08-14 after `npm ci` restored missing local CLI binaries |
| `cargo check` | Passed |
| `npm audit --audit-level=moderate` | Passed on 2026-08-23; current lockfile reports **0 vulnerabilities** across 84 audited dependencies. |
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
| Full Rust library regression | Passed **217/217** on 2026-08-19 with the exact plan command (adds the Phase 4 backup-job/restore cluster); existing compiler warnings are non-failing and unrelated to this status. |
| Phase 4 backup/restore tests | Focused Rust cluster passed: export→encrypt→write→clean-restore round trip with note/graph/audio-chunk identity, wrong-secret/tamper/missing-archive/existing-target preservation, plaintext-staging boundary rejection, job-serialization guard, and fail-closed status. Node `test:backup-flow` passed 10/10 (opaque picker contract, restore confirmation gate, truthful error text) and `test:device-reconcile` passed 6/6 (ownership lookup, duplicate avoidance, stale-cache replacement, sign-out clearing). |
| Phase 4 clean-install/device gates | The real clean-install restore on the approved `D:\FUNG-Phase4-TestStorage`/`D:\FUNG-Phase4-TestRestore` roots and the physical Android/Dashboard identity check have not been run; U9 and release gates remain open. |
| Auth/mobile/design regressions | Passed auth 5/5, mobile capture 4/4, and design-system publishing 2/2. |
| Diff hygiene | `git diff --check` passed. Repository-wide `cargo fmt --check` still reports broad pre-existing formatting debt outside this scoped change. |
| Rebuilt Desktop runtime | The debug `fung.exe` launched with title `FUNG`; a close/relaunch smoke observed PID 37720 then PID 9088 and non-zero window handles, with Genesis counts unchanged (`projects=1`, `recordings=1`, `transcript_segments=13`, `audit_events=1`). Windows Graphics Capture, browser screenshot, and keyboard automation remain unavailable, so visual/keyboard UAT is still open. |
| Real connector/device diagnostics | Claude Desktop MCP registry is empty, no approved vendor endpoint/credential is configured, and `adb`/`scrcpy` are absent. The real-connector and physical-device gates remain blocked, not waived. |
| Python worker syntax | `py_compile scripts/transcribe.py` passed. |
| Current Whisper runtime availability | `py -3` reports no `faster_whisper`, while FUNG's staged `.venv-whisper` runtime imports `faster-whisper` 1.2.1, has the pinned `small` model, and passes the standalone GPU smoke with the staged CUDA 12/cuDNN 9 bundle. Live Meeting real-capture, device, visual, and connector UAT remain open. |

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
| 0.2.11b | Recorded PR #16: mobile/desktop routing fix, reachable backup UI, audio-bearing backup payload, and the connector startup/timeout split. Corrected two 0.2.10b inaccuracies (desktop AccountSettings, audio import). Physical Android and clean-install restore stay open. |
| 0.2.10b | Recorded Phase 4 filesystem test backup/restore implementation and mobile device-reconciliation hardening with 217/217 Rust plus green focused Node evidence; clean-install restore and physical Android identity gates stay open. |
| 0.2.9b | Truth-synced current frontend/Rust verification, the missing local Whisper runtime, and two high npm audit findings; desktop capture/transcription and release/UAT boundaries remain distinct. (Renumbered from a parallel 0.2.7b during the Phase 4 merge; `main` had assigned 0.2.7b/0.2.8b to the Desktop release records below.) |
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
| 0.2.13b | 2026-08-23 | beta | Staged the pinned faster-whisper small model and 11-file CUDA 12/cuDNN 9 bundle; standalone GPU smoke passed, while Live Meeting/device/connector UAT remains open. | working-tree | ATHER |
| 0.2.12b | 2026-08-23 | beta | Truth-synced the merged PR #16 state and separated staged package, model, and CUDA-runtime evidence; physical, restore, connector, and release gates remain open. | working-tree | ATHER |
| 0.2.11b | 2026-08-19 | beta | PR #16 pushed and green but unmerged; routing, backup-payload and connector-timeout defects closed at source with 238/238 Rust and 46/46 Node evidence; device and restore gates unchanged. | `68f0201` | ATHER |
| 0.2.10b | 2026-08-19 | beta | Phase 4 Tasks 5–9 landed on `codex/phase-4-filesystem-test-backup`; automated backup/restore and reconciliation evidence recorded, release/U9 gates unchanged. | working-tree | ATHER |
| 0.2.9b | 2026-08-14 | beta | Current desktop/web build and 212-test Rust evidence recorded; missing Whisper runtime and two high npm audit findings prevent a live-transcription/release claim. | working-tree | ATHER |
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
