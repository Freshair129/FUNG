---
version: "0.1.2b"
created_at: "2026-11-07"
last_update: "2026-11-07"
status: "beta"
superseded_by: null
attributes:
  domain: "programme-delivery"
  doc_type: "plan"
  scope: "Ordered FUNG delivery backlog for the current milestone"
  language: "English"
---

# FUNG delivery backlog

This is the ordered work list for the [roadmap](ROADMAP.md). Items are pointers
to owning plans and issues, which hold the specs. The lead keeps this list
current; GitHub stays authoritative for issue state.

Priorities: **P0** blocks the current milestone (M1). **P1** is needed for M1
but not on the critical path. **P2** is next milestone. **P3** is unscheduled.
Estimates are lead effort estimates in working days, including review and CI.

Owners: **Lead** is Claude (plan, review, merge). **Codex** is the executor
(`gpt-6-luna`/max for code, `gpt-6.1-terra`/high for tests). **Owner** is the
project owner, for decisions, hardware sessions and release actions.

## M1 — v0.1.2 endpoint-first (as of 2026-11-07)

| ID | Pri | Item | Owner | Est. | State | Depends | Link |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B-01 | P0 | Merge PR #101 (#96 E07 diagnostic record) | Owner/Lead | <0.1 | Done 2026-11-07 (1c65caa) | — | PR #101 |
| B-02 | P0 | E2: PID mode experimental label and warning | Codex/Lead | 0.5 | Done 2026-11-07 (PR #102, afb2f85) | — | [E2](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-03 | P0 | E3 + E3-G: Live endpoint-routing UI and fail-closed legacy job guard | Codex/Lead | 1 | Done 2026-11-07 (PR #102, afb2f85) | — | [E3](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-04 | P0 | Decide mic capture in routed mode | Owner | decision | Decided 2026-11-07: include mic (delegated to lead) | B-03 | [Open gaps](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-05 | P0 | E4: transcription for routed recordings (spec, code, tests) | Lead → Codex | 2–3 | Done 2026-11-07 (PR #103) | B-03, B-04 | [E4](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-06 | P0 | E6: routed mic integration | Codex/Lead | 1–3 | Done 2026-11-08 (PR #104, e7608d0) | B-04 | — |
| B-07 | P0 | Build installed candidate and write the E5 runbook | Lead/Codex | 0.5 | Done 2026-11-08: candidate from e7608d0, [runbook](../plans/2026-11-08-e5-installed-endpoint-acceptance-runbook.md) | B-02, B-03, B-05 | [Routing Task 4](../plans/2026-10-25-dedicated-endpoint-routing-plan.md) |
| B-08 | P0 | E5: installed endpoint acceptance session | Owner + Lead | 1–2 | Ready for owner session | B-07 | same |
| B-09 | P0 | #94 installed delayed-playback alignment retest | Owner + Lead | 0.5 | Open | B-07 | Issue #94 |
| B-10 | P1 | #79 standalone transcription through durable restart | Codex/Lead | 0.5–1 | Open | B-07 | Issue #79 |
| B-11 | P1 | #81 shell layout / nested scroll native visual check | Codex (Terra) | 0.5 | Open | B-07 | Issue #81 |
| B-12 | P1 | Update v0.1.2 release notes and 08-real-progress to the endpoint-first gate | Lead | 0.25 | — | B-08 | [v0.1.2](../releases/v0.1.2.md) |
| B-13 | P1 | Packaged build, installer smoke, release integrity gates | Lead | 0.5 | — | B-08 | [Release spec](../specs/2026-08-14-desktop-live-meeting-public-release.md) |
| B-15 | P1 | PID recording Archive badge (needs a native recording-level source marker; E2.3 stopped) | Lead → Codex | 0.5–1 | Spec needed | B-02 | [Open gaps](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-14 | P0 | Merge, tag v0.1.2, publish, web promotion | Owner | — | — | all P0 | Owner action |

Critical path: B-03 → B-04 → B-05 → B-07 → B-08 → B-14.

## Maintenance (any time)

| ID | Pri | Item | Owner | Est. | State | Link |
| --- | --- | --- | --- | --- | --- | --- |
| B-20 | P1 | Dependabot #90/#92: rebase on current `main`, merge when CI passes | Lead | 0.1 | Frontend CI failed on a stale base | PR #90, PR #92 |
| B-21 | P1 | Dependabot security job fails resolving `filelock` (unresolvable dependency) | Lead | 0.25 | Open | Actions "Dependabot Updates" |
| B-22 | P2 | Remove Cargo lane `F:\cargo-lanes\fung-issue96-w1-20261107` (machine-local) | Lead | <0.1 | — | — |
| B-23 | P2 | Codex local build admission blocked by concurrent-growth policy; agree a standing storage budget so Codex can run Cargo locally | Owner + Lead | decision | Open | `~/.codex/policies` |

## M2 and later

| ID | Pri | Item | Owner | Link |
| --- | --- | --- | --- | --- |
| B-30 | P2 | E3b: routed capture Product MCP adapters | Codex/Lead | [E3b](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-31 | P2 | Product MCP stages after MCP-02 | Codex/Lead | [MCP roadmap](../plans/2026-10-20-product-mcp-feature-roadmap.md) |
| B-32 | P2 | Live waveform for routed sources (native level telemetry) | Codex/Lead | [Open gaps](../plans/2026-11-07-endpoint-first-release-plan.md) |
| B-33 | P2 | #82 reference dashboard layout | Codex/Lead | Issue #82 |
| B-34 | P3 | Re-plan master-plan Phases 3–5 against current truth | Lead + Owner | [Master plan](../plans/2026-08-09-fung-master-implementation-plan.md) |
| B-35 | P3 | #96 PID Stop/Resume loss: experimental-mode defect, no repair path | — | Issue #96, [RCA](../verification/rca/2026-11-01-issue-96-pid-audio-gap-rca.md) |

## CHANGELOG

| Version | Date | Summary | Agent |
| --- | --- | --- | --- |
| 0.1.2b | 2026-11-08 | B-06 done; B-07 candidate and E5 runbook done; B-08 ready. | Claude (lead) |
| 0.1.1b | 2026-11-07 | B-02, B-03, B-05 done; B-06 spec next. | Claude (lead) |
| 0.1.0b | 2026-11-07 | First consolidated backlog for M1 (v0.1.2 endpoint-first), maintenance, M2+. | Claude (lead) |
