---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-31"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Completed routed-source media and quarantine-safe playback"
  language: "Thai-first"
---

# Completed routed-source playback and quarantine

> Use TDD and independent task/interface reviews under master-plan §9.
> Root owns native, installed-package and audio tests.

**Goal:** enable explicit playback of a trusted completed routed source and stop
access when its recording is quarantined, without weakening existing legacy paths.
**Architecture:** Genesis source/chunk bindings are authoritative. A private
verified source reader checks exact-frame PCM on opened custodied handles; a
separately reviewed interface amendment connects it to the bounded native player.
Durable quarantine must revoke active playback and prevent late installation.
**Tech stack:** Rust1.99.1, Genesis SDK, hound/cpal, Tauri, React/TypeScript.
**Specs:** [routing](../specs/2026-10-25-dedicated-endpoint-routing-design.md),
[existing playback contract](../specs/2026-09-17-callmd-desktop-contracts.md),
[consumer guard audit](../verification/implementation-reports/2026-10-26-routed-consumer-guard-audit.md).
Baseline `4950696c607c5b77eaba3840b2413ab4702beee2`; existing worktree.

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
surface-param routes ungated. No all-system fallback, fabricated PCM, relaxed
loss tolerance, default/routing/volume mutation, target injection or new driver.
Keep test endpoints muted, preserve normal profiles and evidence. Existing SEC-1
main-origin and owner-bound handles and SEC-2 capture exclusion remain mandatory.
No arbitrary paths/PCM through renderer; preserve legacy mic/system/file behavior.
Limit player to20,000 descriptors/8MiB metadata, two handles,64KiB reads,1MiB queue
and10s open deadline. Aggregate source quarantine blocks the routed recording.
Both complete installed PID and endpoint gates remain prerequisites to merge/tag.

## Task1 — private completed-source reader

Files: new `src-tauri/src/completed_source_media.rs`, existing
`src-tauri/src/capture_source_custody.rs` and `src-tauri/src/lib.rs` for private
registration only. Narrow reuse/extraction of existing custodied-file helpers in
`src-tauri/src/desktop_playback.rs` is permitted when needed for same-handle
containment; preserve all existing legacy behavior. No public IPC, schema, route
or auth changes.

- [ ] Define private manifest and verified chunk opening from authoritative
  recording/source/epoch bindings, with existing fresh frontier guard. Do not
  treat its frontier as a lasting permission or enable a consumer yet.
- [ ] RED actual Genesis fixture tests for two same-named sources, wrong project,
  recording/source/epoch, pending/active/quarantined, altered/truncated file,
  path escape and one-frame tail. Snapshot real rows/media and test denial.
- [ ] GREEN: bounded same-handle containment, length, SHA-256, WAV header and exact
  frame provenance; retain 1-frame sub-ms tails. No whole-recording allocation.
  Revalidate on reopen and fail closed on concurrent frontier changes.
- [ ] Independent task review, Rust fmt/Clippy and meaningful targeted tests;
  reader alone is not user-visible playback or endpoint acceptance.

**Ruling, 2026-10-29 continuation:** the original playback contract limits
20,000 descriptors/8MiB metadata per player; it does not specify an 8MiB total
SDK allocator peak. The earlier attempted plan strengthened that requirement
without authority. Use the existing embedded `Storage::query_relational`
authority and aggregate guard, narrow paginated projections and the unchanged
retained-manifest bound. No HQL2 principal, schema or namespace change is needed.
SDK page materialization remains outside any claimed peak-memory guarantee;
do not describe a post-read byte check as pre-allocation enforcement. New
fixture tests must exercise the real reader rather than mark the earlier stub
RED as a completed implementation. Earlier evidence remains retained at
`G:\AI-Studio\verification\fung-completed-source-reader-20261029`.

## Task2 — owner interface and playback/quarantine integration

- [x] Verify the pinned Genesis SDK's exclusive store ownership in an isolated
  Windows cross-process fixture before relying on process-local revocation. Use
  a fresh temporary root only, actual child PID/receipt, writable and read-only
  opens while held and after release, same-lock-file identity for path aliases,
  positive controls, bounded owned-child cleanup. This is not protection against
  lock-file bypass or proof of player cancellation.
  Actual Windows run: one integration test passed, one child helper ignored in
  the parent runner, exit0. All seven child cases were asserted inside the parent;
  source and log are retained outside the repo under
  `G:\AI-Studio\verification\fung-completed-source-reader-20261029`. The
  source hardcodes this machine's verification root and is not a portable CI test.
- [ ] Propose and independently review an amendment to the existing CallMD source
  selection/response contract before changing any public wire or UI. Explicitly
  define legacy versus source identity, exact frame/seek semantics, errors,
  ownership, cancellation and resource bounds. Do not invent new auth/schema.
- [ ] After CONTRACT_TEST_REVIEW and BACKEND_INTERFACE_REVIEW, implement actual
  player source selection, verified reopen, queue/handle revocation and UI with
  tests covering quarantine at preparation, install, ready, paused and seek.
  Durable state precedes notification; retries and restart consult durable state.
  Never claim already rendered audio is recalled or revocation is instantaneous.
- [ ] Independently review backend, UI and contract; test actual native player
  decoding/queue and actual installed isolated package. Audio export, recovery,
  ASR, MCP and two-app-plus-mic remain separate endpoint gates.

### Source selector checkpoint — 2026-10-31

The TypeScript bridge represents the candidate source catalog/open wire and
rejects invalid, duplicate or unsafe-number source identities. Source and legacy
playback states have distinct types; status/control responses are checked
against the issued handle and selection. The review controller retains
unconfirmed close handles within the loaded window module, and does not
acknowledge close while an open is unresolved. This custody does not survive
webview reload or replace native owner/adoption cleanup.

The routed-recording selector loads the explicit catalog and presents an
unavailable state on failure; it does not substitute a legacy/system channel.
Selection epoch and committed UI revision fence late responses. Mock IPC and
controller/helper tests pass; these are not mounted browser, native command,
installed-package, real audio, or renderer-adoption evidence. The required
backend/interface review and remaining quarantine/restart cases remain open.

## Task3 — PID and complete installed acceptance

- [ ] Identify and demonstrate a supported arbitrary-PID loss repair without
  suppressing timeline gaps or relying on a cooperative test target.
- [ ] Build/install a frozen isolated package; qualify full PID and endpoint
  capture, playback, output/export, persistence/recovery and required UAT.
- [ ] Update owning documentation and progress; merge/tag only after both full
  actual installed modes and required reviews/CI pass.
