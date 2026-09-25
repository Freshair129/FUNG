import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MeetingIntelligencePanel } from "../src/components/MeetingIntelligencePanel.tsx";
import type {
  MeetingAgentStatus,
  MeetingEventHandlers,
  MeetingIntelligenceService,
  MeetingPreflight,
  TranscriptSnapshot,
  TranscriptUtterance,
} from "../src/lib/meetingIntelligence.ts";

const projectId = "project-ui-fixture";
const recordingId = "recording-ui-fixture";
const nativeScope = {
  project_id: projectId,
  recording_id: recordingId,
  meeting_session_id: "meeting-ui-fixture",
  source_session_id: "source-ui-fixture",
  track_id: "mic",
  source_generation: 1,
};

let transcript: TranscriptSnapshot = {
  projectId,
  recordingId,
  cursor: 4,
  revision: 1,
  utterances: [
    {
      utteranceId: "utterance-ui-fixture",
      revisionId: "revision-ui-fixture-1",
      revision: 1,
      startMs: 1000,
      endMs: 2400,
      rawText: "ยอดขายไตรมาสนี้เท่ากับ 42 ล้านบาท",
      text: "ยอดขายไตรมาสนี้เท่ากับ 42 ล้านบาท",
      state: "committed",
      origin: "local_asr",
      language: "th",
      confidence: 0.97,
      modelRunId: "local-run-fixture",
      scope: nativeScope,
      speakerLabel: null,
      speakerId: null,
      actorKind: "human",
    },
  ],
  gaps: [],
  legacySnapshot: false,
};

const capability = (readiness: "ready" | "blocked" | "unavailable", reasonCode: string | null = null) => ({
  readiness,
  reasonCode,
});

let status: MeetingAgentStatus = {
  projectId,
  recordingId,
  revision: 1,
  lastObservedTranscriptCursor: 0,
  mode: "off",
  state: "stopped",
  expiresAt: null,
  blockers: [],
  allowedTopics: ["sales"],
  localAgent: capability("ready"),
  transcriptRead: capability("ready"),
  knowledgeRead: capability("ready"),
  automaticTrigger: capability("blocked", "TRUSTED_PARTICIPANT_ATTRIBUTION_UNAVAILABLE"),
  externalJoin: capability("unavailable", "PROVIDER_UNCONFIGURED"),
  externalMediaRead: capability("unavailable", "PROVIDER_UNCONFIGURED"),
  externalChatSend: capability("unavailable", "PROVIDER_UNCONFIGURED"),
  externalLinkSend: capability("unavailable", "PROVIDER_UNCONFIGURED"),
  externalFileUpload: capability("unavailable", "PROVIDER_UNCONFIGURED"),
};

const collections = [
  {
    collectionId: "collection-ui-fixture",
    label: "Fixture: รายงานยอดขาย Q3",
    readable: true,
    selected: true,
    classification: "confidential",
  },
];

const subscriptions: MeetingEventHandlers[] = [];
let fixtureRevision = status.revision;

