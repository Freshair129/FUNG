---
version: "0.1.0b"
created_at: "2026-09-17T05:15:25+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T05:15:25+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved live owner lifetime integration; static baseline evidence"
  risk: "MEDIUM"
---

# Live view lifetime versus native capture ownership

## Symptom

The baseline composition cannot satisfy the approved one-owner-for-desktop-lifetime
requirement when the Live view is closed. This is a source-level mismatch, not a
claim of an observed native recording stop or user data loss.

## Evidence

- Controller src/App.tsx:1254 conditionally renders LiveMeetingPanel only while
  liveMeetingOpen. onClose sets that flag false.
- Baseline src/components/LiveMeetingPanel.tsx:138 cleanup marks disposed and
  invokes resolved unlisten functions. The cleanup does not call native stop.
- The same baseline component holds live identity, segments and elapsed state;
  its mount reads status before all four listener subscriptions complete.
- Approved contracts section9 requires a lifetime live owner, subscribe-first
  attach, bounded bootstrap buffer, correct cleanup and separate view visibility.
- Current native/device reproduction is NOT_RUN; no app was launched on user data.

## Root cause

The parent uses the presentation-open flag as the lifetime boundary of the
component owning the native session listeners. Closing the view therefore
tears down that owner while native capture can continue independently.
No native stop is implied by view closure or by listener cleanup.

## Why it escaped detection

The old composition models Live as an open/closed panel, not a persistent owner
with navigable presentation. Build/type checks alone cannot validate mount,
late subscription cleanup, event continuity or native stop completion.
This is a demonstrated design/test gap, not an attribution to a historical CI run.

## Proposed prevention within already approved scope

UI_LIVE owns the bounded subscribe/bootstrap/cleanup/state behavior and an
integration adapter in its existing exact file lease. INTEGRATE owns the
App.tsx mount: exactly one persistent LiveMeetingPanel with independently
controlled visibility, never a second live event store or subscriber.
Shell active-capture navigation warning and reachable global Stop remain required.
Start waits player close acknowledgement. Stop-and-leave waits native
active:false AND stopping:false; stop acknowledgement alone is insufficient.

The approved task-owned tests must exercise mount/cleanup, hidden-view behavior,
event ordering and actual action binding. Browser/source/unit evidence remains
separate from native capture/device verification. No new source lease,
dependency, auth/config change or security waiver is granted here.
Controller does not implement this fix or transfer source.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record source-backed lifetime cause and approved integration prevention | UNCOMMITTED;base376ef30 | Codex orchestrator |
