---
version: "1.0.0"
created_at: "2026-08-26T00:00:00+07:00,Terra 5.6,review 9ea8dc31fb65a4a2d9aa406644fe42e043205e94"
last_update: "2026-08-26T00:00:00+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "technical-review"
  scope: "W2 E1 D-GDA8 Hyper-V provisioning candidate review only"
  language: "Thai"
  risk: "HIGH"
  complexity: "C-3"
  verdict: "FAIL"
  candidate_commit: "9ea8dc31fb65a4a2d9aa406644fe42e043205e94"
  candidate_sha256: "0DBC79898ACDC3D71B4BFEF6816E2A4AE3F33674A5536FCA6A3042EDC192E4B0"
---

# Terra Review — Phase 4 W2 E1 Hyper-V VM Provisioning Amendment (D-GDA8)

## Verdict

**FAIL (P1)** — candidate bytes are not approval-ready. Two hard safety gates
are internally ambiguous: exact target identity and checkpoint contamination.
No provisioning, lifecycle, or external action is authorised by this review.

## Candidate identity and review isolation

| Check | Evidence | Result |
|---|---|---|
| Candidate path | `docs/specs/2026-08-26-phase-4-w2-e1-hyper-v-vm-provisioning-amendment.md` | PASS |
| Candidate commit | `9ea8dc31fb65a4a2d9aa406644fe42e043205e94` is current `HEAD`; its tree changes one file only | PASS |
| Candidate SHA-256 | Independently calculated `0DBC79898ACDC3D71B4BFEF6816E2A4AE3F33674A5536FCA6A3042EDC192E4B0` | PASS |
| Candidate diff / whitespace | `git show --check` and `git diff --check <parent> <candidate>` pass | PASS |
| Review isolation | This review writes only this report. Candidate, source/config, host, Hyper-V, groups/IAM, ISO/media, VM/VHD, evidence root, keyring, credentials, provider, device, ledger, release, production, and pre-existing dirty files remain untouched | PASS |

The candidate's self-referential frontmatter leaves its commit/hash externally
bound. That pattern is acceptable for a candidate because the committed blob
above was independently hashed. It does not cure the two substantive P1
findings below.

## Dependency and source-contract alignment

The candidate correctly preserves D-GDA7's ordering: E0 is accepted only for
AC-GDA7-01; E1 still requires a named clean Windows VM/equivalent and real
OS-keyring lifecycle evidence before E3/E4. It does not relabel D-GDA6
local/static proof as real-keyring proof. D-GDA6's approved non-test contract
remains `RegisteredBrokerEntrypoints` in `auth_session.rs`; its
`startup_recover()` is called from production setup in `lib.rs`. The candidate
correctly requires a later hash-pinned registered route rather than the
test-only `FakeKeyring` evidence.

Authority separation is also sound: Boss selects an elevated session, VM/media,
synthetic account and later lifecycle approval; no automatic elevation,
Hyper-V Administrators membership, IAM/group change, provider call, cleanup, or
production action is granted. Exact D-GDA8 approval is explicitly restricted to
later provisioning, not lifecycle execution, group/IAM mutation, credentials,
provider actions, cleanup, push, PR, merge, release, deploy, or production.

## Mandatory-gate assessment

| Gate | Independent assessment | Result |
|---|---|---|
| 1. Identity, scope, parent/peer/source alignment | Exact file/commit/hash and one-file candidate scope pass. D-GDA7/D-GDA6/workflow/source boundaries are aligned. | PASS |
| 2. Authority | Boss/session selection is explicit; automatic elevation and group/IAM mutation are forbidden. | PASS |
| 3. Exact VM/path collision safety | Candidate has a contradictory count and set of target roots; see P1-01. | FAIL |
| 4. Resource/drift | Gen2/4 vCPU/static 6 GB/dynamic 80 GB and start thresholds are explicit. The point-in-time ~7.1 GB free RAM is correctly `NOT READY`; D: free disk and logical CPU facts do not imply readiness. The stronger wording "no provision/start" is coherent pending a fresh threshold record. | PASS |
| 5. Media/license/boot security | Boss-supplied supported Windows x64 ISO, hash, out-of-band license provenance, Secure Boot and conditional vTPM are bounded; download and activation-secret handling are forbidden. | PASS |
| 6. Network and transfer isolation | No vNIC/external network, host share, clipboard, provider credential, or personal account; transfer is separately approved, read-only, hash-pinned and no-network. | PASS |
| 7. Checkpoint/contamination/retention | Automatic checkpoints and post-secret export/clone/dump are forbidden, but the baseline-checkpoint rule does not exclude synthetic guest credentials; see P1-02. | FAIL |
| 8. Artifact/harness separation | Clean pinned worktree and non-production artifact manifest are required; provisioning stops before real `NativeKeyring`/registered-broker lifecycle. | PASS |
| 9. AC/SC/exit and promotion boundary | AC/SC/exit classes distinguish provisioning from lifecycle and exclude push/PR/merge/release/deploy/production. Derivative exact-target and checkpoint ACs cannot pass until P1s are fixed. | FAIL |
| 10. Approval semantics | D-GDA8 text correctly limits a later exact approval to provisioning only; it does not authorize lifecycle, group/IAM change, cleanup, or external action. | PASS |
| 11. Host-fact interpretation | Observations are explicitly point-in-time. False CPU virtualization flags under an active hypervisor are correctly called non-authoritative, not proof of absent hardware support. | PASS |

## Findings

### P1-01 — exact target boundary is contradictory

