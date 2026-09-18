---
version: "0.1.1b"
created_at: "2026-09-17T09:47:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T12:24:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved bounded Live dark-label contrast correction; independently reviewed source, fresh browser blocked by webview attachment"
  risk: "LOW"
---

# Live dark form labels remain dark

## Symptom

After IntegrationFIX2 restored theme ancestry, History and Live receive dark
palettes correctly, but the two consent/system-capture labels and language label
in Live are visibly difficult to read. No capture or user-data operation occurred.

## Evidence

- Main actual CUA at127.0.0.1:15475/app?surface=desktop,1280x800, AppSHA256
  d26064747c604dc0d350160e7ccebeb881ecc745557cbd35cfa19d4266334039.
- `.live-workspace__check` and `.live-workspace__field` computed color
  rgb(48,56,51), opacity1, on capture-card background rgba(34,38,36,0.92).
  Screenshot shows near-dark text on dark card. The consent checkbox is enabled;
  this is not a disabled-control-only contrast observation.
- LiveWorkspace.css149-155 explicitly sets these labels to #303833. The dark
  root sets --workspace-ink:#f4f1ea at505+, but this direct label color overrides
  inherited root color. The dark text selector list at520-527 omits both labels;
  the field-select override at546 styles the select, not its parent label.
- Source CSS remains accepted prior hash
  a629bf1b2bd5c84539adeb3d34d225930826da9c91293d5cc53fefa8dfe813bd.

## Root Cause

Hard-coded light-palette label color has no dark override. Restored integrated
theme ancestry exposes this existing leaf-palette omission; theme resolution
itself now works. No History data or native listener defect is asserted.

## Why it escaped detection

Prior isolated tests did not assert actual form-label contrast in dark mode.
Before the ancestor correction, the integrated panels stayed light, masking it.

## Proposed prevention and exact next lease

Boss's subsequent explicit `Approve` authorizes the following exact lease. A fresh Luna/max may edit ONLY:

1. src/components/desktop/LiveWorkspace.css — dark label override using existing
   approved readable palette; preserve light mode and layout.
2. tests/callmdLiveWorkspace.test.mjs — regression plus actual production-component
   fixture observation of dark label colors/contrast, retaining StrictMode tests.
3. docs/verification/implementation-reports/2026-09-17-callmd-ui-live.md — evidence.

No App, native, dependency, schema, settings, data or unrelated palette changes.
Independent Terra re-review and a new authorized browser cycle are required.
Controller must not implement or transfer source. Existing native/CI gates remain.

## Pause condition

The previous-turn pause below is historical. The subsequent explicit `Approve`
authorizes this three-file correction and a fresh scoped browser verification
cycle. Complexity C-2, risk LOW. Success requires preserved light-mode labels,
readable dark labels, retained lifecycle regressions, build, independent review,
and actual production-component/browser checks. It grants no native runtime,
credential, install, commit, deployment, or broader palette authority.

agent-browser-verify limits automatic browser fix/retry cycles to two. Initial
App lifecycle failure was followed by retry1(theme ancestry) and retry2(this
contrast defect). No third automatic code/browser cycle is dispatched this turn.
This proposal requests direction; it does not grant its own implementation lease.

## Version Diff / CHANGELOG

Implementation checkpoint12:36: fresh Bernoulli Luna/max completed the exact
three-file lease and was closed. Worker Node12/12, build1812, diffcheck PASS;
main independently matched all35 pins (only the three authorized changes).
Fresh Zeno Terra/high returned PASS_SCOPED with50/50 CallMD tests. Actual browser could not attach a webview;
visual contrast remains unverified on this revision, not declared failed or
passed from source assertions. See callmd-browser-reverify.md v0.1.3b.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | beta | Recorded explicit approval for the exact three-file contrast correction and new scoped verification cycle | UNCOMMITTED; base376ef30 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | candidate | Source-backed contrast RCA and next bounded lease awaiting direction | UNCOMMITTED; base376ef30 | Codex orchestrator |
