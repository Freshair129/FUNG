import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import {
  askRecording as bridgeAskRecording,
  closePlayback as bridgeClosePlayback,
  controlPlayback as bridgeControlPlayback,
  correctTranscriptSegment,
  createJob,
  getPlayback as bridgeGetPlayback,
  getRecording as bridgeGetRecording,
  listExportArtifacts,
  listRecordings,
  listTranscriptSegments,
  meetingSummaries,
  openPlayback as bridgeOpenPlayback,
  releaseRecordingList,
  renameSpeaker,
  type ExportArtifact,
  type Job,
  type TranscriptSegment,
  type TranscriptView,
} from "../../tauri.ts";
import type { MeetingSummaries } from "../../lib/meetingSummaries.ts";
import {
  normalizeReviewError,
  settleReviewLoad,
  type CloseReviewPlayer,
  type PlaybackAction,
  type PlaybackChannel,
  type PlaybackCloseAcknowledgement,
  type PlaybackState,
  type ReadState,
  type RecordingAnswer,
  type RecordingKey,
  type RecordingReviewActions,
  type RecordingReviewProps,
  type RecordingRow,
  type RecordingReviewProps as FrozenRecordingReviewProps,
  type RecordingPage,
  type ReviewError,
  type ReviewErrorCode,
  type ReviewRequestIdentity,
} from "./contracts.ts";
import "./RecordingReview.css";

export const RECORDING_PAGE_LIMIT = 50;
export const PLAYBACK_POLL_INTERVAL_MS = 250;

export type RecordingReviewBridge = {
  listRecordings: typeof listRecordings;
  releaseRecordingList: typeof releaseRecordingList;
  getRecording: typeof bridgeGetRecording;
  listTranscriptSegments: typeof listTranscriptSegments;
  meetingSummaries: typeof meetingSummaries;
  listExportArtifacts: typeof listExportArtifacts;
  askRecording: typeof bridgeAskRecording;
  openPlayback: typeof bridgeOpenPlayback;
  controlPlayback: typeof bridgeControlPlayback;
  getPlayback: typeof bridgeGetPlayback;
  closePlayback: typeof bridgeClosePlayback;
  correctTranscriptSegment: typeof correctTranscriptSegment;
  renameSpeaker: typeof renameSpeaker;
  createJob: typeof createJob;
};

export const defaultRecordingReviewBridge: RecordingReviewBridge = {
  listRecordings,
  releaseRecordingList,
  getRecording: bridgeGetRecording,
  listTranscriptSegments,
  meetingSummaries,
  listExportArtifacts,
  askRecording: bridgeAskRecording,
  openPlayback: bridgeOpenPlayback,
  controlPlayback: bridgeControlPlayback,
  getPlayback: bridgeGetPlayback,
  closePlayback: bridgeClosePlayback,
  correctTranscriptSegment,
  renameSpeaker,
  createJob,
};

export type RecordingReviewControllerProps = {
  selectedProjectId: string | null;
  selection: RecordingKey | null;
  onSelect: (selection: RecordingKey | null) => void;
  scopeChoice?: "A" | "B";
  visible?: boolean;
  registerClosePlayer?: (
    closePlayer: CloseReviewPlayer | null,
  ) => void | (() => void);
  registerRecoveryRefresh?: (
    refreshRecovered: RecoveryRefresh | null,
  ) => void | (() => void);
  bridge?: RecordingReviewBridge;
};

export type RecoveryRefresh = (selection: RecordingKey) => Promise<void>;

export type RecordingReviewViewProps = FrozenRecordingReviewProps & {
  question: string;
  onQuestionChange: (question: string) => void;
  onAskQuestion: () => Promise<void>;
  visible?: boolean;
};

export type RecordingReviewControllerSnapshot = {
  recording: ReadState<RecordingRow>;
  list: ReadState<RecordingPage>;
  transcript: ReadState<TranscriptView>;
  summaries: ReadState<MeetingSummaries>;
  exportState: ReadState<ExportArtifact[]>;
  playback: ReadState<PlaybackState>;
  ask: ReadState<RecordingAnswer>;
  question: string;
};

type PollFence = {
  token: number;
  playerEpoch: number;
  handle: string;
  identity: ReviewRequestIdentity;
};

function idleReadState<T>(): ReadState<T> {
  return {
    status: "idle",
    identity: null,
    data: null,
    error: null,
  };
}

function loadingReadState<T>(
  identity: ReviewRequestIdentity,
  data: T | null = null,
): ReadState<T> {
  return {
    status: "loading",
    identity,
    data,
    error: null,
  };
}

function reviewError(
  code: ReviewErrorCode,
  message: string,
  retryable = false,
): ReviewError {
  return { code, message, retryable };
}

function statusForReviewError(error: ReviewError): ReadState<unknown>["status"] {
  return error.code === "NATIVE_UNAVAILABLE" ? "unavailable" : "error";
}

function sameKey(
  left: RecordingKey | null,
  right: RecordingKey | null,
): boolean {
  return (
    left?.projectId === right?.projectId &&
    left?.recordingId === right?.recordingId
  );
}

export function sameReviewIdentity(
  left: ReviewRequestIdentity | null,
  right: ReviewRequestIdentity | null,
): boolean {
  return (
    left?.projectId === right?.projectId &&
    left?.recordingId === right?.recordingId &&
    left?.selectionEpoch === right?.selectionEpoch &&
    left?.requestId === right?.requestId
  );
}

export function makeReviewRequestIdentity(
  key: RecordingKey,
  selectionEpoch: number,
  requestId: string,
): ReviewRequestIdentity {
  return {
    projectId: key.projectId,
    recordingId: key.recordingId,
    selectionEpoch,
    requestId,
  };
}

export function formatRecordingDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return (
      String(hours) +
      ":" +
      String(minutes).padStart(2, "0") +
      ":" +
      String(seconds).padStart(2, "0")
    );
  }
  return String(minutes) + ":" + String(seconds).padStart(2, "0");
}

export function clampPlaybackPosition(
  positionMs: number,
  durationMs: number,
): number {
  if (!Number.isFinite(positionMs)) return 0;
  if (!Number.isFinite(durationMs) || durationMs <= 0) return 0;
  return Math.min(Math.max(0, positionMs), durationMs);
}

export function playbackPositionForKey(
  key: string,
  positionMs: number,
  durationMs: number,
): number | null {
  if (key === "Home") return 0;
  if (key === "End") return Math.max(0, durationMs);
  if (key === "ArrowLeft") {
    return clampPlaybackPosition(positionMs - 5000, durationMs);
  }
  if (key === "ArrowRight") {
    return clampPlaybackPosition(positionMs + 5000, durationMs);
  }
  return null;
}

