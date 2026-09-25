/**
 * Typed local meeting-intelligence boundary for the Desktop UI.
 *
 * The command port is injected by the Tauri integration owner so this module
 * remains importable in Node contract tests. Provider operations are absent
 * from this interface by design; external meeting delivery is unavailable.
 */

export type MeetingScope = {
  projectId: string;
  recordingId: string;
};

export type NativeMeetingScope = {
  project_id: string;
  recording_id: string;
  meeting_session_id: string;
  source_session_id: string;
  track_id: string;
  source_generation: number;
};

export type TranscriptUtterance = {
  utteranceId: string;
  revisionId: string;
  revision: number;
  startMs: number;
  endMs: number;
  rawText: string;
  text: string;
  state: "provisional" | "committed" | "reviewed" | "retracted";
  origin: "local_asr" | "provider_asr" | "human" | "refinement";
  language: string | null;
  confidence: number | null;
  modelRunId: string | null;
  scope: NativeMeetingScope;
  speakerLabel: string | null;
  speakerId: string | null;
  actorKind: "human" | "self" | "agent" | "unknown";
};

export type NativeTranscriptUtterance = {
  revision_id?: string;
  id?: string;
  utterance_id: string;
  revision: number;
  supersedes_revision: number | null;
  expected_revision: number | null;
  origin: "local_asr" | "provider_asr" | "human" | "refinement";
  raw_text: string;
  effective_text: string;
  language: string | null;
  confidence: number | null;
  start_ms: number;
  end_ms: number;
  model_run_id: string | null;
  review_state: string;
  scope: NativeMeetingScope;
  speaker_label?: string | null;
  speaker_id?: string | null;
  actor_kind?: "human" | "self" | "agent" | "unknown";
};

export type NativeTranscriptSnapshot = {
  recording_id?: string;
  recordingId?: string;
  high_watermark?: number;
  highWatermark?: number;
  utterances: NativeTranscriptUtterance[];
  coverage?: Array<{
    id: string;
    kind: "audio" | "gap";
    start_ms: number;
    end_ms: number;
    gap_reason?: string | null;
  }>;
  legacy_snapshot?: boolean;
  legacySnapshot?: boolean;
};

export type TranscriptGap = {
  id: string;
  startMs: number;
  endMs: number;
  reason: string;
};

export type TranscriptSnapshot = MeetingScope & {
  cursor: number;
  revision: number;
  utterances: TranscriptUtterance[];
  gaps: TranscriptGap[];
  legacySnapshot: boolean;
};

export type TranscriptEvent =
  | {
      type: "revision";
      projectId: string;
      recordingId: string;
      cursor: number;
      utterances: TranscriptUtterance[];
    }
  | {
      type: "gap";
      projectId: string;
      recordingId: string;
      cursor: number;
      gaps: TranscriptGap[];
    }
  | {
      type: "control";
      projectId: string;
      recordingId: string;
      cursor: number;
      eventKind: string;
    };

export type ReplayCursor = {
  projectId: string;
  recordingId: string;
  afterCursor: number;
};

export type NativeReplayCursor = {
  recording_id: string;
  after_cursor: number;
};

export type ReplayPage = {
  recordingId: string;
  highWatermark: number;
  nextCursor: number;
  hasMore: boolean;
  events: TranscriptEvent[];
};

export type NativeMeetingReplayRow = {
  id: string;
  cursor: number;
  event_type: string;
  payload: {
    event_type?: string;
    scope: NativeMeetingScope;
    source_cursor?: number;
    committed_cursor?: number;
    coverage?: Array<{
      id: string;
      kind: "audio" | "gap";
      start_ms: number;
      end_ms: number;
      gap_reason?: string | null;
    }>;
    revision?: NativeTranscriptUtterance | null;
    revisions?: NativeTranscriptUtterance[];
    attribution?: unknown;
  };
  payload_hash: string;
  transaction_id: string;
  committed_at: string;
};

export type NativeMeetingReplayPage = {
  recordingId: string;
  highWatermark: number;
  nextCursor: number;
  hasMore: boolean;
  events: NativeMeetingReplayRow[];
};

