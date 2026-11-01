---
version: "0.1.0"
created_at: "2026-11-01"
last_update: "2026-11-01"
status: candidate
superseded_by: null
attributes:
  domain: documentation-governance
  doc_type: plan
  scope: "Approved Incident/RCA documentation, templates and issue 96"
  language: Thai-first
---

# Incident/RCA Documentation Implementation Plan

> For agentic workers: use superpowers:executing-plans for this bounded documentation task.

**Goal:** บันทึก ADR, templates และรายงาน #96 ตามโครงสร้างที่ผู้ใช้อนุมัติ พร้อม Symptoms และ traceability.

Tracking issue: [#99 — Incident/RCA documentation](https://github.com/Freshair129/FUNG/issues/99). User subsequently authorized a documentation commit and continued #96 investigation; merge/tag remain gated on both installed modes.

**Architecture:** Incident เป็นเจ้าของอาการและผลกระทบ; RCA อ้างอิงอาการและหลักฐานเดิมเพื่อวิเคราะห์สาเหตุ. ID คงที่ไม่ผูกกับชื่อไฟล์; ไม่สร้างฐานข้อมูลหรือ doc graph เพิ่ม.

**Tech Stack:** Markdown, YAML front matter, existing documentation preflight.

**Spec:** [ADR — Incident/RCA records](../decisions/2026-11-01-incident-rca-records.md); ผู้ใช้อนุมัติการแยกรายงาน เพิ่ม Symptoms และให้เริ่มสร้างเอกสารในแชตวันที่ 2026-11-01 (Asia/Bangkok).

## Global constraints

- Preserve existing product work, historical reports and external evidence bytes.
- Keep document status separate from incident/investigation/root-cause status.
- Unknown dates, owners and causal claims remain explicitly unknown.
- No new audio run, package qualification, commit, merge or release in this task.

## Review focus

- ID collisions and dangling annotation/symptom/evidence references.
- Diagnostic evidence incorrectly promoted to installed acceptance.
- Duplicated symptom ownership or metadata/body contradictions.
- Machine-local evidence confused with portable repository links.
- Template placeholders accidentally treated as registered records.

## Tasks and verification ledger

- [x] Task 1: Write `docs/decisions/2026-11-01-incident-rca-records.md`; extend `docs/CONTRIBUTING.md`; create `docs/verification/{incidents,rca}/{README,TEMPLATE}.md`. Check metadata, ID policy, lifecycle and index links.
- [x] Task 2: Write dated #96 Incident/RCA in their respective directories. Reconcile the current upstream and installed reports; verify external manifest hash. Keep root cause unconfirmed.
- [x] Task 3: Link ADR, plan and verification indexes; run existing preflight when available, scoped ID/link/front-matter checks and `git diff --check`; obtain a fresh documentation review and record actual results here.

Ruling: documentation-only changes need structural/evidence checks, not application builds or audio retests. Existing dirty product files remain untouched. No automatic commit of unrelated work.

## Verification result — 2026-11-01

- Scoped checker: 12 YAML documents parsed; 26 unique document/anchor/annotation definitions; 4 annotations resolve; 131 local links and 22 anchors checked, zero errors. Repository front-matter ID definitions checked for collisions with this scope. Template placeholders excluded from registration.
- External completed A/B/B/A manifest SHA-256 recomputed and matches the RCA evidence entry. No raw audio rerun or universal evidence rehash claim.
- `git diff --check` passed; existing CRLF conversion warnings are unrelated to documentation correctness.
- Fresh read-only documentation review found one qualification issue: PCM16 leading frames are near-zero, not necessarily exact zero. Corrected F01 to distinguish Float32 exact zeros from PCM16 magnitude ≤1. No new product acceptance claim.
- The checkout contains historical preflight snapshots/pointers but no executable documentation preflight entry point was found in the inspected repository scripts/config. Those snapshots were not represented as fresh runs. A bounded temporary checker was used instead, with PyYAML 6.0.3 isolated under `C:\Users\suanr\AppData\Local\Temp\fung-rca-doccheck-20261101`; no app dependency change.
- Reproduction on this machine: bundled Python `C:\Users\suanr\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe` runs `C:\Users\suanr\AppData\Local\Temp\fung-rca-doccheck-20261101\check.py`; receipt `result.json` beside it. Temporary paths are local and not guaranteed durable; this ledger records the bounded result, not a new CI gate. Remote URLs, legacy repo-wide link completeness and runtime behavior are outside scope.
