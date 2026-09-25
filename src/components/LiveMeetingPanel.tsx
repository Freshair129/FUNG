// @req FR-102, FR-103, FR-104, FR-105, FR-115, NFR-104, NFR-106
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  askRecording,
  generateMeetingSummary,
  liveCaptureDevices,
  listTranscriptSegments,
  liveMeetingStart,
  liveMeetingStatus,
  liveMeetingStop,
  meetingSummaries,
  type LiveSegmentEvent,
  type LiveCaptureDevices,
  type LiveStartOutput,
  type LiveStatusEvent,
  type LiveStatusOutput,
  type LiveSummaryEvent,
  type LiveTopicEvent,
  type TranscriptView,
} from "../tauri";
import type { MeetingSummaries } from "../lib/meetingSummaries";
import {
  normalizeReviewError,
  settleReviewLoad,
  type CloseReviewPlayer,
  type DesktopCapabilities,
  type LiveDevices,
  type LivePhase,
  type LiveWorkspaceProps,
  type ReadState,
  type RecordingAnswer,
  type RecordingKey,
  type ReviewError,
  type ReviewRequestIdentity,
  type StopAndLeaveAction,
} from "./desktop/contracts";
import { LiveWorkspace } from "./desktop/LiveWorkspace";
import "./LiveMeetingPanel.css";

// <ExternalMeetingToolsPanel> remains rendered by LiveWorkspace so the existing
// default-off preview/approval/revoke surface has one presentation owner.

type BufferedLiveEvent =
  | { kind: "live-status"; payload: LiveStatusEvent }
  | { kind: "live-segment"; payload: LiveSegmentEvent }
  | { kind: "live-topic"; payload: LiveTopicEvent }
  | { kind: "live-summary"; payload: LiveSummaryEvent };

export type LiveLifecycleDependencies = {
  listen: <T>(
    eventName: string,
    handler: (payload: T) => void,
  ) => Promise<UnlistenFn>;
  readStatus: () => Promise<LiveStatusOutput>;
  readTranscript: (selection: RecordingKey) => Promise<TranscriptView>;
};

export type LiveLifecycleSink = {
  onAuthoritativeStatus: (status: LiveStatusOutput) => void;
  onStatusEvent: (event: LiveStatusEvent) => void;
  onSegment: (event: LiveSegmentEvent) => void;
  onTopic: (event: LiveTopicEvent) => void;
  onSummary: (event: LiveSummaryEvent) => void;
  onTranscript: (selection: RecordingKey, view: TranscriptView) => void;
  onIncomplete: () => void;
  onError: (error: ReviewError) => void;
};

export type LiveEventLifecycle = {
  start: () => Promise<void>;
  dispose: () => void;
  setKnownIdentity: (selection: RecordingKey | null) => void;
  setVisible: (visible: boolean) => void;
  refreshStatus: (force?: boolean) => Promise<void>;
};

const MAX_BOOTSTRAP_EVENTS = 200;
const UNKNOWN_REFRESH_COOLDOWN_MS = 250;

function sameRecordingKey(
  left: RecordingKey | null,
  right: RecordingKey | null,
): boolean {
  return Boolean(
    left &&
      right &&
      left.projectId === right.projectId &&
      left.recordingId === right.recordingId,
  );
}

function createIdentity(
  selection: RecordingKey,
  selectionEpoch: number,
  requestId: string,
): ReviewRequestIdentity {
  return {
    projectId: selection.projectId,
    recordingId: selection.recordingId,
    selectionEpoch,
    requestId,
  };
}

function emptyReadState<T>(): ReadState<T> {
  return {
    status: "idle",
    identity: null,
    data: null,
    error: null,
  };
}

const EMPTY_MEETING_SUMMARIES: ReadState<MeetingSummaries> = emptyReadState();

export type LiveControllerSnapshot = {
  liveStatus: ReadState<LiveStatusOutput>;
  phase: LivePhase;
  selection: RecordingKey | null;
};

export type LiveMeetingController = {
  getSnapshot: () => LiveControllerSnapshot;
  subscribe: (listener: (snapshot: LiveControllerSnapshot) => void) => () => void;
  stopAndLeave: StopAndLeaveAction;
  dispose: () => void;
};

export type LiveControllerAdapter = {
  controller: LiveMeetingController;
  publish: (snapshot: LiveControllerSnapshot) => void;
  dispose: () => void;
};

const CONTROLLER_DISPOSED_ERROR: ReviewError = {
  code: "LEGACY_COMMAND_FAILED",
  message: "The live controller is no longer active.",
  retryable: false,
};

/**
 * Local shell handoff. It forwards owner snapshots and the existing stop
 * action without giving the shell a second native listener or status reader.
 */
