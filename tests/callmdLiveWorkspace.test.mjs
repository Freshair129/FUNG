// @req FR-102, FR-103, FR-104, FR-105, FR-115, NFR-104, NFR-106
// @tested tests/callmdLiveWorkspace.test.mjs
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";
import vm from "node:vm";

const livePanelPath = fileURLToPath(new URL("../src/components/LiveMeetingPanel.tsx", import.meta.url));
const liveWorkspacePath = fileURLToPath(new URL("../src/components/desktop/LiveWorkspace.tsx", import.meta.url));
const liveWorkspaceCssPath = fileURLToPath(new URL("../src/components/desktop/LiveWorkspace.css", import.meta.url));
const contractsPath = fileURLToPath(new URL("../src/components/desktop/contracts.ts", import.meta.url));
const root = path.resolve(path.dirname(livePanelPath), "../..");
const fixtureMode = process.argv.includes("--fixture");

function transpile(source, filename) {
  return ts.transpileModule(source, {
    fileName: filename,
    compilerOptions: {
      target: ts.ScriptTarget.ES2020,
      module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX,
      esModuleInterop: true,
    },
  }).outputText;
}

function runCommonJs(source, filename, modules) {
  const module = { exports: {} };
  const context = {
    module,
    exports: module.exports,
    console,
    Date,
    JSON,
    Math,
    Map,
    Promise,
    Set,
    Array,
    Object,
    RegExp,
    String,
    Number,
    Boolean,
    Error,
    setTimeout,
    clearTimeout,
    setImmediate,
    crypto: globalThis.crypto,
    window: {
      setTimeout,
      clearTimeout,
      addEventListener() {},
      removeEventListener() {},
    },
    require(specifier) {
      if (specifier in modules) return modules[specifier];
      if (specifier.endsWith(".css")) return {};
      throw new Error(`Unexpected production-module dependency: ${specifier}`);
    },
  };
  context.globalThis = context;
  vm.runInNewContext(source, context, { filename });
  return module.exports;
}

const [panelSource, workspaceSource, liveWorkspaceCss, contractsSource] = await Promise.all([
  readFile(livePanelPath, "utf8"),
  readFile(liveWorkspacePath, "utf8"),
  readFile(liveWorkspaceCssPath, "utf8"),
  readFile(contractsPath, "utf8"),
]);

const contracts = runCommonJs(
  transpile(contractsSource, contractsPath),
  contractsPath,
  {
    "../../tauri": {},
    "../../lib/meetingSummaries": {},
  },
);

const reactStub = {
  useCallback: (callback) => callback,
  useEffect: () => undefined,
  useMemo: (factory) => factory(),
  useRef: (value) => ({ current: value }),
  useState: (value) => [value, () => undefined],
};

const panel = runCommonJs(
  transpile(panelSource, livePanelPath),
  livePanelPath,
  {
    react: reactStub,
    "react/jsx-runtime": { Fragment: "fragment", jsx: () => null, jsxs: () => null },
    "@tauri-apps/api/event": { listen: async () => () => undefined },
    "../tauri": {
      askRecording: async () => undefined,
      generateMeetingSummary: async () => undefined,
      listTranscriptSegments: async () => ({ segments: [], capped: false, cap: 200, cappedRecordingIds: [] }),
      liveMeetingStart: async () => undefined,
      liveMeetingStatus: async () => ({ active: false, stopping: false, projectId: null, recordingId: null, elapsedMs: null }),
      liveMeetingStop: async () => "stopped",
      meetingSummaries: async () => ({ rows: [], otherRecordings: 0, unattributable: 0, attributionComplete: true }),
    },
    "../lib/meetingSummaries": {},
    "./desktop/contracts": contracts,
    "./desktop/LiveWorkspace": { LiveWorkspace: () => null },
  },
);

const workspace = runCommonJs(
  transpile(workspaceSource, liveWorkspacePath),
  liveWorkspacePath,
  {
    react: reactStub,
    "react/jsx-runtime": { Fragment: "fragment", jsx: () => null, jsxs: () => null },
    "../ExternalMeetingToolsPanel": { ExternalMeetingToolsPanel: () => null },
    "../../tauri": {},
    "../../lib/meetingSummaries": {},
    "./contracts": contracts,
  },
);

