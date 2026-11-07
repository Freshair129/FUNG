---
version: "0.1.0"
created_at: "2026-11-07"
last_update: "2026-11-07"
status: "approved"
superseded_by: null
attributes:
  domain: "desktop-audio-capture"
  doc_type: "decision"
  scope: "Desktop release scope for per-application capture: endpoint mode first, PID mode experimental"
  language: "English"
decision:
  status: approved
  approved_at: "2026-11-07"
  authority: "Project owner (Boss) instruction in the lead chat on 2026-11-07: \"Ship endpoint, PID experimental\""
relations:
  issues:
    - https://github.com/Freshair129/FUNG/issues/96
---

# Decision Record — Endpoint-first release; PID capture is experimental

## Document control

| Field | Value |
|---|---|
| Decision owner | Project owner (Boss) |
| Status | `approved` |
| Date | 2026-11-07 |
| Related requirements | [Desktop live meeting public release](../specs/2026-08-14-desktop-live-meeting-public-release.md) |
| Related specifications | [Routed playback quarantine contract](../specs/2026-10-29-routed-playback-quarantine-contract.md) |
| Related plans | [Dedicated endpoint routing](../plans/2026-10-25-dedicated-endpoint-routing-plan.md), [Endpoint-first release plan](../plans/2026-11-07-endpoint-first-release-plan.md) |
| Supersedes | The Task 3 disposition in the dedicated endpoint routing plan: "keep PID mode and no release until both PID and endpoint modes pass. Do not hide, disable or waive the original defect." |
| Superseded by | none |

## Context

Issue #96: Windows selected-process (PID) loopback loses active PCM at a
renderer Stop/Resume. Measured losses were about −1,504 to −1,984 frames at
Stop and +441 at Resume. More than a dozen studies are recorded in the
[#96 RCA](../verification/rca/2026-11-01-issue-96-pid-audio-gap-rca.md),
ending with the inconclusive E07 diagnostic on 2026-11-07. None produced a
product repair. The earlier disposition required both PID and dedicated-endpoint
modes to pass installed acceptance before any release, so the release was
blocked indefinitely on #96.

Dedicated-endpoint routed capture has qualified native feasibility. Its custody,
playback, recovery and export foundations are on `main` (PR #98, PR #100). Its
Live UI and transcription integration are not built.

## Decision

1. The release gate for per-application capture is **dedicated-endpoint mode**.
   PID mode is no longer a release gate.
2. PID ("เลือกรายแอป") mode stays available as **experimental**. The product does
   not hide or silently remove it. Its UI marks it experimental. Before the user
   starts a PID capture, the UI warns in Thai that audio may be missing when the
   selected app pauses or resumes playback. PID recordings stay labelled
   experimental in Archive.
3. Issue #96 stays **open** as a known defect of the experimental mode. It is
   relabelled from release blocker to experimental-mode defect. The defect is
   not waived as fixed; only its release-gating role changes.
4. All non-PID gates still apply to the release. These are installed endpoint
   acceptance, microphone and system capture, saved audio, transcript, close and
   reopen, export and recovery. Merge, tag and release remain separate owner
   actions.

## Alternatives considered

| Option | Benefits | Costs/risks | Reason not selected |
|---|---|---|---|
| Keep investigating PID, no release | Single complete per-app mode | Unbounded; 12+ studies without a repair | Owner chose to unblock release |
| Remove PID mode | No known-defective path exposed | Loses a working-in-most-cases option; contradicts earlier "do not hide" stance | Owner chose experimental |
| Ship PID as normal with a footnote | No UI work | Users lose audio silently | Violates no-synthetic-success principle |

## Consequences

### Positive

- The release no longer depends on an unresolved OS-level behaviour.
- Endpoint mode work (UI, transcription, installed acceptance) becomes the
  critical path with defined scope.

### Negative / trade-offs

- Users must route apps to a dedicated output device themselves. The routing
  needs Windows sound settings, and a second output endpoint must exist.
- PID recordings can contain gaps. The experimental label and warning carry
  that risk to the user.

### Required follow-up

- Implement the experimental label and warning (plan E2).
- Build the Live endpoint-routing UI (plan E3; routing plan Task 3).
- Make routed recordings transcribable through the normal post-Stop pipeline
  (plan E4).
- Installed endpoint acceptance on real hardware (plan E5; routing plan Task 4).
  This needs the owner.
- Update `docs/Desktop/08-real-progress.md` and the #96 incident record to
  reflect the new gate.

## Security, privacy, and AI impact

There is no change to data boundaries or providers. Endpoint capture records
only the endpoints and applications that the user selected. This does not
change existing consent or quarantine behaviour. Rollback means reverting this
record, which makes PID a release gate again. No data migration is involved.

## Verification and review

This is an owner decision recorded by the lead. Implementation evidence will be
recorded in the plan and in verification reports, not here.
