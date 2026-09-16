---
version: "0.1.1b"
created_at: "2026-09-17T01:59:54.000+07:00,Codex DOC_ACCEPTANCE,c378af9fac3c00db063948f49f9ee857ebad9126"
last_update: "2026-09-17T02:21:36+07:00,Codex DOC_ACCEPTANCE (fresh finalization)"
status: "candidate"
superseded_by: null
base_sha: "c378af9fac3c00db063948f49f9ee857ebad9126"
branch: "codex/callmd-ui-dag"
attributes:
  domain: "desktop-acceptance"
  doc_type: "feature-specification"
  scope: "P1-A versus P1-B acceptance and proposed baseline RCA; documentation only"
  complexity: "C-3"
  risk: "MEDIUM documentation; HIGH proposed backend/security"
  implementation_authority: "absent"
---

# Call.md-inspired FUNG desktop acceptance plan

## 1. Authority, recommendation, and explicit choices

Boss's “ap[prove” authorizes workflow v0.1.0b and this documentation wave.
It selects neither P1-A nor P1-B and authorizes no executable tests, product code,
CI repair, provider operation, commit, push, or deployment.
The manifest records WORKFLOW accepted, with product_implementation_authorized=false.
Its candidate metadata and earlier scan snapshots do not supersede the actual user instruction.
The prior drafting checkpoint recorded 24 nodes/27 dependencies; the current manifest is authoritative at 25 nodes/28 edges. The gate scan's initial 22-node observation is historical.
Evidence: docs/plans/2026-09-17-callmd-ui-task-dag.json:13,254-365;
docs/verification/implementation-reports/2026-09-17-callmd-ui-orchestration.md:90-105.

Recommended choice: **P1-B**, a useful local review workspace with actual recording history,
native desktop playback, and recording-scoped Q&A, contingent on the reviewed backend contract.
Fallback choice: **P1-A**, resurface existing current-recording functions with honest unavailable
history/playback and legacy local-knowledge Q&A with a scope caveat. Boss may select A without implying B exists.
No new cloud/provider/schema; bookmarks, agenda, derived metrics, and proactive assistance stay P2.
Retain Tauri/React/Rust/Genesis, FUNG identity, Thai-first copy, local access, existing auth custody,
default-off external tools, approval boundaries, and canceled Drive.
Every proposed behavior below is a candidate acceptance requirement, not an existing API.

| Boss choice at feature approval | Recommended disposition | Effect on acceptance |
|---|---|---|
| A or B | B; A remains selectable | B-only ACs become N/A only with recorded explicit A selection |
| Playback transport | PROPOSED native PCM16 player; Tauri controls/metadata only | DOC_REVIEW synchronizes SEC-1/SEC-2, authorization, frame seeks, gaps and cleanup |
| Reopen semantics | Read persisted recording; never implicitly restart capture | Same isolation rules after close/reopen/restart |
| Q&A scope | B: selected recording; A: local knowledge, project filter applies only to transcripts | B excludes graph/live tail; strict project isolation under A requires separately approved repair |
| Baseline remediation | Separate bounded approval | Documentation may proceed; green baseline/code dispatch waits |
| Artifact format and scope | Review scoped three-surface SVG/PNG boards for this wave | Boss explicitly accepts this candidate format/scope or requests the full brief package; no hidden waiver |

The contract's native PCM transport is a concrete PROPOSED design awaiting Boss selection.
DOC_REVIEW must resolve mechanism, DTO/error mapping, resource lifetime, and exact test mapping
across the contracts, UX, and acceptance documents before APPROVAL can authorize affected code.
A transport needing CSP/capability/schema expansion requires an explicit revised scope decision.

## 2. Evidence and current capability boundary

