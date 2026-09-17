---
version: "0.1.0b"
created_at: "2026-09-17T03:18:33+07:00,Luna max worker,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T03:18:33+07:00,Luna max worker"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-report"
  scope: "Approved cover v0.2.0b documentation coherence amendment only"
  complexity: "C-3"
  risk: "MEDIUM documentation; HIGH downstream native/backend"
  evidence_boundary: "static documentation and control-plane checks; no product execution"
  requested_model: "gpt-5.6-luna"
  requested_reasoning_effort: "max"
  agent_id: "01a0abdc-2fe7-76f3-a054-939dc880d5cd"
  runtime_model_identity: "not independently attested"
---

# Call.md desktop parallel amendment — Luna worker report

## Outcome

**PASS — documentation coherence only.** The exact approved cover v0.2.0b
scheduling/authority alignment is applied to the three leased specification
documents and this report. The current approval record is the
[source of authority](2026-09-17-callmd-approval.md); earlier pending wording and
the earlier serial schedule remain provenance only.

Independent Terra amendment review is **NOT_RUN** and is the required next gate.
Product tests, Rust commands, native registration/runtime, device/audio, hosted CI,
integration, production, commit, push, merge, and release are **NOT_RUN**. No
environment blocker was encountered because this lease required no product
execution: `BLOCKED_ENV = none`.

## Requested execution identity and base

| Item | Recorded value |
|---|---|
| Requested model | `gpt-5.6-luna` |
| Requested effort | `max` |
| Agent ID | `01a0abdc-2fe7-76f3-a054-939dc880d5cd` |
| Runtime model identity | Not independently attested; no hidden runtime-model claim |
| Base | `376ef30db13670e4dea816ceff440f44ce73fffd` |
| Branch | `codex/callmd-ui-dag` |
| Current manifest | 26 nodes / 30 dependency edges; max 3 Luna workers |
| Authority | [2026-09-17 current execution approval](2026-09-17-callmd-approval.md) |

## Exact write lease and changed scope

Only these four files were edited by this worker:

1. `docs/specs/2026-09-17-callmd-desktop-contracts.md`
2. `docs/specs/2026-09-17-callmd-desktop-acceptance.md`
3. `docs/design/2026-09-17-callmd-desktop-ui.md` — text only; no SVG/PNG asset change
4. `docs/verification/implementation-reports/2026-09-17-callmd-parallel-amendment.md`

The amendment makes only these approved coherence changes:

- Records current full P1-B selection, scoped SVG/PNG acceptance, exact baseline
  preservation scope, and current approval-record linkage; prior review provenance
  remains visible.
- Replaces the obsolete serial order with
  `CONTRACT_TEST_REVIEW → BACKEND_INTERFACE_REVIEW`, then allows
  `BACKEND_RECORDING → BACKEND_REVIEW` and
  `SHARED_CONTRACT → SHARED_REVIEW → UI_SHELL/UI_LIVE/UI_HISTORY → UI_TASK_REVIEW`
  on disjoint leases. `INTEGRATE` joins `BACKEND_REVIEW` and `UI_TASK_REVIEW`.
- States that both `BACKEND_RECORDING` and `SHARED_CONTRACT` start only after an
  accepted interface review; native/backend security review is retained.
- Defines pure DTO/adapter/interface tests as capable of passing before Rust
  commands/native registration exist, while keeping native/runtime proof separate.
  Executed failures caused only by missing selected implementation are explicitly
  `EXPECTED_RED_MISSING_IMPLEMENTATION`; runner/import/setup errors are never
  accepted as expected-red or green and are classified `BLOCKED_ENV` or `FAIL`.
- Keeps runtime/native gates with backend and integrated verification; does not
  change product semantics, DTOs, commands, PCM support, custody, Q&A scope,
  resource limits, security boundaries, or evidence vocabulary.

No workflow, manifest, ledger, product/test/CI, source, asset, cloud/provider,
schema, CSP, Drive, commit, push, merge, deploy, or release file was edited by
this worker. Existing unrelated controller changes were present before this
lease and were not touched.

## New document revisions and digests