`§1.1` identifies three filesystem paths as "Approved targets":
`D:\FUNG-W2-VM`, `D:\FUNG-W2-VM\D-GDA7\E1`, and
`D:\FUNG-W2-Evidence\D-GDA7\E1`. `§2`, however, defines only two filesystem
roots — `D:\FUNG-W2-VM\D-GDA7\E1\cycle-1` for config/VHD and
`D:\FUNG-W2-Evidence\D-GDA7\E1\cycle-1` for evidence — plus the VM name.
Despite that, D-GDA8-02 says "VM name and three target roots per §2".

This is not a terminology-only defect. It leaves the collision preflight,
provisioning envelope, retention and a later exact cleanup approval unable to
determine whether the parent path, the `E1` child, or the `cycle-1` leaf is the
write/retain target. Treating `D:\FUNG-W2-VM` as a target also conflicts with
the candidate's own prohibition on broad roots. The required exact identity for
destructive-safety checks is therefore not one unambiguous set.

### P1-02 — baseline checkpoint can capture a guest credential

D-GDA8-05 requires a synthetic, disposable guest account/material. P6 then
lists a Boss-supplied guest account as an input to guest bootstrap, while
D-GDA8-06 permits a baseline checkpoint so long as it occurs before
"keyring/test material." Neither that decision nor AC-PROV-06 states that a
synthetic local account/password (or its credential-derived guest state) is
secret material for checkpoint purposes.

Consequently, a compliant-looking bootstrap could create the synthetic account,
then checkpoint before FUNG keyring test material. That checkpoint would retain
a reusable authentication state despite the stated no-secret checkpoint policy.
Redacting the password from evidence does not remove it from a VM checkpoint.
This contradiction is unsafe for a disposable isolation boundary.

No P0, P2, or P3 finding is recorded. Both P1 findings independently prevent
approval-ready provisioning.

## D-GDA8 and acceptance matrix

| Decision / criterion | Disposition |
|---|---|
| D-GDA8-01 / AC-PROV-02 | PASS — Boss-controlled elevation; no automatic group/IAM mutation. |
| D-GDA8-02 / AC-PROV-01 | FAIL — P1-01 leaves the exact target set inconsistent. |
| D-GDA8-03 / AC-PROV-03 | PASS candidate — conservative resource envelope and fresh-start gate are explicit; live execution remains blocked by current RAM. |
| D-GDA8-04 / AC-PROV-04 | PASS candidate — media/license/Secure Boot/vTPM boundary is correct. |
| D-GDA8-05 / AC-PROV-05 | PASS candidate — no-vNIC/no-network and transfer boundary are correct. |
| D-GDA8-06 / AC-PROV-06 | FAIL — P1-02 permits a potentially credential-bearing baseline checkpoint. |
| D-GDA8-07 / AC-PROV-07 | PASS candidate — real native/registered lifecycle remains separately gated. |
| D-GDA8-08 / AC-PROV-08 | FAIL derivative — an immutable envelope cannot truthfully bind an ambiguous target/checkpoint policy. |
| SC-PROV-01 and SC-PROV-05 | FAIL derivative — immutable target identity and a Terra-acceptable package are not yet possible. |
| SC-PROV-02 and SC-PROV-04 | PASS candidate — secret exclusion and evidence-class separation are explicitly required. |
| SC-PROV-03 | NOT EXECUTED — it requires later approved provisioning and powered-off readback. |

## External and destructive boundary

No host query, elevation, group/IAM change, VM/VHD creation, ISO/media action,
keyring access, guest bootstrap, checkpoint, network/provider action, device
action, cleanup, deletion, push, PR, merge, release, deployment, or production
operation occurred. E1 remains `BLOCKED` pending a corrected candidate, its
review and exact approval, followed by the named elevated boundary, fresh
resource evidence, approved media, and a separately approved lifecycle harness.

## Required Luna fix packet — cycle 2

1. Replace every reference to "three target roots" with one consistent,
   enumerated identity set: (a) VM name, (b) the sole config/VHD root
   `D:\FUNG-W2-VM\D-GDA7\E1\cycle-1`, and (c) the sole evidence root
   `D:\FUNG-W2-Evidence\D-GDA7\E1\cycle-1`. State that those are **two
   filesystem roots plus one VM identity**. Remove `D:\FUNG-W2-VM` and
   `D:\FUNG-W2-VM\D-GDA7\E1` as executable targets; they may be named only
   as non-target parents when necessary. Reconcile `§1.1`, `§2`, D-GDA8-02,
   prerequisites, P1/P5/P8, AC-PROV-01, cleanup, and envelope fields.
2. Make the baseline safe and unambiguous. For this provisioning amendment,
   prohibit all checkpoints rather than allowing a baseline checkpoint. Require
   a redacted immutable baseline fingerprint and powered-off record instead.
   State that any future checkpoint requires a separate amendment proving it
   predates **all** guest account creation, passwords, authentication state,
   keyring material, test material, and any other secret-bearing state.
   Reconcile D-GDA8-05/06, P6, AC-PROV-06, SC-PROV-02/03, stop conditions,
   retention, schema, and lifecycle wording.
3. Preserve all other authority, no-network, no-delete, lifecycle-separation,
   external-gate and dirty-worktree boundaries. Commit only the new candidate
   file revision; calculate a new SHA-256; submit it to fresh Terra review.

No approval phrase is issued for this failed candidate. A new commit/hash and
fresh independent review are mandatory after Luna's correction.

## Version Diff

- Adds the independent Terra D-GDA8 candidate review.
- Accepts candidate identity/scope, authority, resource, media, isolation,
  artifact, source-contract and approval-separation boundaries.
- Rejects the draft on two P1 safety ambiguities: exact target identities and
  credential-bearing baseline checkpoints.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-26 | stable | FAIL: D-GDA8 requires Luna cycle-2 correction for target identity and checkpoint credential contamination before approval. | recorded by this review commit | Terra 5.6 |
