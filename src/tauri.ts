// @req FR-106, FR-108, FR-116
// @tested tests/externalMeetingTools.test.mjs
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  createMeetingIntelligenceService,
  type MeetingNativeCommandArgs,
  type MeetingNativeCommandResult,
  type MeetingNativeEventPayload,
  type MeetingNativePort,
} from "./lib/meetingIntelligence.ts";
import {
  EMPTY_MEETING_SUMMARIES as EMPTY_SUMMARIES,
  type MeetingSummaries as MeetingSummariesShape,
} from "./lib/meetingSummaries.ts";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ConnectorCapability,
  ExternalConnectorDisconnectReceipt,
  ExternalConnectorRegisterInput,
  ExternalConnectorSummary,
  ExternalToolRun,
  MeetingToolCancelReceipt,
  MeetingToolExecutionEnvelope,
  MeetingToolPreviewEnvelope,
  MeetingToolRevokeReceipt,
} from "./lib/externalMeetingTools.ts";
import { isPlaybackSourceCatalog } from "./components/desktop/contracts.ts";
import type {
  CaptureRoutePlan,
  PlaybackAction,
  PlaybackChannel,
  PlaybackCloseAcknowledgement,
  PlaybackState,
  PlaybackSource,
  NativeCaptureInventory,
  RoutedCaptureStatus,
  RecordingAnswer,
  RecordingListRelease,
  RecordingPage,
  RecordingRow,
  ReviewError,
  ValidatedRoutePlan,
} from "./components/desktop/contracts.ts";

export type {
  CaptureRoutePlan,
  PlaybackAction,
  PlaybackChannel,
  PlaybackCloseAcknowledgement,
  PlaybackState,
  PlaybackSource,
  NativeCaptureInventory,
  RoutedCaptureStatus,
  RecordingAnswer,
  RecordingListRelease,
  RecordingPage,
  RecordingRow,
  ReviewError,
  ValidatedRoutePlan,
} from "./components/desktop/contracts.ts";

export type Health = {
  app: string;
  version: string;
  databasePath: string;
  sqliteWal: boolean;
  genesisPath: string;
  genesisStableFrontier: number;
  storageAuthority: string;
  localApi: {
    running: boolean;
    bind: string | null;
    /** Set only while opt-in LAN sharing is on. */
    lanBind: string | null;
  };
  /** Jobs waiting or retrying in the engine's queue. */
  pendingJobs: number;
};

export type Project = {
  id: string;
  name: string;
  storagePath: string;
  activeRecordingId: string | null;
  createdAt: string;
  updatedAt: string;
};

export type DesktopMeetingActionItem = {
  id: string;
  title: string;
  projectId: string;
  projectName: string;
  recordingId: string;
  meetingTitle: string;
  meetingCreatedAt: string;
  owner: string | null;
  confidence: number | null;
  evidenceCount: number;
  priority: "urgent" | "deadline" | "unspecified";
};

export type DesktopMeetingActionItemSnapshot = {
  items: DesktopMeetingActionItem[];
  totalCount: number;
};

export type Job = {
  id: string;
  projectId: string;
  type: string;
  status: string;
  progress: number;
  inputRefs: string[];
  outputRefs: string[];
  providerId: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  startedAt: string | null;
  finishedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type DetailedTranscriptionReadiness = {
  available: boolean;
  reason: string;
  accuracyQualified: boolean;
};

export type DetailedTranscriptProposal = {
  id: string;
  transcriptSegmentId: string;
  originalText: string;
  proposedText: string;
  modelName: string;
  createdAt: string;
};

export type ModelProvider = {
  id: string;
  label: string;
  runtimeLocation: string;
  kind: string;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
};

export type TranscriptSegment = {
  id: string;
  projectId: string;
  recordingId: string;
  speakerId: string | null;
  speakerName: string | null;
  startMs: number;
  endMs: number;
  text: string;
  confidence: number | null;
  createdAt: string;
};

const fallbackHealth: Health = {
  app: "FUNG",
  version: "0.1.1",
  databasePath: "browser-preview",
  sqliteWal: true,
  genesisPath: "browser-preview",
  genesisStableFrontier: 0,
  storageAuthority: "browser preview",
  pendingJobs: 0,
  localApi: {
    running: false,
    bind: null,
    lanBind: null,
  },
};

const canInvoke = () => typeof window !== "undefined" && Boolean("__TAURI_INTERNALS__" in window);

/** Raw command bridge for panels that own their own error handling, or `null`
 * when this surface is a plain browser and no command can run. */
export const nativeInvoke = canInvoke()
  ? (<T,>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args))
  : null;

export function createTauriMeetingIntelligenceService() {
  const invokeNative = nativeInvoke;
  if (!invokeNative) return null;
  const port: MeetingNativePort = {
    invoke: async function invokeMeetingCommand<K extends keyof MeetingNativeCommandArgs>(
      command: K,
      args: MeetingNativeCommandArgs[K],
    ): Promise<MeetingNativeCommandResult[K]> {
      return invokeNative<MeetingNativeCommandResult[K]>(
        command,
        args as unknown as Record<string, unknown>,
      );
    },
    listen: async function listenMeetingEvent<K extends keyof MeetingNativeEventPayload>(
      eventName: K,
      handler: (payload: MeetingNativeEventPayload[K]) => void,
    ): Promise<() => void> {
      return listen<MeetingNativeEventPayload[K]>(eventName, (event) => handler(event.payload));
    },
  };
  return createMeetingIntelligenceService(port);
}

