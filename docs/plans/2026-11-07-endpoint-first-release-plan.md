---
version: "0.1.1b"
created_at: "2026-11-07"
last_update: "2026-11-08"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-audio-capture"
  doc_type: "plan"
  scope: "Deliver dedicated-endpoint capture as the release per-app mode; mark PID capture experimental"
  language: "English"
---

# Endpoint-first release plan

Decision: [Endpoint-first release; PID experimental](../decisions/2026-11-07-endpoint-first-release-pid-experimental.md)
(approved by the owner, 2026-11-07). This plan schedules work. It is not
evidence that any task is complete.

Lead: Claude. Executor: local Codex CLI. Model rule: code, spec and docs work
uses `gpt-6-luna` with effort `max`; test, debug and computer-use work uses
`gpt-6.1-terra` with effort `high`.

## Global constraints

- Thai UI labels, named exports, the existing LiveWorkspace CSS/BEM convention
  (`live-workspace__*`), the established shell, grid and tab patterns. SEC-1 and
  SEC-2 are unchanged.
- No capture fallback, fabricated PCM, relaxed tolerance, target injection,
  implicit or automatic device rerouting, or new driver. The app never changes
  Windows routing. The user routes apps in Windows settings.
- The UI never reports success that native did not report. Partial or failed
  sources are shown as they are.
- Codex does not commit, push, merge, tag, close issues or release.

## Work packages

### E1 — Decision and gate records (lead) — done in this change

The ADR, this plan, and a superseding note in the
[routing plan](2026-10-25-dedicated-endpoint-routing-plan.md) Task 3. Issue #96
is relabelled from release blocker to experimental-mode defect.

### E2 — PID mode marked experimental (Luna max, then Terra high)

Spec:

1. In `src/components/desktop/LiveWorkspace.tsx`, the application tab label
   becomes `เลือกรายแอป (ทดลอง)`. Keep the tab, its id and its keyboard
   behaviour.
2. When `captureSourceMode === "application"`, show a note above the start
   control with `role="note"` and class `live-workspace__experimental-note`
   (add CSS next to the existing `live-workspace__device-issue` style, using
   existing tokens). Text:
   `โหมดทดลอง: เสียงอาจขาดหายช่วงสั้น ๆ เมื่อแอปที่เลือกหยุดหรือเริ่มเล่นเสียงใหม่ สำหรับงานสำคัญให้ใช้โหมด "อุปกรณ์เฉพาะต่อแอป"`
3. In Archive/recording review, recordings captured in PID mode show a badge
   `ทดลอง` next to the source label. Identify PID recordings by the exact source
   value the native capture path writes for selected-process capture. Find it in
   `src-tauri/src/lib.rs` / `live_application_audio.rs`, where `recording.source`
   or the source rows are written. If no stable marker exists, stop and report
   it. Do not invent a schema field.
4. Contract tests in the existing Node suites (`tests/callmd*.test.mjs`) cover
   the label, the note's presence only in application mode, and the Archive badge.

### E3 — Live endpoint-routing UI (Luna max, then Terra high)

This implements routing-plan Task 3 for the desktop UI. The Product MCP
adapters are out of scope for E3 and are tracked as E3b.

Native contract (already on `main`, `src-tauri/src/capture_routing*.rs`,
`routed_capture.rs`):

- `capture_route_inventory({ processIds?: number[] }) -> NativeInventory`.
  This returns endpoints with `state`, `mixFormat` and `sessions`, the default
  roles, and per-PID `ApplicationIdentity` observations. `Observation<T>` is
  serialized as `{ status: "known", value: T } | { status: "unavailable", value:
  string }` (`#[serde(tag = "status", content = "value")]`).
- `capture_route_validate({ plan }) -> ValidatedRoutePlan | RouteError`.
- `capture_route_start({ projectId, plan }) -> RoutedCaptureStatus`.
- `capture_route_status() -> RoutedCaptureStatus | null`.
- `capture_route_stop() -> RoutedCaptureStatus`.
- `CaptureRoutePlan = { policyVersion: 1, routes: CaptureRoute[] }`, with 1–2
  routes. `CaptureRoute = { sourceId (UUID v4), label, endpointId, application:
  ApplicationIdentity, allowedChildren: [] }`. The UI sends `allowedChildren: []`
  in v1.
