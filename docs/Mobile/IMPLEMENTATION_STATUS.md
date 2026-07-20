---
version: "0.4.0b"
created_at: "2026-07-20T12:10:00+07:00,ATHER"
last_update: "2026-07-21T04:18:10+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "implementation-evidence"
  scope: "FUNG Mobile iOS and Android"
  language: "Thai"
---

# FUNG Mobile — Implementation Status

## Classification

- Complexity: C-3 — Architecture-Driven Implementation
- Change risk: HIGH — mobile lifecycle, durable capture, local database, LAN trust และ MCP
- Architecture baseline: `docs/Mobile/TECHNICAL_DESIGN.md` version `1.0.0b` (`beta`) after approved GenesisBlockDB boundary correction
- Assessment rule: นับว่า `verified` เฉพาะสิ่งที่มี test, build artifact หรือ interaction evidence ใน workspace นี้

## Outcome

FUNG Mobile มี runnable mobile surface, native segmented recording, pairing/MCP/Timeline/Stories/Processing/Agent Voice contracts และได้ cut over production runtime จาก FUNG-owned SQLite ไปใช้ GenesisBlockDB handle เดียวแล้ว ทุก production mutation ที่ wire แล้วผ่าน `GenesisTransaction`; legacy `fung.db` เปิดแบบ read-only เพื่อ import ครั้งเดียวและไม่ dual-write หลัง cutover

สถานะนี้เป็น **implementation beta ไม่ใช่ release completion**: build และ automated suites ผ่าน แต่ APK Genesis-enabled ชุดล่าสุดยังไม่ได้ติดตั้ง/ทำ physical-device UAT ซ้ำ เพราะรอบตรวจนี้ไม่พบ Android device ผ่าน ADB และ upstream acceptance ส่วน physical artifact, independent projection rebuild, backup/restore, lock/integrity และ self-host ยังไม่ครบ

## Phase Matrix

| Phase | Implemented in this workspace | Evidence | Exit status |
| --- | --- | --- | --- |
| 0 — Feasibility / stack gate | Tauri Android project, local JDK/SDK/NDK toolchain, arm64 Rust cross-build, signed debug APK | `app-arm64-debug.apk`; Android manifest; APK signature verification; Samsung install/UAT | Partial — Android install passes; iOS/macOS and extended lifecycle evidence pending |
| 1 — Shared core boundary | React mobile surface delegates durable operations to one shared GenesisBlockDB handle; production code no longer opens a FUNG-owned SQLite authority | `src-tauri/src/genesis_adapter.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/bin/fung-cli.rs` | Verified by compile/tests and source audit; physical-device reopen pending |
| 2 — Recording / recovery | Android microphone foreground service, native-first 5-second AAC segments, atomic journal, SHA-256 reconciliation, safe short-tail finalization and pause-adjusted UI clock; browser MediaRecorder fallback | completed 7-segment device journal; `FungRecorderService`; capture orchestration tests | Partial — foreground record/pause/resume/stop passes; screen-off and kill/restart suite pending |
| 3 — Shell / voice UX | voice-first porcelain UI, local Thai intent parser, destructive confirmation flag, permission-denied truth state | browser interaction evidence and Rust parser test | Partial — command grammar works; embedded on-device STT model pending |
| 4 — Notes / graph | notes, immutable revisions and epistemic relations commit through one Genesis transaction with canonical IDs and GraphQuery hydration | adapter transaction tests; Notes/Graph interaction evidence | Verified at code/test level; vector/provider and physical-device proof remain pending |
| 5 — Desktop pairing / delegation | 6-digit pairing proof hash, paired-device/capability/job schema, pairing UX | paired-device interaction evidence | Partial — simulated endpoint registration works; mutual-auth encrypted Desktop transport and resumable remote execution pending |
| 6 — MCP | opt-in local HTTP JSON-RPC gateway, Bearer authentication, bounded read-only tools, explicit LAN exposure flag | `contracts/mobile-mcp-v1.yaml`; `mobile_mcp_set_enabled` | Verified at code/test-build level; device-to-client interoperability suite pending |
| 7 — AI / release hardening | local intent grammar, permission error UX, Android arm64 debug artifact | browser console 0 errors; APK v2 signature verified | Partial — on-device STT/LLM asset, 3-hour capture, battery/thermal, release signing and store checks pending |
| 8 — Dark / Speaker Timeline | persistent System/Light/Dark preference; DAW-style speaker lanes, waveform clips, overlap, playhead, seek, zoom/pinch, rename/split/merge/confirm; revision ledger; Genesis-backed diarization queue/import and checksum-verified source segment playback | production build; Rust/mobile tests; Light/Dark 393×852 evidence | Partial — playback implementation/build verified; physical playback, actual Desktop diarization executor and gesture/performance suite pending |
| 9 — Stories / Processing / Agent Voice | non-destructive multivoice Story sequence, split/trim/move, undo/redo, effect-chain metadata, Whisper package truth, refinement review, processing-job queue, rights-scoped `fung.voice.speak` | production build; 7 Rust tests; 393×852 Light/Dark browser evidence; signed arm64 debug APK | Partial — standalone metadata editing is verified; actual DSP/LLM/TTS providers, mutual-auth Desktop execution and real-device audio render remain gated |

