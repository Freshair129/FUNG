import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import ts from "typescript";

const root = process.cwd();
const require = createRequire(import.meta.url);
const sourcePath = path.resolve(root, "src/components/desktop/RecordingReview.tsx");
const sourceText = readFileSync(sourcePath, "utf8");

const tauriUrl = pathToFileURL(path.resolve(root, "src/tauri.ts")).href;
const contractsUrl = pathToFileURL(
  path.resolve(root, "src/components/desktop/contracts.ts"),
).href;
const reactUrl = pathToFileURL(require.resolve("react")).href;
const jsxRuntimeUrl = pathToFileURL(require.resolve("react/jsx-runtime")).href;

let productionSource = ts.transpileModule(sourceText, {
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

const production = await import(
  "data:text/javascript;base64," +
    Buffer.from(productionSource, "utf8").toString("base64"),
);

const {
  PLAYBACK_POLL_INTERVAL_MS,
  RECORDING_PAGE_LIMIT,
  RecordingReviewController,
  clampPlaybackPosition,
  consumePlaybackEvent,
  formatRecordingDuration,
  isRecordingAnswerFor,
  playbackPositionForKey,
  sameReviewIdentity,
} = production;

const A = { projectId: "project-a", recordingId: "recording-a" };
const B = { projectId: "project-a", recordingId: "recording-b" };
const NOW = "2026-09-17T00:00:00.000Z";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
}

async function settle() {
  for (let index = 0; index < 4; index += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function pairKey(projectId, recordingId) {
  return projectId + ":" + recordingId;
}

function recording(key) {
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

function page(projectId, snapshotId, nextCursor, items) {
  return {
    projectId,
    snapshotId,
    asOf: NOW,
    items,
    nextCursor,
  };
}

function transcript(key, text = "บทถอดเสียง") {
  return {
    segments: [
      {
        id: key.recordingId + "-segment-1",
        projectId: key.projectId,
        recordingId: key.recordingId,
        speakerId: "speaker-1",
        speakerName: "ผู้พูด 1",
        startMs: 0,
        endMs: 1200,
        text,
        confidence: 0.99,
        createdAt: NOW,
      },
    ],
    capped: false,
    cap: 0,
    cappedRecordingIds: [],
  };
}

function summaries(key) {
  return {
    rows: [
      {
        id: key.recordingId + "-summary-1",
        kind: "meeting",
        content: JSON.stringify({ text: "สรุปจริง" }),
        evidenceCount: 1,
        createdAt: NOW,
        recordingId: key.recordingId,
        superseded: false,
      },
    ],
    otherRecordings: 1,
    unattributable: 0,
    attributionComplete: true,
  };
}

function answer(key, requestId, text = "คำตอบจากบันทึกนี้") {
  return {
    projectId: key.projectId,
    recordingId: key.recordingId,
    requestId,
    scope: "recording",
    status: "answered",
    answer: text,
    model: "test-model",
    sources: [
      {
        segmentId: key.recordingId + "-segment-1",
        projectId: key.projectId,
        recordingId: key.recordingId,
        startMs: 0,
        endMs: 1200,
        text: "แหล่งอ้างอิงจากบันทึก",
        citationIndex: 1,
      },
    ],
    graphPolicy: "excluded",
    liveTailPolicy: "excluded",
  };
}

function playback(
  key,
  handle,
  state = "paused",
  streamEpoch = 7,
  positionMs = 0,
) {
  return {
    handle,
    projectId: key.projectId,
    recordingId: key.recordingId,
    channel: "mic",
    state,
    positionMs,
    durationMs: 90000,
    streamEpoch,
    degraded: false,
    missingRanges: [],
    outputLatencyMs: null,
    error: null,
  };
}

function job(projectId, recordingId, type) {
  return {
    id: "job-1",
    projectId,
    type,
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
}

function createBridge(overrides = {}) {
  const calls = {
    list: [],
    release: [],
    get: [],
    transcript: [],
    summaries: [],
    exports: [],
    ask: [],
    open: [],
    control: [],
    poll: [],
    close: [],
    jobs: [],
    corrections: [],
    speakers: [],
  };

  const bridge = {
    async listRecordings(projectId, limit, cursor) {
      calls.list.push({ projectId, limit, cursor });
      if (overrides.listRecordings) {
        return overrides.listRecordings(projectId, limit, cursor);
      }
      return page(projectId, "snapshot-" + projectId, null, [
        recording({ projectId, recordingId: "recording-a" }),
      ]);
    },
    async releaseRecordingList(snapshotId) {
      calls.release.push(snapshotId);
      if (overrides.releaseRecordingList) {
        return overrides.releaseRecordingList(snapshotId);
      }
      return { released: true };
    },
    async getRecording(projectId, recordingId) {
      calls.get.push({ projectId, recordingId });
      if (overrides.getRecording) {
        return overrides.getRecording(projectId, recordingId);
      }
      return recording({ projectId, recordingId });
    },
    async listTranscriptSegments(projectId, recordingId) {
      calls.transcript.push({ projectId, recordingId });
      if (overrides.listTranscriptSegments) {
        return overrides.listTranscriptSegments(projectId, recordingId);
      }
      return transcript({ projectId, recordingId });
    },
    async meetingSummaries(projectId, recordingId) {
      calls.summaries.push({ projectId, recordingId });
      if (overrides.meetingSummaries) {
        return overrides.meetingSummaries(projectId, recordingId);
      }
      return summaries({ projectId, recordingId });
    },
    async listExportArtifacts(projectId) {
      calls.exports.push({ projectId });
      if (overrides.listExportArtifacts) {
        return overrides.listExportArtifacts(projectId);
      }
      return [
        {
          id: "export-1",
          kind: "srt",
          filePath: "export-1.srt",
          createdAt: NOW,
        },
      ];
    },
    async askRecording(projectId, recordingId, question, requestId) {
      calls.ask.push({ projectId, recordingId, question, requestId });
      if (overrides.askRecording) {
        return overrides.askRecording(projectId, recordingId, question, requestId);
      }
      return answer({ projectId, recordingId }, requestId);
    },
    async openPlayback(projectId, recordingId, channel) {
      calls.open.push({ projectId, recordingId, channel });
      if (overrides.openPlayback) {
        return overrides.openPlayback(projectId, recordingId, channel);
      }
      return playback({ projectId, recordingId }, "handle-" + recordingId);
    },
    async controlPlayback(handle, expectedEpoch, action, positionMs) {
      calls.control.push({ handle, expectedEpoch, action, positionMs });
      if (overrides.controlPlayback) {
        return overrides.controlPlayback(handle, expectedEpoch, action, positionMs);
      }
      return playback(A, handle, action === "play" ? "playing" : "paused", expectedEpoch, positionMs ?? 0);
    },
    async getPlayback(handle) {
      calls.poll.push({ handle });
      if (overrides.getPlayback) {
        return overrides.getPlayback(handle);
      }
      return playback(A, handle);
    },
    async closePlayback(handle) {
      calls.close.push(handle);
      if (overrides.closePlayback) {
        return overrides.closePlayback(handle);
      }
      return { closed: true };
    },
    async correctTranscriptSegment(projectId, recordingId, segmentId, correctedText) {
      calls.corrections.push({ projectId, recordingId, segmentId, correctedText });
      if (overrides.correctTranscriptSegment) {
        return overrides.correctTranscriptSegment(
          projectId,
          recordingId,
          segmentId,
          correctedText,
        );
      }
      return undefined;
    },
    async renameSpeaker(speakerId, displayName) {
      calls.speakers.push({ speakerId, displayName });
      if (overrides.renameSpeaker) {
        return overrides.renameSpeaker(speakerId, displayName);
      }
      return undefined;
    },
    async createJob(jobType, projectId, recordingId) {
      calls.jobs.push({ jobType, projectId, recordingId });
      if (overrides.createJob) {
        return overrides.createJob(jobType, projectId, recordingId);
      }
      return job(projectId, recordingId, jobType);
    },
  };

  return { bridge, calls };
}

async function loadedController(bridge, selection = A) {
  const controller = new RecordingReviewController(bridge);
  controller.syncScope(selection.projectId, selection);
  await settle();
  assert.equal(controller.snapshot.list.status, "ready");
  assert.equal(controller.snapshot.recording.status, "ready");
  assert.equal(controller.snapshot.transcript.status, "ready");
  assert.equal(controller.snapshot.summaries.status, "ready");
  assert.equal(controller.snapshot.exportState.status, "ready");
  return controller;
}

test("production helpers preserve exact identity, bounds, and honest duration formatting", () => {
  assert.equal(formatRecordingDuration(0), "0:00");
  assert.equal(formatRecordingDuration(3661000), "1:01:01");
  assert.equal(clampPlaybackPosition(-1, 90000), 0);
  assert.equal(clampPlaybackPosition(90001, 90000), 90000);
  assert.equal(playbackPositionForKey("ArrowRight", 1000, 90000), 6000);
  assert.equal(playbackPositionForKey("ArrowLeft", 1000, 90000), 0);
  assert.equal(playbackPositionForKey("Home", 1000, 90000), 0);
  assert.equal(playbackPositionForKey("End", 1000, 90000), 90000);
  assert.equal(playbackPositionForKey(" ", 1000, 90000), null);
  assert.equal(
    sameReviewIdentity(
      { ...A, selectionEpoch: 2, requestId: "read-1" },
      { ...A, selectionEpoch: 2, requestId: "read-1" },
    ),
    true,
  );
  assert.equal(
    sameReviewIdentity(
      { ...A, selectionEpoch: 2, requestId: "read-1" },
      { ...A, selectionEpoch: 3, requestId: "read-1" },
    ),
    false,
  );
});

test("playback UI event boundaries consume typed action rejections", async () => {
  const unhandled = [];
  const onUnhandled = (reason) => unhandled.push(reason);
  process.on("unhandledRejection", onUnhandled);

  const opening = deferred();
  const controlling = deferred();
  consumePlaybackEvent(opening.promise);
  consumePlaybackEvent(controlling.promise);
  opening.reject({ code: "PLAYBACK_IO_FAILED" });
  controlling.reject({ code: "PLAYBACK_STALE_EPOCH" });

  await settle();
  process.off("unhandledRejection", onUnhandled);
  assert.deepEqual(unhandled, []);
});

test("selection success and rejection races cannot overwrite the current exact pair", async () => {
  const pending = new Map();
  for (const key of [A, B]) {
    pending.set(pairKey(key.projectId, key.recordingId), {
      recording: deferred(),
      transcript: deferred(),
      summaries: deferred(),
    });
  }
  const { bridge, calls } = createBridge({
    getRecording: (projectId, recordingId) =>
      pending.get(pairKey(projectId, recordingId)).recording.promise,
    listTranscriptSegments: (projectId, recordingId) =>
      pending.get(pairKey(projectId, recordingId)).transcript.promise,
    meetingSummaries: (projectId, recordingId) =>
      pending.get(pairKey(projectId, recordingId)).summaries.promise,
  });
  const controller = new RecordingReviewController(bridge);

  controller.syncScope(A.projectId, A);
  await settle();
  controller.syncScope(B.projectId, B);
  await settle();

  const current = pending.get(pairKey(B.projectId, B.recordingId));
  current.recording.resolve(recording(B));
  current.transcript.resolve(transcript(B, "บทของ B"));
  current.summaries.resolve(summaries(B));
  await settle();

  const old = pending.get(pairKey(A.projectId, A.recordingId));
  old.recording.reject(new Error("late /private/token"));
  old.transcript.reject(new Error("late transcript"));
  old.summaries.reject(new Error("late summary"));
  await settle();

  assert.equal(controller.snapshot.recording.status, "ready");
  assert.equal(controller.snapshot.recording.data.id, B.recordingId);
  assert.equal(controller.snapshot.transcript.status, "ready");
  assert.equal(controller.snapshot.transcript.data.segments[0].recordingId, B.recordingId);
  assert.equal(controller.snapshot.summaries.status, "ready");
  assert.equal(controller.snapshot.summaries.data.rows[0].recordingId, B.recordingId);
  assert.equal(controller.snapshot.recording.error, null);
  assert.deepEqual(calls.get, [
    { projectId: A.projectId, recordingId: A.recordingId },
    { projectId: B.projectId, recordingId: B.recordingId },
  ]);
  assert.deepEqual(calls.transcript, [
    { projectId: A.projectId, recordingId: A.recordingId },
    { projectId: B.projectId, recordingId: B.recordingId },
  ]);
  controller.dispose();
});

test("history pages use limit 50 and release snapshots on refresh and disposal", async () => {
  let listCall = 0;
  const { bridge, calls } = createBridge({
    listRecordings: (projectId, limit, cursor) => {
      listCall += 1;
      if (listCall === 1) {
        return page(projectId, "snapshot-1", "cursor-1", [recording(A)]);
      }
      if (cursor === "cursor-1") {
        return page(projectId, "snapshot-1", null, [recording(B)]);
      }
      return page(projectId, "snapshot-2", null, [recording(A)]);
    },
  });
  const controller = new RecordingReviewController(bridge);
  controller.syncScope(A.projectId, null);
  await settle();

  assert.equal(calls.list[0].limit, RECORDING_PAGE_LIMIT);
  assert.equal(RECORDING_PAGE_LIMIT, 50);
  await controller.listNext();
  assert.deepEqual(calls.list[1], {
    projectId: A.projectId,
    limit: 50,
    cursor: "cursor-1",
  });
  assert.deepEqual(
    controller.snapshot.list.data.items.map((item) => item.id),
    [A.recordingId, B.recordingId],
  );

  await controller.refresh();
  await settle();
  assert.ok(calls.release.includes("snapshot-1"));
  assert.equal(controller.snapshot.list.data.snapshotId, "snapshot-2");

  controller.dispose();
  await settle();
  assert.ok(calls.release.includes("snapshot-2"));
});

test("recording Q&A, export listing, and queued jobs keep project/recording scope", async () => {
  let mode = "wrong";
  const { bridge, calls } = createBridge({
    askRecording: (projectId, recordingId, question, requestId) => {
      if (mode === "wrong") return answer(B, requestId, "ผิดคู่");
      if (mode === "error") return Promise.reject(new Error("C:\\secret\\token"));
      return answer({ projectId, recordingId }, requestId, "คำตอบถูกคู่");
    },
  });
  const controller = await loadedController(bridge, A);

  controller.setQuestion("คำถามแรก");
  await assert.rejects(
    () => controller.ask(A, "คำถามแรก", "ask-wrong"),
    (error) => error?.code === "SCOPE_MISMATCH",
  );
  assert.equal(controller.snapshot.ask.error.code, "SCOPE_MISMATCH");

  mode = "error";
  controller.setQuestion("เก็บคำถามเมื่อเกิดข้อผิดพลาด");
  await assert.rejects(
    () => controller.ask(A, "เก็บคำถามเมื่อเกิดข้อผิดพลาด", "ask-error"),
    (error) => error instanceof Error,
  );
  assert.equal(controller.snapshot.question, "เก็บคำถามเมื่อเกิดข้อผิดพลาด");
  assert.equal(controller.snapshot.ask.error.code, "LEGACY_COMMAND_FAILED");
  assert.equal(controller.snapshot.ask.error.message, "Legacy desktop command failed.");

  mode = "answer";
  controller.setQuestion("คำถามที่สำเร็จ");
  const successful = await controller.ask(A, "คำถามที่สำเร็จ", "ask-ok");
  assert.equal(successful.recordingId, A.recordingId);
  assert.equal(controller.snapshot.ask.status, "ready");
  controller.setQuestion("คำถามใหม่");
  assert.equal(controller.snapshot.ask.status, "idle");
  assert.equal(controller.snapshot.ask.data, null);

  assert.deepEqual(calls.exports[0], { projectId: A.projectId });
  await controller.queueExistingJob(A, "export.render");
  assert.deepEqual(calls.jobs.at(-1), {
    jobType: "export.render",
    projectId: A.projectId,
    recordingId: A.recordingId,
  });
  assert.match(sourceText, /ไฟล์ส่งออกทั้งโครงการ/);
  assert.doesNotMatch(sourceText, /meetingAsk/);
  assert.ok(calls.ask.every((call) => call.projectId === A.projectId && call.recordingId === A.recordingId));
  controller.dispose();
});

test("playback opens explicitly paused, serializes epoch controls, and closes late handles", async () => {
  const openPending = deferred();
  const controls = [];
  const { bridge, calls } = createBridge({
    openPlayback: () => openPending.promise,
    controlPlayback: (handle, expectedEpoch, action, positionMs) => {
      const operation = deferred();
      controls.push({ handle, expectedEpoch, action, positionMs, operation });
      return operation.promise;
    },
    getPlayback: (handle) => Promise.resolve(playback(A, handle, "playing", 7, 1200)),
  });
  const controller = await loadedController(bridge, A);

  assert.equal(calls.open.length, 0);
  const opening = controller.openPlayback(A, "mic");
  await settle();
  assert.equal(calls.open.length, 1);
  assert.equal(calls.control.length, 0);
  openPending.resolve(playback(A, "handle-a", "paused", 7, 0));
  const opened = await opening;
  assert.equal(opened.state, "paused");
  assert.equal(controller.snapshot.playback.data.state, "paused");
  assert.equal(calls.control.length, 0);

  await assert.rejects(
    () => controller.playbackControl("handle-a", 6, "play"),
    (error) => error?.code === "PLAYBACK_STALE_EPOCH",
  );
  assert.equal(calls.control.length, 0);

  const firstControl = controller.playbackControl("handle-a", 7, "play");
  await settle();
  const secondControl = controller.playbackControl("handle-a", 7, "seek", 5000);
  await settle();
  assert.equal(controls.length, 1);
  assert.equal(controls[0].action, "play");
  controls[0].operation.resolve(playback(A, "handle-a", "playing", 7, 0));
  await firstControl;
  await settle();
  assert.equal(controls.length, 2);
  assert.deepEqual(controls[1], {
    handle: "handle-a",
    expectedEpoch: 7,
    action: "seek",
    positionMs: 5000,
    operation: controls[1].operation,
  });
  controls[1].operation.resolve(playback(A, "handle-a", "paused", 7, 5000));
  await secondControl;

  const stopPolling = controller.startPolling();
  await controller.pollNow();
  assert.equal(calls.poll.length, 1);
  const pollCount = calls.poll.length;
  controller.setVisible(false);
  stopPolling();
  await settle();
  await delay(300);
  assert.equal(calls.poll.length, pollCount);
  controller.dispose();
  await settle();
  assert.ok(calls.release.includes("snapshot-project-a"));

  const lateOpen = deferred();
  const late = createBridge({ openPlayback: () => lateOpen.promise });
  const lateController = await loadedController(late.bridge, A);
  const lateOpening = lateController.openPlayback(A, "file");
  await settle();
  lateController.syncScope(B.projectId, B);
  await settle();
  lateOpen.resolve(playback(A, "late-handle", "paused", 8, 0));
  await assert.rejects(
    () => lateOpening,
    (error) => error?.code === "PLAYBACK_STALE_EPOCH",
  );
  await settle();
  assert.ok(late.calls.close.includes("late-handle"));
  lateController.dispose();
});

test("playback polling fences overlaps and ignores stale success and rejection", async () => {
  const firstPollDeferred = deferred();
  const staleSuccessDeferred = deferred();
  const staleRejectionDeferred = deferred();
  const polls = [
    firstPollDeferred,
    staleSuccessDeferred,
    staleRejectionDeferred,
  ];
  const { bridge, calls } = createBridge({
    getPlayback: () => polls.shift().promise,
  });
  const controller = await loadedController(bridge, A);
  await controller.openPlayback(A, "mic");

  const firstPoll = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 1);

  const overlappingPoll = controller.pollNow();
  await overlappingPoll;
  assert.equal(calls.poll.length, 1);

  firstPollDeferred.resolve(
    playback(A, "handle-recording-a", "paused", 7, 300),
  );
  await firstPoll;

  const staleSuccess = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 2);
  controller.syncScope(B.projectId, B);
  await settle();
  staleSuccessDeferred.resolve(
    playback(A, "handle-recording-a", "playing", 7, 1000),
  );
  await staleSuccess;
  await settle();
  assert.equal(controller.snapshot.playback.identity, null);
  assert.equal(controller.snapshot.playback.error, null);

  await controller.openPlayback(B, "mic");
  const staleRejection = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 3);
  controller.syncScope(A.projectId, A);
  await settle();
  staleRejectionDeferred.reject(new Error("late stale playback failure"));
  await staleRejection;
  await settle();
  assert.equal(controller.snapshot.playback.identity, null);
  assert.equal(controller.snapshot.playback.error, null);
  controller.dispose();
});

test("hung polls release only their own generation fence across close and reopen", async () => {
  const oldSuccess = deferred();
  const currentSuccess = deferred();
  const oldRejection = deferred();
  const currentRejection = deferred();
  const pollRequests = [
    oldSuccess,
    currentSuccess,
    oldRejection,
    currentRejection,
  ];
  const { bridge, calls } = createBridge({
    getPlayback: () => pollRequests.shift().promise,
  });
  const controller = await loadedController(bridge, A);

  async function closeAndReopen() {
    controller.setVisible(false);
    assert.deepEqual(await controller.closePlayer(), { closed: true });
    controller.setVisible(true);
    await controller.openPlayback(A, "mic");
  }

  await controller.openPlayback(A, "mic");
  const oldSuccessPoll = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 1);

  await closeAndReopen();
  const currentSuccessPoll = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 2);
  await controller.pollNow();
  assert.equal(calls.poll.length, 2);

  oldSuccess.resolve(playback(A, "handle-recording-a", "playing", 7, 300));
  await oldSuccessPoll;
  await settle();
  assert.equal(calls.poll.length, 2);
  currentSuccess.resolve(
    playback(A, "handle-recording-a", "playing", 7, 900),
  );
  await currentSuccessPoll;
  assert.equal(controller.snapshot.playback.data.positionMs, 900);

  const oldRejectionPoll = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 3);
  await closeAndReopen();
  const currentRejectionPoll = controller.pollNow();
  await settle();
  assert.equal(calls.poll.length, 4);
  await controller.pollNow();
  assert.equal(calls.poll.length, 4);

  oldRejection.reject(new Error("late old poll failure"));
  await oldRejectionPoll;
  await settle();
  assert.equal(calls.poll.length, 4);
  assert.equal(controller.snapshot.playback.error, null);
  currentRejection.resolve(
    playback(A, "handle-recording-a", "playing", 7, 1200),
  );
  await currentRejectionPoll;
  assert.equal(controller.snapshot.playback.data.positionMs, 1200);
  controller.dispose();
});

