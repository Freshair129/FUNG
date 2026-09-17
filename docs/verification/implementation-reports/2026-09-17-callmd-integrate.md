---
version: "0.1.5b"
created_at: "2026-09-17T08:20:37.1845547+07:00,Luna max,UNCOMMITTED base 376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T12:38:00+07:00,Codex documentation-only acceptance reconciliation"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-product"
  doc_type: "implementation-report"
  scope: "P1-B CallMD Home/Live/History integration wiring"
  complexity: "C-3"
  risk: "MEDIUM UI wiring; HIGH native playback and recording custody"
  result: "DONE_WITH_CONCERNS"
  commit: "UNCOMMITTED"
---

# CallMD P1-B integration report

## Result

CURRENT ACCEPTANCE: BLOCKED, not complete. After the subsequent explicit Boss
`Approve`, fresh Bernoulli Luna/max implemented only the three-file dark-label
FIX3; fresh Zeno Terra/high returned PASS_SCOPED with50/50 CallMD Node tests.
Worker build1812 and diffcheck passed. All agents are closed and leases released.
Main changed governance/docs only. Fresh browser verification could not attach
the Codex webview before any page was observed (BLOCKED_BROWSER_ATTACH); no
fresh visual PASS is claimed. App/themeFIX2 and native source remain unchanged.
Browser proof and its limits are in2026-09-17-callmd-browser-reverify.md.
Native source pins remain unchanged; prior Cargo/strictClippy evidence is carried
NOT_RERUN on the new UI snapshot, never relabeled a fresh full-suite pass.

Version diff0.1.4b ->0.1.5b: documentation-only reconciliation of approved FIX3
source/test review,50/50 current Node tests, and browser tooling blocker. No
additional integration implementation or controller code edit was performed.

**DONE_WITH_CONCERNS.** The approved App/shell/live/history integration is
wired in the exact source lease and remains uncommitted. No commit, stage,
merge, push, deploy, native-app launch, provider/device/audio/userdata access,
or credential operation was performed. This report is not self-acceptance.

This completion preserved the historical evidence below, added the two
accepted frozen Live FIX2 test/report artifacts byte-for-byte, forwarded the
approved live phase from `App`, and tightened only the existing mounted History
fixture refresh oracle to exactly one refresh. The six previously reviewed
Socrates transfers remain unchanged; no controller transfer or controller code
edit is claimed.

Base HEAD and branch were preserved:

- `376ef30db13670e4dea816ceff440f44ce73fffd`
- `codex/callmd-ui-dag`

The backend source join was accepted for integration input, but the supplied
backend review (`docs/verification/implementation-reports/2026-09-17-callmd-backend-review.md`,
SHA-256 `dd70ffd80550f27afac60feaee5543f5cb58bd7a1d3faba850d39e1338a47052`)
does not waive full native, hosted-CI, Whisper, strict-clippy, packaged, or
runtime gates.

## Integration outcome

- `App` owns `activeSurface`, theme, project selection, and the independent
  `{projectId, recordingId}` review pair.
- Exactly one `LiveMeetingPanel` and one `RecordingReview` are always mounted;
  navigation changes `visible` rather than unmounting either owner.
- `App` subscribes only to the accepted live controller adapter. Native event
  listeners and live polling remain in `LiveMeetingPanel`.
- `App` forwards `livePhase={liveSnapshot.phase}` adjacent to `liveStatus`, so
  the Shell's accepted phase/read-state precedence receives the owner's
  authoritative phase.
- Capture start receives the review-player close acknowledgement first;
  stop-and-leave delegates to the live controller and requires
  `active === false && stopping === false`.
- Browser reads use explicit `unavailable` state. Browser fallback arrays are
  not presented as verified native history/project success.
- Project selection validates against the full project list, so project six
  and later are not reset by the legacy five-card presentation slice.
- Recovery observation retains only the successful `recovery_scan` mapping and
  verifies `recovery_recover.adopted.recordingId`; no project is guessed.
- Settings, pairing, recovery, export, tools, lazy boundaries, local-only
  desktop bootstrap, and legacy workspace reachability remain wired.
- The rail now exposes actual stop and native review actions; the existing
  `desktopBootstrap` start path markers remain intact.
- `src/tauri.ts` was transferred as an accepted exact-byte artifact and was
  not semantically changed by integration.
- The mounted History fixture now requires `counters.current.list ===
  listBeforeCurrent + 1` for the current-pair refresh; its other five oracles
  and cleanup remain unchanged.

## Exact integration-edited paths and final SHA-256

The following are the leased integration paths. The report's own hash is
computed after this file is written and is recorded in the final handoff
because embedding it here would make the self-hash circular.