- `RouteError.code` is one of the 16 `Rejection` variants, serialized in
  PascalCase because the enum has no `rename_all`: `PolicyVersion`,
  `RouteCount`, `InvalidSourceId`, `DuplicateSource`, `DuplicateEndpoint`,
  `InvalidInput`, `InventoryError`, `StaleInventory`, `MissingEndpoint`,
  `AmbiguousEndpoint`, `DefaultEndpoint`, `EndpointInactive`, `StaleProcess`,
  `InvalidChild`, `AmbiguousSession` and `UnknownSession`.

UI spec:

1. Add a fourth source tab `อุปกรณ์เฉพาะต่อแอป` (id
   `live-source-tab-routed`, mode value `"routed"`) after `เลือกรายแอป (ทดลอง)`.
   Extend the tab keyboard handler to cover four tabs.
2. Routed mode panel:
   - Explanation (`live-workspace__muted`):
     `บันทึกเสียงของแอปผ่านอุปกรณ์เสียงที่แยกไว้ให้แอปนั้นโดยเฉพาะ ตั้งค่าใน Windows: การตั้งค่า > ระบบ > เสียง > Volume mixer แล้วเลือกอุปกรณ์เอาต์พุตของแอปเป็นอุปกรณ์ที่เลือกด้านล่าง เสียงทุกอย่างที่ออกอุปกรณ์นั้นจะถูกบันทึก โหมดนี้ไม่บันทึกไมโครโฟน`
   - Route rows. There is 1 row by default, and the user can add a 2nd with
     `เพิ่มเส้นทาง` (max 2). Each row has an app select, an endpoint select, a
     label input, and a remove button for row 2.
     - App select: take the processes from `liveCaptureDevices()` application
       options. Then call `capture_route_inventory` with those PIDs. List only
       PIDs whose identity is `known`.
     - Endpoint select: list endpoints with `state` Active. Disable, with the
       reason `อุปกรณ์เริ่มต้นของระบบ`, any endpoint whose id equals any default
       role. Disable an endpoint already chosen in the other row.
     - Label: default to the app name. Trim it and cap it at 64 characters.
   - Provide a `รีเฟรช` button that re-reads both inventories. Show inventory
     `errors` with `live-workspace__device-issue`.
3. Validate before start. `ตรวจเส้นทาง` calls `capture_route_validate`. Show
   readiness per route: `waitingForTarget` →
   `รอให้แอปเริ่มเล่นเสียงผ่านอุปกรณ์นี้`, `armed` → `พร้อมบันทึก`. Map every
   `RouteError.code` to a Thai message, defined in one exported table in
   `contracts.ts` with no untranslated fallback text. `staleInventory` and
   `staleProcess` prompt the user to refresh. Generate `sourceId` with
   `crypto.randomUUID()` per row, keep it while the row is unchanged, and
   regenerate it when the row's app or endpoint changes.
4. Start is enabled only after a successful validation of the current plan.
   Any edit invalidates it. Start calls `capture_route_start` with the active
   project id; if none is selected, Live creates and selects a project using
   the existing import naming (`ประชุมใหม่ N`) before starting routed capture.
   Native revalidates; show its error as it is. While this mode is active, the
   other source tabs and the normal start path are disabled. Only one capture
   mode can run at a time.
5. While recording, poll `capture_route_status` every 1 s. Stop polling on
   unmount or stop. For each source, show the label, the state (ready, failed
   with `firstFailure`, or drained) and the durable duration
   (`durableFrames / 48000` when `durableCountsKnown`; otherwise `—`). Show
   recording-level `phase`, `durationMs`, `firstFailure` and `warnings`.
   Status is plain text with `role="status"`. The recording-level
   `firstFailure` uses `role="alert"`.
