---
version: "0.1.1b"
created_at: "2026-09-17T13:18:15+07:00,Descartes Terra reviewer,053d2c5024033d4eed0ec6bd057ce45b0ef112d1"
last_update: "2026-09-17T13:27:22+07:00,Descartes Terra reviewer"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  scope: "Independent read-only review of approved PR59 CI workflow repair"
  complexity: "C-2"
  risk: "MEDIUM: CI integration semantics; no product behavior change reviewed"
---

# PR59 CI repair — independent Terra review

## Scoped verdict

**PASS — frozen workflow repair is correct within its approved scope.**

**WARN — report-gate reconciliation remains required before publication:** this
reviewer's byte-exact three-way fixture produced a different result hash from
Faraday's report, although both reports retain the required custody ordering
and the frozen tracked workflow hash remains exact.

The prospective repaired Windows Rust sequence is resource roots → Node 22 →
`test:native-session-custody` → strict Clippy → full Cargo test. This restores
the custody matrix that current `main` removed during the otherwise
conflict-free synthetic merge, without removing a CallMD suite, test script,
strict warning policy, or Rust regression step.

This is an implementation-semantics verdict only. It is **not** a hosted-CI,
merge, deployment, native-runtime, device, or production acceptance result.

## Authority and evidence boundary

- Reviewed head: `053d2c5024033d4eed0ec6bd057ce45b0ef112d1`
  (`codex/callmd-ui-dag`).
- Reviewed `main`: `05ed107a2233e8785b95d2ba7dc282c47aee35a7`.
- Reviewed common base: `c378af9fac3c00db063948f49f9ee857ebad9126`.
- Approved workflow preimage: `3755ef4ce6c9e5adb46ee7c9e746dfdf0df4d21b2785c87a6e1d83daa71d861a`.
- Frozen repaired workflow SHA-256: `52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e`
  (matched twice during this review).
- Candidate playback RCA SHA-256:
  `2d6c1511db07f1ed73293ce72b0894b44c98f58768db0f846500db7b37ef948e`
  (matched). Its status remains `CANDIDATE / ROOT CAUSE UNKNOWN / NO CODE FIX
  AUTHORIZED`; it is not part of this workflow repair.
- Faraday verification report SHA-256:
  `8f2f1dc6997898c92220d13103b3bedfaf95b766d2d2957c92690cdb69e05e6e`
  (matched after the worker closed).

No source, test, workflow, package, Git, dependency, native-app, credential,
or user-data write was performed by this reviewer. This leased report and a
task-owned, subsequently removed prospective-merge fixture are the only
reviewer-generated artifacts.

## Exact diff and merge reconstruction

`git merge-tree` of the stated base, head, and main exited `0` (no merge
conflict). Its un-repaired result deletes the original tail Node/custody block
because `main` deleted that block. The frozen worktree diff moves that same
block before the existing strict checks; it adds no test command and deletes no
test command.

At [ci.yml](C:/Users/pc/.codex/worktrees/9000/fung/.github/workflows/ci.yml:81),
the resource roots are created before Node setup at line 82 and custody
execution at line 88. Strict Clippy and the full Cargo suite remain at lines
96 and 97. All five CallMD frontend commands remain at lines 28-32.

The custody test itself launches `cargo test --manifest-path
src-tauri/Cargo.toml native_behavioral_ -- --nocapture`
([nativeSessionCustody.test.mjs](C:/Users/pc/.codex/worktrees/9000/fung/tests/nativeSessionCustody.test.mjs:137)). It therefore needs Rust available, the
repository checkout as its working directory, and the empty bundle-resource
roots before its Cargo-backed checks. The restored Windows ordering supplies
those prerequisites.

## Inventory and static validation

| Check | Result |
|---|---|
| Frozen workflow SHA-256 | PASS — exact approved `523571…019e` |
| Candidate playback RCA SHA-256 | PASS — exact frozen `2d6c15…948e` |
| Three-way merge reconstruction | PASS — exact fixture `git merge-file prospective-merged.yml base.yml head-frozen.yml`: exit `0`, no markers |
| Prospective workflow result | PASS — Git blob `f9dee9eb19ed72f2dc7b3eb470e11dcc244a63d6`; SHA-256 `52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e` |
| Prospective CI command inventory | PASS — exact `ciCoverage.test.mjs` executed from the fixture: 4 passed, 0 failed, exit `0` |
| Inventory assertions | PASS — all `test:` package scripts wired; every suite file scripted; every CI `npm run` resolves; quoted/comment parsing fixture passes |
| Workflow whitespace/errors | PASS — `git diff --check -- .github/workflows/ci.yml` produced no findings |
| Scope diff | PASS — worktree diff is `.github/workflows/ci.yml` only, 10 additions / 7 deletions; it relocates the existing Node/custody block |
| Full Rust build/test | NOT RUN — intentionally outside this bounded workflow review |
| Hosted frontend/Rust execution | NOT RUN — commit/push are authorized after review but not yet performed |