| Ref | Source evidence (file:line; prior scan citations retained) | What it establishes |
|---|---|---|
| E1 | docs/verification/implementation-reports/2026-09-17-callmd-fung-contract-scan.md:78-115; src/App.tsx:627-720; src/tauri.ts:324-338,450-475 | Projects/active recording, scoped transcript, project artifact list |
| E2 | Same scan:117-138; src/components/LiveMeetingPanel.tsx:38-220; src-tauri/src/live_meeting.rs:63-110,1555-1605 | In-memory session owner, recording events, durable recovery; no durable event epoch |
| E3 | Same scan:111-114,169-185; src-tauri/src/meeting_intel.rs:224-280,1019-1086 | Q&A project scope; summaries attributed to recordings; B requires contracts |
| E4 | Same scan:140-163; src-tauri/src/local_api.rs:807-845,899-974,1014-1086; src/components/InstrumentRail.tsx:97-105 | Tokenized web audio/ranges exist; desktop Play unavailable; no desktop playback wrapper verified |
| E5 | docs/design/FRONTEND_REDESIGN_BRIEF.md:45-74,261-278,300-306 | Fixed desktop geometry, brand/Thai, lazy/egress/scoping rules, required state artifacts |
| E6 | package.json:6-38; .github/workflows/ci.yml:27-90; tests/ciCoverage.test.mjs:19-48 | Existing commands and two stale CI invocations; directional coverage gap |
| E7 | .brain/rca/2026-08-10-desktop-callmd-ui-blank-screen.md:17-48 | Historical build-pass/blank-native-root defect motivates native bootstrap proof |
| E8 | docs/Desktop/08-real-progress.md:723-729; docs/Mobile/IMPLEMENTATION_STATUS.md:64-77,176-178 | Historical local/runtime limits; no current candidate acceptance |
| E9 | docs/verification/implementation-reports/2026-09-17-callmd-source-ui-scan.md:29-33,107 | Screenshot is visual reference; channel/word-count assumptions cannot become product facts |
| E10 | docs/specs/2026-09-17-callmd-desktop-contracts.md:90-122,153-291,311-358 | Candidate native PCM/frame/gap/ownership design; corrected legacy Q&A scope and NEW B exclusion policy |

This wave reuses source findings in E1–E4/E9, not a new implementation audit.
The local API is a separate authorized surface; its existence does not establish desktop B.
Summary calls require (projectId, recordingId). Existing meetingAsk filters transcripts by project,
but the legacy local-knowledge path includes graph/live-tail retrieval without project-scope enforcement (E10); unchanged A cannot promise isolated answers. Existing APIs retain their current argument/result/error semantics; NEW B alone applies strict structured validation.
Export creation takes the pair; listExportArtifacts remains project-scoped.
Mic/system/file identify audio origins, not verified people.

## 3. Fixtures, evidence vocabulary, and common oracle

Use a NEW disposable Genesis test profile: projects A/B; A recordings r1/r2; B recording r3.
Each has unique Thai sentinel text, overlapping times, and distinct summary/audio/export fixtures.
Include interrupted r2, zero-transcript r4, deleted/missing audio, and ambiguously attributed summaries.
These are proposed test data, never claims about user data; no schema change or production copy.
Capture immutable selected pair plus local request generation before every asynchronous operation.
Generation is a proposed in-memory UI guard, not an invented durable epoch/API field.
Switch A/r1 → A/r2 → B/r3, resolve earlier requests last, and inject late events from r1.
Require zero stale-response commits or wrong-pair rows, summaries, actions, audio, or notices.
Require zero foreign context/citations for NEW B ask; legacy A must disclose its broader local scope.
Test authorization at Rust/data retrieval, not just renderer filtering; reject A/r3 mismatches.
For Q&A, inspect retrieved context/citations deterministically before any model smoke;
a plausible generated answer alone cannot prove scope isolation.
PASS requires observed evidence; FAIL is a violated assertion; NOT_RUN is no execution;
BLOCKED_ENV records missing prerequisite; N/A requires explicit selected-scope exclusion.
Every result binds AC IDs, candidate SHA/diff digest, input contract digest, environment,
command/exit code, output location, timestamp, and observer. No current product PASS is claimed.

## 4. Feature AC / measurable SC / exit matrix

