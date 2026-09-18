---
id: RCA-CALLMD-BASELINE-GATES
version: "0.1.0b"
created_at: "2026-09-17T02:54:13+07:00,Codex,HEAD 376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T02:54:13+07:00,Codex"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "root-cause-analysis"
  scope: "FUNG CI command closure and native session custody gate disposition"
  risk: "HIGH"
  evidence_state: "LOCAL_SOURCE_INSPECTION; TEST_CI_COVERAGE_PASS; CI_AND_NATIVE_RUNTIME_NOT_RUN"
  agent_id: "01a0abc4-596f-7f70-9f4e-c2e71b015eba"
  requested_model: "gpt-5.6-luna"
  reasoning_effort: "max"
  request_authority: "explicit user request"
  provenance: "Controller normalized actual dispatch ID after worker close; runtime model identity is not independently attested"
---

# RCA — CallMD baseline CI closure and native session custody disposition

## Scope and status

This is a bounded, documentation-only baseline RCA for pinned-branch HEAD
`376ef30db13670e4dea816ceff440f44ce73fffd`, based on product source base
`c378af9fac3c00db063948f49f9ee857ebad9126`. Local main is separately reported
at `05ed107a2233e8785b95d2ba7dc282c47aee35a7`; it is not this worker's base and
is not adopted here. This RCA does not authorize product, test, package, CI,
provider, device, runtime, commit, or Drive changes.

The disposition is **REVIEW_REQUIRED**. No self-acceptance or green-CI claim is
made. The controller's pre-existing dirty ledger
`docs/verification/implementation-reports/2026-09-17-callmd-ui-orchestration.md`
was preserved.

## Symptom

On the pinned branch, the current workflow invokes two npm scripts that are not
defined in the current `package.json`:

- `.github/workflows/ci.yml:36` invokes `npm run test:google-drive`.
- `.github/workflows/ci.yml:90` invokes `npm run test:native-session-custody`.

Neither script exists in `package.json`. Therefore the hosted frontend job
will fail at the canceled Drive command, and the Rust job will fail at the
stale native-session command if it reaches that step. The local coverage guard
does not detect this workflow-to-package direction.

The second symptom is a gate-disposition ambiguity: the Google Drive
cancellation removed `tests/nativeSessionCustody.test.mjs` together with
Drive code, but that historical suite also carried non-Drive native broker and
credential-custody checks. Deleting its stale CI step may therefore remove a
required security gate, not merely remove canceled-provider coverage.

## Evidence

### 1. Exact CI/package mismatch

- `package.json` contains the current `test:*` scripts, but no
  `test:google-drive` or `test:native-session-custody` entry (negative lookup;
  current script block begins at `package.json:8`).
- `.github/workflows/ci.yml:31-56` runs the frontend npm suites and retains the
  stale `test:google-drive` call at line 36.
- `.github/workflows/ci.yml:58-90` owns the Windows Rust job and retains the
  stale `test:native-session-custody` call at line 90.
- `npm run test:ci-coverage` was run as the permitted diagnostic: **PASS**,
  exit `0`, `2/2` tests. This is not closure proof: 
  `tests/ciCoverage.test.mjs:18-31` checks that every package `test:*` script
  appears in the workflow, while `tests/ciCoverage.test.mjs:33-43` checks that
  every tracked test file has a package script. It does not parse every
  `npm run` command in the workflow and resolve it back to `package.json`.

### 2. Google Drive cancellation disposition

- `docs/decisions/2026-09-17-google-drive-scope-cancellation.md:18-26`
  cancels Google Drive implementation, test expansion, provider/deployment/UAT
  work, and active contract tests while retaining historical evidence.
- The same decision's follow-up at `:40-45` explicitly requires removal of
  active Drive adapters, UI, commands, Edge functions, and contract tests.
- Commit `a9f9b80` removed the `package.json` entries for both
  `test:google-drive` and `test:native-session-custody`, and deleted
  `tests/googleDriveContract.test.mjs` and
  `tests/nativeSessionCustody.test.mjs`. This was a mixed deletion: the
  native-session file was not Drive-only.
- Current `git ls-files tests` contains `tests/authFlow.test.mjs` and other
  active suites, but no `tests/nativeSessionCustody.test.mjs` or
  `tests/googleDriveContract.test.mjs`.

Disposition of `test:google-drive`: **REMOVED / DO NOT RESTORE**. Restoring
that script, test, provider, or any Drive implementation would contradict the
approved cancellation. Historical Drive specs/reports remain provenance only.

### 3. Native custody coverage that remains

Coverage is **PARTIAL**, not equivalent on the immediate evidence:

- `src-tauri/src/auth_session.rs:1-5` retains the native broker boundary: the
  module documents OS-keyring-only refresh credentials and zeroizing native
  custody for access/callback/code/verifier material.
