---
version: "0.1.0b"
created_at: "2026-09-21T03:36:16+07:00,RWANG,base-b336f33"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "knowledge-and-evidence"
  doc_type: "feature-spec"
  scope: "Meeting-scoped retrieval, business facts, citations and shareable artifacts"
  language: "Thai"
---

# FUNG — Meeting Knowledge & Evidence Specification

## 1. Outcome and current boundary

เมื่อบทสนทนากล่าวถึง “ยอดขายปีที่แล้ว” FUNG ค้นเฉพาะคลังที่เลือก ตรวจว่าหมายถึงบริษัท/ช่วงเวลา/นิยามใด เสนอคำตอบพร้อมอ้างอิง และสร้างเอกสารหรือลิงก์ที่มีสิทธิ์แชร์ได้ ไม่ค้นเจอเพียงชื่อไฟล์แล้วแต่งตัวเลขขึ้นมา

C-3 / HIGH; candidate documentation only. [Domain map](../architecture/MEETING_INTELLIGENCE_DOMAINS.md) assigns this to D11; [Meeting Agent](2026-09-21-meeting-agent-participation-spec.md) owns triggers; D13 owns delivery.

Source inspected: `meeting_intel.rs` has older `meeting_ask` and a separately guarded `meeting_ask_recording`. The recording-scoped answer explicitly excludes graph/live tail; it is not a general enterprise document search engine. Current external MCP allowlist has `documents.search`, `documents.get_metadata`, `crm.customer_status.read` — **no general content download or arbitrary SQL capability**.

Do not broaden existing recording-scoped QA. Add an explicit knowledge-collection query route, with different scope shown in UI and response provenance.

## 2. Product scope and corpus

MVP sources: explicitly imported local PDF/text/Markdown and supported document/table extracts with parser/version manifest; approved recordings as opt-in knowledge sources; manually attached meeting reference documents. DOCX/XLSX/CSV support must be qualified per parser and declared in capability results, not implied by accepting any extension.

External systems are connectors behind separately approved read scopes. This spec does not reinstate the canceled Google Drive backup/provider workstream. A future Drive knowledge connector would be a distinct requirement/approval, not reuse historical backup credentials.

No default whole-disk scan, all-meetings search, unsandboxed macros/formulas, email crawl, hidden cloud embedding, web search or user impersonation.

## 3. Supporting profiles and permissions

| Profile | Required fields / owner |
| --- | --- |
| Local account/vault | authenticated local owner, unlocked state; D1 |
| Person | optional display name and reviewed provider link; D2; not an access token |
| Workspace/business context | organization IDs, timezone, language, fiscal calendar/version, currency presentation, permitted collections; explicit user settings |
| Knowledge collection | owner/vault/project scope, classification, source roots/connectors, index mode, retention, share policy |
| Meeting context | occurrence, meeting date/timezone, chosen organizations/collections, audience rule, maximum classification |
| Agent policy | read collections, query limits, allowed output destinations, trigger mode, expiry; D8/D12 |

Example: “ปีที่แล้ว” is resolved relative to the meeting's local date, not the date a catch-up job runs. Display Buddhist/calendar year conversion explicitly where used. If fiscal vs calendar year is ambiguous, ask or show alternatives; do not silently assume company financial settings.

Read rights and share rights are independent. Host status, API participant name, email-like display text and voice match do not grant access to business data.

## 4. Functional requirements

| ID | Contract | Acceptance |
| --- | --- | --- |
| KE-01 | User selects collections/documents per session; default current recording and explicit references only | unrelated vault/project/document never appears |
| KE-02 | Index immutable document versions with content hash/parser version and source locator | every citation resolves to exact bytes/version |
| KE-03 | Extract text/tables with page/section/cell lineage; retain extraction warnings | OCR/parse failure yields unavailable/partial, not fabricated content |
| KE-04 | Filter by actor read ACL before retrieval/ranking and model context construction | denied content/title/snippet not leaked in results/logs |
| KE-05 | Retrieve bounded candidates using local lexical search first; optional local embeddings through D9 | embedding failure can degrade; no second vector database |
| KE-06 | Treat retrieved material as data; reject tool/policy instructions embedded in documents | prompt-injection corpus cannot change grants/destination |
| KE-07 | Resolve company, metric, period, unit, actual/budget and version for quantitative questions | ambiguous request returns needs_clarification |
| KE-08 | Calculate derived values with deterministic typed arithmetic and lineage | no LLM-generated arithmetic without operands/formula |
| KE-09 | Build answer claims each linked to selected source spans/cells | unsupported claims removed or marked unknown |
| KE-10 | Revalidate source version, revocation, expiry and sharing rights at publication | deleted/restricted source blocks unsent answer |
| KE-11 | Build minimized publication artifact/link separately from retrieval payload | no full document upload implied by answering one metric |
| KE-12 | Invalidate caches/evidence bundles on source change, delete, ACL or policy revision | stale results cannot be freshly published |
| KE-13 | Expose index health and unavailable-source reasons without disclosing forbidden metadata | partial corpus is visible to authorized operator |
| KE-14 | Preserve exact query scope/provenance, bounded budgets and cancellation | current recording QA remains recording-scoped |