## Verified Evidence

### Build and automated checks

- `npm run build` — passed
- `npm run test:mobile` — 4 passed, 0 failed, including native-first ordering and pause-adjusted clock
- `cargo test --all-targets` — 10 passed, 0 failed in FUNG
- Targeted GenesisBlockDB suites — 19 passed, 0 failed (`napi_rest_parity`, `relational_u2`, `sqlite_substrate_s0`, `unified_transaction_u3`)
- Android arm64 Rust library — compiled
- Android arm64 debug APK — built and verified with Android `apksigner`
- Packaged manifest — `microphone` foreground-service type plus `RECORD_AUDIO` and `FOREGROUND_SERVICE_MICROPHONE` verified from the APK
- APK signature — Android Debug certificate, APK Signature Scheme v2
- APK SHA-256 — `21D79821CA50419AD4E9AD2DD3F3841618F0427237F4E19E66D4D63DA3704D05`
- Browser console during core flow — 0 errors, 0 warnings

### Interaction checks at 393 × 852

- Home renders the approved voice-first visual hierarchy
- Create Note → edit title/body → save → note appears in the library
- New note appears as a graph node; confirmed and inferred relation styles remain distinguishable
- Microphone denial never claims audio was recorded
- Pairing modal accepts a six-digit code and displays the resulting Desktop capabilities
- MCP toggle reports local-only state explicitly
- Theme cycles through System/Light/Dark and persists in local storage
- Speaker Timeline renders three distinguishable lanes, overlapping turns, time ruler and playhead at 393 × 852
- Selecting a turn opens rename, split, merge and confirmation controls without exposing biometric identity claims
- Stories Editor split changes revision 1 → 2; undo restores revision 1 and the original clip range
- Processing Studio shows Base, Small and Large as installed while Medium and Turbo remain explicitly unavailable
- Processing actions remain disabled without Desktop; no simulated progress or provider output is shown
- Agent Voice remains default-deny and does not expose a speaking claim without a rights-valid profile, provider and grant

### Physical Android checks at 384 × 853 logical px

- APK installed with `adb install -r` while preserving `UAT_Note` and existing recordings
- Native recorder starts without a duplicate WebView permission activity; safe offset and segment count increase
- Pause keeps elapsed at `00:00:24` across an approximately 32-second pause; Resume continues to `00:00:29` instead of adding paused duration
- Stop completes journal `15f573e4-ef97-4e12-a941-ad017cf2e4c1` with safe offset `29,091 ms` and 7 canonical segments; recorder foreground service is absent afterward
- Home, Notes and Processing scroll to final content above the five-item dock
- System/Light/Dark appearance control has an approximately 47.5 dp height, does not overlap the Notes FAB and is restored to System
- Contextual Timeline, Stories and Processing routes open from a completed recording
- Physical evidence above applies to the previous native-capture APK. The rebuilt Genesis-enabled APK, source playback and populated speaker lanes require a fresh device run; they are not marked PASS from code/build evidence alone

### Artifacts

- Android APK: `src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk`
- UX screenshots: `output/playwright/fung-mobile-*.png`
- Speaker Timeline screenshots: `output/playwright/fung-mobile-speaker-timeline-light.png`, `output/playwright/fung-mobile-speaker-timeline-dark.png`
- Stories screenshots: `output/playwright/fung-mobile-stories-light.png`, `output/playwright/fung-mobile-stories-dark.png`
- Processing screenshots: `output/playwright/fung-mobile-processing-light.png`, `output/playwright/fung-mobile-processing-dark.png`
- Mobile command contract: `contracts/mobile-api-v1.yaml`
- MCP contract: `contracts/mobile-mcp-v1.yaml`
- Historical FUNG-owned SQL schema: `schemas/mobile-genesisblockdb-v1.sql` — migration input/test fixture only, not a production authority

## Current GenesisBlockDB Operational-Boundary Conformance