export function createLiveControllerAdapter(
  initialSnapshot: LiveControllerSnapshot,
  stopAndLeave: StopAndLeaveAction,
): LiveControllerAdapter {
  let snapshot = initialSnapshot;
  let disposed = false;
  const listeners = new Set<(next: LiveControllerSnapshot) => void>();

  const controller: LiveMeetingController = {
    getSnapshot: () => snapshot,
    subscribe: (listener) => {
      if (disposed) return () => undefined;
      listeners.add(listener);
      listener(snapshot);
      return () => listeners.delete(listener);
    },
    stopAndLeave: async () => {
      if (disposed) throw CONTROLLER_DISPOSED_ERROR;
      try {
        return await stopAndLeave();
      } catch (error) {
        throw normalizeReviewError(error);
      }
    },
    dispose: () => {
      if (disposed) return;
      disposed = true;
      listeners.clear();
    },
  };

  return {
    controller,
    publish: (next) => {
      if (disposed) return;
      snapshot = next;
      for (const listener of listeners) {
        try {
          listener(snapshot);
        } catch {
          // A shell subscriber cannot interrupt the Live owner.
        }
      }
    },
    dispose: controller.dispose,
  };
}

function nextRequestId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return prefix + "-" + String(Date.now()) + "-" + Math.random().toString(16).slice(2);
}

function safeCaptureStopTimeout(): ReviewError {
  return {
    code: "LEGACY_COMMAND_FAILED",
    message: "Native capture did not settle in the allowed time.",
    retryable: true,
  };
}

/**
 * The Live owner uses this immutable shared settlement helper for both
 * recording-scoped questions and summary reads. A stale rejection is just as
 * harmless as a stale success.
 */
export function settleLiveRequest<T>(
  current: ReadState<T>,
  identity: ReviewRequestIdentity,
  outcome:
    | { status: "fulfilled"; data: T }
    | { status: "rejected"; error: ReviewError },
): ReadState<T> {
  return settleReviewLoad(current, { identity, outcome });
}

/**
 * A capture start is admitted only after the review player confirms closure.
 * The native start callback is never invoked for a false or failed close.
 */
export async function runStartWithCloseAck<T>(
  closeReviewPlayer: CloseReviewPlayer,
  start: () => Promise<T>,
): Promise<T> {
  const blocked: ReviewError = {
    code: "PLAYBACK_BUSY",
    message: "Close the review player before starting capture.",
    retryable: true,
  };

  try {
    const acknowledgement = await closeReviewPlayer();
    if (!acknowledgement || acknowledgement.closed !== true) {
      throw blocked;
    }
    return await start();
  } catch (error) {
    if (error === blocked) throw blocked;
    throw normalizeReviewError(error);
  }
}

/**
 * Stop acknowledgement means only that the request was accepted. Navigation
 * can use this helper to wait for the native owner to become truly inactive.
 */
export async function waitForCaptureInactive(
  readStatus: () => Promise<LiveStatusOutput>,
  options: {
    pollMs?: number;
    timeoutMs?: number;
    sleep?: (ms: number) => Promise<void>;
    onStatus?: (status: LiveStatusOutput) => void;
  } = {},
): Promise<LiveStatusOutput> {
  const pollMs = options.pollMs ?? 250;
  const timeoutMs = options.timeoutMs ?? 12_000;
  const sleep = options.sleep ?? ((ms: number) => new Promise<void>((resolve) => {
    window.setTimeout(resolve, ms);
  }));
  const deadline = Date.now() + timeoutMs;

  while (Date.now() <= deadline) {
    const status = await readStatus();
    options.onStatus?.(status);
    if (!status.active && !status.stopping) return status;
    await sleep(pollMs);
  }

  throw safeCaptureStopTimeout();
}

/**
 * Merge persisted and live rows without allowing a late duplicate segment to
 * create a second visible row. The map key deliberately includes recordingId.
 */
export function mergeLiveSegments(
  current: readonly LiveSegmentEvent[],
  incoming: readonly LiveSegmentEvent[],
): LiveSegmentEvent[] {
  const byIdentity = new Map<string, LiveSegmentEvent>();
  for (const segment of current) {
    byIdentity.set(segment.recordingId + "\u0000" + segment.segmentId, segment);
  }
  for (const segment of incoming) {
    byIdentity.set(segment.recordingId + "\u0000" + segment.segmentId, segment);
  }
  return Array.from(byIdentity.values()).sort((left, right) => (
    left.startMs - right.startMs ||
    left.endMs - right.endMs ||
    left.segmentId.localeCompare(right.segmentId)
  ));
}

