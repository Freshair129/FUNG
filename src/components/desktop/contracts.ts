import type { ReactNode } from "react";
import type {
  ExportArtifact,
  DetailedTranscriptionReadiness,
  DetailedTranscriptProposal,
  Job,
  LiveCaptureDevices,
  LiveSegmentEvent,
  LiveStartOutput,
  LiveStatusOutput,
  LiveTopicEvent,
  Project,
  TranscriptView,
} from "../../tauri.ts";
import type { MeetingSummaries } from "../../lib/meetingSummaries.ts";

/**
 * The exact pair used by every selected-recording read or action.
 * These are opaque IDs; they are not filesystem paths.
 */
export type RecordingKey = {
  projectId: string;
  recordingId: string;
};

export type ReviewErrorCode =
  | "INVALID_ARGUMENT"
  | "PROJECT_NOT_FOUND"
  | "RECORDING_NOT_FOUND"
  | "SCOPE_MISMATCH"
  | "STORAGE_READ_FAILED"
  | "INVALID_RECORDING_METADATA"
  | "RESOURCE_LIMIT"
  | "CURSOR_INVALID"
  | "CURSOR_EXPIRED"
  | "NATIVE_UNAVAILABLE"
  | "LEGACY_COMMAND_FAILED"
  | "NO_EVIDENCE"
  | "PROVIDER_UNAVAILABLE"
  | "PROVIDER_FAILED"
  | "MODEL_OUTPUT_INVALID"
  | "PLAYBACK_OUTPUT_UNSUPPORTED"
  | "PLAYBACK_FORMAT_UNSUPPORTED"
  | "PLAYBACK_TIMELINE_INVALID"
  | "PLAYBACK_SOURCE_MISSING"
  | "PLAYBACK_PATH_DENIED"
  | "PLAYBACK_BUFFER_UNDERRUN"
  | "PLAYBACK_STALE_EPOCH"
  | "PLAYBACK_HANDLE_INVALID"
  | "PLAYBACK_IO_FAILED"
  | "PLAYBACK_DEVICE_LOST"
  | "PLAYBACK_CAPTURE_ACTIVE"
  | "PLAYBACK_BUSY"
  | "PLAYBACK_OPEN_TIMEOUT";

export type ReviewError = {
  code: ReviewErrorCode;
  message: string;
  retryable: boolean;
};

const reviewErrorCodes: ReadonlySet<string> = new Set([
  "INVALID_ARGUMENT",
  "PROJECT_NOT_FOUND",
  "RECORDING_NOT_FOUND",
  "SCOPE_MISMATCH",
  "STORAGE_READ_FAILED",
  "INVALID_RECORDING_METADATA",
  "RESOURCE_LIMIT",
  "CURSOR_INVALID",
  "CURSOR_EXPIRED",
  "NATIVE_UNAVAILABLE",
  "LEGACY_COMMAND_FAILED",
  "NO_EVIDENCE",
  "PROVIDER_UNAVAILABLE",
  "PROVIDER_FAILED",
  "MODEL_OUTPUT_INVALID",
  "PLAYBACK_OUTPUT_UNSUPPORTED",
  "PLAYBACK_FORMAT_UNSUPPORTED",
  "PLAYBACK_TIMELINE_INVALID",
  "PLAYBACK_SOURCE_MISSING",
  "PLAYBACK_PATH_DENIED",
  "PLAYBACK_BUFFER_UNDERRUN",
  "PLAYBACK_STALE_EPOCH",
  "PLAYBACK_HANDLE_INVALID",
  "PLAYBACK_IO_FAILED",
  "PLAYBACK_DEVICE_LOST",
  "PLAYBACK_CAPTURE_ACTIVE",
  "PLAYBACK_BUSY",
  "PLAYBACK_OPEN_TIMEOUT",
]);

export function normalizeReviewError(error: unknown): ReviewError {
  if (typeof error === "object" && error !== null) {
    const candidate = error as Record<string, unknown>;
    if (
      typeof candidate.code === "string" &&
      reviewErrorCodes.has(candidate.code) &&
      typeof candidate.message === "string" &&
      typeof candidate.retryable === "boolean"
    ) {
      return candidate as ReviewError;
    }
  }

  return {
    code: "LEGACY_COMMAND_FAILED",
    message: "Desktop command failed.",
    retryable: false,
  };
}

export type ReviewRequestIdentity = RecordingKey & {
  selectionEpoch: number;
  requestId: string;
};

export type ReadState<T> = {
  status: "idle" | "loading" | "ready" | "error" | "unavailable";
  identity: ReviewRequestIdentity | null;
  data: T | null;
  error: ReviewError | null;
};

