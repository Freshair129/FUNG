---
version: "0.1.1b"
created_at: "2026-09-17T01:38:00+07:00,Codex,c378af9fac3c00db063948f49f9ee857ebad9126"
last_update: "2026-09-17T01:58:37+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-report"
  scope: "Call.md-to-FUNG parallel scan and workflow evidence; no implementation"
---

# Call.md UI orchestration ledger

## Authority and evidence boundary

- User instruction: multi-agent DAG scan; primary workers Luna `max`; parallel
  execution; orchestrator must not edit implementation code.
- Base: `c378af9fac3c00db063948f49f9ee857ebad9126`.
- Branch/worktree: `codex/callmd-ui-dag` at
  `C:/Users/pc/.codex/worktrees/9000/fung`.
- Source reference `F:/call.md-main` and main checkout remained read-only.
- Result SHA: **UNCOMMITTED**. No push, deployment, provider operation, package
  installation, product/test implementation, or memory update in this run.
- Current work: approved workflow and feature-documentation wave, not approved
  feature code. Boss's `ap[prove` response approves the workflow and drafting
  described in the preceding question, not a future feature package.

## Actual dispatches

### Scan wave (completed historical dispatches)

These are actual subagents, not a hypothetical team. All three were dispatched
concurrently using `gpt-5.6-luna` with `reasoning_effort=max`; each was permitted
to write only its own report.

| DAG node | Agent | Actual ID | Evidence / state |
|---|---|---|---|
| SCAN_SOURCE | Dewey | `01a0ab7a-30b2-7571-b78d-3d4ce42d64bb` | [Source UI scan](2026-09-17-callmd-source-ui-scan.md); COMPLETED / accepted as bounded source evidence; agent closed |
| SCAN_FUNG | Sagan | `01a0ab7a-314c-78b3-ada0-8b842844541d` | [FUNG contract scan](2026-09-17-callmd-fung-contract-scan.md); COMPLETED / accepted as bounded contract evidence; agent closed |
| SCAN_GATES | Dirac | `01a0ab7a-31f5-7a62-8bc7-ed0053be473b` | [DAG/gates scan](2026-09-17-callmd-dag-gates-scan.md); COMPLETED / accepted as bounded graph evidence; agent closed |

Codex authored only the [workflow](../../plans/2026-09-17-callmd-ui-luna-max-dag-workflow.md),
[task manifest](../../plans/2026-09-17-callmd-ui-task-dag.json), and this ledger.
Source-report frontmatter was normalized by Codex without altering its
findings. The existing workflow's Terra feature/task/integration reviews are **NOT_RUN**;
they remain future gates, not evidence for this proposal.

### Documentation wave (approved and dispatched)

The prior agent IDs were resumed with scan context and requested Luna max
instructions. All three accepted bounded documentation packets in parallel on
the unchanged base. **Model continuity across resume is not attested:** the
contract and acceptance workers later reported GPT-6 supplied model context.
Do not count those resumed outputs as verified Luna/max executions. Their
documents remain usable candidate inputs, not proof of model compliance.
Previous scan outputs and their separate dispatch records remain historical.

| DAG node | Agent / ID | Exclusive write partition | State |
|---|---|---|---|
| DOC_CONTRACT | Sagan / `01a0ab7a-314c-78b3-ada0-8b842844541d` | `docs/specs/2026-09-17-callmd-desktop-contracts.md`; own `callmd-doc-contract.md` report | DONE_WITH_CONCERNS / closed; resumed model unattested |
| DOC_UX | Dewey / `01a0ab7a-30b2-7571-b78d-3d4ce42d64bb` | `docs/design/2026-09-17-callmd-desktop-ui.md`; `docs/design/callmd-desktop/`; own `callmd-doc-ux.md` report | Drafts/assets preserved; interrupted and shutdown before report; resumed model unattested |
| DOC_ACCEPTANCE | Dirac / `01a0ab7a-31f5-7a62-8bc7-ed0053be473b` | `docs/specs/2026-09-17-callmd-desktop-acceptance.md`; own `callmd-doc-acceptance.md` report | DONE_WITH_CONCERNS / closed; resumed model unattested |

#### Model-provenance checkpoint and fresh dispatches

The resumed DOC_CONTRACT and DOC_ACCEPTANCE owners returned
DONE_WITH_CONCERNS and were closed. Their reports preserve the discrepancy.
The controller requested fresh agents with explicit tool parameters
`model=gpt-5.6-luna`, `reasoning_effort=max` for bounded finalization of the
existing drafts, not an exhaustive re-audit. A successful dispatch establishes
requested configuration; no hidden runtime attestation is inferred.

