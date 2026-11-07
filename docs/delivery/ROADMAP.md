---
version: "0.1.1b"
created_at: "2026-11-07"
last_update: "2026-11-07"
status: "beta"
superseded_by: null
attributes:
  domain: "programme-delivery"
  doc_type: "plan"
  scope: "FUNG delivery roadmap: release milestones, gates and target windows"
  language: "English"
---

# FUNG delivery roadmap

This is the single view of what ships next, what blocks it, and when. It links
to the owning plans and does not repeat their content. Status truth stays in
[Desktop real progress](../Desktop/08-real-progress.md), the
[implementation ledger](../verification/implementation-reports/progress.md) and
GitHub. The ordered work list is the [backlog](BACKLOG.md).

Target windows are lead estimates from 2026-11-07. They are not commitments.
Each window assumes no new release-blocking defect and that the owner is
available for the installed sessions. Update this file when a milestone moves.

## Released

| Version | Date | Scope | Notes |
| --- | --- | --- | --- |
| v0.1.0 | 2026-08-14 | Windows x64 public beta: Live Meeting, mic + system capture, local CPU transcription | [Release notes](../releases/v0.1.0.md); unsigned installer |

Tag `v0.1.1` exists without a GitHub release.

## M1 — v0.1.2 Desktop beta: per-app capture (endpoint mode)

**Goal:** release per-application meeting capture through dedicated audio
endpoints. PID capture ships as experimental, per the
[2026-11-07 decision](../decisions/2026-11-07-endpoint-first-release-pid-experimental.md).
The release notes draft is [v0.1.2](../releases/v0.1.2.md). It still describes
the old PID gate and must be updated before publication.

**Target window:** 2026-11-14 to 2026-11-21. The earliest date applies if E4
needs no mic integration and installed acceptance passes in one session.

| Gate | Owner | State (2026-11-07) | Plan |
| --- | --- | --- | --- |
| Routed custody, playback, recovery, export on `main` | Lead/Codex | Done (PR #98, #100) | [Close ownership](../plans/2026-10-30-playback-close-ownership.md) |
| PID mode marked experimental (E2) | Lead/Codex | Done (PR #102) | [Endpoint-first](../plans/2026-11-07-endpoint-first-release-plan.md) |
| Live endpoint-routing UI (E3) | Lead/Codex | Done (PR #102) | same |
| Transcription of routed recordings (E4) | Lead/Codex | Done (PR #103), post-Stop only | same |
| Mic in routed mode (E6) | Lead/Codex | Decided: include; spec next | same |
| Installed endpoint acceptance (E5) | **Owner + lead** | Not started; needs hardware | [Routing plan Task 4](../plans/2026-10-25-dedicated-endpoint-routing-plan.md) |
| System loopback delayed-playback retest (#94) | Owner + lead | Open | [Installed UAT](../verification/implementation-reports/2026-10-23-installed-releasefix-uat.md) |
| Standalone transcription durable restart (#79) | Lead/Codex + owner | Open | Issue #79 |
| Shell layout and nested-scroll visual check (#81) | Lead/Codex | Open; native visual check | Issue #81 |
| Packaged build, installer smoke, release integrity | Lead | Not started | [Public release spec](../specs/2026-08-14-desktop-live-meeting-public-release.md) gates |
| Merge, tag, GitHub release, web promotion | **Owner** | — | Owner action |

#82 (reference dashboard layout) is not an M1 gate unless the owner adds it.

## M2 — Product MCP and routed MCP adapters

**Goal:** finish the Product MCP roadmap stages (MCP-02 review onward) and
expose routed capture to MCP (E3b).
**Target window:** after M1, about 2–3 weeks. This will be re-estimated when M1 ships.
Plans: [Product MCP roadmap](../plans/2026-10-20-product-mcp-feature-roadmap.md),
[endpoint-first E3b](../plans/2026-11-07-endpoint-first-release-plan.md).

## M3 — Platform phases from the master plan

**Goal:** the remaining master-plan phases: Phase 3 controller acceptance,
Phase 4 clean-install and device proof, and Phase 5 hardening and signed
release. Google Drive is cancelled.
**Target window:** not estimated. It needs re-planning against the current
truth before any date is given.
Plan: [master implementation plan](../plans/2026-08-09-fung-master-implementation-plan.md).

## Not scheduled

- Issue #96 PID Stop/Resume loss: experimental-mode defect, no repair path
  identified. See the [RCA](../verification/rca/2026-11-01-issue-96-pid-audio-gap-rca.md).
- GPU acceleration, code signing, auto-update: out of scope per the
  [public release spec](../specs/2026-08-14-desktop-live-meeting-public-release.md).

## CHANGELOG

| Version | Date | Summary | Agent |
| --- | --- | --- | --- |
| 0.1.1b | 2026-11-07 | E2, E3, E4 done; mic in routed mode decided (E6). | Claude (lead) |
| 0.1.0b | 2026-11-07 | First consolidated roadmap: M1 v0.1.2 endpoint-first, M2 MCP, M3 platform phases. | Claude (lead) |