- `src-tauri/src/auth_session.rs:3003-3877` retains the Rust
  `native_behavioral_*` tests for current account lifecycle, callback, refresh,
  cleanup, generation, marker, registry, and recovery behavior. Source
  inspection counted 22 such functions; this worker did not execute Cargo.
- `tests/authFlow.test.mjs:45-83` remains an active npm/CI suite with eight
  tests covering Desktop/browser-Mobile separation, native PKCE/token-boundary
  assertions, callback parsing, and enrollment proof. It is narrower than the
  deleted custody suite.
- The historical `tests/nativeSessionCustody.test.mjs` contained 12 tests. In
  addition to Drive-specific assertions, it checked closed typed IPC, removal
  of secret-bearing aliases, Desktop consumer token absence, registered
  production-entrypoint convergence, typed recovery/custody source invariants,
  and it spawned the Rust `native_behavioral_` matrix. Those checks are not all
  present in `tests/authFlow.test.mjs`.
- The native security specification still names
  `tests/nativeSessionCustody.test.mjs` plus the native behavioral suite as the
  required evidence at
  `docs/specs/2026-08-24-native-session-broker-amendment.md:294-310`.
  That specification predates the Drive cancellation, so it proves the
  security intent, not a current accepted replacement.

Conclusion: current Rust and auth-flow coverage proves that some native
session custody coverage remains, but the immediate references do not prove
equivalence to the deleted standalone gate. The custody disposition is
**UNKNOWN — do not delete/waive the gate by inference**.

### 4. Baseline authority and execution status

- The CallMD workflow plan explicitly records the two undefined scripts and
  says not to silently fix CI or restore canceled Drive code at
  `docs/plans/2026-09-17-callmd-ui-luna-max-dag-workflow.md:292-305`.
- The feature approval boundary separately says not to delete the native
  custody gate without replacement evidence at
  `docs/plans/2026-09-17-callmd-desktop-feature-approval.md:82-84`.
- **PASS:** permitted local `npm run test:ci-coverage` diagnostic, 2/2.
- **NOT_RUN:** hosted GitHub CI, `npm run test:native-session-custody`, any
  missing stale command, Cargo tests, build, native app, provider, device, and
  production checks.
- **NOT_RUN / NOT CLAIMED:** no source, test, package, or CI file was edited by
  this RCA worker; no Drive code was restored.

### 5. Existing local-main repair candidate

- **EXISTING_CANDIDATE — controller-observed via read-only git diff:** local
  main is at
  `05ed107a2233e8785b95d2ba7dc282c47aee35a7`, while this pinned branch remains
  at `376ef30db13670e4dea816ceff440f44ce73fffd`.
- The controller-supplied `git diff c378af9..05ed107a --
  .github/workflows/ci.yml` and main history identify `d10bbf8` (`Fix stale CI
  suite references`) as an existing candidate that removes **both** stale
  invocations: the Drive step and the final setup-node/
  `test:native-session-custody` step from `.github/workflows/ci.yml`.
- This candidate resolves the **pinned-branch command-closure symptom only if
  its exact workflow change is separately adopted**. It is not present on the
  pinned branch, and this worker did not merge, cherry-pick, or change the
  base.
- `d10bbf8` supplies no proof that the deleted standalone native custody gate
  is equivalent to the retained Rust/auth-flow coverage. Therefore main
  adoption and custody-equivalence remain **UNKNOWN / NOT ESTABLISHED in this
  bounded review**. The candidate must not be described as security-gate
  closure or as authorization to waive custody; this does not imply that no
  CI cleanup candidate exists.
- The other main commits named by the controller (`9e3a387` and `0e6e213`)
  are context only; this RCA does not inspect or adopt them.

## Root Cause

Commit `a9f9b80` correctly removed canceled Google Drive implementation and its
active test/script, but it also removed the mixed native-session custody test
and script without first separating the non-Drive security assertions. On the
pinned branch, the workflow retained both invocations, leaving stale CI
commands. The existing `test:ci-coverage` guard only proves
package-to-workflow and test-file-to-script coverage; it cannot detect a
workflow command whose package script was deleted. Local main later contains
`d10bbf8`, an existing workflow cleanup candidate, but that commit removes the
native step without proving custody equivalence.

The resulting failure is therefore two coupled but distinct defects:

1. **Confirmed closure defect:** stale `test:google-drive` and
   `test:native-session-custody` workflow commands have no package-script
   targets.
2. **Unresolved security-gate disposition:** removing the native session test
   with Drive did not establish that retained Rust/auth-flow checks replace its
   standalone static and behavioral assertions.

## Why the issue escaped detection

- The Drive removal was treated as a file/script bundle, although
  `tests/nativeSessionCustody.test.mjs` mixed Drive assertions with broader
  native broker custody checks.
- The coverage test is intentionally one-directional: it enumerates scripts
  from `package.json`, so deleted scripts disappear from its expected set and
  stale workflow calls remain invisible.
- The local diagnostic passed 2/2, which can look like closure if its exact
  assertion direction is not read.