| Path | Revision | Lines | SHA-256 after amendment |
|---|---:|---:|---|
| `docs/specs/2026-09-17-callmd-desktop-contracts.md` | `0.1.1b → 0.1.2b` | 409 | `E34B3727184EE99DA3E689573163EFD19F79F6BF81655FFD7B84175DC840AE7C` |
| `docs/specs/2026-09-17-callmd-desktop-acceptance.md` | `0.1.1b → 0.1.2b` | 352 | `CE44C55CAB1B1D42D2D2806D8B5920AB71C3DD4D95B64BDB779A6A1E1CA5B06C` |
| `docs/design/2026-09-17-callmd-desktop-ui.md` | `0.1.0b → 0.1.1b` | 227 | `191421D8B83275A441DBF8CD6DE36476E9E7EC9E4E1A12E531F3FB074CD5C533` |

The report's own digest is intentionally not embedded in itself; it is reported
after this write to avoid a self-hash cycle.

## Preserved exact boundaries

The amendment did not redesign the reviewed contracts. The documents still retain
the exact `RecordingKey` pair, existing/new command names and DTO/error shapes,
`ReviewError` mapping, project/recording validation, local-provider-only Q&A,
graph/live-tail exclusion for new recording Q&A, source and prompt caps, native
capture/playback admission, custody/path checks, owner/epoch handling, two source
handles, `<=64 KiB` reads, `<=1 MiB` queue, resource limits, PCM16 mono/stereo
8–96 kHz exact-rate support, no resampler/codec expansion, no renderer media path,
and no cloud/schema/CSP/Drive expansion.

## Verification evidence

Commands run from the approved worktree:

| Check | Result |
|---|---|
| `git rev-parse --verify HEAD` | **PASS** — `376ef30db13670e4dea816ceff440f44ce73fffd` |
| PowerShell JSON parse of `docs/plans/2026-09-17-callmd-ui-task-dag.json` | **PASS** — 26 nodes, 30 dependency edges, max 3, `gpt-5.6-luna`, `max` |
| `git diff --check -- <three amended specs>` | **PASS** — no whitespace errors |
| Approval-record content check | **PASS** — current `approve`, cover `v0.2.0b`, and base `376ef...` present |
| Current approval-link check | **PASS** — all three amended specs link `2026-09-17-callmd-approval.md` |
| Required-term/scope guard scan | **PASS** — 63 retained scope/control terms; required interface/native/NOT_RUN terms present in all three docs |
| Stale pre-amendment authority/order scan | **PASS** — no matches for unapproved A/B, old serial-order, pending-format, or 25/28-current claims |
| Product/runtime/native/device/hosted-CI commands | **NOT_RUN** — outside this documentation lease |
| Independent Terra review | **NOT_RUN** — handoff required |

## Risks and handoff

- Native playback, capture admission, recording authority, and provider/device
  behavior remain HIGH-risk downstream gates; static or pure tests do not promote
  them.
- The accepted format is the scoped SVG/PNG P1 exception only; the full
  Figma/Penpot/full-brief package remains outside this amendment.
- Any later semantic contract change invalidates dependent interface/backend/shared/UI
  reviews and requires a fresh bounded review cycle.

Return this exact four-file documentation diff for independent Terra review. No
commit, push, merge, cherry-pick, rebase, deploy, cloud/provider/device/app launch,
secret handling, Drive restoration, or implementation-code action was performed.

## Version Diff

- `contracts 0.1.1b → 0.1.2b`: current authority and interface-first fork/join;
  exact contract/security/resource scope preserved.
- `acceptance 0.1.1b → 0.1.2b`: current approval, 26/30 manifest overlay,
  interface/pure/expected-red/native test disposition, and retained NOT_RUN gates.
- `UI 0.1.0b → 0.1.1b`: approved P1-B/order and scoped SVG/PNG exception in text;
  visual semantics/assets unchanged; runtime remains NOT_RUN.
- `report new → 0.1.0b`: bounded evidence and handoff record.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | candidate | Completed approved documentation coherence amendment; Terra review pending; product/runtime evidence not claimed | UNCOMMITTED; base 376ef30 | Luna max worker |