function nativeUnavailable(operation: string): ReviewError {
  return {
    code: "NATIVE_UNAVAILABLE",
    message: operation + " requires the desktop app",
    retryable: false,
  };
}

export async function getHealth(): Promise<Health> {
  if (!canInvoke()) return fallbackHealth;
  return invoke<Health>("app_health");
}

/** What `start_local_api` returns: where the desktop's loopback API listens
 * and the per-launch connect URL (`http://127.0.0.1:PORT/#TOKEN`) a user
 * pastes into the web dashboard on this same machine. */
export type LocalApiInfo = {
  bind: string;
  token: string;
  connectUrl: string;
  /** Opt-in LAN sharing (`set_local_api_lan`): a phone on the same Wi-Fi
   * opens `lanUrl` (QR) to get the desktop-served recordings page. */
  lanEnabled: boolean;
  lanBind: string | null;
  /** Null while LAN sharing is off, or when no LAN IPv4 could be found. */
  lanUrl: string | null;
};

export type ProductMcpStatus = {
  running: boolean;
  bind: string;
  port: number;
  endpoint: string;
};

export type ProductMcpGrant = {
  id: string;
  clientName: string;
  scopes: string[];
  projectScope: string[];
  allProjects: boolean;
  active: boolean;
  createdAt: string;
  expiresAt: string | null;
};

export type ProductMcpGrantInput = {
  clientName: string;
  scopes: string[];
  projectScope: string[];
  allProjects: boolean;
};

export type ProductMcpCreatedGrant = {
  grant: ProductMcpGrant;
  token: string;
};

export async function productMcpStatus(): Promise<ProductMcpStatus> {
  if (!canInvoke()) return { running: false, bind: "127.0.0.1", port: 49175, endpoint: "http://127.0.0.1:49175/mcp" };
  return invoke<ProductMcpStatus>("product_mcp_status");
}

export async function productMcpStart(port: number): Promise<ProductMcpStatus> {
  if (!canInvoke()) throw new Error("Product MCP requires the FUNG Desktop app");
  return invoke<ProductMcpStatus>("product_mcp_start", { port });
}

export async function productMcpStop(): Promise<ProductMcpStatus> {
  if (!canInvoke()) throw new Error("Product MCP requires the FUNG Desktop app");
  return invoke<ProductMcpStatus>("product_mcp_stop");
}

export async function productMcpGrantsList(): Promise<ProductMcpGrant[]> {
  if (!canInvoke()) return [];
  return invoke<ProductMcpGrant[]>("product_mcp_grants_list");
}

export async function productMcpGrantCreate(input: ProductMcpGrantInput): Promise<ProductMcpCreatedGrant> {
  if (!canInvoke()) throw new Error("Product MCP requires the FUNG Desktop app");
  return invoke<ProductMcpCreatedGrant>("product_mcp_grant_create", { input });
}

export async function productMcpGrantRevoke(grantId: string): Promise<void> {
  if (!canInvoke()) throw new Error("Product MCP requires the FUNG Desktop app");
  return invoke<void>("product_mcp_grant_revoke", { grantId });
}

const previewLocalApi: LocalApiInfo = {
  bind: "browser-preview",
  token: "",
  connectUrl: "",
  lanEnabled: false,
  lanBind: null,
  lanUrl: null,
};

export async function startLocalApi(): Promise<LocalApiInfo> {
  if (!canInvoke()) return previewLocalApi;
  return invoke<LocalApiInfo>("start_local_api");
}

export async function setLocalApiLan(enabled: boolean): Promise<LocalApiInfo> {
  if (!canInvoke()) return previewLocalApi;
  return invoke<LocalApiInfo>("set_local_api_lan", { enabled });
}

export async function listProjects(): Promise<Project[]> {
  if (!canInvoke()) return [];
  return invoke<Project[]>("list_projects");
}

export async function listMeetingActionItems(): Promise<DesktopMeetingActionItemSnapshot> {
  if (!canInvoke()) return { items: [], totalCount: 0 };
  return invoke<DesktopMeetingActionItemSnapshot>("list_meeting_action_items");
}

export async function createProject(name: string): Promise<Project> {
  if (!canInvoke()) {
    const now = new Date().toISOString();
    return {
      id: crypto.randomUUID(),
      name,
      storagePath: "browser-preview",
      activeRecordingId: null,
      createdAt: now,
      updatedAt: now,
    };
  }
  return invoke<Project>("create_project", { name });
}

export async function listJobs(): Promise<Job[]> {
  if (!canInvoke()) return [];
  return invoke<Job[]>("list_jobs");
}

export async function createJob(
  jobType: string,
  projectId?: string,
  recordingId?: string,
): Promise<Job> {
  if (!canInvoke()) {
    const now = new Date().toISOString();
    return {
      id: crypto.randomUUID(),
      projectId: projectId ?? "browser-preview",
      type: jobType,
      status: "queued",
      progress: 0,
      inputRefs: [],
      outputRefs: [],
      providerId: null,
      errorCode: null,
      errorMessage: null,
      startedAt: null,
      finishedAt: null,
      createdAt: now,
      updatedAt: now,
    };
  }
  return invoke<Job>("create_job", {
    jobType,
    projectId: projectId ?? null,
    recordingId: recordingId ?? null,
  });
}

export async function detailedTranscriptionReadiness(): Promise<DetailedTranscriptionReadiness> {
  if (!canInvoke()) {
    return {
      available: false,
      reason: "โหมดละเอียดต้องใช้ runtime ในแอป FUNG Desktop",
      accuracyQualified: false,
    };
  }
  return invoke<DetailedTranscriptionReadiness>("detailed_transcription_readiness");
}

