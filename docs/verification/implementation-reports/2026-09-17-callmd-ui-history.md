---
version: "0.1.3b"
created_at: "2026-09-17T06:39:52.950+07:00,Codex,Luna/max,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T06:53:26.3965939+07:00,Codex controller documentation correction"
status: "need review"
superseded_by: null
attributes:
  doc_type: "implementation-report"
  scope: "UI_HISTORY"
  risk: "MEDIUM/C-3"
  agent: "Codex"
  model: "Luna/max"
  base: "376ef30db13670e4dea816ceff440f44ce73fffd"
  evidence: "local static/build plus mocked production controller only"
  lifecycle: "beta"
  review_status: "independent Terra required"
---

# UI_HISTORY implementation report — FIX2

## Scope and correction

Implemented the selected-B desktop recording history/review surface in the assigned
history-376ef30 worktree. App/shared/bridge/CI/package/native source was not
implemented or registered here. The earlier root-location build is historical
wrong-location evidence and is not candidate proof. This FIX2 is limited to the
approved RCA lifecycle correction; no new feature scope was added.

The bounded FIX1 and FIX2 RCA corrections were completed before validation:

- Ten accepted dependency files were materialized in the assigned worktree and
  raw SHA256-verified.
- RecordingReview.tsx and .css were transferred there with their preserved
  source hashes before root cleanup.
- Root src/tauri.ts was restored via absolute apply_patch to the HEAD 376ef30
  blob. Only the six matching introduced root copies were removed.
- Root pre-existing baseline files were not touched.
- Playback polling now has a generation/handle/epoch/identity fence; invalidating
  a generation releases only that generation's fence, so a hung old poll cannot
  block a reopened player and its late settlement cannot clear a newer fence.
- Playback-open/control UI event promises are consumed while the controller
  retains and renders their typed safe errors.
- Failed close custody retains the old handle separately, keeps capture
  admission fail-closed, and never republishes an old identity after scope
  changes.
- A local optional recovery-refresh registration seam lets the mounted review
  controller reuse refresh() only for its current exact RecordingKey. Controller
  tests verify matching/foreign/disposed behavior; registration cleanup guards
  are source-reviewed, not mounted-effect regression evidence.

## Owned deliverables

| File | SHA256 |
|---|---|
| src/components/desktop/RecordingReview.tsx | 8e391a68235b0be710ed3e152d63380d16dc25a570c55bde8a7e647f4265537d |
| src/components/desktop/RecordingReview.css | 19c92aed68dc112feb142ccf346278684c1ac5f9d95a5e13ad6c85d279caee73 |
| tests/callmdRecordingReview.test.mjs | 9c8963cc56febb26786e30f21929ffdb59cf29d8b85cfb99b55129d7ea1483d3 |
| docs/verification/implementation-reports/2026-09-17-callmd-ui-history.md | recorded after write in handoff |

The assigned worktree also contains the ten frozen phase0 dependencies at the
accepted hashes from the dispatch packet; they are not implementation edits.

## Frozen lease hashes

| File | SHA256 |
|---|---|
| .github/workflows/ci.yml | 33bc2dfa62fe55b8a62f5fa116cbae48cb04b3dc419e15ed86de2cf2d0a58db0 |
| package.json | 02c9cd48c56b230852938f7c62063191b8c0b135feb58585b37925aa80006654 |
| tests/ciCoverage.test.mjs | 2839a68b71308e164562a8f133abed45948636b353c4ea69f91cec2b68f11197 |
| tests/nativeSessionCustody.test.mjs | fe8af82184b1729abf48bae7c8883708def93817e98026701a66407db6a4d8c4 |
| docs/verification/implementation-reports/2026-09-17-callmd-baseline.md | c4ac4fd88e935bc7a261bc67de7bfcb2b69cdecf225f9297156edf506bff7289 |
| tests/callmdDesktopContracts.test.mjs | d6d53a7531e24492ce2ceddf76e58def018602a0cdca33754c1f8bf3e2369ee6 |
| docs/verification/implementation-reports/2026-09-17-callmd-contract-tests.md | e067b84d2b38540efd22d17898c671450306ffa1f305b7c2053a6e819ee84efd |
| src/tauri.ts | 816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe |
| src/components/desktop/contracts.ts | 7f2bd4191f804fca5788684a5891471fda70d713110c5e75b3b5f67870cbae81 |
| docs/verification/implementation-reports/2026-09-17-callmd-shared-contract.md | f8cb9c003d931868229d3ac5d07fbfb9234c403563337b35aa6458744cdbf229 |

## Implementation evidence

- Real listRecordings history with limit 50, cursor paging, snapshot release on
  refresh/project change/dispose, and distinct idle/loading/ready/empty/error/
  unavailable states.
- Exact {projectId, recordingId, selectionEpoch, requestId} guards cover
  recording, transcript, summaries, project exports, Q&A, playback open/control/
  poll, including stale rejection paths. Frozen settleReviewLoad is used.
- Detail reopen uses getRecording; transcript and summaries retain the exact
  pair. Project exports are labeled ไฟล์ส่งออกทั้งโครงการ; export.render
  captures the clicked pair.
- Q&A calls askRecording, keeps the question on failure, clears old answers
  when the question changes, and renders source time/text. No legacy meetingAsk.
- Playback opens only by explicit action, starts paused, uses PCM16 WAV wording,
  serializes handle/epoch controls, supports +/-5s/Home/End, polls every 250ms
  only while visible, closes on selection/visibility/disposal, closes late
  handles, and exposes closePlayer(): Promise<{closed:boolean}>.
