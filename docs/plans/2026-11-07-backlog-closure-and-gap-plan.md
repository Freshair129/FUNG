---
version: "0.1.0b"
created_at: "2026-11-07"
last_update: "2026-11-07"
status: "candidate"
superseded_by: null
attributes:
  domain: "programme-delivery"
  doc_type: "plan"
  scope: "Close in-flight issue-96 branch work and remaining open issues/PRs as of 2026-11-07"
  language: "English"
---

# Backlog closure and gap plan — 2026-11-07

Lead: Claude (planning, documentation, dispatch, review, merge, reporting).
Executor: local Codex CLI 0.162.1. This plan schedules work; it is not evidence
that any task is complete.

## Source of truth used

`AGENTS.md` names the [master plan](2026-08-09-fung-master-implementation-plan.md)
as the roadmap; no `docs/delivery/ROADMAP.md` exists in the repository. Status
truth comes from [Desktop real progress](../Desktop/08-real-progress.md) and the
[implementation ledger](../verification/implementation-reports/progress.md).

## Snapshot (2026-11-07)

| Item | State | Evidence |
| --- | --- | --- |
| Worktrees | `main` (clean, = `origin/main` 090daac) and `fix/issue-96-app-loopback-transition` | `git worktree list` |
| Branch `fix/issue-96-…` / PR #98 (draft) | 25 pushed commits; CI frontend+rust green on 95a7c96. Working tree holds ~18.6k uncommitted changed lines in 38 files plus 12 untracked source/doc files (routed playback, source lease, scoped fs, writer buffering, renderer-loss containment) | `git status`, [playback close plan](2026-10-30-playback-close-ownership.md), [writer buffering](2026-11-01-routed-writer-buffering.md) |
| #96 PID Stop/Resume PCM loss | Open, release blocker. No product repair. Paired renderer-queue diagnostic is reviewed GO but was blocked by the Dota guard | Issue #96 comments 2026-11-02, [RCA](../verification/rca/2026-11-01-issue-96-pid-audio-gap-rca.md) |
| #99 Incident/RCA docs | Docs pushed on the #96 branch; lands when that branch merges | commits 17a01fb, 02e39d9 |
| #93 stop-wait guard | Fix eae669a on `main`; installed retest showed finalization beyond former timeout and guard cleared | [installed UAT](../verification/implementation-reports/2026-10-23-installed-releasefix-uat.md) |
| #95 app inventory on STA | MTA worker fix; installed ec494fd package listed both controlled PIDs | same report, "ec494fd installed retest" |
| #94 system loopback initial silence | Native WASAPI packet-flag clock; native probes v2/v3/active-v4/idle-v5 pass; installed 40be411 system/app WAVs match source PCM | same report; ledger row 2026-10-24 |
| #79, #81, #82 | Source/browser evidence merged via PR #80; native/installed visual acceptance not recorded as passed | ledger rows 2026-10-15 |
| Dependabot #90, #92 | Frontend CI failed on a stale 2026-10-20 base; `main` CI is green | `gh pr view` |

## Global constraints (carry verbatim into every Codex handoff)

- Thai UI labels, named exports, existing CSS conventions, anon-key-only
  configuration, surface-param routes ungated; SEC-1/SEC-2 boundaries unchanged.
- No capture fallback, fabricated PCM, relaxed audio tolerance, target
  injection, implicit rerouting, or new driver (from the
  [routed playback contract](../specs/2026-10-29-routed-playback-quarantine-contract.md)).
- CPU/CI tests are not installed, device, PID or endpoint acceptance. Report
  exact commands, revision and counts; failed or interrupted runs are not passes.
- Codex does not commit, push, merge, tag, close issues or release. Lead does.
- Model selection: test/debug/computer-use work uses `gpt-6.1-terra` with
  effort `high`; code/spec/docs work uses `gpt-6-luna` with effort `max`.
  Both verified on this machine with `codex exec -m <model> -c
  model_reasoning_effort="<effort>"`.

## Work packages

### W0 — Hygiene (lead)

Move the stray native inventory dumps `--out` and `inventory.json` (endpoint
IDs; not source) out of the repository to
`G:\AI-Studio\verification\fung-stray-20261107\`. Exit: neither file in
`git status`.

### W1 — Verify the uncommitted #96 working tree (Codex, Terra/high)

Run on the exact tree: `cargo fmt --all -- --check`, strict all-target Clippy
(`-D warnings`), the Rust library suite, all registered Node suites
(`npm test` or the package's equivalent), `npm run build`, `git diff --check`,
and a relative-link check of changed docs. Fix only test, lint, formatting or
link failures; any product-behavior change needed to pass is stopped and
reported. Exit: a verification report under
`docs/verification/implementation-reports/` with commands, counts, failures and
fixes, plus the model/effort used.

### W2 — Secure the work (lead)

Review the W1 diff and report, split the tree into coherent commits (writer
buffering; source custody/lease; routed playback and catalog; renderer-loss
containment; frontend contracts; docs), push to PR #98 and confirm hosted CI.
PR #98 stays draft. Exit: clean `git status`, CI green on pushed head.

### W3 — Close evidenced issues (lead)

Close #93 and #95 with links to installed evidence. For #94, close only if the
40be411 installed report explicitly covers delayed-playback alignment;
otherwise leave open with the missing step named. Leave #79/#81/#82 open with
a comment naming the single installed check each still needs. Ask Dependabot
to rebase #90/#92; merge when CI is green on the new base (scripts-only
dependency bumps).

### W4 — Routed playback queued-executor timeout (Codex, Luna/max then Terra/high)

Implement the "Queued executor timeout" section of the
[playback close plan](2026-10-30-playback-close-ownership.md): one absolute
deadline captured before enqueue, runtime-timer bound, exact-generation
cancellation, retained claim until exact unstarted rollback, remaining budget
passed to the executor. Then a Terra round for scheduler rejection,
accepted-but-stalled, start-versus-timeout, discard/poison and runtime shutdown
races. Starts only after W2.

### W5 — #96 paired renderer-queue diagnostic (Codex, Terra/high)

Run the already-reviewed diagnostic in `E:\verification\fung-pid96-render-drain-20261102`
only when the Dota guard is clear, without bypassing it. Record the result in
the #96 RCA as an experiment. Diagnostic only; no product change.

## Stop points (need Boss)

1. **Release scope for #96.** Merge of PR #98 to `main` requires installed
   acceptance of **both** PID and endpoint modes. More than a dozen PID studies
   have found no product fix. If W5 does not yield a repair, the lead stops and
   asks whether to (a) keep investigating, (b) ship endpoint mode and mark PID
   experimental/disabled, or (c) another option. This is a scope/release
   decision; the lead will not make it.
2. Installed-package acceptance needing real audio routing, HDMI endpoints, or
   reinstall of the user's profile.
3. Any merge of PR #98, tag, or release.

## Exit gates for this plan

- W0–W3 complete with clean worktree and green CI on PR #98.
- W4 implemented with CPU race tests passing and recorded.
- W5 executed or explicitly blocked with the reason.
- Stop point 1 presented to Boss with evidence.