| Path | SHA-256 |
|---|---|
| `src/App.tsx` | `d26064747c604dc0d350160e7ccebeb881ecc745557cbd35cfa19d4266334039` |
| `src/styles.css` | `2e54342531d62fb5d1b920d975df9880c27e76e525df219176c96cf4ce4fc14b` |
| `src/components/InstrumentRail.tsx` | `312de9bfc874df16b76442ffcee71f7e98fd97400e358ce80ea25384439e05df` |
| `src/tauri.ts` | `816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe` |
| `package.json` | `1d00347d00ab805c28bd8887d716d6af2a602255763f257cc92d0be4daf75b2d` |
| `.github/workflows/ci.yml` | `3755ef4ce6c9e5adb46ee7c9e746dfdf0df4d21b2785c87a6e1d83daa71d861a` |
| `tests/callmdDesktopIntegration.test.mjs` | `fda7b3d858f26bf4247319161911eda92fb30ebba20409d3bfcb5a5eb7069c35` |

## Exact-byte accepted transfer ledger

The table below is the historical pre-completion ledger retained for
provenance. It records the earlier 30-artifact evidence and is not the current
post-completion root state. The supplied raw 35-path manifest was read back
35/35 before this completion; current accepted pins are reconciled below.

| Tree | Path | SHA-256 |
|---|---|---|
| shared | `.github/workflows/ci.yml` | `33bc2dfa62fe55b8a62f5fa116cbae48cb04b3dc419e15ed86de2cf2d0a58db0` |
| shared | `package.json` | `02c9cd48c56b230852938f7c62063191b8c0b135feb58585b37925aa80006654` *(pre-integration transfer hash)* |
| shared | `tests/ciCoverage.test.mjs` | `2839a68b71308e164562a8f133abed45948636b353c4ea69f91cec2b68f11197` |
| shared | `tests/nativeSessionCustody.test.mjs` | `fe8af82184b1729abf48bae7c8883708def93817e98026701a66407db6a4d8c4` |
| shared | `docs/verification/implementation-reports/2026-09-17-callmd-baseline.md` | `c4ac4fd88e935bc7a261bc67de7bfcb2b69cdecf225f9297156edf506bff7289` |
| shared | `tests/callmdDesktopContracts.test.mjs` | `d6d53a7531e24492ce2ceddf76e58def018602a0cdca33754c1f8bf3e2369ee6` |
| shared | `docs/verification/implementation-reports/2026-09-17-callmd-contract-tests.md` | `e067b84d2b38540efd22d17898c671450306ffa1f305b7c2053a6e819ee84efd` |
| shared | `src/tauri.ts` | `816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe` |
| shared | `src/components/desktop/contracts.ts` | `7f2bd4191f804fca5788684a5891471fda70d713110c5e75b3b5f67870cbae81` |
| shared | `docs/verification/implementation-reports/2026-09-17-callmd-shared-contract.md` | `f8cb9c003d931868229d3ac5d07fbfb9234c403563337b35aa6458744cdbf229` |
| shell | `src/components/desktop/DesktopShell.tsx` | `d9f69e2951b88d8eb51572656695d8c5c38be4d47ca3f2bb8d478452e4ff72f0` |
| shell | `src/components/desktop/DesktopShell.css` | `c2de7032b06e92ca8126f1b680c5a26686108cf5c6363587805e9e3f3c911d7e` |
| shell | `tests/callmdDesktopShell.test.mjs` | `d66520ab6fbb4118958bca5bc460e0ba5a46c585b01f3dbed2a8c9b702baf158` |
| shell | `docs/verification/implementation-reports/2026-09-17-callmd-ui-shell.md` | `42c95b657c5357f7efb868e3277e4db4555ef3540cd9a0b269316b937bdc3a3c` |
| live | `src/components/LiveMeetingPanel.tsx` | `a2a72af87552a9dfa2366de21b3b1f2c5e80b8fd257826457aed1aa3d912d7ca` |
| live | `src/components/LiveMeetingPanel.css` | `5d929f202a2e5bd929e8fb4cc74742ce658e612681046b954b160b95f38249a4` |
| live | `src/components/desktop/LiveWorkspace.tsx` | `9a46ffa5694ebea88efbf72fcdee27d26e40de960e55143a641489963547c48c` |
| live | `src/components/desktop/LiveWorkspace.css` | `a629bf1b2bd5c84539adeb3d34d225930826da9c91293d5cc53fefa8dfe813bd` |
| live | `tests/callmdLiveWorkspace.test.mjs` | `7a2881f7f4afecb275f44a07e3b3f30d796ed203bb682ac4e761eb004abdbca1` |
| live | `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md` | `4a4f1e92602cd9ba2d4054b830ef80567449468a6eedf3c6f49eb42f46530bd5` |
| history | `src/components/desktop/RecordingReview.tsx` | `8e391a68235b0be710ed3e152d63380d16dc25a570c55bde8a7e647f4265537d` |
| history | `src/components/desktop/RecordingReview.css` | `19c92aed68dc112feb142ccf346278684c1ac5f9d95a5e13ad6c85d279caee73` |
| history | `tests/callmdRecordingReview.test.mjs` | `9c8963cc56febb26786e30f21929ffdb59cf29d8b85cfb99b55129d7ea1483d3` |
| history | `docs/verification/implementation-reports/2026-09-17-callmd-ui-history.md` | `eea3dec8901c95a896eb61d027f7e14721261506da56bbbca8487028452bf229` |
| backend | `src-tauri/src/recording_review.rs` | `460e68324427ac358627ac3c6c18e468ad27d668385ef2e3317b49622df3e43e` |
| backend | `src-tauri/src/desktop_playback.rs` | `b1de7c78b94c5711af57c7f9f9e7c0ad54c088dce1f50fb1abdcb3067a583fae` |
| backend | `src-tauri/src/lib.rs` | `2ebb0abcfef27cc5c600ca73b38af6051727c74613a91f974eacf562dc8de7b5` |
| backend | `src-tauri/src/meeting_intel.rs` | `cc87422b81a29fdb32b3af18b58e37799b26d7a22617c6f765cae3481a388477` |
| backend | `src-tauri/src/live_meeting.rs` | `66e85d914ca468a140f8cfd56d30072ccbc8cbd808b0b3560aa359075f5677c5` |
| backend | `docs/verification/implementation-reports/2026-09-17-callmd-backend-recording.md` | `d7b5495d205ec0437ed21564f49a850981b8c8cdfa59d05015c171cddd97d078` |