const {
  createLiveEventLifecycle,
  createLiveControllerAdapter,
  mergeLiveSegments,
  runStartWithCloseAck,
  settleLiveRequest,
  waitForCaptureInactive,
} = panel;

assert.equal(typeof createLiveEventLifecycle, "function");
assert.equal(typeof mergeLiveSegments, "function");
assert.equal(typeof runStartWithCloseAck, "function");
assert.equal(typeof settleLiveRequest, "function");
assert.equal(typeof waitForCaptureInactive, "function");
assert.equal(typeof createLiveControllerAdapter, "function");
assert.equal(typeof workspace.getLiveSummaryPresentation, "function");

const keyA = { projectId: "project-a", recordingId: "recording-a" };
const keyB = { projectId: "project-b", recordingId: "recording-b" };

function statusFor(key, overrides = {}) {
  return {
    active: true,
    stopping: false,
    projectId: key.projectId,
    recordingId: key.recordingId,
    elapsedMs: 1200,
    ...overrides,
  };
}

function segmentFor(recordingId, segmentId, startMs = 1000) {
  return {
    recordingId,
    segmentId,
    channel: "mic",
    speaker: "ผู้พูด",
    startMs,
    endMs: startMs + 400,
    text: `ข้อความ ${segmentId}`,
    confidence: 0.9,
  };
}

function transcriptFor(key) {
  return {
    segments: [{
      id: "persisted-1",
      projectId: key.projectId,
      recordingId: key.recordingId,
      speakerName: "ผู้พูด",
      startMs: 100,
      endMs: 500,
      text: "ข้อความที่บันทึกไว้",
      confidence: 0.95,
    }],
    capped: false,
    cap: 200,
    cappedRecordingIds: [],
  };
}

function createSink() {
  const events = {
    authoritative: [],
    status: [],
    segments: [],
    topics: [],
    summaries: [],
    transcripts: [],
    incomplete: 0,
    errors: [],
  };
  return {
    events,
    sink: {
      onAuthoritativeStatus: (value) => events.authoritative.push(value),
      onStatusEvent: (value) => events.status.push(value),
      onSegment: (value) => events.segments.push(value),
      onTopic: (value) => events.topics.push(value),
      onSummary: (value) => events.summaries.push(value),
      onTranscript: (selection, view) => events.transcripts.push({ selection, view }),
      onIncomplete: () => { events.incomplete += 1; },
      onError: (value) => events.errors.push(value),
    },
  };
}

function flush() {
  return new Promise((resolve) => setImmediate(resolve));
}

async function flushAsyncWork() {
  await Promise.resolve();
  await flush();
  await Promise.resolve();
  await flush();
}

const FIXTURE_DIRECTORY_PREFIX = "fung-callmd-live-fixture-";

function verifyFixtureCleanupTarget(fixtureRoot) {
  let canonicalTempRoot;
  let canonicalFixtureRoot;
  try {
    canonicalTempRoot = realpathSync.native(path.resolve(os.tmpdir()));
    canonicalFixtureRoot = realpathSync.native(path.resolve(fixtureRoot));
  } catch (error) {
    throw new Error(
      `Refusing recursive fixture cleanup: canonical target verification failed for ${fixtureRoot}`,
      { cause: error },
    );
  }

  const relative = path.relative(canonicalTempRoot, canonicalFixtureRoot);
  const targetName = path.basename(canonicalFixtureRoot);
  const isImmediateChild =
    relative !== "" &&
    !relative.includes("\\") &&
    !relative.includes("/") &&
    path.dirname(canonicalFixtureRoot).toLowerCase() === canonicalTempRoot.toLowerCase();
  const hasTaskPrefix = targetName.startsWith(FIXTURE_DIRECTORY_PREFIX);
  console.log(`FIXTURE_CLEANUP_GENERATED_ROOT=${fixtureRoot}`);
  console.log(`FIXTURE_CLEANUP_CANONICAL_ROOT=${canonicalFixtureRoot}`);

  if (!isImmediateChild || !hasTaskPrefix) {
    throw new Error(
      `Refusing recursive fixture cleanup: ${canonicalFixtureRoot} is not an immediate ${FIXTURE_DIRECTORY_PREFIX} child of ${canonicalTempRoot}`,
    );
  }

  return canonicalFixtureRoot;
}

