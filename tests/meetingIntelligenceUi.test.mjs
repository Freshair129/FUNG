import assert from "node:assert/strict";
import test from "node:test";
import {
  applyTranscriptEvent,
  buildHumanCorrectionRequest,
  canUseLocalDrafting,
  createMeetingIntelligenceService,
  MEETING_INTELLIGENCE_EXTERNAL_DISPATCH_AVAILABLE,
  normalizeReplayPage,
  normalizeTranscriptSnapshot,
} from "../src/lib/meetingIntelligence.ts";

const nativeScope = {
  project_id: "project-a",
  recording_id: "recording-a",
  meeting_session_id: "meeting-a",
  source_session_id: "source-a",
  track_id: "mic",
  source_generation: 1,
};

const nativeUtterance = (revision, text) => ({
  id: `revision-${revision}`,
  utterance_id: "utterance-a",
  revision,
  supersedes_revision: revision > 1 ? revision - 1 : null,
  expected_revision: revision > 1 ? revision - 1 : null,
  origin: revision > 1 ? "human" : "local_asr",
  raw_text: text,
  effective_text: text,
  language: "th",
  confidence: revision > 1 ? null : 0.95,
  start_ms: 1000,
  end_ms: 2400,
  model_run_id: revision > 1 ? null : "run-a",
  review_state: revision > 1 ? "reviewed" : "committed",
  scope: nativeScope,
});

test("private drafting is unavailable until vault, transcript, agent and knowledge are ready", () => {
  const status = {
    localAgent: { readiness: "ready" },
    transcriptRead: { readiness: "ready" },
    knowledgeRead: { readiness: "ready" },
  };
  assert.equal(canUseLocalDrafting(false, false, status), false);
  assert.equal(canUseLocalDrafting(true, true, status), false);
  assert.equal(canUseLocalDrafting(true, false, null), false);
  assert.equal(canUseLocalDrafting(true, false, { ...status, knowledgeRead: { readiness: "blocked" } }), false);
  assert.equal(canUseLocalDrafting(true, false, status), true);
});

function snapshot(cursor = 4, utterances = []) {
  return {
    projectId: "project-a",
    recordingId: "recording-a",
    cursor,
    revision: utterances.reduce((value, row) => Math.max(value, row.revision), 0),
    utterances,
    gaps: [],
    legacySnapshot: false,
  };
}

test("snapshot normalization keeps raw and effective revision text with full scope", () => {
  const normalized = normalizeTranscriptSnapshot({
    recording_id: "recording-a",
    high_watermark: 9,
    utterances: [nativeUtterance(2, "แก้ไขแล้ว")],
  }, "project-a");

  assert.equal(normalized.projectId, "project-a");
  assert.equal(normalized.cursor, 9);
  assert.equal(normalized.utterances[0].rawText, "แก้ไขแล้ว");
  assert.equal(normalized.utterances[0].text, "แก้ไขแล้ว");
  assert.equal(normalized.utterances[0].scope.source_session_id, "source-a");
  assert.equal(normalized.utterances[0].state, "reviewed");
});

test("human correction request binds expected revision and native source scope", () => {
  const utterance = normalizeTranscriptSnapshot({
    recording_id: "recording-a",
    high_watermark: 9,
    utterances: [nativeUtterance(3, "ข้อความเดิม")],
  }, "project-a").utterances[0];

  const request = buildHumanCorrectionRequest(utterance, " ข้อความใหม่ ");
  assert.deepEqual(request.scope, nativeScope);
  assert.equal(request.revision.utterance_id, "utterance-a");
  assert.equal(request.revision.revision, 4);
  assert.equal(request.revision.supersedes_revision, 3);
  assert.equal(request.revision.expected_revision, 3);
  assert.equal(request.revision.origin, "human");
  assert.equal(request.revision.raw_text, "ข้อความใหม่");
  assert.equal(request.revision.effective_text, "ข้อความใหม่");
  assert.equal(request.revision.review_state, "reviewed");
  assert.equal(request.revision.model_run_id, null);
});

