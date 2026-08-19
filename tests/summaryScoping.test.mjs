// @req FR-104
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { EMPTY_MEETING_SUMMARIES } from "../src/lib/meetingSummaries.ts";

/**
 * The attribution itself lives in Rust (`meeting_intel::attribute_summaries`)
 * and is unit-tested there. What these pin is the seam: that the client
 * cannot ask for summaries without naming a recording, and that it renders
 * the current summary rather than whichever row happens to come first.
 */

const panel = readFileSync("src/components/LiveMeetingPanel.tsx", "utf8");
const api = readFileSync("src/tauri.ts", "utf8");
const rust = readFileSync("src-tauri/src/meeting_intel.rs", "utf8");

test("summaries cannot be asked for without a recording", () => {
  // A project-only read is the defect: it returned every meeting in the
  // project as though they all belonged to the session on screen.
  assert.match(
    api,
    /export async function meetingSummaries\(\s*projectId: string,\s*recordingId: string,\s*\)/,
    "the client call must require a recording id",
  );
  assert.match(
    rust,
    /pub\(crate\) fn meeting_summaries\(\s*project_id: String,\s*recording_id: String,/,
    "the command must require a recording id",
  );
});

test("the panel scopes to the recording the summary event names", () => {
  // Not the panel's own active ids: a queued summary can land after the
  // session has been cleared, and then the ids are null or already moved on.
  assert.match(
    panel,
    /loadSummaries\(\s*activeProjectRef\.current,\s*event\.payload\.recordingId,?\s*\)/,
    "the reload must use the recording the event reported",
  );
});

test("the panel shows the current summary of each kind, not the first row", () => {
  assert.match(
    panel,
    /row\.kind === kind && !row\.superseded/,
    "an older duplicate must not be rendered as the current summary",
  );
});

test("what the query left out is shown, not swallowed", () => {
  // A user who now sees fewer rows than before is owed the reason.
  assert.match(panel, /summaries\.otherRecordings > 0/);
  assert.match(panel, /row\.superseded/);
  assert.match(
    panel,
    /summaries\.unattributable > 0 && summaries\.attributionComplete/,
    "orphans must only be claimed when the lookup was complete",
  );
});

test("starting a session clears the previous meeting's counts too", () => {
  // Clearing only the rows would leave a stale "N more in this project"
  // line describing a query that no longer applies.
  assert.match(panel, /setSummaries\(EMPTY_MEETING_SUMMARIES\)/);
  assert.deepEqual(EMPTY_MEETING_SUMMARIES, {
    rows: [],
    otherRecordings: 0,
    unattributable: 0,
    attributionComplete: true,
  });
});

test("the response shape matches what Rust serialises", () => {
  // serde renames to camelCase, so the two lists must agree field for field
  // or a count silently reads as undefined and never renders.
  const struct = rust.slice(
    rust.indexOf("pub(crate) struct MeetingSummaries {"),
    rust.indexOf("/// Number of evidence segment ids"),
  );
  const camel = (name) =>
    name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
  const fields = [...struct.matchAll(/pub\(crate\) (\w+):/g)]
    .map((match) => camel(match[1]))
    .sort();
  assert.deepEqual(fields, Object.keys(EMPTY_MEETING_SUMMARIES).sort());
});
