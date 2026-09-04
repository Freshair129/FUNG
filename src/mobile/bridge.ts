import { invoke } from "@tauri-apps/api/core";
import { supabase } from "../lib/supabase";
import type { DelegatedExecutor, DelegatedJob, EffectNode, GraphEdge, GraphNode, MobileNote, RecordingListItem, StorySequence, TimelineData } from "./model";

const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type CaptureSession = {
  recordingId: string;
  state: string;
  safeOffsetMs: number;
  segmentCount: number;
};

export type NativeSegment = {
  sequence: number;
  relativePath: string;
  durationMs: number;
  byteSize: number;
  checksum: string;
};

export type NativeRecorderStatus = {
  available: boolean;
  recordingId: string | null;
  state: string;
  safeOffsetMs: number;
  segmentCount: number;
  /** Live amplitude 0-100 from MediaRecorder.getMaxAmplitude; 0 when idle. */
  levelPercent?: number;
  segments: NativeSegment[];
};

export type PlaybackSegment = { sequence: number; durationMs: number; mimeType: string; bytes: number[]; hasNext: boolean };

export type OnDeviceAiStatus = {
  probeAvailable: boolean;
  tier: "core" | "ai_lite" | "ai_standard" | "ai_pro";
  reason: string;
  sdkInt: number | null;
  arm64: boolean | null;
  totalRamMb: number | null;
  availableRamMb: number | null;
  freeStorageMb: number | null;
};

const unavailableRecorder = (): NativeRecorderStatus => ({
  available: false,
  recordingId: null,
  state: "unavailable",
  safeOffsetMs: 0,
  segmentCount: 0,
  segments: [],
});

export async function startCapture(projectId: string): Promise<CaptureSession | null> {
  if (!isTauri()) return null;
  return invoke<CaptureSession>("mobile_capture_start", { projectId });
}

export async function appendCaptureSegment(recordingId: string, bytes: Uint8Array, durationMs: number): Promise<CaptureSession | null> {
  if (!isTauri()) return null;
  return invoke<CaptureSession>("mobile_capture_append_segment", {
    recordingId,
    bytes: Array.from(bytes),
    durationMs,
  });
}

export async function finishCapture(recordingId: string): Promise<CaptureSession | null> {
  if (!isTauri()) return null;
  return invoke<CaptureSession>("mobile_capture_finish", { recordingId });
}

export async function loadPlaybackSegment(recordingId: string, sequence: number): Promise<PlaybackSegment | null> {
  if (!isTauri()) return null;
  return invoke<PlaybackSegment>("mobile_capture_playback_segment", { recordingId, sequence });
}

export async function reconcileNativeCapture(recordingId: string, segments: NativeSegment[]): Promise<CaptureSession | null> {
  if (!isTauri()) return null;
  return invoke<CaptureSession>("mobile_capture_reconcile_native", { recordingId, segments });
}

export async function startNativeRecorder(recordingId: string): Promise<NativeRecorderStatus> {
  if (!isTauri()) return unavailableRecorder();
  return invoke<NativeRecorderStatus>("mobile_native_recorder_start", { recordingId });
}

export async function nativeRecorderStatus(recordingId: string): Promise<NativeRecorderStatus> {
  if (!isTauri()) return unavailableRecorder();
  return invoke<NativeRecorderStatus>("mobile_native_recorder_status", { recordingId });
}

export async function controlNativeRecorder(recordingId: string, action: "pause" | "resume" | "stop"): Promise<NativeRecorderStatus> {
  if (!isTauri()) return unavailableRecorder();
  return invoke<NativeRecorderStatus>("mobile_native_recorder_control", { recordingId, action });
}

export async function persistNote(note: MobileNote): Promise<void> {
  if (!isTauri()) return;
  await invoke("mobile_note_upsert", { note });
}

export async function queryGraph(projectId: string): Promise<{ nodes: GraphNode[]; edges: GraphEdge[] } | null> {
  if (!isTauri()) return null;
  return invoke("mobile_graph_query", { projectId, depth: 2 });
}

export async function pairingComplete(
  peerDeviceId: string,
  name: string,
  endpoint: string,
  pairingSessionId: string,
  publicKey: string | null,
): Promise<void> {
  if (!isTauri()) return;
  await invoke("mobile_pairing_complete", { peerDeviceId, name, endpoint, pairingSessionId, publicKey });
}