test("transcript reducer ignores duplicate, stale and cross-recording events", () => {
  const existing = normalizeTranscriptSnapshot({
    recording_id: "recording-a",
    high_watermark: 4,
    utterances: [nativeUtterance(2, "ฉบับล่าสุด")],
  }, "project-a").utterances[0];
  const base = snapshot(4, [existing]);
  const duplicate = applyTranscriptEvent(base, {
    type: "revision", projectId: "project-a", recordingId: "recording-a", cursor: 4,
    utterances: [existing],
  });
  assert.equal(duplicate.accepted, false);
  assert.equal(duplicate.snapshot, base);

  const wrongRecording = applyTranscriptEvent(base, {
    type: "control", projectId: "project-a", recordingId: "recording-b", cursor: 5,
    eventKind: "ignored",
  });
  assert.equal(wrongRecording.accepted, false);
  assert.equal(wrongRecording.snapshot.cursor, 4);
});

test("transcript reducer replaces only with a newer revision and flags cursor gaps", () => {
  const current = normalizeTranscriptSnapshot({
    recording_id: "recording-a",
    high_watermark: 4,
    utterances: [nativeUtterance(1, "ฉบับแรก")],
  }, "project-a").utterances[0];
  const base = snapshot(4, [current]);
  const newer = normalizeTranscriptSnapshot({
    recording_id: "recording-a",
    high_watermark: 5,
    utterances: [nativeUtterance(2, "ฉบับแก้ไข")],
  }, "project-a").utterances[0];
  const applied = applyTranscriptEvent(base, {
    type: "revision", projectId: "project-a", recordingId: "recording-a", cursor: 5,
    utterances: [newer],
  });
  assert.equal(applied.accepted, true);
  assert.equal(applied.snapshot.utterances.length, 1);
  assert.equal(applied.snapshot.utterances[0].text, "ฉบับแก้ไข");

  const missed = applyTranscriptEvent(applied.snapshot, {
    type: "control", projectId: "project-a", recordingId: "recording-a", cursor: 7,
    eventKind: "batch_processed",
  });
  assert.equal(missed.needsReplay, true);
  assert.equal(missed.snapshot.cursor, 5);
});

test("replay normalizes persisted revision payload and coverage gaps", () => {
  const page = normalizeReplayPage({
    recordingId: "recording-a",
    highWatermark: 7,
    nextCursor: 7,
    hasMore: false,
    events: [
      {
        id: "event-a",
        cursor: 6,
        event_type: "transcript_revision",
        payload: { scope: nativeScope, revision: nativeUtterance(2, "revision"), coverage: [] },
        payload_hash: "hash-a",
        transaction_id: "tx-a",
        committed_at: "2026-09-24T00:00:00Z",
      },
      {
        id: "event-b",
        cursor: 7,
        event_type: "source_gap",
        payload: {
          scope: nativeScope,
          coverage: [{ id: "gap-a", kind: "gap", start_ms: 2400, end_ms: 3400, gap_reason: "overrun" }],
        },
        payload_hash: "hash-b",
        transaction_id: "tx-b",
        committed_at: "2026-09-24T00:00:01Z",
      },
    ],
  });

  assert.equal(page.events.length, 2);
  assert.equal(page.events[0].type, "revision");
  assert.equal(page.events[0].utterances[0].text, "revision");
  assert.equal(page.events[1].type, "gap");
  assert.equal(page.events[1].gaps[0].reason, "overrun");
});

