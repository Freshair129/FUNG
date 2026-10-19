---
version: "0.1.0b"
created_at: "2026-08-12T03:08:00+07:00,Agent: ATHER"
last_update: "2026-08-12T03:08:00+07:00,Agent: ATHER"
status: "done_with_concerns"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  scope: "Restart, visual, device, and real-connector UAT evidence"
  doc_type: "uat-report"
---

# Task 11 — Runtime/UAT Gate Recheck

## Disposition

**DONE_WITH_CONCERNS.** The Windows desktop process can be closed and reopened,
and the existing Genesis-backed project/recording/transcript rows remain
readable after relaunch. The visual/keyboard, physical-device, and real-
connector gates remain blocked by environment prerequisites; no release claim
or feature-flag promotion is made.

## Evidence

### Restart/reopen smoke

1. Started `src-tauri/target/debug/fung.exe` and observed PID `37720`, title
   `FUNG`, and a non-zero main window handle.
2. Requested `CloseMainWindow()`; the prior process exited before the 15-second
   deadline.
3. Relaunched the same binary and observed PID `9088`, title `FUNG`, and a
   non-zero main window handle.
4. Read the app's Genesis projection in read-only mode before/after relaunch.
   Counts remained stable: `projects=1`, `recordings=1`,
   `transcript_segments=13`, `audit_events=1`.

This proves process reopen plus durable base-record visibility. It does **not**
close AC-111 because this dataset has `summaries=0`; post-meeting summary,
export, and evidence review after restart still need a completed local-model
meeting or a controlled fixture.

### Visual/keyboard UAT

- `npm run build` and the static external-tools checks remain green.
- A fresh Vite visual attempt listened on `127.0.0.1:4173`, but HTTP requests
  timed out with zero response bytes.
- `npx playwright screenshot` could not start because the Playwright Chromium
  executable is not installed. A direct Chrome headless attempt produced no
  screenshot because the local Vite endpoint was not responding.

Therefore 1200×780 screenshot and keyboard-only interaction are **blocked**,
not passed. Existing screenshots are historical artifacts and are not reused as
current UAT evidence.

### Physical-device UAT

- `where.exe adb` and `where.exe scrcpy` found no executable.
- `adb devices -l` cannot run, so no Android device or real microphone/system
  capture session is available from this host.

The device capture-isolation gate remains **blocked**.

### Real connector UAT

- `C:\Users\freshair\AppData\Roaming\Claude\claude_desktop_config.json`
  parses with `mcpServers: {}`.
- `GKS_MCP_ROOT` is present but its directory contains no server files.
- The repository contains only `tests/fixtures/fake_external_mcp.rs`; no
  approved vendor connector, endpoint, or credential is configured.

The real document/CRM connector gate remains **blocked** pending an approved
server and test credential. The local fixture path remains the only executable
connector evidence.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Recorded relaunch/process-window evidence and the exact blockers for visual/keyboard, physical-device, and real-connector UAT. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-12 | done_with_concerns | Rechecked restart smoke and recorded environment-bounded UAT blockers. | pending | ATHER |
