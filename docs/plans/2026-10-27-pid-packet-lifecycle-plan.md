---
version: "0.1.0b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "PID packet progress and cancellation; offline candidate only"
  language: "Thai-first"
---

# PID packet lifecycle implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development; preserve RED/GREEN evidence and independent review.

**Goal:** Correct independently reproducible PID reader progress/cancellation defects and build an isolated candidate without running audio.

**Architecture:** Keep the current process-loopback backend, decoder, clock and writer. Extract a small private production packet/lifecycle seam used by both native code and deterministic scripted tests. This is component repair, not proof that the native renderer-transition loss is repaired.

**Tech stack:** Rust, Windows WASAPI, existing Tauri/React/NSIS packaging; no new dependency.

**Spec:** [Original #96 contract and repair plan](2026-10-25-issue-96-application-transition.md), [historical native failures](../verification/implementation-reports/2026-10-25-pid-producer-lifecycle.md).

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance or reinterpretation of missing active audio as silence.
Keep speakers muted and normal-profile data untouched. Existing failed receipts
and historical acceptance remain intact. No execution of the previously blocked
Microsoft reference executable through another path.

User direction on 2026-10-27: continue source work and build a new package; **all real audio tests wait for the user to announce readiness**. Do not start capture/render/inventory tools, install or launch the candidate, alter device routing/mute/volume, merge, tag or release. Both endpoint and PID installed audio gates stay open.

## Evidence and scope ruling

Baseline `6981de467efc1b665563a53774d5a237c43130db` has green hosted CI and package C negative/reopen checks. Original #96 still loses correlated active frames at renderer Stop/Resume. Historical 1 ms timeout draining, MMCSS, alternate formats/flags, timer and MF candidates failed. Repeating those as a demonstrated fix is out of scope.

Static reader findings are separate: inner draining does not recheck cancellation; a positive-size/empty-acquisition anomaly loops without yielding; late activation can start after the ready receiver times out; readiness-send failure is ignored. The dependency also classifies failed event waits as timeout. Deterministic tests may prove corrections to these component defects. They cannot reconstruct PCM the native API did not deliver or close #96.

## Files and review focus

- `src-tauri/src/live_application_audio.rs`: native adapter, startup/readiness handoff and shared pump call sites. A sibling private module is allowed if it keeps the seam focused; register only privately.
- Existing test module or new sibling `*_tests.rs`: bounded scripted native adapter tests, using the actual production driver rather than a second simulated algorithm.
- This plan, parent #96 plan, Desktop progress and verification report/index: distinguish source candidate, built package and unexecuted native acceptance.

Review stop during continuously nonempty buffers; empty-after-peek without spinning; readiness cancellation before/after Start; wait/stop/queue/read errors preserving the primary cause and accepted tail; panic/unwind cleanup and native owner thread. Process-generation pinning is a separate existing contract gap, not silently solved here.

## Task 1 — production packet/lifecycle correction

- [x] Preserve baseline tests, extract the current behavior into the smallest private seam, then capture deterministic failing behavior before repairs. No copy of the drain algorithm under cfg(test).
- [x] Add a bounded scripted source that requests stop after one packet and only becomes finite after native Stop; prove stop-before-next-drain, exactly-once successful Stop and exact accepted/final PCM.
- [x] Exercise positive-size/zero-frame acquisition without hot retry. Check cancellation/control before retry and fail honestly if a stopped-buffer anomaly prevents proving a complete drain; do not manufacture samples.
- [x] Abort before Start when canceled; if ready delivery is lost after Start, stop and close the writer without pretending readiness succeeded. Keep late-worker cancellation state local even when the caller times out; do not claim hard cancellation of a dependency activation blocked inside Windows.
- [x] Distinguish event timeout from wait failure through a minimal native wait adapter if the dependency cannot; do not replace the backend, reduce timeout or adopt historical timeout-drain workarounds.
- [x] Cover wait/read/queue/stop failures and unwind cleanup with the actual shared driver. Preserve all existing decoder, timestamp, gap, backpressure and durable-tail checks.
- [x] Run focused tests, exact Rust 1.99.1 fmt/strict all-target Clippy, full library/integration and custody-contract checks. Record failures rather than deleting or weakening them.
- [x] Independent spec/code review and bounded fix loop.

## Task 2 — build-only candidate and evidence

- [x] Build actual frontend and NSIS using a separate UAT app identifier/configuration; freeze source, configuration, installer and executable hashes.
- [x] Do not install/launch or execute native audio tests. Keep package C and previous evidence intact.
- [x] Update owning docs/index and record exact verification commands, source revision, failures, package paths and native/installed NOT_RUN status.
- [x] Commit/push source and evidence to the existing Draft PR after verification; no merge/tag/release.

## External references

- [GetBuffer](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer) and [GetNextPacketSize](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getnextpacketsize): native packet protocol.
- [Stop](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-stop): stops production without resetting its clock.

The original arbitrary-target transition acceptance remains open even if every offline task above passes.
