export type MobileTab = "home" | "notes" | "voice" | "pitching" | "timeline" | "graph" | "devices";

export type PitchTimerMode = "topic_countdown" | "total_countdown" | "count_up";

export type PitchTopic = {
  id: string;
  title: string;
  targetDurationSec: number;
  notes?: string;
};

export type PitchSessionState = {
  active: boolean;
  recordingId: string | null;
  startedAt: number | null;
  pausedAt: number | null;
  elapsedMs: number;
  currentTopicIndex: number;
  timerMode: PitchTimerMode;
  topics: PitchTopic[];
  enableFrontCamera: boolean;
  enableVoiceCommands: boolean;
  dndEnabled: boolean;
  walLastSyncedAt: number | null;
};

export type ThemePreference = "system" | "light" | "dark";

export type Speaker = {
  id: string;
  projectId: string;
  key: string;
  displayName: string;
  confidence: number | null;
};

export type SpeakerTurn = {
  id: string;
  projectId: string;
  recordingId: string;
  speakerId: string;
  startMs: number;
  endMs: number;
  confidence: number | null;
  status: "proposed" | "confirmed";
  overlap: boolean;
  revision: number;
};

export type TimelineData = {
  recordingId: string | null;
  durationMs: number;
  speakers: Speaker[];
  turns: SpeakerTurn[];
  source: "local" | "desktop" | "preview";
};

export type StoryClip = {
  id: string;
  sourceTurnId: string | null;
  sourceRecordingId: string;
  sourceStartMs: number;
  sourceEndMs: number;
  timelineStartMs: number;
  speakerId: string;
  effectChainId: string | null;
  revision: number;
};

export type StorySequence = {
  id: string;
  projectId: string;
  title: string;
  durationMs: number;
  revision: number;
  clips: StoryClip[];
};

export type ModelPackage = {
  id: string;
  label: string;
  sizeLabel: string;
  installed: boolean;
  compatible: boolean;
  runtimeLocation: "mobile" | "desktop";
  languages: number;
};

export type RefinementProposal = {
  id: string;
  originalText: string;
  proposedText: string;
  status: "proposed" | "accepted" | "rejected";
  policy: string;
};

export type EffectKind = "pitch_shift" | "reverb" | "delay" | "compressor" | "low_pass";

export type EffectNode = {
  id: string;
  kind: EffectKind;
  label: string;
  bypassed: boolean;
  parameters: Record<string, number>;
};

export type VoiceProfile = {
  id: string;
  displayName: string;
  rightsBasis: "owned_recording" | "licensed_pack" | "explicit_consent";
  rightsState: "valid" | "revoked" | "expired";
  providerAvailable: boolean;
};

export type TtsRuntimeType = "python_script" | "rest_api" | "local_binary";

export type TtsPythonScriptConfig = {
  runtime_type: "python_script";
  venv_path: string;
  script_path: string;
  model_path?: string;
  device: "cuda" | "cpu";
};

export type TtsRestApiConfig = {
  runtime_type: "rest_api";
  endpoint: string;
  auth_header?: string;
};

export type TtsLocalBinaryConfig = {
  runtime_type: "local_binary";
  binary_path: string;
  model_path?: string;
  args_template: string;
};

export type TtsProviderConfig =
  | TtsPythonScriptConfig
  | TtsRestApiConfig
  | TtsLocalBinaryConfig;

export type TtsTestResult = {
  status: "ok" | "error";
  latencyMs?: number;
  audioPath?: string;
  message?: string;
};

export type TtsValidation = {
  ok: boolean;
  error?: string;
  warnings: string[];
};

export type TtsRegisterResult = {
  providerId: string;
  validation: TtsValidation;
};

export type CreativeStudioState = {
  story: StorySequence;
  undo: StorySequence[];
  redo: StorySequence[];
  selectedModelId: string;
  proposals: RefinementProposal[];
  effects: EffectNode[];
  voiceProfile: VoiceProfile;
  agentVoiceGrant: boolean;
  updatedAt: string;
};

// "ai_proposed" is server-assigned only (graph_build.rs extraction layer) —
// mobile_relation_upsert's CLIENT_WRITABLE_EPISTEMIC_STATUS list rejects it
// from clients, so it only ever appears on edges read from queryGraph.
export type EpistemicStatus = "confirmed" | "inferred" | "evidence" | "superseded" | "disputed" | "ai_proposed";

export type MobileNote = {
  id: string;
  title: string;
  body: string;
  projectId: string;
  createdAt: string;
  updatedAt: string;
  evidenceLabel?: string;
};

// "meeting" | "speaker" | "topic" | "decision" | "action_item" | "mention"
// come from graph_build.rs's structural + LLM-extraction layers.
export type GraphNode = {
  id: string;
  label: string;
  kind: "note" | "project" | "recording" | "person" | "meeting" | "speaker" | "topic" | "decision" | "action_item" | "mention";
  x: number;
  y: number;
};

export type GraphEdge = {
  id: string;
  sourceId: string;
  targetId: string;
  predicate: string;
  status: EpistemicStatus;
};

export type DeviceState = {
  id: string;
  name: string;
  endpoint: string;
  trustState: "unpaired" | "paired" | "unreachable" | "revoked";
  capabilities: string[];
  lastSeenAt?: string;
  cloudDeviceId?: string;
  pairingSessionId?: string;
};

export type CaptureState = {
  state: "idle" | "preparing" | "recording" | "paused" | "finalizing" | "completed" | "recovery_required";
  recordingId: string | null;
  startedAt: number | null;
  pausedAt: number | null;
  elapsedMs: number;
  safeOffsetMs: number;
  segmentCount: number;
  storageWarning: boolean;
  backend: "web" | "android-native" | null;
  error: {
    stage: "session" | "native-start" | "web-permission" | "native-sync";
    detail: string;
  } | null;
};

export type MobileSnapshot = {
  projectId: string;
  notes: MobileNote[];
  nodes: GraphNode[];
  edges: GraphEdge[];
  devices: DeviceState[];
};
