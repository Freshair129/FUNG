---
version: "1.0.0"
created_at: "2026-08-26T00:00:00+07:00,Terra 5.6,review 4de939efe66ec47a52033b54b078e6bfce4370cc"
last_update: "2026-08-26T00:00:00+07:00,Terra 5.6"
status: "stable"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "technical-review"
  scope: "W2 E1 D-GDA8 Hyper-V provisioning cycle-2 candidate review only"
  language: "Thai"
  risk: "HIGH"
  complexity: "C-3"
  verdict: "FAIL"
  candidate_commit: "4de939efe66ec47a52033b54b078e6bfce4370cc"
  candidate_sha256: "C3F0BBD503E6CB0D7E6374CC19800C1D88579B195A8136B3CE41EED7F208E187"
  predecessor_review_commit: "1ce72c51cfc5849381d3506ddbc4f94f096f62c8"
---

# Terra Review — Phase 4 W2 E1 Hyper-V VM Provisioning Amendment (D-GDA8), cycle 2

## Verdict

**FAIL (P1)** — cycle 2 closes the two cycle-1 P1 findings, but it introduces
one material operational contradiction in the baseline requirement. The exact
candidate bytes are therefore not approval-ready for provisioning. This review
does not authorize any VM, guest, keyring, lifecycle, cleanup, provider,
release, or production action.

## Candidate identity and review isolation

| Check | Evidence | Result |
|---|---|---|
| Candidate path | `docs/specs/2026-08-26-phase-4-w2-e1-hyper-v-vm-provisioning-amendment.md` | PASS |
| Candidate commit and ancestry | Current review base is `4de939efe66ec47a52033b54b078e6bfce4370cc`; its direct parent is the required cycle-1 Terra review `1ce72c51cfc5849381d3506ddbc4f94f096f62c8` | PASS |
| Candidate commit scope | `git diff-tree` reports exactly one changed path: the candidate document | PASS |
| Exact reviewed bytes | Independently calculated SHA-256 is `C3F0BBD503E6CB0D7E6374CC19800C1D88579B195A8136B3CE41EED7F208E187` | PASS |
| Candidate hygiene | `git diff --check 4de939e^ 4de939e` passes; focused secret-pattern scan reports 0 findings | PASS |
| External-action boundary | The focused candidate diff is documentation only. No source/config, host, Hyper-V, group/IAM, ISO/media, VM/VHD, evidence, keyring, credential, provider, device, ledger, release, deployment, or production action is present in the candidate or performed by this review | PASS |
| Review isolation | This review writes only this report; pre-existing dirty/untracked work remains outside the review commit | PASS |

The candidate frontmatter intentionally binds its own commit/hash only after a
focused commit. The independently calculated blob hash above is the immutable
review identity; a new candidate byte requires a fresh hash and review.

## Predecessor and registered-path contract

D-GDA7 keeps E1 as clean Windows VM/equivalent plus real OS-keyring lifecycle;
E0 is accepted only for AC-GDA7-01, and E1 remains upstream of E3/E4. D-GDA6
accepts only local/static registered-path behavior. In source,
`NativeRegisteredBroker` composes `NativeKeyring`, `NativeClock`,
`NativeListener`, and `NativeProvider`; `auth_session::startup_recover()` is
called from Tauri setup. Those references are correctly treated as later
lifecycle-harness provenance, not as clean-VM or real-keyring proof.

## Cycle-1 finding closure matrix

| Cycle-1 finding | Cycle-2 evidence | Closure |
|---|---|---|
| P1-01 — target identity ambiguous | §2, D-GDA8-02, preflight P1/P5/P8, AC-PROV-01, retention and schema consistently enumerate one VM identity `FUNG-W2-E1-KEYRING-C1`, two filesystem roots, and one exact VHDX. Source/artifact references are provenance inputs only. Collision checks include parent ownership, canonical resolution, reparse/junction, mount/path escape, and Hyper-V registration. | CLOSED |
| P1-02 — baseline checkpoint may retain credential state | D-GDA8-06, P6/P8, AC-PROV-06, stop and retention clauses prohibit automatic/manual/baseline/production checkpoints and prohibit snapshot, export, clone, memory dump, and save-state substitutes. Baseline is a redacted immutable manifest and hashes, not VM state. | CLOSED as raised |

