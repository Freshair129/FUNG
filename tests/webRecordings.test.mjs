// @req FR-101
// @tested tests/webRecordings.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  RECORDER_MIME_CANDIDATES,
  formatBytes,
  formatDurationMs,
  pickRecorderMimeType,
  webRecordingExtension,
  webRecordingFileName,
} from "../src/web/webRecordings.ts";

test("the recorder prefers WebM/Opus and falls back down the list", () => {
  assert.equal(RECORDER_MIME_CANDIDATES[0], "audio/webm;codecs=opus");
  // Chrome / Firefox / Edge.
  assert.equal(pickRecorderMimeType(() => true), "audio/webm;codecs=opus");
  // Safari: no WebM at all, MP4 only.
  assert.equal(
    pickRecorderMimeType((mime) => mime === "audio/mp4"),
    "audio/mp4",
  );
  // A browser that records nothing must be told apart from "use the default".
  assert.equal(pickRecorderMimeType(() => false), null);
});

test("download names carry the local time and the real container", () => {
  assert.equal(webRecordingExtension("audio/webm;codecs=opus"), "webm");
  assert.equal(webRecordingExtension("audio/webm"), "webm");
  assert.equal(webRecordingExtension("audio/mp4"), "m4a");
  assert.equal(webRecordingExtension("audio/ogg;codecs=opus"), "ogg");
  assert.equal(webRecordingExtension("video/x-unknown"), "bin");

  const local = new Date(2026, 8, 16, 12, 3); // 16 Sep 2026 12:03 local time
  assert.equal(
    webRecordingFileName({ createdAt: local.toISOString(), mimeType: "audio/webm;codecs=opus" }),
    "fung-web-2026-09-16-1203.webm",
  );
  assert.equal(
    webRecordingFileName({ createdAt: "not a date", mimeType: "audio/mp4" }),
    "fung-web-unknown.m4a",
  );
});

test("durations and sizes read the way the desktop list shows them", () => {
  assert.equal(formatDurationMs(0), "0:00");
  assert.equal(formatDurationMs(50_350), "0:50");
  assert.equal(formatDurationMs(61_000), "1:01");
  assert.equal(formatDurationMs(3_600_000 + 5_000), "1:00:05");
  assert.equal(formatDurationMs(-5), "0:00");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(20 * 1024), "20 KB");
  assert.equal(formatBytes(1.5 * 1024 * 1024), "1.5 MB");
});

/**
 * The browser-only surface must stay honest about what it is: no placeholder
 * tile pretending a feature is coming, no public link into the desktop shell
 * (which calls Tauri IPC unguarded and crashes in a browser), and sign-in
 * controls only when the build actually carries Supabase configuration —
 * otherwise a button that cannot work. Source pins, because none of this is
 * reachable from a Node test any other way.
 */
test("the web surface links only to things that work in a browser", () => {
  const dashboard = readFileSync("src/web/Dashboard.tsx", "utf8");
  assert.doesNotMatch(dashboard, /เร็วๆ นี้/, "the recorder tile is real now; no 'coming soon' placeholder");
  assert.match(dashboard, /useWebRecorder\(/);
  assert.match(dashboard, /listWebRecordings\(/);
  assert.match(dashboard, /download=\{webRecordingFileName\(recording\)\}/);

  const landing = readFileSync("src/landing/LandingPage.tsx", "utf8");
  assert.doesNotMatch(
    landing,
    /surface=desktop/,
    "the desktop shell is not a browser surface; do not link the public site into it",
  );
  assert.match(landing, /import \{ supabaseConfigured \} from "\.\.\/lib\/bootstrap"/);
  for (const match of landing.matchAll(/onClick=\{handleLogin\}/g)) {
    const preceding = landing.slice(Math.max(0, match.index - 400), match.index);
    assert.match(
      preceding,
      /supabaseConfigured/,
      "every sign-in control on the landing page must be hidden when Supabase is unconfigured",
    );
  }

  const supabaseClient = readFileSync("src/lib/supabase.ts", "utf8");
  assert.doesNotMatch(
    supabaseClient,
    /createClient\(supabaseUrl \?\? ""/,
    "an empty URL throws inside createClient and white-screens the landing page",
  );
  assert.match(supabaseClient, /supabase\.unconfigured\.invalid/);

  const vercel = JSON.parse(readFileSync("vercel.json", "utf8"));
  const sources = vercel.rewrites.map((rewrite) => rewrite.source);
  assert.ok(
    sources.some((source) => source.startsWith("/((?!assets/)")),
    "deep links other than /app and /auth/callback must reach the SPA, not a Vercel 404",
  );

  const css = readFileSync("src/web/Dashboard.css", "utf8");
  assert.match(css, /@media \(max-width: 640px\)/, "phone-width browsers land on this dashboard");
});
