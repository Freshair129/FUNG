---
version: "0.2.0b"
created_at: "2026-08-23T01:15:22+07:00,ATHER"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "ai-ml-governance"
  doc_type: "complexity-rule"
  scope: "FUNG audio and AI data lineage"
---

# AI Data Pipeline and Lineage

## Logical flow

```text
source audio
  → capture metadata and durable chunks
  → normalised audio / preprocessing
  → speech-to-text segments
  → optional diarization / speaker hypotheses
  → reviewed transcript
  → meeting intelligence and artifacts
  → local persistence / export / approved external delivery
```

The flow describes the governed boundary. A line in this document is not
evidence that the corresponding worker or UI is complete.

## Data classes

| Data | Classification | Required controls |
|---|---|---|
| Raw audio | Sensitive | local-first storage, consent, retention, access control |
| Audio-derived features | Sensitive | minimise, document purpose, avoid unnecessary export |
| Transcript | Confidential/sensitive | provenance, review state, scoped access, redaction where needed |
| Speaker hypothesis/profile | Sensitive/biometric-risk | opt-in design, explicit purpose, no silent identity claim |
| Summary/action candidates | Confidential | model provenance, human review, distinguish candidate from fact |
| Run manifest | Internal | model/runtime/config/hash, no secrets or raw content by default |

## Lineage requirements

Each artifact should be traceable to:

- source asset identifier and capture timestamp;
- preprocessing and segmentation configuration;
- model name, version, runtime, device, and quantization;
- prompt/template version for generated text;
- parent job and retry/attempt identifier;
- human edits, review state, and export/delivery decision.

## Egress rules

- Raw audio and full transcript remain local unless a separately approved
  egress requirement exists.
- External payloads contain only approved fields and selected evidence refs.
- Credentials never appear in GenesisBlockDB records, logs, exports, prompts,
  model output, or test snapshots.
- If a worker fails, the pipeline records an unavailable/degraded state and
  preserves durable local capture; it must not fabricate completion.

## Live meeting / knowledge / publication lineage — candidate

Detailed owners: [Live transcript](../specs/2026-09-21-live-meeting-transcription-spec.md), [Knowledge](../specs/2026-09-21-meeting-knowledge-evidence-spec.md), [Meeting Agent](../specs/2026-09-21-meeting-agent-participation-spec.md).

```text
source session + participant/track generation + media clock/coverage
 -> ASR model run -> utterance revision -> committed transcript cursor
 -> trigger -> selected collection/index snapshot -> exact document versions
 -> evidence bundle + claim/cell locators -> draft payload/hash
 -> grant + audience snapshot + final eligibility check
 -> outbox attempt -> provider receipt / delivery_unknown
```

Partial text is ephemeral and never public evidence. Corrected transcript, participant mapping, document version or ACL creates a new revision/invalidation; do not mutate the lineage of already sent content.

| New data class | Controls |
| --- | --- |
| Provider participant/session metadata | protected personal metadata; source label != confirmed human identity |
| Knowledge chunks/metric observations | source ACL/classification, exact source hashes and locator, no arbitrary execution |
| Publication artifact | separate re-share permission, redaction/hash, audience/expiry and remote-retention disclosure |
| Gateway transport/control state | minimal encrypted bounded media buffer plus stop/reconciliation journal; not a second corpus |
| Delivery receipts | room/message IDs, accepted vs delivered vs unknown; no secret tokens |

Managed meeting media can pass through a third-party bot even when ASR remains local. Record that path explicitly in the [egress register](../appendices/E-egress-register.md); no cloud fallback or full-corpus transfer is implied. Voice embeddings/enrollment samples stay excluded. Google Chat is not a substitute destination for Meet chat.

Proposed retention defaults and delete/revoke behavior are in the owning specs. Removing local data or a link cannot guarantee remote recipients/provider backups delete copies; retain content-free cleanup outcomes and disclose unverified deletion.

## Retention and deletion

Retention is a product/privacy decision, not a model default. Each deployment
must record the retention period, deletion authority, backup implications, and
whether derived artifacts are deleted with the source audio.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Added source-to-transcript-to-knowledge-to-delivery lineage and explicit managed-media/remote-copy boundaries. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.0b | 2026-09-21 | candidate | Added source-to-transcript-to-knowledge-to-delivery lineage and explicit managed-media/remote-copy boundaries. | working-tree | RWANG |
| 0.1.0b | 2026-08-23 | candidate | Added AI data lineage, classes, and egress rules. | pending | ATHER |
