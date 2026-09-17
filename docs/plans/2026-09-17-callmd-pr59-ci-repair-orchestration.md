---
version: "0.1.2b"
created_at: "2026-09-17T12:56:32+07:00,Codex,053d2c5024033d4eed0ec6bd057ce45b0ef112d1"
last_update: "2026-09-17T13:29:06+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  scope: "Approved PR59 workflow repair and parallel read-only playback RCA"
  complexity: "C-2"
  risk: "MEDIUM: CI integration semantics; no approved product behavior change"
---

# PR59 release-repair execution overlay

## Authority and pinned inputs

Boss requested `commit push deploy`, then approved the bounded plan in
`.brain/rca/2026-09-17-callmd-pr59-ci-merge-inventory.md`. This overlay supplements
the existing CallMD DAG; it does not reopen or promote its native/browser gates.

- Branch: `codex/callmd-ui-dag`; head `053d2c5024033d4eed0ec6bd057ce45b0ef112d1`.
- Main: `05ed107a2233e8785b95d2ba7dc282c47aee35a7`.
- Previous synthetic merge: `23c5edba4baac89c76bc622e5f14cbe4a3b11c44`.
- PR59 remains open/unmerged; head/base were refreshed at12:56 ICT.
- Workflow preimage SHA256: `3755ef4ce6c9e5adb46ee7c9e746dfdf0df4d21b2785c87a6e1d83daa71d861a`.
- Existing35-pin candidate packet remains historical; this approved lease may
  replace only its workflow artifact. All other implementation files are frozen.

## DAG and exclusive leases

`APPROVE -> CI-REPAIR -> TERRA-REVIEW -> EXACT-PUBLISH -> HOSTED-VERIFY`

`APPROVE -> PLAYBACK-RCA -> PROPOSE-IF-NEEDED -> NEW-BOSS-APPROVAL`

Both branches are acyclic and independent. Main owns governance, dispatch,
remote evidence and publication coordination, never implementation code/tests.
Maximum three active Luna workers; this wave uses two. No closed agent resumes.

| Node | Owner | Write lease | Initial status |
|---|---|---|---|
| CI-REPAIR | Faraday, Luna/max, `01a0adef-fd97-7681-835f-27d670cc4a92` | All leases released | ACCEPTED with provenance disposition below; agent CLOSED |
| PLAYBACK-RCA | Helmholtz, Luna/max, `01a0adef-fe37-7993-baf2-009d510f96a1` | Report-only lease released; owned diagnostics removed | COMPLETE; RCA UNKNOWN; agent CLOSED |
| TERRA-REVIEW | Descartes, Terra/high, `01a0adff-ee33-73f2-912d-1851f4014b71` | All leases released | PASS scoped; WARN disposition below; agent CLOSED |
| EXACT-PUBLISH | Orchestrator Git coordination after review | Explicit five-file set below | READY; not yet executed when this record was frozen |
| HOSTED-VERIFY | Orchestrator read-only GitHub checks | This ledger evidence only | WAITING |

Both workers use the already isolated root
`C:\Users\pc\.codex\worktrees\9000\fung`; previous source leases are released.
All shell commands specify that cwd; patches use absolute paths. The other
checkout and CallMD source are read-only. Git metadata writes, commit/push,
merge, deployment and dependency installation are prohibited for workers.

## Acceptance and publication gate

Frozen workflow handoff SHA256:
`52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e`.
Main independently read its exact diff/hash; only workflow implementation
changed. Worker reports conflict-free prospective merge with resources, Node22
and custody execution before strict Clippy/cargo test. Review was pending at
that handoff and is dispositioned below; hosted execution remains pending.

### Final pre-publication review disposition

Terra v0.1.1b independently serialized all three inputs, verified their raw Git
blob IDs against the pinned base/main/frozen head, executed `git merge-file`
with exit0/no conflicts, and ran the unmodified inventory suite against the
resulting temporary fixture: **4 PASS / 0 FAIL**. Its resulting workflow is
byte-identical to frozen SHA256 `52357104f784ce02a151780143bce5fe765d0b4d1e6502669b498acc326a019e`,
Git blob `f9dee9eb19ed72f2dc7b3eb470e11dcc244a63d6`.