export async function listDetailedTranscriptProposals(
  projectId: string,
  recordingId: string,
): Promise<DetailedTranscriptProposal[]> {
  if (!canInvoke()) return [];
  return invoke<DetailedTranscriptProposal[]>("list_detailed_transcript_proposals", {
    projectId,
    recordingId,
  });
}

export async function reviewDetailedTranscriptProposal(
  projectId: string,
  recordingId: string,
  proposalId: string,
  decision: "accepted" | "rejected",
): Promise<void> {
  if (!canInvoke()) throw new Error("การรีวิวโหมดละเอียดต้องใช้ FUNG Desktop");
  await invoke<void>("review_detailed_transcript_proposal", {
    projectId,
    recordingId,
    proposalId,
    decision,
  });
}

/**
 * Outcomes of a cancel request. `requestedWhileRunning` matters: the job
 * engine cannot interrupt a handler mid-call, so the work in flight still
 * finishes on its own terms and the cancel applies when it returns.
 */
export type CancelOutcome =
  | "cancelled"
  | "requestedWhileRunning"
  | "notPending";

export async function cancelJob(jobId: string): Promise<CancelOutcome> {
  if (!canInvoke()) return "notPending";
  return invoke<CancelOutcome>("cancel_job", { jobId });
}

/**
 * Whether this installation can diarize, and which prerequisite is missing
 * when it cannot. See `src-tauri/src/diarization.rs`.
 */
export type DiarizationReadiness = {
  available: boolean;
  blocker:
    | "runtimeMissing"
    | "workerMissing"
    | "dependenciesMissing"
    | "modelNotFetched"
    | null;
  detail: string | null;
  runtimePresent: boolean;
  workerPresent: boolean;
  dependenciesPresent: boolean;
  modelPresent: boolean;
  tokenConfigured: boolean;
  model: string;
  cacheRoot: string;
};

export async function diarizationStatus(): Promise<DiarizationReadiness | null> {
  // `null` means "cannot be asked", which is not the same as "unavailable" —
  // the browser preview has no backend to probe.
  if (!canInvoke()) return null;
  return invoke<DiarizationReadiness>("diarization_status");
}

/**
 * Whether this installation may fetch media from a URL, and what is missing
 * when it may not. See `src-tauri/src/media_fetch.rs`.
 */
export type MediaFetchReadiness = {
  available: boolean;
  blocker:
    | "runtimeMissing"
    | "workerMissing"
    | "dependenciesMissing"
    | "consentWithheld"
    | null;
  /** Stable code for the same blocker; branch on this, never on `detail`. */
  blockerCode: string | null;
  detail: string | null;
  runtimePresent: boolean;
  workerPresent: boolean;
  dependenciesPresent: boolean;
  consentGranted: boolean;
  /**
   * Whether a JS runtime is staged for YouTube's signature challenges. Not a
   * blocker — other sites work without it — so this is an advisory the UI
   * shows rather than a reason to refuse.
   */
  jsRuntimePresent: boolean;
  jsRuntimeDetail: string | null;
  maxDurationS: number;
  packagesDir: string;
};

export async function mediaFetchStatus(): Promise<MediaFetchReadiness | null> {
  // `null` means "cannot be asked", which is not the same as "unavailable" —
  // the browser preview has no backend to probe.
  if (!canInvoke()) return null;
  return invoke<MediaFetchReadiness>("media_fetch_status");
}

/**
 * Grants or revokes permission for FUNG to reach the internet for media.
 * Returns the resulting readiness, so a caller that turns consent on learns
 * in the same round-trip whether anything else is still missing.
 */
export async function setMediaFetchConsent(enabled: boolean): Promise<MediaFetchReadiness | null> {
  if (!canInvoke()) return null;
  return invoke<MediaFetchReadiness>("media_fetch_consent_set", { enabled });
}

/** Fetches the audio behind a URL and transcribes it, as one job. */
export async function fetchAndTranscribe(url: string, projectId?: string): Promise<Job> {
  if (!canInvoke()) {
    const now = new Date().toISOString();
    return {
      id: crypto.randomUUID(),
      projectId: projectId ?? "browser-preview",
      type: "transcript.transcribe",
      status: "running",
      progress: 0,
      inputRefs: [url],
      outputRefs: [],
      providerId: null,
      errorCode: null,
      errorMessage: null,
      startedAt: now,
      finishedAt: null,
      createdAt: now,
      updatedAt: now,
    };
  }
  return invoke<Job>("fetch_and_transcribe", { url, projectId: projectId ?? null });
}

/**
 * A file the project has exported. `kind` matches the `export_artifacts`
 * enum: `wav`/`mp3` source-audio exports, `srt`/`vtt` subtitle renders, and
 * `txt` from a meeting summary.
 */
export type ExportArtifact = {
  id: string;
  kind: string;
  filePath: string;
  createdAt: string;
};

/** A project's exports, newest first. */
export async function listExportArtifacts(projectId: string): Promise<ExportArtifact[]> {
  if (!canInvoke()) return [];
  return invoke<ExportArtifact[]>("list_export_artifacts", { projectId });
}

export async function listRecordings(
  projectId: string,
  limit = 50,
  cursor: string | null = null,
): Promise<RecordingPage> {
  if (!canInvoke()) throw nativeUnavailable("Recording history");
  return invoke<RecordingPage>("desktop_recordings_list", {
    projectId,
    limit,
    cursor,
  });
}

