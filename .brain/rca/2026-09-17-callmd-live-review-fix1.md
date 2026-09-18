---
version: "0.1.0b"
created_at: "2026-09-17T06:04:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T06:04:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Approved Live UI review corrections"
  risk: "MEDIUM"
---

# Live review FIX1

## Symptom and evidence

Terra Boyle 01a0ac6b-ecbb-7242-97fb-421afc1dab30 independently returned FAIL
against frozen LiveMeetingPanel.tsx 1d64d42185271ae431db6b4346daae09d0326152d558c622ff1f76bcee904cc4
and LiveWorkspace.tsx 961fc87b1541f27d29c169257837b75fff05b0b6ca7134c5f8f4235cecbde9a4.
At panel:452,998-1017 the owner keeps native status/actions private; Shell needs
that status and a truly-inactive stop-and-leave seam without becoming an owner.
At panel:246,392 transcript reads use pair-only settlement: A-to-B-to-A permits
an old A rejection to publish after a current A request. At workspace:353,636-655
the empty-current-summary branch hides excluded/unattributable/incomplete notices.

## Root cause

The isolated Live component omitted its already-authorized local integration
adapter. The lifecycle transcript branch lacks identity epoch/request generation
guards used by other requests. Disclosure rendering is incorrectly conditional
on a nonempty current-recording summary rather than the underlying query result.

## Why it escaped detection

Eight focused tests and build pass, but no test binds the future Shell adapter,
settles the actual lifecycle transcript A-to-B-to-A/repeated-pair requests in reverse,
or renders an empty current summary with excluded results. Review caught these
before App integration; no observed native data loss is claimed.

## Bounded prevention and verification

Fresh Luna/max may change only the original six Live lease files in the absolute
live-376ef30 worktree. Add a local exported controller/status handoff with disposal
semantics; keep Live the sole native capture/listener owner and preserve shared
contracts. Guard transcript success AND rejection using full lifecycle identity,
epoch and latest request generation. Render no-current-summary and exclusion/
incomplete disclosures together, never foreign summary content. Add deterministic
production-path regressions for these three cases; retain the eight tests and
external-tools/summary/traceability/build checks. Correct the report metadata to
0.1.1b with timestamps, lifecycle status, separate UNCOMMITTED and changelog.
No App/shared/package/CI/native/provider/browser/device changes. Independent
re-review is required; native presence and suite registration remain separate
integration gates, not test waivers.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Record three confirmed Live review defects and bounded FIX1 | UNCOMMITTED;base376ef30 | Codex orchestrator |