- Quiet Archive porcelain/ink/slate/sage styling, Thai honest labels, semantic
  buttons/forms, aria-busy, focus styles, and no fabricated waveform.
- Corrected stale playback-epoch rejection to retain the valid current playback
  state while surfacing the typed stale-epoch error.
- FIX1/FIX2 tests exercise the production controller through deferred poll
  success/rejection, including a hung old poll followed by acknowledged
  close/reopen, one current-generation poll in flight, late old settlements,
  event-boundary rejection consumption, false close acknowledgement, retained
  capture custody, exact-pair recovery refresh, and controller disposal.
  Registrar cleanup guards have static review only; mounted unregistration and
  retained-callback-after-cleanup behavior remain NOT_RUN as stated below.

## Acceptance evidence

| AC | Local evidence in this lease | Boundary |
|---|---|---|
| AC-03 | Pair/epoch/request guards; transcript correction uses the exact pair | Native authorization is not proven here |
| AC-04 | Summary reads validate the selected pair; summary-scoping 6/6 | Native summary/runtime not proven |
| AC-05 | `askRecording`, pair-filtered answer validation, retained question/error, no legacy ask | Provider and prompt execution not run |
| AC-06 | Project export label and clicked-pair `export.render`; job-actions 17/17 | Export runtime not proven |
| AC-07 | Limit-50 pages, deterministic snapshot identity, refresh/release and empty/error/unavailable states | Native list command not present in this isolated tree |
| AC-08 | Paused explicit open, serialized epoch controls, close/reopen and no renderer audio path | PCM16 device output/native custody not proven |
| AC-09 | Position-only seek rail and explicit no-waveform presentation | No live/native audio observation |
| AC-10 | Read-only reopen and disposal/close lifecycle regressions | Restart/recovery runtime not run |
| AC-11 | Stale success/rejection, hung poll generations, event rejection consumption, disposal guards | Independent Terra review pending |
| AC-12 | Thai/Quiet Archive CSS, semantic controls, `aria-busy`, focus styles | Browser keyboard/screen-reader review not run |
| AC-13 | No fetch/audio URL/buffer, no shared/App/CI/config changes, Drive untouched | Hosted CI and packaged proof not run |

## Local validation

All commands below used explicit cwd
C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/history-376ef30.

| Command | Result |
|---|---|
| node --test --experimental-strip-types tests/callmdRecordingReview.test.mjs | PASS: 11/11 |
| npm run test:summary-scoping | PASS: 6/6 |
| npm run test:job-actions | PASS: 17/17 |
| npm run build | PASS: tsc + Vite build |
| node --test --experimental-strip-types tests/callmdDesktopContracts.test.mjs | 12 pass, 1 expected native-presence red; no harness error |

Owned tests exercise selection success/rejection races, pagination release,
wrong-pair payloads, Q&A error scope, no autoplay, late open close, control
epochs/serialization, polling/disposal, hung-generation fencing, recovery
registration scope, and project-vs-pair export scope.

CI registration of this new test remains Integration-owned; no package/CI edit
was made and no inventory result is being waived here.

## Integration API

RecordingReview with selectedProjectId, selection, onSelect, optional scopeChoice,
visible, registerClosePlayer, registerRecoveryRefresh, and bridge is the
integration entry point. `registerRecoveryRefresh` receives
`(selection: RecordingKey) => Promise<void>` after recovery succeeds; it calls
the mounted controller's existing refresh() only when the exact pair is selected.
Static review confirms registration cleanup invalidates old callbacks. Tests
invoke controller.refreshRecovered directly and do NOT mount the registrar or
invoke a retained callback after React cleanup. That mounted registration/
unregistration regression is NOT_RUN and remains owned by Integration/final
audit; no waiver or browser/native proof is claimed. The App owns project/selection/theme/routes.
Review owns request identity, reads, playback lifecycle, and snapshot release.
registerClosePlayer receives the close adapter and must await {closed:false} as
fail-closed before capture admission.
bridge is injectable for integration tests and implements the accepted list,
detail, transcript, summary, export, Q&A, job, correction, rename, and playback
wrappers. RecordingReviewView consumes frozen RecordingReviewProps plus question
callbacks.

## Limits and handoff

This is local static/build plus mocked-bridge production-controller evidence only.
It does not prove native module presence, Rust behavior, PCM16 device output,
microphone/device behavior, provider availability, user data, browser focus
runtime, hosted CI, or production readiness. The accepted contract suite's one
red assertion is the separate missing-native-module gate.

No commit, staging, push, merge, deploy, dependency install, schema/auth/config/
CSP/capability change, or Drive action was performed. Work remains
UNCOMMITTED. Independent Terra review and integration-owned App wiring/runtime
checks are required; this worker does not self-accept the change.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.3b | 2026-09-17 | need review | Controller documentation-only correction: distinguish controller tests from unrun mounted registrar lifecycle evidence; source/tests unchanged | UNCOMMITTED; base376ef30 | Codex controller |
| 0.1.2b | 2026-09-17 | beta / need review | Bounded FIX2: generation-scoped hung-poll fence and safe selected-pair recovery-refresh registration with deferred regressions | UNCOMMITTED; base376ef30 | Luna/max |
| 0.1.1b | 2026-09-17 | beta / need review | Bounded FIX1: poll fence, consumed playback rejections, and separate failed-close custody with deferred regressions | UNCOMMITTED; base376ef30 | Luna/max |
| 0.1.0b | 2026-09-17 | beta | Initial UI_HISTORY selected-B implementation and local evidence | UNCOMMITTED; base376ef30 | Luna/max |
