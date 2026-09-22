---
version: "0.2.1b"
created_at: "2026-09-21T05:45:50+07:00,RWANG,base-b336f33"
last_update: "2026-09-21T06:34:20+07:00,01a0c11c-0b30-78d1-bc5c-24e4c9efaa9e,gpt-5.6-luna,max"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-plan"
  scope: "FUNG meeting-intelligence D10-D13 workflow and current feature track only"
  language: "Thai/English"
---

# FUNG Meeting-Intelligence Multi-Agent Workflow

## 1. Authority, scope, and evidence boundary

This is a bounded, declarative workflow package for the current FUNG
meeting-intelligence track: live transcription (LT), knowledge/evidence (KE),
meeting-agent participation and delivery (MA), Google Meet provider/gateway
(GM), and speaker-identity/API boundaries (SI-API). It is a candidate
workflow, not an installed scheduler and not implementation authorization.
The machine-readable companion is
[2026-09-21-meeting-intelligence-task-dag.json](./2026-09-21-meeting-intelligence-task-dag.json).

The authoring task is documentation/workflow setup and validation only. It
does not authorize source or feature implementation, provider procurement or
activation, model downloads, a real meeting, external messages, credential or
gateway setup, commit/push/PR/deployment, or release. The parent orchestrator
may dispatch, schedule, inspect evidence, and review risk; it must not write or
repair source, tests, docs, manifests, or workflow artifacts for a child task.
Boss retains feature, external-provider, merge, and release approvals.

At this freeze:

- The candidate specs and Google Meet decision are inputs, not acceptance
  evidence.
- The frozen baseline reports are at
  <code>docs/verification/implementation-reports/2026-09-21-meeting-workflow-baseline.md</code>
  and
  <code>docs/verification/implementation-reports/2026-09-21-meeting-workflow-baseline.json</code>,
  with status <code>DONE_WITH_OPEN_CONCERNS</code>. The baseline worker is
  closed; the reference excludes this concurrently authored workflow package,
  records the initial 40-path snapshot and protected/input-spec hashes, and
  issues no G1/G2 verdict.
- The workflow-author dispatch is the user-supplied
  <code>01a0c0ee-e8e0-76f3-843a-375905d51e20</code>; the baseline-agent dispatch
  is the user-supplied <code>01a0c0ee-e9d5-7220-89bf-2e4bb97ad501</code>. These
  are provenance inputs, not G1/G2 runs.
- G1 and G2 have **NOT_RUN**. No PASS, ACCEPTED, fabricated run, or reviewer
  identity is recorded for this package.
- The exact workflow, manifest, and both linked-parent files are frozen as one
  review set after the checks listed below. Later verdicts or reports must be
  separate files and must not silently mutate this set.

Risk classification: **HIGH**. The track crosses durable revisions, source
coverage, sensitive identity, ACL/audience decisions, outbox delivery,
external ingress, and release boundaries.

## 2. Models, roles, and independence

The literal model/effort settings are part of every future task packet. No
silent downgrade is allowed.

| Role | Model / reasoning effort | Authority | Required separation |
| --- | --- | --- | --- |
| Parent orchestrator | Parent Codex session / policy-controlled | Dispatches, schedules, checks leases, reviews high-risk objections, freezes handoffs | No source, test, docs, manifest, or repair writes; cannot waive failed tests or G2 |
| Worker | <code>gpt-5.6-luna</code> / <code>max</code> | One exact partition; implementation and its tests/report after approvals | Cannot review or accept its own work; no outside-partition writes |
| G1 first verification gate | <code>gpt-5.6-luna</code> / <code>max</code> | Independent read-only review of the contract/schema handoff | Different human/agent identity from the worker and workflow author; report-only |
| Luna fixer | <code>gpt-5.6-luna</code> / <code>max</code> | Repairs only after a failed or invalidated handoff | Fresh review cycle; cannot self-accept |
| Luna integration writer | <code>gpt-5.6-luna</code> / <code>max</code> | One serial integration partition after pre-integration review | Sole integration writer; exact shared-file leases only |
| Luna full verifier | <code>gpt-5.6-luna</code> / <code>max</code> | Read-only integrated verification before G2 | Independent from the integration writer; report-only |
| G2 final gate | <code>gpt-5.6-terra</code> / <code>high</code> | Read-only final integrated gate | Independent final gate; no repair, merge, or release action |
| Boss | Human approval authority | Feature contract, provider/external action, merge, and release decisions | Not substituted by an agent PASS |