| Node | Fresh requested model | Agent ID / nickname | Lease handoff |
|---|---|---|---|
| DOC_CONTRACT | Luna max | `01a0aba1-1e28-7483-b2e6-279a4ce4b72f` / Ramanujan | Sagan completed/closed → fresh exclusive owner RUNNING |
| DOC_ACCEPTANCE | Luna max | `01a0aba1-9055-7c12-9631-061d9eec3f7d` / Lorentz | Dirac completed/closed → fresh exclusive owner RUNNING |
| DOC_UX | Luna max | `01a0aba5-49c6-7cf0-8730-efa52945c531` / Mendel | Dewey interrupted/shutdown after drafts/assets → fresh exclusive owner RUNNING |

At most three worker partitions are active; new workers never overlap an old
owner's lease. If a fresh explicitly configured dispatch also reports a model
mismatch, record it and stop model-dependent acceptance for Boss direction.
No model uncertainty grants code authority or permits an unannounced substitute.

Reports reside under `docs/verification/implementation-reports/2026-09-17-`.
No source, test implementation, CI configuration, main/source checkout,
credentials, or sibling worker document may be edited by these packets. UX
SVG boards are design documents, not application code or runtime evidence.
The independent Terra documentation review remains NOT_RUN until dispatched.

## Scan synthesis

- Source: current mounted Call.md composition differs from the screenshot;
  several screenshot-matching cards are static/unmounted or not found as
  renderer actions. Call.md capture/assist paths depend on cloud services.
  Reuse presentation patterns, not the runtime or renderer-held key storage.
- Thai WPM and question counts need FUNG-owned locale semantics; screenshot
  values are not data. Bookmarks, new agenda/metrics and proactive-assist
  semantics remain P2/gated.
- The source worker's broad suggested UI partitions are advisory. This
  workflow selects desktop only and reserves `App.tsx` for Integration Luna;
  it does not activate the worker's conditional mobile/web suggestions.
- Existing graph: 105 nodes / 335 relationships; zero dangling endpoints;
  six missing paths all belong to canceled Drive code/tests. There are 26
  node-hash mismatches under the worker's documented SHA-256-prefix comparison,
  not 26 asserted unique changed files. The original generator does not state
  its algorithm. Three document-reference SCCs are cyclic, while the 14
  `depends_on` edges are acyclic. This is not a scheduler DAG.
- Baseline CI: two stale command references (`test:google-drive` and
  `test:native-session-custody`) are absent from package scripts. Existing
  coverage checks do not check the reverse CI-to-package direction. Recorded
  as a separate prerequisite; no CI/source fix made.
- Gate-worker review disposition: serial CI/bridge lease transfers and
  isolated generated-output/runtime-profile ownership are now explicit.
  Post-proposal control-plane changes require recorded revisions and
  invalidation. Baseline repair deliberately requires separate explicit
  authority; documentation discovery is not blocked by red/unknown baseline.
  The gate worker's unfinished formal new-DAG checks are covered by the
  orchestrator's local validation below, not relabeled as independent checks.
- FUNG contracts: current live transcript/session controls, recording-scoped
  transcript/summary, jobs and export actions can be resurfaced. Desktop
  multi-recording enumeration/playback bridges are missing; Q&A is
  project-wide. A local-API capability is not a desktop-bridge contract.
- Draft revision before handoff: added `BACKEND_RECORDING` and
  `BACKEND_REVIEW` between approved contract tests and shared bridge work.
  Final manifest is 24 nodes / 27 edges (initial draft was 22 / 25). Boss must
  explicitly select or exclude missing history/playback/Q&A/reopen behavior.
  Exclusion is not the default and is not a completed implementation claim.
  No code, schema or external-API authority was granted by this revision.

## Verification record

| Check | Result | Evidence boundary |
|---|---|---|
| Base/branch and initial worktree status | PASS | Local Git inspection; clean base, isolated new branch |
| Actual parallel Luna max dispatch | PASS | Three returned agent IDs above, disjoint report partitions |
| Final task manifest structural/ownership validation | PASS (local) | 24 nodes, 27 edges, unique IDs, valid endpoints, no cycle; three disjoint groups of three workers; all eight code-writing nodes have APPROVAL as an ancestor; all workers Luna max; reviewers read-only |
| Scan evidence SHA-256 | PASS (local) | All three accepted report hashes match the manifest |
| Artifact metadata, links and path inventory | PASS (local) | Six artifacts exist with required metadata and valid local Markdown links; 15 unique write paths currently exist, 30 are declared future paths, not current capabilities |
| Final Git scope | PASS (local) | Exactly six new documentation/JSON artifacts; no tracked diff or product-code changes; `git diff --check` clean for tracked files |
| Product tests/build/native runtime | NOT_RUN | Documentation-only scope |
| Terra review, hosted CI, physical device/provider proof | NOT_RUN | Not replaced by static scans |
| Commit/push/merge/release | NOT_RUN | Not authorized by the current task |

