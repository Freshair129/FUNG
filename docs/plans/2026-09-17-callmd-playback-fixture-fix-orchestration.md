---
version: "0.1.1b"
created_at: "2026-09-17T17:22:20+07:00,Codex,343f6ea30a1404a4d756cc4a02f0a5458e11dca2"
last_update: "2026-09-17T17:41:16+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  scope: "Approved four-fixture canonical-root correction"
  complexity: "C-2"
  risk: "HIGH: custody-related fixtures; no production changes"
---

# Four-fixture correction execution

Boss approved RCA v0.2.0b after CI143 proved short-root/long-handle mismatch.
Base: `343f6ea30a1404a4d756cc4a02f0a5458e11dca2`; current main:
`05ed107a2233e8785b95d2ba7dc282c47aee35a7`; PR59 remains unmerged.
Source preimage: `22caf03015b390bca32dc25076256f3c17edeacc45a5a8b9a8091d3007e47990`.
Protected65854-byte prefix before `#[cfg(test)]\nmod tests {`:
`e2c09351b7a9d0c1718eaa0cb5ab6c1bdf99f6afb027b35941db6b267b55621d`.
CI workflow stays `52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e`.

## DAG / ownership

`APPROVE -> FIXTURE-FIX -> TERRA-REVIEW -> EXACT-PUBLISH -> HOSTED-VERIFY`

Main's governance/remote checks run in parallel with implementation. Shared
source has one writer only. All commands use explicit cwd in the isolated root
`C:\Users\pc\.codex\worktrees\9000\fung`; patches use absolute paths.
No closed agent resumes. Main never edits implementation code/tests.

| Node | Owner / lease | Status |
|---|---|---|
| FIXTURE-FIX | Hilbert Luna/max `01a0aee3-77f0-77c0-8146-c7c13dbef6ec`: only four approved fixture call sites inside `src-tauri/src/desktop_playback.rs` test module and `docs/plans/2026-09-17-callmd-playback-fixture-fix-verification.md` | ACCEPTED; source frozen, agent CLOSED |
| TERRA-REVIEW | Schrodinger Terra/high `01a0aeed-a732-7ea1-8251-6fcf65250b10`; dedicated report only | PASS_WITH_HOSTED_GATE_OPEN; agent CLOSED |
| PUBLISH/VERIFY | Orchestrator: approved RCA, this record, explicit reviewed Git paths, read-only hosted checks | READY for scoped publication; fresh hosted run NOT_RUN |
| DESKTOP-PREFLIGHT | Locke Luna/max `01a0aee6-cecf-78f0-a319-ecde67ac9f31`: read-only machine/repository checks; only `docs/plans/2026-09-17-callmd-desktop-local-deploy-preflight.md` writable | RUNNING in parallel; no install/build/launch authority for this worker |

Boss selected **Desktop on this machine** as the deployment destination during
this execution. This resolves the earlier destination question; it does not
waive CI, native/browser/packaged acceptance, rollback or user-data preservation
gates. No Vercel deployment is requested. Preflight reports the exact installed
target and safe update route before any installation or native launch.

Canonicalize only the four affected test roots; fail on canonicalization error.
Keep raw descriptors/audio, expected errors, assertions, negative-custody tests,
production prefix and other fixtures unchanged. Route failure diagnostics to
the actual supplied root. No comparator/helper refactor, dependency/workflow
change, skip/timeout adjustment, production logging or unrelated cleanup.

Require formatting, focused20 tests, source/prefix pins and independent review
before scoped commit/push. Then verify actual hosted merge SHA, all four former
failures, full Cargo, custody, strict Clippy and frontend. Local PASS is not a
hosted pass or packaged/native acceptance. New failures reopen RCA, never a
security exemption. No PR merge, deployment, native launch, installation,
provider/keyring/user-data access is permitted by this fixture approval.

## Frozen publication packet

Both worker and independent reviewer passed formatting and the focused module:
20 passed, 0 failed, 0 ignored. Production prefix and workflow pins above remain
unchanged. Source postimage SHA-256:
`efed4fbe4b6746a09e8f88b0c7ec9fed81a8da35684dd8cfd7a29061b327dbaf`.
Worker report SHA-256:
`32b206ba61faf087687632450569c9aaac6c8f50e47b3d6245d0bc296fcb1384`.
Independent review SHA-256:
`d92fc175c4dbaf893b6ee76cd5fda11a546a1fae64f3978a36e403c227901468`.

The explicit publication set is limited to:

- `src-tauri/src/desktop_playback.rs`
- `.brain/rca/2026-09-17-callmd-hosted-playback-path.md`
- `docs/plans/2026-09-17-callmd-playback-fixture-fix-orchestration.md`
- `docs/plans/2026-09-17-callmd-playback-fixture-fix-verification.md`
- `docs/plans/2026-09-17-callmd-playback-fixture-fix-review.md`

Desktop preflight is separate and excluded. This is a pre-publication snapshot,
not evidence that the push, hosted run, installation or native acceptance has
already happened. Subsequent verification must name the actual published SHA
and hosted merge revision.

## Version diff / CHANGELOG

New ->0.1.0b: record approved bounded fixture fix and independent publication gate.
0.1.0b ->0.1.1b: freeze the reviewed source/report pins and five-file publication set.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Local source review accepted; hosted verification remains open | UNCOMMITTED; base343f6ea | Codex orchestrator |
| 0.1.0b | 2026-09-17 | beta | Fixture correction dispatched after confirmed RCA | UNCOMMITTED; base343f6ea | Codex orchestrator |
