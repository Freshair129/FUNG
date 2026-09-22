---
version: "0.1.0b"
created_at: "2026-09-21T09:21:26+07:00,Sartre,01a0c1b9-b266-7332-9a87-a612b0c0ebf0,b336f33ec400a38f003a0665c121069a87a543ac"
last_update: "2026-09-21T09:21:26+07:00,Sartre,01a0c1b9-b266-7332-9a87-a612b0c0ebf0"
status: "need review"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "decision-record"
  scope: "local Genesis schema-limit remediation and SI/D8 privacy repair"
  execution_state: "PREPARATION_SELECTION_NOT_IMPLEMENTED"
  approval_status: "USER_SCOPE_APPROVED_NUMERIC_CAP_SELECTION_REQUIRES_IMPLEMENTATION_REVIEW"
  change_risk: "HIGH"
  author_id: "01a0c1b9-b266-7332-9a87-a612b0c0ebf0"
  nickname: "Sartre"
  parent_orchestrator: "01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75"
  model: "gpt-5.6-luna"
  reasoning_effort: "max"
---

# Decision record — local Genesis schema-limit remediation

## Document control

| Field | Value |
|---|---|
| Decision owner | Parent orchestrator / user-approved local repair scope; implementation workers validate and report |
| Status | `candidate` / preparation selection only |
| Date | 2026-09-21 |
| Related task | N3 remediation R1; original N3/N4 contract/schema gate |
| Related requirements | Existing approved meeting-intelligence LT/KE/MA/SI-API local scope; SI/D8 privacy contract |
| Related specifications | `docs/architecture/MEETING_INTELLIGENCE_DOMAINS.md`; `docs/specs/2026-09-21-speaker-identity-domain-design.md`; current workflow/DAG |
| Superseded by | none |

## Context and root cause

Frozen N3 source arithmetic is `v10=41`, `added=25`, `v11=66`. Genesis
`79b41a3f4ae4026d086b634c631f4f4a7ccbd142` rejects packages over 64 tables;
the representative N3 test exits `101` during schema installation.

Independent N4 also found a separate HIGH gap: plaintext identity-bearing
values remain beside syntactic local ciphertext references, and attribution is
serialized into durable revision/event payloads. A prefix plus a 64-hex hash
does not prove blob/key existence, authenticated decryption, AAD/scope
binding, or ciphertext agreement.

The existing approved SI/D8 design is the authority. This record does not
redesign privacy or product scope.

## Approved bounded remediation / implementation selection

The latest user approval covers a local Genesis dependency adjustment with
regression tests, the SI/D8 private-identity repair, and fresh independent
Luna/Terra gates. The exact table cap **128** is selected here as the bounded
implementation target for the frozen 66-table package. It is not a prior user
quote, an upstream approval, or a higher-resource qualification.

1. In the independent local Genesis clone only, change the package table guard
   `64 -> 128`.
2. Keep `named_queries=128`, per-table `columns=128`, primary-key width `4`,
   indexes `64`, package hashing, identifier validation, schema sequencing,
   migration checks, transaction/CAS, and all atomicity checks unchanged.
3. Under existing SI/D8, validate encrypted local relationship custody,
   key/blob existence, authenticated decryption/AAD/scope, and ciphertext hash
   before the authoritative transaction. Private relationship authority must
   use approved opaque/vault-scoped references and ciphertext metadata, without
   plaintext Person/provider identity in durable projections or serialized
   attribution/event/WAL/audit payloads.
4. Preserve provider-label/Person/ACL separation, expected revisions, scope,
   review state, post-commit events, and the single Genesis atomic boundary.
   Fail closed on missing, tampered, swapped, undecryptable, stale, or revoked
   references. Do not add a provider, biometric/TTS flow, second store, or
   product scope.

## Local WIP dependency boundary

Later implementation may use the existing commented FUNG `Cargo.toml` patch
convention, pointed at the prepared clone:

```toml
[patch."https://github.com/Freshair129/GenesisBlock.git"]
genesis-block-native = { path = "C:/Users/pc/AppData/Local/Temp/codex-fung-genesis-schema-limit-r1-20260921" }
```

This is temporary, Windows-specific, nonportable WIP. It must not be
committed, published, placed in global Cargo configuration, or used to alter
the cached checkout/registry/cache. The existing shared target and
process-only debug/incremental settings remain the only later build target.

## Alternatives and gates

| Option | Disposition |
|---|---|
| Local package cap `64 -> 128` only | **Selected for local experiment** |
| Namespace/package split | Not selected; cross-namespace FK/ACL and one-transaction atomicity are unproven |
| Remove a retained aggregate | Not selected; changes approved scope |
| Change other Genesis limits | Rejected; outside bounded repair |
| Keep plaintext identity beside ciphertext shape refs | Rejected; fails SI/D8 and N4 HIGH stop point |

No source lease is acquired by this record. Parent allocation must keep the
Genesis/Cargo repair and FUNG adapter/types/privacy repair disjoint, then
dispatch fresh independent Luna and Terra gates only after both handoffs are
reviewed. This record does not claim any implementation or gate result.

## Security, privacy, and AI impact

No provider, model, audio, credential, user database, or external endpoint was
accessed. The intended repair reduces plaintext private-relationship exposure
while retaining local-first defaults and SI/D8 purpose/ACL separation.

## Verification and review

- Evidence: `docs/verification/implementation-reports/2026-09-21-meeting-remediation-r1-bootstrap.md` and its JSON companion; frozen N3/N4 report/RCA; upstream AGENT/C4/domain docs.
- Reviewers: parent high-risk orchestrator and later independent Luna/Terra gates; not yet accepted.
- Revisit trigger: local cap still rejects v11, private custody cannot be proven, protected leases drift, or upstream publication is requested.

## Version diff

| Version | Status | Difference |
|---|---|---|
| new -> 0.1.0b | need review | Records the approved bounded local remediation selection and its nonportable/nonrelease boundary; no implementation performed. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-21 | need review | Candidate local Genesis schema-limit remediation decision prepared by Sartre; no implementation or publication authorization implied. | working-tree; base b336f33ec400a38f003a0665c121069a87a543ac | Sartre / 01a0c1b9-b266-7332-9a87-a612b0c0ebf0 |