The orchestrator accepts the scoped source verdict and dispositions both WARNs:

- The repeated idempotent resource creation is non-blocking; no implementation
  change is needed for the approved CI repair.
- Faraday's prospective temporary-artifact hash `c2880d...ffd4` is NOT accepted
  as the publication gate's merge artifact. Its cause remains unresolved and
  is not represented as a proven encoding difference. The original worker
  report is retained as historical evidence; its prospective hash is explicitly
  superseded for this gate by Terra's verified exact-input reproduction above.
  This replaces an unverified secondary artifact with independently reproduced
  evidence; it does not waive merged-result testing or alter any source bytes.

All source/report agents are closed. Main's final pre-publication audit again
found all34 non-workflow artifact pins unchanged and the index empty. The exact
publication set is:

1. `.github/workflows/ci.yml`
2. `.brain/rca/2026-09-17-callmd-pr59-ci-merge-inventory.md`
3. `docs/plans/2026-09-17-callmd-pr59-ci-repair-verification.md`
4. `docs/plans/2026-09-17-callmd-pr59-ci-repair-review.md`
5. This orchestration record.

This document is frozen before commit/push so its `NOT_RUN` hosted state is
explicitly a pre-publication snapshot. Subsequent exact commit/remote SHA and
GitHub run outcomes must be checked live, not inferred from this snapshot.

Final staging lint removed one trailing space on a blank line inside the worker
report's diff excerpt; no implementation or evidence value changed. Terra's
recorded original worker-report hash remains historical. Published report hash
after this documentation-only whitespace correction is
`faf91f788b3ec31165e339d9907bf94f2f5856d38479be6c1b6394ab3835dfcd`.

Playback RCA v0.1.1b is frozen at
`2d6c1511db07f1ed73293ce72b0894b44c98f58768db0f846500db7b37ef948e`.
Local focused playback tests passed20/20; the short-path hypothesis was not
reproduced and hosted root cause remains UNKNOWN. This candidate RCA stays
outside the workflow repair publication set. Main verified its task-specific
temporary directory is absent; shared temporary storage was retained.

Pre-publication baseline audit: all34 non-workflow artifacts in the existing
integration checkpoint matched their SHA256 pins; index was empty. Generated
diagnostic `.tmp/` outputs, if present, are excluded from publication. Only
explicit reviewed paths may be staged; no blanket `git add` is permitted.

1. Minimal workflow-only implementation; Windows Node22/resource prerequisites,
   custody test and every other test/strict-Clippy invocation retained.
2. Local inventory and prospective merged-result inventory pass independently.
3. Terra reviews frozen diff/hashes; any implementation correction goes to Luna.
4. Stage explicit reviewed paths only, commit/push without force, verify remote
   exact SHA and fresh PR merged-result workflow. Do not merge main wholesale.
5. Hosted frontend/Rust checks, including actual custody execution, must pass
   before claiming CI completion. Local checks alone are insufficient.

Four hosted playback failures remain separate blockers. Investigation may
confirm a root cause and propose a bounded fix; source/test changes require
new approval. Deployment target is still unanswered (installed Desktop, web,
or both). No deploy, installation, native launch, credential/user-data access,
release tag, PR merge or acceptance waiver is authorized by this overlay.

## Version diff / CHANGELOG

New ->0.1.0b: record approved CI repair, disjoint parallel RCA, review/publication
gates and unchanged deployment boundaries.
0.1.0b ->0.1.1b: freeze workflow for independent Terra review and record bounded
playback investigation outcome without inventing a root cause.
0.1.1b ->0.1.2b: accept byte-verified prospective inventory, disposition the
secondary worker-hash discrepancy, and authorize the exact five-file publication
under Boss's existing approval; hosted/deployment gates remain open.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.2b | 2026-09-17 | beta | Scoped repair accepted using Terra exact-input evidence; exact publication ready | PRE_PUBLICATION; inspected053d2c5 | Codex orchestrator |
| 0.1.1b | 2026-09-17 | beta | Freeze workflow and dispatch Terra; playback remains unknown | UNCOMMITTED; inspected053d2c5 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | beta | Activate bounded CI repair and parallel playback RCA | UNCOMMITTED; inspected053d2c5 | Codex orchestrator |
