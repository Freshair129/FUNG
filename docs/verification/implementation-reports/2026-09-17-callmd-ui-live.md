---
version: "0.1.3b"
created_at: "2026-09-17T12:30:13+07:00,RWANG,gpt-5.6-luna/max,base=376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T12:30:13+07:00,RWANG,gpt-5.6-luna/max"
status: "beta"
review_status: "need review"
task: "UI_LIVE / FIX3"
agent: "RWANG"
model: "gpt-5.6-luna/max"
base: "376ef30db13670e4dea816ceff440f44ce73fffd"
commit: "UNCOMMITTED"
worktree: "C:/Users/pc/.codex/worktrees/9000/fung"
---

# FUNG Live UI — FIX3 handoff

## FIX3 scope and confirmed RCA

This bounded FIX3 lease implements the approved contrast correction in
`.brain/rca/2026-09-17-callmd-live-dark-label-contrast.md`. After FIX2 restored
theme ancestry, `.live-workspace__check` and `.live-workspace__field` retained
the light-palette `#303833` color on the dark capture card. That direct leaf
color overrode the existing dark `--workspace-ink: #f4f1ea`; theme resolution
itself was not defective.

The preimages were verified before editing:

| Path | Preimage SHA-256 |
|---|---|
| `src/components/desktop/LiveWorkspace.css` | `a629bf1b2bd5c84539adeb3d34d225930826da9c91293d5cc53fefa8dfe813bd` |
| `tests/callmdLiveWorkspace.test.mjs` | `d77d811c1c6b2b9056f40d3ecc77f462c2455e9aa411c843aedc6692ffe9fd49` |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md` | `3acf87b6864eae3e225e5aa14b9ce4a649b24d1a4f351f853dd38c87a0c49e0e` |

Only these three files are in this lease. No App/theme resolver, TSX
production logic, native, contract, dependency, route, Tauri, provider,
userdata, credential, package, CI, config, schema, CSP, or capability path was
changed.

## FIX3 bounded implementation and validation

`LiveWorkspace.css` adds one dark-only color declaration for the two existing
label selectors, using `var(--workspace-ink)`. Light mode, layout, markup, and
selector semantics remain unchanged.

`tests/callmdLiveWorkspace.test.mjs` preserves the existing eleven Node tests
and adds one CSS regression that requires the original light `#303833` rule,
the dark `var(--workspace-ink)` override, and no dark layout declarations. The
existing `--fixture` harness remains the real production `LiveMeetingPanel`
under `React.StrictMode`; it now retains its five lifecycle checks and adds
light-label and dark-label observations for seven expected rows total. The
fixture was not run by this worker because main owns browser execution.

The source was frozen after validation. Current source hashes:

