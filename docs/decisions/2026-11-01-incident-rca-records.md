---
id: FUNG-ADR-INCIDENT-RCA-001
title: "Incident and RCA records with stable identity and evidence traceability"
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: beta
superseded_by: null
attributes:
  domain: documentation-governance
  doc_type: decision
  scope: "Incident/RCA records and issue 96 initial adoption"
  language: Thai-first
decision:
  status: approved
  approved_at: "2026-11-01"
  authority: "Project owner instruction in this chat: write ADR, templates and begin documents"
relations:
  issues:
    - https://github.com/Freshair129/FUNG/issues/99
aliases:
  - "Incident RCA documentation standard"
  - "มาตรฐานเอกสารอาการและสาเหตุ"
tags:
  - governance/documentation
  - practice/incident-response
  - practice/root-cause-analysis
---

# ADR: Incident/RCA records

## Context and approval

การสอบสวน #96 มีหลายรายงานและ artifact roots แต่ยังไม่มีทะเบียน Incident/RCA ที่เชื่อมอาการ ข้อค้นพบ สมมติฐาน และงานแก้ไขอย่างถาวร. รายงานทดลองเดิมเป็นหลักฐานตามเวลา ไม่ควรถูกคัดลอกเป็น current truth หลายแห่ง.

ผู้ใช้เลือกโครงสร้างแยก Incident/RCA, เพิ่ม Symptoms และสั่งให้เขียน ADR, templates และเริ่มสร้างเอกสารวันที่ 2026-11-01 (Asia/Bangkok). การอนุมัติครอบคลุมโครงสร้างเอกสาร ไม่ใช่การรับรอง root cause หรือ release ของ #96. Document status `beta` แยกจาก `decision.status: approved`; reviewer ทางเทคนิคยังไม่ถูกแต่งตั้ง.

## Decision

1. เก็บ Incident ใน `docs/verification/incidents/`; RCA ใน `docs/verification/rca/`. แต่ละหมวดมี README เป็นทะเบียน ID/path และ TEMPLATE.
2. Incident เป็น canonical source ของ Symptoms, ผลกระทบ, timeline และการควบคุมเหตุ. RCA อ้างอิง Symptom ID แล้วเป็นเจ้าของข้อค้นพบ สมมติฐาน causal analysis และ corrective/preventive actions.
3. หลักฐานการทดลองยังอยู่ใน [implementation reports](../verification/implementation-reports/README.md) และ artifact roots เดิม. ไม่ย้าย/เขียนทับหลักฐานย้อนหลัง.
4. ใช้ YAML front matter ต่อจาก [CONTRIBUTING](../CONTRIBUTING.md): `id`, `title`, version/dates/status, `attributes`, `aliases`, `tags`, `relations`, `ownership`, `annotations` และสถานะตามชนิดเอกสาร.
5. Index เป็นทะเบียนนำทาง ไม่ทำสำเนารายละเอียดผลทดสอบ. ใช้ preflight เดิมเมื่อมีเครื่องมือ; ไม่เพิ่ม `.doc-graph` หรือ registry database.

## Stable identifiers and ownership

| Entity | Format | Canonical owner |
| --- | --- | --- |
| Incident | `FUNG-INC-NNNNNN` | Incident front matter + incidents index |
| RCA | `FUNG-RCA-NNNNNN-RR` | RCA front matter + RCA index |
| Symptom | `<incident-id>-SNN` | Incident Symptoms |
| Finding / hypothesis / evidence / action | `<rca-id>-FNN/HNN/ENN/ANN` | RCA sections |
| Incident evidence / action | `<incident-id>-ENN/ANN` | Incident sections, when independently needed |
| Annotation | `<document-id>-N` + two digits | Owning document `annotations` |

Examples: `FUNG-INC-000096-S01`, `FUNG-RCA-000096-01-E01`, annotation `FUNG-RCA-000096-01-N01` (suffix N plus two digits).

Allocate the next unused six-digit incident number after checking both indexes and repository definitions. The initial issue 96 record uses reserved `000096`; future numbers are independent of GitHub issue numbers. Serialize allocation through review; reject collisions before merge. Never recycle IDs. RCA `RR` identifies a separate investigation under the incident, not an edit version. Update `version` for edits; a genuinely superseding investigation gets a new RCA ID and `superseded_by` link. File moves retain IDs and update all incoming links, with compatibility pointers where needed.