test("Tauri bridge uses the registered transcript command argument contract", async () => {
  const calls = [];
  const port = {
    invoke: async (command, args) => {
      calls.push({ command, args });
      if (command === "meeting_transcript_snapshot") {
        return { recording_id: "recording-a", high_watermark: 0, utterances: [] };
      }
      if (command === "replay_meeting_events") {
        return { recordingId: "recording-a", highWatermark: 0, nextCursor: 0, hasMore: false, events: [] };
      }
      return { utteranceId: "utterance-a", revision: 2, cursor: 1, committed: true };
    },
    listen: async () => () => undefined,
  };
  const service = createMeetingIntelligenceService(port);
  await service.snapshot("recording-a", "project-a");
  await service.replay({ projectId: "project-a", recordingId: "recording-a", afterCursor: 3 }, 40);
  await service.correct({
    scope: nativeScope,
    revision: {
      id: "revision-b",
      utterance_id: "utterance-a",
      revision: 2,
      supersedes_revision: 1,
      expected_revision: 1,
      origin: "human",
      raw_text: "แก้แล้ว",
      effective_text: "แก้แล้ว",
      language: "th",
      confidence: null,
      start_ms: 1000,
      end_ms: 2400,
      model_run_id: null,
      review_state: "reviewed",
    },
  });

  assert.deepEqual(calls.map(({ command }) => command), [
    "meeting_transcript_snapshot",
    "replay_meeting_events",
    "correct_meeting_utterance",
  ]);
  assert.deepEqual(calls[0].args, { projectId: "project-a", recordingId: "recording-a" });
  assert.deepEqual(calls[1].args, {
    projectId: "project-a",
    cursor: { recording_id: "recording-a", after_cursor: 3 },
    limit: 40,
  });
  assert.equal(calls[2].args.request.revision.expected_revision, 1);
});

test("local vault and knowledge actions carry explicit project and recording scope", async () => {
  const calls = [];
  const imported = {
    collectionId: "collection-a",
    documentId: "document-a",
    versionId: "version-a",
    chunkCount: 3,
    contentSha256: "sha256-a",
    parserVersion: "parser-1",
    warnings: [],
    citationLocators: [{ chunkId: "chunk-a", label: "บรรทัด 1–2 · อักขระ 0–30" }],
  };
  const port = {
    invoke: async (command, args) => {
      calls.push({ command, args });
      if (command === "meeting_local_owner_vault_options") return [{ vaultId: "vault-a", keyAvailable: true, accountBound: true }];
      if (command === "meeting_local_owner_provision") return "vault-b";
      if (command === "meeting_knowledge_collection_create") return { collectionId: "collection-a" };
      if (command === "meeting_knowledge_import_selected") return imported;
      if (command === "meeting_knowledge_collections_list") return [];
      return undefined;
    },
    listen: async () => () => undefined,
  };
  const service = createMeetingIntelligenceService(port);

  assert.deepEqual(await service.vaultOptions(), [{ vaultId: "vault-a", keyAvailable: true, accountBound: true }]);
  assert.equal(await service.provisionVault(), "vault-b");
  await service.unlockVault("vault-a");
  await service.lockVault();
  assert.deepEqual(await service.createCollection("project-a", "confidential"), { collectionId: "collection-a" });
  assert.deepEqual(await service.importSelectedDocument("project-a", "collection-a"), imported);
  const metricRequest = {
    projectId: "project-a",
    recordingId: "recording-a",
    requestId: "metric-request-a",
    collectionIds: ["collection-a"],
    documentVersionId: "version-a",
    chunkId: "chunk-a",
    metricKey: "revenue",
    organizationRef: "org-a",
    periodStart: "2026-01-01",
    periodEnd: "2026-12-31",
    calendar: "gregorian",
    unit: "money",
    currency: "USD",
    scale: "raw",
    basis: "actual",
    value: "125.50",
  };
  await service.saveMetric(metricRequest);
  await service.computeMetric({
    projectId: "project-a",
    recordingId: "recording-a",
    collectionIds: ["collection-a"],
    metricKey: "revenue",
    organizationRef: "org-a",
    periodStart: "2026-01-01",
    periodEnd: "2026-12-31",
    calendar: "gregorian",
    unit: "money",
    currency: "USD",
    scale: "raw",
  });
  await service.collections("project-a", "recording-a");

  assert.deepEqual(calls, [
    { command: "meeting_local_owner_vault_options", args: {} },
    { command: "meeting_local_owner_provision", args: {} },
    { command: "meeting_local_owner_unlock", args: { vaultId: "vault-a" } },
    { command: "meeting_local_owner_lock", args: {} },
    { command: "meeting_knowledge_collection_create", args: { projectId: "project-a", classification: "confidential" } },
    { command: "meeting_knowledge_import_selected", args: { projectId: "project-a", collectionId: "collection-a" } },
    { command: "meeting_knowledge_metric_save", args: { request: metricRequest } },
    { command: "meeting_knowledge_metric_compute", args: { request: {
      projectId: "project-a",
      recordingId: "recording-a",
      collectionIds: ["collection-a"],
      metricKey: "revenue",
      organizationRef: "org-a",
      periodStart: "2026-01-01",
      periodEnd: "2026-12-31",
      calendar: "gregorian",
      unit: "money",
      currency: "USD",
      scale: "raw",
    } } },
    { command: "meeting_knowledge_collections_list", args: { projectId: "project-a", recordingId: "recording-a" } },
  ]);
});

