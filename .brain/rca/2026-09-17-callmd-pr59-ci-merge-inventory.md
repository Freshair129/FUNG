---
version: "0.1.1b"
created_at: "2026-09-17T12:48:00+07:00,Codex,053d2c5024033d4eed0ec6bd057ce45b0ef112d1"
last_update: "2026-09-17T12:56:32+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "release-verification"
  doc_type: "root-cause-analysis"
  scope: "PR59 merged-result CI inventory failure; approved bounded workflow repair"
  complexity: "C-2"
  risk: "MEDIUM: CI coverage and integration semantics; no product behavior change proposed"
---

# PR59 loses the native custody CI invocation in the merge result

## Symptom

The user requested commit, push and deploy. Existing commit
`053d2c5024033d4eed0ec6bd057ce45b0ef112d1` is already on
`origin/codex/callmd-ui-dag`; remote main is
`05ed107a2233e8785b95d2ba7dc282c47aee35a7`. PR59 is open and not merged.
No duplicate commit or push was needed. No merge or deployment was performed.

Hosted CI35186717039 built the frontend successfully, then failed its inventory
guard. Later frontend suites were skipped. This is not a successful CI run.

## Evidence

- GitHub PR59 reports the exact head053d2c5 and base05ed107 above.
- Frontend job105090249337 checked out synthetic merge
  `23c5edba4baac89c76bc622e5f14cbe4a3b11c44`, as shown by its checkout/log output.
- `npm run build` passed with1812 modules. `test:ci-coverage` returned3 PASS,
  1 FAIL: `test:native-session-custody` exists in package.json but is not invoked
  by `.github/workflows/ci.yml`.
- The workflow fetched directly at23c5edb ends the Windows job after cargo test;
  it lacks Node setup and the custody invocation.
- Local head053d2c5 has the invocation; local `node --test
  tests/ciCoverage.test.mjs` passed4/4 on this inspection.
- Common merge base is `c378af9fac3c00db063948f49f9ee857ebad9126`. Its workflow
  contains the Windows Node22 setup and custody invocation. Main05ed107 deletes
  that block; head053d2c5 preserves it unchanged relative to the common base.
- All35 frozen candidate hashes still match the reviewed source/report packet.
- Final hosted Rust result on23c5edb: formatting and strict Clippy passed;
  cargo test returned467 passed,4 failed,1 ignored. The four failures are
  desktop_playback tests named `eof_closes_source_and_clears_source_identity`,
  `stereo_wav_duration_is_per_channel_at_supported_rates`,
  `unsupported_wav_is_rejected_before_worker_creation`, and
  `valid_pcm16_source_is_validated_from_the_custodied_file`. All reached
  `PLAYBACK_PATH_DENIED`; the unsupported-format test expected
  `PLAYBACK_FORMAT_UNSUPPORTED` instead. Job105090249134 provides this evidence.
  These results do not prove local head053d2c5 Clippy or native/runtime acceptance.

Sources: https://github.com/Freshair129/FUNG/pull/59 and
https://github.com/Freshair129/FUNG/actions/runs/35186717039 .

## Root Cause

This is a three-way integration conflict in meaning, despite a textually
mergeable PR. Main deleted a CI block that the feature branch retained unchanged
from the common base. The synthetic merge therefore carries the deletion while
also carrying the feature branch's package script. The inventory assertion is
correct: the resulting workflow does not execute the retained custody suite.

## Why the issue escaped detection

The accepted local inventory/build checks ran on the isolated feature head.
They did not verify its synthesized merge with the current main. Passing local
head checks were never proof of merged-result coverage; hosted CI exposed that
missing integration check before publication.

## Approved prevention and bounded repair

Boss's latest `approve` authorizes this exact five-step workflow repair after
the candidate proposal. It does not authorize a playback source/test fix,
select a deployment target, waive any failed check, or authorize a PR merge.
Implementation and independent review are tracked in
`docs/plans/2026-09-17-callmd-pr59-ci-repair-orchestration.md`.

1. Fresh Luna/max owns `.github/workflows/ci.yml` only for implementation, plus
   a dedicated verification report. Main remains an orchestrator, not a coder.
2. Reconcile the custody invocation so it survives the merge with current main
   and executes in the Windows Rust job with Node22 and resource prerequisites.
   Preserve all CallMD and other test invocations, strict Clippy and cargo tests.
   A small relocation of the existing block is preferable to restoring canceled
   work or merging unrelated main changes without review.
3. Do not remove the package script, delete its suite, weaken the inventory
   assertion, skip a failure, or introduce a warning exemption. No dependency,
   lockfile, native/auth/backup/schema/CSP/capability or Drive change is proposed.
4. Independent Terra review checks the exact diff and the synthesized merge.
   Verify local inventory plus fresh hosted frontend/Rust jobs, including actual
   custody execution, before considering the CI gate closed. Further failures
   require their own evidence-backed scope; they are not pre-authorized here.
5. Only the reviewed repair files may enter a new commit/push after approval.
   Bind any deployment to the resulting exact revision and selected target.

## Separate deployment decisions and gates

- The four hosted playback failures are an additional, independently blocking
  observation. Their root cause is NOT CONFIRMED by the logs. Source inspection
  shows the four tests use `temp_wave` and uncanonicalized TempDir roots, while
  production `open_custodied_file` compares the final file-handle path with the
  supplied root. A Windows path-representation/fixture discrepancy is a
  hypothesis, not an accepted RCA. A read-only/repro investigation is required
  before proposing any native/test change. This CI workflow repair proposal
  does not authorize changing path custody or loosening its assertions.

- The deploy target is not yet selected: installed Desktop, web Vercel, or both.
  Deploying web does not update the installed native Desktop executable.
- This workspace has no `.vercel/project.json` and no Vercel CLI. The connected
  Vercel account's returned project inventory contains only `resume`, not FUNG.
  This does not prove FUNG has no deployment elsewhere. Do not create/relink an
  unrelated project or change its production alias.
- Browser visual proof remains blocked by Codex webview attachment in the prior
  run. Native capture/playback, actual Whisper, packaged/device/provider and
  release acceptance are not promoted by this preflight.
- No native launch, user-data/credential access, installation, release tag,
  deployment, PR edit/merge, source edit or test-assertion weakening occurred.

## Version diff / CHANGELOG

New ->0.1.0b: document exact remote/CI evidence, root cause and candidate repair.
0.1.0b ->0.1.1b: record explicit approval of the bounded workflow repair only.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Boss approved workflow-only repair; playback fix and deployment remain gated | UNCOMMITTED; inspected053d2c5 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | candidate | Merged-result CI custody invocation loss; repair awaits approval | UNCOMMITTED; inspected053d2c5 | Codex orchestrator |
