// @req FR-106, FR-108, FR-116
// @tested tests/externalMeetingTools.test.mjs
import { invoke } from "@tauri-apps/api/core";
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
} from "./lib/externalMeetingTools";

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
  };
};

export type Project = {
  id: string;
  name: string;
  storagePath: string;
  activeRecordingId: string | null;
  createdAt: string;
  updatedAt: string;
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
  version: "0.1.0",
  databasePath: "browser-preview",
  sqliteWal: true,
  genesisPath: "browser-preview",
  genesisStableFrontier: 0,
  storageAuthority: "browser preview",
  localApi: {
    running: false,
    bind: null,
  },
};

const canInvoke = () => Boolean("__TAURI_INTERNALS__" in window);

/** Raw command bridge for panels that own their own error handling, or `null`
 * when this surface is a plain browser and no command can run. */
export const nativeInvoke = canInvoke()
  ? (<T,>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args))
  : null;

export async function getHealth(): Promise<Health> {
  if (!canInvoke()) return fallbackHealth;
  return invoke<Health>("app_health");
}

export async function startLocalApi(): Promise<string> {
  if (!canInvoke()) return "browser-preview";
  return invoke<string>("start_local_api");
}

export async function listProjects(): Promise<Project[]> {
  if (!canInvoke()) return [];
  return invoke<Project[]>("list_projects");
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

export async function createJob(jobType: string, projectId?: string): Promise<Job> {
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
  return invoke<Job>("create_job", { jobType, projectId: projectId ?? null });
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

export async function listTranscriptSegments(projectId: string): Promise<TranscriptSegment[]> {
  if (!canInvoke()) return [];
  return invoke<TranscriptSegment[]>("list_transcript_segments", { projectId });
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

export async function minimizeWindow(): Promise<void> {
  if (!canInvoke()) return;
  await getCurrentWindow().minimize();
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
  micDevice: string;
  systemDevice: string | null;
  warning: string | null;
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
  state: "running" | "ready" | "failed";
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

export type SummaryRow = {
  id: string;
  kind: string;
  content: string;
  evidenceCount: number;
  createdAt: string;
};

export async function liveMeetingStart(options?: {
  projectId?: string;
  captureSystem?: boolean;
  language?: string;
}): Promise<LiveStartOutput> {
  if (!canInvoke()) throw new Error("Live Meeting ต้องรันในแอปเดสก์ท็อป");
  return invoke<LiveStartOutput>("live_meeting_start", {
    projectId: options?.projectId ?? null,
    captureSystem: options?.captureSystem ?? true,
    language: options?.language ?? null,
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

export async function meetingSummaries(projectId: string): Promise<SummaryRow[]> {
  if (!canInvoke()) return [];
  return invoke<SummaryRow[]>("meeting_summaries", { projectId });
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
