---
version: "0.1.0b"
created_at: "2026-09-17T04:00:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T04:00:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Missing approved NEW-B preimplementation test coverage only"
  risk: "MEDIUM"
---

# Contract-test review coverage RCA

## Symptom and evidence

Independent Terra Avicenna 01a0ac03-20b3-79b2-8c9b-4dc15ac22aae returned FAIL.
The actual runner reproduced10 tests,3pass/7expected implementation-absence reds,
zero harness failures. Candidate test SHA a336637c02099fe5bb59a5146d9ece56252867537616da97ebebd8ce892f8805.
Report SHA3f9d99d93878dcb3f3b0bad42cc91f57d10380592632293b9b9b2e949980a5ac.
Approved AC-11/T03 requires NEW-B full request identity and stale rejections.

## Root cause

The initial tests reused the existing transcript-only settlement adapter, which
does not include projectId or selectionEpoch and uses legacy rejected state.
It therefore cannot encode the NEW-B history/audio/Q&A request identity requirement.
The wrong-pair seam test covered getRecording only, omitting askRecording/openPlayback.
The worker report disclosed the gap; this is incomplete coverage, not a runtime bug
claim or evidence that missing NEW-B implementations currently leak data.

## Why it escaped initial author completion

The worker lacked a named pure helper and deferred that internal interface to review,
leaving the behavior only in approved prose. Existing green transcript checks were
not falsely labeled NEW-B coverage, but delivery was still incomplete for the gate.
Independent preimplementation review detected it before downstream code dispatch.

## Prevention and exact fix scope

A bounded Luna fixer owns only the existing new contract-test file and its report.
Add an expected-red production shared review-load helper target within approved
src/components/desktop/contracts.ts future ownership (no implementation edits now).
Exercise each mismatched project/recording/epoch/request for success AND rejection,
including current rejection becoming ReadState.error. Name the minimal helper shape
in the report so the interface reviewer can freeze it before shared/backend forks.
Add wrong-pair bridge cases for askRecording/openPlayback. Preserve meaningful
assertion-red versus harness failure and all accepted tests. Do not change package/CI.
Correct report timestamp from actual clock and retain prior invalid timestamp provenance.
This implements already approved behaviors; no new feature/security/provider scope.
Acceptance requires independent bounded re-review, then separate interface review.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | New evidence-backed bounded preimplementation coverage RCA | UNCOMMITTED; base376ef30 | Codex orchestrator |