export type NativeMeetingRevisionRequest = {
  scope: NativeMeetingScope;
  revision: {
    id: string;
    utterance_id: string;
    revision: number;
    supersedes_revision: number;
    expected_revision: number;
    origin: "human";
    raw_text: string;
    effective_text: string;
    language: string | null;
    confidence: number | null;
    start_ms: number;
    end_ms: number;
    model_run_id: string | null;
    review_state: "reviewed";
  };
};

export type TranscriptCorrectionReceipt = {
  utteranceId: string;
  revision: number;
  cursor: number;
  committed: true;
};

export type KnowledgeCollection = {
  collectionId: string;
  label: string;
  readable: boolean;
  selected: boolean;
  classification: string;
};

export type OwnerVaultOption = {
  vaultId: string;
  keyAvailable: boolean;
  accountBound: boolean;
};

export type MeetingPeopleProfile = {
  profileId: string;
  displayName: string;
  revision: number;
};

export type MeetingPeopleSpeaker = {
  speakerId: string;
  displayLabel: string;
  evidenceRevision: number;
};

export type MeetingPeopleLink = {
  linkId: string;
  speakerId: string;
  profileId: string;
  displayName: string;
  status: "pending_review" | "confirmed" | "rejected" | "revoked" | "stale";
  revision: number;
  evidenceRevision: number;
};

export type MeetingPeopleSnapshot = {
  vaultId: string;
  profiles: MeetingPeopleProfile[];
  speakers: MeetingPeopleSpeaker[];
  links: MeetingPeopleLink[];
};

export type MeetingPeopleLinkProposalRequest = {
  scope: NativeMeetingScope;
  speakerId: string;
  profileId: string;
  expectedEvidenceRevision: number;
};

export type MeetingPeopleLinkMutationRequest = {
  scope: NativeMeetingScope;
  linkId: string;
  expectedRevision: number;
};

export type KnowledgeCollectionClassification = "internal" | "confidential";

export type KnowledgeImportReceipt = {
  collectionId: string;
  documentId: string;
  versionId: string;
  chunkCount: number;
  contentSha256: string;
  parserVersion: string;
  warnings: string[];
  citationLocators: Array<{ chunkId: string; label: string }>;
};

export type KnowledgeMetricBasis = "actual" | "budget";
export type KnowledgeMetricDecimal = { coefficient: string; scale: number };
export type KnowledgeMetricCitation = {
  collectionId: string;
  documentId: string;
  documentVersionId: string;
  documentVersionNumber: number;
  sourceVersion: string;
  contentSha256: string;
  locator: Record<string, unknown>;
  retrievedAt: string;
  readGrantId: string;
  aclRevision: number;
};
export type KnowledgeMetricSaveRequest = MeetingScope & {
  requestId: string;
  collectionIds: string[];
  documentVersionId: string;
  chunkId: string;
  metricKey: string;
  organizationRef: string;
  periodStart: string;
  periodEnd: string;
  calendar: string;
  unit: string;
  currency: string | null;
  scale: string;
  basis: KnowledgeMetricBasis;
  value: string;
};
export type KnowledgeMetricSaveReceipt = {
  observationId: string;
  basis: KnowledgeMetricBasis;
  value: KnowledgeMetricDecimal;
  citation: KnowledgeMetricCitation;
};
export type KnowledgeMetricComputeRequest = MeetingScope & {
  collectionIds: string[];
  metricKey: string;
  organizationRef: string;
  periodStart: string;
  periodEnd: string;
  calendar: string;
  unit: string;
  currency: string | null;
  scale: string;
};
export type KnowledgeMetricComputeResult = {
  metricKey: string;
  organizationRef: string;
  periodStart: string;
  periodEnd: string;
  percentage: KnowledgeMetricDecimal;
  citations: KnowledgeMetricCitation[];
};

export type CollectionSelectionRequest = MeetingScope & {
  collectionIds: string[];
  expectedRevision: number;
  requestId: string;
};

export type AgentMode = "off" | "observe" | "draft";
export type Readiness = "ready" | "blocked" | "unavailable";

export type AgentCapability = {
  readiness: Readiness;
  reasonCode: string | null;
};

