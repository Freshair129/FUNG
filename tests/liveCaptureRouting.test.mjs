// @req Live capture device routing spec 0.1.0b
// @tested tests/liveCaptureRouting.test.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const [rust, appAudio, lib, bridge, contracts, workspace, workspaceCss, settings, settingsCss] = await Promise.all([
  readFile(new URL("../src-tauri/src/live_meeting.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/live_application_audio.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/tauri.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/contracts.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/LiveWorkspace.tsx", import.meta.url), "utf8"),
  readFile(new URL("../src/components/desktop/LiveWorkspace.css", import.meta.url), "utf8"),
  readFile(new URL("../src/components/SettingsPanel.tsx", import.meta.url), "utf8"),
  readFile(new URL("../src/components/SettingsPanel.css", import.meta.url), "utf8"),
]);

test("native live capture exposes both source lists and a persisted selection", () => {
  assert.match(rust, /pub\(crate\) struct LiveCaptureDevices/);
  assert.match(rust, /inputs: Vec<LiveCaptureDevice>/);
  assert.match(rust, /loopback_outputs: Vec<LiveCaptureDevice>/);
  assert.match(rust, /applications: Vec<crate::live_application_audio::LiveCaptureApplication>/);
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
  assert.match(workspace, /captureSystem: captureSourceMode !== "mic"/);
});

test("explicit routes fail closed while default routes retain legacy behavior", () => {
  assert.match(rust, /validate_requested_device\(CaptureDeviceKind::Mic/);
  assert.match(rust, /if capture_system && selected_application\.is_none\(\)/);
  assert.match(rust, /resolve_capture_device\(kind, requested_id\)/);
  assert.match(rust, /เปิดเสียงระบบที่เลือกไม่สำเร็จ/);
  assert.match(rust, /จับเสียงระบบไม่ได้ \(\{error\}\) — อัดเฉพาะไมค์/);
});

test("Live offers active application audio as a distinct, revalidated source", () => {
  assert.match(rust, /applications: Vec<crate::live_application_audio::LiveCaptureApplication>/);
  assert.match(rust, /active_audio_applications\(\)/);
  assert.match(rust, /application_capture_available/);
  assert.match(appAudio, /SessionState::Active/);
  assert.match(rust, /application_process_id: Option<u32>/);
  assert.match(bridge, /applicationProcessId\?: number/);
  assert.match(bridge, /applicationProcessId: options\?\.applicationProcessId \?\? null/);
  assert.match(workspace, /เลือกแอป/);
  assert.match(workspace, /aria-label="แอปที่กำลังส่งเสียง"/);
  assert.match(workspace, /applicationProcessId: captureSourceMode === "application"/);
  assert.match(appAudio, /new_application_loopback_client/);
  assert.match(rust, /validate_active_audio_application/);
});

test("capture meters are recording-scoped and render separate measured channels", () => {
  assert.match(rust, /CaptureEvent::Level/);
  assert.match(rust, /waveform: waveform_peaks_from_i16\(samples, 24\)/);
  assert.match(rust, /live-audio-level/);
  assert.match(bridge, /LiveAudioLevelEvent/);
  assert.match(workspace, /live-workspace__channel-waveform/);
  assert.match(workspace, /ไมโครโฟน/);
  assert.match(workspace, /เสียงที่เลือก/);
  assert.match(workspace, /liveAudioLevels/);
  assert.match(workspaceCss, /live-workspace__channel-waveform/);
  assert.match(workspaceCss, /prefers-reduced-motion/);
});

test("settings categories use a visible horizontal tab grid without a scrolling tab rail", () => {
  assert.match(settings, /aria-orientation="horizontal"/);
  assert.match(settingsCss, /grid-template-columns: repeat\(4, minmax\(0, 1fr\)\)/);
  assert.match(settings, /event\.key === "ArrowRight"/);
  assert.match(settings, /event\.key === "ArrowLeft"/);
  assert.doesNotMatch(settingsCss, /\.settings-drawer__tabs\s*\{[^}]*overflow-y:\s*auto/s);
});
