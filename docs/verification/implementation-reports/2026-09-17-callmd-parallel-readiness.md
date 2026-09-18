---
version: "0.1.0b"
created_at: "2026-09-17T02:54:33+07:00,Luna max worker,HEAD 376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T02:54:33+07:00,Luna max worker"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-report"
  scope: "P1-B UI/history plus PCM16 playback and recording-scoped Q&A scheduling readiness"
  complexity: "C-3"
  risk: "MEDIUM UI; HIGH native/backend admission"
  evidence_boundary: "read-only scheduling and ownership inspection; no product implementation"
---

# P1-B parallel readiness

## Outcome

Full P1-B intent is confirmed: new desktop UI, real project recording history,
compatible integer PCM16 WAV playback, and recording-scoped Q&A. “Do both in
parallel” confirms scope; it does not create an overlapping backend/UI lease.
The current DAG has one safe implementation parallel wave only:

`UI_SHELL || UI_LIVE || UI_HISTORY`, after the backend, shared-contract, and
review gates. `BACKEND_RECORDING` cannot run concurrently with `UI_HISTORY` in
the accepted graph.

Worker result: `DONE_WITH_CONCERNS` — readiness is documented, but downstream
product nodes remain `WAITING_APPROVAL`/`WAITING_DEPENDENCY`; this is not a
Terra verdict, feature acceptance, or code-dispatch record.

## Evidence and frozen boundaries

| Evidence | Finding |
|---|---|
| Pinned execution | Branch `codex/callmd-ui-dag`, HEAD `376ef30db13670e4dea816ceff440f44ce73fffd`; product source base `c378af9fac3c00db063948f49f9ee857ebad9126`. |
| Current scope record | Approval record confirms full B and says not to re-ask A/B or playback/Q&A; product dispatch remains gated. [`callmd-approval.md:44-57`](2026-09-17-callmd-approval.md:44) |
| DAG | Manifest v0.1.4b declares max 3 Luna/max workers, `product_implementation_authorized=false`, and current code nodes remain planned. [`callmd-ui-task-dag.json:1-15`](../../plans/2026-09-17-callmd-ui-task-dag.json:1) |
| Accepted inputs | DOC_CONTRACT, DOC_UX, DOC_ACCEPTANCE and DOC_REVIEW are accepted; the reviewed author leases are released. No implementation lease is active. |
| Main divergence | Controller observed local main has advanced to `05ed107a`, with stale CI steps removed by `d10bbf8`, via worktree/diff inspection. That is a read-only candidate only; no adoption, merge, or lease transfer is authorized. If considered later, baseline identity and acceptance must be revalidated against the pinned branch/target SHA, with a recorded DAG revision and descendant invalidation where applicable. |

The current controller documents and preflight state are preserved. This report
does not modify the ledger, manifest, approval record, source, tests, CI, or
design assets.

## Exact ownership and lease bottlenecks

The following are the frozen section-10 partitions, not invented interfaces:

| Node / lane | Exact owned paths relevant here | Admission |
|---|---|---|
| `BACKEND_RECORDING` | `src-tauri/src/recording_review.rs`, `src-tauri/src/desktop_playback.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/meeting_intel.rs`, `src-tauri/src/live_meeting.rs`, plus colocated Rust tests and its report | One HIGH-risk lease. Serial after `CONTRACT_TEST_REVIEW`; then `BACKEND_REVIEW`. It owns recording enumeration, native PCM playback, capture admission, and new recording-scoped Q&A. |
| `SHARED_CONTRACT` | `src/components/desktop/contracts.ts`, `src/tauri.ts`, and its report | Serial after `BACKEND_REVIEW`; then `SHARED_REVIEW`. `src/tauri.ts` cannot be shared with `INTEGRATE`. |
| `UI_HISTORY` | `src/components/desktop/RecordingReview.tsx`, `src/components/desktop/RecordingReview.css`, `tests/callmdRecordingReview.test.mjs`, and its report | Ready only after `SHARED_REVIEW`; consumes the accepted B action/DTO contract and owns no backend or bridge file. |
| `UI_SHELL` / `UI_LIVE` | Shell paths, or the named `LiveMeetingPanel`/`LiveWorkspace` paths and test/report paths in the manifest | Disjoint from `UI_HISTORY`; may join the same UI wave after `SHARED_REVIEW`. |
| `INTEGRATE` | `src/App.tsx`, `src/styles.css`, `src/tauri.ts`, `package.json`, `.github/workflows/ci.yml`, `src/components/InstrumentRail.tsx`, integration test/report | One serial integration lease after UI review. CI transfers only after `BASELINE_REVIEW`; bridge transfers only after shared/UI review. |