export async function releaseRecordingList(snapshotId: string): Promise<RecordingListRelease> {
  if (!canInvoke()) throw nativeUnavailable("Recording history");
  return invoke<RecordingListRelease>("desktop_recordings_release", { snapshotId });
}

export async function getRecording(projectId: string, recordingId: string): Promise<RecordingRow> {
  if (!canInvoke()) throw nativeUnavailable("Recording review");
  return invoke<RecordingRow>("desktop_recording_get", { projectId, recordingId });
}

export async function askRecording(
  projectId: string,
  recordingId: string,
  question: string,
  requestId: string,
): Promise<RecordingAnswer> {
  if (!canInvoke()) throw nativeUnavailable("Recording questions");
  return invoke<RecordingAnswer>("meeting_ask_recording", {
    projectId,
    recordingId,
    question,
    requestId,
  });
}

type PlaybackExpectation = {
  projectId: string;
  recordingId: string;
  channel: PlaybackChannel | "source";
  sourceId?: string;
  captureEpoch?: number;
};

type PlaybackIdentity = PlaybackExpectation & {
  generation: number;
  adoptionToken: string;
  acknowledged: boolean;
};

type PlaybackAdoptionOffer = { generation: number; token: string };

// Only acknowledged, owner-issued handles enter this map. Native ownership
// remains authoritative; this local map fences status/control responses.
const playbackIdentities = new Map<string, PlaybackIdentity>();
const PLAYBACK_HEARTBEAT_INTERVAL_MS = 2_000;
const PLAYBACK_TERMINAL_HEARTBEAT_CODES = new Set([
  "PLAYBACK_HANDLE_INVALID",
  "PLAYBACK_LEASE_EXPIRED",
  "PLAYBACK_ADOPTION_EXPIRED",
  "PLAYBACK_ADOPTION_REQUIRED",
]);
let playbackHeartbeatTimer: number | null = null;

export class PlaybackCloseUnconfirmedError extends Error {
  readonly code = "PLAYBACK_CLOSE_UNCONFIRMED";
  readonly retryable: boolean;
  readonly closeRetryable: boolean;
  readonly handle: string;
  constructor(handle: string, closeRetryable = true) {
    super(closeRetryable
      ? "ยังยืนยันการปิดเครื่องเล่นไม่ได้ โปรดลองปิดอีกครั้ง"
      : "ยืนยันการรับเครื่องเล่นไม่ได้ ระบบจะปิดเซสชันเมื่อสิทธิ์หมดอายุ");
    this.retryable = closeRetryable;
    this.closeRetryable = closeRetryable;
    this.handle = handle;
    this.name = "PlaybackCloseUnconfirmedError";
  }
}

function sourceRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown> : null;
}

