---
version: "0.1.1b"
created_at: "2026-09-20T18:00:00+07:00,RWANG"
last_update: "2026-09-20T22:47:23+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-spec"
  scope: "FUNG Desktop recording output custody and destination selection"
  default_output_path: "C:\\Users\\pc\\Documents\\fung"
implementation_verified: false
---

# FUNG — Recording output destination

## 1. Problem and evidence

Desktop capture currently uses `AppState.data_root` for both internal application
state and user-owned audio. The live session therefore writes to
`%APPDATA%\\dev.fung.local\\projects\\<project>\\live\\<recording>\\chunks`.
This makes the captured WAV files difficult to find and couples user audio to
the internal database/runtime location.

## 2. Approved decision

Split the roots:

- **Internal data root** remains the Tauri app-data directory. GenesisBlockDB,
  credentials, model/runtime cache, temporary fetch/import staging and job
  state remain here.
- **Recording output root** is the user-visible root for new desktop projects,
  live chunks, imported/custodied media and generated exports.
- The default recording output root is resolved from the Windows Documents
  directory and the `fung` child folder. On this machine it is
  `C:\\Users\\pc\\Documents\\fung`; the implementation must not hard-code the
  username.

New output uses this layout:

```text
<output-root>\\projects\\<project-id>\\live\\<recording-id>\\chunks
<output-root>\\projects\\<project-id>\\imports\\<recording-id>\\<file>
<output-root>\\projects\\<project-id>\\exports\\<artifact>
```

The current output root is persisted in an internal configuration file. Known
previous output roots are retained for custody validation so changing the
destination does not make existing recordings unreadable.

Existing AppData recordings are not moved automatically. They remain readable
through the legacy AppData custody root; migration is a separate future action.

## 3. Native contract

Add these desktop commands:

| Command | Contract |
| --- | --- |
| `recording_output_get` | Return current path, default path, `isDefault`, writable state and a truthful issue message. |
| `recording_output_set` | Accept one absolute directory, create it when possible, validate it is writable, persist it, and reject changes while capture is active. |
| `recording_output_reset` | Restore the Documents\\fung default using the same validation and capture guard. |

The native folder picker remains the only way the UI selects a directory. The
renderer may submit the selected path to the native command, but native code
revalidates it before persistence or capture.

Playback, recovery, audio custody and export paths must accept the internal
legacy root plus every persisted output root. No arbitrary renderer-supplied
path becomes a read/write root.

## 4. UI contract

Add a dedicated desktop surface named `output` / **ไฟล์บันทึก**, reachable from
the existing sidebar menu. It contains:

- current destination path;
- writable/unavailable status;
- **เลือกโฟลเดอร์** native folder-picker action;
- **คืนค่าเริ่มต้น** action for `Documents\\fung`;
- a notice that the change affects new output and does not move old recordings;
- a disabled state while a live capture owns the storage boundary.

The page must not expose internal database, credential, model-cache or temporary
staging paths. The page must remain usable without scrolling at the supported
desktop and compact responsive widths.

## 5. Implementation status

The approved path split is implemented in the desktop working tree:

- `recording-output.json` remains under the internal AppData root; it stores the
  current, default and known previous output roots.
- New projects, live captures, local uploads and new Zoom imports resolve their
  project storage from the selected output root. Existing projects continue to
  use their ledger-owned storage path.
- Native playback custody accepts AppData legacy projects plus every persisted
  output root. No automatic migration is performed.
- The `ไฟล์บันทึก` surface uses the native folder picker and disables changes
  while the native capture guard owns the session.

Automated evidence currently passes: Rust library `478 passed / 1 ignored`,
focused output contract `3/3`, Desktop shell `11/11`, Desktop integration
`6/6`, live routing `4/4`, and the Vite production build. A real native output
smoke that records WAV under `Documents\fung`, changes the folder, restarts the
app, and reviews an old AppData recording remains **NOT_RUN**; therefore this
document intentionally keeps `implementation_verified: false`.

## 6. Acceptance criteria

- A new real desktop recording writes its WAV chunks below
  `C:\\Users\\pc\\Documents\\fung\\projects\\...` by default.
- A user can select another writable local directory from the **ไฟล์บันทึก**
  surface and the selection survives an app restart.
- New imports and exports use the selected output root.
- Changing the destination while recording is rejected without changing the
  active session.
- Existing AppData recordings remain reviewable and playable.
- An unavailable/deleted destination is shown as unavailable and prevents a new
  capture with an actionable error; it does not silently fall back to AppData.
- The internal DB, credentials and model/runtime cache remain under AppData.
- Rust, TypeScript, focused desktop tests, build and a real output-path smoke
  test pass. Screenshots are visual evidence only; the WAV path and ledger rows
  are the functional evidence.

## 7. Risk and non-goals

Risk: **HIGH / C-3**. The change crosses native path custody, recording,
import, export, playback, recovery and desktop navigation.

Non-goals:

- automatic migration or deletion of existing audio;
- moving GenesisBlockDB or secrets to Documents;
- cloud sync or sharing of the selected folder;
- allowing the web renderer to write arbitrary filesystem paths.

## Version diff

| Version | Change |
| --- | --- |
| 0.1.1b | Implemented the native output-root manager, `ไฟล์บันทึก` surface, selected-root ingestion paths and legacy playback allow-list; real native output smoke remains NOT_RUN. |
| 0.1.0b | Candidate spec separating internal AppData from selectable user-visible recording output. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-20 | beta | Implemented selectable output custody and UI; automated verification passed, native output-path smoke remains NOT_RUN. | working-tree | RWANG |
| 0.1.0b | 2026-09-20 | candidate | Defined Documents\\fung default, native destination selection, custody history and non-migrating legacy behavior. | working-tree | RWANG |