test("recovery refresh seam accepts only the mounted selected pair", async () => {
  const { bridge, calls } = createBridge();
  const controller = await loadedController(bridge, A);
  const initialListCalls = calls.list.length;
  const initialReleases = calls.release.length;
  const refreshRecovered = controller.refreshRecovered;

  await refreshRecovered(B);
  await settle();
  assert.equal(calls.list.length, initialListCalls);
  assert.equal(calls.release.length, initialReleases);

  await refreshRecovered(A);
  await settle();
  assert.equal(calls.list.length, initialListCalls + 1);
  assert.ok(calls.release.length > initialReleases);

  controller.syncScope(B.projectId, B);
  await settle();
  const staleListCalls = calls.list.length;
  const staleReleases = calls.release.length;
  await refreshRecovered(A);
  await settle();
  assert.equal(calls.list.length, staleListCalls);
  assert.equal(calls.release.length, staleReleases);

  controller.dispose();
  await settle();
  const disposedListCalls = calls.list.length;
  const disposedReleases = calls.release.length;
  await refreshRecovered(B);
  await settle();
  assert.equal(calls.list.length, disposedListCalls);
  assert.equal(calls.release.length, disposedReleases);
});

test("failed close keeps capture custody without republishing stale player UI", async () => {
  const closeResults = [
    { closed: false },
    { closed: false },
    { closed: true },
  ];
  const { bridge, calls } = createBridge({
    closePlayback: async () => closeResults.shift(),
  });
  const controller = await loadedController(bridge, A);
  await controller.openPlayback(A, "mic");

  controller.syncScope(B.projectId, B);
  await settle();
  assert.equal(calls.close.length, 1);
  assert.equal(controller.snapshot.playback.status, "idle");
  assert.equal(controller.snapshot.playback.identity, null);

  const retry = await controller.closePlayer();
  assert.deepEqual(retry, { closed: false });
  assert.equal(calls.close.length, 2);
  assert.equal(controller.snapshot.playback.status, "idle");
  assert.equal(controller.snapshot.playback.identity, null);

  const finalClose = await controller.closePlayer();
  assert.deepEqual(finalClose, { closed: true });
  assert.equal(calls.close.length, 3);
  controller.dispose();
});

test("the source keeps browser/native boundaries explicit and exposes no autoplay shortcuts", () => {
  assert.doesNotMatch(sourceText, /\bfetch\s*\(/);
  assert.doesNotMatch(sourceText, /new\s+Audio|AudioContext|URL\.createObjectURL/);
  assert.doesNotMatch(sourceText, /onKeyDown=\{[^}]*Space/);
  assert.match(sourceText, /askRecording/);
  assert.match(sourceText, /ไฟล์ส่งออกทั้งโครงการ/);
  assert.match(sourceText, /registerRecoveryRefresh/);
  assert.match(sourceText, /refreshRecovered/);
  assert.equal(PLAYBACK_POLL_INTERVAL_MS, 250);
});
