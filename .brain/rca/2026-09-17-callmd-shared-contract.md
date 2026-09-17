---
version: "0.1.0b"
created_at: "2026-09-17T04:43:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T04:43:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Shared consumer contract fidelity to already approved full P1-B"
  risk: "MEDIUM"
---

# Shared-contract consumer gaps RCA

## Symptom / evidence

Independent Terra Herschel 01a0ac26-3911-70d1-84aa-c383a59b7114 returned FAIL.
Frozen shared contracts source SHA a52b8ddb50204d7e39c1d09208af1786fe494ee9c925c3631dc687b4fe993aae.
Bridge SHA816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe.
The bridge, settlement helper and regression checks passed; failure is limited to
the consumer contract.13contract tests=12PASS/1native-presence expected-red;
summary6/6, jobs17/17, egress8/8, buildPASS, all8 disconnected wrappers rejected.

## Root cause

1. LiveWorkspaceActions.ask and its state copied legacy AskAnswer semantics.
   Selected-B Live Q&A requires RecordingKey/requestId and RecordingAnswer so the
   prompt excludes foreign recording/graph/live-tail evidence as approved.
2. Unknown legacy command failures have no shared safe normalizer even though
   the approved error contract requires LEGACY_COMMAND_FAILED without raw errors.
3. Project[] plus nullable RecordingKey cannot represent a selected project before
   selecting a recording. Controlled active surface/Home action are also absent.

## Why it escaped author checks

Current executable bridge tests cover the native/wire boundary and stale helper;
they do not type-check future UI consumers or exercise legacy-error conversion.
The app still builds because the new UI consumers have not yet been implemented.
Review caught the gaps before admitting those consumers; no runtime leak is claimed.

## Proposed prevention / exact bounded correction

A Luna fixer owns only shared contracts.ts and its implementation report.
Keep tauri.ts and every accepted baseline/test/native interface byte immutable.
Use selected-B ask(selection,question,requestId) -> Promise<RecordingAnswer> and
ReadState<RecordingAnswer>; do not add unused speculative legacy UI APIs.
Add one small exported safe error normalizer: preserve validated ReviewError;
map unknown legacy errors to fixed safe LEGACY_COMMAND_FAILED, no message parsing
or echoing path/token/stderr. Existing legacy wrapper semantics remain unchanged.
Add selectedProjectId:string|null, a small controlled activeSurface and showHome.
No generic new legacy action, new store/listener/provider/dependency/route needed.

Run existing contract/regression/build checks and reproducible inline native-free
normalizer/type probes. Do not edit the frozen contract tests or add unapproved
test paths; UI-owned tests later cover consumers in their approved leases.
Independent bounded shared re-review is mandatory.

Only shared/UI descendants are held. Frozen native commands/DTO/security limits,
accepted contract-test bytes and interface review inputs are unchanged, so the
concurrent backend remains authorized. No new feature or security waiver.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | New evidence-backed consumer-fidelity correction scope | UNCOMMITTED; base376ef30 | Codex orchestrator |

