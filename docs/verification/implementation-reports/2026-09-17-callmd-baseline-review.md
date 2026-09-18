---
version: "0.1.0b"
created_at: "2026-09-17T03:34:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T03:34:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "review-report"
  scope: "Independent baseline preservation review; not new product acceptance"
---

# CallMD baseline independent review

Reviewer Tesla `01a0abea-b7b1-7130-83f4-447ae122aa41`, fresh explicit
`gpt-5.6-terra` / `high`, completed and closed. Runtime model identity is not
independently attested. Reviewer wrote no source/report; controller records
the actual returned result below. Worker Ampere's actual ID is
`01a0abdc-2f27-7ac1-9bc3-3cfde906d533` (worker itself could not see that ID).
Candidate remains uncommitted in its isolated worktree; root transfer is a
separate byte-verified Luna operation, not a commit or main merge.

## Returned verdict

**PASS — baseline preservation gate.**

Hard-gate result: approved repair is correctly scoped. No native/product source changed; only the approved CI/package/test/report surface is dirty and all five frozen SHA-256 values match.

- Drive CI invocation removed: [.github/workflows/ci.yml:36](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/.github/workflows/ci.yml:36)
- Windows Rust custody execution retained: [.github/workflows/ci.yml:57](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/.github/workflows/ci.yml:57), [.github/workflows/ci.yml:89](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/.github/workflows/ci.yml:89)
- Direct, non-alias custody script: [package.json:9](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/package.json:9)
- Reverse CI→package closure plus negative fixtures: [tests/ciCoverage.test.mjs:96](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/tests/ciCoverage.test.mjs:96), [tests/ciCoverage.test.mjs:107](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/tests/ciCoverage.test.mjs:107). Existing package→CI and test-file→script checks remain at lines 71–94.
- Restored Drive-free custody suite has no `drive` reference, no env skip, and preserves typed IPC, token/consumer boundaries, callback/lifecycle, recovery/custody, and a real behavioral Rust matrix: [tests/nativeSessionCustody.test.mjs:11](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/tests/nativeSessionCustody.test.mjs:11), [137](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/tests/nativeSessionCustody.test.mjs:137), [170](C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30/tests/nativeSessionCustody.test.mjs:170).

Historical comparison: 11 of 12 historical tests are preserved/adapted; the omitted test was solely Drive admission/provider fencing. The current behavioral assertion is stronger: it rejects a zero-test Cargo result.

Rerun evidence:

- `npm run test:ci-coverage`: 4/4 pass.
- `npm run test:native-session-custody`: 11/11 pass.
- Direct Cargo matrix: 22/22 `native_behavioral_` pass; not zero-filtered.
- `npm run test:auth`: 8/8 pass.
- `git diff --check`: pass.

Verified candidate/base: `HEAD = 376ef30db13670e4dea816ceff440f44ce73fffd`; no `src` or `src-tauri` diff. Minimal fixes: none.

All frozen hashes matched exactly:

- CI: `33bc2dfa62fe55b8a62f5fa116cbae48cb04b3dc419e15ed86de2cf2d0a58db0`
- package: `02c9cd48c56b230852938f7c62063191b8c0b135feb58585b37925aa80006654`
- CI coverage test: `2839a68b71308e164562a8f133abed45948636b353c4ea69f91cec2b68f11197`
- Custody test: `fe8af82184b1729abf48bae7c8883708def93817e98026701a66407db6a4d8c4`
- Report: `c4ac4fd88e935bc7a261bc67de7bfcb2b69cdecf225f9297156edf506bff7289`

This is baseline-gate evidence only; hosted CI, full native acceptance, product flows, provider readiness, and production remain out of scope/not claimed.

## Acceptance boundary

Controller accepts PASS for this exact baseline preservation repair only.
Native runtime/device/provider/hosted CI and new UI/audio/Q&A functionality are
not accepted by this result. Empty CI build-resource directories are not a
prepared transcription runtime. Frozen reviewed bytes are the digests above;
source transfer must preserve them or invalidate this acceptance.

## Version Diff

- `new → 0.1.0b`: actual independent PASS and bounded evidence.
- Product feature source/version: unchanged at this baseline checkpoint.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record independently reproduced preservation baseline PASS | UNCOMMITTED; base376ef30 | Codex recorder; Tesla reviewer |