The manifest ownership check found zero exact-path overlap among the three UI
lanes. The unavoidable bottlenecks are the single backend lease (including
`lib.rs` registration and native capture/playback admission), the shared bridge,
and the baseline/CI lease. UI-level guards cannot replace the native admission
contract.

## Maximal safe schedule using existing nodes

1. **Current preflight:** documentation-only baseline verification/RCA and this
   readiness report may run concurrently because their exact write paths are
   disjoint. Neither is a baseline acceptance or a product-code dispatch.
2. **Admission, serial:** retain the full-B scope record; obtain the scoped
   SVG/PNG format decision separately; keep CI/custody remediation as a separate
   authorization. Do not dispatch product code while either required gate is
   unresolved.
3. **Baseline, serial:** `BASELINE → BASELINE_REVIEW`. No CI repair is
   authorized by this task. The main-side `d10bbf8` is not adopted into the
   pinned branch and cannot substitute for this gate.
4. **Contract tests, serial:** `CONTRACT_TESTS → CONTRACT_TEST_REVIEW`.
5. **Native B backend, serial:** `BACKEND_RECORDING → BACKEND_REVIEW`.
6. **Shared registration, serial:** `SHARED_CONTRACT → SHARED_REVIEW`.
7. **UI wave, maximal current concurrency:** dispatch exactly
   `UI_SHELL || UI_LIVE || UI_HISTORY` (three workers maximum, disjoint paths),
   then `UI_TASK_REVIEW`.
8. **Integration and proof, serial:** `INTEGRATE → VERIFY →
   INTEGRATION_REVIEW → HANDOFF`. No commit, push, merge, release, device,
   hosted-CI, or production claim follows from this report.

## Backend/UI overlap decision

No alternate overlapping execution is currently authorized. The accepted
dependency chain is:

`CONTRACT_TEST_REVIEW → BACKEND_RECORDING → BACKEND_REVIEW →
SHARED_CONTRACT → SHARED_REVIEW → UI_HISTORY`.

Running `UI_HISTORY` beside `BACKEND_RECORDING` would consume an unreviewed
bridge/native contract and would bypass the accepted `BACKEND_REVIEW` and
`SHARED_REVIEW` gates. It is therefore unsafe under the current manifest.

If true backend/UI overlap is later desired, the exact control-plane proposal is
to add a read-only, hash-pinned backend-interface review node after
`CONTRACT_TEST_REVIEW`; make `SHARED_CONTRACT` depend on that interface review
while `BACKEND_RECORDING → BACKEND_REVIEW` runs separately; and add
`BACKEND_REVIEW` as a required dependency of `INTEGRATE`. The amendment must
also preserve the `src/tauri.ts` transfer and exact backend/UI leases. This is a
proposal only, not an applied DAG change and not permission to invent an API.

## Independent gates and evidence status

- Scoped three-surface SVG/PNG delivery: **WAITING_APPROVAL**; static mockup
  evidence is not runtime or full-brief acceptance.
- Baseline CI/custody repair: **NOT_AUTHORIZED**; read-only candidate evidence
  may be reviewed, but no repair or main-side adoption is claimed.
- Product tests, frontend/Rust builds, native audio, device, hosted CI and
  production: **NOT_RUN**.
- Commit/push/merge/release: **NOT_AUTHORIZED**.

## Version Diff

- `new → 0.1.0b`: bounded P1-B scheduling report; confirms the only current
  implementation parallel wave is the three UI lanes after serial backend and
  shared-contract review, records disjoint ownership and baseline/main-SHA
  boundaries, and preserves `NOT_RUN`/authorization limits.
- Application source/version: unchanged.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | candidate | Record bounded P1-B concurrency readiness; no product implementation or baseline adoption | UNCOMMITTED; pinned HEAD 376ef30 | Luna max worker |
