// @req AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-13
// @tested tests/callmdDesktopIntegration.test.mjs
//
// The normal Node mode verifies the production composition and exercises the
// accepted review controller. `--fixture` is deliberately separate: it
// starts a temporary Vite page for a real ReactDOMClient mount so a controller
// can observe the mounted registrar proof without changing the product route
// or pretending that browser execution is native evidence.

import assert from "node:assert/strict";
import { readFile, mkdtemp, rm, writeFile } from "node:fs/promises";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixtureMode = process.argv.includes("--fixture");

function countMatches(source, expression) {
  return [...source.matchAll(expression)].length;
}

const FIXTURE_DIRECTORY_PREFIX = "fung-callmd-react-fixture-";

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

async function settle() {
  for (let index = 0; index < 5; index += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function loadRecordingReviewProduction() {
  const sourcePath = path.resolve(root, "src/components/desktop/RecordingReview.tsx");
  const source = await readFile(sourcePath, "utf8");
  const require = createRequire(import.meta.url);
  const reactUrl = pathToFileURL(require.resolve("react")).href;
  const jsxRuntimeUrl = pathToFileURL(require.resolve("react/jsx-runtime")).href;
  const tauriUrl = pathToFileURL(path.resolve(root, "src/tauri.ts")).href;
  const contractsUrl = pathToFileURL(
    path.resolve(root, "src/components/desktop/contracts.ts"),
  ).href;

  let productionSource = ts.transpileModule(source, {
    compilerOptions: {
      jsx: ts.JsxEmit.ReactJSX,
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: sourcePath,
  }).outputText;

  productionSource = productionSource
    .replace(/import\s+["']\.\/RecordingReview\.css["'];?\s*/g, "")
    .replaceAll('"../../tauri.ts"', JSON.stringify(tauriUrl))
    .replaceAll('"./contracts.ts"', JSON.stringify(contractsUrl))
    .replaceAll('"react/jsx-runtime"', JSON.stringify(jsxRuntimeUrl))
    .replaceAll('"react"', JSON.stringify(reactUrl));

  return import(
    "data:text/javascript;base64," +
      Buffer.from(productionSource, "utf8").toString("base64")
  );
}

async function loadThemeHelpersProduction(appSource) {
  const helperStart = appSource.indexOf("type ResolvedTheme =");
  const helperEnd = appSource.indexOf("\ntype LibraryItem", helperStart);
  assert.ok(helperStart >= 0, "App must expose the production theme helper region");
  assert.ok(helperEnd > helperStart, "App theme helper region must have a bounded end");

  const helperSource = appSource.slice(helperStart, helperEnd);
  const productionSource = ts.transpileModule(helperSource, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: "App.theme-helpers.ts",
  }).outputText;

  return import(
    "data:text/javascript;base64," +
      Buffer.from(productionSource, "utf8").toString("base64")
  );
}

const NOW = "2026-09-17T00:00:00.000Z";
const pairA = { projectId: "project-a", recordingId: "recording-a" };

function makeRecording(key) {
  return {
    id: key.recordingId,
    projectId: key.projectId,
    source: "desktop",
    status: "complete",
    durationMs: 90000,
    createdAt: NOW,
    updatedAt: NOW,
    language: "th",
    channels: ["mic"],
    captureState: "inactive",
  };
}

function makeReviewBridge() {
  const calls = {
    list: 0,
    release: 0,
    get: 0,
    transcript: 0,
    summaries: 0,
    exports: 0,
  };

  const bridge = {
    async listRecordings(projectId) {
      calls.list += 1;
      return {
        projectId,
        snapshotId: "fixture-snapshot-" + calls.list,
        asOf: NOW,
        items: [makeRecording(pairA)],
        nextCursor: null,
      };
    },
    async releaseRecordingList() {
      calls.release += 1;
      return { released: true };
    },
    async getRecording(projectId, recordingId) {
      calls.get += 1;
      return makeRecording({ projectId, recordingId });
    },
    async listTranscriptSegments(projectId, recordingId) {
      calls.transcript += 1;
      return {
        segments: [{
          id: recordingId + "-segment",
          projectId,
          recordingId,
          speakerId: "speaker-1",
          speakerName: "ผู้พูด 1",
          startMs: 0,
          endMs: 1000,
          text: "fixture transcript",
          confidence: 0.99,
          createdAt: NOW,
        }],
        capped: false,
        cap: 0,
        cappedRecordingIds: [],
      };
    },
    async meetingSummaries() {
      calls.summaries += 1;
      return {
        rows: [],
        otherRecordings: 0,
        unattributable: 0,
        attributionComplete: true,
      };
    },
    async listExportArtifacts() {
      calls.exports += 1;
      return [];
    },
    async askRecording(projectId, recordingId, question, requestId) {
      return {
        projectId,
        recordingId,
        requestId,
        scope: "recording",
        status: "answered",
        answer: question,
        model: "fixture",
        sources: [],
        graphPolicy: "excluded",
        liveTailPolicy: "excluded",
      };
    },
    async openPlayback(projectId, recordingId) {
      return {
        handle: "fixture-handle",
        projectId,
        recordingId,
        channel: "mic",
        state: "paused",
        positionMs: 0,
        durationMs: 90000,
        streamEpoch: 1,
        degraded: false,
        missingRanges: [],
        outputLatencyMs: null,
        error: null,
      };
    },
    async controlPlayback(handle, expectedEpoch, action, positionMs = 0) {
      return {
        handle,
        projectId: pairA.projectId,
        recordingId: pairA.recordingId,
        channel: "mic",
        state: action === "play" ? "playing" : "paused",
        positionMs,
        durationMs: 90000,
        streamEpoch: expectedEpoch,
        degraded: false,
        missingRanges: [],
        outputLatencyMs: null,
        error: null,
      };
    },
    async getPlayback(handle) {
      return {
        handle,
        projectId: pairA.projectId,
        recordingId: pairA.recordingId,
        channel: "mic",
        state: "paused",
        positionMs: 0,
        durationMs: 90000,
        streamEpoch: 1,
        degraded: false,
        missingRanges: [],
        outputLatencyMs: null,
        error: null,
      };
    },
    async closePlayback() {
      return { closed: true };
    },
    async correctTranscriptSegment() {},
    async renameSpeaker() {},
    async createJob(jobType, projectId, recordingId) {
      return {
        id: "fixture-job",
        projectId,
        type: jobType,
        status: "queued",
        progress: 0,
        inputRefs: [recordingId],
        outputRefs: [],
        providerId: null,
        errorCode: null,
        errorMessage: null,
        startedAt: null,
        finishedAt: null,
        createdAt: NOW,
        updatedAt: NOW,
      };
    },
  };

  return { bridge, calls };
}

if (!fixtureMode) {
  const [appSource, stylesSource, mainSource, liveSource, reviewSource] = await Promise.all([
    readFile(path.resolve(root, "src/App.tsx"), "utf8"),
    readFile(path.resolve(root, "src/styles.css"), "utf8"),
    readFile(path.resolve(root, "src/main.tsx"), "utf8"),
    readFile(path.resolve(root, "src/components/LiveMeetingPanel.tsx"), "utf8"),
    readFile(path.resolve(root, "src/components/desktop/RecordingReview.tsx"), "utf8"),
  ]);

  test("App mounts one persistent Live owner and one persistent Review owner", () => {
    assert.equal(countMatches(appSource, /<LiveMeetingPanel\b/g), 1);
    assert.equal(countMatches(appSource, /<RecordingReview\b/g), 1);
    assert.match(appSource, /visible=\{activeSurface === "live"\}/);
    assert.match(appSource, /visible=\{activeSurface === "review"\}/);
    assert.match(appSource, /liveStatus=\{liveStatus\}\s+livePhase=\{liveSnapshot\.phase\}/);
    assert.match(appSource, /closeReviewPlayer=\{closeReviewPlayer\}/);
    assert.match(appSource, /onControllerChange=\{handleLiveControllerChange\}/);
    assert.match(appSource, /registerClosePlayer=\{registerClosePlayer\}/);
    assert.match(appSource, /registerRecoveryRefresh=\{registerRecoveryRefresh\}/);
    assert.doesNotMatch(appSource, /liveMeetingStatus\s*\(/);
    assert.doesNotMatch(appSource, /@tauri-apps\/api\/event/);
    assert.match(liveSource, /hidden=\{!visibleAttribute\}/);
    assert.match(reviewSource, /hidden=\{!visible\}/);
  });

  test("integration keeps project identity separate from recording identity and preserves unavailable reads", () => {
    assert.match(appSource, /projects\.some\(\(project\) => project\.id === selectedRecording\)/);
    assert.doesNotMatch(appSource, /libraryItems\.some\(\(item\) => item\.id === selectedRecording\)/);
    assert.match(appSource, /const \[reviewSelection, setReviewSelection\] = useState/);
    assert.match(appSource, /selection\.projectId !== selectedProjectId/);
    assert.match(appSource, /NATIVE_UNAVAILABLE/);
    assert.match(appSource, /setProjectState\(createReadState<Project\[\]>\("unavailable"/);
    assert.match(appSource, /project=\{projectState\}/);
    assert.match(appSource, /activeRecordingId/);
  });

  test("production paths keep close acknowledgement, true inactive stop, recovery mapping, and shell slots", () => {
    assert.match(liveSource, /runStartWithCloseAck\(/);
    assert.match(liveSource, /closeReviewPlayer\s*:/);
    assert.match(appSource, /return liveController\.stopAndLeave\(\)/);
    assert.match(appSource, /active === true \|\| liveStatus\.data\?\.stopping === true/);
    assert.match(appSource, /recoveryProjectByRecordingRef/);
    assert.match(appSource, /report\.interrupted/);
    assert.match(appSource, /adopted\?\.recordingId/);
    assert.match(appSource, /settingsSlot=/);
    assert.match(appSource, /pairingSlot=/);
    assert.match(appSource, /recoverySlot=/);
    assert.match(appSource, /<SettingsPanel[\s\S]*invoke=\{nativeInvoke\}/);
    assert.match(mainSource, /if \(rootRoute === "desktop"\) return <App \/>/);
    assert.match(mainSource, /React\.StrictMode/);
  });

  test("the new shell owns actions and removes the legacy presentation tree", () => {
    assert.match(appSource, /startRecording:\s*\(\) =>/);
    assert.match(appSource, /stopRecording:\s*handleStopCapture/);
    assert.match(appSource, /exportMedia:/);
    assert.doesNotMatch(appSource, /<InstrumentRail\b|<HomeScreen\b/);
    assert.doesNotMatch(appSource, /callmd-legacy-workspace|className="app-shell|fab-topbar|power-dock/);
    assert.match(appSource, /mainContent=\{\(/);
    assert.doesNotMatch(
      stylesSource,
      /(^|\n)\s*\.callmd-desktop-content\s*\{/m,
      "integration styles must remain shell scoped",
    );
  });

  test("integrated surfaces share the resolved theme owner and production system subscription follows cleanup", async () => {
    const helpers = await loadThemeHelpersProduction(appSource);
    assert.equal(helpers.resolveEffectiveTheme("light", "dark"), "light");
    assert.equal(helpers.resolveEffectiveTheme("dark", "light"), "dark");
    assert.equal(helpers.resolveEffectiveTheme("system", "light"), "light");
    assert.equal(helpers.resolveEffectiveTheme("system", "dark"), "dark");

    const mediaListeners = new Set();
    const media = {
      matches: false,
      addEventListener(type, listener) {
        assert.equal(type, "change");
        mediaListeners.add(listener);
      },
      removeEventListener(type, listener) {
        assert.equal(type, "change");
        mediaListeners.delete(listener);
      },
    };
    const matchMediaCalls = [];
    const previousWindow = globalThis.window;
    const hadWindow = Object.prototype.hasOwnProperty.call(globalThis, "window");
    globalThis.window = {
      matchMedia(query) {
        matchMediaCalls.push(query);
        return media;
      },
    };

    try {
      assert.equal(helpers.readSystemTheme(), "light");
      const resolvedThemes = [];
      const cleanup = helpers.subscribeToSystemTheme((theme) => resolvedThemes.push(theme));
      assert.deepEqual(resolvedThemes, ["light"]);
      assert.equal(mediaListeners.size, 1);
      assert.deepEqual(matchMediaCalls, ["(prefers-color-scheme: dark)", "(prefers-color-scheme: dark)"]);

      media.matches = true;
      for (const listener of mediaListeners) listener({ matches: true });
      media.matches = false;
      for (const listener of mediaListeners) listener({ matches: false });
      assert.deepEqual(resolvedThemes, ["light", "dark", "light"]);

      cleanup();
      assert.equal(mediaListeners.size, 0);
      media.matches = true;
      for (const listener of mediaListeners) listener({ matches: true });
      assert.deepEqual(resolvedThemes, ["light", "dark", "light"]);
    } finally {
      if (hadWindow) globalThis.window = previousWindow;
      else delete globalThis.window;
    }

    assert.match(appSource, /const \[systemTheme, setSystemTheme\] = useState<ResolvedTheme>\(\(\) => readSystemTheme\(\)\)/);
    assert.match(appSource, /useEffect\(\(\) => subscribeToSystemTheme\(setSystemTheme\), \[\]\)/);
    assert.match(appSource, /const effectiveTheme = resolveEffectiveTheme\(theme, systemTheme\)/);
    assert.ok(appSource.includes("className={`callmd-surface-stack theme-${effectiveTheme}`}"));
    assert.doesNotMatch(appSource, /className={`app-shell theme-\$\{effectiveTheme\}`}/);

    const surfaceStart = appSource.indexOf("className={`callmd-surface-stack theme-${effectiveTheme}`}");
    assert.ok(surfaceStart >= 0);
    assert.doesNotMatch(appSource, /callmd-legacy-workspace/);
    const surfaceMarkup = appSource.slice(surfaceStart);
    assert.match(surfaceMarkup, /<LiveMeetingPanel\b/);
    assert.match(surfaceMarkup, /<RecordingReview\b/);
  });

  test("actual RecordingReviewController enforces the full project/recording pair", async () => {
    const { RecordingReviewController } = await loadRecordingReviewProduction();
    const { bridge, calls } = makeReviewBridge();
    const controller = new RecordingReviewController(bridge);
    controller.syncScope(pairA.projectId, pairA);
    await settle();
    assert.equal(controller.snapshot.list.status, "ready");
    assert.equal(controller.snapshot.recording.status, "ready");

    const beforeForeign = calls.list;
    await controller.refreshRecovered({ projectId: "foreign-project", recordingId: pairA.recordingId });
    assert.equal(calls.list, beforeForeign, "a foreign project/recording pair must be a no-op");

    await controller.refreshRecovered(pairA);
    await settle();
    assert.ok(calls.list > beforeForeign, "the current pair must refresh through the production controller");
    controller.dispose();
  });
} else {
  const { createServer } = await import("vite");
  const { default: reactPlugin } = await import("@vitejs/plugin-react");
  const fixtureRoot = await mkdtemp(path.join(os.tmpdir(), "fung-callmd-react-fixture-"));
  console.log(`FIXTURE_GENERATED_ROOT=${fixtureRoot}`);
  const fixtureEntry = String.raw`
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { RecordingReview } from "@callmd-review";

const NOW = "2026-09-17T00:00:00.000Z";
const CURRENT = { projectId: "project-a", recordingId: "recording-a" };
const FOREIGN = { projectId: "foreign-project", recordingId: "recording-a" };

function row(key) {
  return {
    id: key.recordingId,
    projectId: key.projectId,
    source: "desktop",
    status: "complete",
    durationMs: 90000,
    createdAt: NOW,
    updatedAt: NOW,
    language: "th",
    channels: ["mic"],
    captureState: "inactive",
  };
}

function makeBridge(counters, redraw) {
  const bump = () => redraw((value) => value + 1);
  return {
    async listRecordings(projectId) {
      counters.list += 1;
      bump();
      return { projectId, snapshotId: "fixture-" + counters.list, asOf: NOW, items: [row(CURRENT)], nextCursor: null };
    },
    async releaseRecordingList() {
      counters.release += 1;
      bump();
      return { released: true };
    },
    async getRecording(projectId, recordingId) {
      counters.get += 1;
      bump();
      return row({ projectId, recordingId });
    },
    async listTranscriptSegments(projectId, recordingId) {
      counters.transcript += 1;
      bump();
      return { segments: [{ id: recordingId + "-segment", projectId, recordingId, speakerId: "speaker-1", speakerName: "fixture", startMs: 0, endMs: 1000, text: "fixture", confidence: 1, createdAt: NOW }], capped: false, cap: 0, cappedRecordingIds: [] };
    },
    async meetingSummaries() {
      counters.summaries += 1;
      bump();
      return { rows: [], otherRecordings: 0, unattributable: 0, attributionComplete: true };
    },
    async listExportArtifacts() {
      counters.exports += 1;
      bump();
      return [];
    },
    async askRecording(projectId, recordingId, question, requestId) {
      return { projectId, recordingId, requestId, scope: "recording", status: "answered", answer: question, model: "fixture", sources: [], graphPolicy: "excluded", liveTailPolicy: "excluded" };
    },
    async openPlayback(projectId, recordingId) {
      return { handle: "fixture-handle", projectId, recordingId, channel: "mic", state: "paused", positionMs: 0, durationMs: 90000, streamEpoch: 1, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
    },
    async controlPlayback(handle, expectedEpoch, action, positionMs = 0) {
      return { handle, projectId: CURRENT.projectId, recordingId: CURRENT.recordingId, channel: "mic", state: action === "play" ? "playing" : "paused", positionMs, durationMs: 90000, streamEpoch: expectedEpoch, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
    },
    async getPlayback(handle) {
      return { handle, projectId: CURRENT.projectId, recordingId: CURRENT.recordingId, channel: "mic", state: "paused", positionMs: 0, durationMs: 90000, streamEpoch: 1, degraded: false, missingRanges: [], outputLatencyMs: null, error: null };
    },
    async closePlayback() { return { closed: true }; },
    async correctTranscriptSegment() {},
    async renameSpeaker() {},
    async createJob(jobType, projectId, recordingId) {
      return { id: "fixture-job", projectId, type: jobType, status: "queued", progress: 0, inputRefs: [recordingId], outputRefs: [], providerId: null, errorCode: null, errorMessage: null, startedAt: null, finishedAt: null, createdAt: NOW, updatedAt: NOW };
    },
  };
}

function Result({ label, pass, detail }) {
  return <li data-result={pass ? "PASS" : "FAIL"} style={{ color: pass ? "#146b3a" : "#a31d1d" }}><strong>{pass ? "PASS" : "FAIL"}</strong> {label} — {detail}</li>;
}

function RegistrarFixture() {
  const counters = useRef({ list: 0, release: 0, get: 0, transcript: 0, summaries: 0, exports: 0 });
  const entries = useRef([]);
  const retainedA = useRef(null);
  const retainedB = useRef(null);
  const unmountBaseline = useRef(null);
  const [redrawVersion, redraw] = useState(0);
  const [mode, setMode] = useState("function");
  const [mounted, setMounted] = useState(true);
  const [phase, setPhase] = useState("boot");
  const [results, setResults] = useState([]);
  const bridge = useMemo(() => makeBridge(counters.current, redraw), [redraw]);
  const onSelect = useCallback(() => {}, []);
  const registerRecoveryRefresh = useCallback((callback) => {
    if (callback) {
      entries.current.push({ callback, mode });
      if (mode === "function") {
        return () => {
          counters.current.functionCleanup = (counters.current.functionCleanup ?? 0) + 1;
          redraw((value) => value + 1);
        };
      }
      return undefined;
    }
    counters.current.voidCleanup = (counters.current.voidCleanup ?? 0) + 1;
    redraw((value) => value + 1);
    return undefined;
  }, [mode, redraw]);

  useEffect(() => {
    if (phase !== "boot") return undefined;
    const timer = setTimeout(() => {
      const last = entries.current.at(-1)?.callback ?? null;
      retainedA.current = last;
      setMode("void");
      setPhase("replacement");
    }, 40);
    return () => clearTimeout(timer);
  }, [phase]);

  useEffect(() => {
    if (phase !== "replacement") return undefined;
    const timer = setTimeout(async () => {
      const callbackB = entries.current.at(-1)?.callback ?? null;
      retainedB.current = callbackB;
      const listBeforeA = counters.current.list;
      if (retainedA.current) await retainedA.current(CURRENT);
      const oldNoCalls = counters.current.list === listBeforeA;

      const listBeforeForeign = counters.current.list;
      if (callbackB) await callbackB(FOREIGN);
      const foreignNoOp = counters.current.list === listBeforeForeign;

      const listBeforeCurrent = counters.current.list;
      if (callbackB) await callbackB(CURRENT);
      const currentRefresh = counters.current.list === listBeforeCurrent + 1;

      unmountBaseline.current = { list: counters.current.list, release: counters.current.release };
      setResults([
        { label: "registrar A→B cleanup", pass: (counters.current.functionCleanup ?? 0) > 0, detail: "function cleanup calls=" + (counters.current.functionCleanup ?? 0) },
        { label: "retained A after replacement", pass: oldNoCalls, detail: "list delta=" + (counters.current.list - listBeforeA) },
        { label: "foreign pair no-op", pass: foreignNoOp, detail: "list delta=" + (counters.current.list - listBeforeForeign) },
        { label: "current pair refresh", pass: currentRefresh, detail: "list delta=" + (counters.current.list - listBeforeCurrent) },
      ]);
      setMounted(false);
      setPhase("unmounted");
    }, 40);
    return () => clearTimeout(timer);
  }, [phase]);

  useEffect(() => {
    if (phase !== "unmounted") return undefined;
    const timer = setTimeout(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
      const afterUnmount = { list: counters.current.list, release: counters.current.release };
      if (retainedB.current) await retainedB.current(CURRENT);
      const retainedNoCalls = counters.current.list === afterUnmount.list && counters.current.release === afterUnmount.release;
      setResults((current) => [
        ...current,
        { label: "void/null registrar cleanup", pass: (counters.current.voidCleanup ?? 0) > 0, detail: "null cleanup calls=" + (counters.current.voidCleanup ?? 0) },
        { label: "retained B after unmount", pass: retainedNoCalls, detail: "list/release delta=" + (counters.current.list - afterUnmount.list) + "/" + (counters.current.release - afterUnmount.release) },
      ]);
      setPhase("done");
    }, 40);
    return () => clearTimeout(timer);
  }, [phase]);

  void redrawVersion;
  return <main style={{ fontFamily: "system-ui", maxWidth: 960, margin: "32px auto", lineHeight: 1.6 }}>
    <h1>FUNG mounted registrar fixture</h1>
    <p>Component-only proof: real ReactDOMClient + production RecordingReview + deterministic bridge prop. No Tauri invoke is installed.</p>
    <p data-testid="phase">phase: {phase}</p>
    <p data-testid="counters">list={counters.current.list} release={counters.current.release} get={counters.current.get} transcript={counters.current.transcript} summaries={counters.current.summaries} exports={counters.current.exports}</p>
    {mounted ? <RecordingReview selectedProjectId={CURRENT.projectId} selection={CURRENT} onSelect={onSelect} visible={true} registerRecoveryRefresh={registerRecoveryRefresh} bridge={bridge} /> : null}
    <h2>Registrar results</h2>
    <ul>{results.map((result) => <Result key={result.label} {...result} />)}</ul>
    {phase === "done" ? <p data-testid="fixture-complete">FIXTURE_COMPLETE — read all PASS/FAIL rows and counters above.</p> : null}
  </main>;
}

createRoot(document.getElementById("root")).render(<RegistrarFixture />);
`;
  const fixtureHtml = `<!doctype html><html><head><meta charset="UTF-8"><title>FUNG registrar fixture</title></head><body><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>`;
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
        { find: "@callmd-review", replacement: path.resolve(root, "src/components/desktop/RecordingReview.tsx") },
        { find: /^react$/, replacement: reactDirectory },
        { find: /^react\//, replacement: reactDirectory + "/" },
        { find: /^react-dom$/, replacement: reactDomDirectory },
        { find: /^react-dom\//, replacement: reactDomDirectory + "/" },
      ],
    },
    server: {
      host: "127.0.0.1",
      port: 0,
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
    console.log("STARTUP_COMMAND=node tests/callmdDesktopIntegration.test.mjs --fixture");
    console.log("EXPECTED_READOUT=FIXTURE_COMPLETE plus six visible PASS rows");
    console.log("EXPECTED_ROWS=registrar A→B cleanup; retained A after replacement; foreign pair no-op; current pair refresh; void/null registrar cleanup; retained B after unmount");
    console.log("EVIDENCE_BOUNDARY=component-only browser proof; not native, CI, packaged, provider, device, or production evidence");
    await new Promise((resolve) => {
      const shutdown = async () => {
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