6. Stop calls `capture_route_stop` and shows the final status. Then follow the
   same post-Stop navigation and refresh as the existing capture path
   (find it in `LiveWorkspace.tsx` / `App.tsx`). Do not start transcription for
   routed recordings; E4 owns that. Show
   `บันทึกเสียงแล้ว · การถอดความสำหรับโหมดนี้ยังไม่พร้อม` and keep the recording
   visible in Archive, where export and playback already support routed sources.
7. Typed wrappers live in `src/tauri.ts` (named exports, `canInvoke()`
   guards as in neighbours). Types live in `src/components/desktop/contracts.ts`.
8. Do not show a per-route live waveform in v1, because native status exposes
   no levels. Show text status only, and note this as a gap.

Tests (Terra round): contract tests for the plan shape (`policyVersion` 1,
UUID ids, `allowedChildren: []`, ≤2 routes, no duplicate endpoints); default
endpoints and inactive endpoints disabled; validation invalidated by edit;
error-code table completeness against the Rust enum; tab exclusivity; status
rendering of partial and failed sources; and no transcription start for routed.

### E3b — Product MCP routed capture adapters (later)

This is routing-plan Task 3's MCP portion. It is scheduled after E3 lands.

### E3-G — Fail-closed guard for legacy audio jobs (done, PR #102)

`fill_transcript_gaps`, `create_detailed_transcript_draft` and local
diarization now call `require_legacy_media_source`. On a routed recording they
fail permanently with `routed_transcription_unavailable`. Archive disables those
actions with the same Thai reason. Before this guard, `transcript.retry`
reported success with zero work, and `transcript.detailed` read routed WAVs
outside custody.

### E4 — Transcription for routed recordings (spec, 2026-11-07)

This section answers the plan's open questions under the constraints of the
[routing design](../specs/2026-10-25-dedicated-endpoint-routing-design.md),
"Source custody" and "Contamination" sections. It uses the lead survey of
`main` 3f20b4b.

**Scope v1:** transcribe a routed recording after Stop, one ASR track per
source. No live (during-recording) transcription. No diarization. No schema
change.

1. **Eligibility (fail closed).** A routed recording is transcribable only when
   `recordings.source == "routed_endpoint"` and `status == "completed"`, with
   no quarantined source. In that state `completed_source_media::completed_manifest`
   succeeds. A `partial` or quarantined recording gets **no** transcript. The
   job fails permanently with code `routed_transcription_blocked` and the Thai
   reason `มีแหล่งเสียงที่ถูกกักกัน จึงไม่สร้างคำถอดความรวมสำหรับการบันทึกนี้`.
   This follows the design rule: "until lineage exists, any quarantined routed
   source blocks the combined transcript".
2. **Trigger.** After `capture_route_stop` returns a final status with phase
   `completed`, the backend enqueues one `transcript.retry` job for the
   recording, through the same job engine and outbox path that `create_job`
   uses. Find where that path is called and reuse it. Once that job succeeds,
   the backend calls `meeting_intel::queue_post_meeting(..., true)` exactly as
   the legacy Stop does. The frontend triggers nothing new; it only reflects
   job state.
3. **Dispatch.** In `job_engine.rs`, `JobKind::TranscriptRetry` branches on
   `recordings.source`. Legacy sources keep `fill_transcript_gaps` (E3-G
   guard). Routed sources call a new
   `live_meeting::transcribe_routed_recording(storage, data_root, runtime,
   language, project_id, recording_id, job_id)`. `transcript.detailed` and
   `speakers.diarize` stay refused for routed recordings (E3-G) in v1.
