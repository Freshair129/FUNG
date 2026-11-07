---
id: null
title: "Incident template — replace before registration"
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: draft
superseded_by: null
attributes:
  domain: incident-management
  doc_type: template
  scope: "Reusable Incident Report with canonical Symptoms"
  language: Thai-first
aliases: []
tags: []
relations:
  issues: []
  rcas: []
ownership:
  owner: null
  reviewer: null
incident:
  status: open
  environment: null
  severity: null
  started_at: null
  detected_at: null
  resolved_at: null
annotations: []
---

# Incident Report — <title>

Template only: copy as `YYYY-MM-DD-topic.md`, allocate an ID under the [ADR](../../decisions/2026-11-01-incident-rca-records.md), replace metadata dates/title/domain/scope/doc_type (`incident-report`), and register in [index](README.md). Placeholders below are instructions, not evidence or valid IDs.

## Summary and current state

Describe the event, current response and confirmed environment. Unknown owner/severity/dates remain null with reasons; do not imply a production outage from a UAT failure.

## Symptoms — อาการที่พบ (canonical)

Repeat a subsection per symptom with an explicit anchor using its complete ID (`<a id="FUNG-INC-NNNNNN-S01"></a>` after replacement).

| Field | Required content |
| --- | --- |
| Symptom ID | `<incident-id>-S01` |
| Observation | What was heard/seen/measured; distinguish user report and instrumentation |
| Expected / Actual | Expected behavior and measured deviation, units and tolerance |
| Trigger | Action and source stream lifecycle, not assumed cause |
| Frequency | Count / denominator, run names and date; unknown when not measured |
| Scope / Impact | Affected mode/app/device/build and bounded impact |
| Reproduction | Ordered steps or link to pinned protocol; prerequisites and ownership |
| Evidence | Linked evidence ID(s), files/manifest and result |
| Related findings | Linked RCA finding IDs, or pending |

## Impact and boundaries

Affected users/data/features; measured duration; known unaffected controls and limits. State whether user or production impact is unknown.

## Detection and timeline

| Timestamp / precision | Event or state transition | Evidence | Actor |
| --- | --- | --- | --- |
| Unknown until established | Do not invent incident start time | Pending | Unassigned |

Use explicit timezone for timestamps, date-only for date-only evidence. Separate occurrence from discovery/reporting time.

## Containment and workaround

Action, owner, actual execution/result, limitations and rollback. Label proposed workarounds as untested. Do not declare mitigation from a different mode's partial success.

## Recovery and follow-up

| Action ID | Work / owner / deadline | Status | Verification or linked RCA action |
| --- | --- | --- | --- |
| Allocate only if owned here | Unassigned / null | proposed | Pending |

## Closure criteria

List required functional and installed/environment gates, evidence, reviewer and remaining failures. Resolution and closure must be explicitly supported.

## Related RCA and evidence

Link RCA, source reports and any incident-owned evidence. Evidence entries need ID/anchor, path, date, provenance, result, hash where verified and scope limits. Local-only artifacts must be labeled. Do not copy an RCA evidence definition under the same ID.

## Annotations

Add structured entries to front matter only when useful:

```yaml
annotations:
  - id: <incident-id>-N01
    target: <incident-id>-S01
    kind: uncertainty
    text: "Describe the qualification, correction or review note"
    evidence_refs: []  # Resolve IDs when supporting evidence exists
```

## Changelog

Record substantive updates and status transitions; preserve prior failed evidence.
