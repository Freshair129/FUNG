---
version: "0.1.0b"
created_at: "2026-08-12T02:26:56+07:00,Agent: ATHER"
last_update: "2026-08-12T02:26:56+07:00,Agent: ATHER"
status: "need review"
superseded_by: null
attributes:
  domain: "documentation-governance"
  scope: "FUNG current preflight staleness-wording correction"
  doc_type: "fix-report"
---

# Task 9 — Preflight Staleness-Wording Fix

**Disposition: DONE_WITH_CONCERNS**

**Change risk:** LOW — current documentation wording and metadata only. No
source, tests, graph, Appendix D, requirements, implementation plan, runtime,
or UAT evidence changed.

## Before / After

| Artifact | Before | After |
|---|---|---|
| `docs/.preflight-report.json` WARN-03 | Reported that three document hashes required graph refresh. | States that all 28/28 mapped hashes are current; the warning is only unavailable git-date comparison for untracked scoped documents. |
| `docs/DOC_PREFLIGHT_2026-08-11.md` current staleness row | Presented completed hash refresh work as the current warning. | States that all 28/28 mapped graph-node hashes match and `stats.stale_edges` is 0; only untracked-document git-date provenance remains a warning. |

Historical audit sections, including the older post-fix table and independent
audit refresh, remain verbatim.

## Validation

```powershell
Get-Content -Raw docs/.doc-graph.json | ConvertFrom-Json
```

PASS — graph `stats.stale_edges` is `0`.

```powershell
# SHA-256 prefix comparison for each path-bearing graph node
```

PASS — 28 path-bearing graph nodes were checked; 28 match and 0 mismatch.

```powershell
Get-Content -Raw docs/.preflight-report.json | ConvertFrom-Json
```

PASS — JSON parses. Markdown whitespace inspection found no trailing
whitespace in the modified or new sections and each changed file ends with a
newline. `docs/DOC_PREFLIGHT_2026-08-11.md` retains four pre-existing trailing
double-spaces on lines 15--18 as Markdown hard breaks; they are outside this
fix scope and were not changed.

## Remaining Gates / Concerns

1. Four mapped scoped documents are untracked, so git cannot compare their
   commit dates. They are current-working-tree evidence, not commit-dated
   provenance.
2. This correction does not change the open real-connector, visual/keyboard,
   restart/provenance, artifact-secret-scan, real-device capture-isolation,
   packaging, full-regression, runtime, or UAT gates.
3. No runtime, UAT, or focused test execution is claimed by this documentation
   wording fix.

## Version Diff

- `new -> 0.1.0b`: recorded the bounded correction of current staleness
  wording after independent 28/28 mapped-hash verification.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-12 | need review | Corrected current staleness wording; retained the untracked-document git-date provenance warning. | pending | ATHER |

## Review Gate — T9

**Verdict: FAIL — one documentation-evidence detail must be reconciled before
this correction can be accepted as fully consistent.** The mapped-hash reflight
is sound: all 28 path-bearing graph nodes match their recorded SHA-256 prefixes
and `stats.stale_edges` is `0`. The current Markdown preflight correctly retains
only git-date provenance for untracked scoped documents as its staleness
warning, and historical audit sections remain present as history. No runtime,
focused-test, device, visual/keyboard, real-connector, packaging, or UAT result
is claimed by this review.

| Review dimension | Result | Independent evidence |
|---|---|---|
| Scope and preservation | PASS | T9 is limited to the JSON finding and current Markdown staleness row; the older 2026-08-12 post-fix, independent-audit, remediation, and initial-audit sections remain intact. |
| Mapped-hash freshness | PASS | Fresh SHA-256 comparison checked 28 path-bearing graph nodes: 28 match, 0 mismatch; graph `stats.stale_edges` is `0`. |
| Staleness classification | PASS | The current table at `DOC_PREFLIGHT_2026-08-11.md:48` says the sole staleness warning is unavailable git-date comparison for untracked scoped documents, not stale hashes. |
| JSON and summary consistency | FAIL | `.preflight-report.json` summary has four warnings and four warning findings, matching the current Markdown summary. However, WARN-03 states that **four** scoped documents are untracked while its `files` array lists only three; the omitted mapped document is `docs/appendices/D-traceability.md`. Reconcile that evidence list or its stated count. |
| Provenance boundary | WARN | Git confirms four mapped document nodes are untracked, so commit-date comparison is unavailable. This is a valid open warning, not evidence that any mapped hash is stale. |
| Claim discipline | PASS | T9, its Fix Cycle 1, and the current preflight explicitly preserve open runtime/UAT gates and make no new execution claim. |