Each row's exit additionally requires focused behavior tests and independent review at the same revision.
Native rows also require native observations; static/mock evidence cannot close them.

| Feature and AC | P1-A expectation | P1-B expectation | SC and feature exit |
|---|---|---|---|
| AC-01 shell/local bootstrap | Current-recording shell reachable without env/login | Same | Non-zero native root; local record/import/settings/recovery reachable; E5/E7 native evidence |
| AC-02 live lifecycle | Preserve existing start/stop/status and capture consent | Same | One active owner; duplicate starts/stops create no extra session; degraded capture displayed truthfully |
| AC-03 transcript/provenance | Pair-scoped current transcript/corrections | Pair-scoped selected recording | Zero leaked rows; cap warning precedes text; corrections retain source/provenance |
| AC-04 summary | Pair-scoped read/generation | Same on selected recording | Zero other/unattributable summaries shown as current; excluded counts and errors visible |
| AC-05 Q&A | Visible “ค้นความรู้ในเครื่อง”; project limits transcript search only; graph/live context may come from elsewhere | Visible “ถามเฉพาะการบันทึกนี้”; pair-filtered persisted transcripts, graph/live tail excluded | B has zero foreign context/citations; A's caveat is explicit; B unavailable never falls back to legacy ask |
| AC-06 exports/jobs | Export current pair; artifact list labeled project | Export selected pair; same project artifact truth | Correct job identity; no unrelated artifact claimed as selected export; atomic-failure behavior retained |
| AC-07 recording history | Current recording only; no fake older rows | Real complete/paged project recording enumeration | Deterministic ordering, zero duplicate/omitted fixture IDs, honest truncation and unavailable metadata |
| AC-08 native playback | Unavailable with reason; no functional player claim | Play/pause/frame-seek/channel on authorized compatible PCM16 audio | Correct selected frames, declared gap silence/warnings; release on switch; native format/range/path/owner cases pass |
| AC-09 audio presentation | Real supported live level or explicit unavailable | Same; review waveform unavailable, position/seek line only | No fabricated waveform/activity; unavailable ≠ measured zero; position is not sample-exact audible time |
| AC-10 reopen/restart/recovery | Reopen current persisted recording when available | Reopen any enumerated persisted recording | No implicit capture restart; interrupted work disclosed; recovery updates once without duplicate adoption |
| AC-11 async/events | Pair/generation isolation and single subscription owner | Same for all history/audio/Q&A loads | Zero stale commits or unhandled post-unmount changes across prescribed interleavings |
| AC-12 UX/accessibility | Thai-first light/dark; normal/empty/loading/error | Same including B states | Each required artifact/case reviewed; keyboard reaches all enabled actions; no clipping at fixed viewport |
| AC-13 preserved boundaries | Lazy routes/mobile/web/auth/default-off tools | Same | No eager auth/network dependency, direct renderer egress, token exposure, or Drive restoration |

Global SC: every selected AC has evidence; zero unresolved scope/security/isolation failures.
SC does not impose unmeasured latency or throughput promises; record actual hardware/timing.
Product exit: selected ACs, baseline gates, local/frontend/Rust/native proof, task/integration reviews,
and Boss handoff all refer to the same candidate. Package launch is not release/deployment authority.

## 5. Stateful and adversarial test matrix