export type MeetingAgentStatus = MeetingScope & {
  revision: number;
  lastObservedTranscriptCursor: number;
  mode: AgentMode;
  state: "stopped" | "starting" | "observing" | "drafting" | "paused" | "blocked";
  expiresAt: string | null;
  blockers: string[];
  allowedTopics?: string[];
  localAgent: AgentCapability;
  transcriptRead: AgentCapability;
  knowledgeRead: AgentCapability;
  automaticTrigger: AgentCapability;
  externalJoin: AgentCapability;
  externalMediaRead: AgentCapability;
  externalChatSend: AgentCapability;
  externalLinkSend: AgentCapability;
  externalFileUpload: AgentCapability;
};

export type MeetingPreflight = {
  status: MeetingAgentStatus;
  collectionRevision: number;
  selectedCollectionIds: string[];
  localLimits: {
    maxActiveRuns: number;
    maxRetrievalAttempts: number;
    triggerExpiryMs: number;
    maxRunsPerMinute: number;
    maxRunsPerHour: number;
  };
};

export type Citation = {
  documentId: string;
  versionId: string;
  locator: string;
  label: string;
};

export type PrivateDraft = {
  draftId: string;
  revision: number;
  text: string;
  citations: Citation[];
  basedOnTranscriptCursor: number;
  expiresAt: string;
  state: "private" | "stale" | "blocked";
};

export type LocalDeliveryPreview = {
  intentId: string;
  payloadHash: string;
  destinationSummary: string;
  state: "awaiting_approval" | "approved_local_only" | "blocked" | "expired";
  approvalScope: "local_preview_only";
  externalDispatchAvailable: false;
};

export type DeliveryStatusEvent = MeetingScope & {
  intentId: string;
  state: LocalDeliveryPreview["state"] | "delivery_unknown" | "cancelled";
  revision: number;
  externalDispatchAvailable: false;
};

export type MeetingHistoryEntry = {
  id: string;
  kind: "mode_changed" | "draft_created" | "draft_blocked" | "preview_created" | "approval_recorded" | "revoked";
  state: string;
  createdAt: string;
  transcriptCursor: number | null;
  evidenceCount: number;
};

export type MeetingAgentEvent = MeetingScope & {
  revision: number;
  lastObservedTranscriptCursor: number;
  state: MeetingAgentStatus["state"];
  mode: AgentMode;
  blockers: string[];
  automaticTrigger?: AgentCapability;
};

export type MeetingDraftEvent = MeetingScope & {
  draft: PrivateDraft;
};

export type MeetingPolicyBlockedEvent = MeetingScope & {
  code: string;
  reason: string;
  sourceCursor: number | null;
};

export type MeetingDeliveryEvent = DeliveryStatusEvent;

export type AccountLifecycleEvent = {
  state: string;
  accountGeneration: number;
};

export type MeetingEventHandlers = {
  onTranscript: (event: TranscriptEvent) => void;
  onAgentStatus: (event: MeetingAgentEvent) => void;
  onDraft: (event: MeetingDraftEvent) => void;
  onPolicyBlocked: (event: MeetingPolicyBlockedEvent) => void;
  onDelivery: (event: MeetingDeliveryEvent) => void;
  onAccountLifecycleChanged?: (event: AccountLifecycleEvent) => void;
};

export type AgentMutationRequest = MeetingScope & {
  requestId: string;
  expectedRevision: number;
};

export type AgentStartRequest = AgentMutationRequest & {
  mode: "observe" | "draft";
};

export type AgentPolicyRequest = AgentMutationRequest & {
  mode: AgentMode;
  allowedTopics: string[];
  expiresAt: string | null;
};

export type AgentAskRequest = AgentMutationRequest & {
  question: string;
  collectionIds: string[];
  transcriptCursor: number;
};

export type DeliveryPreviewRequest = AgentMutationRequest & {
  draftId: string;
  draftRevision: number;
  scope: "local_preview_only";
};

export type DeliveryApprovalRequest = AgentMutationRequest & {
  intentId: string;
  approvedPayloadHash: string;
  scope: "local_preview_only";
};

