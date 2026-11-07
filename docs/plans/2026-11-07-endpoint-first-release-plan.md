---
version: "0.1.0b"
created_at: "2026-11-07"
last_update: "2026-11-07"
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
  serialized as `{ known: T } | { unavailable: string }`. Verify the serde shape
  in code before typing it.
- `capture_route_validate({ plan }) -> ValidatedRoutePlan | RouteError`.
- `capture_route_start({ projectId, plan }) -> RoutedCaptureStatus`.
- `capture_route_status() -> RoutedCaptureStatus | null`.
- `capture_route_stop() -> RoutedCaptureStatus`.
- `CaptureRoutePlan = { policyVersion: 1, routes: CaptureRoute[] }`, with 1–2
  routes. `CaptureRoute = { sourceId (UUID v4), label, endpointId, application:
  ApplicationIdentity, allowedChildren: [] }`. The UI sends `allowedChildren: []`
  in v1.
- `RouteError.code` is one of `policyVersion`, `routeCount`, `invalidSourceId`,
  `duplicateSource`, `duplicateEndpoint`, `invalidInput`, `inventoryError`,
  `staleInventory`, `missingEndpoint`, `ambiguousEndpoint`, `defaultEndpoint`,
  `endpointInactive`, `staleProcess`, `invalidChild`, `ambiguousSession` or
  `unknownSession`. Verify the exact serialized casing in code.

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
   project id. Native revalidates; show its error as it is. While this mode is
   active, the other source tabs and the normal start path are disabled. Only
   one capture mode can run at a time.
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

### E4 — Transcription for routed recordings (spec pending)

`job_engine.rs` handles `routed_endpoint` only for `ExportRender`. The lead
will investigate the transcription input path and write a spec before any code
round. The spec must answer: which source audio feeds ASR (one track per source
or a mix); how speaker attribution maps to routes; and how partial or failed
sources are reported.

### E5 — Installed endpoint acceptance (owner, hardware)

This is routing-plan Task 4. It needs the owner's machine with a second output
endpoint and an installed candidate. **Stop point:** the lead prepares the
runbook and the candidate, and the owner runs or approves the session.

## Open gaps recorded

- Routed mode records no microphone. Meetings may need mic plus routed app
  audio. Decision needed after E3: either add a mic source to the routed
  coordinator, or run it alongside. It is out of scope until decided.
- No live waveform for routed sources (native exposes no levels).

## Exit

E2–E4 are merged with green CI. E5 is accepted by the owner. Release, tag and
merge of release candidates remain owner actions.
