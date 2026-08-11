// @req FR-101
// @tested tests/desktopBootstrap.test.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  hasSupabaseConfig,
  resolveBodySurface,
  resolveRootRoute,
} from "../src/lib/bootstrap.ts";

test("Tauri root boots the desktop app without Supabase configuration", () => {
  assert.equal(
    resolveRootRoute({
      path: "/",
      surface: null,
      isTauriRuntime: true,
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
