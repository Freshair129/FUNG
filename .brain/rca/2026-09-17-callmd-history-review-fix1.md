---
version: "0.1.0b"
created_at: "2026-09-17T06:04:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T06:04:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved recording review lifecycle corrections"
  risk: "MEDIUM"
---

# History review FIX1

## Symptom and evidence

Terra Hegel 01a0ac6e-83b4-7710-b972-405f6cc06e64 independently returned FAIL
against RecordingReview.tsx ac0d228f2249d8cc5f930bfd2e5e9740096fd6663feddc32878e784f84fc8aff.
At line635, the 250ms timer can launch another native status request before the
previous one settles. At line1884, void playbackOpen/control event calls discard
normal rejecting promises. At lines487,552, a failed old-player close following
selection/project change emits the previous recording's player identity.

## Root cause

Polling lacks an in-flight fence. UI event boundaries do not consume typed action
rejections. Close-failure custody retention is coupled to current view publication
instead of retaining the old handle separately from the newly selected view.

## Why it escaped detection

Six production-controller tests plus build pass but omit slow overlapping polls,
rejected DOM action boundaries and failed close after a scope switch. Independent
review found them before App integration. No actual user recording was accessed.

## Bounded prevention and verification

Fresh Luna/max may change only the original four History lease files in absolute
history-376ef30 paths. Fence polling in flight, restore the fence on all outcomes
and preserve visibility/disposal guards. Consume action promise failures and
surface typed errors at UI handlers. Retain failed-close custody so capture
admission remains fail-closed, but do not restore an old identity into a new view.
Add deferred-promise tests of actual controller/UI paths for all three findings,
including stale rejection and close-ack false. Preserve selection/pagination/
snapshot/player behavior, shared bridge and contract. Correct report metadata
to0.1.1b with timestamps, lifecycle status, separate UNCOMMITTED and changelog.
Rerun owned tests/build/summary/job-actions and contract tests. No App/shared/CI/
package/native/browser/provider/device changes. Independent re-review required.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record three confirmed History review defects and bounded FIX1 | UNCOMMITTED;base376ef30 | Codex orchestrator |
