// @req Recording output destination spec 0.1.0b
// @tested tests/recordingOutput.test.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const [native, lib, live, localApi, zoom, playback, bridge, contracts, shell, panel] = await Promise.all([
  readFile(new URL("../src-tauri/src/recording_output.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/live_meeting.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/local_api.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/zoom_sync.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/desktop_playback.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/tauri.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/contracts.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/DesktopShell.tsx", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/RecordingOutputPanel.tsx", import.meta.url), "utf8"),
]);

test("native output custody persists the default, current, and known roots", () => {
  assert.match(native, /recording-output\.json/);
  assert.match(native, /known_paths/);
  assert.match(native, /recording_output_get/);
  assert.match(native, /recording_output_set/);
  assert.match(native, /recording_output_reset/);
  assert.match(native, /capture_is_active\(\)/);
  assert.match(lib, /recording_output: Arc<Mutex<recording_output::RecordingOutputManager>>/);
  assert.match(lib, /document_dir\(\)/);
  assert.match(lib, /recording_output::recording_output_get/);
  assert.match(lib, /recording_output::recording_output_set/);
  assert.match(lib, /recording_output::recording_output_reset/);
});

test("new capture and ingestion paths use the selected root while legacy paths stay ledger-owned", () => {
  assert.match(live, /ensure_current_writable\(\)/);
  assert.match(live, /project_storage_path\(&state\.genesis, &project_id\)/);
  assert.match(localApi, /recording_output.*ensure_current_writable/s);
  assert.match(localApi, /create_project_named\(storage, &output_root/);
  assert.match(zoom, /recording_output.*ensure_current_writable/s);
  assert.match(zoom, /output_root\.join\("projects"\)/);
  assert.match(playback, /known_roots_from_config/);
  assert.match(playback, /allowed_projects_roots/);
});

test("desktop exposes the output surface, picker actions, and capture lock", () => {
  assert.match(contracts, /DesktopSurface = .*"output"/);
  assert.match(contracts, /showOutput: \(\) => void/);
  assert.match(shell, /id: "output".*ไฟล์บันทึก/s);
  assert.match(shell, /actions\.showOutput/);
  assert.match(panel, /pickRecordingOutputFolder/);
  assert.match(panel, /recordingOutputSet/);
  assert.match(panel, /recordingOutputReset/);
  assert.match(panel, /disabled=\{locked \|\| busy \|\| !nativeInvoke\}/);
  assert.match(panel, /ไฟล์บันทึกเดิมจะไม่ถูกย้าย/);
  assert.match(bridge, /recordingOutputGet/);
  assert.match(bridge, /recordingOutputSet/);
  assert.match(bridge, /recordingOutputReset/);
});
