---
version: "0.1.1b"
created_at: "2026-08-12T01:19:19+07:00,Agent: ATHER"
last_update: "2026-08-12T01:27:48+07:00,Agent: ATHER"
status: "need review"
superseded_by: null
attributes:
  domain: "documentation-governance"
  scope: "FUNG hard-finding documentation reconciliation"
  doc_type: "fix-report"
---

# Task 4 — Hard-Finding Documentation Reconciliation

**Status:** DONE_WITH_CONCERNS
**Project:** FUNG
**Completed:** 2026-08-12 01:19 ICT
**Change risk:** MEDIUM — cross-document governance corrections only; no application behavior changed.

## Evidence Boundary

This task used the current working-tree Sprint 4 evidence already verified by
Task 3: the registered eight-command Tauri boundary, embedded default-off
operator UI, `npm run test:external-tools` result of 5/5, and direct graph-edge
recount. Code presence is recorded as implementation evidence only; it does
not close a release, connector, device, visual, or regression gate.

## Finding Disposition

| Finding | Disposition | Changed path | Evidence-preserving result |
|---|---|---|---|
| H1 — requirements/status contradiction | FIXED | `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md` | FR-106--FR-114 and FR-116 now identify current default-off code-level backend/operator evidence. Their traceability mapping likewise names the current surface. The rows retain open visual/keyboard, restart, artifact-secret-scan, real-device capture-isolation, real-connector, and full-regression gates. |
| H2 — graph coverage overstated | FIXED | `docs/.doc-graph.json` | Coverage metadata now derives from the retained edges: 22/26 code mappings (85%) and 16/26 test mappings (62%). The 18 manual edges and one open contradiction edge were preserved. |
| H3 — non-executable requirement traceability | OPEN BY SCOPE | None | No `@req`/`@tested` annotations were added. The current command-surface and source/state tests do not provide an unambiguous one-to-one annotation scheme for every requirement, and adding source/test annotations would exceed this documentation-only task's allowed paths. Manual graph edges remain non-executable evidence. |
| H4 — credential security wording | FIXED | `docs/specs/2026-08-11-live-meeting-external-retrieval-design.md` | The design now states the sole exception: `external_connector_register` may receive one transient credential only for direct keyring storage. Other commands accept no raw credential. It explicitly records that production stdio execution does not currently prove credential resolution/use. |
| H5 — obsolete preflight reference | FIXED | `docs/implementation-plan.md` | The plan now names `DOC_PREFLIGHT_2026-08-11.md` v0.3.0b as CRITICAL and blocks completion/flag promotion pending disposition of that gate. |

## Exact Changed Paths

- `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md` — v0.1.1b
- `docs/.doc-graph.json` — v1.0.1 coverage metadata
- `docs/specs/2026-08-11-live-meeting-external-retrieval-design.md` — v0.1.3b
- `docs/implementation-plan.md` — v0.1.5b
- `docs/.rwang-tasks/task-4-report.md` — new v0.1.0b report

## Remaining Gates

1. H3 remains open until an approved, exact requirement-to-code/test annotation
   approach is designed and applied in a scoped source/test task.
2. `DOC_PREFLIGHT_2026-08-11.md` v0.3.0b remains the current CRITICAL plan
   control until it is re-run against the reconciled requirements and its
   conclusion is independently reviewed.
3. No real-connector, visual/keyboard, restart/provenance, artifact-wide secret
   scan, real-device capture-isolation, packaging, or full-regression claim was
   added or closed.
4. The preserved graph contradiction edge describes the pre-reconciliation
   requirements conflict. It requires a later graph/preflight reflight before
   being resolved; this task kept it open as required by H2 rather than silently
   removing historical audit evidence.

## Validation

- Parsed `docs/.doc-graph.json` successfully after the metadata correction.
- Recomputed retained graph edges: 26 requirements; 22 with code edges; 16 with
  test edges; 18 manual edges; one open contradiction edge.
- Inspected the command DTO/registration/execution boundary: registration has
  `credential: Option<String>` and stores it through the keyring lifecycle;
  `meeting_tool_execute` constructs stdio configuration without calling
  `resolve_connector_credential`.
- Confirmed this task did not edit source, tests, feature flags, or application
  behavior.

## Version Diff

| Version | Change |
|---|---|
| 0.1.1b | Fix Cycle 1 corrects FR-101's stale microphone-rail routing claim with current source and passing desktop-bootstrap regression evidence; H3 and all other open gates remain unchanged. |
| 0.1.0b | Initial hard-finding reconciliation report; H1, H2, H4, and H5 fixed in documentation, with H3 and release evidence explicitly open. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-08-12 | need review | Recorded FR-101 fix-cycle correction while retaining H3 and all release evidence gates. | pending | ATHER |
| 0.1.0b | 2026-08-12 | need review | Recorded hard-finding corrections and retained evidence gates. | pending | ATHER |

## Review Gate — T4

**Verdict: FAIL — do not promote this reconciliation as fully code-aligned.**

