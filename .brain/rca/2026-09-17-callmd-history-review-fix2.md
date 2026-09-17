---
version: "0.1.0b"
created_at: "2026-09-17T06:29:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T06:29:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved History lifetime and recovery integration correction"
  risk: "MEDIUM"
---

# History review FIX2

## Symptom and evidence

Independent Terra Confucius01a0ac88-5835-7432-a211-a135ddedb935 returned FAIL
on TSX96d30a55bc5373d5c29a73d637184cfd6ac152d5348eaa815354842eeaef64c3.
RecordingReview.tsx:636-642,942-951,1437-1491 invalidates pollToken on hide/dispose
but retains a global pollInFlight boolean. A getPlayback promise left pending,
followed by successful close and a new open, prevents all new polling indefinitely.
The reviewer separately confirmed no StrictMode list-bootstrap defect; the
controller's earlier tentative suspicion is not a confirmed finding.

Reviewer also confirmed the approved recovery-refresh requirement lacks a local
integration registration seam. App must notify the mounted controller of a
recovered pair without remounting or duplicating native/player ownership.
Existing public controller.refresh() is the operation to reuse; shared contracts
and RecoveryNotice need no edits.

## Root cause

Poll concurrency is represented by a lifetime-global boolean, while poll validity
is session/token scoped. Invalidated unresolved work therefore owns the fence
for later sessions. The local integration API exposes close acknowledgement but
not recovery invalidation for the still-mounted controller.

## Why it escaped detection

FIX1 nine tests cover settled stale success/rejection and close-failure custody,
not a never-settled old-generation poll followed by a new handle. Integration
was not mounted during isolated tests, so no recovery callback binding was used.
No native/device runtime incident or data loss is claimed.

## Bounded FIX2 and prevention

Fresh Luna/max may edit only the original four History lease files in absolute
history-376ef30 paths. Scope the in-flight polling fence to the valid generation/
handle; do not allow an old completion to clear a newer fence. Preserve at most
one poll for the current valid generation and all visibility/custody/epoch guards.
Test hung old poll -> acknowledged close -> reopen -> current poll settles,
then late old success and rejection cannot mutate view or clear current fence.

Add a small local optional registration callback in RecordingReview.tsx (not a
shared contract/store) through which App can notify a recovered RecordingKey.
Only refresh the currently selected matching pair through existing refresh();
foreign/stale/disposed notifications must not publish or release custody. Avoid
remounting or toggling selection merely to refresh. Register/unregister safely
and document the exact API for integration. App invokes it only after the
existing recovery command succeeds; this worker does not edit App/RecoveryNotice.
Add actual-production seam registration/matching/disposal regressions, retain
the nine existing cases and rerun build/summary/jobs/contracts. Report0.1.2b
with beta lifecycle status, separate UNCOMMITTED and actual hashes. Independent
re-review required; no native/CI/environment gate waived.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record token-fence lifecycle defect and missing approved recovery seam | UNCOMMITTED;base376ef30 | Codex orchestrator |
