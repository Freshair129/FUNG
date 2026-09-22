---
version: "0.1.0b"
created_at: "2026-09-22T08:22:31.530+07:00,Codex / GPT-5,base-b336f33,pr-8d9077b"
last_update: "2026-09-22T08:22:31.530+07:00,Codex / GPT-5"
status: "candidate"
superseded_by: null
attributes:
  doc_type: "rca"
  domain: "ci"
  scope: "PR #61 frontend baseline contract and Windows Rust wrapper failures"
  risk: "MEDIUM"
  workflow_level: "C-2"
  actual_identity: "Codex / GPT-5 (current worker)"
  agent: "Codex"
  model: "GPT-5"
  parent_role: "orchestrator/reviewer only; no children"
  base_sha: "b336f33ec400a38f003a0665c121069a87a543ac"
  pr_sha: "8d9077b8365af84ef112855b4b091fdde33dd87e"
  pr_url: "https://github.com/Freshair129/FUNG/pull/61"
  ci_run_url: "https://github.com/Freshair129/FUNG/actions/runs/35674629701"
  evidence_status: "CI failures confirmed; no implementation, Cargo rerun, merge, or production claim"
  write_lease: ".brain/rca/2026-09-22-pr61-ci-failures.md only"
---

# RCA — PR #61 CI failures

## Decision

PR #61 remains open and unstable. The two failures have different causes:

1. **Frontend:** a pre-existing red baseline contract, not caused by PR #61.
   Base commit b336f33 already defines test:live-capture-routing but its
   workflow does not invoke it. The existing test:ci-coverage contract rejects
   that mismatch. PR #61 does not change package.json or tests/ciCoverage.test.mjs.

2. **Rust:** caused by PR #61's new PowerShell wrapper. The wrapper reaches
   New-Item -LiteralPath when GitHub's fresh target directory is absent.
   New-Item has no LiteralPath parameter in pwsh; the wrapper fails before Cargo.

No CI, merge, deployment, release, or production success is claimed.

## Symptom

The PR check run was:

- PR: https://github.com/Freshair129/FUNG/pull/61
- Run: https://github.com/Freshair129/FUNG/actions/runs/35674629701
- Frontend job: https://github.com/Freshair129/FUNG/actions/runs/35674629701/job/106578401302
- Rust job: https://github.com/Freshair129/FUNG/actions/runs/35674629701/job/106578401512

The frontend job failed at 2026-09-22T01:08:16Z in Run npm run
test:ci-coverage:

~~~text
test:live-capture-routing defined in package.json but never invoked by
.github/workflows/ci.yml — wire it in, or delete it if it is not meant to gate anything
~~~

The Rust job passed formatting, native-session custody, and clippy, then failed
at 2026-09-22T01:10:54Z in Run Rust library regression with test runtime:

~~~text
meeting-intelligence-test-runtime.ps1 failed closed: A parameter cannot be
found that matches parameter name 'LiteralPath'.
~~~

The same log shows the shell as:

~~~text
shell: C:\Program Files\PowerShell\7\pwsh.EXE -command ". '{0}'"
~~~

The error occurred before the wrapper's “Running Cargo library regression”
message; no Cargo test was started by this failed step.

## Evidence

### 1. PR/base identity and changed-file boundary

Read-only commands used:

~~~powershell
gh pr view 61 --repo Freshair129/FUNG --json number,title,state,baseRefName,headRefName,baseRefOid,headRefOid,url,author,commits,statusCheckRollup,mergeStateStatus
gh api repos/Freshair129/FUNG/compare/b336f33ec400a38f003a0665c121069a87a543ac...8d9077b8365af84ef112855b4b091fdde33dd87e --jq '{status,total_commits,ahead_by,behind_by,files: [.files[] | {filename,status,additions,deletions,changes}]}'
git diff --quiet b336f33ec400a38f003a0665c121069a87a543ac 8d9077b8365af84ef112855b4b091fdde33dd87e -- package.json tests/ciCoverage.test.mjs
~~~

Observed:

- Base: b336f33ec400a38f003a0665c121069a87a543ac.
- PR head: 8d9077b8365af84ef112855b4b091fdde33dd87e.
- The PR is one commit ahead of base.
- package.json and tests/ciCoverage.test.mjs are byte-identical between base
  and PR.
- The PR changes .github/workflows/ci.yml, adds the wrapper, and adds reports;
  it does not add the missing frontend workflow invocation.

Compare view:
https://github.com/Freshair129/FUNG/compare/b336f33ec400a38f003a0665c121069a87a543ac...8d9077b8365af84ef112855b4b091fdde33dd87e

### 2. Frontend failure is pre-existing relative to PR #61