| Case | Trace | Required observations; applicable scope |
|---|---|---|
| T01 no optional configuration | AC-01,13 | Clean disposable profile, optional cloud env absent, offline: visible FUNG shell and local entry; no forced login. A/B |
| T02 start race/degraded capture | AC-02,11 | Repeated start while starting; close/reopen during capture; mic denied/disconnected; status/error distinguish capture from STT failure. A/B |
| T03 stale async response | AC-03–08,11 | Reverse completion order for transcript, summaries, Q&A, artifacts, history and audio; stale success AND stale rejection cannot replace current state. A/B as applicable |
| T04 cross-project forgery | AC-03–08,13 | Submit A/r3 to pair-scoped native operations; reject without exposing B metadata/content/paths. NEW B ask rejects it; legacy A ask has no recording-isolation promise. A/B per actual contract |
| T05 summary ambiguity | AC-04 | Current, other-recording, missing/ambiguous attribution; render only current with otherRecordings/unattributable counts; no newest-project-summary substitution. A/B |
| T06 summary generation race | AC-04,11 | Switch selection during generation/event delivery; reload event's recording, retain generation guard; no notification on wrong meeting. A/B |
| T07 Q&A isolation/caps | AC-05 | B excludes graph/live-tail sources before inference and sends only pair-filtered persisted transcripts to the prompt; no evidence means no model call; foreign/invalid citations fail. A exposes local-knowledge caveat and searchedRowsCapped, never claims project-isolated context |
| T08 export truth/retry | AC-06 | Old project artifacts coexist with new selected export; only correlate if provenance proves pair/job; failed export never shown complete; previous artifact preserved. A/B |
| T09 history ordering/errors | AC-07 | Empty project, tied timestamps, replayed/expired/foreign cursors, invalid required times/negative duration, resource bound and query failure; INVALID_RECORDING_METADATA ≠ empty. B; A shows scope limit |
| T10 playback lifetime | AC-08,11 | Open r1, play, pause, seek, switch r2/channel, unmount, reopen, restart; no old audio continues or handles survive invalidation. B |
| T11 live level/review seek | AC-09 | Real live levels distinguish measured silence from unavailable/null. Review waveform remains unavailable; a position/seek line is not a waveform. B position follows native status; no invented decoded peaks. A/B |
| T12 incomplete audio | AC-08–10 | Known missing first/middle/last intervals preserve time with silence, degraded=true and missingRanges shown before play; corrupt headers/inconsistent timelines/wholly absent source return explicit errors. Never silently skip gaps. B |
| T13 frame seek/channel | AC-08,13 | Negative/nonfinite/past-duration seek fails; valid seek aligns down to whole frame and reports actual position; EOF play restarts at zero; stale queued frames discarded; unavailable channel never substitutes another. B |
| T14 path authorization | AC-08,13 | Absolute path injection, ../ traversal, encoded traversal, sibling prefix, Windows case/drive/UNC/reparse escape, replaced file; fail closed outside canonical project custody. B |
| T15 listener cleanup | AC-02,10,11 | 10 open/close cycles including listener registration resolving after disposal; no accumulated listeners, repeated rows/actions, or orphan playback handles. A/B |
| T16 restart/recovery | AC-10 | Stop normally then relaunch; interrupt disposable recording then relaunch; live status inactive until confirmed; recover only explicit durable interrupted work, retry idempotently. A/B |
| T17 themes/Thai/keyboard | AC-12 | Two themes × four states at 1280×800; Thai combining marks/long names; Tab/Shift+Tab/Enter/Space/Escape and focus return; loading errors announced without focus theft. A/B |
| T18 preserved routes/egress | AC-13 | Existing lazy desktop surfaces, web /app/auth callback, mobile shell and coarse-pointer web routing retained; default-off tools produce no request; canceled Drive absent. A/B |
| T19 approval custody | AC-05,13 | Disabled/revoked/stale external-tool approvals cannot execute or transfer to another recording; preview/approve/cancel/result retain existing gates. Use controlled local stubs; real external calls require separate authority. A/B |
| T20 formats/resources | AC-08,13 | PCM16 mono/stereo 8–96 kHz at compatible native output rate; compressed/float/mismatched chunks and incompatible devices fail explicitly. Enforce two handles, 64 KiB read blocks, 1 MiB queue; >1 s underrun pauses. B |
| T21 owner/epoch/capture race | AC-02,08,11,13 | Foreign/restarted-window handles, spoofed owner and stale expectedEpoch fail; second player busy; capture-start versus player-open serialized natively. Close acknowledgement precedes capture until native capture is truly inactive; failed start releases admission. B |