function validSourceEpoch(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function validPlaybackIdentity(value: unknown, expected: PlaybackExpectation, handle?: string): value is PlaybackState {
  const row = sourceRecord(value);
  if (!row || row.projectId !== expected.projectId || row.recordingId !== expected.recordingId
    || row.channel !== expected.channel || typeof row.handle !== "string" || !row.handle
    || (handle !== undefined && row.handle !== handle)) return false;
  if (expected.channel !== "source") return row.source === undefined || row.source === null;
  const source = sourceRecord(row.source);
  return source !== null && source.sourceId === expected.sourceId
    && validSourceEpoch(source.captureEpoch) && source.captureEpoch === expected.captureEpoch;
}

async function rejectInvalidOpen(): Promise<never> {
  // No valid nonce means there is no close authority to send. Native adoption
  // timeout owns cleanup of this undelivered session; do not create renderer
  // close custody for a handle that cannot be retried with exact identity.
  throw new Error("PLAYBACK_ADOPTION_OFFER_INVALID");
}

async function acceptPlaybackOpen(
  value: unknown,
  expected: PlaybackExpectation,
): Promise<PlaybackState> {
  if (!validPlaybackIdentity(value, expected)) return rejectInvalidOpen();
  const row = sourceRecord(value);
  const adoption = sourceRecord(row?.adoption);
  if (!adoption || !Number.isSafeInteger(adoption.generation) || (adoption.generation as number) <= 0
    || typeof adoption.token !== "string" || !adoption.token) return rejectInvalidOpen();
  const adoptedIdentity: PlaybackIdentity = {
    ...expected,
    generation: adoption.generation as number,
    adoptionToken: adoption.token,
    acknowledged: false,
  };
  try {
    const receipt = await invoke<{ adopted?: boolean }>("desktop_playback_adopt", {
      handle: value.handle,
      lease: { generation: adoptedIdentity.generation, token: adoptedIdentity.adoptionToken },
    });
    if (receipt?.adopted !== true) throw new Error("PLAYBACK_ADOPTION_UNCONFIRMED");
    playbackIdentities.set(value.handle, { ...adoptedIdentity, acknowledged: true });
  } catch {
    playbackIdentities.delete(value.handle);
    throw new PlaybackCloseUnconfirmedError(value.handle, false);
  }
  const { adoption: _adoption, ...state } = value as PlaybackState & { adoption: PlaybackAdoptionOffer };
  return state;
}

export async function openPlayback(
  projectId: string,
  recordingId: string,
  channel: PlaybackChannel,
): Promise<PlaybackState> {
  if (!canInvoke()) throw nativeUnavailable("Desktop playback");
  const value = await invoke<unknown>("desktop_playback_open", {
    projectId, recordingId, channel,
  });
  return acceptPlaybackOpen(value, { projectId, recordingId, channel });
}

export async function listPlaybackSources(projectId: string, recordingId: string): Promise<PlaybackSource[]> {
  if (!canInvoke()) throw nativeUnavailable("Desktop playback sources");
  const rows = await invoke<unknown>("desktop_playback_sources", { projectId, recordingId });
  if (!isPlaybackSourceCatalog(rows)) throw new Error("INVALID_PLAYBACK_SOURCE_METADATA");
  return rows.map(({ sourceId, captureEpoch, label, kind, sampleRate, durationFrames }) =>
    ({ sourceId, captureEpoch, label, kind, sampleRate, durationFrames }));
}

export async function openSourcePlayback(
  projectId: string, recordingId: string, sourceId: string, captureEpoch: number,
): Promise<PlaybackState> {
  if (!canInvoke()) throw nativeUnavailable("Desktop source playback");
  if (!sourceId || !validSourceEpoch(captureEpoch)) throw new Error("INVALID_PLAYBACK_SOURCE_METADATA");
  const value = await invoke<unknown>("desktop_playback_open_source", {
    projectId, recordingId, sourceId, captureEpoch,
  });
  return acceptPlaybackOpen(value, { projectId, recordingId, channel: "source", sourceId, captureEpoch });
}

function validatePlaybackResponse(value: unknown, handle: string): PlaybackState {
  const expected = playbackIdentities.get(handle);
  if (!expected?.acknowledged || !validPlaybackIdentity(value, expected, handle)) {
    throw new Error("INVALID_PLAYBACK_SOURCE_METADATA");
  }
  return value;
}

export async function controlPlayback(
  handle: string, expectedEpoch: number, action: PlaybackAction, positionMs?: number,
): Promise<PlaybackState> {
  if (!canInvoke()) throw nativeUnavailable("Desktop playback");
  const identity = playbackIdentities.get(handle);
  if (!identity?.acknowledged) throw new Error("INVALID_PLAYBACK_SOURCE_METADATA");
  const args: { handle: string; expectedEpoch: number; action: PlaybackAction; positionMs?: number; lease: { generation: number; token: string } } = {
    handle, expectedEpoch, action,
    lease: { generation: identity.generation, token: identity.adoptionToken },
  };
  if (positionMs !== undefined) args.positionMs = positionMs;
  const value = await invoke<unknown>("desktop_playback_control", args);
  return validatePlaybackResponse(value, handle);
}

export async function getPlayback(handle: string): Promise<PlaybackState> {
  if (!canInvoke()) throw nativeUnavailable("Desktop playback");
  const identity = playbackIdentities.get(handle);
  if (!identity?.acknowledged) throw new Error("INVALID_PLAYBACK_SOURCE_METADATA");
  const value = await invoke<unknown>("desktop_playback_status", {
    handle,
    lease: { generation: identity.generation, token: identity.adoptionToken },
  });
  return validatePlaybackResponse(value, handle);
}

export async function closePlayback(handle: string): Promise<PlaybackCloseAcknowledgement> {
  if (!canInvoke()) throw nativeUnavailable("Desktop playback");
  const identity = playbackIdentities.get(handle);
  if (!identity) throw new Error("INVALID_PLAYBACK_SOURCE_METADATA");
  const receipt = await invoke<PlaybackCloseAcknowledgement>("desktop_playback_close", {
    handle,
    lease: { generation: identity.generation, token: identity.adoptionToken },
  });
  if (receipt?.closed === true) playbackIdentities.delete(handle);
  return receipt;
}

export async function heartbeatPlaybackSessions(): Promise<void> {
  for (const [handle, identity] of playbackIdentities) {
    if (!identity.acknowledged) continue;
    try {
      await invoke("desktop_playback_heartbeat", {
        handle,
        lease: { generation: identity.generation, token: identity.adoptionToken },
      });
    } catch (error) {
      const code = sourceRecord(error)?.code;
      if (typeof code === "string"
        && PLAYBACK_TERMINAL_HEARTBEAT_CODES.has(code)
        && playbackIdentities.get(handle) === identity) {
        playbackIdentities.delete(handle);
      }
    }
  }
}

export function startPlaybackHeartbeat(): void {
  if (!canInvoke() || playbackHeartbeatTimer !== null) return;
  playbackHeartbeatTimer = window.setInterval(() => {
    void heartbeatPlaybackSessions().catch(() => undefined);
  }, PLAYBACK_HEARTBEAT_INTERVAL_MS);
}

export async function listModelProviders(): Promise<ModelProvider[]> {
  if (!canInvoke()) return [];
  return invoke<ModelProvider[]>("list_model_providers");
}

// ── TTS Provider Management ──

export async function ttsProviderRegister(
  label: string,
  configJson: string,
): Promise<{ providerId: string; validation: { ok: boolean; error?: string; warnings: string[] } }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_register", { input: { label, configJson } });
}

export async function ttsProviderUpdate(
  providerId: string,
  label?: string,
  configJson?: string,
): Promise<{ ok: boolean; error?: string; warnings: string[] }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_update", { input: { providerId, label, configJson } });
}

export async function ttsProviderToggle(
  providerId: string,
  enabled: boolean,
): Promise<boolean> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_toggle", { providerId, enabled });
}

export async function ttsProviderTest(
  providerId: string,
  testText?: string,
): Promise<{ status: string; latencyMs?: number; audioPath?: string; message?: string }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_provider_test", { providerId, testText });
}

export async function ttsSynthesizeText(
  text: string,
  providerId?: string,
  refAudio?: string,
  refText?: string,
): Promise<{ audioPath: string; latencyMs: number }> {
  if (!canInvoke()) throw new Error("Tauri not available");
  return invoke("tts_synthesize_text", {
    input: { text, providerId, refAudio, refText },
  });
}