const service = {
  vaultOptions: async () => [{ vaultId: "vault-ui-fixture", keyAvailable: true, accountBound: true }],
  provisionVault: async () => "vault-ui-fixture",
  unlockVault: async () => undefined,
  lockVault: async () => undefined,
  createCollection: async () => ({ collectionId: "collection-ui-fixture" }),
  importSelectedDocument: async (_project: string, collectionId: string) => ({
    collectionId,
    documentId: "document-ui-fixture",
    versionId: "version-ui-fixture-1",
    chunkCount: 1,
    contentSha256: "a".repeat(64),
    parserVersion: "fixture-parser-1",
    warnings: [],
    citationLocators: [{ chunkId: "chunk-ui-fixture", label: "หน้า 1 · บรรทัด 1" }],
  }),
  saveMetric: async () => { throw new Error("Not used by this UI fixture"); },
  computeMetric: async () => { throw new Error("Not used by this UI fixture"); },
  people: async () => ({ vaultId: "vault-ui-fixture", profiles: [], speakers: [], links: [] }),
  createPersonProfile: async () => { throw new Error("Not used by this UI fixture"); },
  updatePersonProfile: async () => { throw new Error("Not used by this UI fixture"); },
  archivePersonProfile: async () => undefined,
  proposeSpeakerIdentity: async () => { throw new Error("Not used by this UI fixture"); },
  confirmSpeakerIdentity: async () => undefined,
  rejectSpeakerIdentity: async () => undefined,
  unlinkSpeakerIdentity: async () => undefined,
  snapshot: async () => transcript,
  replay: async () => ({
    recordingId,
    highWatermark: transcript.cursor,
    nextCursor: transcript.cursor,
    hasMore: false,
    events: [],
  }),
  correct: async (request: { revision: { raw_text: string; effective_text: string } }) => {
    const prior = transcript.utterances[0];
    const corrected: TranscriptUtterance = {
      ...prior,
      revisionId: "revision-ui-fixture-2",
      revision: 2,
      rawText: request.revision.raw_text,
      text: request.revision.effective_text,
      state: "reviewed",
      origin: "human",
      confidence: null,
      modelRunId: null,
    };
    transcript = { ...transcript, cursor: transcript.cursor + 1, revision: 2, utterances: [corrected] };
    return { utteranceId: corrected.utteranceId, revision: 2, cursor: transcript.cursor, committed: true as const };
  },
  collections: async () => collections,
  setCollections: async (request: { collectionIds: string[] }) => ({ revision: ++fixtureRevision, selectedCollectionIds: request.collectionIds }),
  preflight: async (): Promise<MeetingPreflight> => ({
    status,
    collectionRevision: fixtureRevision,
    selectedCollectionIds: collections.filter((row) => row.selected).map((row) => row.collectionId),
    localLimits: { maxActiveRuns: 1, maxRetrievalAttempts: 1, triggerExpiryMs: 10000, maxRunsPerMinute: 1, maxRunsPerHour: 5 },
  }),
  start: async (request: { mode: "observe" | "draft" }) => {
    status = { ...status, revision: ++fixtureRevision, mode: request.mode, state: request.mode === "draft" ? "drafting" : "observing" };
    return status;
  },
  pause: async () => { status = { ...status, revision: ++fixtureRevision, state: "paused" }; return status; },
  stop: async () => { status = { ...status, revision: ++fixtureRevision, mode: "off", state: "stopped" }; return status; },
  setPolicy: async (request: { mode: "off" | "observe" | "draft" }) => {
    status = { ...status, revision: ++fixtureRevision, mode: request.mode, state: request.mode === "off" ? "stopped" : request.mode === "draft" ? "drafting" : "observing" };
    return status;
  },
  ask: async () => ({
    draftId: "draft-ui-fixture",
    revision: 1,
    text: "จากเอกสารที่เลือก ยอดขายเท่ากับ 42 ล้านบาท",
    citations: [{ documentId: "document-ui-fixture", versionId: "version-ui-fixture-1", locator: "page:1;line:1", label: "รายงานยอดขาย Q3 · หน้า 1" }],
    basedOnTranscriptCursor: transcript.cursor,
    expiresAt: "2026-09-25T04:00:00+07:00",
    state: "private" as const,
  }),
  previewDelivery: async () => ({
    intentId: "intent-ui-fixture",
    payloadHash: "b".repeat(64),
    destinationSummary: "Google Meet chat · fixture room",
    state: "awaiting_approval" as const,
    approvalScope: "local_preview_only" as const,
    externalDispatchAvailable: false as const,
  }),
  approveDelivery: async () => ({
    intentId: "intent-ui-fixture",
    payloadHash: "b".repeat(64),
    destinationSummary: "Google Meet chat · fixture room",
    state: "approved_local_only" as const,
    approvalScope: "local_preview_only" as const,
    externalDispatchAvailable: false as const,
  }),
  revoke: async () => {
    status = { ...status, revision: ++fixtureRevision, mode: "off", state: "stopped" };
    return status;
  },
  status: async () => status,
  history: async () => [],
  subscribe: async (_scope: unknown, handlers: MeetingEventHandlers) => {
    subscriptions.push(handlers);
    return () => undefined;
  },
} as unknown as MeetingIntelligenceService;