Proposed history order: parsed createdAt descending, recording ID ascending; invalid required record
metadata (IDs/times or negative duration) fails INVALID_RECORDING_METADATA; no unknown-last sort. Fixed snapshot pages cannot reorder on concurrent imports.
Test limits 1..100 (default 50), 10,000 rows/8 MiB, two snapshots/window, 5-minute idle/15-minute
absolute expiry; explicit refresh after expiry or new imports. No capped list presented as complete.
Reopen means read durable state; current A lacks historical enumeration and must say so.
Returning to an already active live owner may observe it; navigating/reopening cannot start a new capture.
On restart, stale in-memory live topics are not recovered as persisted facts.
Do not synthesize a durable event sequence to implement test T15; test observable cleanup and scope.
B playback selects one channel on a native cpal/hound PCM16 WAV player; source is mono/stereo
8–96 kHz and the selected device must accept the exact source rate; no resampler/new codec. Compressed imports/float WAV return format unavailable.
List channels describe ledger inventory only; explicit open verifies custody/header/timeline/device.
Gaps use disclosed silence/missingRanges; overlap fails PLAYBACK_TIMELINE_INVALID. Never imply completeness.
Native frame addressing covers [0,durationMs]; no HTTP transport, renderer fetch, or renderer audio buffer.
Existing web local-API range tests remain regression gates; NEW B has no mandatory playback HTTP-range cases and those tests do not verify the native player.
Owner-bound process handles, streamEpoch, queued-frame invalidation and native capture/playback mutual exclusion until capture is confirmed truly inactive are
required security/resource tests (E10). No arbitrary path or bearer token is exposed to the renderer.
Review has no waveform or decoded-waveform feature contract; show unavailable. Its seek line displays native position only.
No person labels, WPM, sentiment, agenda, or bookmarks may be fabricated to fill reference artwork.

## 6. Existing verification commands: inventory, not execution

Run from the approved isolated repository root; all commands below are **NOT_RUN in this wave**.
Their existence was read from package.json:8-38; this does not establish passing tests.

| Layer / related AC | Exact existing command(s) | Source |
|---|---|---|
| CI inventory / AC-13 | npm run test:ci-coverage | package.json:8; tests/ciCoverage.test.mjs:19-48 |
| Bootstrap / AC-01,13 | npm run test:desktop-bootstrap | package.json:19 |
| Session/scoping/jobs / AC-03–06,11 | npm run test:summary-scoping; npm run test:job-actions; npm run test:traceability | package.json:22-24 |
| Auth/authority / AC-13 | npm run test:auth; npm run test:w1-authority-schema; npm run test:device-authority | package.json:14-15,29 |
| Egress/tools / AC-13 | npm run test:egress; npm run test:external-tools | package.json:21,26 |
| Recovery/retained surfaces / AC-10,13 | npm run test:recovery; npm run test:backup-flow; npm run test:device-reconcile | package.json:16-18 |
| Web/audio / AC-08,09,13 | npm run test:local-api-client; npm run test:web-recordings; npm run test:audio-viz | package.json:27-30 |
| Mobile/design / AC-12,13 | npm run test:mobile; npm run test:design-system | package.json:9,13 |
| Audio/package / AC-08,13 | npm run test:transcribe-concat; npm run test:diarization; npm run test:release | package.json:20,25,31 |
| Frontend build | npm run build | package.json:32; expands to tsc && vite build |
| Rust format | cargo fmt --manifest-path src-tauri/Cargo.toml --all --check | .github/workflows/ci.yml:74 |
| Rust lint | cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings | .github/workflows/ci.yml:82 |
| Rust tests | cargo test --manifest-path src-tauri/Cargo.toml | .github/workflows/ci.yml:83 |
| Native development launch, later approval/profile required | npm run desktop | package.json:35; a dev launch is not packaged runtime proof |

The semicolons above separate commands for inventory; capture each command's exit/log independently.
Do not execute the absent test:google-drive or test:native-session-custody as if they exist.
All 21 actual package test commands are inventoried; they cover existing contracts, not all proposed B behavior.
Existing Node source assertions and mocked tests cannot establish native capture/audio/keyboard behavior.