| Upstream requirement | Current result |
| --- | --- |
| U1 — one Genesis handle/endpoint | PASS IN PRODUCTION CODE — app and CLI share Genesis `Storage`; direct SQL remains test-only and read-only legacy import |
| U2 — one canonical Genesis mutation path | PASS FOR WIRED FEATURES — row and graph mutations use signed/idempotent `GenesisTransaction` |
| U3 — relational schema package and public joins | PASS IN TARGETED SUITES — versioned schema, typed mutation/query/join and API parity are covered |
| U4 — canonical cross-domain `EntityId` | PASS FOR NOTES/RELATIONS — relational and graph mutations share the public entity ID |
| U5 — cross-domain recovery/frontier | PASS IN TARGETED SUITES — reopen, idempotency, compaction and stable-frontier cases pass |
| U6 — rebuildable projections | PARTIAL — relational replay/rebuild evidence exists; independent graph/vector rebuild coverage is not yet complete |
| U7 — physical mobile Genesis artifact | PARTIAL — arm64 APK builds and verifies; current Genesis-enabled APK has not passed fresh physical-device UAT |
| U8 — self-host API surface | NOT PROVEN in the FUNG delivery boundary |
| U9 — coherent Genesis backup unit | NOT IMPLEMENTED |
| U10 — lock/integrity model | NOT PROVEN by the current acceptance packet |
| U11 — public API parity / no HQL correctness dependency | PASS IN TARGETED SUITES — Rust/N-API/REST/C/JNI/Kotlin surfaces compile/test without HQL dependency |
| U12 — full acceptance matrix | NOT MET — U6/U7/U8/U9/U10 and external platform/provider gates remain |

This table supersedes the `0.3.2b` direct-SQL baseline. It records implementation/test evidence only and intentionally does not upgrade unrun physical or backup acceptance to PASS.

## Visual Fidelity Ledger

| Anchor from approved concept | Implemented result | Assessment |
| --- | --- | --- |
| Brand and standalone state form the first horizontal row | `FUNG` and the shield status remain above the primary task | Match |
| “วันนี้” introduces the immediate context | Same heading and quiet editorial spacing | Match |
| Voice orb is the dominant touch target | Central porcelain orb remains the largest control; touch copy is “กดค้างเพื่อพูด” | Match, scaled for 393 × 852 |
| Recording and note creation are paired quick actions | Both actions remain directly below the voice orb | Match |
| Recent work is secondary to capture | One recent recording is visible above the fixed navigation | Match; implementation shows the truthful audio time range |
| Voice occupies the center of persistent navigation | Raised indigo microphone action remains central across every screen | Match |
| Quiet porcelain surface and restrained green/indigo accents | Light and dark modes preserve tactile depth without ornamental skeuomorphism | Match |

Above-fold copy difference: the approved concept uses “บันทึกไว้บนมือถือ”; the implementation uses the concrete source range “เสียง 09:42–10:18” so the preview states what the item actually contains.

### M9/M10 Fidelity Ledger

| Approved anchor | Implemented result | Assessment |
| --- | --- | --- |
| Shared ruler/playhead across three speaker lanes | Three anonymous lanes, time ruler, clip waveforms and selected trim handles at 393×852 | Match; compacted for touch width |
| Selected-clip inspector and immutable-source notice | Transcript, waveform, split/trim/move/export controls and explicit source-preservation copy | Match |
| Processing tabs and execution-location truth | Four tabs plus FUNG Desktop/local-network status; standalone state stays truthful | Match |
| Whisper package list | Verified Desktop snapshot marks Base/Small/Large installed and Medium/Turbo absent | Match with corrected package truth |
| Original/proposed refinement | Separate original and proposal rows with accept/reject state | Match |
| Effect chain | Pitch, Reverb, Compressor and Low-pass with bypass/remove/add metadata controls | Match; render remains provider-gated |
| Agent Voice | MCP grant, voice profile and session area remain visible but default-deny | Intentional safety delta: no fabricated provider, grant or speaking state |

Browser note: the Codex in-app browser could not reach the workspace localhost due to isolation, so final UI evidence used the approved Playwright CLI fallback against the production build. Console result was 0 errors and 0 warnings.

## Required External Gates

1. macOS + Xcode + signed iPhone are required to generate and verify the iOS shell.
2. Android and iPhone reference devices are required for screen-off, background, interruption, forced-termination, 60-minute and 3-hour recording tests.
3. The Desktop runtime needs a concrete authenticated pairing endpoint before mutual-auth encryption and delegated job resume can be proven end-to-end.
4. Product must select and license the on-device Thai STT model before offline speech-to-text size, latency, battery and privacy gates can pass.
5. Release keystores, Apple signing identities, store metadata and distribution credentials are intentionally outside this debug artifact.
6. Product/legal approval is required for voice-rights evidence retention, grant expiry and revocation policy before Agent Voice can be enabled.
7. DSP, transcript-refinement LLM and owned-voice synthesis providers must be selected and proven before preview/render/speech are described as operational.
8. Timeline source-audio transport is wired with path-sandbox and SHA-256 checks but must pass on-device playback UAT; populated speaker lanes still require imported diarization or explicit manual turn data.
9. Genesis coherent backup/restore, lock/integrity, self-host and independent projection rebuild suites remain required for upstream U12 completion.

