---
version: "0.1.1b"
created_at: "2026-09-20T18:30:00+07:00,RWANG"
last_update: "2026-09-20T22:05:05+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-spec"
  scope: "FUNG Desktop live capture device selection and source-channel routing"
implementation_verified: false
---

# FUNG — Live capture device routing

## 1. Decision and current evidence

The current Desktop Live Meeting implementation already has two durable source
channels:

- `mic`: microphone capture;
- `system`: Windows WASAPI render loopback for machine playback.

Each source has its own capture thread and chunk namespace. Before this slice,
both sources resolved through the operating system defaults. The implemented
preflight now enumerates and selects one microphone and one loopback output,
while preserving the existing default route when no explicit selection is made.

This change adds OBS-like device selection at the source level without changing
the existing two-channel ledger boundary.

Implementation status: native enumeration, persisted selections, renderer
selectors, refresh/error states, native start-time revalidation, and the
no-silent-fallback path are implemented and covered by local automated checks.
Real Windows device click-through is still **NOT_RUN**; therefore
`implementation_verified` remains false.

## 2. Scope

### In scope

- Enumerate available Windows capture devices for the `mic` source.
- Enumerate available Windows render devices that can be opened as loopback for
  the `system` source.
- Let the user choose one device per source before starting Live Meeting.
- Keep the existing separate `mic` and `system` durable chunks and playback
  channel labels.
- Persist the last valid selection in internal application configuration, with
  an explicit **ค่าเริ่มต้นระบบ** option.
- Revalidate the opaque device selection in native code at start time.
- Show the actually opened device in the live status and start result.
- Refresh the list and explain unavailable/disconnected devices without silently
  changing a user-selected route.

### Out of scope for this slice

- More than one microphone input at the same time.
- Per-application audio routing or process-specific capture like OBS Application
  Audio Capture.
- Arbitrary user-defined channel names or a new multi-channel database schema.
- Mixing, phase alignment, resampling policy changes, or speaker identity
  verification.
- Moving the database, credentials, model cache, or job state out of AppData.
- Automatic migration of existing recordings.

The existing `mic`/`system` source-channel meaning remains provenance, not a
verified claim that every utterance belongs to a particular person.

## 3. Native contract

### 3.1 Device enumeration

Add a read-only command:

`live_capture_devices() -> LiveCaptureDevices`

Illustrative response shape:

```text
{
  inputs: [{ id, name, isDefault, available }],
  loopbackOutputs: [{ id, name, isDefault, available }],
  selectedMicDeviceId: string | null,
  selectedSystemDeviceId: string | null,
  issue: string | null
}
```

`id` is an opaque native device key generated from the current direction/name/
ordinal enumeration. It is intentionally not exposed as a renderer-constructed
Windows endpoint GUID: the renderer must not construct or modify it, and the
native layer must resolve it again against the current device list. Persistence
therefore survives ordinary restart while the endpoint remains stably named and
enumerated; a device change is reported as unavailable rather than guessed.
The response may include a truthful issue/warning when enumeration is partial;
an empty list must not be presented as a successful absence of hardware when
enumeration failed.

### 3.2 Start options

Extend `live_meeting_start` with optional source selections:

```text
micDeviceId?: string | null
systemDeviceId?: string | null
```

Null/omitted values mean **ค่าเริ่มต้นระบบ** and preserve the current behavior.
An explicit selection that cannot be resolved or opened must return an
actionable device error. It must not silently switch to another device. If the
system source is disabled, `systemDeviceId` is ignored and no loopback stream is
opened.

The native boundary remains authoritative for host selection, stream format,
availability, and actual device name. The renderer-provided device name is
never trusted.

### 3.3 Persistence and lifecycle

- Store selected opaque keys in internal application configuration, not in
  GenesisBlockDB recording rows and not in exported audio artifacts.
- Validate the stored key on enumeration/start; if it is gone, show
  **อุปกรณ์เดิมไม่พร้อมใช้งาน** and offer **ค่าเริ่มต้นระบบ** or a new selection.