export type ReviewLoadSettlement<T> = {
  identity: ReviewRequestIdentity;
  outcome:
    | { status: "fulfilled"; data: T }
    | { status: "rejected"; error: ReviewError };
};

/**
 * Settles only the currently selected request. A stale fulfillment or
 * rejection returns the exact current object, so late failures cannot replace
 * current data with an error (or make it look empty).
 */
export function settleReviewLoad<T>(
  current: ReadState<T>,
  settlement: ReviewLoadSettlement<T>,
): ReadState<T> {
  const currentIdentity = current.identity;
  const nextIdentity = settlement.identity;
  if (
    currentIdentity === null ||
    currentIdentity.projectId !== nextIdentity.projectId ||
    currentIdentity.recordingId !== nextIdentity.recordingId ||
    currentIdentity.selectionEpoch !== nextIdentity.selectionEpoch ||
    currentIdentity.requestId !== nextIdentity.requestId
  ) {
    return current;
  }

  if (settlement.outcome.status === "fulfilled") {
    return {
      ...current,
      status: "ready",
      data: settlement.outcome.data,
      error: null,
    };
  }

  return {
    ...current,
    status: "error",
    data: null,
    error: settlement.outcome.error,
  };
}

export type PlaybackChannel = "mic" | "system" | "file";

export type RecordingCaptureState = "active" | "stopping" | "inactive";

export type RecordingRow = {
  id: string;
  projectId: string;
  source: string;
  status: string;
  durationMs: number;
  createdAt: string;
  updatedAt: string;
  language: string | null;
  channels: PlaybackChannel[];
  captureState: RecordingCaptureState;
};

export type RecordingPage = {
  projectId: string;
  snapshotId: string;
  asOf: string;
  items: RecordingRow[];
  nextCursor: string | null;
};

export type RecordingListRelease = {
  released: boolean;
};

export type RecordingAnswerSource = {
  segmentId: string;
  projectId: string;
  recordingId: string;
  startMs: number;
  endMs: number;
  text: string;
  citationIndex: number;
};

export type RecordingAnswer = {
  projectId: string;
  recordingId: string;
  requestId: string;
  scope: "recording";
  status: "answered" | "insufficient_evidence";
  answer: string;
  model: string | null;
  sources: RecordingAnswerSource[];
  graphPolicy: "excluded";
  liveTailPolicy: "excluded";
};

export type PlaybackStateName = "paused" | "playing" | "ended" | "error";

export type PlaybackMissingRange = {
  startMs: number;
  endMs: number;
  reason: string;
};

export type PlaybackState = {
  handle: string;
  projectId: string;
  recordingId: string;
  channel: PlaybackChannel;
  state: PlaybackStateName;
  positionMs: number;
  durationMs: number;
  streamEpoch: number;
  degraded: boolean;
  missingRanges: PlaybackMissingRange[];
  outputLatencyMs: number | null;
  error: ReviewError | null;
};

export type PlaybackAction = "play" | "pause" | "seek";

export type PlaybackCloseAcknowledgement = {
  closed: boolean;
};

export type ThemeChoice = "system" | "light" | "dark";
export type MaterialChoice = "glass" | "solid";
export type TransparencyChoice = "full" | "reduced";
export type ScopeChoice = "A" | "B";
export type DesktopSurface = "home" | "live" | "review" | "output" | "appearance";
export type DesktopAccountStatus = {
  state:
    | "signed_out"
    | "login_pending"
    | "authenticated"
    | "refreshing"
    | "refresh_failed"
    | "logout_pending"
    | "credential_cleanup_failed"
    | "shutdown";
  email: string | null;
};
export type LivePhase =
  | "idle"
  | "starting"
  | "listening"
  | "degraded"
  | "stopping"
  | "stopped"
  | "error";

export type LiveDevices = {
  mic: string | null;
  system: string | null;
};

export type LiveStartOptions = {
  projectId?: string;
  captureSystem?: boolean;
  language?: string;
  transcriptProfile?: "chunked" | "revisioned";
  micDeviceId?: string;
  systemDeviceId?: string;
};

export type CapabilityState = {
  available: boolean;
  reasonCode: string | null;
};

export type DesktopCapabilities = Readonly<Record<string, CapabilityState>>;

/**
 * Starting capture must close the review player first and await its native
 * acknowledgement. A false acknowledgement blocks the start request.
 */
export type CloseReviewPlayer = () => Promise<PlaybackCloseAcknowledgement>;

export type StartCaptureAction = (
  options: LiveStartOptions,
  closeReviewPlayer: CloseReviewPlayer,
) => Promise<LiveStartOutput>;

export type StopCaptureAction = () => Promise<string>;

/**
 * Resolves only after the native status is active=false and stopping=false.
 * A stop request acknowledgement alone is not sufficient for navigation.
 */