The shared `package.json` and workflow entries above are the pre-integration
transfer hashes; their post-integration hashes are in the integration table.

## Verification evidence

| Command/evidence | Result |
|---|---|
| `npm run build` | **PASS** — `tsc` plus Vite 8.1.3 production build; 1812 modules transformed |
| `npm run test:ci-coverage` | **PASS** — 4 passed, 0 failed |
| `npm run test:callmd-contracts` | **PASS** — 13 passed, 0 failed |
| `npm run test:callmd-shell` | **PASS** — 8 passed, 0 failed |
| `npm run test:callmd-live` | **PASS** — 11 passed, 0 failed |
| `npm run test:callmd-history` | **PASS** — 11 passed, 0 failed |
| `npm run test:callmd-integration` | **PASS** — 6 passed, 0 failed; includes the approved theme-resolution/subscription regression |
| `npm run test:desktop-bootstrap` | **PASS** — 10 passed, 0 failed |
| `npm run test:summary-scoping` | **PASS** — 6 passed, 0 failed |
| `npm run test:job-actions` | **PASS** — 17 passed, 0 failed |
| `npm run test:egress` | **PASS** — 8 passed, 0 failed |
| `npm run test:local-api-client` | **PASS** — 8 passed, 0 failed |
| `npm run test:traceability` | **PASS** — 1 passed, 0 failed |
| `node_modules/.bin/tsc --noEmit` | **PASS** |
| `git diff --check` | **PASS** |

The five new suites total **48 passing Node cases**. Existing assertions were
retained; the native-session-custody script remains registered and was not
weakened, but its full native/Rust execution is not claimed here.

### Mounted registrar fixture

The explicit component-only command is:

```text
node tests/callmdDesktopIntegration.test.mjs --fixture
```

It starts an ephemeral Vite loopback page at a dynamic URL of the form
`http://127.0.0.1:<port>/`, imports the production `RecordingReview` into a
real `ReactDOMClient.createRoot`, and passes only a deterministic fixture
bridge. It does not install Tauri mocks into the product route and has no
production route.

The Luna integration worker reports guarded CUA observation at
`http://127.0.0.1:5173/` showing
`FIXTURE_COMPLETE` plus six visible PASS rows: registrar A→B cleanup, retained
A after replacement, foreign-pair no-op, current-pair refresh, void/null
registrar cleanup, and retained B after unmount with `list/release delta=0/0`.
The fixture code now guards both shutdown cleanup branches. The worker reports
terminating only the verified fixture process after observation because the
PTY did not deliver a clean SIGINT; the remaining generated root was then
logged, canonicalized, verified as an immediate
`fung-callmd-react-fixture-*` child of canonical `os.tmpdir()`, and removed by
same fail-closed host cleanup check. No broad cleanup was reported. This is
worker-reported observation/cleanup, not an action performed or independently
observed by the main orchestrator. Independent controller reproduction is pending.