Base and PR package.json both contain the script at line 12:

https://github.com/Freshair129/FUNG/blob/b336f33ec400a38f003a0665c121069a87a543ac/package.json#L8-L13

~~~text
12: "test:live-capture-routing": "node --test tests/liveCaptureRouting.test.mjs",
~~~

The base and PR coverage test derive all test:* scripts from package.json and
assert that each appears as npm run <name> in the workflow:

https://github.com/Freshair129/FUNG/blob/b336f33ec400a38f003a0665c121069a87a543ac/tests/ciCoverage.test.mjs#L71-L83

The base workflow invokes test:ci-coverage at line 31, but its frontend list
does not contain npm run test:live-capture-routing:

https://github.com/Freshair129/FUNG/blob/b336f33ec400a38f003a0665c121069a87a543ac/.github/workflows/ci.yml#L29-L60

The base commit's own CI run already failed with the identical message:

- Base run: https://github.com/Freshair129/FUNG/actions/runs/35519287180
- Base frontend job:
  https://github.com/Freshair129/FUNG/actions/runs/35519287180/job/106100452985
- Exact read-only command:

~~~powershell
gh run view 35519287180 --repo Freshair129/FUNG --job 106100452985 --log |
  Select-String -Pattern 'Run npm run test:ci-coverage|test:live-capture-routing|never invoked|Process completed'
~~~

The base log contains test:live-capture-routing defined in package.json but
never invoked by .github/workflows/ci.yml and exits 1. This establishes a
baseline failure before PR #61.

The earlier b336f33 diff shows the origin of the baseline regression:

~~~powershell
git diff --unified=30 0a1aa33 b336f33ec400a38f003a0665c121069a87a543ac -- package.json .github/workflows/ci.yml tests/ciCoverage.test.mjs tests/liveCaptureRouting.test.mjs
~~~

That diff adds the package script and removes the prior workflow invocation.
Therefore the frontend condition is pre-existing for PR #61, although it was
introduced by the base live-capture commit b336f33.

### 3. Rust failure is introduced by PR #61

Base workflow line 97 directly ran:

~~~text
cargo test --manifest-path src-tauri/Cargo.toml
~~~

https://github.com/Freshair129/FUNG/blob/b336f33ec400a38f003a0665c121069a87a543ac/.github/workflows/ci.yml#L62-L97

PR #61 replaces that final direct test with setup-python and the wrapper:

https://github.com/Freshair129/FUNG/blob/8d9077b8365af84ef112855b4b091fdde33dd87e/.github/workflows/ci.yml#L97-L123

The wrapper validates the target with Get-Item -LiteralPath, then attempts to
create a missing target at line 78 with New-Item -LiteralPath:

https://github.com/Freshair129/FUNG/blob/8d9077b8365af84ef112855b4b091fdde33dd87e/scripts/meeting-intelligence-test-runtime.ps1#L67-L85

The catch converts that parameter error into the observed failed-closed
message:

https://github.com/Freshair129/FUNG/blob/8d9077b8365af84ef112855b4b091fdde33dd87e/scripts/meeting-intelligence-test-runtime.ps1#L149-L151

Exact filtered PR log command:

~~~powershell
gh run view 35674629701 --repo Freshair129/FUNG --job 106578401512 --log |
  Select-String -Pattern 'Rust library regression with test runtime|meeting-intelligence-test-runtime|LiteralPath|parameter|failed closed|shell:|Run cargo clippy|Process completed'
~~~

The PR log shows:

- clippy finished successfully at 01:10:52Z;
- the wrapper was invoked under PowerShell 7 at 01:10:53Z;
- the error was emitted at 01:10:54Z;
- no Cargo test output occurred after wrapper entry.

Read-only local cmdlet compatibility check:

~~~powershell
pwsh -NoProfile -Command '$PSVersionTable.PSVersion; foreach ($name in @("Get-Item","New-Item","Set-Item","Remove-Item","Test-Path")) { $command = Get-Command -Name $name -CommandType Cmdlet; $parameter = $command.Parameters["LiteralPath"]; "{0}: LiteralPath={1}" -f $name, ($null -ne $parameter) }'
~~~

Observed output:

~~~text
7.6.5
Get-Item: LiteralPath=True
New-Item: LiteralPath=False
Set-Item: LiteralPath=True
Remove-Item: LiteralPath=True
Test-Path: LiteralPath=True
~~~

This isolates the defect to the New-Item cmdlet/parameter syntax, not to the
other LiteralPath uses or to a Cargo failure.

## Root Cause

### Root cause A — frontend baseline contract