export async function deviceIdentityEnsure(): Promise<{ fingerprint: string; created: boolean } | null> {
  if (!isTauri()) return null;
  return invoke("device_identity_ensure");
}

export async function devicePublicKey(): Promise<string | null> {
  if (!isTauri()) return null;
  return invoke<string>("device_public_key");
}

export async function setMcpEnabled(enabled: boolean, exposeLan: boolean): Promise<{ enabled: boolean; bind: string | null }> {
  if (!isTauri()) return { enabled, bind: enabled ? "browser-preview" : null };
  return invoke("mobile_mcp_set_enabled", { enabled, exposeLan });
}

export async function onDeviceAiStatus(): Promise<OnDeviceAiStatus | null> {
  if (!isTauri()) return null;
  return invoke<OnDeviceAiStatus>("mobile_on_device_ai_status");
}

export async function queryRecordings(projectId: string): Promise<RecordingListItem[]> {
  if (!isTauri()) return [];
  return invoke<RecordingListItem[]>("mobile_recordings_query", { projectId });
}

export async function queryTimeline(projectId: string): Promise<TimelineData | null> {
  if (!isTauri()) return null;
  return invoke<TimelineData>("mobile_timeline_query", { projectId });
}

export async function startDiarization(projectId: string, recordingId: string): Promise<{ id: string; state: string }> {
  if (!isTauri()) return { id: crypto.randomUUID(), state: "preview" };
  return invoke("mobile_diarization_start", { projectId, recordingId });
}

export async function startProcessingJob(projectId: string, operation: "transcript.refine" | "audio.effect_preview" | "story.export" | "model.install", inputRef: string): Promise<{ id: string; state: string } | null> {
  if (!isTauri()) return null;
  return invoke("mobile_processing_job_start", { projectId, operation, inputRef });
}

export async function renameSpeaker(speakerId: string, displayName: string): Promise<void> {
  if (!isTauri()) return;
  await invoke("mobile_speaker_rename", { speakerId, displayName });
}

export async function splitSpeakerTurn(turnId: string, splitMs: number): Promise<TimelineData | null> {
  if (!isTauri()) return null;
  return invoke("mobile_speaker_turn_split", { turnId, splitMs });
}

export async function mergeSpeakers(projectId: string, sourceSpeakerId: string, targetSpeakerId: string): Promise<TimelineData | null> {
  if (!isTauri()) return null;
  return invoke("mobile_speaker_merge", { projectId, sourceSpeakerId, targetSpeakerId });
}

export async function confirmSpeakerTurn(turnId: string): Promise<void> {
  if (!isTauri()) return;
  await invoke("mobile_speaker_turn_confirm", { turnId });
}

export async function queryStory(projectId: string): Promise<StorySequence | null> {
  if (!isTauri()) return null;
  return invoke("mobile_story_query", { projectId });
}

export async function createStory(projectId: string, title: string): Promise<StorySequence | null> {
  if (!isTauri()) return null;
  return invoke("mobile_story_create", { projectId, title });
}

export async function moveStoryClip(clipId: string, timelineStartMs: number): Promise<StorySequence | null> {
  if (!isTauri()) return null;
  return invoke("mobile_story_clip_move", { clipId, timelineStartMs });
}

export async function trimStoryClip(clipId: string, sourceStartMs: number, sourceEndMs: number): Promise<StorySequence | null> {
  if (!isTauri()) return null;
  return invoke("mobile_story_clip_trim", { clipId, sourceStartMs, sourceEndMs });
}

export async function splitStoryClip(clipId: string, splitSourceMs: number): Promise<StorySequence | null> {
  if (!isTauri()) return null;
  return invoke("mobile_story_clip_split", { clipId, splitSourceMs });
}

export async function storyHistory(sequenceId: string, direction: "undo" | "redo"): Promise<StorySequence | null> {
  if (!isTauri()) return null;
  return invoke(direction === "undo" ? "mobile_story_undo" : "mobile_story_redo", { sequenceId });
}

export async function reviewRefinement(proposalId: string, decision: "accepted" | "rejected"): Promise<void> {
  if (!isTauri()) return;
  await invoke("mobile_refinement_review", { proposalId, decision });
}

export async function updateEffectChain(projectId: string, ownerKind: "project" | "story_clip" | "voice_profile", ownerId: string, label: string, effects: EffectNode[]): Promise<string | null> {
  if (!isTauri()) return null;
  const nodes = effects.map((effect, position) => ({ id: effect.id, kind: effect.kind, position, parameters: effect.parameters, bypassed: effect.bypassed }));
  return invoke("mobile_effect_chain_update", { projectId, ownerKind, ownerId, label, nodes });
}