| Path | SHA-256 | State |
|---|---|---|
| `src/components/desktop/LiveWorkspace.css` | `99cecc78ca46f1670cbfe062b877071973405a55f3a4f8a7b638c94a8703d61e` | frozen |
| `tests/callmdLiveWorkspace.test.mjs` | `0552e2cc5375dea5a3d48020e44c0ed6e1959df261ecba8ec4abd095f4d423b2` | frozen |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md` | computed after final write | report |

## FIX3 command results and handoff boundary

| Command | Result |
|---|---|
| `node --test --experimental-strip-types tests/callmdLiveWorkspace.test.mjs` | PASS: 12/12, 0 harness errors; original 11 retained plus 1 contrast regression |
| `npm run build` | PASS: TypeScript + Vite, 1,812 modules transformed |
| `git diff --check` | PASS: no whitespace errors in the scoped diff |
| `node tests/callmdLiveWorkspace.test.mjs --fixture` | NOT_RUN by this worker; main-owned browser/server execution |

The fixture/App browser result, Terra review, and any main orchestration result
are not claimed here. Full native/Whisper/strictClippy/hosted-CI gates remain
open and unwaived, as do packaged, provider, device, screen-reader/keyboard,
production, and release gates. This report is a scoped handoff, not
self-acceptance.

## Historical FIX2 evidence (preserved)

## Scope and confirmed RCA

This bounded FIX2 lease addresses the two leaf regressions recorded in
`.brain/rca/2026-09-17-callmd-integrated-live-lifecycle.md` after App
integration:

1. `.live-overlay` had `display: flex`, which overrode the browser's
   `[hidden]` behavior. The hidden Live owner therefore retained rendered
   dimensions and could remain in the visible/focusable surface.
2. `LiveMeetingPanel` constructed its controller adapter during render, while
   a separate cleanup-only effect disposed it. React StrictMode replay reused
   the disposed adapter on the second effect setup, so later snapshot
   publication and subscription became no-ops and the integrated App could
   remain at its initial loading snapshot.

The implementation changes only these four leased paths:

- `src/components/LiveMeetingPanel.tsx`
- `src/components/LiveMeetingPanel.css`
- `tests/callmdLiveWorkspace.test.mjs`
- `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md`

`src/components/desktop/LiveWorkspace.tsx` and
`src/components/desktop/LiveWorkspace.css` remained frozen. App, Shell,
shared contracts, native, provider, device, userdata, credentials, package,
CI, config, schema, CSP, cloud, Rust/Cargo, cache, and Git state were not
changed. The separate Shell phase/read-state correction remains with its
disjoint lane; this report does not claim it.

## Bounded implementation

The adapter is now created at effect setup from the latest snapshot refs and
the current stop delegate, then registered from that same setup. Cleanup first
invalidates the current adapter reference when it still owns the slot, disposes
that adapter, and releases the registration. StrictMode setup/cleanup/setup
therefore receives a fresh live adapter. A retained old controller is disposed
on genuine unmount and rejects its stop action; stale or foreign callbacks
cannot publish through the next owner.

The adapter effect is ordered before the existing lifecycle bootstrap effect.
The existing four subscribe-before-bootstrap listeners, 200-event disclosure
buffer, late unlisten cleanup, pair/epoch/request-generation guards, and start
close-player / stop-ack / true-inactive behavior were preserved. No listener,
poller, fetch, conditional unmount, native call, or fake inactive state was
added.

The hidden owner remains mounted and subscribed. The CSS rule
`.live-overlay[hidden] { display: none !important; }` now removes it from
layout and focus while hidden; visibility changes do not dispose the owner.
The existing `LiveControllerSnapshot.phase` remains authoritative. Existing
bootstrap status, error, and actual `starting` publication paths were not
conflated with Shell read-state mapping.

## Prevention evidence

The existing eleven Node behavioral tests remain intact. The existing test
module now has an explicit `--fixture` mode, separate from Node assertions,
which mounts the production `LiveMeetingPanel` through installed
ReactDOMClient/Vite under `React.StrictMode`. It does not copy production
orchestration, add a production route, inject runtime JavaScript, or add a
dependency.

Fixture startup and evidence boundary:

- Command: `node tests/callmdLiveWorkspace.test.mjs --fixture`
- Observed URL: `http://127.0.0.1:5174/` (Vite selected 5174 because 5173 was
  already occupied)
- Browser observations from this RWANG / gpt-5.6-luna/max run only:
  - PASS hidden owner: `hidden=true`, `display=none`, `rect=0x0`.
  - PASS visibility toggle: `display=flex`, `rect=1280x720`, `owners=1`.
  - PASS StrictMode replay: published `status=ready`, `active=false`,
    `registrations=2` from the browser-absent authoritative status fallback.
  - PASS native-absent truth: `NATIVE_UNAVAILABLE` remained visible and the
    published status was no longer loading.
  - PASS genuine unmount: retained controller rejected with
    `LEGACY_COMMAND_FAILED`.
- The helper was stopped after closing that browser tab; its exact generated
  root `C:/Users/pc/AppData/Local/Temp/fung-callmd-live-fixture-EyZ4a5` was
  canonicalized, verified as an immediate task-prefixed child of the system
  temp directory, and then removed. No broad cleanup was used.
- These are component-only browser observations, not native, packaged,
  provider, device, hosted-CI, or production evidence. They are attributed to
  this run only; they are not Terra/Pauli or main-orchestrator acceptance.

## Changed paths and final handoff hashes

The report's own SHA-256 is computed after its final write and is supplied in
the final handoff; it is intentionally not self-embedded.