export type MeetingNativeCommandArgs = {
  meeting_local_owner_vault_options: Record<string, never>;
  meeting_local_owner_provision: Record<string, never>;
  meeting_local_owner_unlock: { vaultId: string | null };
  meeting_local_owner_lock: Record<string, never>;
  meeting_knowledge_collection_create: { projectId: string | null; classification: KnowledgeCollectionClassification };
  meeting_knowledge_import_selected: { projectId: string; collectionId: string };
  meeting_knowledge_metric_save: { request: KnowledgeMetricSaveRequest };
  meeting_knowledge_metric_compute: { request: KnowledgeMetricComputeRequest };
  meeting_people_list: { request: { scope: NativeMeetingScope } };
  meeting_people_profile_create: { request: { projectId: string; displayName: string } };
  meeting_people_profile_update: { request: { projectId: string; profileId: string; expectedRevision: number; displayName: string } };
  meeting_people_profile_archive: { request: { profileId: string; expectedRevision: number } };
  meeting_people_link_propose: { request: MeetingPeopleLinkProposalRequest };
  meeting_people_link_confirm: { request: MeetingPeopleLinkMutationRequest };
  meeting_people_link_reject: { request: MeetingPeopleLinkMutationRequest };
  meeting_people_link_unlink: { request: MeetingPeopleLinkMutationRequest };
  meeting_transcript_snapshot: { projectId: string; recordingId: string };
  replay_meeting_events: { projectId: string; cursor: NativeReplayCursor; limit: number };
  correct_meeting_utterance: { request: NativeMeetingRevisionRequest };
  meeting_knowledge_collections_list: { projectId: string; recordingId: string };
  meeting_knowledge_set_selection: { request: CollectionSelectionRequest };
  meeting_agent_preflight: { selection: MeetingScope };
  meeting_agent_start: { request: AgentStartRequest };
  meeting_agent_pause: { request: AgentMutationRequest };
  meeting_agent_stop: { request: AgentMutationRequest };
  meeting_agent_set_policy: { request: AgentPolicyRequest };
  meeting_agent_ask: { request: AgentAskRequest };
  meeting_agent_preview_delivery: { request: DeliveryPreviewRequest };
  meeting_agent_approve_delivery: { request: DeliveryApprovalRequest };
  meeting_agent_revoke: { request: AgentMutationRequest & { reason: string } };
  meeting_agent_status: { selection: MeetingScope };
  meeting_agent_history: { selection: MeetingScope; limit: number };
};

export type MeetingNativeCommandResult = {
  meeting_local_owner_vault_options: OwnerVaultOption[];
  meeting_local_owner_provision: string;
  meeting_local_owner_unlock: void;
  meeting_local_owner_lock: void;
  meeting_knowledge_collection_create: { collectionId: string };
  meeting_knowledge_import_selected: KnowledgeImportReceipt;
  meeting_knowledge_metric_save: KnowledgeMetricSaveReceipt;
  meeting_knowledge_metric_compute: KnowledgeMetricComputeResult;
  meeting_people_list: MeetingPeopleSnapshot;
  meeting_people_profile_create: MeetingPeopleProfile;
  meeting_people_profile_update: MeetingPeopleProfile;
  meeting_people_profile_archive: void;
  meeting_people_link_propose: MeetingPeopleLink;
  meeting_people_link_confirm: void;
  meeting_people_link_reject: void;
  meeting_people_link_unlink: void;
  meeting_transcript_snapshot: NativeTranscriptSnapshot;
  replay_meeting_events: NativeMeetingReplayPage;
  correct_meeting_utterance: TranscriptCorrectionReceipt;
  meeting_knowledge_collections_list: KnowledgeCollection[];
  meeting_knowledge_set_selection: { revision: number; selectedCollectionIds: string[] };
  meeting_agent_preflight: MeetingPreflight;
  meeting_agent_start: MeetingAgentStatus;
  meeting_agent_pause: MeetingAgentStatus;
  meeting_agent_stop: MeetingAgentStatus;
  meeting_agent_set_policy: MeetingAgentStatus;
  meeting_agent_ask: PrivateDraft;
  meeting_agent_preview_delivery: LocalDeliveryPreview;
  meeting_agent_approve_delivery: LocalDeliveryPreview;
  meeting_agent_revoke: MeetingAgentStatus;
  meeting_agent_status: MeetingAgentStatus;
  meeting_agent_history: MeetingHistoryEntry[];
};