async function cleanupFixtureRoot(fixtureRoot) {
  const verifiedRoot = verifyFixtureCleanupTarget(fixtureRoot);
  await rm(verifiedRoot, { recursive: true, force: true });
}

if (!fixtureMode) {
test("subscribes to every live event before status and persisted transcript", async () => {
  const order = [];
  const { events, sink } = createSink();
  const matching = segmentFor(keyA.recordingId, "live-1");
  const lifecycle = createLiveEventLifecycle(
    {
      listen: async (name, handler) => {
        order.push(`listen:${name}`);
        if (name === "live-segment") handler(matching);
        return () => undefined;
      },
      readStatus: async () => {
        order.push("status");
        return statusFor(keyA);
      },
      readTranscript: async () => {
        order.push("transcript");
        return transcriptFor(keyA);
      },
    },
    sink,
  );

  await lifecycle.start();

  assert.deepEqual(order.slice(0, 4), [
    "listen:live-status",
    "listen:live-segment",
    "listen:live-topic",
    "listen:live-summary",
  ]);
  assert.equal(order[4], "status");
  assert.equal(order[5], "transcript");
  assert.deepEqual(events.segments, [matching]);
  assert.equal(events.transcripts.length, 1);
});

test("late subscription resolutions are unlistened immediately after disposal", async () => {
  const pending = [];
  const unlistenCalls = [];
  const { sink } = createSink();
  const lifecycle = createLiveEventLifecycle(
    {
      listen: (name, handler) => new Promise((resolve) => {
        pending.push({ name, handler, resolve });
      }),
      readStatus: async () => {
        throw new Error("status must not run after disposal");
      },
      readTranscript: async () => {
        throw new Error("transcript must not run after disposal");
      },
    },
    sink,
  );

  const bootstrap = lifecycle.start();
  lifecycle.dispose();
  for (const subscription of pending) {
    subscription.resolve(() => { unlistenCalls.push(subscription.name); });
  }
  await bootstrap;

  assert.deepEqual(unlistenCalls.sort(), [
    "live-segment",
    "live-status",
    "live-summary",
    "live-topic",
  ]);
});

test("owner-safe controller handoff publishes snapshots and rejects after disposal", async () => {
  const initialStatus = statusFor(keyA);
  const snapshots = [];
  let nativeCalls = 0;
  const adapter = createLiveControllerAdapter(
    {
      liveStatus: { status: "ready", identity: null, data: initialStatus, error: null },
      phase: "listening",
      selection: keyA,
    },
    async () => {
      nativeCalls += 1;
      return statusFor(keyA, { active: false, stopping: false });
    },
  );

  const unsubscribe = adapter.controller.subscribe((snapshot) => snapshots.push(snapshot));
  assert.equal(snapshots.length, 1);
  adapter.publish({
    liveStatus: { status: "ready", identity: null, data: initialStatus, error: null },
    phase: "stopping",
    selection: keyA,
  });
  assert.equal(snapshots.length, 2);
  assert.equal(adapter.controller.getSnapshot().phase, "stopping");

  adapter.dispose();
  adapter.publish({
    liveStatus: { status: "ready", identity: null, data: initialStatus, error: null },
    phase: "stopped",
    selection: keyA,
  });
  assert.equal(snapshots.length, 2);
  await assert.rejects(
    () => adapter.controller.stopAndLeave(),
    (error) => error.code === "LEGACY_COMMAND_FAILED",
  );
  assert.equal(nativeCalls, 0);
  unsubscribe();
});

test("mismatched events are quarantined and only cause a bounded status refresh", async () => {
  const handlers = new Map();
  let statusReads = 0;
  const { events, sink } = createSink();
  const lifecycle = createLiveEventLifecycle(
    {
      listen: async (name, handler) => {
        handlers.set(name, handler);
        return () => undefined;
      },
      readStatus: async () => {
        statusReads += 1;
        return statusFor(keyA);
      },
      readTranscript: async () => transcriptFor(keyA),
    },
    sink,
  );
  await lifecycle.start();

  handlers.get("live-segment")?.(segmentFor(keyB.recordingId, "wrong"));
  handlers.get("live-topic")?.({
    recordingId: keyB.recordingId,
    topic: "ห้ามรับมาใช้",
    openPoints: [],
    actionItems: [],
    model: "test",
    windowStartMs: 0,
    windowEndMs: 1000,
  });
  handlers.get("live-summary")?.({ recordingId: keyB.recordingId, state: "ready", detail: null, exportPath: null });
  await flushAsyncWork();

  assert.equal(statusReads, 2);
  assert.equal(events.segments.length, 0);
  assert.equal(events.topics.length, 0);
  assert.equal(events.summaries.length, 0);

  const matching = segmentFor(keyA.recordingId, "right");
  handlers.get("live-segment")?.(matching);
  assert.deepEqual(events.segments, [matching]);
});

test("lifecycle transcript success and error guards survive A-B-A and repeated same-pair refresh", async () => {
  const pending = [];
  const statuses = [statusFor(keyA), statusFor(keyB), statusFor(keyA), statusFor(keyA), statusFor(keyA)];
  let statusIndex = 0;
  const { events, sink } = createSink();
  const lifecycle = createLiveEventLifecycle(
    {
      listen: async () => () => undefined,
      readStatus: async () => statuses[statusIndex++] ?? statusFor(keyA),
      readTranscript: async (selection) => new Promise((resolve, reject) => {
        pending.push({ selection, resolve, reject });
      }),
    },
    sink,
  );

  const bootstrap = lifecycle.start();
  await flushAsyncWork();
  assert.equal(pending.length, 1);
  pending[0].resolve(transcriptFor(keyA));
  await bootstrap;

  const refreshB = lifecycle.refreshStatus(true);
  await flushAsyncWork();
  assert.equal(pending.length, 2);
  pending[1].resolve(transcriptFor(keyB));
  await refreshB;

  const staleSuccess = lifecycle.refreshStatus(true);
  await flushAsyncWork();
  assert.equal(pending.length, 3);
  lifecycle.setKnownIdentity(keyB);
  lifecycle.setKnownIdentity(keyA);
  pending[2].resolve(transcriptFor(keyA));
  await staleSuccess;

  const staleError = lifecycle.refreshStatus(true);
  await flushAsyncWork();
  assert.equal(pending.length, 4);
  lifecycle.setKnownIdentity(keyB);
  lifecycle.setKnownIdentity(keyA);
  pending[3].reject({ code: "STORAGE_READ_FAILED", message: "old transcript", retryable: true });
  await staleError;

  const currentRefresh = lifecycle.refreshStatus(true);
  await flushAsyncWork();
  assert.equal(pending.length, 5);
  pending[4].resolve(transcriptFor(keyA));
  await currentRefresh;

  assert.deepEqual(
    events.transcripts.map((item) => item.selection.projectId + "/" + item.selection.recordingId),
    ["project-a/recording-a", "project-b/recording-b", "project-a/recording-a"],
  );
  assert.deepEqual(events.errors, []);
});

test("bootstrap overflow is explicit and segment merge deduplicates by recording pair", async () => {
  const { events, sink } = createSink();
  const lifecycle = createLiveEventLifecycle(
    {
      listen: async (name, handler) => {
        if (name === "live-segment") {
          for (let index = 0; index < 201; index += 1) {
            handler(segmentFor(keyA.recordingId, `bootstrap-${index}`, index));
          }
        }
        return () => undefined;
      },
      readStatus: async () => statusFor(keyA),
      readTranscript: async () => ({ segments: [], capped: false, cap: 200, cappedRecordingIds: [] }),
    },
    sink,
  );
  await lifecycle.start();

  assert.equal(events.incomplete, 1);
  assert.equal(events.segments.length, 200);

  const merged = mergeLiveSegments(
    [segmentFor(keyA.recordingId, "same", 100), segmentFor(keyB.recordingId, "same", 200)],
    [{ ...segmentFor(keyA.recordingId, "same", 100), text: "อัปเดตแล้ว" }],
  );
  assert.equal(merged.length, 2);
  assert.equal(merged.find((item) => item.recordingId === keyA.recordingId)?.text, "อัปเดตแล้ว");
});

test("late ask and summary outcomes cannot settle a different full identity", () => {
  const askIdentity = { ...keyA, selectionEpoch: 2, requestId: "ask-current" };
  const staleAskIdentity = { ...keyA, selectionEpoch: 1, requestId: "ask-old" };
  const askState = { status: "loading", identity: askIdentity, data: null, error: null };
  const answer = {
    ...keyA,
    requestId: "ask-current",
    scope: "recording",
    status: "answered",
    answer: "คำตอบจากหลักฐาน",
    model: "local-test",
    sources: [],
    graphPolicy: "excluded",
    liveTailPolicy: "excluded",
  };
  const settledAsk = settleLiveRequest(askState, askIdentity, { status: "fulfilled", data: answer });
  assert.equal(settledAsk.status, "ready");
  assert.equal(settledAsk.data, answer);
  assert.strictEqual(
    settleLiveRequest(settledAsk, staleAskIdentity, {
      status: "rejected",
      error: { code: "PROVIDER_FAILED", message: "old ask", retryable: true },
    }),
    settledAsk,
  );

  const summaryIdentity = { ...keyA, selectionEpoch: 2, requestId: "summary-current" };
  const staleSummaryIdentity = { ...keyB, selectionEpoch: 1, requestId: "summary-old" };
  const summaryState = { status: "loading", identity: summaryIdentity, data: null, error: null };
  const summary = { rows: [], otherRecordings: 0, unattributable: 0, attributionComplete: true };
  const settledSummary = settleLiveRequest(summaryState, summaryIdentity, {
    status: "fulfilled",
    data: summary,
  });
  assert.equal(settledSummary.status, "ready");
  assert.equal(settledSummary.data, summary);
  assert.strictEqual(
    settleLiveRequest(settledSummary, staleSummaryIdentity, {
      status: "rejected",
      error: { code: "STORAGE_READ_FAILED", message: "old summary", retryable: true },
    }),
    settledSummary,
  );
});

test("empty current summary keeps foreign, unattributable, and incomplete disclosures", () => {
  const presentation = workspace.getLiveSummaryPresentation({
    rows: [{
      id: "foreign-summary",
      kind: "whole_story",
      content: "ข้อมูลของรายการอื่น",
      evidenceCount: 1,
      createdAt: "2026-09-17T00:00:00Z",
      recordingId: keyB.recordingId,
      superseded: false,
    }],
    otherRecordings: 1,
    unattributable: 2,
    attributionComplete: true,
  }, keyA.recordingId);

  assert.equal(presentation.currentRows.length, 0);
  assert.deepEqual(
    [...presentation.disclosures.map((item) => item.key)],
    ["other-recordings", "unattributable"],
  );

  const incomplete = workspace.getLiveSummaryPresentation({
    rows: [],
    otherRecordings: 1,
    unattributable: 3,
    attributionComplete: false,
  }, keyA.recordingId);
  assert.deepEqual(
    [...incomplete.disclosures.map((item) => item.key)],
    ["other-recordings", "incomplete-attribution"],
  );
});

test("start fails closed before native start when review close is false or errors", async () => {
  let starts = 0;
  await assert.rejects(
    () => runStartWithCloseAck(
      async () => ({ closed: false }),
      async () => { starts += 1; return "started"; },
    ),
    (error) => error.code === "PLAYBACK_BUSY",
  );
  assert.equal(starts, 0);

  await assert.rejects(
    () => runStartWithCloseAck(
      async () => { throw new Error("legacy close failure"); },
      async () => { starts += 1; return "started"; },
    ),
    (error) => error.code === "LEGACY_COMMAND_FAILED",
  );
  assert.equal(starts, 0);

  assert.equal(
    await runStartWithCloseAck(async () => ({ closed: true }), async () => {
      starts += 1;
      return "started";
    }),
    "started",
  );
  assert.equal(starts, 1);
});

test("stop-and-leave waits for active=false and stopping=false", async () => {
  const statuses = [
    statusFor(keyA, { active: true, stopping: true }),
    statusFor(keyA, { active: true, stopping: false }),
    statusFor(keyA, { active: false, stopping: true }),
    statusFor(keyA, { active: false, stopping: false }),
  ];
  let reads = 0;
  const result = await waitForCaptureInactive(
    async () => statuses[reads++],
    { pollMs: 0, sleep: async () => undefined },
  );
  assert.equal(reads, 4);
  assert.deepEqual(result, statuses[3]);
});

test("visibility changes do not stop the lifecycle or remove listeners", async () => {
  const { events, sink } = createSink();
  const unlistenCalls = [];
  const handlers = new Map();
  const lifecycle = createLiveEventLifecycle(
    {
      listen: async (name, handler) => {
        handlers.set(name, handler);
        return () => { unlistenCalls.push(name); };
      },
      readStatus: async () => statusFor(keyA),
      readTranscript: async () => ({ segments: [], capped: false, cap: 200, cappedRecordingIds: [] }),
    },
    sink,
  );

  await lifecycle.start();
  lifecycle.setVisible(false);
  const matching = segmentFor(keyA.recordingId, "while-hidden");
  handlers.get("live-segment")?.(matching);
  assert.deepEqual(events.segments, [matching]);
  assert.deepEqual(unlistenCalls, []);

  lifecycle.dispose();
  assert.equal(unlistenCalls.length, 4);
});

test("live form labels preserve light color and use workspace ink in dark mode", () => {
  assert.match(
    liveWorkspaceCss,
    /\.live-workspace__check,\s*\.live-workspace__field\s*\{[^}]*color:\s*#303833;/s,
  );

  const darkLabelRule = liveWorkspaceCss.match(
    /\.theme-dark\s+\.live-workspace__check,\s*\.theme-dark\s+\.live-workspace__field\s*\{([^}]*)\}/s,
  );
  assert.ok(darkLabelRule, "dark form-label override must remain present");
  assert.match(darkLabelRule[1], /color:\s*var\(--workspace-ink\);/);
  assert.doesNotMatch(darkLabelRule[1], /display|align-items|gap|width|font-size/);
});
} else {
  const { createServer } = await import("vite");
  const { default: reactPlugin } = await import("@vitejs/plugin-react");
  const fixtureRoot = await mkdtemp(path.join(os.tmpdir(), FIXTURE_DIRECTORY_PREFIX));
  console.log(`FIXTURE_GENERATED_ROOT=${fixtureRoot}`);
  const fixtureEntry = String.raw`
import React, { useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { LiveMeetingPanel } from "@callmd-live";

function wait(milliseconds) {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function Result({ label, pass, detail }) {
  return <li data-result={pass ? "PASS" : "FAIL"}><strong>{pass ? "PASS" : "FAIL"}</strong> {label} — {detail}</li>;
}

function LiveLifecycleFixture() {
  const [visible, setVisible] = useState(false);
  const [theme, setTheme] = useState("light");
  const [mounted, setMounted] = useState(true);
  const [phase, setPhase] = useState("mounting");
  const [results, setResults] = useState([]);
  const [snapshot, setSnapshot] = useState(null);
  const snapshotRef = useRef(null);
  const registrationCountRef = useRef(0);
  const retainedControllerRef = useRef(null);
  const subscriptionRef = useRef(null);
  const scenarioStartedRef = useRef(false);

  const addResult = useCallback((label, pass, detail) => {
    setResults((current) => [...current, { label, pass, detail }]);
  }, []);

  const onControllerChange = useCallback((controller) => {
    subscriptionRef.current?.();
    subscriptionRef.current = null;
    if (!controller) {
      return;
    }
    retainedControllerRef.current = controller;
    registrationCountRef.current += 1;
    subscriptionRef.current = controller.subscribe((nextSnapshot) => {
      snapshotRef.current = nextSnapshot;
      setSnapshot(nextSnapshot);
    });
  }, []);

  useEffect(() => {
    if (scenarioStartedRef.current) return;
    scenarioStartedRef.current = true;
    window.setTimeout(async () => {
      const hiddenOverlay = document.querySelector(".live-overlay");
      const hiddenRect = hiddenOverlay?.getBoundingClientRect();
      const hiddenDisplay = hiddenOverlay ? window.getComputedStyle(hiddenOverlay).display : "missing";
      addResult(
        "hidden owner is removed from layout",
        Boolean(hiddenOverlay?.hidden && hiddenDisplay === "none" && hiddenRect?.width === 0 && hiddenRect?.height === 0),
        "hidden=" + Boolean(hiddenOverlay?.hidden) + " display=" + hiddenDisplay + " rect=" + (hiddenRect?.width ?? -1) + "x" + (hiddenRect?.height ?? -1),
      );

      setVisible(true);
      setPhase("visible");
      await wait(120);
      const visibleOverlay = document.querySelector(".live-overlay");
      const visibleRect = visibleOverlay?.getBoundingClientRect();
      const overlayCount = document.querySelectorAll(".live-overlay").length;
      addResult(
        "visibility toggle keeps one mounted owner",
        Boolean(visibleOverlay && !visibleOverlay.hidden && window.getComputedStyle(visibleOverlay).display !== "none" && visibleRect?.width > 0 && visibleRect?.height > 0 && overlayCount === 1),
        "display=" + (visibleOverlay ? window.getComputedStyle(visibleOverlay).display : "missing") + " rect=" + (visibleRect?.width ?? -1) + "x" + (visibleRect?.height ?? -1) + " owners=" + overlayCount,
      );

      const lightLabels = Array.from(document.querySelectorAll(".live-workspace__check, .live-workspace__field"));
      const lightLabelColors = lightLabels.map((label) => window.getComputedStyle(label).color);
      addResult(
        "light form labels preserve readable leaf color",
        lightLabels.length === 3 && lightLabelColors.every((color) => color === "rgb(48, 56, 51)"),
        "labels=" + lightLabels.length + " colors=" + lightLabelColors.join(","),
      );

      setTheme("dark");
      await wait(120);
      const darkLabels = Array.from(document.querySelectorAll(".live-workspace__check, .live-workspace__field"));
      const darkLabelColors = darkLabels.map((label) => window.getComputedStyle(label).color);
      addResult(
        "dark form labels use readable workspace ink",
        darkLabels.length === 3 && darkLabelColors.every((color) => color === "rgb(244, 241, 234)"),
        "labels=" + darkLabels.length + " colors=" + darkLabelColors.join(","),
      );

      const currentSnapshot = snapshotRef.current;
      const statusEscapedLoading = currentSnapshot?.liveStatus?.status !== "loading" && currentSnapshot?.liveStatus?.data?.active === false && currentSnapshot?.liveStatus?.data?.stopping === false;
      addResult(
        "StrictMode publishes authoritative inactive after replay",
        Boolean(statusEscapedLoading),
        "status=" + (currentSnapshot?.liveStatus?.status ?? "missing") + " active=" + String(currentSnapshot?.liveStatus?.data?.active ?? "missing") + " registrations=" + registrationCountRef.current,
      );
      const unavailableVisible = document.body.textContent?.includes("NATIVE_UNAVAILABLE") === true;
      addResult(
        "native-absent capability remains truthful after bootstrap",
        unavailableVisible && Boolean(statusEscapedLoading),
        "NATIVE_UNAVAILABLE visible=" + unavailableVisible + " statusEscapedLoading=" + Boolean(statusEscapedLoading),
      );

      const retainedController = retainedControllerRef.current;
      setMounted(false);
      setPhase("unmounted");
      await wait(120);
      let rejectedAfterUnmount = false;
      let rejectionCode = "missing";
      try {
        await retainedController?.stopAndLeave();
      } catch (error) {
        rejectionCode = error?.code ?? "unknown";
        rejectedAfterUnmount = rejectionCode === "LEGACY_COMMAND_FAILED";
      }
      addResult(
        "retained controller rejects after genuine unmount",
        rejectedAfterUnmount,
        "code=" + rejectionCode,
      );
      setPhase("done");
    }, 160);
  }, [addResult]);

  return <main className={theme === "dark" ? "theme-dark" : ""} style={{ fontFamily: "system-ui", maxWidth: 960, margin: "32px auto", lineHeight: 1.6 }}>
    <h1>FUNG Live lifecycle fixture</h1>
    <p>Component-only proof: real ReactDOMClient + production LiveMeetingPanel + native-absent browser. No Tauri internals are installed.</p>
    <p data-testid="phase">phase: {phase}</p>
    <p data-testid="published-snapshot">published status={snapshot?.liveStatus?.status ?? "missing"} active={String(snapshot?.liveStatus?.data?.active ?? "missing")} phase={snapshot?.phase ?? "missing"}</p>
    {mounted ? (
      <React.StrictMode>
        <LiveMeetingPanel
          onClose={() => undefined}
          projectId="fixture-project"
          visible={visible}
          onControllerChange={onControllerChange}
        />
      </React.StrictMode>
    ) : null}
    <h2>Live lifecycle results</h2>
    <ul>{results.map((result) => <Result key={result.label} {...result} />)}</ul>
    {phase === "done" ? <p data-testid="fixture-complete">FIXTURE_COMPLETE — read all PASS/FAIL rows above.</p> : null}
  </main>;
}

createRoot(document.getElementById("root")).render(<LiveLifecycleFixture />);
`;
  const fixtureHtml = `<!doctype html><html><head><meta charset="UTF-8"><title>FUNG Live lifecycle fixture</title></head><body><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>`;
  await writeFile(path.join(fixtureRoot, "main.tsx"), fixtureEntry, "utf8");
  await writeFile(path.join(fixtureRoot, "index.html"), fixtureHtml, "utf8");

  const fixtureRequire = createRequire(import.meta.url);
  const reactDirectory = path.dirname(fixtureRequire.resolve("react"));
  const reactDomDirectory = path.dirname(fixtureRequire.resolve("react-dom"));
  const server = await createServer({
    root: fixtureRoot,
    plugins: [reactPlugin()],
    resolve: {
      alias: [
        { find: "@callmd-live", replacement: path.resolve(root, "src/components/LiveMeetingPanel.tsx") },
        { find: /^react$/, replacement: reactDirectory },
        { find: new RegExp("^react/"), replacement: reactDirectory + "/" },
        { find: /^react-dom$/, replacement: reactDomDirectory },
        { find: new RegExp("^react-dom/"), replacement: reactDomDirectory + "/" },
      ],
    },
    server: {
      host: "127.0.0.1",
      port: 0,
      strictPort: false,
      fs: { allow: [root, fixtureRoot] },
    },
  });

  try {
    await server.listen();
    const address = server.httpServer?.address();
    const port = typeof address === "object" && address !== null ? address.port : null;
    if (!port) throw new Error("fixture server did not expose a loopback port");
    const url = `http://127.0.0.1:${port}/`;
    console.log(`FIXTURE_URL=${url}`);
    console.log("STARTUP_COMMAND=node tests/callmdLiveWorkspace.test.mjs --fixture");
    console.log("EXPECTED_READOUT=FIXTURE_COMPLETE plus seven visible PASS rows");
    console.log("EXPECTED_ROWS=hidden owner is removed from layout; visibility toggle keeps one mounted owner; light form labels preserve readable leaf color; dark form labels use readable workspace ink; StrictMode publishes authoritative inactive after replay; native-absent capability remains truthful after bootstrap; retained controller rejects after genuine unmount");
    console.log("EVIDENCE_BOUNDARY=component-only browser proof; not native, CI, packaged, provider, device, or production evidence");
    await new Promise((resolve) => {
      let stopping = false;
      const shutdown = async () => {
        if (stopping) return;
        stopping = true;
        await server.close();
        await cleanupFixtureRoot(fixtureRoot);
        resolve();
      };
      process.once("SIGINT", shutdown);
      process.once("SIGTERM", shutdown);
    });
  } catch (error) {
    await server.close();
    await cleanupFixtureRoot(fixtureRoot);
    throw error;
  }
}
