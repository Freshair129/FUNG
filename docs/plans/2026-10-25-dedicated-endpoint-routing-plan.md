---
version: "0.1.1b"
created_at: "2026-10-25"
last_update: "2026-10-26"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "implementation-plan"
  scope: "Dedicated endpoint feasibility then multi-source product integration"
  language: "Thai-first"
---

# Dedicated endpoint routing implementation plan

> For agentic workers: use subagent-driven-development under master-plan §9, with
> independent review per task. The native spike is authorized; product integration
> cannot advance past its native evidence gate. No source-test or probe result
> substitutes for installed acceptance.

**Goal:** preserve source audio through stream destruction/recreation and keep
multiple routed applications on separate durable tracks, then qualify a rebuilt
installed package before release.

**Architecture:** explicit IMMDevice IDs, one endpoint per application, optional
owned silent keepers, typed route admission and per-source custody through the
existing native capture/Genesis pipeline. No automatic system fallback.

**Tech stack:** Windows WASAPI, Rust/Tauri, GenesisBlockDB adapter, React/TypeScript,
existing Product MCP. No new driver in the initial existing-device scope.

**Spec:** [Dedicated endpoint routing](../specs/2026-10-25-dedicated-endpoint-routing-design.md).

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. Preserve normal profile and original failure
evidence. Keep test endpoints muted and preserve unrelated endpoint state. No fabricated PCM, relaxed
loss tolerance, default-device mutation, target injection or implicit system fallback.

## Review focus

- Endpoint mix is not PID-filtered audio; never claim a security isolation guarantee.
- Stop discards unplayed render buffers; native played-prefix evidence is the oracle.
- Existing two-channel filename inference cannot silently own new source identities.
- Late contamination invalidates derived output and access, not just future exports.
- Dota/Focusrite and all unrelated apps/settings remain untouched during tests.

## Task 0 — existing-device feasibility

Files: external retained tools/receipts under
`G:\AI-Studio\verification\fung-endpoint-routing-20261025`; repository design and
the dated verification report/index. This is throwaway code, not a product adapter.

- [x] Inventory all native endpoints, stable IDs, role defaults, mix formats and
  per-endpoint session/error state without capture/settings mutation.
- [x] Freeze independent marker policy, explicit-endpoint render/capture tools and
  sources/hashes. Use idle nondefault endpoints and recheck sessions at native start.
- [x] Run two simultaneous marker routes and two Stop/destroy/recreate cycles each.
- [x] Independently qualify played edges, separate-source fit, cross-gain threshold,
  silence floor, native gaps/queue errors and all process exit codes.
- [x] Run wrong/missing endpoint, unknown-session preflight/runtime contamination
  rejection, and keeper-disabled comparison. Record unsupported/ambiguous cases.
- [x] Independent feasibility review; preserve failure/NOT_PROVEN rather than
  adjusting criteria. Only qualified native evidence unlocks Task 1 execution.

## Task 1 — native source inventory and route admission

Create `src-tauri/src/capture_routing.rs`; register module in `src-tauri/src/lib.rs`.
Modify `live_meeting.rs` DTO dispatch and the native command registration only after
pure policy/native inventory tests pass. Keep the old device preference format.

- [x] Define `EndpointDescriptor`, `ApplicationIdentity`, `CaptureRoutePlan` and
  `ValidatedRoutePlan`, with opaque native ID, UUID source ID, process creation
  identity, policy version and explicitly allowed child identities. Native session
  instance IDs are recorded as provenance; same-process stream recreation remains
  allowed without expanding the authorized process set.
- [x] Failing policy tests: duplicate endpoint/source ID, stale or reused PID,
  unknown session, PID-zero/cross-process ambiguity, inventory error, missing
  endpoint, and default-role-only routing. No failed check may choose a default.
- [x] Implement read-only native inventory and route preflight. Race revalidation
  occurs during admission, not only when a UI dropdown first loads.
- [x] Tests prove each route remains a distinct source, session monitoring is
  diagnostic rather than a privacy guarantee, and capabilities fail explicitly.
- [x] Independent code review; test fixture errors cannot enter shipped defaults.

## Task 2 — authoritative source custody and endpoint capture

Create `src-tauri/src/live_endpoint_audio.rs` as a focused adapter; modify
`live_system_audio.rs` shared reader helpers only as necessary. Modify
`genesis_adapter.rs`, `live_meeting.rs`, `live_transcript.rs`, `audio_export.rs` and
`recording_review.rs` in one coherent source-model change, preserving legacy paths.

- [ ] Add failing Genesis tests for source/chunk FK ownership in both transcript
  modes, cross-recording mismatch, atomic binding/checkpoints and duplicate retry.
