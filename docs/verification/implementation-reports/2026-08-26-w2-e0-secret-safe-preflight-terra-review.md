---
version: "1.0.0"
created_at: "2026-08-26T08:31:43+07:00,Terra 5.6,base ddcf514a482b3410d4fca6c45fc8e5f99615c97e"
last_update: "2026-08-26T08:31:43+07:00,Terra 5.6"
status: "need review"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "implementation-review"
  scope: "W2 E0 independent evidence review only; no external execution"
  language: "Thai"
  risk: "HIGH"
  complexity: "C-3"
  task_id: "W2-E0-TERRA-REVIEW"
  verdict: "FAIL — P2 immutable downstream-readiness rationale requires fresh envelope"
  reviewed_head: "ddcf514a482b3410d4fca6c45fc8e5f99615c97e"
  candidate_commit: "4915432e8629f94f59a48507a67688773b700133"
  candidate_sha256: "3E6B88D1266CC2B23E88B90CCEE968EB64F61F0C485C22C030B6EDFE4ED98E8F"
  envelope_sha256: "541BED896C3B930C68C812F404BE9578A5BC0E60D22016085B8FEFC5DF61821F"
---

# W2 E0 secret-safe preflight — Terra 5.6 independent review

## Verdict

**FAIL — E0 is not accepted as a complete immutable evidence package.**
The narrow substance of **AC-GDA7-01** passes: the evidence root is outside
the source, JSON is valid, the approved candidate and envelope hashes match,
and the reviewed artifacts contain no detected secret value. However, the
immutable envelope and Luna report give an incorrect/misleading E4 blocker:
they treat the non-empty parent `D:\FUNG-Phase4-TestRestore` as a reason
that the clean-install restore target is not clean.

The approved D-GDA7 runbook instead requires a newly created or non-existing
child `D:\FUNG-Phase4-TestRestore\restore-<archive-id>`. A `README.md` in
the approved parent does not make that future child invalid. E4 is still
truthfully **BLOCKED** because E1-E3 are blocked and no child target has been
created and inspected, but the stated parent-not-empty rationale is not a
valid blocker. Because downstream readiness truthfulness is part of this
review goal and this statement is in immutable E0 evidence, a fresh versioned
envelope and corresponding Luna report are required; Terra does not repair
either artifact.

No P0 or P1 finding was identified. This verdict neither fails source code
nor authorizes any external action.

## Reviewed evidence and identity

| Check | Evidence | Result |
|---|---|---|
| Candidate identity | `4915432e8629f94f59a48507a67688773b700133`; only `docs/specs/2026-08-26-phase-4-w2-external-evidence-execution-amendment.md` changed | PASS |
| Candidate bytes | SHA-256 observed `3E6B88D1266CC2B23E88B90CCEE968EB64F61F0C485C22C030B6EDFE4ED98E8F` | PASS |
| E0 report identity | `ddcf514a482b3410d4fca6c45fc8e5f99615c97e`; only the Luna E0 report changed | PASS |
| Envelope identity | `D:\FUNG-W2-Evidence\D-GDA7\E0\e0-preflight-envelope.json`; JSON parse passed; SHA-256 observed `541BED896C3B930C68C812F404BE9578A5BC0E60D22016085B8FEFC5DF61821F` | PASS |
| Evidence-root isolation | Canonical root is a normal directory, not a link, and is not a child of `D:\FUNG`, TestStorage, or TestRestore | PASS |
| Commit hygiene | Both focused commits have one-file scope; `git diff --check ddcf514^..ddcf514` passed | PASS |

## Secret-safe collection and provenance

| Requirement | Independent observation | Result |
|---|---|---|
| No secret value collection | Envelope records only boolean presence for approved variable names. The stated scan scope is exactly the envelope and Luna E0 report; an independent pattern scan of those same two paths returned 0 findings. | PASS |
| No forbidden identity/material | No `.env`, keyring dump, provider response, device identifier, OAuth code, token, archive bytes, or account identity appears in the reviewed artifacts. | PASS |
| Agent versus authority | Envelope distinguishes execution agent `Luna 5.6`, operator authority `Boss`, and `operator_impersonation=false`. | PASS |
| Source provenance | `source_head=08624df3a7422f306c3882dd2816e29fd8f0a32c`, branch `codex/backlog-truth-sync`, candidate commit and candidate SHA agree with the reviewed report and approved contract. | PASS |
| Dirty-state preservation | Envelope records 0 staged, 6 unstaged, 8 untracked, 0 conflicted, total 14. Current review began from the same 14 pre-existing entries; no entry was staged by this review. | PASS |
| Target and cleanup | Root was recorded absent before creation and empty after creation; cleanup disposition retains the immutable envelope and records no destructive action. | PASS |

## Timestamp concern

