// @req FR-103
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  RUNNABLE_JOB_TYPES,
  isJobActionEnabled,
  jobActionBlockedReason,
  resolveJobAction,
} from "../src/lib/jobActions.ts";
globalThis.window = {};
const { beginTranscriptLoad, settleTranscriptLoad } = await import("../src/tauri.ts");

/**
 * The whole point of this module is that a button either does something or
 * says why it cannot. These tests pin both halves, and the last one pins the
 * runnable set against the Rust enum so the two cannot drift into a UI that
 * offers work the engine will refuse.
 */

test("the summary buttons all queue the one pass that produces them", () => {
  // Recap, intent, and actions are three views of a single summarisation
  // run. Queueing three jobs would run the same LLM pass three times.
  for (const value of ["summary.recap", "summary.intent", "summary.actions"]) {
    assert.deepEqual(resolveJobAction(value), {
      kind: "queue",
      jobType: "summary.generate",
      needsRecording: true,
    });
  }
});

test("transcribe is the import flow, not a queued job", () => {
  assert.deepEqual(resolveJobAction("transcript.transcribe"), {
    kind: "import",
  });
});

test("the transcript catch-up pass and the graph build queue as themselves", () => {
  assert.equal(resolveJobAction("transcript.retry").jobType, "transcript.retry");
  assert.equal(resolveJobAction("graph.build").jobType, "graph.build");
});

test("every button with no implementation says which one it is", () => {
  // A generic "not available" would leave the user unable to tell a missing
  // model from a feature nobody wrote. Each reason must be distinct.
  const inert = [
    "capture.marker",
    "speakers.lock",
    "review.evidence",
    "summary.compare",
    "export.queue",
    "archive.project",
  ];
  const reasons = new Set();
  for (const value of inert) {
    const plan = resolveJobAction(value);
    assert.equal(plan.kind, "unavailable", `${value} must not be runnable`);
    assert.ok(plan.reason.length > 0, `${value} needs a reason`);
    reasons.add(plan.reason);
  }
  assert.equal(
    reasons.size,
    inert.length,
    "each unavailable action needs its own reason, not one shared blanket",
  );
});

test("an unknown job type is refused rather than passed through", () => {
  const plan = resolveJobAction("something.invented");
  assert.equal(plan.kind, "unavailable");
  assert.match(plan.reason, /something\.invented/);
});

test("an unimplemented action is disabled even when a recording exists", () => {
  assert.equal(isJobActionEnabled("archive.project", true), false);
  assert.equal(
    jobActionBlockedReason("archive.project", true),
    resolveJobAction("archive.project").reason,
  );
});

test("a runnable action waits for a recording rather than queueing nothing", () => {
  assert.equal(isJobActionEnabled("summary.recap", false), false);
  assert.equal(isJobActionEnabled("summary.recap", true), true);
  // The two blocked states must read differently: one clears on its own.
  assert.notEqual(
    jobActionBlockedReason("summary.recap", false),
    jobActionBlockedReason("archive.project", false),
  );
  assert.equal(jobActionBlockedReason("summary.recap", true), null);
});

test("import stays available before any recording exists", () => {
  // It is how the first recording gets made; gating it on one would deadlock.
  assert.equal(isJobActionEnabled("transcript.transcribe", false), true);
});

test("the runnable set matches the job kinds the engine registers", () => {
  // The Rust enum is the authority. If a kind is added there and not here,
  // the UI silently keeps calling it unsupported.
  const rust = readFileSync("src-tauri/src/job_engine.rs", "utf8");
  const arm = /JobKind::\w+ => "([a-z.]+)"/g;
  const registered = new Set();
  for (const match of rust.matchAll(arm)) {
    registered.add(match[1]);
  }
  assert.deepEqual(
    [...registered].sort(),
    [...RUNNABLE_JOB_TYPES].sort(),
    "job_engine::JobKind and RUNNABLE_JOB_TYPES have drifted apart",
  );
});

test("the desktop shell disables tile buttons instead of filing dead rows", () => {
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(
    app,
    /disabled=\{[^}]*!tileActionEnabled\(currentTile\.primaryAction\)/,
    "the primary tile button must respect what can actually run",
  );
  assert.match(
    app,
    /disabled=\{!tileActionEnabled\(currentTile\.secondaryAction\)\}/,
    "the secondary tile button must respect what can actually run",
  );
  assert.match(
    app,
    /className="action-notice"/,
    "a refused action must have somewhere to say why",
  );
});

test("queued jobs target the project's active recording", () => {
  const app = readFileSync("src/App.tsx", "utf8");
  // There is no recording picker in this shell; queueing against anything
  // other than the active recording would summarise the wrong meeting.
  assert.match(app, /activeRecordingId/);
  assert.match(
    app,
    /createJob\(plan\.jobType, selectedProjectId, activeRecordingId\)/,
  );
});

