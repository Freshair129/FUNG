// @req AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-13
// @tested tests/callmdDesktopContracts.test.mjs
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

const EXPECTED_RED = "EXPECTED_RED_MISSING_IMPLEMENTATION";

// This is an IPC seam only. It intercepts calls made by the production bridge
// so wire names, arguments, DTOs, and error preservation can be tested before
// Rust exists. It deliberately contains no recording, playback, or Q&A logic.
const calls = [];
const responses = new Map();
const REJECTION = Symbol("rejection");

function resetIpc() {
  calls.length = 0;
  responses.clear();
}

function queueResponse(command, value) {
  const queue = responses.get(command) ?? [];
  queue.push(value);
  responses.set(command, queue);
}

function queueRejection(command, error) {
  queueResponse(command, { [REJECTION]: error });
}

async function mockInvoke(command, args = {}) {
  calls.push({ command, args });
  const queue = responses.get(command) ?? [];
  const next = queue.shift();
  responses.set(command, queue);
  if (next?.[REJECTION]) throw next[REJECTION];
  return next;
}

// Load the production bridge with the existing Tauri IPC mechanism available.
// Missing exports must remain assertion failures, not module-loader failures.
globalThis.window = { __TAURI_INTERNALS__: { invoke: mockInvoke } };
const api = await import("../src/tauri.ts");

const pairA1 = { projectId: "project-a", recordingId: "recording-a1" };
const pairB3 = { projectId: "project-b", recordingId: "recording-b3" };

const recordingA1 = {
  id: pairA1.recordingId,
  projectId: pairA1.projectId,
  source: "mic",
  status: "completed",
  durationMs: 12_000,
  createdAt: "2026-09-17T01:00:00.000Z",
  updatedAt: "2026-09-17T01:01:00.000Z",
  language: "th",
  channels: ["mic"],
  captureState: "inactive",
};

const recordingPage = {
  projectId: pairA1.projectId,
  snapshotId: "snapshot-a",
  asOf: "2026-09-17T01:02:00.000Z",
  items: [recordingA1],
  nextCursor: null,
};

const playbackState = {
  handle: "handle-a",
  projectId: pairA1.projectId,
  recordingId: pairA1.recordingId,
  channel: "mic",
  state: "paused",
  positionMs: 0,
  durationMs: recordingA1.durationMs,
  streamEpoch: 0,
  degraded: false,
  missingRanges: [],
  outputLatencyMs: null,
  error: null,
};

const recordingAnswer = {
  projectId: pairA1.projectId,
  recordingId: pairA1.recordingId,
  requestId: "request-a1",
  scope: "recording",
  status: "answered",
  answer: "มีการตกลงขั้นตอนถัดไป",
  model: "local-test-model",
  sources: [
    {
      segmentId: "segment-a1-1",
      projectId: pairA1.projectId,
      recordingId: pairA1.recordingId,
      startMs: 1000,
      endMs: 2500,
      text: "ขั้นตอนถัดไป",
      citationIndex: 0,
    },
  ],
  graphPolicy: "excluded",
  liveTailPolicy: "excluded",
};

function requiredExport(name) {
  assert.equal(
    typeof api[name],
    "function",
    `${EXPECTED_RED}: production bridge export ${name} is absent from src/tauri.ts`,
  );
  return api[name];
}

function lastCall(command) {
  const call = [...calls].reverse().find((candidate) => candidate.command === command);
  assert.ok(call, `production bridge did not invoke ${command}`);
  return call;
}

function reviewError(code, retryable = false) {
  return { code, message: `safe ${code.toLowerCase()} message`, retryable };
}

const reviewContractsPath = new URL("../src/components/desktop/contracts.ts", import.meta.url);