export function createLiveEventLifecycle(
  dependencies: LiveLifecycleDependencies,
  sink: LiveLifecycleSink,
): LiveEventLifecycle {
  let disposed = false;
  let started = false;
  let bootstrapReady = false;
  let bootstrapPromise: Promise<void> | null = null;
  let visible = true;
  let identity: RecordingKey | null = null;
  let bootstrapBuffer: BufferedLiveEvent[] = [];
  let overflowed = false;
  let unknownRefreshPromise: Promise<void> | null = null;
  let lastUnknownRefreshAt = 0;
  let identityEpoch = 0;
  let transcriptRequestGeneration = 0;
  const unlisteners: UnlistenFn[] = [];

  const invokeUnlisten = (unlisten: UnlistenFn) => {
    try {
      void unlisten();
    } catch {
      // Cleanup must remain best-effort and must never block disposal.
    }
  };

  const setLifecycleIdentity = (selection: RecordingKey | null) => {
    identity = selection;
    identityEpoch += 1;
    transcriptRequestGeneration += 1;
  };

  const loadTranscript = async (selection: RecordingKey) => {
    const requestEpoch = identityEpoch;
    const requestGeneration = ++transcriptRequestGeneration;
    const isCurrentRequest = () => (
      !disposed &&
      sameRecordingKey(identity, selection) &&
      identityEpoch === requestEpoch &&
      transcriptRequestGeneration === requestGeneration
    );

    try {
      const view = await dependencies.readTranscript(selection);
      if (isCurrentRequest()) {
        sink.onTranscript(selection, view);
      }
    } catch (error) {
      if (isCurrentRequest()) {
        sink.onError(normalizeReviewError(error));
      }
    }
  };

  const acceptAuthoritativeStatus = async (
    status: LiveStatusOutput,
    loadPersistedTranscript: boolean,
  ) => {
    if (disposed) return;
    if (status.active && status.projectId && status.recordingId) {
      const nextIdentity = {
        projectId: status.projectId,
        recordingId: status.recordingId,
      };
      if (!sameRecordingKey(identity, nextIdentity)) {
        setLifecycleIdentity(nextIdentity);
      }
    }
    sink.onAuthoritativeStatus(status);
    if (
      loadPersistedTranscript &&
      status.active &&
      identity
    ) {
      await loadTranscript(identity);
    }
  };

  const refreshStatus = async (force = false) => {
    if (disposed) return;
    if (unknownRefreshPromise) return unknownRefreshPromise;
    const now = Date.now();
    if (!force && now - lastUnknownRefreshAt < UNKNOWN_REFRESH_COOLDOWN_MS) return;
    lastUnknownRefreshAt = now;
    unknownRefreshPromise = (async () => {
      try {
        const status = await dependencies.readStatus();
        await acceptAuthoritativeStatus(status, true);
      } catch (error) {
        if (!disposed) sink.onError(normalizeReviewError(error));
      } finally {
        unknownRefreshPromise = null;
      }
    })();
    return unknownRefreshPromise;
  };

  const processEvent = (event: BufferedLiveEvent) => {
    if (disposed) return;
    const eventRecordingId = event.payload.recordingId;
    if (!identity || identity.recordingId !== eventRecordingId) {
      void refreshStatus();
      return;
    }

    if (event.kind === "live-status") sink.onStatusEvent(event.payload);
    if (event.kind === "live-segment") sink.onSegment(event.payload);
    if (event.kind === "live-topic") sink.onTopic(event.payload);
    if (event.kind === "live-summary") sink.onSummary(event.payload);
  };

  const receive = (event: BufferedLiveEvent) => {
    if (disposed) return;
    if (!bootstrapReady) {
      if (bootstrapBuffer.length >= MAX_BOOTSTRAP_EVENTS) {
        bootstrapBuffer.shift();
        overflowed = true;
      }
      bootstrapBuffer.push(event);
      return;
    }
    processEvent(event);
  };

  const subscribe = <T,>(
    eventName: string,
    kind: BufferedLiveEvent["kind"],
  ) => dependencies.listen<T>(eventName, (payload) => {
    if (kind === "live-status") receive({ kind, payload: payload as LiveStatusEvent });
    if (kind === "live-segment") receive({ kind, payload: payload as LiveSegmentEvent });
    if (kind === "live-topic") receive({ kind, payload: payload as LiveTopicEvent });
    if (kind === "live-summary") receive({ kind, payload: payload as LiveSummaryEvent });
  }).then((unlisten) => {
    if (disposed) {
      invokeUnlisten(unlisten);
    } else {
      unlisteners.push(unlisten);
    }
    return unlisten;
  });

  const bootstrap = async () => {
    const subscriptions = [
      subscribe<LiveStatusEvent>("live-status", "live-status"),
      subscribe<LiveSegmentEvent>("live-segment", "live-segment"),
      subscribe<LiveTopicEvent>("live-topic", "live-topic"),
      subscribe<LiveSummaryEvent>("live-summary", "live-summary"),
    ];
    const results = await Promise.allSettled(subscriptions);
    for (const result of results) {
      if (result.status === "rejected" && !disposed) {
        sink.onError(normalizeReviewError(result.reason));
      }
    }
    if (disposed) return;

    try {
      const status = await dependencies.readStatus();
      await acceptAuthoritativeStatus(status, true);
    } catch (error) {
      if (!disposed) sink.onError(normalizeReviewError(error));
    }

    if (disposed) return;
    bootstrapReady = true;
    if (overflowed) sink.onIncomplete();
    const queued = bootstrapBuffer;
    bootstrapBuffer = [];
    for (const event of queued) processEvent(event);
  };

  const start = () => {
    if (bootstrapPromise) return bootstrapPromise;
    if (started) return Promise.resolve();
    started = true;
    bootstrapPromise = bootstrap();
    return bootstrapPromise;
  };

  const dispose = () => {
    if (disposed) return;
    disposed = true;
    bootstrapBuffer = [];
    for (const unlisten of unlisteners.splice(0)) invokeUnlisten(unlisten);
  };

  return {
    start,
    dispose,
    setKnownIdentity: (selection) => {
      if (!disposed) setLifecycleIdentity(selection);
    },
    setVisible: (nextVisible) => {
      visible = nextVisible;
      void visible;
    },
    refreshStatus,
  };
}

