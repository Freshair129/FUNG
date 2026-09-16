---
version: "0.1.1b"
created_at: "2026-09-17T02:07:55+07:00"
last_update: "2026-09-17T02:18:40+07:00"
status: "candidate"
superseded_by: null
base_sha: "c378af9fac3c00db063948f49f9ee857ebad9126"
branch: "codex/callmd-ui-dag"
attributes:
  doc_type: "implementation-report"
  task: "DOC_CONTRACT"
  result: "DONE_WITH_CONCERNS"
  complexity: "C-3"
  risk: "MEDIUM documentation; HIGH proposed native/backend boundary"
  evidence_state: "static documentation checks only"
---

# DOC_CONTRACT candidate handoff

Result: DONE_WITH_CONCERNS. The bounded specification is complete for independent
document review. Workflow/drafting authority is recorded; A/B selection, product
implementation, CI repair, commit/push/deployment remain unapproved.

## Deliverables and conclusions

- docs/specs/2026-09-17-callmd-desktop-contracts.md — candidate v0.1.1b,
  architecture/DTOs, existing versus NEW commands, Mermaid dependencies,
  recording cursors, scope/error states, Q&A, lifecycle/recovery, native playback,
  shared UI props, exact partitions and named approval choices.
- docs/verification/implementation-reports/2026-09-17-callmd-doc-contract.md —
  this evidence and handoff report.

B is recommended; A remains selectable. B reads existing Genesis schema and uses
Rust-native PCM output through existing cpal/hound dependencies. Tauri carries
control/metadata only: no media URL, HTTP listener, renderer fetch, new decoder,
resampler, provider or schema. PCM16 WAV compatibility is conditional on the output
device accepting the source rate; ordinary 44.1/48 kHz can be unavailable. MP3 imports
are explicitly unsupported by the proposed player. This is a capability boundary,
not a universal-playback claim.

NEW B commands receive structured pair validation/errors. Existing transcript,
summary/job and legacy Q&A APIs retain current arguments and native error semantics;
legacy no-match Q&A remains its successful empty-source AskAnswer, not the NEW B
insufficient_evidence/ReviewError result. Stop acknowledges a stop request; it does
not certify that capture has completed, and active:true/stopping:true remains owned
by capture until active:false/stopping:false. Legacy ask is local-knowledge scope:
only transcript retrieval has a project filter.
Graph search and live-tail injection can cross that scope (meeting_intel.rs:301-385).
NEW recording ask excludes both before prompt construction, preserving exact evidence
IDs instead of claiming that filtering returned citations prevents contamination.

## Evidence and scope

The spec's E1–E14 evidence table cites the approved workflow inputs, existing source
and prior scan. Focused refreshes covered the manifest nodes, current DTOs/listeners,
meeting_intel.rs graph/live tail, Tauri config/capabilities, dependency declarations
and live WAV format. No public documentation was required or consulted.
Prior scan reports and concurrent worker documents were preserved.

The manifest records desktop_playback.rs NEW, the narrow live_meeting.rs admission
guard, existing LiveMeetingPanel.css, InstrumentRail.tsx and
callmdDesktopIntegration.test.mjs as CANDIDATE lease amendments only. lib.rs remains
the exclusive backend owner's C-3 integration boundary. These entries do not authorize
code; feature approval and an exact lease transfer remain required. CSP/capabilities
need no expansion under this design; a later discovered requirement must receive an
exact amendment.
SettingsPanel and schema/job-engine/local-API files are excluded from proposed edits.

## Actual checks and acceptance

| Check | Result | Evidence/limit |
|---|---|---|
| Branch/base | PASS | codex/callmd-ui-dag; HEAD c378af9fac3c00db063948f49f9ee857ebad9126. |
| Current manifest parse | PASS | PowerShell ConvertFrom-Json; 24 nodes, code authority false, DOC_CONTRACT allocation matches the two deliverables. |
| Primary source references | PASS | 16 path/primary line-range references exist and are in bounds; this does not prove all semantics or suffix ranges automatically. |
| Proposed partition paths | PASS | Nine existing paths present; 14 NEW paths absent; no duplicated source/test paths in the proposed partition inventory. |
| Markdown structure | PASS | Paired Mermaid fences; no TBD/TODO/FIXME placeholders. Diagram rendering was NOT_RUN. |
| Git/document write scope | PASS at finalization checkpoint | This turn wrote only the two allocated Markdown documents; no src/tests/CI/manifest/provider/commit/push changes. Other untracked docs remain owned by parallel workers. |
| API/scope review | PASS as static self-check | Existing command names/fields/limits cross-checked with source; legacy error/no-match semantics remain distinct from NEW B structured errors. |
| Capture/playback coherence | PASS with UNKNOWN boundary | Stop request versus actual inactive is explicit; playback is blocked through active/starting/stopping. cpal/hound and PCM16 shape support the proposal, but device acceptance, callback timing and race safety are not proven. |
| Candidate lease alignment | PASS | SEC-1/SEC-2/UI-1 and integration-test extensions are recorded in the manifest as CANDIDATE only; no feature/code authority is claimed. |
| Full DAG analysis | NOT_RUN this wave | No fresh cycle/edge/lease validation of the orchestrator manifest; parsing is not DAG acceptance. |
| Product validation | NOT_RUN | No tests, builds, app launch, native output, device, provider, network operation, CI or deployment. |
| Independent/self approval | NOT_RUN | No self-independent approval, Terra verdict, A/B selection or product implementation approval claimed. |

Documentation AC: concrete A/B recommendation and fallback, exact command/DTO/error
contracts, scoped cursor/Q&A/export rules, bounded playback/disposal, shared props,
single-owner partitions and named choices are delivered. SC: bounded reviewable draft
with evidence and explicit unknowns. Exit: handoff for independent document review.

## Concerns and next decisions

- Boss must choose A or B and accept B's PCM boundary plus SEC-1/SEC-2/UI-1 scope.
- Native device timing, callback/resource bounds, packaged command authorization,
  capture-stop race behavior and legacy custody-root compatibility remain UNKNOWN
  until implementation verification. The current source/dependency evidence supports
  a feasible bounded proposal but is not proof of native playback feasibility.
- No schema change can make project export artifacts recording-attributed without a
  new decision; this proposal keeps the list visibly project-scoped.
- Worker-model provenance (fresh finalization request): gpt-5.6-luna with reasoning
  effort max was explicitly requested for this turn. The available evidence here
  establishes the request, but does not expose an independent runtime model
  attestation; this report therefore does not certify Luna/max execution. History is
  retained: the prior resumed worker reported supplied GPT-6 context and could not
  certify Luna/max, which motivated this fresh explicit request.

## Version diff and changelog

new → 0.1.0b: added the complete candidate contract specification and bounded worker
report. Only the two allocated documents were written by this documentation worker.
0.1.0b → 0.1.1b: recorded the fresh explicit Luna/max request without execution
attestation, retained the resumed-model discrepancy, and added final static coherence
findings for API semantics, capture/playback state, native feasibility and candidate
lease authority.

| Version | Date | Status | Summary | Commit | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | candidate | Final DOC_CONTRACT coherence review; static evidence only; model request recorded without attestation | UNCOMMITTED; base c378af9 | Codex contract worker |
| 0.1.0b | 2026-09-17 | candidate | DOC_CONTRACT handoff with static evidence and pending choices | UNCOMMITTED; base c378af9 | Codex contract worker |
