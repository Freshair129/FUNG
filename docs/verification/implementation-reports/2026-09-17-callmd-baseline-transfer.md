---
version: "0.1.0b"
created_at: "2026-09-17T03:43:09+07:00,Luna max"
last_update: "2026-09-17T03:43:09+07:00,Luna max"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "transfer-report"
  scope: "Bounded mechanical BASELINE source transfer"
---
# CallMD BASELINE transfer

Transfer: approved cover0.2.0b fullB/baseline; source integration under Luna ownership; no new implementation/design.
Source: C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30
Destination: C:/Users/pc/.codex/worktrees/9000/fung
Actual base: 376ef30db13670e4dea816ceff440f44ce73fffd
Authority: independent Terra Tesla baseline review PASS; Boss latest approval.

## Hash identity

All five source and destination raw SHA-256 values matched exactly; all were LF-only.
.github/workflows/ci.yml — 33bc2dfa62fe55b8a62f5fa116cbae48cb04b3dc419e15ed86de2cf2d0a58db0
package.json — 02c9cd48c56b230852938f7c62063191b8c0b135feb58585b37925aa80006654
tests/ciCoverage.test.mjs — 2839a68b71308e164562a8f133abed45948636b353c4ea69f91cec2b68f11197
tests/nativeSessionCustody.test.mjs — fe8af82184b1729abf48bae7c8883708def93817e98026701a66407db6a4d8c4
docs/verification/implementation-reports/2026-09-17-callmd-baseline.md — c4ac4fd88e935bc7a261bc67de7bfcb2b69cdecf225f9297156edf506bff7289

## Verification

- npm run test:ci-coverage: PASS — 4/4.
- npm run test:auth: PASS — 8/8.
- git diff --check: PASS.
- Rust/full/native suites, build, runtime, and hosted CI: NOT RUN by instruction.

## Controls

- Applied only the five frozen source paths plus this report; source worktree was not written.
- Existing destination DOCS/report dirt was preserved; no other controller product path was changed.
- No staging, commit, merge, cherry-pick, push, native recompilation, or runtime execution.

## Version Diff

- new → 0.1.0b: byte-verified baseline transfer and two targeted root-test results; no product-source implementation claim.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Completed the approved five-file BASELINE transfer with exact hash identity and bounded root verification. | UNCOMMITTED; base 376ef30 | Luna max |