**No finding.** The raw envelope field is
`timestamps.utc = 2026-08-26T01:24:01.7312241+00:00`; the report records the
same UTC value. The separate ICT value is
`2026-08-26T08:24:01.7308610+07:00`. These represent the same wall-clock
instant to sub-millisecond collection variance and use the correct offsets.
The controller-observed concern that `timestamps.utc` might contain `+07:00`
is not reproduced; the immutable envelope and report are not contradictory on
timezone provenance.

## Acceptance and success-criteria matrix

| Item | Disposition | Reason |
|---|---|---|
| AC-GDA7-01 — separate, secret-safe E0 root | PASS in substance | Isolation, JSON validity, allowed scan scope, hash, and no detected secret values were independently confirmed. |
| SC-01 — provenance/authority/time/target/cleanup | PASS | Required fields are present and reconcile with commit history, except for the separate downstream wording finding below. |
| SC-02 — secret-safe artifact | PASS | Boolean-only environment facts and two-artifact scan are consistent with the envelope and review. |
| SC-03 — E0 independent from E1-E5 and truthful blockers | FAIL | E0 is appropriately separated, E1-E3 are validly BLOCKED, but E4 includes a misleading parent-directory rationale in immutable evidence. |
| SC-04 — reproducible hashes | PASS | Candidate and envelope hashes match the approved/expected values. |
| SC-05 — focused report commit and diff hygiene | PASS | E0 commit scope is one report path and its diff check passes. |

## Downstream readiness review

| Lane | Terra disposition | Correction or boundary |
|---|---|---|
| E1 clean Windows VM/keyring | BLOCKED — truthful | Missing Hyper-V query authorization and no observed clean Windows VM/equivalent with real OS keyring are prerequisites, not E0 failure or PASS evidence. Any equivalent rationale must be recorded when E1 is actually run. |
| E2 staging Supabase/Edge/RLS | BLOCKED — truthful | Absent CLI/environment-presence facts and absent staging authority/live RLS evidence remain blockers, not proof of a provider or deployment result. |
| E3 Google OAuth/Drive | BLOCKED — truthful | Client-ID presence is absent, E2 is blocked, and no OAuth/provider action occurred. |
| E4 clean-install restore | BLOCKED — classification retained, stated parent reason rejected | E1-E3 are blocked and no `restore-<archive-id>` child has been created/inspected. The parent's `README.md` is not itself a clean-target blocker. |
| E5 Android/Dashboard/FUNGWIRE | BLOCKED — classification retained | The deciding absence is physical-device identity/delegation/revoke evidence. `adb` and `scrcpy` absence may describe this host's optional tooling, but is not a contract-defined mandatory condition and must not be treated as one. |

## Findings by priority

- **P2-01 — Incorrect E4 readiness rationale in immutable evidence.** Envelope
  `downstream_readiness.E4` and Luna report wording assert that the approved
  restore parent is not an empty clean-install target because it contains
  `README.md`. D-GDA7 specifies a fresh child `restore-<archive-id>` under
  that parent. The parent may contain a README; the future child must instead
  be new/non-existing and observed empty before restore. This makes the E4
  *reason* misleading, though it does not make E4 ready. A fresh versioned
  envelope/report is required; do not mutate the reviewed bytes.
- **P3-01 — E5 tooling interpretation watchpoint.** `adb=false` and
  `scrcpy=false` are truthful host observations, but they cannot independently
  define E5 readiness. A reissued matrix must identify lack of physical-device
  evidence/operator/device as the blocker and list any tooling only as an
  optional collection method.

## External and promotion boundary

This review performed no network, credential, keyring, VM, provider,
Supabase/Edge/RLS, archive, device, FUNGWIRE, release, deployment, monitoring,
ledger, push, PR, merge, signing, or production action. E1-E5 being BLOCKED
is not a test failure of E0 and is not PASS evidence for any external lane.
Nothing in this report authorizes remediation outside a new E0 evidence cycle.

## Exact next action

Luna must create **E0 cycle 2** as a fresh, versioned external envelope and a
matching fresh Luna report, leaving the reviewed envelope/report unchanged.
The new E4 reason must say: E4 remains BLOCKED because E1-E3 are blocked and
no fresh `D:\FUNG-Phase4-TestRestore\restore-<archive-id>` child has been
created/validated; it must not use the parent's `README.md` as a blocker. The
new E5 reason must identify missing physical-device evidence without making
`adb` or `scrcpy` mandatory. Recompute hashes, run the same secret-safe scan,
then submit only those new artifacts to a fresh Terra review before E1-E5.

## Version Diff

- Adds the independent W2 E0 Terra review only.
- Confirms candidate/envelope identity, secret-safe scope, provenance,
  timestamp correctness, isolated evidence root, focused commit scope, and
  external boundary.
- Rejects the immutable E0 package solely for a P2 E4 downstream-readiness
  rationale; preserves the underlying E0 isolation/redaction evidence.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 1.0.0 | 2026-08-26 | need review | E0 AC-GDA7-01 substance passes, but P2 E4 readiness wording requires a fresh immutable envelope/report before acceptance. | recorded by this focused review commit | Terra 5.6 |