## 5. Ingestion, indexing and update lifecycle

```text
selected source -> custody + hash -> bounded extraction -> source ACL
 -> chunks + table observations -> index run -> query snapshot
 -> evidence bundle -> grounded draft -> share-policy check -> D13
```

Document: `importing → extracting → indexing → ready | partial | failed`; later `superseded | revoked | deleted`.
Index run has input version set, parser/preprocessor/embedding fingerprint, status and coverage. Never mark an entire collection ready because one file succeeded.

- File path is resolved inside selected custody roots; validate symlink/junction escape, extension and detected MIME, size/page/row limits before processing.
- Defaults proposed for qualification: 25 MiB/file, 500 pages or 100,000 table rows, 60-second parser task with bounded memory. Larger files require explicit batch import, not hanging live capture.
- PDF OCR is optional separate local job with OCR provenance; do not claim scanned pages readable when OCR absent.
- Office macros, embedded scripts and external links are not executed. Spreadsheet formula cells use validated cached values or approved deterministic evaluator; missing/stale calculation is disclosed. No external workbook refresh from an agent prompt.
- Native graph/vector/relational indexing must use qualified Genesis interfaces. Lexical-only is a valid initial implementation; no claim a current vector index exists.
- Changes produce a new version and index generation. Reads use one consistent snapshot; replacement never mixes old title with new table cells.
- Revocation immediately makes sources ineligible, then purges derived index/cache/artifacts according to retention. Old sent copies cannot be guaranteed recallable.

## 6. Proposed data model

All IDs are vault-scoped, globally unique local IDs. Secret tokens never stored in these rows.

| Aggregate | Minimum fields / invariant |
| --- | --- |
| knowledge_collections | id, vault/project, owner, classification, read/share policy refs, revision, status |
| knowledge_documents | collection, source kind/ref, title, current version, ACL revision, status |
| knowledge_document_versions | document, content hash, custody ref, MIME, source modifiedAt, ingestedAt, parser version, validity/asOf |
| knowledge_chunks | version, chunk ID, text ref, locator, token/byte bounds, extraction quality, index generation |
| knowledge_metric_observations | version, metric key, organization, period start/end/calendar, decimal value, unit/currency/scale, actual/budget, locator |
| knowledge_index_runs | input versions, model/config fingerprints, state, coverage/errors, timings |
| knowledge_evidence_bundles | query/trigger refs, policy+ACL snapshot, selected claim/source refs, output hash, state, expiresAt |

Metric observations are provenance-backed extracted records, not a new authoritative accounting ledger. Extraction confidence is not permission and is not proof of business correctness.

Citation locator variants: page+bounding box or line/span; heading+paragraph; sheet+cell range; recording+segment revision+time range. Export includes document/version/hash/locator and retrieval time, with link access policy.

## 7. Query and evidence contracts

Proposed `knowledge_search` input:

| Field | Meaning |
| --- | --- |
| meetingSessionId, requestId | owned session and dedup key |
| query / triggerRef | bounded user query or committed transcript reference |
| collectionIds, organizationIds | explicit subsets of session grant |
| scope | meeting_references / selected_collections; never implicit all |
| temporalContext | meeting date, timezone, fiscal calendar revision |
| filters | metric/year/classification/source type; policy may only narrow |
| maxResults, deadlineMs | bounded server-enforced budgets |

Response: `status=found|not_found|needs_clarification|partial|denied|unavailable`, `evidenceBundleId`, `indexSnapshotId`, `sources[]`, `warnings[]`, `searchedScope`, `latencyMs`. Model answer is separate from search status.

Each evidence item includes read decision ref, document/version/chunk IDs, locator, content hash, bounded excerpt or typed metric, extraction quality, validity/asOf and sharing classification. Denied content is excluded, not placed in the prompt with “do not reveal” instructions.

Proposed `knowledge_answer_draft` returns claim list, source refs, open ambiguities, deterministic computation refs, model provenance and `shareEligibility`. It has **no send side effect**. `knowledge_prepare_publication` generates a redacted artifact preview/hash for D13.

Defaults: at most 8 evidence chunks / 12,000 context characters, 3 local retrieval attempts, 10-second local lookup deadline; configurable within session caps. External retrieval retains its own approved timeout/budget and exact egress minimization.

## 8. Sales example — normative behavior

Scenario: meeting date **2026-09-21 Asia/Bangkok**, selected company A, calendar-year context. User says “ยอดขายปีที่แล้วเท่าไร เอารายงานมาดูหน่อย”.