| Path | SHA-256 | State |
|---|---|---|
| `src/components/LiveMeetingPanel.tsx` | `c9b4599d1b74c7f88898013de51373cdd682924de2850a477bf2ef6baa93ac8c` | changed |
| `src/components/LiveMeetingPanel.css` | `6cd16f22820df076773420fca624507fc78daa05145ff4d3711243258bd11c1f` | changed |
| `src/components/desktop/LiveWorkspace.tsx` | `9a46ffa5694ebea88efbf72fcdee27d26e40de960e55143a641489963547c48c` | frozen |
| `src/components/desktop/LiveWorkspace.css` | `a629bf1b2bd5c84539adeb3d34d225930826da9c91293d5cc53fefa8dfe813bd` | frozen |
| `tests/callmdLiveWorkspace.test.mjs` | `d77d811c1c6b2b9056f40d3ecc77f462c2455e9aa411c843aedc6692ffe9fd49` | changed |
| `docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md` | computed after final write | changed |

Result: `UNCOMMITTED`; base `376ef30db13670e4dea816ceff440f44ce73fffd`.

## Commands and results

All commands ran from the assigned isolated worktree.

| Command | Result |
|---|---|
| `node --test --experimental-strip-types tests/callmdLiveWorkspace.test.mjs` | PASS: 11/11, 0 harness errors |
| `node tests/callmdLiveWorkspace.test.mjs --fixture` + browser observation | PASS: 5/5 fixture rows; separate from Node proof |
| `npm run test:external-tools` | PASS: 5/5 |
| `npm run test:summary-scoping` | PASS: 6/6 |
| `npm run test:traceability` | PASS: 1/1 |
| `npm run build` | PASS: TypeScript + Vite, 1,808 modules |
| `node --test --experimental-strip-types tests/callmdDesktopContracts.test.mjs` | EXPECTED_RED_MISSING_IMPLEMENTATION: 12 pass / 1 native-presence failure, 0 harness errors |
| `npm run test:ci-coverage` | INTEGRATION_REGISTRATION_OPEN: 3 pass / 1 failure; `callmdDesktopContracts.test.mjs` and `callmdLiveWorkspace.test.mjs` remain unwired |

The isolated package has no `test:callmd-contracts` script; the contract file
was invoked directly and its native-presence assertion was not weakened.

## Review state and limits

React best-practices guidance was applied to the changed TSX: adapter and
registration share one effect lifetime, mutable lifecycle/delegate data stays
in refs, effect dependencies remain narrow, the existing listener topology is
preserved, and no new dependency or inline production component was added.

This is source, Node, local build, and component-only browser-fixture evidence.
Native PCM16/microphone/device, packaged runtime, provider, hosted CI,
screen-reader/keyboard acceptance, production, and self-acceptance evidence
remain `NOT_RUN`. The native-presence contract red and CI-registration red are
separate integration/backend gates, not waivers. Independent Terra review and
the authorized root integrator remain required; this report does not
self-accept the implementation.

## Version diff

`0.1.2b` → `0.1.3b`: added the approved dark form-label contrast correction,
the light-preservation/dark-token Node regression, and the two corresponding
production-component fixture observations while preserving all FIX2 evidence.
No browser fixture result is claimed by this worker.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.3b | 2026-09-17 | beta / need review | FIX3 dark form-label contrast correction; 12/12 focused Node tests and frontend build passed; fixture/browser delegated to main | UNCOMMITTED; base 376ef30 | RWANG / gpt-5.6-luna/max |
| 0.1.2b | 2026-09-17 | beta / need review | FIX2 StrictMode adapter lifetime and true hidden-layout correction with actual React fixture evidence | UNCOMMITTED; base 376ef30 | RWANG / gpt-5.6-luna/max |
| 0.1.1b | 2026-09-17 | beta / need review | FIX1 owner-safe handoff, lifecycle epoch/request guards, and empty-summary disclosure preservation with focused regressions | UNCOMMITTED; base 376ef30 | RWANG / gpt-5.6-luna/max |
| 0.1.0b | 2026-09-17 | beta | Initial bounded Live UI implementation; Terra found three FIX1 defects | UNCOMMITTED; base 376ef30 | RWANG / gpt-5.6-luna/max |