export type MeetingNativeEventPayload = {
  "meeting-transcript-event": NativeMeetingReplayRow;
  "meeting-agent-status": MeetingAgentEvent;
  "meeting-agent-draft": MeetingDraftEvent;
  "meeting-agent-policy-blocked": MeetingPolicyBlockedEvent;
  "meeting-delivery-status": MeetingDeliveryEvent;
  "auth-session-changed": AccountLifecycleEvent;
};

export type MeetingNativePort = {
  invoke: <K extends keyof MeetingNativeCommandArgs>(
    command: K,
    args: MeetingNativeCommandArgs[K],
  ) => Promise<MeetingNativeCommandResult[K]>;
  listen: <K extends keyof MeetingNativeEventPayload>(
    eventName: K,
    handler: (payload: MeetingNativeEventPayload[K]) => void,
  ) => Promise<() => void>;
};

export type MeetingIntelligenceService = {
  vaultOptions: () => Promise<OwnerVaultOption[]>;
  provisionVault: () => Promise<string>;
  unlockVault: (vaultId: string | null) => Promise<void>;
  lockVault: () => Promise<void>;
  createCollection: (projectId: string | null, classification: KnowledgeCollectionClassification) => Promise<{ collectionId: string }>;
  importSelectedDocument: (projectId: string, collectionId: string) => Promise<KnowledgeImportReceipt>;
  saveMetric: (request: KnowledgeMetricSaveRequest) => Promise<KnowledgeMetricSaveReceipt>;
  computeMetric: (request: KnowledgeMetricComputeRequest) => Promise<KnowledgeMetricComputeResult>;
  people: (scope: NativeMeetingScope) => Promise<MeetingPeopleSnapshot>;
  createPersonProfile: (projectId: string, displayName: string) => Promise<MeetingPeopleProfile>;
  updatePersonProfile: (projectId: string, profileId: string, expectedRevision: number, displayName: string) => Promise<MeetingPeopleProfile>;
  archivePersonProfile: (profileId: string, expectedRevision: number) => Promise<void>;
  proposeSpeakerIdentity: (request: MeetingPeopleLinkProposalRequest) => Promise<MeetingPeopleLink>;
  confirmSpeakerIdentity: (request: MeetingPeopleLinkMutationRequest) => Promise<void>;
  rejectSpeakerIdentity: (request: MeetingPeopleLinkMutationRequest) => Promise<void>;
  unlinkSpeakerIdentity: (request: MeetingPeopleLinkMutationRequest) => Promise<void>;
  snapshot: (recordingId: string, projectId: string) => Promise<TranscriptSnapshot>;
  replay: (cursor: ReplayCursor, limit: number) => Promise<ReplayPage>;
  correct: (request: NativeMeetingRevisionRequest) => Promise<TranscriptCorrectionReceipt>;
  collections: (projectId: string, recordingId: string) => Promise<KnowledgeCollection[]>;
  setCollections: (request: CollectionSelectionRequest) => Promise<{ revision: number; selectedCollectionIds: string[] }>;
  preflight: (selection: MeetingScope) => Promise<MeetingPreflight>;
  start: (request: AgentStartRequest) => Promise<MeetingAgentStatus>;
  pause: (request: AgentMutationRequest) => Promise<MeetingAgentStatus>;
  stop: (request: AgentMutationRequest) => Promise<MeetingAgentStatus>;
  setPolicy: (request: AgentPolicyRequest) => Promise<MeetingAgentStatus>;
  ask: (request: AgentAskRequest) => Promise<PrivateDraft>;
  previewDelivery: (request: DeliveryPreviewRequest) => Promise<LocalDeliveryPreview>;
  approveDelivery: (request: DeliveryApprovalRequest) => Promise<LocalDeliveryPreview>;
  revoke: (request: AgentMutationRequest & { reason: string }) => Promise<MeetingAgentStatus>;
  status: (selection: MeetingScope) => Promise<MeetingAgentStatus>;
  history: (selection: MeetingScope, limit: number) => Promise<MeetingHistoryEntry[]>;
  subscribe: (selection: MeetingScope, handlers: MeetingEventHandlers) => Promise<() => void>;
};

