// @req FR-101
// @tested tests/desktopBootstrap.test.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  hasSupabaseConfig,
  isMobileTauriPlatform,
  requiresSupabaseConfig,
  resolveBodySurface,
  resolveRootRoute,
} from "../src/lib/bootstrap.ts";

test("Tauri root boots the desktop app without Supabase configuration", () => {
  assert.equal(
    resolveRootRoute({
      path: "/",
      surface: null,
      isTauriRuntime: true,
      isMobilePlatform: false,
      mobileViewport: false,
    }),
    "desktop",
  );
  assert.equal(resolveBodySurface({ path: "/", isTauriRuntime: true }), "app");
  assert.equal(hasSupabaseConfig(undefined), false);
  assert.equal(hasSupabaseConfig({ VITE_SUPABASE_URL: "", VITE_SUPABASE_ANON_KEY: "" }), false);
});

test("web root keeps the landing surface", () => {
  assert.equal(
    resolveRootRoute({
      path: "/",
      surface: null,
      isTauriRuntime: false,
      isMobilePlatform: false,
      mobileViewport: false,
    }),
    "landing",
  );
  assert.equal(resolveBodySurface({ path: "/", isTauriRuntime: false }), "landing");
});

test("desktop bootstrap keeps Supabase consumers behind lazy boundaries", async () => {
  const [mainSource, appSource] = await Promise.all([
    readFile(new URL("../src/main.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/App.tsx", import.meta.url), "utf8"),
  ]);

  assert.doesNotMatch(mainSource, /^import .*\.\/landing\/LandingPage/m);
  assert.doesNotMatch(mainSource, /^import .*\.\/mobile\/MobileApp/m);
  assert.doesNotMatch(mainSource, /^import .*\.\/web\/AuthGuard/m);
  assert.match(mainSource, /^import \{ App \} from "\.\/App";/m);
  assert.doesNotMatch(mainSource, /import\("\.\/App"\)/);

  assert.doesNotMatch(appSource, /^import .*\.\/components\/AccountLoginPanel"/m);
  assert.doesNotMatch(appSource, /^import .*\.\/components\/DevicePairingPanel"/m);
  assert.match(appSource, /import\("\.\/components\/AccountLoginPanel"\)/);
  assert.match(appSource, /import\("\.\/components\/DevicePairingPanel"\)/);
});

test("left action rail contains added actions instead of overlapping the power dock", async () => {
  const css = await readFile(new URL("../src/styles.css", import.meta.url), "utf8");
  const sidebarRule = css.match(/\.fab-sidebar\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? "";

  assert.match(sidebarRule, /grid-auto-rows:\s*40px/);
  assert.match(sidebarRule, /overflow-y:\s*auto/);
  assert.doesNotMatch(sidebarRule, /grid-template-rows:\s*repeat\(6,/);
});

test("microphone rail control opens the real Live Meeting panel", async () => {
  const appSource = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");
  const labelIndex = appSource.indexOf('aria-label="Open Live Meeting"');
  const buttonStart = appSource.lastIndexOf("<button", labelIndex);
  const buttonEnd = appSource.indexOf("</button>", labelIndex);
  const microphoneControl = appSource.slice(buttonStart, buttonEnd + "</button>".length);

  assert.notEqual(labelIndex, -1);
  assert.match(microphoneControl, /setLiveMeetingOpen\(true\)/);
  assert.match(microphoneControl, /setActiveAnchor\("P1"\)/);
  assert.match(microphoneControl, /P1:\s*"live-capture"/);
  assert.doesNotMatch(microphoneControl, /setRecording/);
});

test("the Android shell boots the mobile app, not the desktop shell", () => {
  // Regression: `isTauriRuntime` alone routed every Tauri runtime — Android
  // included — to the fixed-width desktop shell, so the whole src/mobile/
  // tree was unreachable on the device it was written for.
  assert.equal(
    resolveRootRoute({
      path: "/",
      surface: null,
      isTauriRuntime: true,
      isMobilePlatform: true,
      mobileViewport: true,
    }),
    "mobile",
  );
});

test("an explicit surface parameter still overrides platform detection", () => {
  assert.equal(
    resolveRootRoute({
      path: "/",
      surface: "desktop",
      isTauriRuntime: true,
      isMobilePlatform: true,
      mobileViewport: true,
    }),
    "desktop",
  );
  assert.equal(
    resolveRootRoute({
      path: "/",
      surface: "mobile",
      isTauriRuntime: true,
      isMobilePlatform: false,
      mobileViewport: false,
    }),
    "mobile",
  );
});

test("mobile Tauri platforms are detected from the webview user agent", () => {
  const android =
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) " +
    "Chrome/125.0.0.0 Mobile Safari/537.36";
  const windows =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) " +
    "Chrome/125.0.0.0 Safari/537.36";
  const ipad = "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15";

  assert.equal(isMobileTauriPlatform(android), true);
  assert.equal(isMobileTauriPlatform(ipad), true);
  assert.equal(isMobileTauriPlatform(windows), false);
});

test("neither Tauri shell is blocked by a missing Supabase configuration", () => {
  // Capture, transcription and local review never touch Supabase; gating them
  // on a cloud credential would turn local-first operation into an error page.
  assert.equal(requiresSupabaseConfig("desktop", true), false);
  assert.equal(requiresSupabaseConfig("mobile", true), false);
  // The browser-only surfaces have nothing to show without a session.
  assert.equal(requiresSupabaseConfig("mobile", false), true);
  assert.equal(requiresSupabaseConfig("dashboard", false), true);
  assert.equal(requiresSupabaseConfig("auth-callback", false), true);
  assert.equal(requiresSupabaseConfig("landing", false), false);
});

test("the desktop shell can reach the backup panel", async () => {
  // Regression: every backup command's only caller lived in src/web/, a
  // surface the Tauri runtime can never route to, so none of them were
  // reachable from the app that can actually run them.
  const appSource = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");
  assert.match(appSource, /import\("\.\/components\/BackupPanel"\)/);
  assert.match(appSource, /<BackupPanel invoke=\{nativeInvoke\} \/>/);

  const panelSource = await readFile(
    new URL("../src/components/BackupPanel.tsx", import.meta.url),
    "utf8",
  );
  for (const command of [
    "backup_status",
    "backup_list_archives",
    "backup_generate_recovery_phrase",
    "backup_run",
    "backup_restore",
    "backup_restore_select_target",
    "filesystem_backup_select_root",
  ]) {
    const flow = await readFile(new URL("../src/lib/backupFlow.ts", import.meta.url), "utf8");
    assert.ok(flow.includes(command), `backupFlow must still invoke ${command}`);
  }
  assert.match(panelSource, /from "\.\.\/lib\/backupFlow"/);
});