export function formatReviewDate(value: string): string {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "เวลาไม่ถูกต้อง";
  return new Date(timestamp).toLocaleString("th-TH", {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export function parseSummaryContent(content: string): {
  kind: "json" | "text" | "invalid";
  display: string;
} {
  const source = content.trim();
  if (source.length === 0) {
    return { kind: "text", display: "สรุปนี้ยังไม่มีเนื้อหา" };
  }
  try {
    const parsed: unknown = JSON.parse(source);
    return {
      kind: "json",
      display: JSON.stringify(parsed, null, 2),
    };
  } catch {
    return {
      kind: "invalid",
      display: content,
    };
  }
}

export function isRecordingAnswerFor(
  answer: RecordingAnswer,
  key: RecordingKey,
  requestId: string,
): boolean {
  return (
    answer.projectId === key.projectId &&
    answer.recordingId === key.recordingId &&
    answer.requestId === requestId &&
    answer.scope === "recording" &&
    answer.sources.every(
      (source) =>
        source.projectId === key.projectId &&
        source.recordingId === key.recordingId,
    )
  );
}

function validateRecordingRow(
  row: RecordingRow,
  expectedProjectId: string,
  expectedRecordingId?: string,
): ReviewError | null {
  if (
    typeof row.id !== "string" ||
    row.id.length === 0 ||
    typeof row.projectId !== "string" ||
    row.projectId !== expectedProjectId ||
    (expectedRecordingId !== undefined && row.id !== expectedRecordingId) ||
    typeof row.source !== "string" ||
    typeof row.status !== "string" ||
    !Number.isFinite(row.durationMs) ||
    row.durationMs < 0 ||
    typeof row.createdAt !== "string" ||
    !Number.isFinite(Date.parse(row.createdAt)) ||
    typeof row.updatedAt !== "string" ||
    !Number.isFinite(Date.parse(row.updatedAt)) ||
    !Array.isArray(row.channels)
  ) {
    return reviewError(
      row.projectId !== expectedProjectId
        ? "SCOPE_MISMATCH"
        : "INVALID_RECORDING_METADATA",
      row.projectId !== expectedProjectId
        ? "บันทึกนี้ไม่อยู่ในโครงการที่เลือก"
        : "ข้อมูลการบันทึกไม่สมบูรณ์",
    );
  }
  return null;
}

function validateRecordingPage(
  page: RecordingPage,
  expectedProjectId: string,
): ReviewError | null {
  if (
    page.projectId !== expectedProjectId ||
    typeof page.snapshotId !== "string" ||
    page.snapshotId.length === 0 ||
    typeof page.asOf !== "string" ||
    !Number.isFinite(Date.parse(page.asOf)) ||
    !Array.isArray(page.items) ||
    (page.nextCursor !== null && typeof page.nextCursor !== "string")
  ) {
    return reviewError(
      page.projectId !== expectedProjectId
        ? "SCOPE_MISMATCH"
        : "INVALID_RECORDING_METADATA",
      page.projectId !== expectedProjectId
        ? "ประวัติการบันทึกไม่ตรงกับโครงการที่เลือก"
        : "ข้อมูลหน้าประวัติการบันทึกไม่สมบูรณ์",
    );
  }
  for (const row of page.items) {
    const error = validateRecordingRow(row, expectedProjectId);
    if (error) return error;
  }
  return null;
}

function validateTranscriptFor(
  view: TranscriptView,
  key: RecordingKey,
): ReviewError | null {
  if (!Array.isArray(view.segments)) {
    return reviewError("INVALID_RECORDING_METADATA", "ข้อมูลบทถอดเสียงไม่สมบูรณ์");
  }
  for (const segment of view.segments) {
    if (
      segment.projectId !== key.projectId ||
      segment.recordingId !== key.recordingId
    ) {
      return reviewError(
        "SCOPE_MISMATCH",
        "บทถอดเสียงไม่ตรงกับการบันทึกที่เลือก",
      );
    }
  }
  return null;
}

function validateSummariesFor(
  value: MeetingSummaries,
  key: RecordingKey,
): ReviewError | null {
  if (!Array.isArray(value.rows)) {
    return reviewError("INVALID_RECORDING_METADATA", "ข้อมูลสรุปไม่สมบูรณ์");
  }
  for (const row of value.rows) {
    if (row.recordingId !== key.recordingId) {
      return reviewError("SCOPE_MISMATCH", "สรุปไม่ตรงกับการบันทึกที่เลือก");
    }
  }
  return null;
}

function validatePlaybackFor(
  value: PlaybackState,
  key: RecordingKey,
): ReviewError | null {
  if (
    typeof value.handle !== "string" ||
    value.handle.length === 0 ||
    value.projectId !== key.projectId ||
    value.recordingId !== key.recordingId ||
    !Number.isFinite(value.positionMs) ||
    !Number.isFinite(value.durationMs) ||
    value.durationMs < 0 ||
    !Number.isInteger(value.streamEpoch) ||
    value.streamEpoch < 0
  ) {
    return reviewError(
      value.projectId !== key.projectId ||
        value.recordingId !== key.recordingId
        ? "SCOPE_MISMATCH"
        : "INVALID_RECORDING_METADATA",
      "ข้อมูลเครื่องเล่นไม่ตรงกับการบันทึกที่เลือก",
    );
  }
  return null;
}

function createInitialSnapshot(): RecordingReviewControllerSnapshot {
  return {
    recording: idleReadState<RecordingRow>(),
    list: idleReadState<RecordingPage>(),
    transcript: idleReadState<TranscriptView>(),
    summaries: idleReadState<MeetingSummaries>(),
    exportState: idleReadState<ExportArtifact[]>(),
    playback: idleReadState<PlaybackState>(),
    ask: idleReadState<RecordingAnswer>(),
    question: "",
  };
}

export class RecordingReviewController {
  private readonly bridge: RecordingReviewBridge;
  private readonly listeners = new Set<
    (snapshot: RecordingReviewControllerSnapshot) => void
  >();
  private snapshotValue = createInitialSnapshot();
  private projectId: string | null = null;
  private currentSelection: RecordingKey | null = null;
  private onSelectCallback: ((selection: RecordingKey | null) => void) | null =
    null;
  private disposed = false;
  private visible = true;
  private requestSequence = 0;
  private selectionEpoch = 0;
  private listEpoch = 0;
  private snapshotId: string | null = null;
  private playerHandle: string | null = null;
  private playerIdentity: ReviewRequestIdentity | null = null;
  private playerEpoch = 0;
  private failedCloseCustody: {
    handle: string;
    identity: ReviewRequestIdentity | null;
  } | null = null;
  private closeInFlight: Promise<PlaybackCloseAcknowledgement> | null = null;
  private controlQueue: Promise<void> = Promise.resolve();
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private pollToken = 0;
  private pollInFlight: PollFence | null = null;

  public readonly closePlayer: CloseReviewPlayer = async () => {
    if (this.closeInFlight) return this.closeInFlight;

    const custody = this.failedCloseCustody;
    const handle = this.playerHandle ?? custody?.handle ?? null;
    const identity = this.playerIdentity ?? custody?.identity ?? null;
    const closeEpoch = ++this.playerEpoch;
    this.playerHandle = null;
    this.playerIdentity = null;
    this.stopPolling();

    if (!handle) {
      this.emit((current) => ({
        ...current,
        playback: idleReadState<PlaybackState>(),
      }));
      return { closed: true };
    }

    const request = this.startRequest(() => this.bridge.closePlayback(handle));
    this.closeInFlight = request
      .then((acknowledgement) => {
        if (acknowledgement.closed) {
          if (this.failedCloseCustody?.handle === handle) {
            this.failedCloseCustody = null;
          }
          if (this.canPublishPlaybackClose(identity, closeEpoch)) {
            this.emit((current) => ({
              ...current,
              playback: idleReadState<PlaybackState>(),
            }));
          }
          return acknowledgement;
        }

        const error = reviewError(
          "PLAYBACK_IO_FAILED",
          "เครื่องเล่นยังปิดไม่สมบูรณ์ จึงยังเริ่มการบันทึกไม่ได้",
          true,
        );
        this.failedCloseCustody = { handle, identity };
        this.publishPlaybackCloseFailure(identity, closeEpoch, error);
        return { closed: false };
      })
      .catch((error: unknown) => {
        const normalized = normalizeReviewError(error);
        this.failedCloseCustody = { handle, identity };
        this.publishPlaybackCloseFailure(identity, closeEpoch, normalized);
        return { closed: false };
      })
      .finally(() => {
        if (this.playerEpoch === closeEpoch || this.playerHandle === handle) {
          this.closeInFlight = null;
        }
      });

    return this.closeInFlight;
  };

  public constructor(
    bridge: RecordingReviewBridge = defaultRecordingReviewBridge,
  ) {
    this.bridge = bridge;
  }

  public get snapshot(): RecordingReviewControllerSnapshot {
    return this.snapshotValue;
  }

  public subscribe(
    listener: (snapshot: RecordingReviewControllerSnapshot) => void,
  ): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  public setOnSelect(
    onSelect: (selection: RecordingKey | null) => void,
  ): void {
    this.onSelectCallback = onSelect;
  }

  public syncScope(
    selectedProjectId: string | null,
    selection: RecordingKey | null,
    onSelect?: (selection: RecordingKey | null) => void,
  ): void {
    this.disposed = false;
    if (onSelect) this.onSelectCallback = onSelect;

    const projectChanged = selectedProjectId !== this.projectId;
    const selectionChanged = !sameKey(selection, this.currentSelection);
    if (!projectChanged && !selectionChanged) return;

    const previousProjectId = this.projectId;
    this.projectId = selectedProjectId;
    this.currentSelection = selection;

    if (projectChanged) {
      this.listEpoch += 1;
      const previousSnapshotId = this.snapshotId;
      this.snapshotId = null;
      if (previousSnapshotId) void this.releaseSnapshot(previousSnapshotId);
    }

    if (projectChanged || selectionChanged) {
      this.selectionEpoch += 1;
      void this.closePlayer();
      this.emit((current) => ({
        ...current,
        recording: selection
          ? loadingReadState(
              makeReviewRequestIdentity(
                selection,
                this.selectionEpoch,
                this.nextRequestId("selection"),
              ),
            )
          : idleReadState<RecordingRow>(),
        transcript: idleReadState<TranscriptView>(),
        summaries: idleReadState<MeetingSummaries>(),
        exportState: idleReadState<ExportArtifact[]>(),
        ask: idleReadState<RecordingAnswer>(),
        playback: idleReadState<PlaybackState>(),
        question: "",
      }));
    }

    if (!selectedProjectId) {
      this.emit((current) => ({
        ...current,
        list: idleReadState<RecordingPage>(),
      }));
      return;
    }

    if (projectChanged) {
      this.loadFirstPage(selectedProjectId);
    }

    if (selection) {
      if (selection.projectId !== selectedProjectId) {
        const identity = makeReviewRequestIdentity(
          selection,
          this.selectionEpoch,
          this.nextRequestId("selection"),
        );
        const error = reviewError(
          "SCOPE_MISMATCH",
          "การบันทึกที่เลือกไม่อยู่ในโครงการปัจจุบัน",
        );
        this.emit((current) => ({
          ...current,
          recording: {
            status: "error",
            identity,
            data: null,
            error,
          },
          transcript: {
            status: "error",
            identity,
            data: null,
            error,
          },
        }));
      } else {
        this.loadSelection(selection, true);
      }
    }
  }

  public setVisible(visible: boolean): void {
    this.visible = visible;
    if (!visible) void this.closePlayer();
  }

  public startPolling(): () => void {
    this.stopPolling();
    if (!this.visible || !this.playerHandle) return () => undefined;
    const token = ++this.pollToken;
    this.pollTimer = setInterval(() => {
      void this.pollPlayback(token);
    }, PLAYBACK_POLL_INTERVAL_MS);
    return () => {
      if (this.pollToken === token) this.stopPolling();
    };
  }

  public stopPolling(): void {
    this.pollToken += 1;
    this.pollInFlight = null;
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  public async refresh(): Promise<void> {
    const projectId = this.projectId;
    if (!projectId) return;

    this.listEpoch += 1;
    const previousSnapshotId = this.snapshotId;
    this.snapshotId = null;
    if (previousSnapshotId) void this.releaseSnapshot(previousSnapshotId);
    this.loadFirstPage(projectId);

    if (this.currentSelection) {
      this.loadSelection(this.currentSelection, false);
    }
  }

  public readonly refreshRecovered: RecoveryRefresh = async (selection) => {
    const selectionEpoch = this.selectionEpoch;
    if (
      this.disposed ||
      !sameKey(selection, this.currentSelection) ||
      selection.projectId !== this.projectId
    ) {
      return;
    }

    await this.refresh();

    if (
      this.disposed ||
      selectionEpoch !== this.selectionEpoch ||
      !sameKey(selection, this.currentSelection)
    ) {
      return;
    }
  };

  public async listNext(): Promise<void> {
    const projectId = this.projectId;
    const currentPage = this.snapshotValue.list.data;
    const cursor = currentPage?.nextCursor ?? null;
    const currentSnapshotId = this.snapshotId;
    if (
      !projectId ||
      !currentPage ||
      !cursor ||
      !currentSnapshotId ||
      currentPage.snapshotId !== currentSnapshotId ||
      this.snapshotValue.list.status === "loading"
    ) {
      return;
    }

    const identity = this.makeListIdentity(projectId);
    const previousData = currentPage;
    this.emit((current) => ({
      ...current,
      list: loadingReadState(identity, previousData),
    }));

    try {
      const page = await this.startRequest(() =>
        this.bridge.listRecordings(projectId, RECORDING_PAGE_LIMIT, cursor),
      );
      const validation = validateRecordingPage(page, projectId);
      if (
        !this.isCurrentListIdentity(identity, projectId) ||
        page.snapshotId !== currentSnapshotId
      ) {
        if (page.snapshotId !== currentSnapshotId) {
          void this.releaseSnapshot(page.snapshotId);
        }
        return;
      }
      if (validation) {
        this.commitList(identity, { status: "rejected", error: validation }, previousData);
        return;
      }
      const merged: RecordingPage = {
        ...page,
        items: [...previousData.items, ...page.items],
      };
      this.commitList(identity, { status: "fulfilled", data: merged });
    } catch (error: unknown) {
      if (!this.isCurrentListIdentity(identity, projectId)) return;
      this.commitList(
        identity,
        { status: "rejected", error: normalizeReviewError(error) },
        previousData,
      );
    }
  }

  public async select(selection: RecordingKey | null): Promise<void> {
    if (selection && selection.projectId !== this.projectId) return;
    const acknowledgement = await this.closePlayer();
    if (!acknowledgement.closed) return;
    this.onSelectCallback?.(selection);
  }

  public setQuestion(question: string): void {
    this.emit((current) => ({
      ...current,
      question,
      ask: idleReadState<RecordingAnswer>(),
    }));
  }

  public async ask(
    selection: RecordingKey,
    question: string,
    requestId: string,
  ): Promise<RecordingAnswer> {
    const trimmed = question.trim();
    const identity = makeReviewRequestIdentity(
      selection,
      this.selectionEpoch,
      requestId,
    );
    if (
      !sameKey(selection, this.currentSelection) ||
      selection.projectId !== this.projectId
    ) {
      const error = reviewError(
        "SCOPE_MISMATCH",
        "คำถามนี้ไม่ตรงกับการบันทึกที่เลือก",
      );
      this.commitAsk(identity, { status: "rejected", error });
      throw error;
    }
    if (trimmed.length === 0 || trimmed.length > 4000) {
      const error = reviewError(
        "INVALID_ARGUMENT",
        "คำถามต้องมีความยาว 1–4,000 ตัวอักษร",
      );
      this.commitAsk(identity, { status: "rejected", error });
      throw error;
    }

    this.emit((current) => ({
      ...current,
      ask: loadingReadState<RecordingAnswer>(identity),
    }));

    try {
      const answer = await this.startRequest(() =>
        this.bridge.askRecording(
          selection.projectId,
          selection.recordingId,
          trimmed,
          requestId,
        ),
      );
      const validation = isRecordingAnswerFor(answer, selection, requestId)
        ? null
        : reviewError("SCOPE_MISMATCH", "คำตอบไม่ตรงกับการบันทึกที่เลือก");
      if (validation) {
        this.commitAsk(identity, { status: "rejected", error: validation });
        throw validation;
      }
      this.commitAsk(identity, { status: "fulfilled", data: answer });
      return answer;
    } catch (error: unknown) {
      const normalized = normalizeReviewError(error);
      this.commitAsk(identity, { status: "rejected", error: normalized });
      throw error;
    }
  }

  public async openPlayback(
    selection: RecordingKey,
    channel: PlaybackChannel,
  ): Promise<PlaybackState> {
    if (
      this.disposed ||
      !sameKey(selection, this.currentSelection) ||
      selection.projectId !== this.projectId
    ) {
      throw reviewError("SCOPE_MISMATCH", "เครื่องเล่นไม่ตรงกับการบันทึกที่เลือก");
    }

    const closed = await this.closePlayer();
    if (!closed.closed) {
      throw reviewError(
        "PLAYBACK_IO_FAILED",
        "ปิดเครื่องเล่นเดิมไม่สำเร็จ จึงยังเปิดเสียงใหม่ไม่ได้",
        true,
      );
    }

    const identity = makeReviewRequestIdentity(
      selection,
      this.selectionEpoch,
      this.nextRequestId("playback-open"),
    );
    const openEpoch = ++this.playerEpoch;
    this.playerIdentity = identity;
    this.emit((current) => ({
      ...current,
      playback: loadingReadState<PlaybackState>(identity),
    }));

    try {
      const state = await this.startRequest(() =>
        this.bridge.openPlayback(
          selection.projectId,
          selection.recordingId,
          channel,
        ),
      );
      if (
        this.disposed ||
        openEpoch !== this.playerEpoch ||
        !sameKey(selection, this.currentSelection) ||
        !sameReviewIdentity(this.playerIdentity, identity)
      ) {
        await this.safeCloseReturnedHandle(state.handle);
        throw reviewError("PLAYBACK_STALE_EPOCH", "เครื่องเล่นเก่าไม่ถูกนำกลับมาใช้");
      }
      const validation = validatePlaybackFor(state, selection);
      if (validation) {
        this.commitPlayback(identity, { status: "rejected", error: validation });
        throw validation;
      }
      this.playerHandle = state.handle;
      this.emit((current) => ({
        ...current,
        playback: {
          status: state.error ? "error" : "ready",
          identity,
          data: state,
          error: state.error,
        },
      }));
      return state;
    } catch (error: unknown) {
      const stale =
        this.disposed ||
        openEpoch !== this.playerEpoch ||
        !sameReviewIdentity(this.playerIdentity, identity);
      if (!stale) {
        const normalized = normalizeReviewError(error);
        this.playerHandle = null;
        this.playerIdentity = null;
        this.commitPlayback(identity, {
          status: "rejected",
          error: normalized,
        });
      }
      throw error;
    }
  }

  public async playbackControl(
    handle: string,
    expectedEpoch: number,
    action: PlaybackAction,
    positionMs?: number,
  ): Promise<PlaybackState> {
    const operation = this.controlQueue.then(() =>
      this.runPlaybackControl(handle, expectedEpoch, action, positionMs),
    );
    this.controlQueue = operation.then(
      () => undefined,
      () => undefined,
    );
    return operation;
  }

  public async playbackClose(
    handle: string,
  ): Promise<PlaybackCloseAcknowledgement> {
    const ownedHandle = this.playerHandle ?? this.failedCloseCustody?.handle ?? null;
    if (ownedHandle && ownedHandle !== handle) {
      throw reviewError("PLAYBACK_HANDLE_INVALID", "เครื่องเล่นนี้ไม่ใช่ของหน้านี้");
    }
    return this.closePlayer();
  }

  public async correctSegment(
    selection: RecordingKey,
    segmentId: string,
    correctedText: string,
  ): Promise<void> {
    if (!sameKey(selection, this.currentSelection)) {
      throw reviewError("SCOPE_MISMATCH", "การแก้ไขไม่ตรงกับการบันทึกที่เลือก");
    }
    await this.startRequest(() =>
      this.bridge.correctTranscriptSegment(
        selection.projectId,
        selection.recordingId,
        segmentId,
        correctedText,
      ),
    );
    this.loadTranscript(selection);
  }

  public async renameSpeaker(
    speakerId: string,
    displayName: string,
  ): Promise<void> {
    await this.startRequest(() =>
      this.bridge.renameSpeaker(speakerId, displayName),
    );
    if (this.currentSelection) this.loadTranscript(this.currentSelection);
  }

  public async queueExistingJob(
    selection: RecordingKey,
    jobType: string,
  ): Promise<Job> {
    if (!sameKey(selection, this.currentSelection)) {
      throw reviewError("SCOPE_MISMATCH", "งานนี้ไม่ตรงกับการบันทึกที่เลือก");
    }
    return this.startRequest(() =>
      this.bridge.createJob(jobType, selection.projectId, selection.recordingId),
    );
  }

  public dispose(): void {
    if (this.disposed) return;
    this.stopPolling();
    const snapshotId = this.snapshotId;
    this.snapshotId = null;
    if (snapshotId) void this.releaseSnapshot(snapshotId);
    void this.closePlayer();
    this.disposed = true;
    this.listeners.clear();
  }

  public async pollNow(): Promise<void> {
    await this.pollPlayback(this.pollToken);
  }

  private nextRequestId(prefix: string): string {
    this.requestSequence += 1;
    return prefix + "-" + String(this.requestSequence);
  }

  private makeListIdentity(projectId: string): ReviewRequestIdentity {
    return makeReviewRequestIdentity(
      { projectId, recordingId: "" },
      this.listEpoch,
      this.nextRequestId("list"),
    );
  }

  private makeSelectionIdentity(
    key: RecordingKey,
    prefix: string,
  ): ReviewRequestIdentity {
    return makeReviewRequestIdentity(
      key,
      this.selectionEpoch,
      this.nextRequestId(prefix),
    );
  }

  private startRequest<T>(factory: () => Promise<T>): Promise<T> {
    return Promise.resolve().then(factory);
  }

  private emit(
    updater: (
      current: RecordingReviewControllerSnapshot,
    ) => RecordingReviewControllerSnapshot,
  ): void {
    if (this.disposed) return;
    this.snapshotValue = updater(this.snapshotValue);
    for (const listener of this.listeners) listener(this.snapshotValue);
  }

  private isCurrentListIdentity(
    identity: ReviewRequestIdentity,
    projectId: string,
  ): boolean {
    return (
      !this.disposed &&
      this.projectId === projectId &&
      this.snapshotValue.list.identity !== null &&
      sameReviewIdentity(this.snapshotValue.list.identity, identity)
    );
  }

  private isCurrentSelectionIdentity(
    identity: ReviewRequestIdentity,
  ): boolean {
    return (
      !this.disposed &&
      this.currentSelection !== null &&
      sameKey(this.currentSelection, {
        projectId: identity.projectId,
        recordingId: identity.recordingId,
      }) &&
      identity.selectionEpoch === this.selectionEpoch
    );
  }

  private canPublishPlaybackClose(
    identity: ReviewRequestIdentity | null,
    closeEpoch: number,
  ): boolean {
    return (
      identity !== null &&
      this.playerEpoch === closeEpoch &&
      this.isCurrentSelectionIdentity(identity)
    );
  }

  private publishPlaybackCloseFailure(
    identity: ReviewRequestIdentity | null,
    closeEpoch: number,
    error: ReviewError,
  ): void {
    if (!this.canPublishPlaybackClose(identity, closeEpoch)) return;
    this.emit((current) => ({
      ...current,
      playback: {
        status: statusForReviewError(error),
        identity,
        data: current.playback.data,
        error,
      },
    }));
  }

  private async releaseSnapshot(snapshotId: string): Promise<void> {
    try {
      await this.bridge.releaseRecordingList(snapshotId);
    } catch {
      // Release is best-effort after the owner has invalidated the snapshot.
      // A new refresh always creates a new snapshot rather than reusing it.
    }
  }

  private async safeCloseReturnedHandle(handle: string): Promise<void> {
    try {
      await this.bridge.closePlayback(handle);
    } catch {
      // The returned handle is never installed into current UI state.
    }
  }

  private loadFirstPage(projectId: string): void {
    const identity = this.makeListIdentity(projectId);
    const previousData = this.snapshotValue.list.data;
    this.emit((current) => ({
      ...current,
      list: loadingReadState(identity, previousData),
    }));

    void this.startRequest(() =>
      this.bridge.listRecordings(projectId, RECORDING_PAGE_LIMIT, null),
    )
      .then((page) => {
        const validation = validateRecordingPage(page, projectId);
        if (!this.isCurrentListIdentity(identity, projectId)) {
          if (typeof page.snapshotId === "string") {
            void this.releaseSnapshot(page.snapshotId);
          }
          return;
        }
        if (validation) {
          this.commitList(identity, { status: "rejected", error: validation }, previousData);
          return;
        }
        this.snapshotId = page.snapshotId;
        this.commitList(identity, { status: "fulfilled", data: page });
      })
      .catch((error: unknown) => {
        if (!this.isCurrentListIdentity(identity, projectId)) return;
        this.commitList(
          identity,
          { status: "rejected", error: normalizeReviewError(error) },
          previousData,
        );
      });
  }

  private loadSelection(
    selection: RecordingKey,
    resetPlayback: boolean,
  ): void {
    const recordingIdentity = this.makeSelectionIdentity(selection, "recording");
    const transcriptIdentity = this.makeSelectionIdentity(selection, "transcript");
    const summaryIdentity = this.makeSelectionIdentity(selection, "summary");
    const exportIdentity = this.makeSelectionIdentity(selection, "export");

    this.emit((current) => ({
      ...current,
      recording: loadingReadState(recordingIdentity),
      transcript: loadingReadState(transcriptIdentity),
      summaries: loadingReadState(summaryIdentity),
      exportState: loadingReadState(exportIdentity, current.exportState.data),
      ask: idleReadState<RecordingAnswer>(),
      playback: resetPlayback
        ? idleReadState<PlaybackState>()
        : current.playback,
    }));

    const recordingRequest = this.startRequest(() =>
      this.bridge.getRecording(selection.projectId, selection.recordingId),
    );
    const transcriptRequest = this.startRequest(() =>
      this.bridge.listTranscriptSegments(
        selection.projectId,
        selection.recordingId,
      ),
    );
    const summaryRequest = this.startRequest(() =>
      this.bridge.meetingSummaries(
        selection.projectId,
        selection.recordingId,
      ),
    );
    const exportRequest = this.startRequest(() =>
      this.bridge.listExportArtifacts(selection.projectId),
    );

    void recordingRequest.then((value) => {
      const validation = validateRecordingRow(
        value,
        selection.projectId,
        selection.recordingId,
      );
      this.commitRecording(
        recordingIdentity,
        validation
          ? { status: "rejected", error: validation }
          : { status: "fulfilled", data: value },
      );
    }).catch((error: unknown) => {
      this.commitRecording(recordingIdentity, {
        status: "rejected",
        error: normalizeReviewError(error),
      });
    });

    void transcriptRequest.then((value) => {
      const validation = validateTranscriptFor(value, selection);
      this.commitTranscript(
        transcriptIdentity,
        validation
          ? { status: "rejected", error: validation }
          : { status: "fulfilled", data: value },
      );
    }).catch((error: unknown) => {
      this.commitTranscript(transcriptIdentity, {
        status: "rejected",
        error: normalizeReviewError(error),
      });
    });

    void summaryRequest.then((value) => {
      const validation = validateSummariesFor(value, selection);
      this.commitSummaries(
        summaryIdentity,
        validation
          ? { status: "rejected", error: validation }
          : { status: "fulfilled", data: value },
      );
    }).catch((error: unknown) => {
      this.commitSummaries(summaryIdentity, {
        status: "rejected",
        error: normalizeReviewError(error),
      });
    });

    void exportRequest.then((value) => {
      this.commitExports(exportIdentity, {
        status: "fulfilled",
        data: value,
      });
    }).catch((error: unknown) => {
      this.commitExports(exportIdentity, {
        status: "rejected",
        error: normalizeReviewError(error),
      });
    });
  }

  private loadTranscript(selection: RecordingKey): void {
    if (!sameKey(selection, this.currentSelection)) return;
    const identity = this.makeSelectionIdentity(selection, "transcript");
    const previousData = this.snapshotValue.transcript.data;
    this.emit((current) => ({
      ...current,
      transcript: loadingReadState(identity, previousData),
    }));
    void this.startRequest(() =>
      this.bridge.listTranscriptSegments(
        selection.projectId,
        selection.recordingId,
      ),
    )
      .then((value) => {
        const validation = validateTranscriptFor(value, selection);
        this.commitTranscript(
          identity,
          validation
            ? { status: "rejected", error: validation }
            : { status: "fulfilled", data: value },
          previousData,
        );
      })
      .catch((error: unknown) => {
        this.commitTranscript(
          identity,
          { status: "rejected", error: normalizeReviewError(error) },
          previousData,
        );
      });
  }

  private commitState<T>(
    current: ReadState<T>,
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: T }
      | { status: "rejected"; error: ReviewError },
    previousData?: T | null,
  ): ReadState<T> {
    const settled = settleReviewLoad(current, { identity, outcome });
    if (!sameReviewIdentity(settled.identity, identity)) return current;
    if (outcome.status === "rejected") {
      return {
        ...settled,
        status: statusForReviewError(outcome.error),
        data: previousData === undefined ? settled.data : previousData,
      };
    }
    return settled;
  }

  private commitRecording(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: RecordingRow }
      | { status: "rejected"; error: ReviewError },
  ): void {
    this.emit((current) => ({
      ...current,
      recording: this.commitState(current.recording, identity, outcome),
    }));
  }

  private commitList(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: RecordingPage }
      | { status: "rejected"; error: ReviewError },
    previousData?: RecordingPage | null,
  ): void {
    this.emit((current) => ({
      ...current,
      list: this.commitState(current.list, identity, outcome, previousData),
    }));
  }

  private commitTranscript(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: TranscriptView }
      | { status: "rejected"; error: ReviewError },
    previousData?: TranscriptView | null,
  ): void {
    this.emit((current) => ({
      ...current,
      transcript: this.commitState(
        current.transcript,
        identity,
        outcome,
        previousData,
      ),
    }));
  }

  private commitSummaries(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: MeetingSummaries }
      | { status: "rejected"; error: ReviewError },
  ): void {
    this.emit((current) => ({
      ...current,
      summaries: this.commitState(current.summaries, identity, outcome),
    }));
  }

  private commitExports(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: ExportArtifact[] }
      | { status: "rejected"; error: ReviewError },
  ): void {
    this.emit((current) => ({
      ...current,
      exportState: this.commitState(current.exportState, identity, outcome),
    }));
  }

  private commitAsk(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: RecordingAnswer }
      | { status: "rejected"; error: ReviewError },
  ): void {
    this.emit((current) => ({
      ...current,
      ask: this.commitState(current.ask, identity, outcome),
    }));
  }

  private commitPlayback(
    identity: ReviewRequestIdentity,
    outcome:
      | { status: "fulfilled"; data: PlaybackState }
      | { status: "rejected"; error: ReviewError },
  ): void {
    this.emit((current) => ({
      ...current,
      playback: this.commitState(current.playback, identity, outcome),
    }));
  }

  private async runPlaybackControl(
    handle: string,
    expectedEpoch: number,
    action: PlaybackAction,
    positionMs?: number,
  ): Promise<PlaybackState> {
    const identity = this.playerIdentity;
    const currentData = this.snapshotValue.playback.data;
    if (
      !identity ||
      !currentData ||
      this.playerHandle !== handle ||
      currentData.handle !== handle ||
      !this.isCurrentSelectionIdentity(identity)
    ) {
      throw reviewError("PLAYBACK_HANDLE_INVALID", "เครื่องเล่นนี้หมดอายุแล้ว");
    }
    if (currentData.streamEpoch !== expectedEpoch) {
      const error = reviewError(
        "PLAYBACK_STALE_EPOCH",
        "คำสั่งเครื่องเล่นเก่า ไม่ได้ถูกนำไปใช้",
      );
      this.emit((current) => ({
        ...current,
        playback: {
          status: "error",
          identity,
          data: current.playback.data,
          error,
        },
      }));
      throw error;
    }

    const playerEpoch = this.playerEpoch;
    this.emit((current) => ({
      ...current,
      playback: loadingReadState(identity, currentData),
    }));

    try {
      const state = await this.startRequest(() =>
        this.bridge.controlPlayback(
          handle,
          expectedEpoch,
          action,
          positionMs,
        ),
      );
      if (
        playerEpoch !== this.playerEpoch ||
        this.playerHandle !== handle ||
        !sameReviewIdentity(this.playerIdentity, identity)
      ) {
        throw reviewError(
          "PLAYBACK_STALE_EPOCH",
          "คำสั่งเครื่องเล่นเก่า ไม่ได้ถูกนำไปใช้",
        );
      }
      const validation = validatePlaybackFor(state, {
        projectId: identity.projectId,
        recordingId: identity.recordingId,
      });
      if (validation) {
        this.commitPlayback(identity, { status: "rejected", error: validation });
        throw validation;
      }
      this.commitPlayback(identity, {
        status: "fulfilled",
        data: state,
      });
      return state;
    } catch (error: unknown) {
      if (
        playerEpoch !== this.playerEpoch ||
        this.playerHandle !== handle ||
        !sameReviewIdentity(this.playerIdentity, identity)
      ) {
        throw error;
      }
      const normalized = normalizeReviewError(error);
      this.commitPlayback(identity, {
        status: "rejected",
        error: normalized,
      });
      throw error;
    }
  }

  private async pollPlayback(token: number): Promise<void> {
    const handle = this.playerHandle;
    const identity = this.playerIdentity;
    const playerEpoch = this.playerEpoch;
    if (
      !handle ||
      !identity ||
      !this.visible ||
      token !== this.pollToken ||
      !this.isCurrentSelectionIdentity(identity) ||
      this.pollInFlight
    ) {
      return;
    }
    const fence: PollFence = { token, playerEpoch, handle, identity };
    this.pollInFlight = fence;
    try {
      const state = await this.startRequest(() =>
        this.bridge.getPlayback(handle),
      );
      if (
        token !== this.pollToken ||
        playerEpoch !== this.playerEpoch ||
        this.playerHandle !== handle ||
        !sameReviewIdentity(this.playerIdentity, identity)
      ) {
        return;
      }
      const validation = validatePlaybackFor(state, {
        projectId: identity.projectId,
        recordingId: identity.recordingId,
      });
      if (validation) {
        this.commitPlayback(identity, { status: "rejected", error: validation });
        return;
      }
      this.commitPlayback(identity, {
        status: "fulfilled",
        data: state,
      });
    } catch (error: unknown) {
      if (
        token !== this.pollToken ||
        playerEpoch !== this.playerEpoch ||
        this.playerHandle !== handle ||
        !sameReviewIdentity(this.playerIdentity, identity)
      ) {
        return;
      }
      this.commitPlayback(identity, {
        status: "rejected",
        error: normalizeReviewError(error),
      });
    } finally {
      if (this.pollInFlight === fence) {
        this.pollInFlight = null;
      }
    }
  }
}