- [x] Add additive `capture_sources` and `capture_source_chunks` schema via the
  existing adapter migration owner. Old audio rows remain unchanged and readable.
- [ ] Generalize capture events/track maps to owned source identity; never leak
  unbounded static strings. New chunks live in source-owned directories; independent
  clocks preserve global QPC origin and per-source frame provenance.
- [ ] Add failing tests for bounded writer overflow/final tail, one-source removal,
  stop races, unknown session, quarantine-before-enrich, and independent checkpoints.
- [ ] Implement exact-ID endpoint capture and owned silent keeper lifecycle, with
  explicit failures and no global setting changes. Port only proven native behavior.
- [ ] Recovery, review, playback and export query authoritative source bindings.
  Legacy filename inference is limited to unbound old rows; two apps never become
  one `system` track. Export repeated per source retains IDs and PCM hashes.
- [ ] Quarantine affected source epochs and invalidate dependent transcript,
  summary/task/retrieval artifacts; MCP/playback/export cannot bypass native state.
- [ ] Independent review and targeted native/custody suites pass before Task 3.

## Task 3 — Live routing UX and Product MCP

Modify `src/components/desktop/contracts.ts`, `LiveWorkspace.tsx/.css`,
`src/components/LiveMeetingPanel.tsx` and the existing Product MCP capture adapters.
Use the established shell/grid/tabs and existing MCP authorization contracts.

- [ ] Extend typed start options with a versioned route plan, not parallel booleans
  that allow contradictory system/PID/endpoint sources.
- [ ] Failing UI/contract tests: stale inventory, duplicate destination, zero target
  audio/waiting, foreign-session failure, route-specific waveform and source labels.
- [ ] Add read-only inventory/validation and explicit route execution; native runtime
  revalidates all plans, source state and capture/playback admission for UI and MCP.
- [ ] Show each app→endpoint→track and endpoint-mix limitation clearly. Device routing
  is verified after the app plays; no undocumented arbitrary-PID routing setter.
- [ ] Expose terminal/partial source statuses to Archive and MCP, no synthetic success.
- [x] Record the user's explicit disposition: keep PID mode and no release until
  both PID and endpoint modes pass. Do not hide, disable or waive the original defect.
- [ ] Independent UI/MCP review and meaningful contract tests.

## Task 4 — complete source and actual installed acceptance

- [ ] Fresh frontend checks/build, full Rust library tests, Clippy, docs paths and
  diff checks; freeze exact source revision, binaries, installer and runtime hashes.
- [ ] Build/install isolated candidate while preserving normal profile and mute.
  Native owned controls may run without taking desktop focus; visual checks prefer
  the Codex browser where appropriate, clearly separated from native execution.
- [ ] Installed two-route capture with two independent app/player processes,
  microphone source and stream Stop/recreate. Check each waveform/source, actual
  durable PCM, transcript/output completion and per-source warnings.
- [ ] Installed export/reopen, stable track identities, raw chunk/export PCM equality,
  retry, source removal/contamination and crash recovery. Human speech proof is a
  separate gate; do not repeatedly require speech to reprove a synthetic test.
- [ ] Independent installed/client review; no native standalone probe substitute.
- [ ] Update source/installed/CI/release evidence separately. Exact-head CI/reviews,
  normal merge, merged-source package qualification and tag only after all required
  accepted gates, including the original PID-mode release disposition.

## Rollback and open gates

Keep all old source rows, audio, preferences and failed receipts. Failed new routes
stop explicitly and retain private evidence; no fallback or automatic user-device
rerouting. Feature rollback disables new routing in a schema-compatible build,
leaving additive tables and new captured data intact. Do not assume an older binary
can open the newer schema, drop tables to downgrade, or restore an old backup over
new recordings. Binary rollback needs separately qualified recovery/backup handling.
No normal-profile test or virtual-driver installation is implicit.

Current state: bounded native feasibility and negative controls independently
qualified; [evidence](../verification/implementation-reports/2026-10-25-dedicated-endpoint-feasibility.md).
Read-only native admission source integration is separately verified in the
[foundation report](../verification/implementation-reports/2026-10-25-route-admission-foundation.md).
Task 2 now includes the additive schema and trusted in-process source-custody
foundation; [scope and checks](../verification/implementation-reports/2026-10-26-source-custody-foundation.md).
This is not a registered capture executor. Consumer fencing, derived-output
invalidation, source-aware recovery/export and native UI/MCP execution remain open.
Multi-source capture integration, new FUNG installer, installed acceptance, merge
and tag remain open. PID #96 is still a release blocker by user decision.