async function requiredReviewLoadSettlement() {
  assert.ok(
    existsSync(reviewContractsPath),
    `${EXPECTED_RED}: production shared review-load helper file src/components/desktop/contracts.ts is absent`,
  );

  let contracts;
  try {
    contracts = await import(reviewContractsPath.href);
  } catch (error) {
    assert.fail(
      `${EXPECTED_RED}: production shared desktop contracts module could not load: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  assert.equal(
    typeof contracts.settleReviewLoad,
    "function",
    `${EXPECTED_RED}: production shared review-load helper export settleReviewLoad is absent`,
  );
  return contracts.settleReviewLoad;
}

test("all approved NEW B bridge wrappers are exported as production functions", () => {
  const names = [
    "listRecordings",
    "releaseRecordingList",
    "getRecording",
    "askRecording",
    "openPlayback",
    "controlPlayback",
    "getPlayback",
    "closePlayback",
  ];
  const missing = names.filter((name) => typeof api[name] !== "function");
  assert.deepEqual(
    missing,
    [],
    `${EXPECTED_RED}: approved NEW B exports are not present: ${missing.join(", ")}`,
  );
});

test("recording history and scoped-Q&A wrappers preserve the approved wire DTOs", async () => {
  resetIpc();
  const listRecordings = requiredExport("listRecordings");
  const releaseRecordingList = requiredExport("releaseRecordingList");
  const getRecording = requiredExport("getRecording");
  const askRecording = requiredExport("askRecording");

  queueResponse("desktop_recordings_list", recordingPage);
  assert.deepEqual(await listRecordings(pairA1.projectId), recordingPage);
  assert.deepEqual(lastCall("desktop_recordings_list").args, {
    projectId: pairA1.projectId,
    limit: 50,
    cursor: null,
  });

  queueResponse("desktop_recordings_release", { released: true });
  assert.deepEqual(await releaseRecordingList(recordingPage.snapshotId), { released: true });
  assert.deepEqual(lastCall("desktop_recordings_release").args, {
    snapshotId: recordingPage.snapshotId,
  });

  queueResponse("desktop_recording_get", recordingA1);
  assert.deepEqual(await getRecording(pairA1.projectId, pairA1.recordingId), recordingA1);
  assert.deepEqual(lastCall("desktop_recording_get").args, pairA1);

  queueResponse("meeting_ask_recording", recordingAnswer);
  assert.deepEqual(
    await askRecording(pairA1.projectId, pairA1.recordingId, "What happened?", recordingAnswer.requestId),
    recordingAnswer,
  );
  assert.deepEqual(lastCall("meeting_ask_recording").args, {
    ...pairA1,
    question: "What happened?",
    requestId: recordingAnswer.requestId,
  });
  assert.equal(recordingAnswer.scope, "recording");
  assert.equal(recordingAnswer.graphPolicy, "excluded");
  assert.equal(recordingAnswer.liveTailPolicy, "excluded");
});

test("native playback wrappers use the approved handle/epoch/channel wire", async () => {
  resetIpc();
  const openPlayback = requiredExport("openPlayback");
  const controlPlayback = requiredExport("controlPlayback");
  const getPlayback = requiredExport("getPlayback");
  const closePlayback = requiredExport("closePlayback");

  queueResponse("desktop_playback_open", playbackState);
  const opened = await openPlayback(pairA1.projectId, pairA1.recordingId, "mic");
  assert.deepEqual(opened, playbackState);
  assert.equal(opened.state, "paused");
  assert.equal(opened.positionMs, 0);
  assert.deepEqual(lastCall("desktop_playback_open").args, {
    ...pairA1,
    channel: "mic",
  });

  queueResponse("desktop_playback_control", { ...playbackState, positionMs: 1500, streamEpoch: 1 });
  await controlPlayback(playbackState.handle, playbackState.streamEpoch, "seek", 1500);
  assert.deepEqual(lastCall("desktop_playback_control").args, {
    handle: playbackState.handle,
    expectedEpoch: playbackState.streamEpoch,
    action: "seek",
    positionMs: 1500,
  });

  queueResponse("desktop_playback_status", playbackState);
  assert.deepEqual(await getPlayback(playbackState.handle), playbackState);
  assert.deepEqual(lastCall("desktop_playback_status").args, { handle: playbackState.handle });

  queueResponse("desktop_playback_close", { closed: true });
  assert.deepEqual(await closePlayback(playbackState.handle), { closed: true });
  assert.deepEqual(lastCall("desktop_playback_close").args, { handle: playbackState.handle });
});

test("invalid and cross-recording pairs preserve typed native rejection", async () => {
  resetIpc();
  const getRecording = requiredExport("getRecording");
  const invalid = reviewError("INVALID_ARGUMENT");
  const mismatch = reviewError("SCOPE_MISMATCH");

  queueRejection("desktop_recording_get", invalid);
  await assert.rejects(
    () => getRecording("", pairA1.recordingId),
    (error) => {
      assert.deepEqual(error, invalid);
      return true;
    },
  );

  queueRejection("desktop_recording_get", mismatch);
  await assert.rejects(
    () => getRecording(pairA1.projectId, pairB3.recordingId),
    (error) => {
      assert.deepEqual(error, mismatch);
      return true;
    },
  );
  assert.deepEqual(calls.map(({ command, args }) => ({ command, args })), [
    { command: "desktop_recording_get", args: { projectId: "", recordingId: pairA1.recordingId } },
    { command: "desktop_recording_get", args: { projectId: pairA1.projectId, recordingId: pairB3.recordingId } },
  ]);
});

test("recording-scoped Q&A rejects a wrong recording pair at the native boundary", async () => {
  resetIpc();
  const askRecording = requiredExport("askRecording");
  const mismatch = reviewError("SCOPE_MISMATCH");

  queueRejection("meeting_ask_recording", mismatch);
  await assert.rejects(
    () => askRecording(pairA1.projectId, pairB3.recordingId, "Question?", "request-wrong-recording"),
    (error) => {
      assert.deepEqual(error, mismatch);
      return true;
    },
  );
  assert.deepEqual(lastCall("meeting_ask_recording").args, {
    projectId: pairA1.projectId,
    recordingId: pairB3.recordingId,
    question: "Question?",
    requestId: "request-wrong-recording",
  });
});

test("native playback rejects a wrong recording pair at the native boundary", async () => {
  resetIpc();
  const openPlayback = requiredExport("openPlayback");
  const mismatch = reviewError("SCOPE_MISMATCH");

  queueRejection("desktop_playback_open", mismatch);
  await assert.rejects(
    () => openPlayback(pairA1.projectId, pairB3.recordingId, "mic"),
    (error) => {
      assert.deepEqual(error, mismatch);
      return true;
    },
  );
  assert.deepEqual(lastCall("desktop_playback_open").args, {
    projectId: pairA1.projectId,
    recordingId: pairB3.recordingId,
    channel: "mic",
  });
});

test("existing transcript cap is ready data, while a native read failure remains an error", async () => {
  resetIpc();
  const capped = {
    segments: [{ id: "segment-a1-1", projectId: pairA1.projectId, recordingId: pairA1.recordingId }],
    capped: true,
    cap: 1000,
    cappedRecordingIds: [pairA1.recordingId],
  };
  queueResponse("list_transcript_segments", capped);
  const view = await api.listTranscriptSegments(pairA1.projectId, pairA1.recordingId);
  assert.equal(view.capped, true);
  assert.deepEqual(view.cappedRecordingIds, [pairA1.recordingId]);

  const storageFailure = reviewError("STORAGE_READ_FAILED");
  queueRejection("list_transcript_segments", storageFailure);
  await assert.rejects(
    () => api.listTranscriptSegments(pairA1.projectId, pairA1.recordingId),
    (error) => {
      assert.deepEqual(error, storageFailure);
      return true;
    },
  );
});

test("recording-list resource, cursor, unavailable, and legacy errors stay distinct", async () => {
  resetIpc();
  const listRecordings = requiredExport("listRecordings");
  const cases = [
    ["RESOURCE_LIMIT", false, null],
    ["CURSOR_INVALID", false, "cursor-invalid"],
    ["CURSOR_EXPIRED", false, "cursor-expired"],
    ["NATIVE_UNAVAILABLE", false, null],
    ["LEGACY_COMMAND_FAILED", false, null],
  ];

  for (const [code, retryable, cursor] of cases) {
    const expected = reviewError(code, retryable);
    queueRejection("desktop_recordings_list", expected);
    await assert.rejects(
      () => listRecordings(pairA1.projectId, 50, cursor),
      (error) => {
        assert.deepEqual(error, expected);
        return true;
      },
    );
  }
  assert.equal(new Set(cases.map(([code]) => code)).size, cases.length);
});

test("stale transcript success and rejection cannot replace the current production state", () => {
  const viewA = { segments: [{ recordingId: "recording-a1" }], capped: false, cap: 1000, cappedRecordingIds: [] };

  let current = api.beginTranscriptLoad(pairA1.recordingId, 1);
  current = api.settleTranscriptLoad(current, {
    requestId: 1,
    recordingId: pairA1.recordingId,
    outcome: { status: "fulfilled", view: viewA },
  });
  assert.equal(current.status, "ready");

  current = api.beginTranscriptLoad(pairB3.recordingId, 2);
  const beforeLateResults = current;
  const staleSuccess = api.settleTranscriptLoad(current, {
    requestId: 1,
    recordingId: pairA1.recordingId,
    outcome: { status: "fulfilled", view: viewA },
  });
  const staleError = api.settleTranscriptLoad(current, {
    requestId: 1,
    recordingId: pairA1.recordingId,
    outcome: { status: "rejected" },
  });
  assert.deepEqual(staleSuccess, beforeLateResults);
  assert.deepEqual(staleError, beforeLateResults);

  const currentError = api.settleTranscriptLoad(current, {
    requestId: 2,
    recordingId: pairB3.recordingId,
    outcome: { status: "rejected" },
  });
  assert.equal(currentError.status, "rejected");
  assert.equal(currentError.recordingId, pairB3.recordingId);
});

test("production review-load settlement preserves current state for each stale identity and settles matching outcomes", async () => {
  const settleReviewLoad = await requiredReviewLoadSettlement();
  const identity = {
    projectId: pairA1.projectId,
    recordingId: pairA1.recordingId,
    selectionEpoch: 4,
    requestId: "review-request-a1",
  };
  const current = {
    status: "loading",
    identity,
    data: null,
    error: null,
  };
  const data = { recording: recordingA1, transcript: ["segment-a1-1"] };
  const error = reviewError("STORAGE_READ_FAILED");
  const staleIdentities = [
    ["projectId", { ...identity, projectId: pairB3.projectId }],
    ["recordingId", { ...identity, recordingId: pairB3.recordingId }],
    ["selectionEpoch", { ...identity, selectionEpoch: identity.selectionEpoch + 1 }],
    ["requestId", { ...identity, requestId: "review-request-late" }],
  ];

  for (const [field, staleIdentity] of staleIdentities) {
    assert.strictEqual(
      settleReviewLoad(current, {
        identity: staleIdentity,
        outcome: { status: "fulfilled", data },
      }),
      current,
      `${EXPECTED_RED}: stale success changed state for mismatched ${field}`,
    );
  }

  for (const [field, staleIdentity] of staleIdentities) {
    assert.strictEqual(
      settleReviewLoad(current, {
        identity: staleIdentity,
        outcome: { status: "rejected", error },
      }),
      current,
      `${EXPECTED_RED}: stale rejection changed state for mismatched ${field}`,
    );
  }

  assert.deepEqual(
    settleReviewLoad(current, {
      identity,
      outcome: { status: "fulfilled", data },
    }),
    { ...current, status: "ready", data, error: null },
    `${EXPECTED_RED}: matching success must settle ReadState.ready with data`,
  );
  assert.deepEqual(
    settleReviewLoad(current, {
      identity,
      outcome: { status: "rejected", error },
    }),
    { ...current, status: "error", data: null, error },
    `${EXPECTED_RED}: matching rejection must settle ReadState.error with error`,
  );
});

test("recording-scoped Q&A keeps no-evidence, provider, model, and resource outcomes separate", async () => {
  resetIpc();
  const askRecording = requiredExport("askRecording");

  queueResponse("meeting_ask_recording", recordingAnswer);
  const answered = await askRecording(
    pairA1.projectId,
    pairA1.recordingId,
    "What happened?",
    recordingAnswer.requestId,
  );
  assert.equal(answered.status, "answered");
  assert.equal(answered.sources.every((source) =>
    source.projectId === pairA1.projectId && source.recordingId === pairA1.recordingId), true);
  const askWire = lastCall("meeting_ask_recording");
  assert.equal("graph" in askWire.args, false);
  assert.equal("liveTail" in askWire.args, false);
  assert.equal("sources" in askWire.args, false);

  const insufficient = {
    ...recordingAnswer,
    status: "insufficient_evidence",
    answer: "",
    model: null,
    sources: [],
  };
  queueResponse("meeting_ask_recording", insufficient);
  const noEvidence = await askRecording(pairA1.projectId, pairA1.recordingId, "Unknown?", "request-no-evidence");
  assert.equal(noEvidence.status, "insufficient_evidence");
  assert.deepEqual(noEvidence.sources, []);
  assert.equal(noEvidence.model, null);

  for (const code of ["RESOURCE_LIMIT", "PROVIDER_UNAVAILABLE", "PROVIDER_FAILED", "MODEL_OUTPUT_INVALID"]) {
    const expected = reviewError(code, false);
    queueRejection("meeting_ask_recording", expected);
    await assert.rejects(
      () => askRecording(pairA1.projectId, pairA1.recordingId, "Question?", `request-${code}`),
      (error) => {
        assert.deepEqual(error, expected);
        return true;
      },
    );
  }
  assert.equal(calls.some(({ command }) => command === "meeting_ask"), false);
});

test("existing summary and export wrappers retain their different scope contracts", async () => {
  resetIpc();
  const summaries = {
    rows: [],
    otherRecordings: 1,
    unattributable: 0,
    attributionComplete: true,
  };
  queueResponse("meeting_summaries", summaries);
  assert.deepEqual(await api.meetingSummaries(pairA1.projectId, pairA1.recordingId), summaries);
  assert.deepEqual(lastCall("meeting_summaries").args, pairA1);

  queueResponse("list_export_artifacts", []);
  assert.deepEqual(await api.listExportArtifacts(pairA1.projectId), []);
  assert.deepEqual(lastCall("list_export_artifacts").args, { projectId: pairA1.projectId });
  assert.equal("recordingId" in lastCall("list_export_artifacts").args, false);
});

test("native command/module presence is a separate expected-red gate", () => {
  const nativeFiles = [
    "src-tauri/src/recording_review.rs",
    "src-tauri/src/desktop_playback.rs",
  ];
  const sources = ["src-tauri/src/lib.rs", "src-tauri/src/meeting_intel.rs", ...nativeFiles]
    .filter((path) => existsSync(path))
    .map((path) => readFileSync(path, "utf8"))
    .join("\n");
  const missing = [
    ...nativeFiles.filter((path) => !existsSync(path)),
    ...[
      "desktop_recordings_list",
      "desktop_recordings_release",
      "desktop_recording_get",
      "meeting_ask_recording",
      "desktop_playback_open",
      "desktop_playback_control",
      "desktop_playback_status",
      "desktop_playback_close",
    ].filter((command) => !new RegExp(`\\b${command}\\b`).test(sources)),
  ];
  assert.deepEqual(
    missing,
    [],
    `${EXPECTED_RED}: native B command/module presence is not implemented: ${missing.join(", ")}`,
  );
});
