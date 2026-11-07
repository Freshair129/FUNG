import assert from "node:assert/strict";
import test from "node:test";

const calls = [];
const replies = new Map();
const adoptionOffer = (state, generation = 1, token = "native-adoption-token") => ({
  ...state,
  adoption: { generation, token },
});
globalThis.window = { __TAURI_INTERNALS__: { invoke: async (command, args = {}) => {
  calls.push({ command, args });
  const reply = replies.get(command);
  if (typeof reply === "function") return reply(args);
  if (reply instanceof Error) throw reply;
  if (command === "desktop_playback_close" && reply === undefined) return { closed: true };
  return reply;
} } };
const api = await import("../src/tauri.ts");

test("source catalog and exact source open preserve the reviewed native wire", async () => {
  assert.equal(typeof api.listPlaybackSources, "function");
  assert.equal(typeof api.openSourcePlayback, "function");
  const key = { projectId: "project-a", recordingId: "recording-a" };
  const sources = [{ sourceId: "source-a", captureEpoch: 7, label: "Meeting", kind: "app", sampleRate: 48000, durationFrames: 1 }];
  replies.set("desktop_playback_sources", sources);
  assert.deepEqual(await api.listPlaybackSources(key.projectId, key.recordingId), sources);
  assert.deepEqual(calls.at(-1), { command: "desktop_playback_sources", args: key });
  const state = { handle: "opaque", ...key, channel: "source", source: { sourceId: "source-a", captureEpoch: 7 }, state: "paused", positionMs: 0, durationMs: 1, streamEpoch: 0, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
  replies.set("desktop_playback_open_source", adoptionOffer(state, 3, "source-adoption"));
  replies.set("desktop_playback_adopt", { adopted: true });
  assert.deepEqual(await api.openSourcePlayback(key.projectId, key.recordingId, "source-a", 7), state);
  assert.deepEqual(calls.slice(-2), [
    { command: "desktop_playback_open_source", args: { ...key, sourceId: "source-a", captureEpoch: 7 } },
    { command: "desktop_playback_adopt", args: { handle: "opaque", lease: { generation: 3, token: "source-adoption" } } },
  ]);
});

test("legacy open validates and acknowledges the exact native adoption offer before returning", async () => {
  calls.length = 0;
  const state = { handle: "adopted-h", projectId: "p", recordingId: "r", channel: "mic", state: "paused", positionMs: 0, durationMs: 1, streamEpoch: 0, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
  const offer = adoptionOffer(state, 13, "native-adoption-token");
  replies.set("desktop_playback_open", offer);
  replies.set("desktop_playback_adopt", { adopted: true });

  const opened = await api.openPlayback("p", "r", "mic");

  assert.deepEqual(opened, state);
  assert.deepEqual(calls, [
    { command: "desktop_playback_open", args: { projectId: "p", recordingId: "r", channel: "mic" } },
    { command: "desktop_playback_adopt", args: { handle: "adopted-h", lease: { generation: 13, token: "native-adoption-token" } } },
  ]);
});

test("heartbeat renews only the acknowledged handle with its saved native generation", async () => {
  calls.length = 0;
  const state = { handle: "heartbeat-h", projectId: "p", recordingId: "r", channel: "mic", state: "paused", positionMs: 0, durationMs: 1, streamEpoch: 0, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
  replies.set("desktop_playback_open", adoptionOffer(state, 21, "adoption-21"));
  replies.set("desktop_playback_adopt", { adopted: true });

  await api.openPlayback("p", "r", "mic");
  await api.heartbeatPlaybackSessions();

  assert.deepEqual(calls.at(-1), {
    command: "desktop_playback_heartbeat",
    args: { handle: "heartbeat-h", lease: { generation: 21, token: "adoption-21" } },
  });
});

test("open without a native adoption offer does not expose the handle", async () => {
  calls.length = 0;
  replies.set("desktop_playback_open", { handle: "unadopted-h", projectId: "p", recordingId: "r", channel: "mic", state: "paused", positionMs: 0, durationMs: 1, streamEpoch: 0, degraded: false, missingRanges: [], outputLatencyMs: null, error: null });

  await assert.rejects(
    api.openPlayback("p", "r", "mic"),
    /PLAYBACK_ADOPTION_OFFER_INVALID/,
  );
  await api.heartbeatPlaybackSessions();
  await assert.rejects(api.closePlayback("unadopted-h"), /INVALID_PLAYBACK_SOURCE_METADATA/);
  assert.equal(calls.some(({ command, args }) => command === "desktop_playback_heartbeat" && args.handle === "unadopted-h"), false);
  assert.equal(calls.some(({ command, args }) => command === "desktop_playback_close" && args.handle === "unadopted-h"), false);
});

test("source IPC failure remains observable without a legacy fallback", async () => {
  calls.length = 0;
  const error = new Error("PLAYBACK_SOURCE_REVOKED");
  replies.set("desktop_playback_open_source", error);
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 4), /PLAYBACK_SOURCE_REVOKED/);
  assert.deepEqual(calls.map(({ command }) => command), ["desktop_playback_open_source"]);
});

test("source wire rejects unsafe or malformed identity and frame counts", async () => {
  calls.length = 0;
  await assert.rejects(api.openSourcePlayback("p", "r", "s", Number.MAX_SAFE_INTEGER + 1));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", -1));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 0));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 0.5));
  assert.equal(calls.length, 0);
  replies.set("desktop_playback_sources", [{ sourceId: "s", captureEpoch: 1, label: "x", kind: "app", sampleRate: 48000, durationFrames: Number.MAX_SAFE_INTEGER + 1 }]);
  await assert.rejects(api.listPlaybackSources("p", "r"));
});

test("source open refuses mismatched and missing native source identity", async () => {
  const base = { handle: "opaque", projectId: "p", recordingId: "r", channel: "source", state: "paused", positionMs: 0, durationMs: 1, streamEpoch: 0, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
  replies.set("desktop_playback_open_source", adoptionOffer(base));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 1));
  replies.set("desktop_playback_open_source", adoptionOffer({ ...base, source: { sourceId: "other", captureEpoch: 1 } }));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 1));
  replies.set("desktop_playback_open_source", adoptionOffer({ ...base, source: { sourceId: "s", captureEpoch: Number.MAX_SAFE_INTEGER + 1 } }));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 1));
});

