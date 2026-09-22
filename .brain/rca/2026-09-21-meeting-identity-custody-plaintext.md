---
version: '0.1.1b'
created_at: '2026-09-21T09:29:38+07:00,Dalton,b336f33ec400a38f003a0665c121069a87a543ac'
last_update: '2026-09-21T09:32:28+07:00,Dalton,b336f33ec400a38f003a0665c121069a87a543ac'
status: 'need review'
superseded_by: null
attributes:
  domain: 'meeting-intelligence/privacy'
  doc_type: 'rca'
  scope: 'N3/N4 private identity custody and local Genesis dependency preparation'
  lifecycle: 'candidate'
  review_state: 'needreview'
  actualagentid: '01a0c1bb-f1a1-7d32-940d-92ba382ebce8'
  nickname: 'Dalton'
  parentagentid: '01a0bfa8-2ac8-7ed1-a1a9-363e391c7d75'
  model: 'gpt-5.6-luna'
  reasoning_effort: 'max'
  complexity: 'C-3'
  change_risk: 'HIGH'
  expectedrepoformat: 'FUNG frontmatter plus concise RCA, custody diagram, approved repair plan, tests, version diff, and changelog'
  expected_repo_format: 'FUNG frontmatter plus concise RCA, custody diagram, approved repair plan, tests, version diff, and changelog'
  changelog: '0.1.1b: corrected approved clone-only Genesis cap remedy and sentinel test boundary'
  versiondiff: '0.1.1b corrects the local Genesis remedy and clarifies durable sentinel absence'
  write_lease: '.brain/rca/2026-09-21-meeting-identity-custody-plaintext.md only'
---

# RCA — meeting identity custody and Genesis package limit

## Boundary and status

READY for parent high-risk review only. This is read-only preparation plus the
single leased RCA write. The frozen candidate source is unintegrated: there is
no evidence that it wrote live user data because schema installation failed before
fixture setup and before a meeting transaction. The candidate logic is still HIGH
risk if integrated. The original dirty root checkout, old N3 report/RCA, specs,
workflow, and frozen source remain unmodified.

Implementation remains blocked until the parent grants an exact per-file source
lease after the new repair-checkout snapshot is verified. No additional module
lease is requested; the listed adapter/schema/contract/test/report paths are
sufficient unless implementation proves otherwise.

## Symptom

### H2 — private relationship custody is not proven

`ParticipantAttribution` carries plaintext `provider_label`, `person_id`, and
`acl_subject`. The adapter serializes the whole attribution into
`transcript_revisions.attribution_json` and the event-log payload. The candidate
schema also keeps plaintext `participant_profile_id` foreign keys beside
ciphertext references in `recording_participants` and
`speaker_identity_links`.

`local-ciphertext:` plus a 64-hex suffix is accepted as a reference without
loading ciphertext bytes, proving key existence, authenticating the envelope,
checking AAD scope/entity/revision/model context, or matching the decrypted
semantic Person relationship.

### H1 — local Genesis dependency blocks runtime proof

The pinned candidate package is cumulative v10 `41` plus `25` retained v11
tables, or `66`, above the Genesis limit of `64`. The representative atomic
test exits `101` with `REL_SCHEMA_VALIDATION_FAILED: schema resource limit
exceeded` before fixture setup. The earlier N3 runtime-count line is corrected:
the G1 rerun did not emit it; only the source arithmetic and direct install
failure are evidence.

## Evidence

- Frozen `genesis_adapter.rs` SHA256:
  `15682F5CDB7D4C67920854CFE88E971627BBF1A5D6C4E55E6461F413ED7CD8BD`.
- Frozen `meeting_intelligence_schema.rs` SHA256:
  `801856B92C276996737CEACB7A9A39634CA912C5E58272CFFB1D89E79147ABC7`.
- Root G1 contract report SHA256:
  `AAEB676635E3025C4CC4D61F1400114D553B14918AB1CA9F7773A927390AFE8C`.
- Frozen validation is shape/prefix/hash validation; it does not perform
  ciphertext/key/AAD/decrypt/semantic-identity custody checks.
- Frozen adapter captures the storage frontier before identity reads and passes
  an expected frontier at commit; this guard must remain in the repair.
- Approved SI section 6 requires encrypted relationship payloads, opaque indexes,
  expected-revision review guards, and no plaintext Person foreign key.
- Approved SI section 11 requires a separate authenticated identity envelope,
  random nonce, AAD bound to scope/vault/entity/revision/model context, and
  per-person OS-secure-storage keys in `people_metadata`, separate from
  `voice_biometric`.
- Existing backup/device code confirms reusable crypto/keyring/rand/zeroize
  APIs only; its keys, namespace, and backup envelope are out of scope.

## RootCause

Validation treats an attacker-controlled or stale request-shaped attribution as
trusted identity context. The implementation checks relationship syntax and then
serializes private fields before any authenticated decrypt or native authority
check. A key reference or existing key alone is not authorization. Request
`actor`, ACL, provider label, and key reference must remain claims, never a source
of trust. Plaintext schema FKs and full attribution serialization create durable
leak paths even when a ciphertext field is present.

The independent package root cause is cumulative schema growth beyond the pinned
Genesis resource limit. It prevents runtime proof but does not repair H2.

