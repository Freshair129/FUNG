---
version: "0.1.1b"
created_at: "2026-09-17T04:05:15+07:00,CONTRACT_TESTS worker"
last_update: "2026-09-17T04:07:13+07:00,CONTRACT_TESTS worker"
status: "beta"
superseded_by: null
base_sha: "376ef30db13670e4dea816ceff440f44ce73fffd"
attributes:
  domain: "FUNG desktop"
  doc_type: "implementation-report"
  scope: "Executable Node contract tests only; no backend/native implementation"
  risk: "MEDIUM"
---

# CallMD desktop contract-test corrective handoff

## Result and exact scope

- Result: `DONE_WITH_CONCERNS`; ready for independent bounded Terra `CONTRACT_TEST_REVIEW`.
- Base/worktree: `376ef30db13670e4dea816ceff440f44ce73fffd` / `C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30`.
- Only new files written: `tests/callmdDesktopContracts.test.mjs` and this report.
- No implementation, Rust, package, lockfile, CI, commit, staging, push, dependency install, device, native, or production action.
- Existing baseline files/source/package/CI/lock bytes were treated as immutable accepted dependencies. `git diff --check`: PASS.

## Frozen inputs

- Contracts v0.1.2b: `e34b3727184ee99da3e689573163efd19f79f6bf81655ffd7b84175dc840ae7c`.
- Acceptance v0.1.3b: `8686bf25f8aa9accf170e4d41409dab777246178e942178dffee7ee656a0e613c`.
- UI v0.1.1b: `191421d8b83275a441dbf8cd6de36476e9e7ec9e4e1a12e531f3fb074cd5c533`.
- Accepted baseline review: `43f92ddc50ce6950c85fbb8ef9c2b33c708572d10b6a8263efb50b48013df6ae`.
- Accepted amendment review: `9281d93a695fc17a716a2619ce247e491b4be65554c753bfccc17eb37d8971d4`.

## Executed evidence

- Exact command after this corrective pass: `node --test --experimental-strip-types tests/callmdDesktopContracts.test.mjs`.
- Corrective additions: production-helper-gated review-load settlement matrix; independent wrong-pair `askRecording` and `openPlayback` reds.
- Exit `1`; `13` tests, `3` pass, `10` fail, `0` skipped, `0` cancelled.
- Failure classification: `10` expected-red implementation absences; `0` real harness/import/setup failures.
- Test file SHA-256: `d6d53a7531e24492ce2ceddf76e58def018602a0cdca33754c1f8bf3e2369ee6`.
- Every failed test reached an assertion containing `EXPECTED_RED_MISSING_IMPLEMENTATION`; no loader/setup red was counted.
- PASS adapter evidence: existing transcript cap remains ready data while storage failure remains an error; existing stale transcript success and rejection cannot replace the current request; existing summaries require `(projectId, recordingId)` while exports remain project-scoped.
- Expected-red bridge evidence: eight approved NEW exports, exact recording/history/Q&A wire DTOs, exact playback handle/epoch/channel wire, pair/error propagation, list resource/cursor/unavailable distinctions, scoped-Q&A outcomes, and wrong-pair `askRecording`/`openPlayback` cases.
- Expected-red shared-contract evidence: `src/components/desktop/contracts.ts` is checked for the named production `settleReviewLoad` export before import. The test never defines a substitute helper.
- Expected-red native-presence evidence is separate: `recording_review.rs`, `desktop_playback.rs`, and all eight NEW native command names are absent at this base.

## AC mapping and evidence tier

- AC-03/04/06/07/08/09/13: NEW pair, history, playback, summary/export boundary, DTO, and typed-error assertions; `EXPECTED_RED_MISSING_IMPLEMENTATION` pending shared/native implementation.
- AC-05: recording-scoped Q&A answer/insufficient-evidence/provider/resource/model-error DTOs, same-pair sources, and no graph/live-tail wire fields; expected red pending wrapper/native implementation.
- AC-11: production transcript adapter stale-success and stale-rejection behavior passes. NEW B selection/audio/Q&A settlement is now bounded against one named production helper, but remains expected-red until that helper exists.
- No native/device/runtime/packaged/hosted-CI/production PASS is claimed. CI/package wiring is intentionally unwired and reserved for `INTEGRATE`.

## Minimal interface-review note

The docs do not name a pure selection helper. BACKEND_INTERFACE_REVIEW should freeze this exact minimal production surface under `src/components/desktop/contracts.ts`:

```ts
type ReviewRequestIdentity = { projectId: string; recordingId: string; selectionEpoch: number; requestId: string };
type ReadState<T> = { status: "idle" | "loading" | "ready" | "error" | "unavailable"; identity: ReviewRequestIdentity | null; data: T | null; error: ReviewError | null };
type ReviewLoadSettlement<T> = { identity: ReviewRequestIdentity; outcome: { status: "fulfilled"; data: T } | { status: "rejected"; error: ReviewError } };
export function settleReviewLoad<T>(current: ReadState<T>, settlement: ReviewLoadSettlement<T>): ReadState<T>;
```

Mismatched project, recording, selection epoch, or request ID returns the same current state for success and rejection; matching success is `ReadState.ready` with `data`, and matching rejection is `ReadState.error` with `error`. No helper was invented or added in this lane.

The docs freeze DTOs and command names but not TypeScript positional versus `RecordingKey` object syntax. These tests use the existing `src/tauri.ts` convention `(projectId, recordingId, ...)`; interface review should confirm that choice before shared-bridge implementation.

## Unmet gates and handoff

- Pending: independent contract-test review, backend interface review, shared bridge, backend/native/security review, UI reviews, integration, CI wiring, and all native/device/runtime gates.
- Lease handoff: worker writes are complete and released for read-only Terra review; no local `.agents` lease file exists to mutate. No commit/staging/push performed.

## Version Diff / CHANGELOG

- `0.1.0b → 0.1.1b`: added bounded production-helper settlement reds, independent wrong-pair ask/playback reds, exact interface-review shape, and corrected clock provenance; product implementation unchanged.
- The prior `05:05` header was an unverified future-time claim; it is retained only as prior-report provenance, not as execution evidence.
- No runtime/native/packaged/hosted-CI/production green claim is made.

| Version | Date | Status | Summary | Commit | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Corrective bounded coverage: helper settlement matrix plus ask/playback wrong-pair reds; clock provenance corrected | UNCOMMITTED; base 376ef30 | CONTRACT_TESTS |
| 0.1.0b | 2026-09-17 | beta | Added P1-B executable contract-test lane; 3 adapter passes and 7 implementation-absence reds recorded | UNCOMMITTED; base 376ef30 | CONTRACT_TESTS |