export type StopAndLeaveAction = () => Promise<LiveStatusOutput>;

export type DesktopShellActions = {
  selectProject: (projectId: string) => void | Promise<void>;
  selectRecording: (selection: RecordingKey | null) => void;
  showHome: () => void;
  showLive: () => void;
  showReview: () => void;
  showOutput: () => void;
  showAppearance: () => void;
  startRecording: () => void | Promise<void>;
  stopRecording: () => void | Promise<void>;
  openReview: () => void | Promise<void>;
  stopAndLeave: StopAndLeaveAction;
  openSettings: () => void;
  openAccount: () => void;
  openPairing: () => void;
  importMedia: () => void | Promise<void>;
  exportMedia: () => void | Promise<void>;
  exportDisabled?: boolean;
  exportTitle?: string;
  setTheme: (theme: ThemeChoice) => void;
  minimizeWindow: () => void | Promise<void>;
  closeWindow: () => void | Promise<void>;
};

export type DesktopShellProps = {
  scopeChoice: ScopeChoice;
  project: ReadState<Project[]>;
  selectedProjectId: string | null;
  selection: RecordingKey | null;
  activeSurface: DesktopSurface;
  liveStatus: ReadState<LiveStatusOutput>;
  livePhase?: LivePhase;
  theme: ThemeChoice;
  accountStatus?: DesktopAccountStatus | null;
  mainContent: ReactNode;
  settingsSlot?: ReactNode;
  pairingSlot?: ReactNode;
  recoverySlot?: ReactNode;
  actions: DesktopShellActions;
};

export type LiveWorkspaceActions = {
  start: StartCaptureAction;
  stop: StopCaptureAction;
  stopAndLeave: StopAndLeaveAction;
  ask: (
    selection: RecordingKey,
    question: string,
    requestId: string,
  ) => Promise<RecordingAnswer>;
  generateSummary: () => Promise<void>;
  closeView: () => void;
};

/**
 * LiveMeetingPanel remains the sole capture/listener owner. LiveWorkspace
 * receives snapshots and callbacks; it does not create a second store or
 * register another live-event subscription.
 */
export type LiveWorkspaceProps = {
  selection: RecordingKey | null;
  phase: LivePhase;
  elapsedMs: number;
  devices: LiveDevices;
  captureDevices: LiveCaptureDevices | null;
  captureDevicesLoading: boolean;
  captureDevicesError: string | null;
  refreshCaptureDevices: () => void | Promise<void>;
  segmentFeed: readonly LiveSegmentEvent[];
  topic: ReadState<LiveTopicEvent>;
  summaries: ReadState<MeetingSummaries>;
  ask: ReadState<RecordingAnswer>;
  capabilities: DesktopCapabilities;
  operationErrors: readonly ReviewError[];
  actions: LiveWorkspaceActions;
};

export type RecordingReviewActions = {
  refresh: () => Promise<void>;
  listNext: () => Promise<void>;
  select: (selection: RecordingKey | null) => void;
  correctSegment: (
    selection: RecordingKey,
    segmentId: string,
    correctedText: string,
  ) => Promise<void>;
  renameSpeaker: (speakerId: string, displayName: string) => Promise<void>;
  queueExistingJob: (selection: RecordingKey, jobType: string) => Promise<Job>;
  detailedTranscriptionReadiness: () => Promise<DetailedTranscriptionReadiness>;
  listDetailedTranscriptProposals: (selection: RecordingKey) => Promise<DetailedTranscriptProposal[]>;
  reviewDetailedTranscriptProposal: (
    selection: RecordingKey,
    proposalId: string,
    decision: "accepted" | "rejected",
  ) => Promise<void>;
  ask: (
    selection: RecordingKey,
    question: string,
    requestId: string,
  ) => Promise<RecordingAnswer>;
  playbackOpen: (
    selection: RecordingKey,
    channel: PlaybackChannel,
  ) => Promise<PlaybackState>;
  playbackControl: (
    handle: string,
    expectedEpoch: number,
    action: PlaybackAction,
    positionMs?: number,
  ) => Promise<PlaybackState>;
  playbackClose: (handle: string) => Promise<PlaybackCloseAcknowledgement>;
};

export type RecordingReviewProps = {
  selection: RecordingKey | null;
  recording: ReadState<RecordingRow>;
  list: ReadState<RecordingPage>;
  transcript: ReadState<TranscriptView>;
  summaries: ReadState<MeetingSummaries>;
  exportState: ReadState<ExportArtifact[]>;
  playback: ReadState<PlaybackState>;
  ask: ReadState<RecordingAnswer>;
  scopeChoice: ScopeChoice;
  actions: RecordingReviewActions;
};
