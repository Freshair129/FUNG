// @req FR-106, FR-107, FR-108, FR-110, FR-112, FR-116, NFR-102, NFR-106, NFR-108, NFR-110
// @tested tests/externalMeetingTools.test.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  buildExternalToolArguments,
  createExternalToolUiState,
  externalMeetingToolsEnabled,
  externalToolErrorMessage,
  reduceExternalToolState,
} from "../src/lib/externalMeetingTools.ts";

test("external meeting tools stay disabled unless the explicit Vite flag is one", () => {
  assert.equal(externalMeetingToolsEnabled(undefined), false);
  assert.equal(externalMeetingToolsEnabled({}), false);
  assert.equal(externalMeetingToolsEnabled({ VITE_FUNG_EXTERNAL_MEETING_TOOLS: "true" }), false);
  assert.equal(externalMeetingToolsEnabled({ VITE_FUNG_EXTERNAL_MEETING_TOOLS: "1" }), true);
});

test("document and CRM arguments contain only fields approved by the capability", () => {
  assert.deepEqual(
    buildExternalToolArguments("documents.search", {
      query: "  contract  ",
      customerKey: "ignored",
      crmFields: ["status"],
    }),
    { query: "contract" },
  );
  assert.deepEqual(
    buildExternalToolArguments("crm.customer_status.read", {
      query: "ignored",
      customerKey: " customer-42 ",
      crmFields: ["status", "stage", "status", "unapproved"],
    }),
    { customerKey: "customer-42", fields: ["status", "stage"] },
  );
  assert.throws(
    () =>
      buildExternalToolArguments("documents.search", {
        query: " ",
        customerKey: "",
        crmFields: [],
      }),
    /query is required/i,
  );
});

test("tool UI state follows preview running completed and failure transitions", () => {
  const idle = createExternalToolUiState();
  const preview = reduceExternalToolState(idle, {
    type: "previewReady",
    preview: { id: "preview-1" },
  });
  assert.equal(preview.phase, "preview");

  const running = reduceExternalToolState(preview, { type: "executionStarted", runId: "run-1" });
  assert.equal(running.phase, "running");
  assert.equal(running.runId, "run-1");

  const completed = reduceExternalToolState(running, {
    type: "executionCompleted",
    execution: { run: { id: "run-1" }, result: { sourceRefs: ["kb://documents/42"] } },
  });
  assert.equal(completed.phase, "completed");
  assert.deepEqual(completed.execution.result.sourceRefs, ["kb://documents/42"]);

  const failed = reduceExternalToolState(running, {
    type: "executionFailed",
    error: "TOOL_TIMEOUT",
  });
  assert.equal(failed.phase, "failed");
  assert.equal(failed.runId, "run-1");
});

test("stable backend errors become Thai capture-safe operator messages", () => {
  assert.match(externalToolErrorMessage("TOOL_TIMEOUT"), /หมดเวลา/);
  assert.match(externalToolErrorMessage("TOOL_TIMEOUT"), /การบันทึกเสียงยังทำงานต่อ/);
  assert.match(externalToolErrorMessage("PREVIEW_CHANGED"), /เตรียมคำขอใหม่/);
});

test("Tauri client and Live Meeting surface expose only the approved operator workflow", async () => {
  const [tauriSource, panelSource, liveSource, liveStyles] = await Promise.all([
    readFile(new URL("../src/tauri.ts", import.meta.url), "utf8"),
    readFile(new URL("../src/components/ExternalMeetingToolsPanel.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/components/LiveMeetingPanel.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/components/LiveMeetingPanel.css", import.meta.url), "utf8"),
  ]);
  for (const command of [
    "external_connectors_list",
    "external_connector_register",
    "external_connector_disconnect",
    "meeting_tool_suggest",
    "meeting_tool_execute",
    "meeting_tool_cancel",
    "meeting_tool_revoke",
    "meeting_tool_runs_list",
  ]) {
    assert.match(tauriSource, new RegExp(`invoke<[^>]+>\\(\"${command}\"`));
  }
  assert.match(panelSource, /VITE_FUNG_EXTERNAL_MEETING_TOOLS/);
  assert.match(panelSource, /aria-live="polite"/);
  assert.match(panelSource, /เตรียมคำขอ/);
  assert.match(panelSource, /อนุมัติและค้นข้อมูล/);
  assert.match(panelSource, /ยกเลิกการค้น/);
  assert.match(panelSource, /meetingToolRevoke/);
  assert.match(panelSource, /เพิกถอนสิทธิ์การประชุม/);
  assert.match(panelSource, /การบันทึกเสียงยังทำงานต่อ/);
  assert.match(liveSource, /<ExternalMeetingToolsPanel/);
  assert.match(liveStyles, /@media \(max-width: 900px\)/);
  assert.match(liveStyles, /@media \(prefers-reduced-motion: reduce\)/);
  assert.match(liveStyles, /:focus-visible/);
  assert.doesNotMatch(liveStyles, /border-left:/);
});
