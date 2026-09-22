---
version: "0.2.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG AI ethics and governance"
---

# AI Ethics and Governance

## Principles

- Local-first by default; minimise collection and external transfer.
- A transcript, diarization result, summary, or action candidate is evidence
  produced by a system, not automatically verified truth.
- Human authority is required for sensitive identity claims, consequential
  decisions and external delivery. Proposed bounded meeting-publication policies
  are approved explicitly by a human; out-of-policy/sensitive payloads and
  corrections that alter meaning still require review. Legacy per-call rules remain.
- The UI must expose model origin, timestamp, review state, and degraded or
  unavailable conditions.
- Consent, purpose limitation, retention, deletion, and access must be
  explicit for meeting audio and derived artifacts.

## Speaker identity and voice profiles

Voice recognition or persistent speaker profiles are a separate high-risk
capability. They require a dedicated approved requirement and design covering
consent, enrolment, false-match handling, access, deletion, retention, and
prohibited uses. Diarization may label an anonymous speaker segment without
claiming a real-world identity.

## Meeting participation and publication — candidate controls

[Meeting Agent](../specs/2026-09-21-meeting-agent-participation-spec.md) and [knowledge evidence](../specs/2026-09-21-meeting-knowledge-evidence-spec.md) separate consent for recording, external media access, recognition, knowledge read, artifact sharing and spoken output. One permission cannot stand in for another.

- Agent presence/recording must be visible; a notice is not itself proof of valid consent.
- API-provided names are platform labels. They do not prove a real person, allow biometric enrollment or grant access to financial/personnel documents.
- Contextual mentions may produce private suggestions. Automatic publication needs an explicit session policy and deterministic audience/data-class checks; transcript or document prompt injection cannot authorize it.
- Readable business data may still be unshareable with meeting guests. Unknown chat audience blocks confidential auto-sharing.
- Local ASR behind a cloud bot is not an all-local system. Setup must explain media recipients, region, retention, cancellation and external-copy limits.
- Voice identity templates never become TTS voices. Agent speech must not impersonate participants.
- Stop/revoke prevents new eligible actions; already delivered/remote copies require transparent best-effort cleanup, not a false promise of erasure.

These are proposed controls requiring privacy/security review and evidence; they are not legal compliance certification or authorization to record a real meeting.

## Risk register

| Risk | Preventive control | Detection | Response |
|---|---|---|---|
| Hallucinated summary/action | evidence refs, review state, structured output | human review and contradiction checks | mark degraded, correct with provenance |
| Wrong speaker attribution | anonymous labels by default, confidence/overlap handling | labelled evaluation set | remove identity claim, re-review |
| Unauthorised egress | default-deny capability policy and minimisation | audit/secret scans | deny, revoke, investigate |
| Sensitive data over-retention | explicit retention and deletion path | periodic inventory | delete/contain, record incident |
| Model/provider drift | pinned versions and lifecycle gates | regression evaluation | hold promotion or rollback |

## Human review record

For reviewed artifacts, record reviewer role, review time, decision, changed
fields, evidence refs, and unresolved uncertainty. Do not overwrite the raw
model result without retaining the provenance link.

## Incident handling

Privacy, identity, security, or materially misleading-output incidents follow
the repository RCA rule: symptom, evidence, root cause, why detection failed,
and prevention. External delivery is paused when approval or provenance cannot
be established.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Added independent meeting-media/share consent, non-human identity, bounded publication authority and remote-copy disclosures. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-09-21 | candidate | Added independent meeting-media/share consent, non-human identity, bounded publication authority and remote-copy disclosures. | working-tree | RWANG |
| 0.1.0b | 2026-08-23 | candidate | Added AI ethics, speaker identity, and risk controls. | pending | ATHER |
