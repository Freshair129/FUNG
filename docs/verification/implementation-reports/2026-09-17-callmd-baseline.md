---
version: "0.1.0b"
created_at: "2026-09-17T03:29:29.0807262+07:00,Luna max,UNCOMMITTED base 376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T03:29:29.0807262+07:00,Luna max"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-report"
  scope: "CallMD BASELINE preservation branch v0.2.0b"
  risk: "HIGH native custody/security gate; bounded CI/test repair"
  requested_model: "gpt-5.6-luna"
  requested_reasoning_effort: "max"
---

# CallMD BASELINE implementation report

## Result

**PASS** for the approved preservation branch. The stale provider invocation was
removed, the native-session custody gate was restored without provider-specific
references, and the workflow-to-package command closure guard was added. No
product implementation source was changed.

Requested worker: `gpt-5.6-luna`, reasoning effort `max`.
Agent ID: not exposed in this worker context; no runtime model identity claim is
made. Base: `376ef30db13670e4dea816ceff440f44ce73fffd`.
Result SHA: `UNCOMMITTED` (no commit, push, merge, cherry-pick, rebase, deploy,
or provider/device/app launch performed).

## Exact lease and changed paths

Isolated worktree:
`C:/Users/pc/.codex/worktrees/9000/fung/output/callmd-worktrees/baseline-376ef30`

Changed source/test/CI paths:

- `.github/workflows/ci.yml` — removed the stale `test:google-drive` invocation;
  retained `npm run test:native-session-custody` on the Windows Rust lane.
- `tests/ciCoverage.test.mjs` — retained the two existing coverage assertions;
  added exact-token workflow-to-package closure, dynamic-command failure, shell,
  quoting, comment, prefix-collision, and orphan-file fixtures.
- `tests/nativeSessionCustody.test.mjs` — restored 11 provider-neutral custody,
  typed-broker, lifecycle, recovery, source-boundary, and behavioral-matrix
  tests from the historical mixed suite. The file has no `drive` reference.
- `package.json` — added only `test:native-session-custody`.

Required report path:
`docs/verification/implementation-reports/2026-09-17-callmd-baseline.md`

Read-only implementation inputs: `src-tauri/src/auth_session.rs` and
`src-tauri/src/lib.rs`. No edits were made to either file. No lockfile or
package-lock change was made. Cargo target artifacts and the documented empty
resource directories were generated only inside this isolated worktree.

## Evidence

### Pre-change baseline

| Command | Result |
|---|---|
| `npm run test:ci-coverage` | **PASS** — 2 passed, 0 failed; this confirmed the pre-existing one-way guard gap. |
| `npm run test:native-session-custody` | **FAIL (expected stale command)** — npm reported `Missing script`. |
| `npm run test:google-drive` | **FAIL (expected canceled command)** — npm reported `Missing script`. |
| `npm run test:auth` | **PASS** — 8 passed, 0 failed. |
| `cargo test --manifest-path src-tauri/Cargo.toml native_behavioral_ -- --nocapture` before resource setup | **BLOCKED_ENV** — Tauri build script reported missing `..\\.venv-whisper`. |
| Same Cargo command after the existing CI directory setup | **PASS** — 22 passed, 0 failed; nonzero native behavioral count. |

The environment-only Cargo precondition was resolved by creating the empty
`.venv-whisper` and `runtime` directories in the isolated worktree, matching
the existing Windows CI setup. No dependency install or version change was
performed.

### Post-change focused evidence

| Command | Result |
|---|---|
| `npm run test:ci-coverage` | **PASS** — 4 passed, 0 failed. Both original assertions remain, plus exact reverse command closure and regression fixtures. |
| `npm run test:native-session-custody` | **PASS** — 11 passed, 0 failed. Its child Cargo run executed **22** `native_behavioral_` tests, 22 passed, 0 failed. |
| `npm run test:auth` | **PASS** — 8 passed, 0 failed. |
| `git diff --check` | **PASS**. |
| `package.json` JSON parse | **PASS**. |
| `rg -n -i 'drive' tests/nativeSessionCustody.test.mjs` | **PASS** — no matches. |
| Workflow inventory | **PASS** — no `test:google-drive`; native custody command remains on the Rust lane. |

The Rust matrix output was produced by the custody suite itself, not inferred
from static source names. The suite asserts a positive matrix count and checks
the registered-entrypoint rotation and startup-recovery cases.

## Scope and risk

Risk remains **HIGH** because this gate protects native session custody and
secret-boundary behavior, although the implementation is limited to CI/script
wiring and tests. The canceled provider path was not restored. No assertions
were weakened, no timeout was added to conceal a failure, and no native source
fix was attempted.

Unrun/out of scope: hosted CI, full build/clippy/full Rust suite, product UI,
native app launch, provider, cloud, device, production, commit, and release
evidence. Controller-owned unrelated baseline frontend checks were not
duplicated.

## Version Diff

- `new → 0.1.0b`: implemented the approved BASELINE preservation branch on
  `376ef30…`; removed the stale provider workflow command, restored the
  provider-free custody script/suite on the Windows Rust lane, and added exact
  reverse workflow command closure while retaining the prior two assertions.
- Product implementation version: unchanged.
- Result remains uncommitted pending independent Terra review and controller
  integration authority.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Approved BASELINE preservation repair; focused gates passed with 22 native behavioral tests | UNCOMMITTED; base 376ef30 | Luna max |
