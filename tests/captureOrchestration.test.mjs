import assert from "node:assert/strict";
import test from "node:test";
import { acquireCaptureBackend, CaptureStartError, resumeCaptureClock } from "../src/mobile/captureOrchestration.ts";

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