function ReviewStateCard({
  title,
  detail,
  error,
  retryLabel,
  onRetry,
}: {
  title: string;
  detail?: string;
  error?: ReviewError | null;
  retryLabel?: string;
  onRetry?: () => void;
}) {
  return (
    <div className="recording-review__state-card" role={error ? "alert" : "status"}>
      <strong>{title}</strong>
      {detail ? <p>{detail}</p> : null}
      {error ? (
        <p className="recording-review__state-error">
          {error.message}
          {error.code === "CURSOR_EXPIRED"
            ? " กรุณารีเฟรชเพื่ออ่านชุดข้อมูลใหม่"
            : ""}
        </p>
      ) : null}
      {onRetry ? (
        <button type="button" className="recording-review__button" onClick={onRetry}>
          {retryLabel ?? "ลองอีกครั้ง"}
        </button>
      ) : null}
    </div>
  );
}

function TranscriptSegmentRow({
  segment,
  selection,
  actions,
}: {
  segment: TranscriptSegment;
  selection: RecordingKey;
  actions: RecordingReviewActions;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(segment.text);
  const [saving, setSaving] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [speakerDraft, setSpeakerDraft] = useState(segment.speakerName ?? "");
  const [localError, setLocalError] = useState<ReviewError | null>(null);

  useEffect(() => {
    setDraft(segment.text);
    setSpeakerDraft(segment.speakerName ?? "");
  }, [segment.id, segment.text, segment.speakerName]);

  const saveCorrection = async () => {
    setSaving(true);
    setLocalError(null);
    try {
      await actions.correctSegment(selection, segment.id, draft);
      setEditing(false);
    } catch (error: unknown) {
      setLocalError(normalizeReviewError(error));
    } finally {
      setSaving(false);
    }
  };

  const saveSpeaker = async () => {
    if (!segment.speakerId || speakerDraft.trim().length === 0) return;
    setSaving(true);
    setLocalError(null);
    try {
      await actions.renameSpeaker(segment.speakerId, speakerDraft.trim());
      setRenameOpen(false);
    } catch (error: unknown) {
      setLocalError(normalizeReviewError(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <article className="recording-review__segment">
      <div className="recording-review__segment-time">
        {formatRecordingDuration(segment.startMs)}
      </div>
      <div className="recording-review__segment-body">
        <div className="recording-review__segment-meta">
          <span>
            {segment.speakerName ??
              segment.speakerId ??
              "ช่องเสียงยังไม่ระบุผู้พูด"}
          </span>
          {segment.speakerId ? (
            <button
              type="button"
              className="recording-review__text-button"
              onClick={() => setRenameOpen((open) => !open)}
              aria-expanded={renameOpen}
            >
              เปลี่ยนชื่อ
            </button>
          ) : null}
        </div>
        {editing ? (
          <div className="recording-review__edit-stack">
            <label>
              <span className="recording-review__sr-only">
                แก้ไขข้อความช่วงเวลา {formatRecordingDuration(segment.startMs)}
              </span>
              <textarea
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                rows={3}
                disabled={saving}
              />
            </label>
            <div className="recording-review__inline-actions">
              <button
                type="button"
                className="recording-review__button recording-review__button--primary"
                onClick={() => void saveCorrection()}
                disabled={saving}
              >
                {saving ? "กำลังบันทึก…" : "บันทึกการแก้ไข"}
              </button>
              <button
                type="button"
                className="recording-review__button"
                onClick={() => {
                  setDraft(segment.text);
                  setEditing(false);
                }}
                disabled={saving}
              >
                ยกเลิก
              </button>
            </div>
          </div>
        ) : (
          <p className="recording-review__segment-text">{segment.text}</p>
        )}
        {renameOpen ? (
          <form
            className="recording-review__rename"
            onSubmit={(event) => {
              event.preventDefault();
              void saveSpeaker();
            }}
          >
            <label>
              <span>ชื่อที่แสดงของผู้พูด</span>
              <input
                value={speakerDraft}
                onChange={(event) => setSpeakerDraft(event.target.value)}
                disabled={saving}
              />
            </label>
            <button
              type="submit"
              className="recording-review__button"
              disabled={saving || speakerDraft.trim().length === 0}
            >
              บันทึกชื่อ
            </button>
          </form>
        ) : null}
        {localError ? (
          <p className="recording-review__inline-error" role="alert">
            {localError.message}
          </p>
        ) : null}
        {!editing ? (
          <button
            type="button"
            className="recording-review__text-button"
            onClick={() => setEditing(true)}
          >
            แก้ไขข้อความ
          </button>
        ) : null}
      </div>
    </article>
  );
}

function TranscriptPanel({
  state,
  selection,
  actions,
}: {
  state: ReadState<TranscriptView>;
  selection: RecordingKey;
  actions: RecordingReviewActions;
}) {
  if (state.status === "idle" || state.status === "loading") {
    return (
      <section className="recording-review__card" aria-busy={state.status === "loading"}>
        <div className="recording-review__card-heading">
          <h3>บทถอดเสียง</h3>
        </div>
        {state.status === "loading" ? (
          <p className="recording-review__muted">กำลังอ่านบทถอดเสียง…</p>
        ) : (
          <p className="recording-review__muted">เลือกการบันทึกเพื่ออ่านบทถอดเสียง</p>
        )}
      </section>
    );
  }
  if (state.status === "unavailable") {
    return (
      <section className="recording-review__card" aria-busy="false">
        <div className="recording-review__card-heading"><h3>บทถอดเสียง</h3></div>
        <ReviewStateCard
          title="ยังอ่านบทถอดเสียงจาก Desktop ไม่ได้"
          error={state.error}
          onRetry={() => void actions.refresh()}
        />
      </section>
    );
  }
  if (state.status === "error") {
    return (
      <section className="recording-review__card" aria-busy="false">
        <div className="recording-review__card-heading"><h3>บทถอดเสียง</h3></div>
        <ReviewStateCard
          title="อ่านบทถอดเสียงไม่สำเร็จ"
          error={state.error}
          onRetry={() => void actions.refresh()}
        />
        {state.data ? (
          <p className="recording-review__stale-note">
            กำลังแสดงข้อมูลเดิมที่อ่านได้ก่อนหน้านี้
          </p>
        ) : null}
      </section>
    );
  }

  const view = state.data;
  if (!view || view.segments.length === 0) {
    return (
      <section className="recording-review__card" aria-busy="false">
        <div className="recording-review__card-heading"><h3>บทถอดเสียง</h3></div>
        <p className="recording-review__empty">บันทึกนี้ยังไม่มีบทถอดเสียงที่อ่านได้</p>
      </section>
    );
  }

  return (
    <section className="recording-review__card" aria-busy="false">
      <div className="recording-review__card-heading">
        <div>
          <h3>บทถอดเสียง</h3>
          <p className="recording-review__eyebrow">
            {view.segments.length} ช่วง · คู่ข้อมูล {selection.projectId}/{selection.recordingId}
          </p>
        </div>
      </div>
      {view.capped ? (
        <p className="recording-review__notice" role="status">
          บทถอดเสียงยังไม่ครบ — storage อ่านได้ถึง {view.cap} ช่วงต่อครั้ง
        </p>
      ) : null}
      <div className="recording-review__transcript-list">
        {view.segments.map((segment) => (
          <TranscriptSegmentRow
            key={segment.id}
            segment={segment}
            selection={selection}
            actions={actions}
          />
        ))}
      </div>
    </section>
  );
}

function SummaryPanel({
  state,
  selection,
  actions,
}: {
  state: ReadState<MeetingSummaries>;
  selection: RecordingKey;
  actions: RecordingReviewActions;
}) {
  if (state.status === "idle" || state.status === "loading") {
    return (
      <section className="recording-review__card" aria-busy={state.status === "loading"}>
        <div className="recording-review__card-heading"><h3>สรุปของบันทึกนี้</h3></div>
        <p className="recording-review__muted">
          {state.status === "loading" ? "กำลังอ่านสรุป…" : "ยังไม่ได้อ่านสรุป"}
        </p>
      </section>
    );
  }
  if (state.status === "unavailable" || state.status === "error") {
    return (
      <section className="recording-review__card" aria-busy="false">
        <div className="recording-review__card-heading"><h3>สรุปของบันทึกนี้</h3></div>
        <ReviewStateCard
          title={state.status === "unavailable" ? "ยังอ่านสรุปจาก Desktop ไม่ได้" : "อ่านสรุปไม่สำเร็จ"}
          error={state.error}
          onRetry={() => void actions.refresh()}
        />
      </section>
    );
  }

  const summaries = state.data;
  if (!summaries || summaries.rows.length === 0) {
    return (
      <section className="recording-review__card" aria-busy="false">
        <div className="recording-review__card-heading"><h3>สรุปของบันทึกนี้</h3></div>
        <p className="recording-review__empty">ยังไม่มีสรุปที่ผูกกับการบันทึกนี้</p>
        <p className="recording-review__muted">
          สรุปจะอ่านด้วยคู่ข้อมูล {selection.projectId}/{selection.recordingId}
        </p>
      </section>
    );
  }

  return (
    <section className="recording-review__card" aria-busy="false">
      <div className="recording-review__card-heading">
        <div>
          <h3>สรุปของบันทึกนี้</h3>
          <p className="recording-review__eyebrow">
            แสดงเฉพาะ recordingId ของรายการที่เลือก
          </p>
        </div>
      </div>
      {summaries.otherRecordings > 0 ? (
        <p className="recording-review__notice" role="status">
          ไม่แสดงสรุปจากการบันทึกอื่นในโครงการนี้ {summaries.otherRecordings} รายการ
        </p>
      ) : null}
      {summaries.unattributable > 0 ? (
        <p className="recording-review__notice" role="status">
          มี {summaries.unattributable} รายการที่ยืนยันเจ้าของการบันทึกไม่ได้
          {summaries.attributionComplete ? "" : " และการอ่าน attribution ยังไม่ครบ"}
        </p>
      ) : null}
      <div className="recording-review__summary-list">
        {summaries.rows.map((row) => {
          const parsed = parseSummaryContent(row.content);
          return (
            <article key={row.id} className="recording-review__summary">
              <div className="recording-review__summary-heading">
                <strong>{row.kind}</strong>
                <span>{formatReviewDate(row.createdAt)}</span>
              </div>
              {parsed.kind === "invalid" ? (
                <p className="recording-review__inline-error" role="alert">
                  เนื้อหาสรุปส่วนนี้อ่านเป็นโครงสร้างไม่ได้ จึงแสดงข้อความดิบอย่างปลอดภัย
                </p>
              ) : null}
              <pre>{parsed.display}</pre>
            </article>
          );
        })}
      </div>
    </section>
  );
}

export function consumePlaybackEvent(action: Promise<unknown>): void {
  void action.catch(() => undefined);
}

function PlaybackPanel({
  recording,
  selection,
  state,
  actions,
}: {
  recording: RecordingRow;
  selection: RecordingKey;
  state: ReadState<PlaybackState>;
  actions: RecordingReviewActions;
}) {
  const firstChannel = recording.channels[0] ?? "mic";
  const [channel, setChannel] = useState<PlaybackChannel>(firstChannel);

  useEffect(() => {
    setChannel(recording.channels[0] ?? "mic");
  }, [recording.id]);

  const availableChannel = recording.channels.includes(channel)
    ? channel
    : firstChannel;
  const playback = state.data;
  const canOpen = recording.captureState === "inactive";
  const canSeek =
    playback !== null &&
    playback.durationMs > 0 &&
    state.status !== "loading" &&
    playback.error === null;
  const position = playback
    ? clampPlaybackPosition(playback.positionMs, playback.durationMs)
    : 0;

  const selectChannel = async (nextChannel: PlaybackChannel) => {
    if (!playback) {
      setChannel(nextChannel);
      return;
    }
    try {
      const acknowledgement = await actions.playbackClose(playback.handle);
      if (acknowledgement.closed) setChannel(nextChannel);
    } catch {
      // The controller has already surfaced the safe close failure.
    }
  };

  const control = (action: PlaybackAction, nextPosition?: number) => {
    if (!playback) return;
    consumePlaybackEvent(
      actions.playbackControl(
        playback.handle,
        playback.streamEpoch,
        action,
        nextPosition,
      ),
    );
  };

  const seekFromKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (!playback || !canSeek) return;
    const nextPosition = playbackPositionForKey(
      event.key,
      position,
      playback.durationMs,
    );
    if (nextPosition === null) return;
    event.preventDefault();
    control("seek", nextPosition);
  };

  return (
    <section className="recording-review__card recording-review__player" aria-busy={state.status === "loading"}>
      <div className="recording-review__card-heading">
        <div>
          <h3>เล่นเสียงจากบันทึก</h3>
          <p className="recording-review__muted">
            PCM16 WAV เท่านั้น · ไม่มี waveform จำลอง
          </p>
        </div>
        {playback ? (
          <span className="recording-review__player-state">{playback.state}</span>
        ) : null}
      </div>
      {recording.captureState !== "inactive" ? (
        <p className="recording-review__notice" role="status">
          เล่นเสียงไม่ได้ระหว่างบันทึก — รอให้สถานะหยุดสมบูรณ์ก่อน
        </p>
      ) : null}
      {recording.channels.length === 0 ? (
        <p className="recording-review__empty">
          บันทึกนี้ยังไม่มีช่องเสียงที่อ่านได้
        </p>
      ) : (
        <div className="recording-review__player-controls">
          <label>
            ช่องเสียง
            <select
              value={availableChannel}
              onChange={(event) =>
                void selectChannel(event.target.value as PlaybackChannel)
              }
              disabled={state.status === "loading"}
            >
              {recording.channels.map((item) => (
                <option key={item} value={item}>
                  {item === "mic" ? "ไมโครโฟน" : item === "system" ? "ระบบ" : "ไฟล์"}
                </option>
              ))}
            </select>
          </label>
          {!playback ? (
            <button
              type="button"
              className="recording-review__button recording-review__button--primary"
              onClick={() =>
                consumePlaybackEvent(
                  actions.playbackOpen(selection, availableChannel),
                )
              }
              disabled={!canOpen || state.status === "loading"}
            >
              เปิดเครื่องเล่น
            </button>
          ) : (
            <div className="recording-review__inline-actions">
              <button
                type="button"
                className="recording-review__button recording-review__button--primary"
                onClick={() => control("play")}
                disabled={state.status === "loading" || playback.error !== null}
              >
                เล่น
              </button>
              <button
                type="button"
                className="recording-review__button"
                onClick={() => control("pause")}
                disabled={state.status === "loading" || playback.error !== null}
              >
                หยุดชั่วคราว
              </button>
              <button
                type="button"
                className="recording-review__button"
                onClick={() =>
                  consumePlaybackEvent(actions.playbackClose(playback.handle))
                }
                disabled={state.status === "loading"}
              >
                ปิดเครื่องเล่น
              </button>
            </div>
          )}
        </div>
      )}
      {playback ? (
        <div className="recording-review__seek">
          <label htmlFor="recording-review-seek">
            ตำแหน่ง {formatRecordingDuration(position)} /{" "}
            {formatRecordingDuration(playback.durationMs)}
          </label>
          <input
            id="recording-review-seek"
            type="range"
            min={0}
            max={Math.max(0, playback.durationMs)}
            value={position}
            onChange={(event) =>
              control("seek", Number(event.target.value))
            }
            onKeyDown={seekFromKey}
            disabled={!canSeek}
            aria-valuetext={
              formatRecordingDuration(position) +
              " จาก " +
              formatRecordingDuration(playback.durationMs)
            }
          />
          <p className="recording-review__muted">
            ปุ่มลูกศรเลื่อนทีละ 5 วินาที · Home/End ไปต้นหรือท้ายเมื่อช่วงเสียงพร้อม
          </p>
        </div>
      ) : null}
      {playback?.degraded ? (
        <p className="recording-review__notice" role="status">
          มีช่วงเสียงหายหรืออ่านไม่ได้ ระบบคงเวลาเดิมและเติมความเงียบให้ช่วงที่หาย
        </p>
      ) : null}
      {playback?.missingRanges.length ? (
        <ul className="recording-review__missing-ranges">
          {playback.missingRanges.map((range) => (
            <li key={range.startMs + "-" + range.endMs}>
              {formatRecordingDuration(range.startMs)}–{formatRecordingDuration(range.endMs)}: {range.reason}
            </li>
          ))}
        </ul>
      ) : null}
      {state.status === "unavailable" ? (
        <ReviewStateCard
          title="ยังไม่รองรับการเล่นเสียงบน Desktop นี้"
          error={state.error}
        />
      ) : null}
      {state.status === "error" && state.error ? (
        <ReviewStateCard
          title="เล่นเสียงไม่ได้ แต่บทถอดเสียงและสรุปยังใช้งานได้"
          error={state.error}
          retryLabel="ลองเปิดเครื่องเล่นอีกครั้ง"
          onRetry={() =>
            consumePlaybackEvent(
              actions.playbackOpen(selection, availableChannel),
            )
          }
        />
  ) : null}
    </section>
  );
}

function AskPanel({
  state,
  question,
  selection,
  onQuestionChange,
  onAskQuestion,
}: {
  state: ReadState<RecordingAnswer>;
  question: string;
  selection: RecordingKey;
  onQuestionChange: (question: string) => void;
  onAskQuestion: () => Promise<void>;
}) {
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void onAskQuestion();
  };

  return (
    <section className="recording-review__card" aria-busy={state.status === "loading"}>
      <div className="recording-review__card-heading">
        <div>
          <h3>ถามเฉพาะบันทึกนี้</h3>
          <p className="recording-review__muted">
            คำตอบใช้บทถอดเสียงที่ตรงกับ {selection.projectId}/{selection.recordingId} เท่านั้น
          </p>
        </div>
      </div>
      <p id="recording-review-ask-caveat" className="recording-review__notice">
        ไม่ใช้กราฟหรือบริบทสด และไม่เรียก provider อื่นอัตโนมัติ
      </p>
      <form onSubmit={submit} className="recording-review__ask-form">
        <label htmlFor="recording-review-question">คำถาม</label>
        <textarea
          id="recording-review-question"
          value={question}
          onChange={(event) => onQuestionChange(event.target.value)}
          maxLength={4000}
          rows={4}
          aria-describedby="recording-review-ask-caveat"
          placeholder="ถามจากบทถอดเสียงของบันทึกนี้"
          disabled={state.status === "loading"}
        />
        <div className="recording-review__ask-footer">
          <span className="recording-review__muted">{question.length}/4,000</span>
          <button
            type="submit"
            className="recording-review__button recording-review__button--primary"
            disabled={state.status === "loading" || question.trim().length === 0}
          >
            {state.status === "loading" ? "กำลังค้น…" : "ถามบันทึกนี้"}
          </button>
        </div>
      </form>
      {state.status === "error" || state.status === "unavailable" ? (
        <ReviewStateCard
          title={
            state.status === "unavailable"
              ? "ยังถามจาก Desktop ไม่ได้"
              : "ถามบันทึกนี้ไม่สำเร็จ"
          }
          error={state.error}
        />
      ) : null}
      {state.status === "ready" && state.data ? (
        <div className="recording-review__answer" aria-live="polite">
          <div className="recording-review__answer-heading">
            <strong>
              {state.data.status === "insufficient_evidence"
                ? "ยังไม่มีหลักฐานเพียงพอในบันทึกนี้"
                : "คำตอบจากบันทึกนี้"}
            </strong>
            {state.data.model ? <span>{state.data.model} · local</span> : null}
          </div>
          {state.data.answer ? <p>{state.data.answer}</p> : null}
          {state.data.sources.length > 0 ? (
            <div className="recording-review__sources">
              <h4>ช่วงที่ใช้เป็นหลักฐาน</h4>
              {state.data.sources.map((source) => (
                <blockquote key={source.segmentId}>
                  <span>
                    {formatRecordingDuration(source.startMs)}–{formatRecordingDuration(source.endMs)}
                  </span>
                  <p>{source.text}</p>
                </blockquote>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

function ExportPanel({
  state,
  selection,
}: {
  state: ReadState<ExportArtifact[]>;
  selection: RecordingKey;
}) {
  if (state.status === "loading" || state.status === "idle") {
    return (
      <section className="recording-review__card" aria-busy={state.status === "loading"}>
        <div className="recording-review__card-heading"><h3>ไฟล์ส่งออกทั้งโครงการ</h3></div>
        <p className="recording-review__muted">
          {state.status === "loading" ? "กำลังอ่านรายการไฟล์…" : "ยังไม่ได้อ่านรายการไฟล์"}
        </p>
      </section>
    );
  }
  if (state.status === "error" || state.status === "unavailable") {
    return (
      <section className="recording-review__card" aria-busy="false">
        <div className="recording-review__card-heading"><h3>ไฟล์ส่งออกทั้งโครงการ</h3></div>
        <ReviewStateCard
          title={state.status === "unavailable" ? "ยังอ่านไฟล์ส่งออกจาก Desktop ไม่ได้" : "อ่านไฟล์ส่งออกไม่สำเร็จ"}
          error={state.error}
        />
      </section>
    );
  }

  const artifacts = state.data ?? [];
  return (
    <section className="recording-review__card" aria-busy="false">
      <div className="recording-review__card-heading">
        <div>
          <h3>ไฟล์ส่งออกทั้งโครงการ</h3>
          <p className="recording-review__muted">
            รายการนี้เป็นขอบเขตทั้งโครงการ ไม่ใช่หลักฐานว่าไฟล์ทุกไฟล์มาจาก {selection.recordingId}
          </p>
        </div>
      </div>
      {artifacts.length === 0 ? (
        <p className="recording-review__empty">ยังไม่มีไฟล์ส่งออกของโครงการนี้</p>
      ) : (
        <ul className="recording-review__artifact-list">
          {artifacts.map((artifact) => (
            <li key={artifact.id}>
              <span>{artifact.kind}</span>
              <time dateTime={artifact.createdAt}>{formatReviewDate(artifact.createdAt)}</time>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function ReviewActionsPanel({
  selection,
  actions,
}: {
  selection: RecordingKey;
  actions: RecordingReviewActions;
}) {
  const [pending, setPending] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const queue = async (jobType: string, label: string) => {
    setPending(jobType);
    setNotice(null);
    try {
      const job = await actions.queueExistingJob(selection, jobType);
      setNotice("สร้างคำขอ" + label + "แล้ว สถานะงาน: " + job.status);
    } catch (error: unknown) {
      setNotice(normalizeReviewError(error).message);
    } finally {
      setPending(null);
    }
  };

  return (
    <section className="recording-review__card recording-review__actions-card">
      <div className="recording-review__card-heading">
        <div>
          <h3>งานของการบันทึกนี้</h3>
          <p className="recording-review__muted">
            งานส่งออกจับคู่ {selection.projectId}/{selection.recordingId} ตอนกดปุ่ม
          </p>
        </div>
      </div>
      <div className="recording-review__action-grid">
        <button
          type="button"
          className="recording-review__button recording-review__button--primary"
          onClick={() => void queue("export.render", "ส่งออก")}
          disabled={pending !== null}
        >
          ส่งออกบันทึกนี้
        </button>
        <button
          type="button"
          className="recording-review__button"
          onClick={() => void queue("summary.generate", "สร้างสรุป")}
          disabled={pending !== null}
        >
          สร้างสรุปใหม่
        </button>
        <button
          type="button"
          className="recording-review__button"
          onClick={() => void queue("transcript.retry", "ถอดเสียงใหม่")}
          disabled={pending !== null}
        >
          ถอดเสียงใหม่
        </button>
        <button
          type="button"
          className="recording-review__button"
          onClick={() => void queue("speakers.diarize", "แยกผู้พูด")}
          disabled={pending !== null}
        >
          แยกผู้พูด
        </button>
      </div>
      {pending ? <p className="recording-review__muted" role="status">กำลังส่งคำขอ…</p> : null}
      {notice ? <p className="recording-review__notice" role="status">{notice}</p> : null}
      <p className="recording-review__muted">
        เครื่องมือภายนอกยังอยู่ในพื้นที่เดิมและต้องผ่านขั้นตอนอนุมัติเดิม
      </p>
    </section>
  );
}

export function RecordingReviewView({
  selection,
  recording,
  list,
  transcript,
  summaries,
  exportState,
  playback,
  ask,
  scopeChoice,
  actions,
  question,
  onQuestionChange,
  onAskQuestion,
  visible = true,
}: RecordingReviewViewProps) {
  const selectedProjectId = selection?.projectId ?? list.data?.projectId ?? null;
  const hasStaleList = list.status === "error" && list.data !== null;

  return (
    <section
      className="recording-review"
      aria-label="พื้นที่รีวิวการบันทึก"
      hidden={!visible}
      data-scope={scopeChoice}
    >
      <header className="recording-review__header">
        <div>
          <p className="recording-review__eyebrow">Quiet Archive · FUNG</p>
          <h2 id="recording-review-heading">บันทึกย้อนหลัง</h2>
          <p className="recording-review__intro">
            เลือกการบันทึกจริงจากโครงการ แล้วอ่าน ฟัง และถามจากคู่ข้อมูลเดียวกัน
          </p>
        </div>
        <button
          type="button"
          className="recording-review__button"
          onClick={() => void actions.refresh()}
          disabled={!selectedProjectId || list.status === "loading"}
          aria-label="รีเฟรชประวัติบันทึก"
        >
          รีเฟรช
        </button>
      </header>

      <div className="recording-review__layout">
        <aside className="recording-review__list-panel" aria-labelledby="recording-review-list-heading">
          <div className="recording-review__panel-heading">
            <div>
              <p className="recording-review__eyebrow">โครงการที่เลือก</p>
              <h3 id="recording-review-list-heading">
                {selectedProjectId ?? "ยังไม่ได้เลือกโครงการ"}
              </h3>
            </div>
            <span className="recording-review__scope-badge">scope B</span>
          </div>

          <div
            className="recording-review__list"
            aria-busy={list.status === "loading"}
            aria-live="polite"
          >
            {!selectedProjectId ? (
              <ReviewStateCard
                title="เลือกโครงการก่อน"
                detail="ประวัติบันทึกจะอ่านจากโครงการที่เลือกเท่านั้น"
              />
            ) : list.status === "idle" || (list.status === "loading" && !list.data) ? (
              <ReviewStateCard
                title="กำลังอ่านประวัติ…"
                detail="กำลังขอรายการบันทึกจริงจาก Desktop"
              />
            ) : list.status === "unavailable" ? (
              <ReviewStateCard
                title="ยังอ่านประวัติบันทึกจาก Desktop ไม่ได้"
                error={list.error}
                onRetry={() => void actions.refresh()}
              />
            ) : list.status === "error" && !list.data ? (
              <ReviewStateCard
                title="อ่านประวัติบันทึกไม่สำเร็จ"
                error={list.error}
                onRetry={() => void actions.refresh()}
              />
            ) : list.data && list.data.items.length === 0 ? (
              <ReviewStateCard
                title="ยังไม่มีบันทึกย้อนหลังในโครงการนี้"
                detail="เริ่มบันทึกหรือนำเข้าเสียงจากพื้นที่ทำงานเดิมเพื่อให้รายการปรากฏ"
              />
            ) : (
              <>
                {hasStaleList ? (
                  <div className="recording-review__list-warning" role="alert">
                    <strong>รีเฟรชประวัติไม่สำเร็จ</strong>
                    <span>{list.error?.message ?? "ยังคงแสดงข้อมูลเดิม"}</span>
                    <button
                      type="button"
                      className="recording-review__text-button"
                      onClick={() => void actions.refresh()}
                    >
                      ลองรีเฟรชอีกครั้ง
                    </button>
                  </div>
                ) : null}
                {list.data?.items.map((item) => {
                  const active =
                    selection?.projectId === item.projectId &&
                    selection.recordingId === item.id;
                  return (
                    <button
                      type="button"
                      key={item.projectId + ":" + item.id}
                      className={
                        "recording-review__recording-row" +
                        (active ? " is-selected" : "")
                      }
                      aria-pressed={active}
                      onClick={() =>
                        void actions.select({
                          projectId: item.projectId,
                          recordingId: item.id,
                        })
                      }
                    >
                      <span className="recording-review__recording-row-topline">
                        <strong>{item.source || "บันทึกเสียง"}</strong>
                        <span>{formatRecordingDuration(item.durationMs)}</span>
                      </span>
                      <span className="recording-review__recording-row-meta">
                        {formatReviewDate(item.createdAt)} · {item.status}
                      </span>
                      <span className="recording-review__recording-row-meta">
                        {item.captureState === "inactive"
                          ? "หยุดแล้ว"
                          : item.captureState === "stopping"
                            ? "กำลังหยุด"
                            : "กำลังบันทึก"}{" "}
                        · {item.channels.length > 0 ? item.channels.join(" / ") : "ยังไม่มีช่องเสียง"}
                      </span>
                    </button>
                  );
                })}
                {list.data?.nextCursor ? (
                  <button
                    type="button"
                    className="recording-review__button recording-review__next"
                    onClick={() => void actions.listNext()}
                    disabled={list.status === "loading"}
                    aria-busy={list.status === "loading"}
                  >
                    {list.status === "loading" ? "กำลังอ่านหน้าถัดไป…" : "อ่านรายการถัดไป"}
                  </button>
                ) : null}
              </>
            )}
          </div>
        </aside>

        <main className="recording-review__detail" aria-labelledby="recording-review-heading">
          {!selection ? (
            <div className="recording-review__welcome">
              <p className="recording-review__eyebrow">อ่านย้อนหลังแบบเลือกเอง</p>
              <h3>เลือกบันทึกเพื่อเปิดรายละเอียด</h3>
              <p>
                การเลือกนี้เป็นแบบอ่านอย่างเดียว ไม่เริ่มอัด ไม่สร้างงาน และไม่เปิดเสียงอัตโนมัติ
              </p>
            </div>
          ) : recording.status === "idle" || (recording.status === "loading" && !recording.data) ? (
            <ReviewStateCard
              title="กำลังอ่านรายละเอียดการบันทึก…"
              detail="ตรวจสอบคู่ข้อมูลก่อนแสดงบทถอดเสียงและสรุป"
            />
          ) : recording.status === "unavailable" ? (
            <ReviewStateCard
              title="ยังเปิดรายละเอียดจาก Desktop ไม่ได้"
              error={recording.error}
              onRetry={() => void actions.refresh()}
            />
          ) : recording.status === "error" && !recording.data ? (
            <ReviewStateCard
              title="เปิดรายละเอียดการบันทึกไม่สำเร็จ"
              error={recording.error}
              onRetry={() => void actions.refresh()}
            />
          ) : recording.data ? (
            <div className="recording-review__detail-stack">
              <header className="recording-review__detail-header">
                <div>
                  <p className="recording-review__eyebrow">การบันทึกที่เลือก</p>
                  <h3>{recording.data.source || recording.data.id}</h3>
                  <p className="recording-review__muted">
                    {formatReviewDate(recording.data.createdAt)} ·{" "}
                    {formatRecordingDuration(recording.data.durationMs)} ·{" "}
                    {recording.data.status}
                  </p>
                </div>
                <span className="recording-review__capture-state">
                  {recording.data.captureState === "inactive"
                    ? "บันทึกเสร็จแล้ว"
                    : recording.data.captureState === "stopping"
                      ? "กำลังหยุด"
                      : "ยังบันทึกอยู่"}
                </span>
              </header>
              <PlaybackPanel
                recording={recording.data}
                selection={selection}
                state={playback}
                actions={actions}
              />
              <TranscriptPanel
                state={transcript}
                selection={selection}
                actions={actions}
              />
              <SummaryPanel
                state={summaries}
                selection={selection}
                actions={actions}
              />
              <AskPanel
                state={ask}
                question={question}
                selection={selection}
                onQuestionChange={onQuestionChange}
                onAskQuestion={onAskQuestion}
              />
              <ExportPanel state={exportState} selection={selection} />
              <ReviewActionsPanel selection={selection} actions={actions} />
            </div>
          ) : null}
        </main>
      </div>
    </section>
  );
}

export function RecordingReview({
  selectedProjectId,
  selection,
  onSelect,
  scopeChoice = "B",
  visible = true,
  registerClosePlayer,
  registerRecoveryRefresh,
  bridge = defaultRecordingReviewBridge,
}: RecordingReviewControllerProps) {
  const controllerRef = useRef<RecordingReviewController | null>(null);
  if (controllerRef.current === null) {
    controllerRef.current = new RecordingReviewController(bridge);
  }
  const controller = controllerRef.current;
  const [snapshot, setSnapshot] = useState<RecordingReviewControllerSnapshot>(
    controller.snapshot,
  );

  useEffect(() => controller.subscribe(setSnapshot), [controller]);
  useEffect(() => {
    controller.syncScope(selectedProjectId, selection, onSelect);
  }, [
    controller,
    onSelect,
    selectedProjectId,
    selection?.projectId,
    selection?.recordingId,
  ]);
  useEffect(() => {
    controller.setVisible(visible);
  }, [controller, visible]);
  useEffect(() => {
    if (!visible || !snapshot.playback.data?.handle) return undefined;
    return controller.startPolling();
  }, [controller, snapshot.playback.data?.handle, visible]);
  useEffect(() => {
    if (!registerClosePlayer) return undefined;
    const unregister = registerClosePlayer(controller.closePlayer);
    if (typeof unregister === "function") return unregister;
    return () => registerClosePlayer(null);
  }, [controller, registerClosePlayer]);
  useEffect(() => {
    if (!registerRecoveryRefresh) return undefined;
    let active = true;
    const refreshRecovered: RecoveryRefresh = (recoveredSelection) => {
      if (!active) return Promise.resolve();
      return controller.refreshRecovered(recoveredSelection);
    };
    const unregister = registerRecoveryRefresh(refreshRecovered);
    return () => {
      active = false;
      if (typeof unregister === "function") {
        unregister();
      } else {
        registerRecoveryRefresh(null);
      }
    };
  }, [controller, registerRecoveryRefresh]);
  useEffect(() => () => controller.dispose(), [controller]);

  const actions = useMemo<RecordingReviewActions>(
    () => ({
      refresh: () => controller.refresh(),
      listNext: () => controller.listNext(),
      select: (nextSelection) => controller.select(nextSelection),
      correctSegment: (key, segmentId, correctedText) =>
        controller.correctSegment(key, segmentId, correctedText),
      renameSpeaker: (speakerId, displayName) =>
        controller.renameSpeaker(speakerId, displayName),
      queueExistingJob: (key, jobType) =>
        controller.queueExistingJob(key, jobType),
      ask: (key, question, requestId) =>
        controller.ask(key, question, requestId),
      playbackOpen: (key, channel) =>
        controller.openPlayback(key, channel),
      playbackControl: (handle, expectedEpoch, action, positionMs) =>
        controller.playbackControl(handle, expectedEpoch, action, positionMs),
      playbackClose: (handle) => controller.playbackClose(handle),
    }),
    [controller],
  );

  const onAskQuestion = useCallback(async () => {
    if (!selection) return;
    const requestId =
      "ask-" +
      String(Date.now()) +
      "-" +
      String(Math.random()).slice(2, 8);
    try {
      await controller.ask(selection, snapshot.question, requestId);
    } catch {
      // The controller keeps the question and publishes the safe error state.
    }
  }, [controller, selection, snapshot.question]);

  return (
    <RecordingReviewView
      selection={selection}
      recording={snapshot.recording}
      list={snapshot.list}
      transcript={snapshot.transcript}
      summaries={snapshot.summaries}
      exportState={snapshot.exportState}
      playback={snapshot.playback}
      ask={snapshot.ask}
      scopeChoice={scopeChoice}
      actions={actions}
      question={snapshot.question}
      onQuestionChange={(question) => controller.setQuestion(question)}
      onAskQuestion={onAskQuestion}
      visible={visible}
    />
  );
}
