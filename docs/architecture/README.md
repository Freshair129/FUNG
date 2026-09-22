---
version: "0.2.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "architecture-governance"
  doc_type: "core-directive"
  scope: "FUNG architecture views"
---

# FUNG Architecture Views

This directory is the index for architecture-level views. It prevents a new
architecture proposal from being hidden inside a feature document while
preserving the existing current-state source.

## Canonical views

| View | Canonical document | Truth boundary |
|---|---|---|
| Current product architecture | [`../Desktop/ARCHITECTURE.md`](../Desktop/ARCHITECTURE.md) | Current FUNG parent architecture |
| Desktop implementation status | [`../Desktop/08-real-progress.md`](../Desktop/08-real-progress.md) | Evidence-backed current state |
| Mobile implementation status | [`../Mobile/IMPLEMENTATION_STATUS.md`](../Mobile/IMPLEMENTATION_STATUS.md) | Evidence-backed mobile state |
| AI/ML runtime and governance | [`../ai-system/README.md`](../ai-system/README.md) | Model/data/evaluation controls |
| Contracts | [`../../contracts/`](../../contracts/) | API, MCP, CLI, and data boundaries |
| Decisions | [`../decisions/`](../decisions/) | Approved or candidate choices |

## Meeting intelligence extension — candidate

Start with [Meeting Intelligence Domain Map](MEETING_INTELLIGENCE_DOMAINS.md): D1–D9 People/Voice support plus D10 Live Transcription, D11 Knowledge, D12 Meeting Agent and D13 Conversation Delivery.

Detailed contracts:

- [Live transcription](../specs/2026-09-21-live-meeting-transcription-spec.md)
- [Knowledge and evidence](../specs/2026-09-21-meeting-knowledge-evidence-spec.md)
- [Meeting Agent participation](../specs/2026-09-21-meeting-agent-participation-spec.md)
- [People and Voice Identity](../specs/2026-09-21-speaker-identity-domain-design.md)
- [Google Meet API strategy](../decisions/2026-09-21-google-meet-agent-api-strategy.md)

These are design contracts, not deployment or real-meeting evidence. Provider API identity, confirmed Person identity and publication authority are separate.

## Architecture record rules

- Label current state, target state, and historical evidence separately.
- Use a decision record when selecting among materially different approaches.
- Use a design/specification when behavior or contracts change.
- Keep deployment, security, data, and AI runtime impacts explicit.
- Do not call a proposal implemented or production-ready without current
  verification evidence.

## Recommended views for a substantial change

1. Context and actors
2. Container/runtime boundaries
3. Data and trust boundaries
4. State/job lifecycle
5. Deployment and hardware profile
6. Failure, rollback, and observability paths

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Indexed the integrated meeting domains and detailed Google Meet/live/knowledge/agent contracts. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-09-21 | candidate | Indexed the integrated meeting domains and detailed Google Meet/live/knowledge/agent contracts. | working-tree | RWANG |
| 0.1.0b | 2026-08-23 | candidate | Added architecture-view index and current/target boundary rules. | pending | ATHER |