## 7. Future tests and ownership (all NEW; no files authored)

| Proposed NEW path | Future owner | Trace and meaningful assertion |
|---|---|---|
| tests/callmdDesktopContracts.test.mjs | CONTRACT_TESTS | AC-03–11,13; invalid pairs, selected-scope contract, stale completion and explicit unavailable/error distinctions |
| tests/callmdDesktopShell.test.mjs | UI_SHELL | AC-01,12,13; local bootstrap/actions, route/lazy preservation and accessible navigation |
| tests/callmdLiveWorkspace.test.mjs | UI_LIVE | AC-02–05,09–11; lifecycle races, cleanup, provenance and summary exclusions |
| tests/callmdRecordingReview.test.mjs | UI_HISTORY | AC-03–12; A restrictions or B history/player/Q&A state transitions and project export label |
| src-tauri/src/recording_review.rs (NEW module, colocated tests) | BACKEND_RECORDING | AC-07–10,13; actual native pair/path/range/channel authorization and durable reads |
| src-tauri/src/desktop_playback.rs (NEW module, colocated tests) | BACKEND_RECORDING; proposed SEC-1 lease addition | AC-08,10,11,13; PCM formats/gaps/frame seek/owner/epoch/resource behavior |
| tests/callmdDesktopIntegration.test.mjs | INTEGRATE; proposed lease addition | AC-01–13 as selected; mounted ownership, action binding, lazy paths and player/capture exclusion |

Existing src-tauri/src/meeting_intel.rs may receive scoped retrieval tests only in an approved backend lease.
The original manifest proposes the first five paths; E10 adds the player/integration-test paths.
E10 also assigns src-tauri/src/live_meeting.rs admission to BACKEND_RECORDING (SEC-2),
src/components/LiveMeetingPanel.css to UI_LIVE and src/components/InstrumentRail.tsx to INTEGRATE (UI-1).
These exact additions match the manifest's candidate SEC-1/SEC-2/UI-1 leases and require orchestrator lease/DAG revision and named approval before dispatch.
NEW means planned, actual yet UNWRITTEN—not existing/passing; no directory wildcard grants these added paths.
Do not add invented npm script names now. INTEGRATE owns package/CI wiring after test files exist.
Behavioral tests must fail for wrong-pair context/audio and stale results, not merely search for labels.
Mocked DTO/bridge tests precede backend/UI work; Rust authorization and real native runs complete proof.
The current manifest test-runtime contract is Node's existing runner; browser-only TSX interaction tests
may need an explicitly reviewed harness; until available, the test harness and native interaction remain NOT_RUN.
No dependency installation or assertion weakening is authorized to make a candidate appear green.

## 8. Environment and generated-output leases

| Evidence tier | Required environment / limitation |
|---|---|
| Source/docs | Pinned SHA and input digests; read-only inspection. This wave only |
| Node/frontend | Existing installed dependencies; CI declares Node 22 on Ubuntu; record actual compatible Node/npm versions. npm ci needs separate install authority |
| Python concat | Python 3.12 declared in CI; suite starts scripts/transcribe.py and creates temporary audio; real decoder/runtime dependencies must be present, not inferred from fake-model test success |
| Rust | Windows runner, stable Rust with rustfmt/clippy, Windows linker/SDK and build resources; actual availability here UNKNOWN |
| Audio/STT | Approved staged Python/Whisper/model/codec assets for the selected local path; record versions/hashes/profile, never silently download or enable cloud |
| Native Windows | Working WebView2/Tauri runtime, interactive desktop, approved disposable profile, audio output device; mic consent and real capture input for capture cases |
| System capture | Available loopback-capable output/input configuration; test mic/system separately; absent hardware is BLOCKED_ENV, never fabricated samples |
| Packaged runtime | Exact candidate package/resource hash, real profile isolation, cold start/reopen/restart and playback observation; dev/browser evidence insufficient |
| Mobile/web | Existing route/source tests plus scoped browser/native smoke; Android hardware acceptance remains separate, no mobile redesign in P1 |
| Hosted CI | Actual run ID/jobs/conclusion on candidate SHA after separate push authority; no run is created here |

