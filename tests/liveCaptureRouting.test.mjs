// @req Live capture device routing spec 0.1.0b
// @tested tests/liveCaptureRouting.test.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const [rust, lib, bridge, contracts, workspace] = await Promise.all([
  readFile(new URL("../src-tauri/src/live_meeting.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/tauri.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/contracts.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/LiveWorkspace.tsx", import.meta.url), "utf8"),
]);

test("native live capture exposes both source lists and a persisted selection", () => {
  assert.match(rust, /pub\(crate\) struct LiveCaptureDevices/);
  assert.match(rust, /inputs: Vec<LiveCaptureDevice>/);
  assert.match(rust, /loopback_outputs: Vec<LiveCaptureDevice>/);
  assert.match(rust, /selected_mic_device_id: Option<String>/);
  assert.match(rust, /selected_system_device_id: Option<String>/);
  assert.match(rust, /pub\(crate\) fn live_capture_devices/);
  assert.match(lib, /live_meeting::live_capture_devices/);
});

test("the start contract carries opaque per-source route ids into native capture", () => {
  assert.match(bridge, /micDeviceId\?: string;/);
  assert.match(bridge, /systemDeviceId\?: string;/);
  assert.match(bridge, /micDeviceId: options\?\.micDeviceId \?\? null/);
  assert.match(bridge, /systemDeviceId: options\?\.systemDeviceId \?\? null/);
  assert.match(rust, /mic_device_id: Option<String>/);
  assert.match(rust, /system_device_id: Option<String>/);
  assert.match(rust, /resolve_capture_device\(device_kind, device_id\.as_deref\(\)/);
});

test("the live preflight surface exposes explicit mic and loopback selectors", () => {
  assert.match(contracts, /captureDevices: LiveCaptureDevices \| null/);
  assert.match(contracts, /refreshCaptureDevices: \(\) => void \| Promise<void>/);
  assert.match(workspace, /aria-label="อุปกรณ์ไมโครโฟน"/);
  assert.match(workspace, /aria-label="อุปกรณ์เสียงระบบ"/);
  assert.match(workspace, /รีเฟรชอุปกรณ์/);
  assert.match(workspace, /micDeviceId: micDeviceId \|\| undefined/);
  assert.match(workspace, /systemDeviceId: systemDeviceId \|\| undefined/);
});

test("explicit routes fail closed while default routes retain legacy behavior", () => {
  assert.match(rust, /validate_requested_device\(CaptureDeviceKind::Mic/);
  assert.match(rust, /if capture_system \{/);
  assert.match(rust, /system_device_id\.is_some\(\)/);
  assert.match(rust, /เปิดเสียงระบบที่เลือกไม่สำเร็จ/);
  assert.match(rust, /จับเสียงระบบไม่ได้ \(\{error\}\) — อัดเฉพาะไมค์/);
});
