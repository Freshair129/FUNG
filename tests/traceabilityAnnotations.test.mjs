import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const requirementsPath = new URL(
  "../docs/Desktop/LIVE_MEETING_EXTERNAL_RETRIEVAL_REQUIREMENTS.md",
  import.meta.url,
);

// Structured comments use, for example: `// @req FR-106, FR-107`.
// An annotation maps code/test intent; it is not runtime, device, or UAT evidence.
const annotationPattern = /@(req|spec|designs|tested)\s+((?:FR|NFR)-\d+(?:\s*,\s*(?:FR|NFR)-\d+)*)/g;
const requirementPattern = /^\|\s*((?:FR|NFR)-\d+)\s*\|/gm;

const requiredAnnotations = {
  "src-tauri/src/external_mcp.rs": ["FR-108", "FR-112", "FR-113", "NFR-103", "NFR-107", "NFR-108"],
  "src-tauri/src/external_mcp_transport.rs": ["FR-109", "FR-114", "NFR-105"],
  "src-tauri/src/external_mcp_commands.rs": ["FR-106", "FR-107", "FR-108", "FR-110", "FR-111", "FR-112", "FR-113", "FR-114", "FR-116", "NFR-102", "NFR-105", "NFR-107", "NFR-108", "NFR-110"],
  "src-tauri/src/live_meeting.rs": ["FR-102", "FR-103", "FR-104", "FR-114", "NFR-101", "NFR-104", "NFR-109"],
  "src-tauri/src/meeting_intel.rs": ["FR-104", "FR-105", "FR-115", "NFR-101", "NFR-104"],
  "src/lib/externalMeetingTools.ts": ["FR-106", "FR-107", "FR-108", "FR-112", "NFR-102"],
  "src/components/ExternalMeetingToolsPanel.tsx": ["FR-107", "FR-108", "FR-110", "FR-114", "FR-116", "NFR-106", "NFR-108"],
  "src/components/LiveMeetingPanel.tsx": ["FR-102", "FR-103", "FR-104", "FR-105", "FR-115", "NFR-104", "NFR-106"],
  "src/tauri.ts": ["FR-106", "FR-108", "FR-116"],
  "tests/externalMeetingTools.test.mjs": ["FR-106", "FR-107", "FR-108", "FR-110", "FR-112", "FR-116", "NFR-102", "NFR-106", "NFR-108", "NFR-110"],
  "tests/desktopBootstrap.test.mjs": ["FR-101"],
};

function annotationIds(source) {
  const ids = new Set();
  for (const match of source.matchAll(annotationPattern)) {
    for (const id of match[2].split(/\s*,\s*/)) ids.add(id);
  }
  return ids;
}

test("scoped External Retrieval files carry canonical RWANG requirement annotations", async () => {
  const requirements = await readFile(requirementsPath, "utf8");
  const canonicalIds = new Set(
    [...requirements.matchAll(requirementPattern)].map((match) => match[1]),
  );
  const missing = [];
  const unknown = [];

  for (const [path, expectedIds] of Object.entries(requiredAnnotations)) {
    const source = await readFile(new URL(`../${path}`, import.meta.url), "utf8");
    const annotatedIds = annotationIds(source);

    for (const id of annotatedIds) {
      if (!canonicalIds.has(id)) unknown.push(`${path}: ${id}`);
    }
    for (const id of expectedIds) {
      if (!canonicalIds.has(id)) unknown.push(`contract: ${path}: ${id}`);
      if (!annotatedIds.has(id)) missing.push(`${path}: ${id}`);
    }
  }

  assert.deepEqual(unknown, [], "every annotated requirement ID must exist in the canonical requirements document");
  assert.deepEqual(
    missing,
    [],
    "annotations establish source/test intent only; they do not prove runtime, device, or UAT completion",
  );
});