async function subscribeLiveEvent<T>(
  eventName: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(eventName, (event) => handler(event.payload));
}

function persistedSegments(
  selection: RecordingKey,
  view: TranscriptView,
): LiveSegmentEvent[] {
  return view.segments
    .filter((segment) => (
      segment.projectId === selection.projectId &&
      segment.recordingId === selection.recordingId
    ))
    .map((segment) => ({
      recordingId: segment.recordingId,
      segmentId: segment.id,
      channel: "recorded",
      speaker: segment.speakerName ?? segment.speakerId ?? "ไม่ระบุชื่อ",
      startMs: segment.startMs,
      endMs: segment.endMs,
      text: segment.text,
      confidence: segment.confidence,
    }));
}

function scopeMeetingSummaries(
  summaries: MeetingSummaries,
  recordingId: string,
): MeetingSummaries {
  const kinds = Array.from(new Set(summaries.rows.map((row) => row.kind)));
  const rows = kinds.flatMap((kind) => {
    const current = summaries.rows.find((row) => (
      row.recordingId === recordingId && row.kind === kind && !row.superseded
    ));
    return current ? [current] : [];
  });
  return {
    ...summaries,
    rows,
    otherRecordings: summaries.otherRecordings > 0 ? summaries.otherRecordings : 0,
    unattributable: summaries.unattributable > 0 && summaries.attributionComplete
      ? summaries.unattributable
      : 0,
  };
}

export type LiveMeetingPanelProps = {
  onClose: () => void;
  projectId: string | null;
  visible?: boolean;
  closeReviewPlayer?: CloseReviewPlayer;
  onControllerChange?: (controller: LiveMeetingController | null) => void;
};