The base live-capture commit added a test:* package script while removing its
CI step. The pre-existing ciCoverage test correctly derives the script list
from package.json and fails when that script is absent from ci.yml. PR #61
does not touch that contract, so its frontend failure is inherited baseline
redness, not a PR #61 regression.

### Root cause B — Windows PowerShell wrapper compatibility

PR #61 introduces a wrapper that assumes New-Item accepts -LiteralPath. The
GitHub Windows job uses pwsh (PowerShell 7 family), and New-Item does not
expose that parameter. The GitHub target path is under runner.temp and is
fresh for the job, so the missing-target branch executes. The wrapper throws
before Cargo and its catch reports the parameter error.

The minimal literal-safe fix is to replace only the directory-creation line
with:

~~~powershell
[System.IO.Directory]::CreateDirectory($TargetDir) | Out-Null
~~~

Do not replace it with New-Item -Path if literal path semantics are required:
Path accepts wildcard expansion, whereas Directory.CreateDirectory receives
the exact string.

## Why it escaped local detection

The accepted local wrapper run used the already populated frozen-R3 target:

- Manifest:
  C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-repair-r3-20260922\src-tauri\Cargo.toml
- Target:
  C:\Users\pc\AppData\Local\Temp\codex-fung-meeting-intelligence-contract-20260921-target
- Python:
  C:\Users\pc\workspace\fung\.venv-whisper\Scripts\python.exe

The local report records the successful invocation, PowerShell 7 execution,
exit 0, and 503 passed / 0 failed / 1 ignored at lines 127-143:

https://github.com/Freshair129/FUNG/blob/8d9077b8365af84ef112855b4b091fdde33dd87e/docs/verification/implementation-reports/2026-09-22-meeting-test-runtime-wrapper-g1.md#L127-L143

A read-only Test-Path check also found that exact target present. Therefore
the wrapper's line 78 create branch was not exercised locally. The local
fail-closed cases covered a missing manifest, a file supplied as TargetDir,
and missing Python, but not a missing TargetDir:

https://github.com/Freshair129/FUNG/blob/8d9077b8365af84ef112855b4b091fdde33dd87e/docs/verification/implementation-reports/2026-09-22-meeting-test-runtime-wrapper-g1.md#L114-L125

The same report explicitly kept CI status NOT_RUN at lines 157-161. Local
PowerShell 7 success was therefore existing-target evidence, not fresh-runner
compatibility evidence.

## Proposed prevention

- Treat a green local run against a reused build target as insufficient for a
  runner wrapper; exercise both existing-target and missing-target branches.
- Add a no-Cargo preflight case with a fresh target path and an intentionally
  invalid Python path. It should create the target, fail closed at Python
  validation, and emit no Cargo output.
- Record the actual shell path/version and whether the target existed before
  the run in the verification report.
- Check the base commit's required checks before attributing a PR failure.
  A PR cannot be called green while its base is already red.

## Minimal bounded implementation plan

No implementation was applied in this RCA. The exact proposed files are:

1. .github/workflows/ci.yml — add one frontend step,
   npm run test:live-capture-routing, alongside the other frontend test steps.
   This repairs the pre-existing base contract; it is not a PR #61 Rust fix.
2. scripts/meeting-intelligence-test-runtime.ps1 — replace only the
   New-Item -ItemType Directory -Force -LiteralPath $TargetDir line with the
   System.IO.Directory::CreateDirectory line above. Leave the environment
   snapshot/restore and Cargo argument vector unchanged.

Bounded verification after an approved implementation:

1. Parse the wrapper with pwsh -NoProfile and run the fresh-target,
   invalid-Python no-Cargo preflight.
2. Run the frontend coverage contract and the live-capture routing suite.
3. Run the single intended GitHub Actions PR check; inspect both jobs.
4. Keep merge, deployment, production, provider, and model-accuracy claims
   separate until independently evidenced.

No package.json, tests/ciCoverage.test.mjs, Rust source, Cargo manifest, lock
file, existing report, or dirty worktree file should change for this bounded
repair.

## Version diff

- new -> 0.1.0b: added the confirmed PR #61 frontend-baseline and Windows
  PowerShell wrapper RCA, exact base/PR/run evidence, local escape analysis,
  and a two-file bounded implementation plan. No implementation was applied.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-22 | candidate | Confirmed the inherited frontend CI contract failure and PR-introduced New-Item LiteralPath wrapper defect; recorded bounded two-file fix plan with CI/merge success still open. | base b336f33; PR 8d9077b8365af84ef112855b4b091fdde33dd87e | Codex / GPT-5 |

This RCA is report-only. The self-hash is emitted after the write and is
intentionally not embedded in the file. The sole write lease is released after
hash emission; no further edit is authorized by this RCA.