- Hosted CI and the missing commands were not run in this bounded inspection;
  the documented status remains `NOT_RUN`, not green.

## Proposed minimal exact-path repair

This section is a proposal only and requires a separate Boss approval before
any code/test/CI change or baseline adoption.

1. Treat local-main `d10bbf8` as the existing **workflow repair candidate** for
   `.github/workflows/ci.yml`: it removes the stale Drive invocation and the
   stale native-session invocation. Adoption is a separate Boss decision; this
   worker must not merge, cherry-pick, or change the pinned base. No Drive
   command, test, provider, migration, or deployment path may be restored.
2. Before accepting removal of the native CI step as a security disposition,
   resolve the UNKNOWN custody-equivalence question. The minimal preservation
   scope is a Drive-free gate at `tests/nativeSessionCustody.test.mjs` plus its
   `package.json` script, or an explicitly reviewed replacement with the same
   non-Drive assertions. The gate must cover closed typed broker allowlist, no
   secret-bearing legacy aliases or Desktop token DTOs, current registered
   account-entrypoint and recovery invariants, and the current Rust
   `native_behavioral_` matrix. It must not reference or resurrect Drive.
3. If the exact existing main candidate is adopted **and** the custody gate is
   intentionally retired, Boss must explicitly authorize that waiver and name
   the replacement evidence. The 2/2 coverage diagnostic and `d10bbf8` commit
   message are insufficient proof by themselves.
4. `tests/ciCoverage.test.mjs`: add the reverse-direction assertion that every
   `npm run test:*` command present in `.github/workflows/ci.yml` resolves to a
   defined package script. This closes the specific escape path found here;
   it is not a substitute for the custody assertions.
5. Do not alias `test:native-session-custody` to `test:auth` or bare `cargo
   test`, and do not weaken/remove static security assertions without the
   explicit waiver above.

For the preservation branch of the decision, the exact test shape is:

- `tests/nativeSessionCustody.test.mjs`: replace the deleted mixed suite with
   a **Drive-free** native-session custody gate covering the retained
   non-Drive contract: closed typed broker allowlist, no secret-bearing legacy
   aliases or Desktop token DTOs, current registered account-entrypoint and
   recovery invariants, and the current Rust `native_behavioral_` matrix. The
   replacement must not reference or resurrect Drive.
- `package.json`: add back exactly one script,
   `test:native-session-custody`, targeting that Drive-free test file. Do not
   add `test:google-drive`.

No safe equivalence has been established for deletion of the native CI step,
an alias to `test:auth`, a bare `cargo test`, or weakening/removing static
security assertions. Any such disposition requires the explicit security-gate
waiver and exact replacement evidence above; it must not be inferred from the
2/2 coverage diagnostic or from `d10bbf8`.

## Proposed prevention

- Keep both directions of CI command closure in `tests/ciCoverage.test.mjs`.
- Treat cancellation cleanup as path-by-path: classify each removed test as
  provider-specific, shared security coverage, or historical-only before
  deleting its gate.
- Keep native custody coverage on the toolchain-owning Windows Rust runner and
  keep source/static security assertions separate from product/runtime claims.
- Require the baseline-only review to record exact changed paths and a
  `PASS`/`FAIL`/`UNKNOWN` disposition for the custody gate before UI/history
  implementation dispatch.

## Acceptance / exit criteria for the proposed baseline task

- The Boss-approved baseline disposition names whether the existing main
  `d10bbf8` workflow repair is adopted on the pinned line; this RCA does not
  adopt it.
- `test:google-drive` is absent from adopted active CI and no Drive
  implementation is restored.
- Either a Drive-free `test:native-session-custody` script/test path is
  retained and invoked on the Windows Rust runner with approved non-Drive
  custody assertions, or Boss has explicitly waived that gate with exact
  replacement evidence. This RCA does neither.
- The strengthened coverage guard fails for both package-to-workflow and
  workflow-to-package mismatches.
- The exact baseline-only diff receives independent review; only then may a
  later run report CI/test results. This RCA itself does not satisfy those
  criteria.

## Version Diff

- `new -> 0.1.0b`: documented the confirmed missing npm/CI command closure,
  the Google Drive cancellation disposition, the partial-but-non-equivalent
  retained native custody coverage, and the exact baseline-only repair scope.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | candidate | Bounded RCA for stale CI scripts and unresolved native session custody gate disposition after Google Drive cancellation. | working-tree; base 376ef30 | Codex |

## Boss approval question

Boss, please decide and approve the exact baseline-only disposition: adopt the
existing local-main `d10bbf8` workflow cleanup on the pinned line with no Drive
resurrection, while retaining/replacing the native custody gate at
`tests/nativeSessionCustody.test.mjs` + `package.json` (or explicitly waiving
it with named replacement evidence) and adding the reverse workflow-command
closure assertion in `tests/ciCoverage.test.mjs`; no product changes,
cherry-pick, or weakened security assertions are authorized by this RCA.