export function canUseLocalDrafting(
  vaultUnlockedInPanel: boolean,
  timelineIncomplete: boolean,
  status: MeetingAgentStatus | null,
): boolean {
  if (!vaultUnlockedInPanel || timelineIncomplete || !status) return false;
  return status.localAgent.readiness === "ready"
    && status.transcriptRead.readiness === "ready"
    && status.knowledgeRead.readiness === "ready";
}

export const MEETING_INTELLIGENCE_EXTERNAL_DISPATCH_AVAILABLE = false as const;
export const MEETING_EVENT_BUFFER_LIMIT = 200;
export const MEETING_REPLAY_PAGE_SIZE = 100;

export function createMeetingIntelligenceService(port: MeetingNativePort): MeetingIntelligenceService {
  return {
    vaultOptions: () => port.invoke("meeting_local_owner_vault_options", {}),
    provisionVault: () => port.invoke("meeting_local_owner_provision", {}),
    unlockVault: (vaultId) => port.invoke("meeting_local_owner_unlock", { vaultId }),
    lockVault: () => port.invoke("meeting_local_owner_lock", {}),
    createCollection: (projectId, classification) => port.invoke("meeting_knowledge_collection_create", { projectId, classification }),
    importSelectedDocument: (projectId, collectionId) => port.invoke("meeting_knowledge_import_selected", { projectId, collectionId }),
    saveMetric: (request) => port.invoke("meeting_knowledge_metric_save", { request }),
    computeMetric: (request) => port.invoke("meeting_knowledge_metric_compute", { request }),
    people: (scope) => port.invoke("meeting_people_list", { request: { scope } }),
    createPersonProfile: (projectId, displayName) => port.invoke("meeting_people_profile_create", { request: { projectId, displayName } }),
    updatePersonProfile: (projectId, profileId, expectedRevision, displayName) => port.invoke("meeting_people_profile_update", { request: { projectId, profileId, expectedRevision, displayName } }),
    archivePersonProfile: (profileId, expectedRevision) => port.invoke("meeting_people_profile_archive", { request: { profileId, expectedRevision } }),
    proposeSpeakerIdentity: (request) => port.invoke("meeting_people_link_propose", { request }),
    confirmSpeakerIdentity: (request) => port.invoke("meeting_people_link_confirm", { request }),
    rejectSpeakerIdentity: (request) => port.invoke("meeting_people_link_reject", { request }),
    unlinkSpeakerIdentity: (request) => port.invoke("meeting_people_link_unlink", { request }),
    snapshot: async (recordingId, projectId) => normalizeTranscriptSnapshot(
      await port.invoke("meeting_transcript_snapshot", { projectId, recordingId }),
      projectId,
    ),
    replay: async (cursor, limit) => normalizeReplayPage(await port.invoke("replay_meeting_events", {
      projectId: cursor.projectId,
      cursor: { recording_id: cursor.recordingId, after_cursor: cursor.afterCursor },
      limit,
    })),
    correct: (request) => port.invoke("correct_meeting_utterance", { request }),
    collections: (projectId, recordingId) => port.invoke("meeting_knowledge_collections_list", { projectId, recordingId }),
    setCollections: (request) => port.invoke("meeting_knowledge_set_selection", { request }),
    preflight: (selection) => port.invoke("meeting_agent_preflight", { selection }),
    start: (request) => port.invoke("meeting_agent_start", { request }),
    pause: (request) => port.invoke("meeting_agent_pause", { request }),
    stop: (request) => port.invoke("meeting_agent_stop", { request }),
    setPolicy: (request) => port.invoke("meeting_agent_set_policy", { request }),
    ask: (request) => port.invoke("meeting_agent_ask", { request }),
    previewDelivery: (request) => port.invoke("meeting_agent_preview_delivery", { request }),
    approveDelivery: (request) => port.invoke("meeting_agent_approve_delivery", { request }),
    revoke: (request) => port.invoke("meeting_agent_revoke", { request }),
    status: (selection) => port.invoke("meeting_agent_status", { selection }),
    history: (selection, limit) => port.invoke("meeting_agent_history", { selection, limit }),
    subscribe: async (selection, handlers) => {
      const disposers: Array<() => void> = [];
      try {
        disposers.push(await port.listen("auth-session-changed", (event) => {
          handlers.onAccountLifecycleChanged?.(event);
        }));
        disposers.push(await port.listen("meeting-transcript-event", (event) => {
          const normalized = normalizeTranscriptRow(event);
          if (normalized && isSameScope(selection, normalized)) handlers.onTranscript(normalized);
          }));
        disposers.push(await port.listen("meeting-agent-status", (event) => {
            if (isSameScope(selection, event)) handlers.onAgentStatus(event);
          }));
        disposers.push(await port.listen("meeting-agent-draft", (event) => {
            if (isSameScope(selection, event)) handlers.onDraft(event);
          }));
        disposers.push(await port.listen("meeting-agent-policy-blocked", (event) => {
            if (isSameScope(selection, event)) handlers.onPolicyBlocked(event);
          }));
        disposers.push(await port.listen("meeting-delivery-status", (event) => {
            if (isSameScope(selection, event)) handlers.onDelivery(event);
          }));
      } catch (error) {
        for (const dispose of disposers) dispose();
        throw error;
      }

      let disposed = false;
      return () => {
        if (disposed) return;
        disposed = true;
        for (const dispose of disposers) dispose();
      };
    },
  };
}