This is separate component/React browser evidence. Plain Node CI runs the
integration suite but does **not** claim that the browser fixture ran; the
fixture is not native, packaged, hosted-CI, provider, device, audio, or
production evidence.

## Completion addendum — final source-frozen handoff

This `0.1.2b` completion preserves the historical evidence above and records
the final accepted state. The six reviewed transfers attributed to Socrates
remain unchanged. Cicero completed the two remaining exact-byte Live FIX2
transfers and the three semantic files: `src/App.tsx`,
`tests/callmdDesktopIntegration.test.mjs`, and this report.

Current semantic hashes:

| Path | SHA-256 |
|---|---|
| `src/App.tsx` | `3efb66d9e71b6158eec9e5c0753d7dbcd6c132ac9a186bc40d24c79a9d5fc804` |
| `tests/callmdDesktopIntegration.test.mjs` | `9f015b97e158e65220ba9c2db1f952861ebaa6bd231dea6d268d0c8eb0b0e298` |
| `docs/verification/implementation-reports/2026-09-17-callmd-integrate.md` | computed after final write; supplied in final handoff |

Exactly eight accepted leaf pins match:

| Path | SHA-256 |
|---|---|
| `src/components/desktop/contracts.ts` | `aa6c05c014e62a56d2f2e88f52157c2d1c19ce82b9861018cdb976ce9883ddf2` |
| `src/components/desktop/DesktopShell.tsx` | `e9859d79d8a93c1eae48cbd1b553863203d1892a1ec46a1236f89e8a623f6d76` |
| `tests/callmdDesktopShell.test.mjs` | `1bfe6384bb3ee69d12390c1553af7e10f34a9970cc4a3f06ae10a0f825faec2a` |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-shell.md` | `e58390945779783d4c50c8a92751b54ca2ef2c9717997ec94af722341582d90f` |
| `src/components/LiveMeetingPanel.tsx` | `c9b4599d1b74c7f88898013de51373cdd682924de2850a477bf2ef6baa93ac8c` |
| `src/components/LiveMeetingPanel.css` | `6cd16f22820df076773420fca624507fc78daa05145ff4d3711243258bd11c1f` |
| `tests/callmdLiveWorkspace.test.mjs` | `d77d811c1c6b2b9056f40d3ecc77f462c2455e9aa411c843aedc6692ffe9fd49` |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md` | `3acf87b6864eae3e225e5aa14b9ce4a649b24d1a4f351f853dd38c87a0c49e0e` |

The CI workflow pin is corrected to the raw 64-character value
`3755ef4ce6c9e5adb46ee7c9e746dfdf0df4d21b2785c87a6e1d83daa71d861a`.
The prior 0.1.2b integration rerun was 5/5. The current 0.1.3b theme
correction rerun is recorded below; the existing build output remains 1812
transformed modules and `git diff --check` passed.

Previous native/component evidence is carried forward as **NOT_RERUN**:
Rust 465 PASS, 6 Whisper FAIL, 1 skip, and strict Clippy FAIL. No Cargo,
native-app, provider, device, userdata, credential, or browser command was
run by this completion. The prior worker-reported component-only fixture is
historical evidence; main browser verification remains pending and is not
claimed here.

## Version 0.1.3b addendum — approved integrated theme correction

### RCA and bounded correction

The approved RCA is
`.brain/rca/2026-09-17-callmd-integrated-theme-scope.md`. Main's actual
browser evidence showed the Shell in dark mode while `RecordingReview` stayed
light because the integrated `callmd-surface-stack` had no `.theme-dark`
ancestor; the accepted leaf dark selectors require that owner ancestry.

`src/App.tsx` now resolves explicit `light`/`dark` overrides and resolves
`system` from `prefers-color-scheme` at the App owner. One `matchMedia`
`change` subscription follows light → dark → light transitions and returns
its listener cleanup. The resolved owner class is applied to both the
always-mounted Live/History stack and the retained legacy workspace. No leaf
CSS, shared contract, native/capture listener, live-phase forwarding,
route, provider, dependency, polling, or data code changed in this fix.

### Current source hashes