4. **Verified input only.** `transcribe_routed_recording`:
   - Register a `source_playback_lease::SourcePlaybackLease` for the recording
     before reading the manifest. Re-check `lease.is_available()` before each
     chunk and before publishing. Follow `audio_export.rs:274-281/343/415`.
   - Read `completed_manifest`. For each source in manifest order and each
     chunk in sequence, open it with `completed_source_media::open_verified_chunk`.
   - The whisper worker takes a file path. Copy the verified PCM into a
     **private temporary WAV** (same format) at
     `<data_root>/<project>/derived/<recording>/asr-tmp/<uuid>.wav`. Create it
     with create-new and pass that path to the existing worker request
     (`transcribe_chunk`-style JSON: `id`, `path`, `channel`, `startMs`).
     Delete it after the response, and on every error path, via a drop guard.
     Never pass `audio_chunks.file_path` or any custody path to the worker.
   - `startMs` for a chunk is its manifest `start_ms`. All sources share the
     recording clock already used by the manifest.
5. **Speaker attribution.** Each source maps to one speaker row:
   - Key `route:<sourceId>`, id `{project}::speaker::route:<sourceId>`, using
     the existing speaker id helper (~`live_meeting.rs:2634`).
   - Display name: the source's `registration.label`.
   - Upsert it the same way legacy speakers are created. Segments for that
     source carry that `speaker_id`; this is the v1 source lineage.
   - The worker `channel` field is set to `system`, because routed audio is
     remote-party audio; check the worker for valid values. Attribution comes
     from the speaker id, not the channel.
6. **Idempotence and coverage.** A chunk counts as covered when a segment
   exists with that source's speaker id and overlaps the chunk's
   `[start_ms, end_ms)`. This mirrors the legacy rule keyed on speaker instead
   of filename. A retry transcribes only uncovered chunks. Set
   `audio_chunks.transcribed_at` as legacy does.
7. **Publication fence.** Before each segment commit, re-check the lease and
   `capture_source_custody::require_capture_source_access`. On revocation, stop,
   keep the segments already committed, and fail the job permanently with
   `routed_transcription_revoked`. Do not delete prior segments. A later
   quarantine follows the existing invalidation/audit rule; v1 adds no new
   invalidation.
8. **Frontend.**
   - Remove the "not ready" message in `LiveWorkspace.tsx` and `App.tsx`
     (~L744, ~L307) and show the normal post-Stop job progress.
   - In RecordingReview, enable `transcript.retry` for routed recordings with
     status `completed`. Keep detailed and diarize disabled with the E3-G reason.
   - Show the per-source speaker names from the labels.
9. **Tests.** Terra round. Rust unit tests with the
   `routed_media_access.rs`/`audio_export.rs` fixtures:
   - eligible versus partial versus quarantined;
   - the lease revoked mid-run;
   - temp files removed on success and on error;
   - the worker never receives a custody path (use a mock worker or seam);
   - speaker mapping per source;
   - retry idempotence.

   Node contract tests for the UI changes.

**Not in v1:** live routed transcription, diarization within a source, segment
epoch lineage table, and mic in routed mode (decision B-04; the routing design
already anticipates a native-created mic source UUID).

### E6 — Microphone in routed mode (spec, 2026-11-07)

Decision B-04 was delegated to the lead by the owner on 2026-11-07: a routed
recording may include one microphone source. The routing design scopes "at
most two remote application routes plus one microphone". It also says the
microphone in a routed recording "receives a native-created source
UUID/binding". This spec is based on a lead survey of `main` after PR #103.

**Scope v1:** one optional microphone source per routed recording. It is
captured as a WASAPI capture endpoint, chosen explicitly by its endpoint id,
and stored with the same custody, spool, writer and failure rules as routed
application sources. After Stop it is transcribed as speaker `me`. The legacy
Live pipeline and the `live_capture_devices` command stay unchanged.