The P1-02 correction is not itself a checkpoint loophole. However, its new
baseline timing is materially contradictory as described below, so P1-03
prevents approval.

## Mandatory control review

| Area | Result | Review basis |
|---|---|---|
| Exact target normalization | PASS | One VM identity, exactly two filesystem roots and one exact VHDX are consistently named. There is no remaining “three target roots” instruction, broad target, wildcard, or provenance input relabelled as a write root. Collision, retention, and future cleanup target language is exact and fail-closed. |
| Checkpoint and substitute prohibition | PASS | Automatic, manual, baseline and production checkpoints/snapshots are prohibited. Export, clone, memory dump and save-state cannot substitute. A future checkpoint needs a separately hashed amendment. |
| Baseline is manifest, not VM state | PASS subject to P1-03 | The baseline artifact is specified as redacted immutable settings/install/provenance manifest plus hashes; no snapshot is authorized. The timing condition that defines its admissibility is not operationally sound. |
| Authority/elevation | PASS | Boss is the sole approval authority/operator boundary. There is no automatic elevation, Hyper-V Administrators mutation, IAM/group change, or policy change. |
| Resources | PASS as candidate; NOT READY operationally | Gen2, 4 vCPU, static 6 GB RAM, dynamic VHDX maximum 80 GB, start thresholds of free RAM >= 12 GB and disk >= 120 GB are consistent. The documented approximately 7.1 GB free RAM remains below threshold, so it cannot be treated as execution readiness and needs fresh evidence later. |
| Media and platform settings | PASS | Boss-supplied supported Windows x64 media, out-of-band SHA-256/license provenance, no download, Gen2 Secure Boot, and conditional redacted vTPM decision are explicit. |
| Isolation and transfer | PASS | Default no vNIC/no external network, no host share/clipboard/provider credential/personal or production material, and hash-pinned no-network transfer are explicit. |
| Harness and lifecycle separation | PASS | Provisioning stops before lifecycle. Real `NativeKeyring` and registered broker evidence require a separate hash-pinned, approved harness; fake/test-only routes cannot be promoted. |
| Stop, retention, destructive boundary | PASS | Ambiguity fails closed; retain powered-off VM/evidence; no delete, overwrite, recursive cleanup, checkpoint deletion, export or clone is authorized. |
| Approval semantics | PASS | A future approval is provisioning-only and excludes lifecycle, group/IAM, cleanup, credential/provider, push/PR/merge/release/deploy and production. This FAIL review issues no approval phrase. |

## New finding

### P1-03 — baseline chronology requires an unproven credential-free post-install VM

**Evidence.** D-GDA8-06 and P6 require a baseline manifest after “supported
guest installation/bootstrap” but before *any* guest account, password, or
authentication state. The same candidate requires a synthetic local account
and out-of-band password handoff in §5, refers to that account in P4, and
requires synthetic guest account/material in D-GDA8-05. It also says that
synthetic guest credentials may safely exist outside evidence provided they are
never snapshot/checkpointed/exported/cloned/dumped.

**Why this fails.** A normal supported Windows installation/OOBE commonly
establishes a local account and an authentication state during bootstrap. The
candidate neither specifies nor proves a supported, reproducible boundary at
which Windows is both “installed/bootstrap complete” and still has no such
state. Deferring OOBE is not named as an approved method, and would still need
to reconcile the later account creation with the claimed clean baseline.
Consequently AC-PROV-06 and the provisioning exit can demand an impossible or
ambiguous evidence state. An operator could only proceed by silently choosing
an OOBE workaround, treating bootstrap as incomplete, or producing a false
credential-free baseline claim.

**Severity.** P1: this is a core acceptance/secret-custody boundary for the
only approved E1 clean-VM provisioning packet. It must be corrected before
provisioning authority is granted. This finding does not assert that guest
credentials are unsafe in the VM; it rejects the contradictory requirement
that the post-install baseline prove they do not exist at all.

