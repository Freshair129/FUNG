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
    "speakers.diarize",
    "review.evidence",
    "summary.compare",
    "export.render",
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