**Required follow-up:** make WARN-03's stated untracked-document count and its
listed paths agree, then re-run the JSON parse and the 28-path hash comparison.

## Fix Cycle 1

Corrected only `docs/.preflight-report.json` WARN-03 so its details state that
three scoped documents remain untracked, matching its three listed paths.
JSON parse validation passed. No runtime or UAT result is claimed.

## Review Gate — T9 Fix Cycle 1

**Verdict: FAIL — the JSON's internal count now matches its three listed paths,
but those three are not the full current set of untracked mapped documents.**
This preserves the original hash and staleness result: all 28 mapped SHA-256
prefixes are current and `stats.stale_edges` remains `0`.

| Review dimension | Result | Independent evidence |
|---|---|---|
| WARN-03 internal count | PASS | WARN-03 now says three scoped documents are untracked and its `files` array contains the same three paths. |
| Git provenance truth | FAIL | Fresh `git ls-files --error-unmatch` finds four untracked mapped document nodes: the three listed paths plus `docs/appendices/D-traceability.md`. The warning's stated scope must include that fourth document or explain its exclusion. |
| Mapped-hash freshness | PASS | Fresh SHA-256 prefix comparison: 28 mapped paths checked, 28 match, 0 mismatch. |
| Graph staleness | PASS | `docs/.doc-graph.json` reports `stats.stale_edges: 0`. |
| Historical preservation | PASS | The retained historical preflight and audit headings remain present; this cycle records only the WARN-03 wording correction. |
| Claim discipline | PASS | Fix Cycle 1 and this review add no runtime, focused-test, device, visual/keyboard, real-connector, packaging, or UAT success claim. |

**Required follow-up:** reconcile WARN-03 with the actual four untracked mapped
document nodes, then re-run JSON parsing, git tracked-state inspection, and the
28-path hash comparison.

## Fix Cycle 2

Corrected only `docs/.preflight-report.json` WARN-03 so its details state that
four scoped documents remain untracked and its `files` array includes
`docs/appendices/D-traceability.md`. The existing 28/28 mapped-hash and
`stats.stale_edges: 0` wording is retained. JSON parse validation passed; this
documentation correction does not add any runtime or UAT claim.

## Review Gate — T9 Fix Cycle 2

**Verdict: PASS WITH WARN — the reported provenance limitation is now accurate;
the warning remains open by design until the four documents are committed.**

| Review dimension | Result | Independent evidence |
|---|---|---|
| WARN-03 count and paths | PASS | WARN-03 states four untracked scoped documents and lists exactly four paths, including `docs/appendices/D-traceability.md`. |
| Git provenance alignment | PASS | `git ls-files --error-unmatch` confirms those same four paths are the complete untracked mapped-document set. |
| Mapped-hash freshness | PASS | Fresh SHA-256 prefix comparison checked 28 mapped paths: 28 match and 0 mismatch. |
| Graph staleness | PASS | `docs/.doc-graph.json` retains `stats.stale_edges: 0`. |
| Historical preservation | PASS | The historical preflight/audit sections remain present; Fix Cycle 2 is limited to current WARN-03 provenance wording. |
| Claim discipline | PASS | Fix Cycle 2 and this review make no runtime, focused-test, device, visual/keyboard, real-connector, packaging, or UAT success claim. |

**Open warning:** commit the four mapped documents when commit-date provenance is
required. Their current-working-tree hashes are verified current; this is not a
stale-hash finding.