- Do not change a live session's route after capture starts.
- Stopping, restarting, playback, recovery, and existing channel custody remain
  compatible with `mic` and `system` names.

## 4. Desktop UI contract

Add a bounded **แหล่งเสียง** section to the Live Meeting preflight page; it is
not a sidebar action and does not move the recording entry point.

Required controls:

| Source | Control | Behavior |
|---|---|---|
| ไมโครโฟน | Select | Lists inputs, defaults to system default, required before start |
| เสียงระบบ | Toggle + Select | Toggle preserves current behavior; select is disabled when off |
| ทั้งสอง | Refresh | Re-enumerates devices and reports partial/failure state |
| ทั้งสอง | Status | Shows selected route before start and actually opened route after start |

The page must remain keyboard-operable and fit the supported desktop/compact
width without a new page-level scroll requirement. A disconnected selection is
an explicit error state, not silently replaced by another device.

## 5. Acceptance criteria

- The preflight UI lists the actual available microphone and loopback output
  devices on Windows.
- The user can choose a non-default microphone and start a `mic` capture.
- The user can choose a non-default loopback output and start a two-source
  capture.
- The start result/status reports the devices actually opened by native code.
- The resulting recording still contains separate `mic-*.wav` and
  `system-*.wav` chunks with no cross-channel filename or ledger attribution.
- Disabling system capture does not enumerate/open a system stream for the
  session and records microphone-only data.
- A missing selected device blocks or clearly offers an explicit fallback before
  capture; it never silently routes to an unintended device.
- A selected route survives app restart when still available, and the UI gives
  a recovery action when it is not.
- Existing AppData recordings and the approved user output-root design remain
  readable; this feature does not migrate or rewrite them.
- Rust tests, TypeScript/frontend tests, build, and a real Windows device
  click-through pass. Screenshots are visual evidence only; device identity,
  opened-stream status, WAV chunks, and ledger rows are functional evidence.

## 6. Verification plan

### Automated

- Native resolver tests for default selection, valid opaque key, unknown key,
  disconnected key, system-disabled behavior, and no silent fallback.
- Command contract tests for enumeration response and start-option serialization.
- Frontend tests for loading/refresh/error/selection states and the exact device
  IDs sent to the native command.
- Regression tests for current two-channel naming, recovery, playback, and
  output-root custody.

### Real Windows UAT

1. Enumerate devices and record the list as functional evidence.
2. Select a non-default microphone and a non-default output device.
3. Start, speak/play test audio, stop, and verify both actual device names.
4. Verify separate WAV headers/chunks and Genesis ledger channel values.
5. Disconnect a selected device before start and verify the explicit recovery
   path.
6. Restart the app and verify persisted selection or truthful unavailable state.
7. Repeat with system capture disabled and verify microphone-only behavior.

## 7. Risk, dependencies, and non-goals

Risk: **HIGH / C-3**. The change crosses Windows device enumeration, cpal stream
opening, native/renderer contracts, persisted settings, live capture, recovery,
and real-device acceptance.

Dependencies:

- Existing `ChannelKind::{Mic,SystemLoopback}` and `mic`/`system` custody remain
  the compatibility boundary.
- The recording output destination design is a separate change; this feature
  must use whichever validated chunks root is active without moving its custody
  rules.
- Real Windows audio hardware is required to close the runtime gate.

## Version diff

| Version | Change |
|---|---|
| 0.1.1b | Implemented the approved native/UI slice: device enumeration, persisted source selections, refresh/error states, native revalidation, and explicit-selection fail-closed behavior. Automated evidence is green; real Windows device UAT remains NOT_RUN. |
| 0.1.0b | Candidate spec for per-source device selection while preserving the two-channel capture contract. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-20 | beta | Implemented source-level mic/loopback routing and persisted selection with local Rust, frontend, build, and contract evidence; native Windows device UAT remains open. | working-tree | RWANG |
| 0.1.0b | 2026-09-20 | candidate | Defined native enumeration, per-source selection, persistence, no-silent-fallback behavior, UI contract, and Windows UAT. | working-tree | RWANG |