1. **Contract.**
   - Add `microphone: Option<CaptureMicrophone>` to `CaptureRoutePlan`, with
     `#[serde(default)]`. The type is
     `CaptureMicrophone { source_id, label, endpoint_id }` (camelCase,
     `deny_unknown_fields`). `policy_version` stays `1`, because v1 is
     unreleased and the field is optional.
   - The route cap stays `1..=2` in all five places that enforce it
     (`capture_routing.rs` ~L259, ~L500, ~L613; `routed_capture.rs` ~L250;
     `native_capture_group.rs` ~L117). The microphone is not a route. A plan
     with a microphone and no route is rejected with `RouteCount`; mic-only
     recording is legacy mode.
   - Microphone `sourceId` follows the route rules: a canonical, non-nil UUID,
     unique across all routes and the microphone (`DuplicateSource`). The
     frontend creates it with `crypto.randomUUID()` and regenerates it when the
     device changes.
   - New `Rejection` variant `MicrophoneUnavailable`. It covers a microphone
     endpoint that is missing, inactive, not a capture-flow endpoint, or whose
     identity changed between validation and start. Update the TypeScript
     mirror and `ROUTE_ERROR_MESSAGES` with the Thai message
     `ไม่พบไมโครโฟนที่เลือก หรือไมโครโฟนไม่พร้อมใช้งาน`.
2. **Inventory.** Add `inputs: Vec<InputEndpointDescriptor>` to
   `NativeInventory`, where `InputEndpointDescriptor { id, name, state,
   mixFormat }` uses the same `Observation` wrapping as `EndpointDescriptor`.
   Enumerate it with `EnumAudioEndpoints(eCapture, …)`. Keep it separate from
   `endpoints`, which are render-only, so a capture endpoint can never satisfy
   a route and a render endpoint can never satisfy the microphone.
3. **No fallback.** Never use a default capture role or a default input device.
   If no microphone is selected, the plan carries `microphone: null` and the
   recording has no microphone source. If the selected endpoint is unavailable
   at validate or start, reject with `MicrophoneUnavailable`. Never record
   without it, and never substitute another device.
4. **Native capture.** Generalize
   `live_system_audio::spawn_owned_endpoint_capture_thread` with an explicit
   flow parameter, `RenderLoopback` or `Capture`:
   - `RenderLoopback` keeps today's behaviour exactly: `eRender` check and
     `LOOPBACK | AUTOCONVERTPCM | SRC_DEFAULT_QUALITY`.
   - `Capture` requires `eCapture` and initializes with
     `AUTOCONVERTPCM | SRC_DEFAULT_QUALITY`, without `LOOPBACK`. It uses the
     same 48 kHz, 2-channel, 16-bit format, the same identity re-check, the
     same packet, QPC and session-audit path, and the same buffer duration.
   - If `NativeCaptureSessionAudit` encodes the loopback flag or assumes render
     flow, extend its receipt to record the flow. Do not weaken any existing
     audit check for render sources.
   - Record the device mix format observed at start and
     `"conversion": "wasapi-autoconvertpcm"` in the source registration
     metadata. Make no other resampling claim.
5. **Custody.**
   - Register the microphone with `kind: "microphone"`, the shared clock
     origin, `sample_rate: 48000`, and its capture endpoint id in
     `endpoint_id`. Change the microphone rule in `register_capture_sources`
     (~L309) from "endpoint_id must be None" to:
     - the endpoint identity must be valid (`valid_endpoint_identity`);
     - it must differ from every route endpoint id;
     - it is not counted toward the 2-endpoint cap.
     The limits of at most one microphone and 1–3 sources total stay.
   - No microphone row exists today, because no code path ever creates one, so
     no data migration is needed.
   - The active-registration loader used by `native_capture_group` (today
     `load_active_endpoint_registration`, ~L233, which requires
     `kind == "endpoint"`) must also accept `kind == "microphone"` for the
     microphone ticket only, and return the kind. Choose between a sibling
     loader and a kind-returning loader. Either way, the caller declares the
     expected kind and a mismatch is a hard error. The existing test that
     rejects a microphone row through the endpoint path
     (`capture_source_custody_tests.rs` ~L446) must keep passing for the
     endpoint path.
   - `native_capture_group::prepare_group` admits at most two endpoint tickets
     plus at most one microphone ticket. A microphone ticket opens the
     `Capture` flow, and an endpoint ticket opens `RenderLoopback`.
   - `append_capture_source_chunk` keeps requiring
     `recording.source == "routed_endpoint"`.