## Actual prospective-merge reproduction

The input files were serialized as task-owned temporary files, not taken from
an inferred merge result. Their raw Git blob hashes were verified before the
merge:

| Fixture input | Source | Verified Git blob |
|---|---|---|
| `base.yml` | `c378af9fac3c00db063948f49f9ee857ebad9126:.github/workflows/ci.yml` | `2a0dbdac49ed7d58026c5ea19aa3bb2b420e7ee1` |
| `main.yml` | `05ed107a2233e8785b95d2ba7dc282c47aee35a7:.github/workflows/ci.yml` | `d428e20557b4ec65a2ecec0bffccce85608e65af` |
| `head-frozen.yml` | frozen worktree `.github/workflows/ci.yml` | `f9dee9eb19ed72f2dc7b3eb470e11dcc244a63d6` |

`main.yml` was byte-copied to `prospective-merged.yml`; then this command ran
from the repository root:

```powershell
git merge-file .tmp\terra-pr59-merge-review-20260917-1318\prospective-merged.yml `
  .tmp\terra-pr59-merge-review-20260917-1318\base.yml `
  .tmp\terra-pr59-merge-review-20260917-1318\head-frozen.yml
```

It exited `0`, produced zero conflict markers, and yielded the frozen postimage
bytes above. The fixture then received byte-copied `package.json` and all 27
test files; running the repository's unmodified
`tests/ciCoverage.test.mjs` with the fixture as its working directory passed
all four inventory assertions. This is an actual test of the prospective
workflow fixture, not an inference from `git merge-tree` or the tracked
workflow.

### Faraday-result hash difference

Faraday reports prospective SHA-256
`c2880d42f126509c47e0fd34687a26019f181cd903e4ae3f7df402af9962ffd4` and
Git blob `f92c0fd55c247611943a2236bdee3acaf8cb9c1e`. Those do not match this
review's byte-exact result. The common raw, LF-to-CRLF, UTF-8-BOM, and UTF-16LE
serializations of this review result also do not match Faraday's two hashes.
Faraday's excerpt does not specify how its `base.yml` and `main.yml` were
written, so the difference cannot be attributed conclusively to newline or
encoding normalization. It is a report-provenance discrepancy, not evidence of
a tracked workflow/source correctness failure: this review's three input blobs
match the exact pinned Git/worktree artifacts and its result equals the frozen
workflow postimage.

## Finding — duplicated resource step (non-blocking)

**WARN, non-blocking:** the resource-root `New-Item -ItemType Directory -Force
.venv-whisper, runtime` occurs at lines 81 and 92. It is idempotent and creates
only the ignored empty resource directories already required by `tauri-build`;
it grants no additional permission, changes no custody policy, and does not
mask a failure. This review makes no claim about whether its removal is in or
out of scope; no correctness reason to remove it was found.

## Publication blockers

1. **Faraday/Terra prospective-result hashes differ:** reconcile or explicitly
   disposition `c2880d…ffd4` / `f92c0f…c1e` against this report's exact-input
   `523571…019e` / `f9dee…c6` evidence before exact publication. This is a
   report-gate/provenance WARN, not authority to change implementation.
2. **Hosted proof is NOT_RUN:** commit and push are user-authorized after the
   review, but have not yet been performed. Fresh frontend and Windows Rust
   jobs, including actual custody execution, must pass before CI can be claimed
   complete.
3. **Playback remains separately blocking:** the frozen candidate RCA reports
   local `desktop_playback` module `20 passed`, hosted synthetic merge `4`
   failures, and `ROOT_CAUSE_UNKNOWN`. Its alternate-path proposal is not
   approved and must not be included in the workflow-repair commit.

No source-fix permission, CI completion, release promotion, deployment, or
playback conclusion is inferred by this review.

## Version diff / CHANGELOG

New -> `0.1.0b`: independent frozen-diff, prerequisites, merge-reconstruction,
and command-inventory review.
`0.1.0b` -> `0.1.1b`: incorporates the finalized Faraday report, executes the
inventory against a byte-verified prospective-merge fixture, corrects
commit/push authority wording, and records the unresolved fixture-hash
provenance discrepancy.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | candidate | Actual prospective fixture inventory passes; Faraday/Terra result hashes need publication-gate disposition | UNCOMMITTED; inspected 053d2c5 | Descartes Terra reviewer |
| 0.1.0b | 2026-09-17 | candidate | Scoped workflow repair passes; hosted proof and Faraday report remain pending | UNCOMMITTED; inspected 053d2c5 | Descartes Terra reviewer |