tests/transcribeConcatOnly.test.py:33-40,42-73,121-150 uses subprocess/temp files and one fake
faster_whisper module; scripts/transcribe.py:94-98,177 selects decoder/model paths.
Historical docs/Desktop/08-real-progress.md:729 records 450 Rust passes/6 failures/1 ignored
with missing actual Whisper runtime. This is an environment warning from history, not this run.
No device, native toolchain, runtime assets, decoder, browser automation/test harness, or provider was probed now.
A new isolated worktree/profile alone does not prove the app supports profile redirection:
verify its supported configuration before launch; otherwise BLOCKED_ENV, never use existing user data.
Acquire task-specific leases for dist/, src-tauri/target/, generated resources, logs, screenshots,
temporary fixture audio, and the disposable native data root. Record absolute paths and owners.
Treat .venv-whisper/ and runtime/ as separate staged-resource leases; CI's empty directories
(.github/workflows/ci.yml:75-78) allow build setup but do not prove a transcription runtime.
No shared mutable build outputs across workers. No cleanup/delete of pre-existing user data.
Record disk/resource limits and release leases after processes/handles exit; output writes need authority
even when VERIFY has code_writes=false. Integration owns serialized package/CI/shared-file transfers.

## 9. Proposed baseline RCA and separate remediation gate

**Symptom:** CI invokes two undefined npm scripts at the pinned base; current-head CI was not run.
**Evidence:** .github/workflows/ci.yml:36 calls test:google-drive; :90 calls
test:native-session-custody; neither exists among package.json:6-38's 32 scripts/21 test scripts.
The prior gate scan counted 24 CI npm run invocations: build plus 23 test invocations.
**Root cause (static, bounded):** workflow command inventory and package script inventory diverged.
A successful invocation cannot resolve these missing keys. The removal history/intent of the native
custody suite is UNKNOWN; do not claim the underlying custody behavior or all its coverage is absent.
**Why detection escaped:** tests/ciCoverage.test.mjs:22-35 checks package test scripts → CI;
:37-48 checks test files → package commands. Neither checks workflow npm run → package keys.
That is a demonstrated guard gap, not proof of which historical hosted run encountered it.
**Prevention proposal:** add reverse command closure for every active CI npm run, including non-test
scripts; tokenize command boundaries so test:x-extra cannot satisfy test:x; preserve both existing checks.
Cover multiline/shell commands, arguments, quoted names and comments; unsupported dynamic invocations
must have explicit reviewed resolution, never silently pass. Use negative fixtures for each stale script,
a prefix-collision name, and an unwired test file; positive fixtures cover valid current commands.
**Proposed disposition:** remove the canceled Drive invocation without restoring Drive. For custody,
first identify the actual native authorization assertions/command under the accepted baseline.
Replace the stale invocation with an existing verified equivalent, or approve retirement with explicit
coverage evidence; if no equivalent exists, return a new bounded custody-test proposal for approval.
No silent deletion of a security gate and no dummy package script to hide failure.
**Separate approval/exit:** Boss approves exact baseline diff/RCA scope; BASELINE alone owns
.github/workflows/ci.yml, tests/ciCoverage.test.mjs and the future
.brain/rca/2026-09-17-callmd-baseline-gates.md plus its report. This wave creates none of them.
Exit requires closure in both directions, negative regression tests, preserved custody proof,
required baseline suites, BASELINE_REVIEW, and a lease release before INTEGRATE touches CI.
Historical runs 31610747738/31609642060/31770231402 are documented PASS, not current evidence;
31541234799/31337455450 are historical FAIL. See the gate scan's “Documented run IDs and status”.

## 10. Mockup verification and final review gate

