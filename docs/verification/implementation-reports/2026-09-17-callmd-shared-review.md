---
version: "0.1.0b"
created_at: "2026-09-17T05:00:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T05:00:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "review-report"
  scope: "Independent shared contract FIX1 acceptance; no native/runtime proof"
---

# Shared contract independent review

Controller records actual read-only reviews, not a self-review.
Base376ef30; uncommitted output in output/callmd-worktrees/shared-376ef30.
Workers Ohm and FIX1 Locke used requested gpt-5.6-luna/max and are closed.

## Cycle 1 — FAIL

Terra Herschel 01a0ac26-3911-70d1-84aa-c383a59b7114, requested
gpt-5.6-terra/high, found three consumer gaps: B Live used legacy AskAnswer;
safe legacy-error normalization absent; selected-project/surface/Home shell
controls absent. Bridge and stale-settlement helper behavior passed.
RCA: .brain/rca/2026-09-17-callmd-shared-contract.md.
Initial contracts SHA a52b8ddb50204d7e39c1d09208af1786fe494ee9c925c3631dc687b4fe993aae.
It verified contracts12PASS/1native-presence expected-red, summary6/6,
jobs17/17, egress8/8, buildPASS and8/8 disconnected-wrapper typed failures.

## Cycle 2 — PASS after documentation reconciliation

Actual reviewer Meitner 01a0ac36-01f5-7fa1-8415-cf94aeb5539e, fresh explicit
gpt-5.6-terra/high, read-only. Runtime model identity not independently attested.
Initially WARN: all three FIX1 code corrections adequate, but section10 of
controller specification still used older shorthand. Controller edited only
that documentation plus metadata/changelog, v0.1.2b to0.1.3b.
Same live reviewer session confirmed PASS and no native invalidation; no
redundant test rerun after documentation-only change. Reviewer is closed.

- Live ask: RecordingKey/question/requestId to Promise<RecordingAnswer>,
  state ReadState<RecordingAnswer>.
- normalizeReviewError preserves structurally valid ReviewError; unknown
  legacy failures produce fixed safe LEGACY_COMMAND_FAILED, no parsing/echo.
- Shell independent selectedProjectId, nullable recording selection,
  controlled home/live/review and showHome; lifecycle ownership unchanged.
- Final spec SHA: 6f2b55293430048bfff08cc7b5914c06d85823c37a737d442fdb346f0ea55921.
  Native command/DTO/security/test sections unchanged from accepted interface.

Reproduced exact checks:

| Check | Result |
|---|---|
| node --test --experimental-strip-types tests/callmdDesktopContracts.test.mjs | 13 total,12PASS,1approved separate native-presence red; zero harness failures |
| npm run build | PASS,tsc and Vite |
| Inline normalizer probes from FIX1 report | 5/5PASS; valid identity + Error/string/null/malformed safe fallbacks, no raw sentinels echoed |

Frozen source/artifact hashes:

| Path in shared worktree | SHA-256 |
|---|---|
| src/components/desktop/contracts.ts | 7f2bd4191f804fca5788684a5891471fda70d713110c5e75b3b5f67870cbae81 |
| src/tauri.ts | 816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe |
| tests/callmdDesktopContracts.test.mjs | d6d53a7531e24492ce2ceddf76e58def018602a0cdca33754c1f8bf3e2369ee6 |
| docs/verification/implementation-reports/2026-09-17-callmd-contract-tests.md | e067b84d2b38540efd22d17898c671450306ffa1f305b7c2053a6e819ee84efd |
| docs/verification/implementation-reports/2026-09-17-callmd-shared-contract.md | f8cb9c003d931868229d3ac5d07fbfb9234c403563337b35aa6458744cdbf229 |

## Controller acceptance

PASS accepted for shared/UI dependency dispatch, identical snapshots per lane.
Native-presence red is not waived: backend implementation/review joins before
integration. No UI interaction, native/device/provider/hosted-CI/production
acceptance follows. Contract test/bridge bytes unchanged; backend continues.
Three active Luna cap; exact disjoint leases; no commit/release authority.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record FAIL, bounded FIX1, independent PASS and exact UI dependency snapshot | UNCOMMITTED;base376ef30 | Codex recorder |
