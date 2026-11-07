---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "One bounded direct-GetBuffer experiment and legacy recovery admission"
  language: "Thai-first"
---

# Direct acquisition and recovery guard implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development, TDD and independent review under master-plan section 9. Root alone runs native/device actions.

**Goal:** Falsify the remaining packet-availability gate hypothesis and prevent
legacy recovery from falsely completing routed recordings while full source-aware
recovery remains unimplemented.

**Architecture:** A cfg(test)-only reader-driver candidate drains GetBuffer until
the API reports an empty packet, without treating GetNextPacketSize as admission.
Production acquisition remains unchanged unless native bytes/edges demonstrate a
repair under the existing criteria. Separately apply the existing fresh
Genesis-backed legacy-media guard before any legacy recovery path/repair mutation.

**Tech stack:** Rust1.99.1, existing private packet driver/original-byte probe,
Windows WASAPI, Genesis SDK and owned muted HDMI marker/control fixtures.

**Spec:** [Issue96 contract](2026-10-25-issue-96-application-transition.md),
[routing design](../specs/2026-10-25-dedicated-endpoint-routing-design.md),
[original-buffer evidence](../verification/implementation-reports/2026-10-28-pid-original-buffer-diagnostic.md).
User explicitly requested further capture investigation and actual installed
PID plus endpoint qualification before merge/tag; baseline f399d02, existing worktree.

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance or reinterpretation of missing active audio as silence.
Keep speakers muted and normal-profile data untouched. Existing failed receipts
and historical acceptance remain intact. No execution of the previously blocked
Microsoft reference executable through another path.

No target keepalive/injection, driver install, default/routing/volume setters,
unrelated process capture or model/GPU optimization. PID and endpoint stay distinct.
Both full installed gates must pass before merge/tag/release. An unsuccessful
candidate is not enabled in production or repackaged as a repair.

## Evidence and bounded hypothesis

- [x] Primary-source backend research: OBS/bozbez/NAudio wrap the same process
  loopback service; inspected AudioGraph/MediaCapture/AppRecording APIs do not
  establish an independent arbitrary-PID replacement. No actual repair found.
- [x] Independent raw audit: source-correlated endpoint bands exceed raw L/R,
  and no long reader stall explains them. After-Stop renderer clock jumps are
  not a continuously linear physical/acoustic playback oracle.
- All historical candidates, including timer and MF, gate reads on
  GetNextPacketSize. Existing raw receipts describe acquired packets only.
  No false-zero hint has been demonstrated. Direct acquisition is therefore a
  low-confidence, test-only falsification of an unproved application boundary,
  not a different OS backend or documented Windows repair.
- Microsoft documents [GetNextPacketSize as optional](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getnextpacketsize)
  and [GetBuffer empty-read semantics](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer).
  Preserve the same event wait, format, stream flags, clock, writer and controls.

## Review focus

- An empty successful direct read ends a drain without ReleaseBuffer; a genuine
  peek-positive/empty baseline anomaly remains a failure in the production driver.
- Stop occurs once, cancellation cannot starve, native reader/writer errors and
  cleanup failures retain first cause; no added wait, silence or timestamp padding.
- Opt-in candidate is restricted to a distinct explicitly ignored test and scoped
  caller-local request; no product/environment configuration switch.
- Raw/mono receipts identify acquisition strategy and source hashes. Never combine
  raw frames from one variant with control/ledger from another.
- Recovery must inspect fresh source ownership before paths, row writes, finish
  or audit events, including legacy labels hiding authoritative routed bindings.

## Task 1 — test-only direct acquisition

Files: `src-tauri/src/application_packet_driver.rs`,
`live_application_audio.rs`, `application_native_payload_diagnostic.rs` only.

- [x] Retain meaningful RED: a scripted source returns zero size hints while
  GetBuffer has actual payload; the existing peek gate fails to acquire it.
  Pin direct empty after Stop as successful drain, exact-once Stop/error cleanup,
  and existing production peek-positive/empty regression unchanged.
- [x] Implement the smallest cfg(test)-only direct drain entry using existing
  PacketSource/read/accept functions and lifecycle helpers; no production change.
- [x] Add one distinct ignored native direct-acquisition payload test with a
  scoped single-use strategy selector and unchanged 20s raw/mono diagnostics.
- [x] Focused GREEN, fmt/strict Clippy, immutable sources/test executable and
  independent source/apparatus review before root native execution.
- [x] Root runs fresh matched baseline/direct studies with actual explicit
  endpoint reference and both renderer transitions after exact idle/mute guards.
  Independently compare all original/mono/writer bytes and edge differences.
  Only a real native PASS permits a separately reviewed production adoption.
  If it fails, retain failure and stop delivery variants without a new supported
  architecture; no package build presented as an audio repair.

## Task 2 — legacy recovery source admission

Files: `src-tauri/src/recovery.rs` production entry and regression tests. Reuse
`routed_media_access::require_legacy_media_source`; no schema/public command change.

- [x] RED on the actual recover_recording entry: routed pending/recording/
  quarantined/completed and hidden source bindings must error before repair.
  Snapshot recording/chunks/sources/checkpoints/audit and filesystem before/after;
  no mutation or false completed status on rejection.
- [x] Add fresh admission before capture/path access; preserve actionable safe
  error and existing healthy legacy orphan adoption/idempotency.
- [x] Focused GREEN and independent source review. This is a fail-closed repair,
  not implementation or acceptance of source-aware recovery.

Task1 is the first Cargo/native build owner. Task2 may inspect/write only after
Task1's focused freeze, or use root-coordinated serial Cargo checks; no mixed
mutable-source build evidence. All source snapshots include every changed Rust file.

## Task 3 — package and delivery decision

- [ ] If native PID candidate passes, add production regression/adopt only the
  demonstrated fix, complete source-aware endpoint consumers/authority work under
  separate reviewed contract slices, then build/install and qualify both full modes.
- [x] Otherwise keep original PID FAIL and endpoint full acceptance open; deliver
  only verified bounded safety corrections and research/evidence, with accurate
  actual installed/non-installed scope. No blind repeated package/native runs.
- [x] Update owning reports/index/progress, retain all failures, exact source checks,
  commit/push Draft PR; no merge/tag until both actual full installed gates pass.

[Current evidence](../verification/implementation-reports/2026-10-29-capture-backend-and-recovery-guard.md):
baseline and direct acquisition both FAIL; the direct candidate is not adopted.
Task3's conditional adoption/package work is not unlocked. The real recovery
admission defect has RED5/GREEN15 and independent source GO; this is not full
source-aware recovery or actual installed verification. The user has only this
Windows host; no cross-host result is available and no host/settings change is
implied. Both full installed gates remain open.