1. Trigger references committed transcript revision, not a partial “ปีนี้/ปีที่แล้ว” hypothesis.
2. Resolve proposed period 2025-01-01 through 2025-12-31. If workspace uses fiscal year, ask which period unless meeting policy already explicitly defines it.
3. Identify metric: revenue/net sales/gross sales/bookings, actual vs forecast, currency/scale. “ยอดขาย” without an agreed dictionary can require clarification.
4. Search approved company-A collection and reference exact approved report revision; never mix company B or earlier unaudited draft.
5. If two sources disagree, show the conflict and source dates; do not choose the larger number or average them.
6. Produce “ตามรายงาน … ฉบับ … รายได้สุทธิของบริษัท A ปีปฏิทิน 2025 เท่ากับ … บาท” only when evidence contains that value. This document intentionally supplies no fictional sales number.
7. For growth, use `(current - previous) / previous × 100` with decimal operands/rounding and matching metric/currency/period; zero/missing denominator yields not_computable.
8. Create a short citation/link or approved page/table excerpt. Check audience may receive it; if guests are not entitled, keep draft private and explain the block without posting confidential source titles.
9. D13 posts in the bound Meet chat only with current grant and capability. Delivery records exact report version, payload hash and receipt state.

Mere mention “ปีที่แล้วยอดขาย…” can prefetch a private suggestion. Posting without another click requires the separately enabled proactive session policy, not the phrase itself.

## 9. Artifact/link and audience policy

| Output | Required control |
| --- | --- |
| Private local answer | actor read permission; no external upload |
| Existing provider document link | approved host/URL, current source version, audience access checked where API permits |
| FUNG generated excerpt | page/table/claim subset + citation manifest + hash + explicit redaction preview |
| Full original file | separate attachment grant and source re-share authority; not default |
| Secure share link | authenticated viewer/allowlisted audience, expiry/revoke and access log; no public-by-default object |
| Native chat attachment | adapter capability + MIME/size limits + upload/publish receipts; unsupported stays unsupported |

Sending a local `C:\...` path is not document delivery. Bearer links can be forwarded; do not call them private audience enforcement. For confidential sources prefer authenticated viewer links; source ACL visibility may be unavailable, which blocks automatic publication rather than assuming everyone is authorized.

Meet audience may include anonymous guests and chat visibility may persist beyond current call. Use actual destination policy, not just active-speaker roster. Unknown members => deny confidential auto-share. Granting read access to a file is a separate action not part of MVP; agent does not change source permissions.

Proposed generated-link default expiry is 24 hours after meeting end; local preview shows expiry. Retention, recipients and provider copy behavior are disclosed before publication.

## 10. Security, retention and failures

- Encrypt private source/excerpt payloads before entering application persistence/WAL where required by the vault data policy; OS secure store owns local keys. Indexes inherit classification and are not exported in generic logs.
- Credentials live in keyring/server secret manager appropriate to deployment, never prompt/UI event bodies.
- Source URLs use allowed schemes/hosts and revalidate redirects; block localhost, metadata endpoints, private-network SSRF and file URLs on remote fetch paths.
- No remote embeddings/LLM by default. Approved cloud use previews selected excerpts, model endpoint and retention, not only “use AI”.
- Local extraction or indexing failure does not affect meeting capture; unavailable result is preferable to an uncited answer.
- Raw retrieval excerpts cached only for active job or encrypted evidence bundle retention; proposal default evidence drafts expire 24h after meeting, delivery/audit metadata 30d, source files follow collection policy. Published artifact retention is D13 policy and can differ.
- Deleting a collection invalidates retrieval, derived indexes and unsent bundles; retain content-free audit tombstones as policy permits. Backup deletion limitations must be shown.

## 11. Acceptance and rollout

| Test | Expected result |
| --- | --- |
| KE-T01 selected project/vault isolation | no foreign snippet/title/count inference |
| KE-T02 same filename, changed bytes | exact version citation; prior draft becomes stale |
| KE-T03 Thai year/fiscal/company/currency ambiguity | clarifies before publishing a numeric claim |
| KE-T04 contradictory reports / missing figures | source conflict or not_found, no invented arithmetic |
| KE-T05 PDF OCR / spreadsheet external formula / malicious document | bounded failure; no code/network execution |
| KE-T06 source ACL revoke while answer generates | generation/publication canceled; no content sent |
| KE-T07 guest enters / audience unknown / link forwarded | confidential auto-share denied; link policy enforced |
| KE-T08 citation click/manifest on exported excerpt | exact authorized page/cells/version resolvable |
| KE-T09 source delete/reindex vs queued delivery | final eligibility recheck blocks stale payload |
| KE-T10 model offline / optional vector absent | lexical retrieval or honest unavailable; capture unaffected |
| KE-T11 old recording QA regression | scope remains recording; no collection lookup through old command |
| KE-T12 existing MCP content download request | denied unless a new separately approved capability exists |

Rollout: imported meeting references → selected collections with citations → typed numeric fixtures → approved share artifacts → approved external content connector. DoD requires parser/security/ACL/quality tests plus real-source and audience UAT. No source provider, connector or numerical accuracy has been validated by this documentation turn.

## Version Diff

| Version | Change |
| --- | --- |
| 0.0.0 → 0.1.0b | Added scoped knowledge ingestion/retrieval, profile context, citations, sales-metric resolution and share eligibility. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-09-21 | candidate | Proposed knowledge evidence domain and detailed acceptance; documentation only. | working-tree; base b336f33 | RWANG |
