---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Distinguish stopping from destroying an owned target renderer"
  language: "Thai-first"
---

# PID Stop versus renderer destruction

> Use systematic debugging and independent review under master-plan §9. Root alone
> runs native audio/device actions; evidence is separate from installed acceptance.

**Goal:** identify whether missing active PID-loopback samples arise at renderer
Stop or immediate release, then determine whether an actual product fix is possible.
**Architecture:** controlled owned player only; hold its stopped AudioClient and
render service during the pause, then release before recreating the renderer.
Compare with the same fixture configured to destroy immediately and simultaneous endpoint
PCM reference. Existing PID capture, source signal and acceptance criteria stay fixed.
**Tech stack:** Rust1.99.1, Windows WASAPI, frozen diagnostic and owned HDMI fixtures.
**Spec:** [issue 96 plan](2026-10-25-issue-96-application-transition.md),
[endpoint routing design](../specs/2026-10-25-dedicated-endpoint-routing-design.md).

## Constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance, target injection, new driver or volume/default/routing
setter. Keep test speakers muted and protect normal profile and unrelated processes.
Never execute the previously blocked Microsoft reference executable.
Only owned fixture changes; no extra active or silent render stream. Capture retains
48-frame boundary, isolation, complete-tail and error criteria. No merge/tag/release
before actual installed PID and endpoint workflow acceptance both pass.

## Tasks

- [x] Freeze and independently review a fixture that retains only the stopped actual
  renderer/client during the existing bounded pause; log Stop, hold, destruction and
  recreation separately. No extra PCM, Start or Reset during the hold.
- [x] Root checks exact endpoint ID, active/nondefault/idle/muted status; run matched
  immediate-destruction and stopped-held cases with same frozen PID probe, independent
  endpoint control, marker and two transitions. Capture raw and persisted PCM.
- [x] Independently compare played marker edges and original/mono/writer byte custody;
  retain failures, guards, process exits, hashes and cleanup. A held-target pass only
  identifies a boundary: it cannot control arbitrary application object lifetimes.
- [ ] If a supported production repair is proven, document design, add regression,
  implement and independently review it, then build and test an isolated installed
  package through complete PID and endpoint flows. Otherwise keep gates open.
- [x] Update verification report, indexes and progress; commit/push only verified
  scope to Draft PR. Merge/tag only after both full installed gates pass.
