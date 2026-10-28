---
version: "0.1.1b"
created_at: "2026-10-28"
last_update: "2026-10-29"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Test-only original PCM16 stereo evidence before PID decoding"
  language: "Thai-first"
---

# PID original-buffer diagnostic implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development, TDD and independent review under master-plan section 9. Root alone runs native actions.

**Goal:** Separate raw WASAPI packet content from decoding/downmix loss before
considering another production PID repair.

**Architecture:** Extend the already opt-in cfg(test) diagnostic with bounded
original PCM16 stereo bytes copied while a successful non-SILENT buffer is valid,
before decoding and ReleaseBuffer. Preserve existing decoded-mono/writer evidence
and all production capture behavior. SILENT and empty packets retain metadata,
not invented raw zero samples. Files/hash/serialization occur only after stop.

**Tech stack:** Rust 1.99.1, existing Windows WASAPI reader/probe and owned
LM22/Mi fixture apparatus; offline Python/NumPy reconciliation.

**Spec:** [Existing remediation plan](2026-10-27-installed-audio-remediation.md),
[actual n02/installed failure](../verification/implementation-reports/2026-10-28-installed-audio-retest.md).
The user requested continuation; this is the next evidence boundary within the
approved diagnostic investigation, not a new capture mode or product feature.

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance or reinterpretation of missing active audio as silence.
Keep speakers muted and normal-profile data untouched. Existing failed receipts
and historical acceptance remain intact. No execution of the previously blocked
Microsoft reference executable through another path.

No target injection/keepalive, routing/default/volume mutation, driver install,
unrelated process capture, GPU/model optimization or repeated failed flag/timer
guesses. No merge/tag until both complete installed mode acceptances pass.

## Review focus

- Only frames within the existing two-second maximum and non-null/non-SILENT
  pointers may be read; empty and SILENT pointers must never be dereferenced.
- Release executes once on nonempty acquisition even when validation/decode or
  release fails; an empty successful acquisition remains unreleased.
- Bounded frame/byte/packet capacity is preallocated; overflow invalidates
  completeness and must not change native reader/writer results or timing policy.
- Raw packet flags/index/QPC/offset and decoded packet association are explicit;
  native silent semantics do not masquerade as original sampled bytes.
- Files are fresh-only and written after stop/writer join. Actual native errors,
  collector validity and audio completeness remain distinct claims.

## Task 1 — retain original bytes with regression evidence

Files: modify `src-tauri/src/application_native_payload_diagnostic.rs` and
`src-tauri/src/live_application_audio.rs`; no production configuration switch.

- [x] Add meaningful failing tests for original interleaved stereo byte retention,
  metadata/offset order, mixed/negative/extreme downmix equivalence, SILENT-null,
  empty, oversize/null invalid packets and release failure; retain RED logs.
- [x] Add minimal cfg(test)-only predecode observer/collector. Preserve current
  decoded-mono receipt compatibility. Raw cap at most 24*48,000 stereo frames
  (4,608,000 bytes), at most 8,192 packet receipts, no file/hash/serde on read path.
  Expose separate raw validity and post-stop JSON/PCM hashes/format/boundary.
- [x] Run focused tests GREEN, Rust1.99.1 fmt and strict Clippy; freeze source and
  resulting ignored-probe binary hashes. Independent reviewer must approve the
  seam and confirm production build excludes diagnostic behavior.

## Task 2 — fresh native reconciliation

- [x] Root rechecks exact idle/nondefault/muted HDMI admission, hashes and process
  ownership. Run the existing root controller in a fresh directory with the new
  frozen test binary, simultaneous endpoint reference and two Stop/recreate cycles.
  Apparatus reuse is not installed acceptance; old n01/n02 remain untouched.
- [x] Offline validate raw/mono receipt identity, exact payload byte counts/hashes,
  independently downmix original non-SILENT bytes, reconstruct SILENT zeros only
  as explicitly derived semantics, and compare every decoded packet and writer
  sample. Compare original left/right and mono edges to the fault-free reference
  under the original 48-frame criterion. Report any collector/control failure.
- [x] Independent offline review; record precisely which boundary is established.
  No production repair unless an application-side cause is demonstrated; a
  backend architecture change requires its own supported design and evidence.

## Task 3 — delivery checkpoint

- [x] Run verification appropriate to the exact changed source, update report,
  progress and owning indexes with retained failures and open acceptance gates.
- [x] Keep actual installed results at source611aa0b separate from the new
  cfg(test)-only probe. Both full installed acceptances remain open unless
  independently demonstrated. No merge/tag/release.

Checkpoint: [original-buffer evidence](../verification/implementation-reports/2026-10-28-pid-original-buffer-diagnostic.md)
records a valid 3,838,080-byte original stereo capture with independent scalar
downmix and all-writer byte equality. Both channels still fail both edges under
the unchanged criterion. Full library 890/13 ignored and production compilation
pass; diagnostic markers are absent from the independently frozen non-test image.
This is diagnostic completion, not a PID repair or a new installed acceptance.