test("external meeting dispatch remains unavailable in the UI contract", () => {
  assert.equal(MEETING_INTELLIGENCE_EXTERNAL_DISPATCH_AVAILABLE, false);
});

test("People review commands preserve native meeting scope and expected revisions", async () => {
  const calls = [];
  const service = createMeetingIntelligenceService({
    invoke: async (command, args) => {
      calls.push({ command, args });
      return undefined;
    },
    listen: async () => () => undefined,
  });
  const proposal = {
    scope: nativeScope,
    speakerId: "speaker-a",
    profileId: "profile-a",
    expectedEvidenceRevision: 2,
  };
  const mutation = { scope: nativeScope, linkId: "link-a", expectedRevision: 3 };

  await service.people(nativeScope);
  await service.createPersonProfile("project-a", "Alice");
  await service.updatePersonProfile("project-a", "profile-a", 4, "Alice A");
  await service.archivePersonProfile("profile-a", 5);
  await service.proposeSpeakerIdentity(proposal);
  await service.confirmSpeakerIdentity(mutation);
  await service.rejectSpeakerIdentity(mutation);
  await service.unlinkSpeakerIdentity(mutation);

  assert.deepEqual(calls, [
    { command: "meeting_people_list", args: { request: { scope: nativeScope } } },
    { command: "meeting_people_profile_create", args: { request: { projectId: "project-a", displayName: "Alice" } } },
    { command: "meeting_people_profile_update", args: { request: { projectId: "project-a", profileId: "profile-a", expectedRevision: 4, displayName: "Alice A" } } },
    { command: "meeting_people_profile_archive", args: { request: { profileId: "profile-a", expectedRevision: 5 } } },
    { command: "meeting_people_link_propose", args: { request: proposal } },
    { command: "meeting_people_link_confirm", args: { request: mutation } },
    { command: "meeting_people_link_reject", args: { request: mutation } },
    { command: "meeting_people_link_unlink", args: { request: mutation } },
  ]);
});

test("meeting subscription forwards account lifecycle changes to clear private panel state", async () => {
  const listeners = new Map();
  const disposedEvents = [];
  const lifecycleEvents = [];
  const service = createMeetingIntelligenceService({
    invoke: async () => undefined,
    listen: async (eventName, handler) => {
      listeners.set(eventName, handler);
      return () => disposedEvents.push(eventName);
    },
  });
  const dispose = await service.subscribe(
    { projectId: "project-a", recordingId: "recording-a" },
    {
      onTranscript: () => undefined,
      onAgentStatus: () => undefined,
      onDraft: () => undefined,
      onPolicyBlocked: () => undefined,
      onDelivery: () => undefined,
      onAccountLifecycleChanged: (event) => lifecycleEvents.push(event),
    },
  );

  listeners.get("auth-session-changed")({ state: "signed_out", accountGeneration: 7 });

  assert.deepEqual(lifecycleEvents, [{ state: "signed_out", accountGeneration: 7 }]);
  assert.equal(listeners.has("auth-session-changed"), true);
  dispose();
  assert.equal(disposedEvents.includes("auth-session-changed"), true);
});