export function normalizeTranscriptSnapshot(native: NativeTranscriptSnapshot, fallbackProjectId = ""): TranscriptSnapshot {
  const utterances = native.utterances.map(normalizeUtterance);
  return {
    projectId: utterances[0]?.scope.project_id ?? fallbackProjectId,
    recordingId: native.recordingId ?? native.recording_id ?? "",
    cursor: native.highWatermark ?? native.high_watermark ?? 0,
    revision: utterances.reduce((latest, row) => Math.max(latest, row.revision), 0),
    utterances: utterances.sort((left, right) => left.startMs - right.startMs || left.utteranceId.localeCompare(right.utteranceId)),
    gaps: (native.coverage ?? [])
      .filter((row) => row.kind === "gap")
      .map((row) => ({ id: row.id, startMs: row.start_ms, endMs: row.end_ms, reason: row.gap_reason ?? "source_gap" })),
    legacySnapshot: native.legacySnapshot ?? native.legacy_snapshot ?? false,
  };
}

function normalizeUtterance(row: NativeTranscriptUtterance): TranscriptUtterance {
  return {
    utteranceId: row.utterance_id,
    revisionId: row.revision_id ?? row.id ?? "",
    revision: row.revision,
    startMs: row.start_ms,
    endMs: row.end_ms,
    rawText: row.raw_text,
    text: row.effective_text,
    state: row.review_state === "reviewed" ? "reviewed" : row.review_state === "retracted" ? "retracted" : "committed",
    origin: row.origin,
    language: row.language,
    confidence: row.confidence,
    modelRunId: row.model_run_id,
    scope: row.scope,
    speakerLabel: row.speaker_label ?? null,
    speakerId: row.speaker_id ?? null,
    actorKind: row.actor_kind ?? "unknown",
  };
}

export function normalizeReplayPage(native: NativeMeetingReplayPage): ReplayPage {
  const events = native.events
    .map(normalizeTranscriptRow)
    .filter((event): event is TranscriptEvent => event !== null);
  return {
    recordingId: native.recordingId,
    highWatermark: native.highWatermark,
    nextCursor: native.nextCursor,
    hasMore: native.hasMore,
    events,
  };
}

