---
id: null
title: "RCA template — replace before registration"
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: draft
superseded_by: null
attributes:
  domain: root-cause-analysis
  doc_type: template
  scope: "Reusable evidence-backed RCA"
  language: Thai-first
aliases: []
tags: []
relations:
  incident: null
  issues: []
ownership:
  owner: null
  reviewer: null
investigation:
  status: investigating
  root_cause_status: unconfirmed
annotations: []
---

# RCA — <title>

Template only: copy as `YYYY-MM-DD-topic-rca.md`, allocate ID and update metadata dates/title/domain/scope/doc_type (`rca`) under the [ADR](../../decisions/2026-11-01-incident-rca-records.md); register in [index](README.md). Replace all placeholders before registration.

## Summary and causal status

State what is established, unknown and not tested. A completed report is not a confirmed cause or release approval.

## Symptoms — references to Incident

| Symptom link | Investigation question | Related finding IDs |
| --- | --- | --- |
| Link canonical `<incident-id>-S01` | What causal boundary needs isolation? | Pending |

Do not maintain a second symptom description here.

## Environment and provenance

OS/build, devices, source app lifecycle, capture mode, sample format, repository HEAD plus dirty-source manifest, installed package/hash, tools/commands/protocol and collection dates. Separate each candidate and native diagnostic from installed acceptance.

## Data path and failure boundary

Show source → transport/native capture → packet receiver → processing → persistence → playback. Mark observations versus inference and instrumentation limits.

## Findings

Each finding uses a full ID/explicit anchor `<rca-id>-F01`, a bounded statement, confidence expressed in words, linked evidence and limitations.

## Hypotheses and experiments

| Hypothesis ID / anchor | Proposed mechanism | Discriminating experiment | Result / evidence | State / remaining uncertainty |
| --- | --- | --- | --- | --- |
| `<rca-id>-H01` | Falsifiable mechanism | Pin inputs and pass/fail gates before running | Not run | open |

## Causal analysis

Trigger, established failure boundary, supported contributing factors, confirmed root cause (or explicitly unconfirmed), alternative explanations and unknowns. Do not force a five-whys chain without evidence.

## Evidence register

Each entry uses `<rca-id>-E01` and a matching explicit anchor. Include report link, artifact/manifest path, measured SHA-256 if available, date, command/protocol link, provenance, result and what it does NOT establish. Keep failed/refused/invalid-control/not-proven results distinct. Preserve immutable external artifacts.

## Corrective and preventive actions

| Action ID / anchor | Action / related symptom or finding | Owner / deadline | Status | Verification / completion evidence |
| --- | --- | --- | --- | --- |
| `<rca-id>-A01` | Bounded causal test or repair | Unassigned / null | proposed | Define gate before execution |

## Acceptance and review

Explicit regression, real environment/package acceptance, independent review and release gates. No silent gap filling, relaxed thresholds or substitute mode claims unless a separate approved contract changes requirements.

## Annotations

Store structured notes in front matter using document-local annotation IDs, resolvable target IDs and evidence_refs per the ADR. Kind: evidence-boundary / uncertainty / correction / review-note. Unknowns are allowed; unbacked factual assertions are not.

## Changelog

Record updates and hypothesis/status changes without erasing failed experiments.