The [official subagent configuration guidance](https://learn.chatgpt.com/docs/agent-configuration/subagents)
supports explicit model and reasoning-effort overrides. The configured model
references are [gpt-5.6-luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
and [gpt-5.6-terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra).
Those links document configuration facts only; this package does not install
or globally configure an agent.

## 3. State, capacity, and handoff rules

The workflow states are <code>WAITING_APPROVAL</code>,
<code>WAITING_DEPENDENCY</code>, <code>READY</code>, <code>LEASED</code>,
<code>IN_PROGRESS</code>, <code>REVIEW</code>, <code>FROZEN</code>,
<code>ACCEPTED</code>, <code>REPAIR_REQUIRED</code>,
<code>INVALIDATED</code>, <code>BLOCKED</code>, <code>SKIPPED</code>,
<code>NOT_RUN</code>, and <code>CLOSED</code>.

- A worker's <code>DONE</code> event means <code>REVIEW</code>, never
  <code>ACCEPTED</code>.
  <code>FROZEN</code> and its write lease is released; reviewer readiness must
  not depend on the reviewer producing a repair.
- A read-only reviewer may start only after the predecessor handoff is
  <code>FROZEN</code> with its report and hashes frozen and its write lease is
  released. The release is a producer-handoff event before reviewer readiness;
  it must not depend on the reviewer producing a verdict or repair.
- Releasing a write lease does not transfer immutable snapshot custody. The
  frozen N3 handoff, report, and hashes remain immutable review input, and no
  writer may mutate that reviewed snapshot. The separate integration write
  lease may be reacquired only after accepted N12 pre-integration G1 for new
  integration work.
- G1 and G2 are read-only and may write only their own dated report partition.
- There may be at most **3 concurrent open Luna execution agents total** across
  workers, fixers, integration, and reviewers. The open-agent record cap is
  tracked separately: at most 3 open Luna records plus 1 Terra record. Finished
  agents must be closed before another slot is claimed.
- The one integration writer is the only writer for the integration partition.
- A changed contract, schema, security rule, public interface, source
  revision, destination binding, dependency digest, or protected dirty-path
  inventory invalidates every descendant handoff that relied on it.
- A code, test, or docs repair is dispatched to a fresh Luna worker/fixer.
  After three unsuccessful repair cycles, the node escalates to Boss; the
  parent cannot bypass, patch, or declare acceptance.

## 4. Baseline and dirty-worktree protocol

<code>HEAD</code> alone is not a sufficient baseline because the worktree
contains modified tracked files and untracked meeting-intelligence
specifications. The baseline agent must later inventory paths, content digests,
dependency digests, and protected dirty paths in the two expected baseline
reports. It must not edit those reports from this authoring task.

Future fresh worktrees exclude dirty and untracked specifications by default.
Before a feature node can run, an authorized Luna must transfer a verified
pinned snapshot containing the approved specs and record:

1. the pinned source/working-tree snapshot and content digests;
2. dependency and lockfile digests;
3. protected dirty-path checks for every shared file; and
4. the absence of a commit-as-workaround or unreviewed cleanup.

The current documentation-only authoring is a disjoint-write exception. It
does not make a future code node ready. No future implementation node is
<code>READY</code> until its feature approval, frozen contract, and resolved
lease/worktree are recorded.

## 5. Gate sequence and dependency graph

~~~mermaid
flowchart TD
  S[Spec and doc review] --> A[Boss feature-contract approval]
  B[WF-BASELINE inventory and digests] --> C[Local contract/schema writer]
  A --> C
  C --> G1C[Independent Luna G1 contract gate]
  G1C --> LT[LT live transcription lane]
  G1C --> KE[KE knowledge/evidence lane]
  G1C --> SI[SI-API source-participant lane]
  LT --> G1L[Independent Luna local-lanes G1]
  KE --> G1L
  SI --> G1L
  A --> EA[Boss external/provider approval]
  C --> EA
  G1C --> EA
  EA --> GM[GM Meet provider/gateway lane]
  G1L --> MA[MA agent/delivery lane]
  G1L --> PIG[Independent Luna pre-integration G1]
  MA --> PIG
  GM -. optional full Meet variant .-> PIG
  PIG --> I[Single Luna integration writer]
  I --> LV[Luna full integrated verification]
  LV --> G2[Terra final gate]
  G2 --> R[Boss release/merge approval]
~~~

The hard dependency for M1/M2 local progress is feature-contract approval plus
the serialized contract/schema G1. The LT, KE, and SI-API lanes do **not**
depend on provider procurement or GM availability. GM is an independent
external-approval branch. If GM is unavailable, approved local observe/draft
and evidence lanes may proceed; the real-room gate remains
<code>WAITING_APPROVAL</code> or <code>WAITING_DEPENDENCY</code>.

The graph maps the current families as follows:

| Family | Required contract focus | MVP status |
| --- | --- | --- |
| LT | source coverage, ASR/utterance revisions, committed events, cursors, crash/replay recovery, local-only egress | Required M1 lane |
| KE | evidence references, freshness/revocation, minimization, ACL/audience recheck | Required M2 lane |
| MA | committed-input policy, exact payload, destination binding, outbox/idempotency, reconciliation | Local/draft lane; external send gated |
| GM | Meet occurrence/session/media mapping, provider account, public ingress, lease/watchdog, capability limits | Independent optional provider branch |
| SI-API | provider labels versus Person identity, review/expected revision, ACL separation, shared mic/unknown handling | Required participant lane |

Native attachment, TTS, voice enrollment, biometric matching, and other
provider/platform expansion are optional capabilities and are not implicit MVP
nodes.

## 6. Exact proposed partitions and leases

The paths below are proposed future task partitions, not writes authorized by
this documentation task. Each future packet must copy its exact paths and
forbidden paths; a broad directory grant is invalid.

| Partition | Family | Exact proposed paths | Report path | Parallel rule |
| --- | --- | --- | --- | --- |
| <code>P-CONTRACT</code> | LT/KE/MA/SI-API | <code>contracts/meeting-intelligence-v1.yaml</code>; <code>src-tauri/src/meeting_intelligence_schema.rs</code>; <code>src-tauri/src/genesis_adapter.rs</code>; <code>tests/meetingIntelligenceContract.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-contract.md</code> | Serial risk gate; Genesis lease transfers only after G1 |
| <code>P-LT</code> | LT | <code>src-tauri/src/live_meeting.rs</code>; <code>src-tauri/src/live_transcript.rs</code>; <code>src/components/LiveMeetingPanel.tsx</code>; <code>src/components/LiveMeetingPanel.css</code>; <code>tests/liveTranscriptionContract.test.mjs</code>; <code>tests/liveTranscriptionRecovery.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-lt.md</code> | Parallel after G1 contract and atomicity stop |
| <code>P-KE</code> | KE | <code>src-tauri/src/meeting_knowledge.rs</code>; <code>src/lib/meetingKnowledge.ts</code>; <code>src/components/MeetingKnowledgePanel.tsx</code>; <code>tests/meetingKnowledge.test.mjs</code>; <code>tests/meetingKnowledgeAcl.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-ke.md</code> | Parallel after G1 contract |
| <code>P-SI</code> | SI-API | <code>src-tauri/src/meeting_participation.rs</code>; <code>src-tauri/src/speaker_merge.rs</code>; <code>src-tauri/src/local_diarization.rs</code>; <code>src/lib/meetingParticipants.ts</code>; <code>src/components/SpeakerIdentityReview.tsx</code>; <code>tests/meetingParticipantAttribution.test.mjs</code>; <code>tests/speakerIdentityReview.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-si-api.md</code> | Parallel after G1 contract |
| <code>P-MA</code> | MA | <code>src-tauri/src/meeting_agent.rs</code>; <code>src-tauri/src/meeting_delivery.rs</code>; <code>src/lib/meetingAgent.ts</code>; <code>src/components/MeetingAgentPanel.tsx</code>; <code>tests/meetingAgentDelivery.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-ma.md</code> | After local G1; external send remains gated |
| <code>P-GM</code> | GM | <code>src-tauri/src/meeting_provider.rs</code>; <code>src-tauri/src/meeting_gateway_client.rs</code>; <code>gateway/src/meet_ingress.ts</code>; <code>gateway/src/lease_watchdog.ts</code>; <code>gateway/tests/meetingGateway.test.mjs</code>; <code>tests/meetingProviderGateway.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-gm.md</code> | Only after Boss external approval; may be skipped if unavailable |
| <code>P-INTEGRATION</code> | all approved families | <code>src-tauri/src/lib.rs</code>; <code>src-tauri/src/genesis_adapter.rs</code>; <code>src-tauri/src/meeting_intel.rs</code>; <code>src/tauri.ts</code>; <code>package.json</code>; <code>package-lock.json</code>; <code>src-tauri/Cargo.toml</code>; <code>src-tauri/Cargo.lock</code>; <code>.github/workflows/ci.yml</code>; <code>tests/meetingIntelligenceIntegration.test.mjs</code> | <code>docs/verification/implementation-reports/2026-09-21-meeting-intelligence-integration.md</code> | One Luna integration writer; all protected leases serial |
| <code>P-VERIFICATION</code> | G1/Luna/G2 reports | Report files only under <code>docs/verification/implementation-reports/</code> named by node | <code>...-g1-*.md</code>, <code>...-verification.md</code>, <code>...-g2-terra.md</code> | Reviewers may not repair artifacts |

The following are protected shared files and have explicit serialized leases:

| Protected path | Lease owner | Release/transfer condition |
| --- | --- | --- |
| <code>src-tauri/src/genesis_adapter.rs</code> | <code>P-CONTRACT</code>, then <code>P-INTEGRATION</code> | P-CONTRACT releases after the FROZEN N3 handoff, report, and hashes, before N4 starts; this does not transfer immutable snapshot custody. P-INTEGRATION reacquires only after accepted N12 pre-integration G1 |
| <code>src-tauri/src/lib.rs</code> | <code>P-INTEGRATION</code> only | Never parallel with any other writer |
| <code>src/tauri.ts</code> | <code>P-INTEGRATION</code> only | Never parallel with any other writer |
| <code>package.json</code>, <code>package-lock.json</code>, <code>src-tauri/Cargo.toml</code>, <code>src-tauri/Cargo.lock</code> | <code>P-INTEGRATION</code> only | Dependency digests frozen before lease; no worker may edit |
| <code>.github/workflows/ci.yml</code> | <code>P-INTEGRATION</code> only | CI contract review and G2 required; no parent bypass |

Every future worker is also forbidden from writing <code>.env*</code>,
credentials, <code>src-tauri/src/fungwire*.rs</code>, the workflow package,
either linked parent, mobile trees, or any path outside its packet. The GM lane
must not modify existing LAN FUNGWIRE authority as a shortcut.

## 7. Mandatory high-risk stop points

An unresolved **HIGH** objection blocks the affected task even when a reviewer
reports <code>PASS</code>. The parent cannot waive a failed test, a required
evidence field, a stop point, or G2.

| Stop point | Required evidence before continuation | Blocked consequence |
| --- | --- | --- |
| <code>SP-LT-ATOMIC-SOURCE-COVERAGE</code> | Tests and durable evidence show source coverage and ASR revision write, canonical projection, committed event, and processed cursor in one serialized mutation boundary; event emission is post-commit; crash/replay uses a recoverable write-intent/idempotent reconciliation path | Do not activate parallel LT/KE/SI consumers |
| <code>SP-KE-MA-DESTINATION-RECHECK</code> | Immediately before external send, recheck audience, ACL/grant, source revision freshness/revocation, and an immutable <code>DestinationBinding</code> for the same meeting occurrence/provider/account/channel/thread plus approval snapshot | Fail closed; no send or destination substitution |
| <code>SP-MA-DELIVERY-UNKNOWN</code> | A timeout/possible send becomes durable <code>delivery_unknown</code>; reconcile using provider receipt/idempotency evidence before any retry; no blind retry and no automatic resend after uncertainty | Hold the outbox item and escalate to an authorized operator/provider reconciliation |
| <code>SP-GM-PUBLIC-INGRESS-LEASE-AUTHORITY</code> | Public ingress authentication, capability, lease ownership, TTL/watchdog, and cleanup are independently defined and tested; cleanup cannot inherit LAN FUNGWIRE authority or credentials | Keep the real-room/provider route pending; do not expose or reuse LAN authority |
| <code>SP-CUSTODY-MIGRATION</code> | Add-only migration, custody/rollback proof, and expected-revision checks | Block schema or durable write changes |
| <code>SP-CREDENTIAL-EGRESS</code> | Explicit credential, region, retention, egress, redaction, and kill-switch approval | Block provider or external send |
| <code>SP-IDENTITY-ACL</code> | Human review and source/proposed/confirmed identity separation; provider labels are not ACL or Person authority | Block identity merge, enrollment, or publication |
| <code>SP-OUTBOX-SAME-OCCURRENCE</code> | One occurrence-bound idempotency key and destination receipt/revoke evidence | Block duplicate-prone delivery |
| <code>SP-CI-RELEASE</code> | CI evidence, package/lockfile review, rollback artifact, and Boss release approval | Block merge, publication, or deployment |

## 8. Approval, invalidation, and repair policy

The following gates are mandatory and are not inherited from the historical
August workflow or from the September 17 Call.md workflow:

1. Read-only spec/domain review identifies conflicts and a proposed contract
   boundary.
2. Boss approves the feature contract, schema, privacy/security boundary, and
   local M1/M2 scope.
3. The contract/schema writer holds the serial risk lease and produces a
   frozen handoff.
4. An independent Luna/max G1 verifies that handoff before the three local
   lanes become ready.
5. After the accepted independent contract/security G1, Boss separately
   approves any provider, public gateway, credential, region, retention, or
   external-send capability. This G1 prerequisite does not block LT/KE/SI-API
   local progress because those lanes have no dependency on provider approval.
6. Each worker produces tests, a report, hashes, environment, and rollback
   notes; <code>REVIEW</code> is not acceptance.
7. Independent G1 review precedes integration. One Luna/max integration writer
   assembles only approved partitions.
8. A Luna/max full verification is followed by the independent Terra/high G2.
   Boss decides merge/release after G2.

Any changed artifact or contract invalidates its descendants, including a
changed source revision, ACL/audience, destination, dependency digest, public
interface, migration, or protected dirty-path check. The repair path is:

<code>INVALIDATED or FAIL -> fresh Luna/fixer -> REVIEW -> independent G1 again</code>.

At most three unsuccessful cycles are permitted for one node. The fourth
attempt is not automatic; it escalates to Boss with the failed evidence and
open high-risk objections. A PASS report cannot erase a failed test or an
unresolved HIGH objection.

## 9. Task packet and report contract

Every future packet must include:

~~~text
Task ID and exact partition:
Role, agent/person identity, literal model, literal reasoning effort:
Parent frozen handoff hash and dependency digests:
Pinned snapshot/worktree and protected dirty-path result:
Approved inputs and required Boss gate:
Exact writable paths and explicit forbidden paths:
Acceptance criteria, stop points, rollback:
Commands/checks with expected exit codes:
~~~

Every worker, fixer, integration writer, G1, Luna verification, and G2 report
must include:

~~~text
status: PASS | FAIL | SKIPPED | NOT_RUN
task_id:
role / agent identity / model / reasoning_effort:
base snapshot and SHA256 hashes:
environment and dependency digests:
commands, exit codes, and test evidence:
changed paths and lease interval:
stop points and high-risk objections:
rollback or recovery evidence:
next gate or exact blocker:
~~~

<code>PASS</code> means the declared checks and evidence passed; it never means
provider, real-room, release, or production readiness. <code>SKIPPED</code>
must name the reason and gate. <code>NOT_RUN</code> must not be converted into
PASS. No report in this package records that G1 or G2 ran.

## 10. Validation and freeze

The package validator must perform all of the following without a heavy app
build:

- parse JSON;
- require unique node, role, partition, edge, and approval identifiers;
- require edge endpoints to name known nodes and prove the directed graph is
  acyclic;
- prove every code-writing node is dominated by feature-contract approval and
  its required predecessor;
- prove the first G1 role is independent from the worker and author;
- prove actual parallel nodes have disjoint paths, except explicitly listed
  serialized leases;
- confirm the three-Luna cap and separate open-agent cap;
- confirm initial implementation nodes are <code>WAITING_APPROVAL</code> or
  <code>WAITING_DEPENDENCY</code>, with no fabricated run/pass/agent fields;
- confirm the package is declarative and not an installed scheduler.
- prove operational readiness as <code>N3 FROZEN + report + hashes</code> →
  released <code>LEASE-GENESIS-CONTRACT</code> → independent <code>N4 READY</code>,
  without using N4 own verdict to release the predecessor lease;
- prove an N4 <code>FAIL</code> or non-<code>ACCEPTED</code> state never unlocks
  N5/N6/N7/N9, and that the integration write lease is reacquired only after
  accepted N12 pre-integration G1.

The bounded checks for this freeze are:

~~~text
node -e "JSON.parse(require('fs').readFileSync('docs/plans/2026-09-21-meeting-intelligence-task-dag.json','utf8')); console.log('JSON_PARSE')"
node -e "const p=JSON.parse(require('fs').readFileSync('docs/plans/2026-09-21-meeting-intelligence-task-dag.json','utf8')); if(!p.declarative_plan || p.installed_scheduler) process.exit(2); console.log('PLAN_FLAGS_OK')"
git diff --check -- docs/plans/2026-09-21-meeting-intelligence-multiagent-workflow.md docs/plans/2026-09-21-meeting-intelligence-task-dag.json docs/plans/2026-08-23-fung-luna-terra-multiagent-workflow.md docs/plans/2026-08-09-fung-master-implementation-plan.md
Get-FileHash -Algorithm SHA256 -LiteralPath docs/plans/2026-09-21-meeting-intelligence-multiagent-workflow.md,docs/plans/2026-09-21-meeting-intelligence-task-dag.json
CHK-LEASE-READINESS-CYCLE from the manifest: N3 FROZEN handoff/report/hashes → released contract write lease → independent N4 READY; N4 FAIL does not unlock N5/N6/N7/N9; accepted N12 is required before integration lease reacquisition.
~~~

No app build, provider call, model download, real-room test, commit, push, PR,
or deployment is part of this validation. After the checks, the four package
authoring leases are released. Future baseline, G1, Luna verification, and G2
reports are separate artifacts and cannot revise this frozen review set.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b -> 0.2.0b | Added the explicit accepted contract/security G1 prerequisite for the GM consumer and recorded the now-frozen baseline as reference-only evidence; local M1/M2 provider independence remains unchanged. |
| 0.2.0b -> 0.2.1b | Repaired the contract lease readiness cycle: release follows the FROZEN N3 handoff/report/hashes before N4 readiness, immutable snapshot custody stays separate, integration reacquisition is gated by accepted N12, and the operational assertion is declared; G1/G2 remain NOT_RUN. |
| new -> 0.1.0b | Added the bounded current-workstream workflow, model/role governance, serial contract and protected-file leases, independent Luna G1, Terra/high G2, local/provider split, high-risk stop points, packet/report contract, and declarative validation rules. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.1b | 2026-09-21 | candidate | Repair cycle 1 corrected the producer-handoff contract lease release and operational readiness criteria; fresh independent G1 is required and no approval is issued. | working-tree; base b336f33 | 01a0c11c-0b30-78d1-bc5c-24e4c9efaa9e |
| 0.2.0b | 2026-09-21 | candidate | Added the accepted contract/security G1 prerequisite for GM and froze the baseline reference boundary; G1/G2 remain NOT_RUN. | working-tree; base b336f33 | RWANG |
| 0.1.0b | 2026-09-21 | candidate | Created the review-frozen meeting-intelligence workflow package; no feature or provider implementation authorized. | working-tree; base b336f33 | RWANG |