| Review area | Verdict | Evidence |
|---|---|---|
| Completeness | WARN | H1/H2/H4/H5 were addressed in the declared paths, but H3 correctly remains an explicit open scope gate; no executable `@req`/`@tested` mapping was added. The four governed documents are untracked in the current worktree, so their version-diff entries—not a Git base diff—are the available change record. |
| Traceability | WARN | `docs/.doc-graph.json` parses and independently derives 26 requirements, 22 code mappings, 16 test mappings, 18 manual edges, and one open `contradicts` edge. Its metadata now states 22/26 (85%) and 16/26 (62%), but the mappings remain manual/non-executable as H3 states. |
| Consistency | FAIL | The H1 rows changed for FR-106--FR-114 and FR-116 accurately distinguish code-level evidence from open UAT. However, the same requirements table still says FR-101's fixed microphone control “is not routed” (`LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md:81`). Current source opens `LiveMeetingPanel` from both recording action and microphone rail (`src/App.tsx:1052-1057,1438-1447`); `npm run test:desktop-bootstrap` passed 5/5, including that rail regression. Therefore the document-wide implementation/status column is not yet fully current. |
| Standards / control | WARN | H5 correctly points the plan at `DOC_PREFLIGHT_2026-08-11.md` v0.3.0b as CRITICAL (`implementation-plan.md:25`); the preflight itself still records the now-remediated H1 contradiction and needs the reflight already named by this report. The preserved open graph contradiction is intentionally historical per task scope, not closure evidence. |
| Code alignment | FAIL | H4 matches the actual DTO/execution boundary: only `ExternalConnectorRegisterInput` has `credential: Option<String>` (`external_mcp_commands.rs:123-131`), registration writes it through the keyring lifecycle (`:349-380`), and `meeting_tool_execute` creates stdio config without `resolve_connector_credential` (`:1343-1360,1424-1447`). The eight commands are registered (`lib.rs:1992-1999`) and the default-off UI is embedded (`LiveMeetingPanel.tsx:17,338-341`), but the stale FR-101 claim above prevents a whole-document PASS. |
| Writing quality | PASS | The revised H1/H4/H5 wording is specific, bounded, and does not claim real-connector, visual/keyboard, restart, artifact-secret-scan, device, packaging, or full-regression completion. |

### Review validation

- `npm run test:external-tools` — passed 5/5.
- `npm run test:desktop-bootstrap` — passed 5/5.
- Parsed `docs/.doc-graph.json` and independently recounted its retained edge sets.

### Required follow-up before PASS

Truth-sync FR-101 in `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md` to the routed microphone-rail implementation, then re-run the current preflight so its contradiction statement reflects the reconciled document. H3 remains open by explicit scope and must not be represented as fixed without an approved executable traceability approach.

## Fix Cycle 1

FR-101 in `docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md` is
corrected from the stale claim that the fixed microphone control is not routed.
`src/App.tsx` opens the same `LiveMeetingPanel` from both `Start recording` and
the fixed microphone rail, and `npm run test:desktop-bootstrap` passes 5/5,
including the microphone-rail regression. This documentation-only correction
does not close H3: requirement traceability remains manual and non-executable
pending an approved annotation approach. The preflight/graph reflight and all
real-connector, visual/keyboard, restart/provenance, artifact-secret-scan,
real-device capture-isolation, packaging, and full-regression gates remain
open.

## Review Gate — T4 Fix Cycle 1

**Verdict: PASS — the FR-101 documentation correction resolves the prior
code-alignment failure; H3 and all UAT/reflight gates remain explicitly open.**

| Review area | Verdict | Evidence |
|---|---|---|
| H1 / implementation status | PASS | FR-101 now states that both entry points route to `LiveMeetingPanel` and records the focused regression result (`LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md:81`). The recording action opens that panel at `src/App.tsx:1052-1057`; the fixed microphone rail does the same at `:1438-1447`. `npm run test:desktop-bootstrap` passed 5/5, including the microphone-rail regression. FR-106--FR-114 and FR-116 still state code-level implementation only and retain their visual/keyboard, restart, artifact-secret-scan, device, real-connector, and full-regression boundaries. |
| H2 / graph coverage | PASS | Parsed graph recount remains 26 requirements, 22 code mappings, 16 test mappings, 18 manual edges, and one open contradiction edge. Metadata remains 85% (22/26) and 62% (16/26), matching the derived edge sets. |
| H3 / executable traceability | WARN | Still explicitly `OPEN BY SCOPE`: mappings remain manual/non-executable and no `@req`/`@tested` claim was introduced. This is an approved open scope gate, not a false closure. |
| H4 / credential boundary | PASS | Design keeps `external_connector_register` as the sole transient raw-credential exception and states that production stdio execution does not prove credential resolution/use (`live-meeting-external-retrieval-design.md:169-173`). This matches the DTO and execution boundary. |
| H5 / preflight control | PASS | `implementation-plan.md:25` continues to name `DOC_PREFLIGHT_2026-08-11.md` v0.3.0b as the CRITICAL control. Its required reflight remains stated; no stale zero-critical gate was restored. |
| Open-UAT discipline and writing | PASS | No real-connector, visual/keyboard, restart/provenance, artifact-secret-scan, real-device capture-isolation, packaging, or full-regression completion claim was added. The Fix Cycle wording is specific and evidence-bounded. |

### Review validation

- `npm run test:desktop-bootstrap` — passed 5/5.
- `npm run test:external-tools` — passed 5/5.
- Parsed and independently recounted `docs/.doc-graph.json` retained edges.