/**
 * A project's transcript, and whether it is all of it.
 *
 * `capped` is not an error state — the segments returned are real. It means
 * the storage engine's single-read ceiling was reached, so material past it
 * exists and was not read. Rendering the segments without saying so is what
 * this shape exists to prevent: a transcript that stops mid-meeting is
 * indistinguishable from a meeting that ended there.
 */
export type TranscriptView = {
  segments: TranscriptSegment[];
  capped: boolean;
  cap: number;
  /** Which recordings are incomplete, not just that one of them is. */
  cappedRecordingIds: string[];
};

const EMPTY_TRANSCRIPT: TranscriptView = {
  segments: [],
  capped: false,
  cap: 0,
  cappedRecordingIds: [],
};

export type TranscriptLoadState = {
  requestId: number;
  recordingId: string | null;
  status: "idle" | "loading" | "ready" | "rejected";
  view: TranscriptView | null;
};

export function beginTranscriptLoad(recordingId: string | null, requestId: number): TranscriptLoadState {
  return {
    requestId,
    recordingId,
    status: recordingId ? "loading" : "idle",
    view: null,
  };
}

export function settleTranscriptLoad(
  state: TranscriptLoadState,
  request: {
    requestId: number;
    recordingId: string;
    outcome:
      | { status: "fulfilled"; view: TranscriptView }
      | { status: "rejected" };
  },
): TranscriptLoadState {
  if (state.requestId !== request.requestId || state.recordingId !== request.recordingId) return state;
  if (request.outcome.status === "rejected") {
    return { ...state, status: "rejected", view: null };
  }
  return { ...state, status: "ready", view: request.outcome.view };
}

export async function listTranscriptSegments(
  projectId: string,
  recordingId: string,
): Promise<TranscriptView> {
  if (!canInvoke()) return EMPTY_TRANSCRIPT;
  return invoke<TranscriptView>("list_transcript_segments", { projectId, recordingId });
}

export async function correctTranscriptSegment(
  projectId: string,
  recordingId: string,
  segmentId: string,
  correctedText: string,
): Promise<void> {
  if (!canInvoke()) throw new Error("Transcript correction requires the desktop app");
  await invoke<void>("correct_transcript_segment", {
    projectId,
    recordingId,
    segmentId,
    correctedText,
  });
}

export async function renameSpeaker(speakerId: string, displayName: string): Promise<void> {
  if (!canInvoke()) return;
  await invoke<void>("mobile_speaker_rename", { speakerId, displayName });
}

export async function importAndTranscribe(filePath: string, projectId?: string): Promise<Job> {
  if (!canInvoke()) {
    const now = new Date().toISOString();
    return {
      id: crypto.randomUUID(),
      projectId: projectId ?? "browser-preview",
      type: "transcript.transcribe",
      status: "running",
      progress: 0,
      inputRefs: [filePath],
      outputRefs: [],
      providerId: null,
      errorCode: null,
      errorMessage: null,
      startedAt: now,
      finishedAt: null,
      createdAt: now,
      updatedAt: now,
    };
  }
  return invoke<Job>("import_and_transcribe", { filePath, projectId: projectId ?? null });
}

export async function pickAudioOrVideoFile(): Promise<string | null> {
  if (!canInvoke()) return null;
  const selection = await open({
    multiple: false,
    title: "Import audio or video to transcribe",
    filters: [
      { name: "Audio & video", extensions: ["wav", "mp3", "m4a", "mp4", "mov", "mkv", "webm", "ogg", "flac"] },
    ],
  });
  if (!selection) return null;
  return Array.isArray(selection) ? selection[0] ?? null : selection;
}

export type RecordingOutputStatus = {
  currentPath: string;
  defaultPath: string;
  isDefault: boolean;
  writable: boolean;
  issue: string | null;
  captureActive: boolean;
};

export async function recordingOutputGet(): Promise<RecordingOutputStatus> {
  if (!canInvoke()) throw new Error("ปลายทางไฟล์บันทึกต้องรันในแอปเดสก์ท็อป");
  return invoke<RecordingOutputStatus>("recording_output_get");
}

export async function recordingOutputSet(path: string): Promise<RecordingOutputStatus> {
  if (!canInvoke()) throw new Error("ปลายทางไฟล์บันทึกต้องรันในแอปเดสก์ท็อป");
  return invoke<RecordingOutputStatus>("recording_output_set", { path });
}

export async function recordingOutputReset(): Promise<RecordingOutputStatus> {
  if (!canInvoke()) throw new Error("ปลายทางไฟล์บันทึกต้องรันในแอปเดสก์ท็อป");
  return invoke<RecordingOutputStatus>("recording_output_reset");
}

export async function pickRecordingOutputFolder(): Promise<string | null> {
  if (!canInvoke()) return null;
  const selection = await open({
    directory: true,
    multiple: false,
    title: "เลือกโฟลเดอร์ไฟล์บันทึก",
  });
  if (!selection) return null;
  return Array.isArray(selection) ? selection[0] ?? null : selection;
}

export async function minimizeWindow(): Promise<void> {
  if (!canInvoke()) return;
  await getCurrentWindow().minimize();
}

export async function hideWindowToTray(): Promise<void> {
  if (!canInvoke()) return;
  await invoke<void>("desktop_window_hide_to_tray");
}

export async function closeWindow(): Promise<void> {
  if (!canInvoke()) return;
  await getCurrentWindow().close();
}