function Fixture() {
  const [mounted, setMounted] = useState(true);
  const [generation, setGeneration] = useState(0);
  return (
    <>
      <button
        id="remount-panel-action"
        type="button"
        onClick={() => { setMounted(false); setGeneration((value) => value + 1); queueMicrotask(() => setMounted(true)); }}
      >
        รีเซ็ตแผงจำลอง
      </button>
      <button id="stale-subscription-action" type="button" onClick={() => {
        const oldHandler = subscriptions[0]?.onTranscript;
        oldHandler?.({
          type: "revision",
          projectId,
          recordingId,
          cursor: transcript.cursor + 10,
          utterances: [{ ...transcript.utterances[0], text: "STALE EVENT MUST NOT APPEAR", revision: 99 }],
        });
        const visibleText = document.querySelector(".meeting-intelligence__transcript")?.textContent ?? "";
        const probe = document.querySelector<HTMLOutputElement>("#fixture-probe");
        if (probe) probe.value = visibleText.includes("STALE EVENT MUST NOT APPEAR")
          ? "FAIL — stale subscription changed the visible transcript"
          : "PASS — stale subscription was ignored after remount";
      }}>ทดสอบ stale subscription</button>
      <output id="fixture-probe" role="status">ยังไม่ทดสอบ stale subscription</output>
      <button id="locked-vault-draft-action" type="button" onClick={() => {
        const staleText = "STALE DRAFT MUST NOT APPEAR AFTER VAULT LOCK";
        subscriptions[subscriptions.length - 1]?.onDraft({
          projectId,
          recordingId,
          draft: {
            draftId: "draft-after-lock-fixture",
            revision: 1,
            text: staleText,
            citations: [],
            basedOnTranscriptCursor: transcript.cursor,
            expiresAt: "2026-09-25T04:00:00+07:00",
            state: "private",
          },
        });
        setTimeout(() => {
          const visibleText = document.querySelector(".meeting-intelligence__draft-text")?.textContent ?? "";
          const probe = document.querySelector<HTMLOutputElement>("#vault-lock-probe");
          if (probe) probe.value = visibleText.includes(staleText)
            ? "FAIL — locked vault accepted a delayed private draft"
            : "PASS — locked vault ignored a delayed private draft";
        }, 0);
      }}>ทดสอบ draft ที่มาช้าหลังล็อก vault</button>
      <output id="vault-lock-probe" role="status">ยังไม่ทดสอบ draft หลังล็อก vault</output>
      <button id="account-lifecycle-action" type="button" onClick={() => {
        subscriptions[subscriptions.length - 1]?.onAccountLifecycleChanged?.({ state: "signed_out", accountGeneration: 2 });
        setTimeout(() => {
          const visibleText = document.querySelector(".meeting-intelligence__draft-text")?.textContent ?? "";
          const probe = document.querySelector<HTMLOutputElement>("#account-lifecycle-probe");
          if (probe) probe.value = visibleText.includes("จากเอกสารที่เลือก ยอดขายเท่ากับ 42 ล้านบาท")
            ? "FAIL — account lifecycle left a private draft visible"
            : "PASS — account lifecycle cleared the private draft";
        }, 0);
      }}>ทดสอบ account lifecycle</button>
      <output id="account-lifecycle-probe" role="status">ยังไม่ทดสอบ account lifecycle</output>
      {mounted && <MeetingIntelligencePanel key={generation} projectId={projectId} recordingId={recordingId} service={service} />}
    </>
  );
}

createRoot(document.getElementById("root")!).render(<Fixture />);
