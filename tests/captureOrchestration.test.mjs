import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { acquireCaptureBackend, CaptureStartError, nativeRecorderSettled, resumeCaptureClock } from "../src/mobile/captureOrchestration.ts";

test("native recorder starts before WebView media and skips WebView when available", async () => {
  const calls = [];
  const result = await acquireCaptureBackend({
    createSession: async () => { calls.push("session"); return { recordingId: "recording-1" }; },
    startNative: async () => { calls.push("native"); return { available: true }; },
    openWebAudio: async () => { calls.push("web"); return "media"; },
    createRecordingId: () => "fallback-id",
  });

  assert.deepEqual(calls, ["session", "native"]);
  assert.equal(result.backend, "android-native");
  assert.equal(result.media, null);
});

test("WebView media is used only when the native recorder is unavailable", async () => {
  const calls = [];
  const result = await acquireCaptureBackend({
    createSession: async () => { calls.push("session"); return null; },
    startNative: async () => { calls.push("native"); return { available: false }; },
    openWebAudio: async () => { calls.push("web"); return "media"; },
    createRecordingId: () => "fallback-id",
  });

  assert.deepEqual(calls, ["session", "native", "web"]);
  assert.equal(result.backend, "web");
  assert.equal(result.media, "media");
});

test("capture failures retain the stage that failed", async () => {
  await assert.rejects(
    acquireCaptureBackend({
      createSession: async () => ({ recordingId: "recording-1" }),
      startNative: async () => { throw new Error("microphone permission is required"); },
      openWebAudio: async () => "media",
      createRecordingId: () => "fallback-id",
    }),
    (error) => error instanceof CaptureStartError
      && error.stage === "native-start"
      && error.detail.includes("permission"),
  );
});

test("resume clock excludes time spent paused", () => {
  const startedAt = 1_000;
  const pausedAt = 16_000;
  const resumedAt = 33_000;
  const adjustedStart = resumeCaptureClock(startedAt, pausedAt, resumedAt);

  assert.equal(adjustedStart, 18_000);
  assert.equal(resumedAt - adjustedStart, 15_000);
});

test("a stop is settled on the plugin's real terminal state, and the two sources agree", () => {
  // Regression: the shell waited for "completed", which the Android plugin
  // never emits (its terminal state is "stopped"), so every native stop
  // timed out, finishCapture never ran, and recordings stayed "กำลังบันทึก".
  assert.equal(nativeRecorderSettled("stopped"), true);
  assert.equal(nativeRecorderSettled("completed"), true);
  for (const live of ["recording", "paused", "idle", "unavailable", ""]) {
    assert.equal(nativeRecorderSettled(live), false, live);
  }

  const kotlin = readFileSync("src-tauri/mobile/android/dev/fung/local/recorder/RecorderPlugin.kt", "utf8");
  const stopBody = kotlin.slice(kotlin.indexOf("fun stop(invoke: Invoke)"));
  const terminal = /state = "([a-z_]+)"/.exec(stopBody)?.[1];
  assert.ok(terminal, "RecorderPlugin.stop() must set a terminal state");
  assert.equal(nativeRecorderSettled(terminal), true, `plugin stop() ends in "${terminal}" but the shell would keep waiting`);

  const shell = readFileSync("src/mobile/MobileApp.tsx", "utf8");
  assert.match(shell, /nativeRecorderSettled\(settled\.state\)/, "the shell must judge a stop with the shared predicate");
  assert.doesNotMatch(shell, /settled\.state !== "completed"/, "no literal state string may bypass the predicate");
});