Final manifest validation at 2026-09-17T01:50:17+07:00 ran through
`node --input-type=module` using built-in Node modules, exit **0**, with no
generated code file. The checker parsed the JSON,
used depth-first traversal for cycles/topological order, tested approval
ancestry for code nodes, and compared normalized case-insensitive exact and
directory-prefix write paths for each parallel group. It also checked group
capacity, independent dependencies, worker model/effort and read-only reviewer
partitions. It also compared report SHA-256 values, resolved local Markdown
links, checked frontmatter fields, inventoried existing/proposed write paths,
and compared Git's tracked/untracked paths with the six-artifact allowlist.
The earlier 22-node validation is superseded by this 24-node result; worker
reports retain their original checkpoint facts. This does not verify the future product implementation or resolve
candidate interfaces; those remain gated.

The initial workflow/scanning task completed as a **documentation proposal**.
`SCAN_SOURCE`, `SCAN_FUNG`, `SCAN_GATES` and `WORKFLOW` are accepted for this
bounded purpose. At that handoff all workers were closed. Following Boss's
workflow approval, the three documentation nodes are now running; feature
implementation approval remains absent. No autonomous background executor was
installed.

## Handoff protocol

Original approved proposal inputs (historical SHA-256, observed after its validation):

| Input | Digest |
|---|---|
| Workflow v0.1.0b | `800de334e2de5cd5a0dbca480ce3fe392909e9dc43f2dc864a29c8391b882a3e` |
| Task manifest v0.1.0b | `1072446fa8a0ed8b5fe3853997b1d81e1b1478cd32ffd30a1095a7265a814c96` |

The three accepted scan-report digests are stored with their manifest nodes.
The ledger is not self-hashed. Subsequent control-plane changes require a
revision entry and affected-node invalidation, not silent approval carryover.

### Control-plane revision 0.1.1b

- Recorded 2026-09-17T01:58:37+07:00: Boss message `ap[prove` approves the
  original input digests above and the next documentation wave.
- Changed fields: workflow approval/status/version; manifest approval record,
  three DOC node states/agent IDs/disjoint leases; ledger documentation dispatch.
- Subsequent candidate-only lease additions from DOC_CONTRACT:
  `desktop_playback.rs`, `live_meeting.rs`, `LiveMeetingPanel.css`,
  `InstrumentRail.tsx`, and a NEW integration test. SEC-1/SEC-2/UI-1 feature
  approval remains required; no source file was changed. A compact approval
  cover and controller-recorded independent review report join the document
  allowlist. The 24-node dependency topology is unchanged.
- Model-provenance correction and fresh explicit dispatches are recorded above;
  resumed draft outputs alone cannot close the requested-model evidence gate.
- Source/base and feature contracts unchanged. Accepted scans remain valid;
  no implemented descendants exist to invalidate. Code nodes remain planned,
  and the feature `APPROVAL` node is not satisfied.
- New control-plane digests will be recorded at documentation-wave handoff;
  intermediate RUNNING snapshots are not an approved product package.

Resume from the manifest and this ledger, not a stale narrative. Verify HEAD,
Git status and every accepted input digest before dispatch. A completed scan
does not authorize code. After the workflow proposal is reviewed, the next
documentation wave is `DOC_CONTRACT`, `DOC_UX`, and `DOC_ACCEPTANCE`, three
disjoint Luna max tasks. Only an independently reviewed, Boss-approved feature
package can unblock baseline/contract/code nodes.

The JSON is planning metadata; no autonomous background executor has been
installed. No recurring task or separate user-visible task was created.

## Rollback

Only new documentation artifacts were created. If rejected, retain them as a
candidate or remove only the six Call.md workflow/scan artifacts after explicit
direction; there is no application state or database migration to undo.

## Version Diff

- `0.1.0b → 0.1.1b`: record workflow approval and actual resumed Luna max
  documentation wave; feature/code authority remains absent.
- `new → 0.1.0b`: actual agent IDs, bounded ownership, pinned base and evidence
  states for the Call.md UI DAG workflow.
- Application code/version and main checkout: unchanged by this run.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Record Boss workflow approval and parallel documentation dispatch | UNCOMMITTED; base c378af9 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | candidate | Establish actual parallel scan ledger and implementation approval boundary | UNCOMMITTED; base c378af9 | Codex orchestrator |