export async function setAgentVoiceGrant(projectId: string, voiceProfileId: string, enabled: boolean): Promise<boolean> {
  if (!isTauri()) return enabled;
  return invoke("mobile_agent_voice_grant_set", { projectId, mcpClientId: "fung-mobile-mcp", voiceProfileId, enabled });
}

// ---------------------------------------------------------------------
// FUNGWIRE delegation (Task 10): resolve a paired desktop's LAN endpoint
// from Supabase, probe it, hand off a transcription job, and poll for
// progress. Command param names below are read directly off the
// `#[tauri::command]` signatures in `src-tauri/src/fungwire_client.rs` —
// both `fungwire_desktop_reachable` and `fungwire_delegate_transcription`
// take an `own_device_id` (this mobile device's Supabase `devices.id`,
// cached in `localStorage["fung.device.id"]` by MobileApp.tsx) in addition
// to the resolved `endpoint`, so callers must supply both.
// ---------------------------------------------------------------------

export async function desktopEndpoint(deviceId: string): Promise<string | null> {
  const { data } = await supabase.from("devices").select("lan_endpoint, lan_endpoint_updated_at").eq("id", deviceId).maybeSingle();
  return (data?.lan_endpoint as string | undefined) ?? null;
}

export async function desktopReachable(deviceId: string, endpoint: string, ownDeviceId: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("fungwire_desktop_reachable", { desktopDeviceId: deviceId, endpoint, ownDeviceId });
}

export async function delegateTranscription(
  projectId: string,
  recordingId: string,
  desktopDeviceId: string,
  endpoint: string,
  ownDeviceId: string,
  executor: DelegatedExecutor = "local",
): Promise<{ jobId: string } | null> {
  if (!isTauri()) return null;
  return invoke<{ jobId: string }>("fungwire_delegate_transcription", { projectId, recordingId, desktopDeviceId, endpoint, ownDeviceId, executor });
}

type DesktopStatusProbe = {
  sttCloudEnabled: boolean;
};

/** Reads whether the PAIRED DESKTOP (not this mobile device) currently has
 * its STT cloud tier enabled. The setting — like the API key it spends — is
 * owned exclusively by the device that holds the keys (spec §10), so mobile
 * can only ever read it, never toggle it: there is no writing counterpart to
 * this function anywhere in the mobile bridge, by design.
 *
 * `ownDeviceId` is required for the same reason `desktopReachable` needs it:
 * the probe dials the desktop over FUNGWIRE, and the wire protocol's opening
 * `Control::Hello{device_id}` must carry an id the desktop's
 * `paired_devices.db` recognises as the caller (see `fungwire_client.rs`'s
 * module doc). Returns false on ANY failure — desktop unreachable, not
 * paired, an older desktop that doesn't answer `StatusRequest` — matching
 * `desktopReachable`'s fail-closed convention: an unknown policy must never
 * surface a cloud affordance. */
export async function desktopCloudEnabled(deviceId: string, endpoint: string, ownDeviceId: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    const status = await invoke<DesktopStatusProbe>("fungwire_desktop_status_probe", { desktopDeviceId: deviceId, endpoint, ownDeviceId });
    return status.sttCloudEnabled;
  } catch {
    return false;
  }
}

// `fungwire_job_poll` (Rust `JobPollOutput`) reports `{ state, progress,
// executor, error }` — it doesn't echo back the job id or operation, so
// those are filled in from what the caller already knows rather than from
// the wire response. `executor` IS read from the row (Genesis schema v7's
// `delegated_jobs.executor`) so the cloud provenance badge doesn't depend on
// the caller still holding the choice it made at delegation time.
export async function pollDelegatedJob(jobId: string): Promise<DelegatedJob | null> {
  if (!isTauri()) return null;
  const result = await invoke<{ state: DelegatedJob["state"]; progress: number; executor: string | null; error: string | null }>("fungwire_job_poll", { jobId });
  if (!result) return null;
  return {
    id: jobId,
    operation: "transcript.transcribe",
    state: result.state,
    progress: result.progress,
    executorDeviceId: null,
    executor: result.executor === "cloud" ? "cloud" : result.executor === "local" ? "local" : undefined,
    error: result.error ?? undefined,
  };
}
