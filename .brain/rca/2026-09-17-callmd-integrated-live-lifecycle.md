---
version: "0.1.1b"
created_at: "2026-09-17T08:31:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T08:37:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Observed integrated browser Live owner and hidden-view regressions"
  risk: "MEDIUM live owner and navigation safety; no native wire change"
---

# Integrated Live lifecycle: browser failure before final acceptance

## Symptom

On frozen integrated candidate App hashc23c879088dab4656bbd7c5f639c953839d2a2a3798591d71f610bd7314a7716,
main Codex CUA opened /app?surface=desktop at127.0.0.1:15472,1280x800.
No capture was started and no native bridge/app/provider was present.
Home persistently showed preparing capture and a session-active sidebar.
The Live panel was physically visible below Home although marked hidden.
Further browser flow verification stopped at this first broken boundary.
Console warning/error inventory was empty; lack of console errors is not PASS.

## Evidence

- Actual DOM Live DIV:class live-overlay live-overlay--hidden,hidden=true,
  computed display:flex,rectangle864.625x1246.6875. Screenshot visibly shows it.
  Hidden History SECTION has display:none and0x0 rectangle; it is not this fault.
- LiveMeetingPanel.css base .live-overlay sets display:flex; :607 hidden modifier
  only disables pointer events. Author display overrides the UA hidden rule.
- LiveMeetingPanel.tsx:591 creates controllerAdapterRef once; adapter:154/165/178
  permanently disposes, no-ops subscribe/publish after disposal. Cleanup-only
  effect:1014-1016 disposes it without recreation for effect setup replay;
  registration:1019-1023 exposes the same controller again.
- main.tsx:82-86 retains React.StrictMode. This setup/cleanup/setup replay leaves
  the once-created adapter disposed while the Live owner updates its local errors.
- App:111 starts with a truthful native-unavailable snapshot, then :794 reads
  the child's initial loading snapshot and subscribes to that disposed adapter.
  DesktopShell:75 maps every status-read loading state to capture starting.

## Root cause

Two independent integration-visible defects: hidden-view CSS does not actually
remove the Live view from layout; the Live controller handoff has a one-shot
resource lifetime incompatible with StrictMode effect replay. Consequently the
parent keeps the initial loading snapshot although the child displays a command
error. Generic status loading is additionally conflated with a capture-start
operation by the Shell mapper; native startup reads must not assert recording.

## Why it escaped

Isolated adapter tests cover disposal and publishing individually, not a real
React StrictMode owner lifecycle. Integration Node tests inspect composition;
the separate mounted fixture exercises History registrar, not App/Live. A
hidden attribute/source assertion does not test computed layout. Build and
46passing new Node cases therefore did not establish this browser boundary.

## Prevention / bounded fix requirements

Return source defects to fresh Luna owning lanes, never orchestrator code edits.
Preserve exactly one always-mounted Live owner, existing StrictMode and all four
event listeners/cancellation guards. Make owner adapter setup replay-safe while
retained callbacks from genuine disposal remain unusable. Ensure hidden Live
has no rendered dimensions/focusable view without unmounting its owner.
Separate authoritative active/stopping status and actual start phase from plain
status-bootstrap loading; unavailable/error remains truthful, no fabricated
inactive/active payload, no guard weakening. Exact file leases are fixed after
the independent integration preflight returns; no new feature or native wire.

Required evidence: regression tests through actual owner/adapter path and real
React StrictMode mount/cleanup/setup; main browser reproduction on a frozen
candidate has no false capture banner/session assertion and zero hidden-View
dimensions. Then repeat affected integration/build/Node checks and independent
review before acceptance. Earlier source acceptance is invalid for changed
descendants; preserve old hashes/results as historical. Real native/device/audio
and known full-suite/CI gates remain separate. Isolated UI_LIVE FIX2 now owns
its original Panel/CSS/test/report paths; root verification remains unchanged.

## Bounded Shell phase handoff correction for interface review

Independent Terra Pauli confirms the third semantics defect. Existing LivePhase
already models start/stop and is published by LiveControllerSnapshot. Approved
UX requires truthful capture guards; ReadState.loading is a read, not capture.

Corrective internal handoff: DesktopShellProps receives optional livePhase:LivePhase
(default idle for compatibility), forwarded directly from the existing owner by
App. Shell uses explicit phase for actual starting/stopping while preserving
authoritative active/stopping flags and truthful unknown/error/unavailable reads.
Never fabricate inactive payloads or infer capture from a pending status read.
Native DTOs/commands/capabilities/ownership/schema/feature scope stay unchanged;
no new state machine or App poller. This fixes the approved start/navigation
semantics, not a new user capability.

After independent coherence review, exact phase-fixer paths in shared-376ef30:
src/components/desktop/contracts.ts, src/components/desktop/DesktopShell.tsx,
tests/callmdDesktopShell.test.mjs, existing Shell implementation report.
No shared bridge/native wire changes. App forwarding and integration tests belong
to later fresh INTEGRATE FIX1 after accepted inputs. Separate from active Live
and root verification leases. Tests preserve assertions: read loading without
actual start never guards; genuine starting after close acknowledgement does;
active/stopping remain guarded until truly inactive. Retain read-error disclosure.

Terra Pauli coherence review accepts this split with these required clarifications:
precedence is authoritative stopping, explicit owner stopping, authoritative
active, explicit owner starting, explicit owner listening/degraded, then no known
capture. The optional idle default is compatibility, never a fabricated native
inactive result. Loading/unavailable/error alone cannot create capture state or
guards. Conversely a read error must not erase real last-known capture flags or
an explicit capture-relevant owner phase; still show its error/unavailable notice.
Tests must cover each precedence branch, including pending stop with active=true,
bootstrap with no phase, actual start, and retained capture on read failure.
Live and phase workers run disjointly; both outputs require independent acceptance
and hashes before fresh IntegrationFIX1 forwards the phase and updates its tests.

## Version Diff / CHANGELOG

0.1.0b ->0.1.1b: add independently reviewed internal phase-handoff correction,
complete precedence and exact split leases; no native interface/new feature.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Freeze bounded phase semantics and independent review clarifications | UNCOMMITTED;base376ef30 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | beta | Record actual App browser failure and bounded replay/visibility prevention | UNCOMMITTED;base376ef30 | Codex orchestrator |