## Acceptance / Exit Criteria

The current implementation is suitable for code review, automated acceptance and installation of the signed debug APK. It is not release-ready and must not be described as physically verified for source playback, Genesis reopen/migration, populated diarized results, screen-off/kill-safe background recording, production-secure Desktop delegation, coherent backup or iOS completion until those gates pass.

## Version Diff

### `0.0.0` → `0.1.0b`

- Added an evidence-based implementation ledger for Phases 0–7.
- Recorded verified outputs separately from device/toolchain-dependent gates.
- Added exact artifact and contract locations.

### `0.1.0b` → `0.1.1b`

- Replaced Android WebView-owned long-running capture with a native microphone foreground service.
- Added atomic native segment journal and checksum-verified reconciliation into the Rust-owned checkpoint.
- Preserved MediaRecorder as browser/desktop preview fallback.
- Hardened the Windows Android build fallback against packaging a stale Rust library or an incrementally bloated APK.

### `0.1.1b` → `0.2.0b`

- Added persistent System/Light/Dark appearance selection.
- Added a standalone DAW-style speaker timeline with anonymous labels and explicit confidence status.
- Added speaker-turn and waveform-tile schema, viewport indexes and append-only edit revision records.
- Added native query, Desktop diarization queue/import, rename, split, merge and confirm commands.
- Added Light/Dark browser interaction evidence and a transcript-independent Rust timeline test.
- Kept actual Desktop execution, source-audio transport and real-device performance evidence visible as external/integration gates.

### `0.2.0b` → `0.3.0b`

- Added non-destructive Stories Editor metadata, revision ledger and split/trim/move/undo/redo commands.
- Added model-package, refinement-proposal, bounded effect-chain and Desktop processing-job contracts.
- Added `fung.voice.speak` as a default-deny MCP tool requiring valid rights, provider, grant and paired executor.
- Added M9/M10 Light/Dark 393×852 browser evidence and interaction proof.
- Rebuilt and v2-verified the Android arm64 debug APK; preserved external DSP/LLM/TTS and mutual-auth gates.

### `0.3.0b` → `0.3.1b`

- Made Android capture native-first and added staged capture errors.
- Fixed compact-screen scrolling, five-item navigation, contextual Timeline entry and appearance-control ownership.
- Hardened short-tail Stop finalization and corrected Pause/Resume elapsed time.
- Added Samsung Android 16 physical UAT, completed-journal evidence and the new APK hash while keeping playback/diarization gaps explicit.

### `0.3.1b` → `0.3.2b`

- Corrected the false implication that the FUNG-owned SQLite graph schema is a complete GenesisBlockDB integration.
- Added U1–U9 conformance truth showing direct SQL, signed-WAL, native graph/vector, frontier, rebuild and backup gaps.
- Linked the corrected full-feature wiring specification that depends on Genesis U1/U2/U3 before FUNG cutover.

### `0.3.2b` → `0.4.0b`

- Added GenesisBlockDB U2 relational schema/query/mutation parity and U3 unified transaction/frontier support upstream.
- Cut FUNG production persistence, CLI, mobile features, MCP and legacy migration over to one Genesis handle with no production dual write.
- Added checksum-verified source playback segment transport and wired the mobile play control.
- Rebuilt and verified the Android arm64 debug APK and updated automated evidence while keeping fresh physical-device UAT and U6/U8/U9/U10/U12 gaps explicit.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-20 | need review | Initial implementation evidence and phase exit ledger | N/A — workspace is not an initialized Git repository | ATHER |
| 0.1.1b | 2026-07-20 | need review | Android native foreground recorder and Rust reconciliation evidence | N/A — workspace is not an initialized Git repository | ATHER |
| 0.2.0b | 2026-07-20 | need review | Persistent dark appearance and auditable speaker timeline implementation evidence | N/A — workspace is not an initialized Git repository | ATHER |
| 0.3.0b | 2026-07-20 | need review | Stories, processing controls and default-deny MCP agent voice implementation evidence | N/A — workspace is not an initialized Git repository | ATHER |
| 0.3.1b | 2026-07-20 | need review | Android physical-UAT remediation and truthful remaining playback/diarization gates | N/A — workspace is not an initialized Git repository | ATHER |
| 0.3.2b | 2026-07-20 | need review | GenesisBlockDB operational-boundary truth sync and conformance gaps | N/A — workspace is not an initialized Git repository | ATHER |
| 0.4.0b | 2026-07-21 | beta | Genesis relational/unified-transaction cutover, full feature persistence wiring and playback implementation evidence | N/A — workspace is not an initialized Git repository | ATHER |