export async function openExternalAccountPortal(): Promise<void> {
  if (canInvoke()) {
    await invoke<void>("open_external_account_portal");
    return;
  }

  const url = import.meta.env.VITE_FUNG_WEB_APP_URL;
  if (!url?.startsWith("https://")) {
    throw new Error("The hosted account portal is not configured.");
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export type ZoomConnectionStatus = {
  status: "disconnected" | "connecting" | "connected" | "error";
  accountLabel: string | null;
  revokeFailed: boolean;
};

export type ZoomRecordingSummary = {
  uuid: string;
  topic: string;
  startTime: string;
  durationMinutes: number;
  hasParticipantAudio: boolean;
};

const zoomOffline: ZoomConnectionStatus = { status: "disconnected", accountLabel: null, revokeFailed: false };

export async function zoomConnect(): Promise<ZoomConnectionStatus> {
  if (!canInvoke()) return zoomOffline;
  return invoke<ZoomConnectionStatus>("zoom_connect");
}

export async function zoomConnectionStatus(): Promise<ZoomConnectionStatus> {
  if (!canInvoke()) return zoomOffline;
  return invoke<ZoomConnectionStatus>("zoom_connection_status");
}

export async function zoomDisconnect(): Promise<ZoomConnectionStatus> {
  if (!canInvoke()) return zoomOffline;
  return invoke<ZoomConnectionStatus>("zoom_disconnect");
}

export async function zoomListRecordings(): Promise<ZoomRecordingSummary[]> {
  if (!canInvoke()) return [];
  return invoke<ZoomRecordingSummary[]>("zoom_list_recordings");
}

export async function zoomImportRecording(meetingUuid: string): Promise<Job> {
  if (!canInvoke()) throw new Error("Zoom import requires the desktop app.");
  return invoke<Job>("zoom_import_recording", { meetingUuid });
}

export async function graphBuildStart(projectId: string, recordingId: string): Promise<void> {
  if (!canInvoke()) return;
  await invoke<void>("graph_build_start", { projectId, recordingId });
}

// ── Live Meeting (Meeting Mode MVP) ──

export type LiveStartOutput = {
  projectId: string;
  recordingId: string;
  jobId: string;
  micDevice: string | null;
  systemDevice: string | null;
  warning: string | null;
};

export type LiveCaptureDevice = {
  id: string;
  name: string;
  isDefault: boolean;
  available: boolean;
};

export type LiveCaptureApplication = {
  processId: number;
  name: string;
};

export type LiveCaptureDevices = {
  inputs: LiveCaptureDevice[];
  loopbackOutputs: LiveCaptureDevice[];
  applications: LiveCaptureApplication[];
  applicationCaptureAvailable: boolean;
  applicationCaptureIssue: string | null;
  selectedMicDeviceId: string | null;
  selectedSystemDeviceId: string | null;
  issue: string | null;
};

export type LiveAudioLevelEvent = {
  recordingId: string;
  channel: "mic" | "system";
  levelPercent: number;
  waveform: number[];
};

export type LiveAudioLevelSample = {
  levelPercent: number;
  waveform: readonly number[];
  observedAt: number;
};

export type LiveStatusOutput = {
  active: boolean;
  stopping: boolean;
  projectId: string | null;
  recordingId: string | null;
  elapsedMs: number | null;
};

export type LiveSegmentEvent = {
  recordingId: string;
  segmentId: string;
  channel: string;
  speaker: string;
  startMs: number;
  endMs: number;
  text: string;
  confidence: number | null;
};

export type LiveTopicEvent = {
  recordingId: string;
  topic: string;
  openPoints: string[];
  actionItems: string[];
  model: string;
  windowStartMs: number;
  windowEndMs: number;
};

export type LiveStatusEvent = {
  recordingId: string;
  state: string;
  detail: string | null;
  micDevice: string | null;
  systemDevice: string | null;
};

export type LiveSummaryEvent = {
  recordingId: string;
  state: "running" | "ready" | "failed" | "skipped";
  detail: string | null;
  exportPath: string | null;
};

export type AskSource = {
  n: number;
  kind: string;
  projectName: string | null;
  text: string;
  startMs: number | null;
  recordingId: string | null;
};

export type AskAnswer = {
  answer: string;
  sources: AskSource[];
  model: string;
  searchedRowsCapped: boolean;
};

// Response shapes live in ./lib/meetingSummaries so they can be loaded
// outside a browser; re-exported here so callers keep one import site.
export type {
  MeetingSummaries,
  SummaryRow,
} from "./lib/meetingSummaries.ts";
export { EMPTY_MEETING_SUMMARIES } from "./lib/meetingSummaries.ts";

export async function liveCaptureDevices(): Promise<LiveCaptureDevices> {
  if (!canInvoke()) throw new Error("รายการอุปกรณ์บันทึกต้องรันในแอปเดสก์ท็อป");
  return invoke<LiveCaptureDevices>("live_capture_devices");
}

export async function captureRouteInventory(processIds?: number[]): Promise<NativeCaptureInventory> {
  if (!canInvoke()) throw new Error("รายการเส้นทางเสียงต้องรันในแอปเดสก์ท็อป");
  return invoke<NativeCaptureInventory>("capture_route_inventory", { processIds: processIds ?? null });
}

export async function captureRouteValidate(plan: CaptureRoutePlan): Promise<ValidatedRoutePlan> {
  if (!canInvoke()) throw new Error("การตรวจเส้นทางเสียงต้องรันในแอปเดสก์ท็อป");
  return invoke<ValidatedRoutePlan>("capture_route_validate", { plan });
}

export async function captureRouteStart(
  projectId: string,
  plan: CaptureRoutePlan,
): Promise<RoutedCaptureStatus> {
  if (!canInvoke()) throw new Error("การบันทึกผ่านอุปกรณ์เฉพาะต้องรันในแอปเดสก์ท็อป");
  return invoke<RoutedCaptureStatus>("capture_route_start", { projectId, plan });
}

export async function captureRouteStatus(): Promise<RoutedCaptureStatus | null> {
  if (!canInvoke()) return null;
  return invoke<RoutedCaptureStatus | null>("capture_route_status");
}

export async function captureRouteStop(): Promise<RoutedCaptureStatus> {
  if (!canInvoke()) throw new Error("การหยุดบันทึกผ่านอุปกรณ์เฉพาะต้องรันในแอปเดสก์ท็อป");
  return invoke<RoutedCaptureStatus>("capture_route_stop");
}

export async function liveMeetingStart(options?: {
  projectId?: string;
  captureSystem?: boolean;
  captureMicrophone?: boolean;
  language?: string;
  transcriptProfile?: "chunked" | "revisioned";
  micDeviceId?: string;
  systemDeviceId?: string;
  applicationProcessId?: number;
}): Promise<LiveStartOutput> {
  if (!canInvoke()) throw new Error("Live Meeting ต้องรันในแอปเดสก์ท็อป");
  return invoke<LiveStartOutput>("live_meeting_start", {
    projectId: options?.projectId ?? null,
    captureSystem: options?.captureSystem ?? true,
    captureMicrophone: options?.captureMicrophone ?? true,
    language: options?.language ?? null,
    transcriptProfile: options?.transcriptProfile ?? "chunked",
    micDeviceId: options?.micDeviceId ?? null,
    systemDeviceId: options?.systemDeviceId ?? null,
    applicationProcessId: options?.applicationProcessId ?? null,
  });
}

export async function liveMeetingStop(): Promise<string> {
  if (!canInvoke()) throw new Error("Live Meeting ต้องรันในแอปเดสก์ท็อป");
  return invoke<string>("live_meeting_stop");
}

export async function liveMeetingStatus(): Promise<LiveStatusOutput> {
  if (!canInvoke()) {
    return { active: false, stopping: false, projectId: null, recordingId: null, elapsedMs: null };
  }
  return invoke<LiveStatusOutput>("live_meeting_status");
}

export async function meetingAsk(question: string, projectId?: string): Promise<AskAnswer> {
  if (!canInvoke()) throw new Error("ถาม FUNG ต้องรันในแอปเดสก์ท็อป");
  return invoke<AskAnswer>("meeting_ask", { question, projectId: projectId ?? null });
}

export async function meetingSummaries(
  projectId: string,
  recordingId: string,
): Promise<MeetingSummariesShape> {
  if (!canInvoke()) return EMPTY_SUMMARIES;
  return invoke<MeetingSummariesShape>("meeting_summaries", {
    projectId,
    recordingId,
  });
}

export async function generateMeetingSummary(projectId: string, recordingId: string): Promise<void> {
  if (!canInvoke()) return;
  await invoke<void>("generate_meeting_summary", { projectId, recordingId });
}

export async function externalConnectorsList(): Promise<ExternalConnectorSummary[]> {
  if (!canInvoke()) return [];
  return invoke<ExternalConnectorSummary[]>("external_connectors_list");
}

export async function externalConnectorRegister(
  input: ExternalConnectorRegisterInput,
): Promise<ExternalConnectorSummary> {
  if (!canInvoke()) throw new Error("External meeting tools require the desktop app");
  return invoke<ExternalConnectorSummary>("external_connector_register", { input });
}

export async function externalConnectorDisconnect(
  connectorId: string,
): Promise<ExternalConnectorDisconnectReceipt> {
  if (!canInvoke()) throw new Error("External meeting tools require the desktop app");
  return invoke<ExternalConnectorDisconnectReceipt>("external_connector_disconnect", { connectorId });
}

export async function meetingToolSuggest(input: {
  projectId: string;
  recordingId: string;
  connectorId: string;
  capability: ConnectorCapability;
  arguments: Record<string, unknown>;
  evidenceRefs: string[];
}): Promise<MeetingToolPreviewEnvelope> {
  if (!canInvoke()) throw new Error("External meeting tools require the desktop app");
  return invoke<MeetingToolPreviewEnvelope>("meeting_tool_suggest", { input });
}

export async function meetingToolExecute(input: {
  runId: string;
  previewId: string;
  approvedPreviewHash: string;
  arguments: Record<string, unknown>;
}): Promise<MeetingToolExecutionEnvelope> {
  if (!canInvoke()) throw new Error("External meeting tools require the desktop app");
  return invoke<MeetingToolExecutionEnvelope>("meeting_tool_execute", { input });
}

export async function meetingToolCancel(runId: string): Promise<MeetingToolCancelReceipt> {
  if (!canInvoke()) throw new Error("External meeting tools require the desktop app");
  return invoke<MeetingToolCancelReceipt>("meeting_tool_cancel", { runId });
}

export async function meetingToolRevoke(input: {
  grantId: string;
  projectId: string;
  recordingId: string;
}): Promise<MeetingToolRevokeReceipt> {
  if (!canInvoke()) throw new Error("External meeting tools require the desktop app");
  return invoke<MeetingToolRevokeReceipt>("meeting_tool_revoke", { input });
}

export async function meetingToolRunsList(
  projectId: string,
  recordingId: string,
): Promise<ExternalToolRun[]> {
  if (!canInvoke()) return [];
  return invoke<ExternalToolRun[]>("meeting_tool_runs_list", { projectId, recordingId });
}