test("legacy open rejects source identity instead of accepting an ambiguous channel", async () => {
  calls.length = 0;
  replies.set("desktop_playback_open", { handle: "legacy-h", projectId: "p", recordingId: "r", channel: "mic", source: { sourceId: "s", captureEpoch: 1 } });
  await assert.rejects(api.openPlayback("p", "r", "mic"), /PLAYBACK_ADOPTION_OFFER_INVALID/);
  assert.deepEqual(calls.map(({ command }) => command), ["desktop_playback_open"]);
});

test("status and control reject changed source identity or handle", async () => {
  const good = { handle: "h-identity", projectId: "p", recordingId: "r", channel: "source", source: { sourceId: "s", captureEpoch: 4 }, state: "paused", positionMs: 0, durationMs: 1, streamEpoch: 0, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
  replies.set("desktop_playback_open_source", adoptionOffer(good));
  replies.set("desktop_playback_adopt", { adopted: true });
  await api.openSourcePlayback("p", "r", "s", 4);
  replies.set("desktop_playback_status", { ...good, source: { sourceId: "other", captureEpoch: 4 } });
  await assert.rejects(api.getPlayback("h-identity"), /INVALID_PLAYBACK_SOURCE_METADATA/);
  assert.deepEqual(calls.find(({ command }) => command === "desktop_playback_status"), {
    command: "desktop_playback_status",
    args: { handle: "h-identity", lease: { generation: 1, token: "native-adoption-token" } },
  });
  replies.set("desktop_playback_control", { ...good, handle: "different" });
  await assert.rejects(api.controlPlayback("h-identity", 0, "play"), /INVALID_PLAYBACK_SOURCE_METADATA/);
  assert.deepEqual(calls.find(({ command }) => command === "desktop_playback_control"), {
    command: "desktop_playback_control",
    args: { handle: "h-identity", lease: { generation: 1, token: "native-adoption-token" }, expectedEpoch: 0, action: "play" },
  });
});

test("failed adoption removes identity and cannot be retried as a close", async () => {
  calls.length = 0;
  const state = { handle: "needs-close", projectId: "p", recordingId: "r", channel: "source", source: { sourceId: "s", captureEpoch: 1 } };
  replies.set("desktop_playback_open_source", adoptionOffer(state, 4, "needs-close-token"));
  replies.set("desktop_playback_adopt", new Error("ack lost"));
  await assert.rejects(api.openSourcePlayback("p", "r", "s", 1), (error) =>
    error.code === "PLAYBACK_CLOSE_UNCONFIRMED" && error.handle === "needs-close" &&
    error.retryable === false && error.closeRetryable === false);
  await api.heartbeatPlaybackSessions();
  await assert.rejects(api.closePlayback("needs-close"), /INVALID_PLAYBACK_SOURCE_METADATA/);
  assert.equal(calls.some(({ command, args }) =>
    (command === "desktop_playback_heartbeat" || command === "desktop_playback_close") && args.handle === "needs-close"), false);
});

test("heartbeat continues after a non-terminal failure and retains the identity", async () => {
  const handles = ["batch-first", "batch-second", "batch-third"];
  replies.set("desktop_playback_adopt", { adopted: true });
  for (const handle of handles) {
    replies.set("desktop_playback_open", adoptionOffer({ handle, projectId: "p", recordingId: "r", channel: "mic" }));
    await api.openPlayback("p", "r", "mic");
  }
  replies.set("desktop_playback_heartbeat", ({ handle }) => {
    if (handle === handles[0]) throw new Error("temporary failure");
  });
  calls.length = 0;
  await api.heartbeatPlaybackSessions();
  assert.deepEqual(calls.filter(({ command, args }) => command === "desktop_playback_heartbeat" && handles.includes(args.handle)).map(({ args }) => args.handle), handles);
  calls.length = 0;
  await api.heartbeatPlaybackSessions();
  assert.ok(calls.some(({ command, args }) => command === "desktop_playback_heartbeat" && args.handle === handles[0]));
  replies.delete("desktop_playback_heartbeat");
  for (const handle of handles) await api.closePlayback(handle);
});

test("terminal heartbeat removes identity and later ticks skip it", async () => {
  const handle = "expired-heartbeat";
  replies.set("desktop_playback_open", adoptionOffer({ handle, projectId: "p", recordingId: "r", channel: "mic" }));
  replies.set("desktop_playback_adopt", { adopted: true });
  await api.openPlayback("p", "r", "mic");
  replies.set("desktop_playback_heartbeat", (args) => {
    if (args.handle === handle) throw Object.assign(new Error("expired"), { code: "PLAYBACK_LEASE_EXPIRED" });
  });
  calls.length = 0;
  await api.heartbeatPlaybackSessions();
  assert.ok(calls.some((call) => call.command === "desktop_playback_heartbeat" && call.args.handle === handle));
  calls.length = 0;
  await api.heartbeatPlaybackSessions();
  assert.equal(calls.some((call) => call.command === "desktop_playback_heartbeat" && call.args.handle === handle), false);
  await assert.rejects(api.closePlayback(handle), /INVALID_PLAYBACK_SOURCE_METADATA/);
  replies.delete("desktop_playback_heartbeat");
});

test("legacy source null is accepted for backwards-compatible serde Option", async () => {
  replies.set("desktop_playback_open", adoptionOffer({ handle: "legacy-null", projectId: "p", recordingId: "r", channel: "mic", source: null }));
  replies.set("desktop_playback_adopt", { adopted: true });
  const state = await api.openPlayback("p", "r", "mic");
  assert.equal(state.handle, "legacy-null");
});

test("unknown playback handles cannot issue control or status IPC", async () => {
  calls.length = 0;
  await assert.rejects(api.controlPlayback("unknown-h", 0, "play"), /INVALID_PLAYBACK_SOURCE_METADATA/);
  await assert.rejects(api.getPlayback("unknown-h"), /INVALID_PLAYBACK_SOURCE_METADATA/);
  assert.deepEqual(calls, []);
});

test("source catalog rejects duplicate identities and unsafe source data", async () => {
  const source = { sourceId: "s", captureEpoch: 2, label: "x", kind: "app", sampleRate: 48000, durationFrames: 12 };
  replies.set("desktop_playback_sources", [source, { ...source }]);
  await assert.rejects(api.listPlaybackSources("p", "r"), /INVALID_PLAYBACK_SOURCE_METADATA/);
});