// @req D-MVP-01
test("transcript load state rejects stale completions and failed reads", () => {
  const viewA = {
    segments: [{ recordingId: "rec-a" }],
    capped: false,
    cap: 1000,
    cappedRecordingIds: [],
  };
  const viewB = {
    segments: [{ recordingId: "rec-b" }],
    capped: false,
    cap: 1000,
    cappedRecordingIds: [],
  };

  let state = beginTranscriptLoad("rec-a", 1);
  state = settleTranscriptLoad(state, {
    requestId: 1,
    recordingId: "rec-a",
    outcome: { status: "fulfilled", view: viewA },
  });
  assert.equal(state.status, "ready");
  assert.equal(state.view, viewA);

  state = beginTranscriptLoad("rec-b", 2);
  const stale = settleTranscriptLoad(state, {
    requestId: 1,
    recordingId: "rec-a",
    outcome: { status: "fulfilled", view: viewA },
  });
  assert.equal(stale.status, "loading");
  assert.equal(stale.view, null);

  const current = settleTranscriptLoad(state, {
    requestId: 2,
    recordingId: "rec-b",
    outcome: { status: "fulfilled", view: viewB },
  });
  assert.equal(current.status, "ready");
  assert.equal(current.recordingId, "rec-b");
  assert.equal(current.view, viewB);

  const rejected = settleTranscriptLoad(beginTranscriptLoad("rec-c", 3), {
    requestId: 3,
    recordingId: "rec-c",
    outcome: { status: "rejected" },
  });
  assert.equal(rejected.status, "rejected");
  assert.equal(rejected.view, null);
});

// @req D-MVP-01
test("an imported transcription success activates and finalizes its recording", () => {
  const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
  const pipeline = lib.slice(lib.indexOf("fn run_import_pipeline"));
  const success = pipeline.slice(pipeline.indexOf("Ok(output)"), pipeline.indexOf("Err(message)", pipeline.indexOf("Ok(output)")));
  const finalizer = lib.slice(lib.indexOf("fn finalize_import_success"), lib.indexOf("fn run_import_pipeline"));

  assert.match(
    success,
    /finalize_import_success\(/,
    "successful import must run the atomic handoff finalizer",
  );
  assert.match(
    finalizer,
    /upsert\(\s*"projects"[\s\S]*active_recording_id[\s\S]*recording_id[\s\S]*\)/,
    "successful import must atomically select the imported recording",
  );
  assert.match(
    finalizer,
    /audio_chunks[\s\S]*end_ms[\s\S]*output\.duration_ms[\s\S]*transcribed_at/,
    "successful import must finalize the imported chunk and stamp transcription",
  );
});

test("transcript retrieval is recording-scoped through the desktop bridge", () => {
  const api = readFileSync("src/tauri.ts", "utf8");
  const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
  const app = readFileSync("src/App.tsx", "utf8");

  assert.match(
    api,
    /export async function listTranscriptSegments\(\s*projectId: string,\s*recordingId: string,\s*\)/,
    "the bridge must require the selected recording",
  );
  assert.match(
    lib,
    /fn list_transcript_segments\([\s\S]*project_id: String,[\s\S]*recording_id: String,/,
    "the Tauri command must require the selected recording",
  );
  assert.match(
    lib,
    /transcript_segments[\s\S]*recording_id[\s\S]*serde_json::json!\(recording_id\)/,
    "the backend query must filter by recording id",
  );
  assert.match(
    app,
    /listTranscriptSegments\(selectedProjectId, activeRecordingId\)/,
    "the shell must load only the active recording transcript",
  );
});

test("an import rejection is shown as a terminal failure with a next step", () => {
  const app = readFileSync("src/App.tsx", "utf8");
  const importFlow = app.slice(
    app.indexOf("const handleImportAndTranscribe"),
    app.indexOf("const transcriptBlockedReason"),
  );
  assert.match(
    importFlow,
    /const job = await importAndTranscribe\([\s\S]*const finished = await pollJobUntilDone\(job\.id\)[\s\S]*catch \{[\s\S]*setActionNotice\("นำเข้าและถอดเสียงไม่สำเร็จ — ตรวจสอบไฟล์และ local model แล้วลองใหม่"\)/,
    "the import flow must surface rejected import or polling calls with an actionable notice",
  );
  assert.match(
    importFlow,
    /finished\?\.status === "failed"[\s\S]*setActionNotice\("นำเข้าและถอดเสียงไม่สำเร็จ — ตรวจสอบไฟล์และ local model แล้วลองใหม่"\)/,
    "a terminal failed job must use the same non-secret notice",
  );
  assert.doesNotMatch(importFlow, /finished\.errorMessage/, "the notice must not echo worker error details");
});
