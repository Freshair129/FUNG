---
version: "0.1.0b"
created_at: "2026-10-19"
last_update: "2026-10-25"
status: "beta"
superseded_by: null
attributes:
  domain: "product-specification"
  doc_type: "documentation-index"
  scope: "FUNG feature and technical specifications"
  language: "Thai-first"
---

# FUNG specifications

Specifications refine requirements into observable behavior, technical boundaries and acceptance criteria. A dated spec marked `draft` or `candidate` is a proposal; it does not authorize implementation by itself.

## Sources and authority

- Start with [requirements](../requirements/README.md) for cross-platform canonical IDs.
- Some existing Desktop requirement IDs remain owned by [Live Meeting External Retrieval Requirements](../Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md); that file identifies its scope and migration boundary.
- Use [decisions](../decisions/README.md) for accepted architectural choices and [plans](../plans/README.md) for execution sequencing.
- Use the platform status docs and [verification index](../verification/README.md) to establish what is implemented and evidenced.

## Current feature-spec areas

| Area | Relevant specifications |
| --- | --- |
| Meeting capture and transcription | [Live meeting transcription](2026-09-21-live-meeting-transcription-spec.md), [transcript pipeline adaptation](2026-09-21-fung-meeting-transcript-pipeline-adaptation.md), [model profiles](2026-09-21-whisper-model-profiles.md) |
| Meeting knowledge and agents | [Knowledge evidence](2026-09-21-meeting-knowledge-evidence-spec.md), [agent participation](2026-09-21-meeting-agent-participation-spec.md), [Lalin workflow adaptation](2026-09-25-lalin-meeting-agent-workflow-adaptation.md) |
| Speakers and diarization | [Speaker identity domain](2026-09-21-speaker-identity-domain-design.md), [pyannote runtime](2026-09-21-pyannote-diarization-runtime.md) |
| Thai ASR qualification | [Thai ASR model selection](2026-09-30-thai-asr-model-selection-qualification.md) |
| Product MCP | [Desktop Product MCP v2 contract](2026-10-20-product-mcp-v2-contract.md) — v0.1.4b accepted; MCP-02 loopback bridge exists in source, while packaged/client UAT remains open |
| Platform and external services | See dated Zoom, pairing, FUNGWIRE, backup, Google Drive and BYOM specs in this directory; check each file's status and source links. |

## Authoring rules

- Name new dated specs `YYYY-MM-DD-<feature>-spec.md` or `YYYY-MM-DD-<feature>-design.md`.
- State requirement IDs, actor, behavior, non-goals, data/privacy impact, error states and measurable acceptance.
- Keep target-state proposals distinct from current behavior; cite runtime evidence only from verification reports.

## Additional dated specifications

- [2026-08-05-zoom-meeting-ingestion-design](2026-08-05-zoom-meeting-ingestion-design.md) — `approved-design`
- [2026-08-08-auth-web-hybrid-subproject-a-design](2026-08-08-auth-web-hybrid-subproject-a-design.md)
- [2026-08-08-byom-tts-provider-design](2026-08-08-byom-tts-provider-design.md)
- [2026-08-09-phase-1-pairing-desktop-login-design](2026-08-09-phase-1-pairing-desktop-login-design.md)
- [2026-08-09-phase-2-fungwire-design](2026-08-09-phase-2-fungwire-design.md)
- [2026-08-09-phase-3-byom-cloud-keys-design](2026-08-09-phase-3-byom-cloud-keys-design.md)
- [2026-08-11-live-meeting-external-retrieval-design](2026-08-11-live-meeting-external-retrieval-design.md) — `candidate`
- [2026-08-13-phase-4-cloud-backup-mobile-account-design](2026-08-13-phase-4-cloud-backup-mobile-account-design.md) — `superseded`
- [2026-08-13-phase-4-cloud-backup-mobile-account-requirements](2026-08-13-phase-4-cloud-backup-mobile-account-requirements.md) — `beta`
- [2026-08-14-desktop-live-meeting-public-release](2026-08-14-desktop-live-meeting-public-release.md) — `beta`
- [2026-08-23-google-drive-authority-schema-amendment](2026-08-23-google-drive-authority-schema-amendment.md) — `beta`
- [2026-08-23-google-drive-native-authorization-amendment](2026-08-23-google-drive-native-authorization-amendment.md) — `beta`
- [2026-08-23-phase-4-google-drive-oauth-iam-handshake-spec](2026-08-23-phase-4-google-drive-oauth-iam-handshake-spec.md) — `beta`
- [2026-08-24-enrollment-proof-nonce-amendment](2026-08-24-enrollment-proof-nonce-amendment.md) — `beta`
- [2026-08-24-native-session-broker-amendment](2026-08-24-native-session-broker-amendment.md) — `candidate`
- [2026-08-25-native-session-broker-production-path-remediation-amendment](2026-08-25-native-session-broker-production-path-remediation-amendment.md) — `candidate`
- [2026-08-25-native-session-broker-registered-entrypoint-evidence-amendment](2026-08-25-native-session-broker-registered-entrypoint-evidence-amendment.md) — `candidate`
- [2026-08-26-d-mvp-01-packaged-whisper-path-remediation](2026-08-26-d-mvp-01-packaged-whisper-path-remediation.md) — `candidate`
- [2026-08-26-fung-mvp-critical-path-amendment](2026-08-26-fung-mvp-critical-path-amendment.md) — `candidate`
- [2026-08-26-phase-4-w2-e1-hyper-v-vm-provisioning-amendment](2026-08-26-phase-4-w2-e1-hyper-v-vm-provisioning-amendment.md) — `candidate`
- [2026-08-26-phase-4-w2-external-evidence-execution-amendment](2026-08-26-phase-4-w2-external-evidence-execution-amendment.md) — `candidate`
- [2026-10-01-python-runtime-dependabot-remediation](2026-10-01-python-runtime-dependabot-remediation.md) — `candidate`
- [2026-10-20-product-mcp-v2-contract](2026-10-20-product-mcp-v2-contract.md) — `accepted`; paired with `contracts/product-mcp-v2.yaml`

- [Dedicated endpoint routing design](2026-10-25-dedicated-endpoint-routing-design.md) — `candidate`; existing-device inventory and native feasibility before product integration or release.