## WhyEscaped

The candidate tests exercised accepted shapes and fake local ciphertext, not a
real positive Person link through encrypt, persist, reopen, decrypt, and semantic
match. No test established absence of a private sentinel from raw/canonical/event
serialization. The schema install failure stopped the atomicity and reopen tests
before they could expose the durable paths. Native UI, real OS-keyring behavior,
backup recovery, provider, and production data were not exercised.

## ProposedPrevention

### Before / after custody

Before:

`request attribution(provider_label/person_id/acl_subject + placeholder ref)`
`  -> shape validation -> attribution_json + event payload + plaintext FK`

After:

`native trusted context + scope/ownership + lock/revoke/review + frontier`
`  -> authenticated people_metadata envelope and semantic Person check`
`  -> bounded plaintext only -> opaque index/ciphertext + CAS commit`

Raw spoken transcript remains ordinary transcript data in this slice. The private
sentinel used by leak tests is separate from spoken transcript content and must be
absent from the durable and reopened raw store plus serialized/opaque event
output. This does not ban an authorized in-memory decrypted Person result and
does not require encryption of raw speech merely because names can occur in it.

### Minimal approved repair API

1. Add an internal trusted-context capture operation that derives account/scope/
   vault ownership, lock/revocation state, Person-review revision, and the
   capture frontier from native state. Ignore request-supplied actor, ACL,
   provider label, and key reference as authority.
2. Add an identity-envelope operation using the existing crypto dependencies
   and patterns: random nonce, authenticated encryption, AAD for scope/vault/
   entity/revision/model context, opaque key identifier, ciphertext hash, and
   the separate `people_metadata` key namespace. Do not reuse device identity
   or backup keys/envelopes.
3. Make the relationship API accept a protected Person-link reference and
   encrypted payload, not plaintext private fields. Before commit require
   scope/account/vault ownership, unlocked and non-revoked state, current
   review revision, key availability, ciphertext/hash match, authenticated
   decrypt, and semantic Person-link match. Missing/unavailable production
   keys fail closed. Bounded plaintext is zeroized and never logged.
4. Persist only ciphertext plus necessary opaque indexes in Genesis rows and
   emit only opaque relationship metadata in revision/event/audit/backup paths.
   Preserve capture-frontier-before-reads and compare-and-swap at commit.
   Any lock, revoke, or Person-review change invalidates the guarded mutation.
5. Keep v10 backward-compatible and add-only for anonymous/unknown speakers.
   Preserve the approved aggregate v10 plus retained-v11 set. The approved
   local-cap remedy is clone-only Genesis '64 -> 128'; defer that cap and all
   Cargo changes to the shared Dependencyworker. Do not collapse the aggregate
   set, split namespaces, introduce a second database, or add an arbitrary
   renderer path.

### Scoped implementation and tests

Later source lease: `genesis_adapter.rs`, `meeting_intelligence_schema.rs`,
`contracts/meeting-intelligence-v1.yaml`,
`tests/meetingIntelligenceContract.test.mjs`, and the worker remediation report
only. No `lib.rs`, Cargo manifest/lock, app launch, or enrollment work.

Contract-phase tests must execute real encryption/decryption and ciphertext
persistence through an in-memory injected test-key backend; they must not read or
write the real user keyring. Cover:

- positive protected Person link with private sentinel absent from raw/canonical
  source, revision/event serialization, durable rows, and reopen output;
- wrong key, missing key, tamper, AAD/cross-scope swap, wrong vault, placeholder
  reference, caller actor/ACL/provider/keyref injection, and semantic mismatch;
- stale expected revision, frontier CAS conflict, lock, revoke, and Person-review
  changes invalidating the guarded mutation;
- source/raw/canonical/event/cursor atomicity and post-commit behavior;
- v10 add-only/backward preservation and usable anonymous/unknown attribution;
- Genesis install/count proof after the dependency handoff.

These prove only local contract/native-adapter behavior. Native UI, actual OS
keyring integration, backup-key recovery, provider/real-room behavior, and
production readiness remain explicitly `NOT_RUN`.

## Version Diff

- Added candidate v0.1.0b RCA, custody diagram, minimal API, and bounded tests.
- Added corrected evidence boundary for source count `41 + 25 = 66` and install
  failure exit `101`.
- Corrected the local Genesis remedy to clone-only `64 -> 128`, preserving the
  approved aggregate set and deferring cap/Cargo work to Dependencyworker; no
  collapse or namespace split is proposed.
- Clarified that sentinel absence applies to durable/reopened raw storage and
  opaque event output, not an authorized in-memory decrypted Person result.
- No prior report, specification, workflow, source, Cargo file, or build target
  was edited.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-21 | need review / candidate | Initial H2/H1 RCA and approved repair preparation | b336f33ec400a38f003a0665c121069a87a543ac | Dalton / 01a0c1bb-f1a1-7d32-940d-92ba382ebce8 |
| 0.1.1b | 2026-09-21 | need review / candidate | Corrected clone-only Genesis cap remedy and clarified durable sentinel absence | b336f33ec400a38f003a0665c121069a87a543ac | Dalton / 01a0c1bb-f1a1-7d32-940d-92ba382ebce8 |