Aliases are human search names (Thai/English), not identity keys or routing promises. Tags use lowercase `namespace/value`, initially `platform`, `component`, `mode`, `symptom`, `practice`, `governance`; avoid encoding mutable status in tags. Metadata does not itself implement a search engine.

## Annotations and evidence

Each annotation has `id`, `target`, `kind`, `text`, `evidence_refs`; `kind` is `evidence-boundary`, `uncertainty`, `correction`, or `review-note`. Target and evidence IDs must resolve to a record/explicit HTML anchor. Use Markdown relative links in body tables so plain readers can navigate without an ID resolver. Do not replace IDs with line numbers.

Evidence rows include ID, artifact/report path, provenance (revision/source manifest/package hash as applicable), collection date, result, scope and limits. An external local artifact is labeled machine-local and may be unavailable to other readers. Keep SHA-256 of a manifest or artifact where actually computed; never invent a digest. Git HEAD alone is insufficient for a dirty tree. Sanitized portable reports link to immutable local evidence; no raw meeting audio, credentials or personal data are added to Git by default.

## Lifecycle and unknown values

- Document `status` follows CONTRIBUTING; approval/completeness of prose never implies a product pass.
- Incident state: `open → investigating → mitigated → resolved → closed`; reopen to `investigating` if evidence invalidates resolution. `mitigated` requires a demonstrated bounded mitigation; `resolved` requires verified repair; `closed` requires closure review and required acceptance. Record transitions with evidence.
- RCA investigation: `investigating → pending_validation → complete`; reopen on contradictory evidence. Root cause: `unconfirmed`, `supported`, `confirmed`. A hypothesis is `open`, `supported`, `not_supported_in_tested_scope`, or `confirmed`. Never equate “not reproduced here” with globally disproven.
- Use `null` with an explanation for unknown onset/detection dates, owners, reviewers, severity and deadlines. Record timestamps with offsets (or UTC); date-only evidence remains date-only. Do not substitute document creation date for incident start.
- Templates have `id: null` and `attributes.doc_type: template`; placeholders are not registered entities. Replace placeholders before registering a new record.

## Required contents

Incident: summary/state; Symptoms (expected/actual, trigger, frequency, scope, impact, reproduction, evidence, related findings); impact; detection/timeline; containment/workaround; recovery; follow-up and closure; RCA/evidence links; changelog.

RCA: problem/linked Symptoms; environment/provenance; data path and failure boundary; findings; hypothesis/experiment matrix; causal analysis and unknowns; evidence register; corrective/preventive actions with owners/status/verification; acceptance and review; changelog. The same symptom description must not be maintained in both documents.

## Alternatives and consequences

| Option | Benefits | Cost / decision |
| --- | --- | --- |
| One combined report | Few files | Long experiment history obscures response and cause; not selected |
| Separate linked Incident/RCA | Clear ownership, reusable templates, stable references | Selected; requires link/ID validation |
| Database/graph registry | Automated queries | Unnecessary infrastructure and duplicate state for this scope; not selected |

Legacy reports remain valid historical evidence; no repo-wide retrofit. Start with #96 and apply this standard to new records. Existing indexes and platform progress should link to the canonical pair rather than copy their content.

## Verification, review and revisit

Check YAML, required metadata, unique record/annotation IDs, reference targets, local links/anchors, index coverage, digest equality and `git diff --check`. Run available existing preflight without claiming its old JSON snapshot is a fresh check. Templates and case documents must remain navigable without custom tooling. Revisit this ADR if concurrent allocation or evidence volume needs automation.

Links: [Incident template](../verification/incidents/TEMPLATE.md), [RCA template](../verification/rca/TEMPLATE.md), [implementation plan](../plans/2026-11-01-incident-rca-documentation.md).

## Changelog

- 2026-11-01 — 0.1.0: owner-approved structure, Symptoms ownership, stable IDs, evidence annotations and initial #96 adoption. No change to audio acceptance.
