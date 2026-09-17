---
version: "0.1.0b"
created_at: "2026-09-17T07:15:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T07:15:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Final bounded native regression-evidence correction; no new feature"
  risk: "MEDIUM tests and minimal production-used test seams"
---

# Native FIX3: missing production-path evidence

## Symptom and evidence

Mill01a0acb1-9490-7ce1-8cd8-3ecc1b00b887 (soleCargo/Q&A) and Arendt
01a0acb1-955c-7762-a7b7-1bcf5b991252 (staticplayback) independently returned
FAIL/CHANGES_REQUESTED on frozen FIX2. No new functional defect was demonstrated.
Actual async open/close, owned cleanup, disposal generation, shared deadline,
pair-scoped outbound prompt and source results conform statically.

1. meeting_intel.rs:835 returns insufficient evidence before any provider work,
   but test:1876 selects evidence and observes one call. Test:1996 constructs a
   DTO directly. No test calls meeting_ask_recording_checked for valid empty or
   nonmatching evidence while asserting the actual provider stub receives zero.
2. desktop_playback.rs tests:2545,2570,2602,2640 invoke helper channels/reaper or
   install/shutdown directly. None invokes open_playback_owned or the async
   production open/close path at1922/1992. Thus delayed preparation/readiness,
   disposal during open and active close are not covered through their real
   orchestration path, despite correct individual helpers.

## Root cause / why it escaped

FIX2 tests compose independent helpers outside the application path instead of
driving the actual owned pipeline. They prove helper behavior but cannot catch
miswiring between task preparation, dispatch, installation and cleanup. The
no-evidence assertion likewise bypasses the provider boundary it should prove.
Earlier wording about manager-path tests was interpreted too narrowly. Build
and existing focused tests therefore passed without closing these exact gates.

## Bounded FIX3 and acceptance

Fresh Luna/max may edit only desktop_playback.rs, meeting_intel.rs and the
existing backend implementation report, in absolute baseline-376ef30 paths.
Preserve current conforming behavior/interfaces and other three Rust files.
Minimal production-used seams or cfg(test) hooks may substitute preparation,
readiness and worker IO with deterministic local stubs; no copied test-only
open/close orchestration, real device/provider/network, dependency or config.

Tests MUST invoke open_playback_owned (literal production function) and the
same async dispatch/close implementation used by public commands. If an async
body must be extracted to a small shared function to avoid constructing a real
WebviewWindow, the public command must call that function unchanged and tests
must call it too. Existing helper tests alone cannot close this finding.
Drive delayed preparation/readiness, total-deadline expiry, destroy during open,
late readiness and an active delayed close. Assert no post-disposal install,
dispatch remains responsive, close does not acknowledge/release admission before
actual quiescence, and eventual cleanup owns/joins its workers. Keep tests
deterministic and bounded; test-only timeout control must not change production10s.

For Q&A use the existing local capture stub and actual checked pipeline with a
valid pair and empty/nonmatching evidence. Assert insufficient_evidence, empty
sources, model=null and zero provider captures, with no broader fallback. Retain
the adversarial outbound/citation capture test and all prior tests.

Report0.1.3b with an exact test-to-production-call map, hashes and commands.
Run fmt/check/focused/fullRust; separately retain six missingWhisper failures
and untouched strictClippy auth/backup failures. This is FIX3, the final allowed
unsuccessful fix cycle before escalation to Boss; initial review is not counted.
Independent review required. No source integration or native/runtime acceptance.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record actual-pipeline evidence gaps and exact final bounded tests | UNCOMMITTED;base376ef30 | Codex orchestrator |