| Path | SHA-256 |
|---|---|
| `src/App.tsx` | `d26064747c604dc0d350160e7ccebeb881ecc745557cbd35cfa19d4266334039` |
| `tests/callmdDesktopIntegration.test.mjs` | `fda7b3d858f26bf4247319161911eda92fb30ebba20409d3bfcb5a5eb7069c35` |
| `src/styles.css` | `2e54342531d62fb5d1b920d975df9880c27e76e525df219176c96cf4ce4fc14b` |
| `docs/verification/implementation-reports/2026-09-17-callmd-integrate.md` | computed after this final write; returned in handoff |

### Actual checks

| Check | Result |
|---|---|
| `npm run test:callmd-integration` | **PASS — 6/6**; prior five checks retained plus production helper behavior for explicit overrides, system initial state, light → dark → light changes, cleanup, wrapper containment, and shared legacy effective theme |
| `npm run build` | **PASS** — TypeScript plus Vite production build; 1,812 modules transformed |
| `git diff --check` | **PASS** |

### Evidence boundary and release state

This is source/test/build evidence only. Native/component prior gates are
carried as **NOT_RERUN**. Actual OS-dark browser evidence remains unclaimed;
main owns the explicit light/dark browser verification now in progress. The
product/test lease is released. No further product, test, styles, native,
build, or main-review work was performed by this report-only update.

## Owner proof and remaining gates

- Semantic edits in this completion are limited to `src/App.tsx`,
  `tests/callmdDesktopIntegration.test.mjs`, and this report; the two Live
  test/report paths are exact-byte transfers.
- Worker-owned UI/live/history/backend files are exact-byte transfers; any
  future semantic defect must return to its owning review/fixer path.
- No `main.tsx`, bootstrap, SettingsPanel, RecoveryNotice, InstrumentRail.css,
  Rust lockfile/Cargo metadata, schema, auth, config, CSP, or capability file
  was changed by this integration.
- Remaining explicit gates: integrated-candidate Cargo reproduction, strict
  clippy and the known Whisper failures; FIX3's zero-provider-call and delayed
  open/close deterministic production-path evidence was independently accepted
  before integration, but does not prove actual native app/audio behavior;
  clean-VM/package/cold-boot evidence, real native app/audio/provider/device
  checks, hosted CI, and final independent review/acceptance.
- Root remains dirty and **UNCOMMITTED**. No release-readiness claim is made.

## Version diff

- `0.1.3b -> 0.1.4b`: controller documentation-only final acceptance reconciliation;
  records actual dark-label failure, independent scoped FIX2 pass and pause gate.
  No product source/test/style/configuration change.

- `0.1.2b -> 0.1.3b`: recorded the approved integrated theme RCA, effective
  light/dark/system owner correction, production helper transition/cleanup
  evidence at 6/6, current source hashes, and the explicit browser/native
  evidence boundary.
- `0.1.1b -> 0.1.2b`: completed Cicero's two exact-byte Live FIX2 transfers,
  thin `livePhase` forwarding, the exact baseline+1 History oracle, and the
  current local verification record; browser and native gates remain open.

- `0.1.0b -> 0.1.1b`: controller documentation-only correction attributes the
  fixture observation/cleanup to its actual reporting worker and distinguishes
  already accepted FIX3 regressions from pending integrated/native checks.
  No source, test, style or configuration bytes changed in this correction.

- `0.1.0b` (this report): integrates the accepted P1-B shell/live/history
  owners, browser/native read-state boundary, recovery refresh observer,
  rail actions, five-suite package/CI registration, and mounted React fixture.
- Product package version remains `0.1.1`; no dependency or lockfile change.
- Result remains `DONE_WITH_CONCERNS` pending the independent final native and
  controller review gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.4b | 2026-09-17 | beta | Record BLOCKED full acceptance after actual contrast failure; no source edits | UNCOMMITTED; base376ef30 | Codex recorder |
| 0.1.3b | 2026-09-17 | beta / need review | Report-only addendum for the approved integrated theme correction; 6/6 integration test, existing build evidence, current hashes, and browser/native evidence boundaries | UNCOMMITTED; base 376ef30 | RWANG / gpt-5.6-luna/max |
| 0.1.2b | 2026-09-17 | beta / need review | Final source-frozen integration handoff: two Live FIX2 transfers, three semantic files, current checks, and pending browser/native gates | UNCOMMITTED; base 376ef30 | Cicero / gpt-5.6-luna/max |
| 0.1.1b | 2026-09-17 | beta | Correct observer attribution and retained gate wording only | UNCOMMITTED; base 376ef30 | Codex recorder |
| 0.1.0b | 2026-09-17 | beta | Uncommitted P1-B integration wiring with 46 Node cases and separately observed six-PASS React fixture | UNCOMMITTED; base 376ef30 | Luna max |