**Safe replacement principle.** A synthetic guest account/password may exist
out-of-band inside the disposable VM after normal installation/bootstrap, but
must never enter an evidence artifact and must never be captured in a
checkpoint, snapshot, export, clone, memory dump, or save-state. The clean
baseline must instead be defined around absence of FUNG/application keyring,
test material, provider/personal/production material, and secret-bearing
evidence—not a claim that the OS has no guest authentication state.

## D-GDA8 and acceptance matrix

| ID | Result | Reason |
|---|---|---|
| D-GDA8-01 | PASS | Boss-only authority and no automatic mutation are explicit. |
| D-GDA8-02 | PASS | Exact target identity and collision/retention boundaries are normalized. |
| D-GDA8-03 | PASS candidate / NOT READY execution | Envelope is stable; documented free RAM is below the 12 GB start gate. |
| D-GDA8-04 | PASS candidate | Media/license/Secure Boot/vTPM gate is bounded and no-download. |
| D-GDA8-05 | PASS candidate | No-network and artifact-transfer restrictions are explicit. |
| D-GDA8-06 | FAIL (P1-03) | Manifest-only baseline is correct, but its required credential-free post-install chronology is operationally contradictory. |
| D-GDA8-07 | PASS candidate | Registered-path harness and lifecycle remain separately approved. |
| D-GDA8-08 | BLOCKED by D-GDA8-06 | Stop/retention are safe, but a provisioning evidence matrix cannot pass until the baseline rule is corrected. |
| AC-PROV-01 through AC-PROV-05 | PASS candidate | The exact target, authority, resource, media and isolation contracts are reviewable. |
| AC-PROV-06 | FAIL (P1-03) | It requires a clean powered-off baseline without resolving normal OOBE/account state. |
| AC-PROV-07 | PASS candidate | Source/artifact provenance and lifecycle stop boundary are explicit. |
| AC-PROV-08 | BLOCKED by AC-PROV-06 | A Terra provisioning PASS cannot be legitimately recorded until the baseline acceptance is coherent. |

## Authority, external, and destructive boundary

This review has not run a Hyper-V query, created/started/installed a VM,
downloaded or read ISO content, elevated privileges, changed a group/IAM,
accessed OS keyring/credentials, staged artifacts, transferred data, used a
provider/device, or deleted/overwrote any target. It does not change the
current host `NOT READY` fact into a live readiness result. E1 lifecycle,
Google/Supabase, clean-install restore, device proof, signing, release,
deployment and production all remain outside this candidate and open/blocked
according to their separate evidence gates.

## Required Luna cycle-3 fix packet

1. Replace every requirement that a post-install/bootstrap baseline occurs
   before any guest account, password, or authentication state. Do not imply a
   credential-free installed Windows VM unless an approved, reproducible OOBE
   boundary is separately specified and proven.
2. Define the manifest baseline as powered-off, redacted, hash-bound and free
   of FUNG/application keyring entries, test material, provider/personal/
   production material, and secret-bearing evidence. Permit a synthetic
   guest account and its credentials to exist only out-of-band in the
   disposable VM after normal install/bootstrap.
3. Preserve and cross-reference the absolute prohibitions on checkpoint,
   snapshot, export, clone, memory dump and save-state; state that no such
   artifact may contain guest credentials. Reconcile D-GDA8-05/06, P4/P6/P8,
   §5 guest accounts, AC-PROV-06, SC-PROV-02/03, §10, §11, §12, §13, §16 and
   the cycle-fix matrix.
4. Preserve cycle-2's corrected exact target identity, Boss-only authority,
   no automatic elevation/group mutation, resource/media/no-network rules,
   lifecycle separation, retention and no-delete boundaries. Modify only the
   candidate document, calculate new bytes/hash, then submit fresh Terra
   review. No provision/action is authorized during correction.

No approval phrase is issued for this failed candidate.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-26 | stable | FAIL: cycle 2 closes target and checkpoint findings, but P1-03 rejects the credential-free post-install baseline chronology. | recorded by this review commit | Terra 5.6 |