6. **Coordinator (`routed_capture.rs`).**
   - The microphone source gets its own writer, private spool, sink, coalescer
     and `source_worker`, exactly like a route.
   - It has no target and no `WaitingForTarget` state. It is ready once its
     stream delivers its first valid packet.
   - Failure rules are unchanged: any source fault cancels the group, the
     recording becomes `partial`, and the faulted source is quarantined. A
     microphone fault therefore blocks transcription (E4 eligibility).
   - `RoutedSourceStatus` gains `kind: "endpoint" | "microphone"`. The source
     label remains in the plan and frontend state.
7. **Transcription (E4 delta).** In `transcribe_routed_recording`, a
   `microphone` source maps to the legacy self speaker: key `me`, id
   `speaker_id_for(project, "me")`, display name `เรา`, and worker `channel`
   `CHANNEL_MIC`. Endpoint sources keep `route:<sourceId>` and `CHANNEL_SYSTEM`.
   Coverage and idempotence are keyed on that speaker id. Read the kind from
   the custody registration, never from the label.
8. **Frontend (`LiveWorkspace.tsx`).**
   - In routed mode, show a microphone select labelled `ไมโครโฟน`. Its options
     come from inventory `inputs` with state active, using WASAPI ids, not the
     legacy cpal ids from `live_capture_devices`.
   - The first option is `ไม่บันทึกไมโครโฟน` (value `""`), and it is the
     default. Selection is never automatic.
   - `routePlanForDrafts` adds `microphone` when one is selected, using the
     label `ไมโครโฟน`.
   - Show the microphone source in the routed status list with its label. The
     legacy mic picker stays hidden in routed mode.
   - Refresh the inventory with the existing inventory refresh. Add no new
     polling.
9. **Not in v1:**
   - more than one microphone;
   - live (during-recording) transcription;
   - echo cancellation, or mixing the microphone into a route;
   - Product MCP adapters (E3b);
   - level meters.
10. **Tests (Terra round).**
    - Validation:
      - a microphone with zero routes;
      - a missing, inactive or render-flow id used as the microphone;
      - a capture id used as a route;
      - a duplicate source id between a route and the microphone;
      - an identity change between validate and start.
    - Custody: register a microphone with a capture id; reject a microphone id
      equal to a route id, a second microphone, and four sources; check the
      loader's kind mismatch in both directions.
    - Group and coordinator, using the existing fakes in
      `routed_capture_tests.rs`:
      - two routes plus a microphone complete;
      - a microphone fault leaves the recording `partial` and quarantines the
        microphone source;
      - a microphone ticket opens the `Capture` flow.
    - Transcription: the microphone maps to `me` and `CHANNEL_MIC`, the route
      maps to `route:<id>`, and a retry is idempotent across both.
    - Node contract tests:
      - the plan shape with and without a microphone;
      - the default is no microphone;
      - the new rejection message;
      - the microphone row in status.

Native WASAPI capture-flow behaviour on real hardware is verified in E5. The
E5 runbook adds a step: record two apps plus a microphone, then check that the
three tracks and the `เรา` transcript are present.

### E5 — Installed endpoint acceptance (owner, hardware)

This is routing-plan Task 4. It needs the owner's machine with a second output
endpoint and an installed candidate. **Stop point:** the lead prepares the
runbook and the candidate, and the owner runs or approves the session.

## Open gaps recorded

- E2.3 (PID Archive badge) is stopped. Native writes `recording.source =
  "microphone"` for selected-process recordings, and the PID appears only in an
  audit event. A badge needs a stable recording-level marker, which requires a
  native change and its own spec. Backlog B-15.
- Routed status carries `sourceId` but no label. After a webview reload during
  a routed recording, the UI can show only the source id.

- Routed mode records no microphone yet. Decided 2026-11-07: add it (E6, spec above).
- No live waveform for routed sources (native exposes no levels).

## Exit

E2–E4 are merged with green CI. E5 is accepted by the owner. Release, tag and
merge of release candidates remain owner actions.

## CHANGELOG

| Version | Date | Change |
| --- | --- | --- |
| 0.1.1b | 2026-11-08 | Documented first-run project creation before routed capture. |