This wave's deliverable is a **scoped three-surface SVG/PNG board package** from DOC_UX.
The UX inventory must name the three represented surfaces and enumerate covered screens/states;
the board count alone does not prove every required screen or state is represented.
Bundled sharp was reported available for UX rendering; this worker has not rendered/probed it.
Check each generated PNG against its source SVG, declared dimensions, clipping and Thai glyphs;
record renderer/version, output digest and actual scale. Do not label an unverified image “2x”.
These boards are reviewable candidates, not the full brief §9.1 Figma/Penpot + 2x/all-screens delivery.
Approval must explicitly select “accept scoped SVG/PNG for this documentation wave” or
“require the full design-source/export package”; retain outstanding full-brief deliverables either way.
No format choice grants implementation authority or changes mobile/web product scope.
DOC_UX artifacts must map each selected screen/control to AC IDs and E1–E5/current or PROPOSED contract.
For each selected desktop screen, inspect low-fi hierarchy plus light/dark high-fi variants across
normal/empty/loading/error; include live idle/listening/degraded/stopped and A/B capability labels.
Use the fixed 1280×800 window and existing 1304×744 scaled stage (E5); show long Thai and font fallback,
clipping, contrast, focus order/visible focus, disabled reasons, source/cap/exclusion notices.
Export an artifact inventory with relative path, digest, state/theme/viewport, observer and result.
An annotated static HTML/SVG/image can prove intended layout only. Mark interaction, keyboard,
audio, async, accessibility tree, data persistence and native mounting NOT_RUN unless actually exercised.
For this wave, final native/frame interaction is explicitly NOT_RUN even when SVG/PNG rendering succeeds.
Rendered mockups are not application screenshots; the provided Call.md screenshot is reference only.
Missing interaction tooling is an explicit limitation, not permission to claim keyboard or runtime PASS.
Preserved settings/pairing/recovery/export surfaces need reachability evidence; broader redesign-brief
mobile/landing deliverables remain outside this selected desktop slice and require their own review.
DOC_REVIEW checks cross-document transport/scope/AC alignment, fixture/test ownership and exact file
leases; return mismatches to the responsible worker. Boss then selects A/B and approves exact digests.
Any semantic input change invalidates dependent tests/reviews; fresh bounded fixers get at most
three unsuccessful cycles before escalation. The orchestrator does not repair implementation.

## 11. Current outcome, unknowns, version diff and changelog

Draft AC: both choices, every feature row, baseline RCA, environment/lease gates and artifact limits exist.
Draft SC: source command inventory is checked; no product execution or approval is represented as PASS.
Draft exit: deliver this candidate and its report for DOC_REVIEW; no self-acceptance or code dispatch.
Open choices: A/B, synchronized native transport details, custody-remediation disposition,
SEC-1/SEC-2/UI-1 and lease additions, and scoped three-surface SVG/PNG versus full-brief delivery.
Unknown: current toolchain/hardware/profile isolation and compatible interaction harness; no probes run.
NOT_RUN: scope selection, test harness execution, all product tests/builds, native/audio/device/browser/hosted CI, Terra DOC_REVIEW and feature approval.
## Version Diff

0.1.0b → 0.1.1b: metadata clarification records the 24-node drafting checkpoint versus the authoritative 25-node/28-edge manifest, with CONTRACT_TEST_REVIEW and review-readiness clarification; product version unchanged.
Draft synchronization: native PCM/seek/gap/ownership tests and legacy Q&A caveat match E10;
review waveform is explicitly unavailable; scoped SVG/PNG format remains a Boss choice.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | candidate | Metadata clarification: historical 24-node drafting checkpoint versus authoritative 25-node/28-edge manifest; CONTRACT_TEST_REVIEW/review-readiness clarification; actual NOT_RUN boundaries retained | UNCOMMITTED; base c378af9fac3c00db063948f49f9ee857ebad9126 | Codex DOC_ACCEPTANCE |
| 0.1.0b | 2026-09-17 | candidate | P1-A/B AC/SC/exit, isolation/audio/UX matrix, commands/environments, proposed baseline RCA | UNCOMMITTED; base c378af9fac3c00db063948f49f9ee857ebad9126 | Codex DOC_ACCEPTANCE |