export function LiveMeetingPanel({
  onClose,
  projectId,
  visible = true,
  closeReviewPlayer,
  onControllerChange,
}: LiveMeetingPanelProps) {
  const [phase, setPhase] = useState<LivePhase>("idle");
  const [devices, setDevices] = useState<LiveDevices>({ mic: null, system: null });
  const [captureDevices, setCaptureDevices] = useState<LiveCaptureDevices | null>(null);
  const [captureDevicesLoading, setCaptureDevicesLoading] = useState(false);
  const [captureDevicesError, setCaptureDevicesError] = useState<string | null>(null);
  const [selection, setSelection] = useState<RecordingKey | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [segments, setSegments] = useState<LiveSegmentEvent[]>([]);
  const [topic, setTopic] = useState<ReadState<LiveTopicEvent>>(emptyReadState);
  const [summaries, setSummaries] = useState<ReadState<MeetingSummaries>>(emptyReadState);
  const [ask, setAsk] = useState<ReadState<RecordingAnswer>>(emptyReadState);
  const [operationErrors, setOperationErrors] = useState<ReviewError[]>([]);
  const [transcriptLoading, setTranscriptLoading] = useState(false);
  const [transcriptIncomplete, setTranscriptIncomplete] = useState(false);
  const nativeAvailable = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  const refreshCaptureDevices = useCallback(async () => {
    if (!nativeAvailable) return;
    setCaptureDevicesLoading(true);
    setCaptureDevicesError(null);
    try {
      setCaptureDevices(await liveCaptureDevices());
    } catch (error) {
      setCaptureDevicesError(normalizeReviewError(error).message);
    } finally {
      setCaptureDevicesLoading(false);
    }
  }, [nativeAvailable]);

  useEffect(() => {
    void refreshCaptureDevices();
  }, [refreshCaptureDevices]);

  const phaseRef = useRef<LivePhase>("idle");
  const activeKeyRef = useRef<RecordingKey | null>(null);
  const activeProjectRef = useRef<string | null>(null);
  const liveStatusRef = useRef<ReadState<LiveStatusOutput>>({
    status: "loading",
    identity: null,
    data: null,
    error: null,
  });
  const epochRef = useRef(0);
  const lifecycleRef = useRef<LiveEventLifecycle | null>(null);
  const stopAndLeaveDelegateRef = useRef<StopAndLeaveAction>(async () => {
    throw CONTROLLER_DISPOSED_ERROR;
  });
  const controllerAdapterRef = useRef<LiveControllerAdapter | null>(null);
  const sinkRef = useRef<LiveLifecycleSink>({
    onAuthoritativeStatus: () => undefined,
    onStatusEvent: () => undefined,
    onSegment: () => undefined,
    onTopic: () => undefined,
    onSummary: () => undefined,
    onTranscript: () => undefined,
    onIncomplete: () => undefined,
    onError: () => undefined,
  });
  const stopRequestRef = useRef<Promise<string> | null>(null);
  const inactiveWaitRef = useRef<Promise<LiveStatusOutput> | null>(null);

  const publishControllerSnapshot = useCallback(() => {
    controllerAdapterRef.current?.publish({
      liveStatus: liveStatusRef.current,
      phase: phaseRef.current,
      selection: activeKeyRef.current,
    });
  }, []);

  useEffect(() => {
    const adapter = createLiveControllerAdapter(
      {
        liveStatus: liveStatusRef.current,
        phase: phaseRef.current,
        selection: activeKeyRef.current,
      },
      () => stopAndLeaveDelegateRef.current(),
    );
    controllerAdapterRef.current = adapter;
    onControllerChange?.(adapter.controller);

    return () => {
      if (controllerAdapterRef.current === adapter) {
        controllerAdapterRef.current = null;
      }
      adapter.dispose();
      onControllerChange?.(null);
    };
  }, [onControllerChange]);

  const setControllerStatus = useCallback((status: LiveStatusOutput) => {
    liveStatusRef.current = {
      status: "ready",
      identity: null,
      data: status,
      error: null,
    };
    publishControllerSnapshot();
  }, [publishControllerSnapshot]);

  const setControllerStatusError = useCallback((error: ReviewError) => {
    liveStatusRef.current = {
      ...liveStatusRef.current,
      status: "error",
      error,
    };
    publishControllerSnapshot();
  }, [publishControllerSnapshot]);

  const setPhaseSafe = useCallback((next: LivePhase) => {
    phaseRef.current = next;
    setPhase(next);
    publishControllerSnapshot();
  }, [publishControllerSnapshot]);

  const addError = useCallback((error: ReviewError) => {
    setOperationErrors((current) => {
      if (current.some((item) => item.code === error.code && item.message === error.message)) {
        return current;
      }
      return [...current, error].slice(-6);
    });
  }, []);

  const setLiveIdentity = useCallback((next: RecordingKey) => {
    const previous = activeKeyRef.current;
    if (sameRecordingKey(previous, next)) return false;
    epochRef.current += 1;
    activeKeyRef.current = next;
    activeProjectRef.current = next.projectId;
    setSelection(next);
    setSegments([]);
    setTopic(emptyReadState());
    setSummaries(EMPTY_MEETING_SUMMARIES);
    setAsk(emptyReadState());
    setOperationErrors([]);
    setTranscriptLoading(true);
    setTranscriptIncomplete(false);
    lifecycleRef.current?.setKnownIdentity(next);
    publishControllerSnapshot();
    return true;
  }, [publishControllerSnapshot]);

  const onTranscript = useCallback((key: RecordingKey, view: TranscriptView) => {
    if (!sameRecordingKey(activeKeyRef.current, key)) return;
    setTranscriptLoading(false);
    setTranscriptIncomplete(
      view.capped || view.cappedRecordingIds.includes(key.recordingId),
    );
    setSegments((current) => mergeLiveSegments(current, persistedSegments(key, view)));
  }, []);

  const loadTranscriptForKey = useCallback(async (key: RecordingKey) => {
    const epoch = epochRef.current;
    setTranscriptLoading(true);
    try {
      const view = await listTranscriptSegments(key.projectId, key.recordingId);
      if (
        sameRecordingKey(activeKeyRef.current, key) &&
        epochRef.current === epoch
      ) {
        onTranscript(key, view);
      }
    } catch (error) {
      if (
        sameRecordingKey(activeKeyRef.current, key) &&
        epochRef.current === epoch
      ) {
        setTranscriptLoading(false);
        addError(normalizeReviewError(error));
      }
    }
  }, [addError, onTranscript]);

  const loadSummariesForKey = useCallback(async (key: RecordingKey) => {
    const epoch = epochRef.current;
    const identity = createIdentity(key, epoch, nextRequestId("summary"));
    setSummaries((current) => ({
      ...current,
      status: "loading",
      identity,
      error: null,
      data: sameRecordingKey(activeKeyRef.current, key) ? current.data : null,
    }));

    try {
      const result = await meetingSummaries(key.projectId, key.recordingId);
      const scoped = scopeMeetingSummaries(result, key.recordingId);
      setSummaries((current) => settleLiveRequest(
        current,
        identity,
        { status: "fulfilled", data: scoped },
      ));
    } catch (error) {
      const normalized = normalizeReviewError(error);
      setSummaries((current) => settleLiveRequest(
        current,
        identity,
        { status: "rejected", error: normalized },
      ));
      if (
        sameRecordingKey(activeKeyRef.current, key) &&
        epochRef.current === epoch
      ) {
        addError(normalized);
      }
    }
  }, [addError]);

  const loadSummaries = useCallback(async (pid: string, rid: string) => {
    const key = { projectId: pid, recordingId: rid };
    if (!sameRecordingKey(activeKeyRef.current, key)) return;
    await loadSummariesForKey(key);
  }, [loadSummariesForKey]);

  const applyAuthoritativeStatus = useCallback((status: LiveStatusOutput) => {
    setControllerStatus(status);
    if (status.active && status.projectId && status.recordingId) {
      const nextKey = {
        projectId: status.projectId,
        recordingId: status.recordingId,
      };
      const changed = setLiveIdentity(nextKey);
      if (changed) void loadSummariesForKey(nextKey);
      setPhaseSafe(status.stopping ? "stopping" : "listening");
    } else if (status.stopping) {
      setPhaseSafe("stopping");
    } else {
      setPhaseSafe(activeKeyRef.current ? "stopped" : "idle");
    }

    if (status.elapsedMs !== null) setElapsedMs(status.elapsedMs);
  }, [loadSummariesForKey, setControllerStatus, setLiveIdentity, setPhaseSafe]);

  const onStatusEvent = useCallback((event: LiveStatusEvent) => {
    const allowed: LivePhase[] = [
      "idle",
      "starting",
      "listening",
      "degraded",
      "stopping",
      "stopped",
      "error",
    ];
    const next = event.state === "recording" ? "listening" : event.state;
    if (allowed.includes(next as LivePhase)) {
      setPhaseSafe(next as LivePhase);
    } else {
      setPhaseSafe("error");
      addError({
        code: "LEGACY_COMMAND_FAILED",
        message: "Live status returned an unknown state.",
        retryable: false,
      });
    }
    setDevices((current) => ({
      mic: event.micDevice ?? current.mic,
      system: event.systemDevice ?? current.system,
    }));
  }, [addError, setPhaseSafe]);

  const onSegment = useCallback((event: LiveSegmentEvent) => {
    const key = activeKeyRef.current;
    if (!key || key.recordingId !== event.recordingId) return;
    setSegments((current) => mergeLiveSegments(current, [event]));
  }, []);

  const onTopic = useCallback((event: LiveTopicEvent) => {
    const key = activeKeyRef.current;
    if (!key || key.recordingId !== event.recordingId) return;
    setTopic({
      status: "ready",
      identity: createIdentity(key, epochRef.current, "topic"),
      data: event,
      error: null,
    });
  }, []);

  const onSummary = useCallback((payload: LiveSummaryEvent) => {
    const event = { payload };
    const key = activeKeyRef.current;
    if (!key || key.recordingId !== event.payload.recordingId) return;
    if (event.payload.state === "running") {
      setSummaries((current) => ({
        ...current,
        status: "loading",
        identity: createIdentity(key, epochRef.current, "summary-event"),
        error: null,
      }));
      return;
    }
    if (event.payload.state === "failed") {
      const error: ReviewError = {
        code: "PROVIDER_FAILED",
        message: "การสร้างสรุปในเครื่องไม่สำเร็จ",
        retryable: true,
      };
      setSummaries((current) => ({
        ...current,
        status: "error",
        identity: createIdentity(key, epochRef.current, "summary-event"),
        data: null,
        error,
      }));
      addError(error);
      return;
    }
    if (activeProjectRef.current) {
      void loadSummaries(activeProjectRef.current, event.payload.recordingId);
    }
  }, [addError, loadSummaries]);

  const onIncomplete = useCallback(() => {
    setTranscriptIncomplete(true);
    addError({
      code: "RESOURCE_LIMIT",
      message: "Live event bootstrap exceeded the 200-event buffer; displayed live text may be incomplete.",
      retryable: true,
    });
  }, [addError]);

  const onLifecycleError = useCallback((error: ReviewError) => {
    addError(error);
    setControllerStatusError(error);
  }, [addError, setControllerStatusError]);

  sinkRef.current = {
    onAuthoritativeStatus: applyAuthoritativeStatus,
    onStatusEvent,
    onSegment,
    onTopic,
    onSummary,
    onTranscript,
    onIncomplete,
    onError: onLifecycleError,
  };

  useEffect(() => {
    const lifecycle = createLiveEventLifecycle(
      {
        listen: subscribeLiveEvent,
        readStatus: liveMeetingStatus,
        readTranscript: (key) => listTranscriptSegments(key.projectId, key.recordingId),
      },
      {
        onAuthoritativeStatus: (status) => sinkRef.current.onAuthoritativeStatus(status),
        onStatusEvent: (event) => sinkRef.current.onStatusEvent(event),
        onSegment: (event) => sinkRef.current.onSegment(event),
        onTopic: (event) => sinkRef.current.onTopic(event),
        onSummary: (event) => sinkRef.current.onSummary(event),
        onTranscript: (key, view) => sinkRef.current.onTranscript(key, view),
        onIncomplete: () => sinkRef.current.onIncomplete(),
        onError: (error) => sinkRef.current.onError(error),
      },
    );
    lifecycleRef.current = lifecycle;
    void lifecycle.start().catch((error) => sinkRef.current.onError(normalizeReviewError(error)));
    return () => {
      lifecycle.dispose();
      lifecycleRef.current = null;
    };
  }, []);

  useEffect(() => {
    lifecycleRef.current?.setVisible(visible);
  }, [visible]);

  const requestStop = useCallback((): Promise<string> => {
    if (!stopRequestRef.current) {
      stopRequestRef.current = liveMeetingStop()
        .then((receipt) => {
          setPhaseSafe("stopping");
          return receipt;
        })
        .catch((error) => {
          stopRequestRef.current = null;
          const normalized = normalizeReviewError(error);
          setControllerStatusError(normalized);
          throw normalized;
        });
    }
    return stopRequestRef.current;
  }, [setControllerStatusError, setPhaseSafe]);

  const waitForInactive = useCallback((): Promise<LiveStatusOutput> => {
    if (!inactiveWaitRef.current) {
      const waiting = (async () => {
        await requestStop();
        return waitForCaptureInactive(liveMeetingStatus, {
          onStatus: applyAuthoritativeStatus,
        });
      })()
        .catch((error) => {
          const normalized = normalizeReviewError(error);
          addError(normalized);
          setControllerStatusError(normalized);
          setPhaseSafe("error");
          throw normalized;
        })
        .finally(() => {
          inactiveWaitRef.current = null;
          stopRequestRef.current = null;
        });
      inactiveWaitRef.current = waiting;
    }
    return inactiveWaitRef.current;
  }, [addError, applyAuthoritativeStatus, requestStop, setControllerStatusError, setPhaseSafe]);

  const start = useCallback(async (
    options: {
      projectId?: string;
      captureSystem?: boolean;
      language?: string;
      transcriptProfile?: "chunked" | "revisioned";
      micDeviceId?: string;
      systemDeviceId?: string;
    },
    closePlayer: CloseReviewPlayer,
  ): Promise<LiveStartOutput> => {
    setOperationErrors([]);
    try {
      const output = await runStartWithCloseAck(
        closePlayer,
        () => liveMeetingStart({
          projectId: options.projectId ?? projectId ?? undefined,
          captureSystem: options.captureSystem ?? true,
          language: options.language,
          transcriptProfile: options.transcriptProfile,
          micDeviceId: options.micDeviceId,
          systemDeviceId: options.systemDeviceId,
        }),
      );
      const key = { projectId: output.projectId, recordingId: output.recordingId };
      setLiveIdentity(key);
      lifecycleRef.current?.setKnownIdentity(key);
      setPhaseSafe("starting");
      setElapsedMs(0);
      if (output.warning) {
        addError({
          code: "LEGACY_COMMAND_FAILED",
          message: "Native capture returned a warning; recording continues.",
          retryable: true,
        });
      }
      setDevices({ mic: output.micDevice, system: output.systemDevice });
      void loadTranscriptForKey(key);
      void loadSummariesForKey(key);
      return output;
    } catch (error) {
      const normalized = normalizeReviewError(error);
      addError(normalized);
      setControllerStatusError(normalized);
      setPhaseSafe("error");
      throw normalized;
    }
  }, [
    addError,
    loadSummariesForKey,
    loadTranscriptForKey,
    projectId,
    setControllerStatusError,
    setLiveIdentity,
    setPhaseSafe,
  ]);

  const stop = useCallback(async (): Promise<string> => {
    try {
      const receipt = await requestStop();
      void waitForInactive().catch(() => undefined);
      return receipt;
    } catch (error) {
      const normalized = normalizeReviewError(error);
      addError(normalized);
      setControllerStatusError(normalized);
      setPhaseSafe("error");
      throw normalized;
    }
  }, [addError, requestStop, setControllerStatusError, setPhaseSafe, waitForInactive]);

  const stopAndLeave = useCallback(async (): Promise<LiveStatusOutput> => {
    try {
      return await waitForInactive();
    } catch (error) {
      const normalized = normalizeReviewError(error);
      addError(normalized);
      setControllerStatusError(normalized);
      throw normalized;
    }
  }, [addError, setControllerStatusError, waitForInactive]);

  stopAndLeaveDelegateRef.current = stopAndLeave;

  const askForRecording = useCallback(async (
    requestedSelection: RecordingKey,
    question: string,
    requestId: string,
  ): Promise<RecordingAnswer> => {
    const currentSelection = activeKeyRef.current;
    const epoch = epochRef.current;
    const identity = createIdentity(requestedSelection, epoch, requestId);
    const scopeError: ReviewError = {
      code: "SCOPE_MISMATCH",
      message: "The requested recording is not the active live recording.",
      retryable: false,
    };

    if (!sameRecordingKey(currentSelection, requestedSelection)) {
      throw scopeError;
    }

    setAsk({
      status: "loading",
      identity,
      data: null,
      error: null,
    });

    try {
      const result = await askRecording(
        requestedSelection.projectId,
        requestedSelection.recordingId,
        question,
        requestId,
      );
      if (
        result.projectId !== requestedSelection.projectId ||
        result.recordingId !== requestedSelection.recordingId ||
        result.requestId !== requestId ||
        result.graphPolicy !== "excluded" ||
        result.liveTailPolicy !== "excluded" ||
        result.sources.some((source) => (
          source.projectId !== requestedSelection.projectId ||
          source.recordingId !== requestedSelection.recordingId
        ))
      ) {
        throw {
          code: "MODEL_OUTPUT_INVALID",
          message: "Recording-scoped answer identity was invalid.",
          retryable: false,
        } satisfies ReviewError;
      }
      setAsk((current) => settleLiveRequest(
        current,
        identity,
        { status: "fulfilled", data: result },
      ));
      return result;
    } catch (error) {
      const normalized = normalizeReviewError(error);
      setAsk((current) => settleLiveRequest(
        current,
        identity,
        { status: "rejected", error: normalized },
      ));
      if (
        sameRecordingKey(activeKeyRef.current, requestedSelection) &&
        epochRef.current === epoch
      ) {
        addError(normalized);
      }
      throw normalized;
    }
  }, [addError]);

  const generateSummary = useCallback(async (): Promise<void> => {
    const key = activeKeyRef.current;
    if (!key) {
      const error: ReviewError = {
        code: "RECORDING_NOT_FOUND",
        message: "No live recording is selected.",
        retryable: false,
      };
      addError(error);
      throw error;
    }

    const epoch = epochRef.current;
    const identity = createIdentity(key, epoch, nextRequestId("summary"));
    setSummaries((current) => ({
      ...current,
      status: "loading",
      identity,
      error: null,
    }));

    try {
      await generateMeetingSummary(key.projectId, key.recordingId);
      if (
        sameRecordingKey(activeKeyRef.current, key) &&
        epochRef.current === epoch
      ) {
        await loadSummariesForKey(key);
      }
    } catch (error) {
      const normalized = normalizeReviewError(error);
      setSummaries((current) => settleLiveRequest(
        current,
        identity,
        { status: "rejected", error: normalized },
      ));
      if (
        sameRecordingKey(activeKeyRef.current, key) &&
        epochRef.current === epoch
      ) {
        addError(normalized);
      }
      throw normalized;
    }
  }, [addError, loadSummariesForKey]);

  const closeView = useCallback(() => {
    if (
      phaseRef.current === "starting" ||
      phaseRef.current === "listening" ||
      phaseRef.current === "degraded" ||
      phaseRef.current === "stopping"
    ) {
      return;
    }
    onClose();
  }, [onClose]);

  const capabilities: DesktopCapabilities = useMemo(() => ({
    capture: {
      available: nativeAvailable,
      reasonCode: nativeAvailable ? null : "NATIVE_UNAVAILABLE",
    },
    recordingAsk: {
      available: nativeAvailable && selection !== null,
      reasonCode: nativeAvailable ? (selection ? null : "RECORDING_NOT_FOUND") : "NATIVE_UNAVAILABLE",
    },
    summary: {
      available: nativeAvailable && selection !== null,
      reasonCode: nativeAvailable ? (selection ? null : "RECORDING_NOT_FOUND") : "NATIVE_UNAVAILABLE",
    },
  }), [nativeAvailable, selection]);

  const visibleAttribute = visible;
  return (
    <div
      className={"live-overlay" + (visibleAttribute ? "" : " live-overlay--hidden")}
      hidden={!visibleAttribute}
      aria-hidden={!visibleAttribute}
    >
      <div className="live-panel" role="dialog" aria-modal="true" aria-labelledby="live-workspace-title">
        <LiveWorkspace
          selection={selection}
          phase={phase}
          elapsedMs={elapsedMs}
          devices={devices}
          captureDevices={captureDevices}
          captureDevicesLoading={captureDevicesLoading}
          captureDevicesError={captureDevicesError}
          refreshCaptureDevices={refreshCaptureDevices}
          segmentFeed={segments}
          topic={topic}
          summaries={summaries}
          ask={ask}
          capabilities={capabilities}
          operationErrors={operationErrors}
          actions={{
            start,
            stop,
            stopAndLeave,
            ask: askForRecording,
            generateSummary,
            closeView,
          }}
          closeReviewPlayer={closeReviewPlayer}
          transcriptLoading={transcriptLoading}
          transcriptIncomplete={transcriptIncomplete}
          visible={visibleAttribute}
        />
      </div>
    </div>
  );
}
