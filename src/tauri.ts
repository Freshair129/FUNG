import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";

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
};

export type ZoomRecordingSummary = {
  uuid: string;
  topic: string;
  startTime: string;
  durationMinutes: number;
  hasParticipantAudio: boolean;
};

const zoomOffline: ZoomConnectionStatus = { status: "disconnected", accountLabel: null };

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