export function normalizeTranscriptRow(row: NativeMeetingReplayRow | null | undefined): TranscriptEvent | null {
  if (!row || !Number.isSafeInteger(row.cursor) || !row.payload) return null;
  const scope = row.payload.scope;
  if (!scope?.project_id || !scope.recording_id) return null;
  const projectId = scope.project_id;
  const recordingId = scope.recording_id;
  const revisions = [
    ...(row.payload.revision ? [row.payload.revision] : []),
    ...(row.payload.revisions ?? []),
  ];
  if (revisions.length) {
    return {
      type: "revision",
      projectId,
      recordingId,
      cursor: row.cursor,
      utterances: revisions.map((revision) => normalizeUtterance({ ...revision, scope })),
    };
  }
  const gaps = (row.payload.coverage ?? [])
    .filter((coverage) => coverage.kind === "gap")
    .map((coverage) => ({
      id: coverage.id,
      startMs: coverage.start_ms,
      endMs: coverage.end_ms,
      reason: coverage.gap_reason ?? "source_gap",
    }));
  if (gaps.length) return { type: "gap", projectId, recordingId, cursor: row.cursor, gaps };
  return {
    type: "control",
    projectId,
    recordingId,
    cursor: row.cursor,
    eventKind: row.payload.event_type ?? row.event_type,
  };
}

export function buildHumanCorrectionRequest(
  utterance: TranscriptUtterance,
  correctedText: string,
): NativeMeetingRevisionRequest {
  const text = correctedText.trim();
  if (!text) throw new Error("Correction text is required");
  return {
    scope: utterance.scope,
    revision: {
      id: createRequestId("transcript-revision"),
      utterance_id: utterance.utteranceId,
      revision: utterance.revision + 1,
      supersedes_revision: utterance.revision,
      expected_revision: utterance.revision,
      origin: "human",
      raw_text: text,
      effective_text: text,
      language: utterance.language,
      confidence: null,
      start_ms: utterance.startMs,
      end_ms: utterance.endMs,
      model_run_id: null,
      review_state: "reviewed",
    },
  };
}

function isSameScope(left: MeetingScope, right: MeetingScope): boolean {
  return left.projectId === right.projectId && left.recordingId === right.recordingId;
}

export type AppliedTranscriptEvent = {
  snapshot: TranscriptSnapshot;
  accepted: boolean;
  needsReplay: boolean;
};

/** Apply exactly the next recording-wide event; duplicates are harmless. */
export function applyTranscriptEvent(
  snapshot: TranscriptSnapshot,
  event: TranscriptEvent,
): AppliedTranscriptEvent {
  if (!isSameScope(snapshot, event) || event.cursor <= snapshot.cursor) {
    return { snapshot, accepted: false, needsReplay: false };
  }
  if (event.cursor !== snapshot.cursor + 1) {
    return { snapshot, accepted: false, needsReplay: true };
  }

  if (event.type === "gap") {
    return {
      snapshot: { ...snapshot, cursor: event.cursor, gaps: [...snapshot.gaps, ...event.gaps] },
      accepted: true,
      needsReplay: false,
    };
  }

  if (event.type === "control") {
    return { snapshot: { ...snapshot, cursor: event.cursor }, accepted: true, needsReplay: false };
  }

  let utterances = snapshot.utterances;
  for (const incoming of event.utterances) {
    const current = utterances.find((row) => row.utteranceId === incoming.utteranceId);
    if (current && incoming.revision < current.revision) continue;
    utterances = utterances.filter((row) => row.utteranceId !== incoming.utteranceId);
    utterances.push(incoming);
  }
  utterances.sort((left, right) => left.startMs - right.startMs || left.utteranceId.localeCompare(right.utteranceId));
  return {
    snapshot: { ...snapshot, cursor: event.cursor, utterances },
    accepted: true,
    needsReplay: false,
  };
}

export function mergeReplayEvents(
  snapshot: TranscriptSnapshot,
  events: readonly TranscriptEvent[],
): AppliedTranscriptEvent {
  let current = snapshot;
  let accepted = false;
  for (const event of [...events].sort((left, right) => left.cursor - right.cursor)) {
    const next = applyTranscriptEvent(current, event);
    current = next.snapshot;
    accepted ||= next.accepted;
    if (next.needsReplay) return { snapshot: current, accepted, needsReplay: true };
  }
  return { snapshot: current, accepted, needsReplay: false };
}

export function createRequestId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function isAgentRevisionCurrent(currentRevision: number, incomingRevision: number): boolean {
  return incomingRevision >= currentRevision;
}
