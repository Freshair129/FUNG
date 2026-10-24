import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { ExternalMeetingToolsPanel } from "../ExternalMeetingToolsPanel";
import type { LiveSegmentEvent } from "../../tauri";
import type { MeetingSummaries, SummaryRow } from "../../lib/meetingSummaries";
import type {
  CloseReviewPlayer,
  LiveWorkspaceProps,
  RecordingAnswerSource,
  ReviewError,
  ReviewErrorCode,
} from "./contracts";
import "./LiveWorkspace.css";

function scaleLivePeak(value: number): number {
  const clamped = Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0));
  return Math.round(Math.sqrt(clamped / 100) * 100);
}

function appendLivePeak(history: readonly number[], value: number, limit: number): number[] {
  const next = history.length >= limit ? history.slice(history.length - limit + 1) : history.slice();
  next.push(value);
  return next;
}

export type LiveWorkspaceAdapterProps = {
  closeReviewPlayer?: CloseReviewPlayer;
  transcriptLoading?: boolean;
  transcriptIncomplete?: boolean;
  visible?: boolean;
};

export type LiveSummaryDisclosure = {
  key: string;
  tone: "muted" | "notice";
  message: string;
};

export type LiveSummaryPresentation = {
  currentRows: SummaryRow[];
  disclosures: LiveSummaryDisclosure[];
};

function LiveChannelWaveform({
  label,
  sample,
  history,
}: {
  label: string;
  sample: LiveWorkspaceProps["liveAudioLevels"]["mic"];
  history: readonly number[];
}) {
  return (
    <section className="live-workspace__channel-waveform" aria-label={`waveform ${label}`}>
      <div className="live-workspace__waveform-heading">
        <h3>{label}</h3>
        <span>{sample ? `${sample.levelPercent}%` : "รอสัญญาณเสียง"}</span>
      </div>
      <div
        className="live-workspace__waveform-bars"
        role="meter"
        aria-label={`ระดับเสียง ${label}`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={sample?.levelPercent ?? 0}
      >
        {history.length === 0 ? (
          <span className="live-workspace__waveform-empty">ยังไม่มีตัวอย่างเสียงจากช่องนี้</span>
        ) : history.map((level, index) => (
          <span
            key={`${label}-${index}`}
            className="live-workspace__waveform-bar"
            style={{ height: `${Math.max(2, level)}%` }}
            aria-hidden="true"
          />
        ))}
      </div>
    </section>
  );
}

const PHASE_LABELS: Record<string, string> = {
  idle: "พร้อมเริ่มประชุม",
  starting: "กำลังเริ่มบันทึก",
  listening: "กำลังบันทึก",
  degraded: "กำลังบันทึก — การถอดสดมีปัญหา",
  transcribing: "กำลังประมวลผลเสียงช่วงสุดท้าย",
  stopping: "กำลังหยุดบันทึก",
  stopped: "หยุดบันทึกแล้ว",
  error: "เกิดข้อผิดพลาด",
};

const ERROR_LABELS: Record<ReviewErrorCode, string> = {
  INVALID_ARGUMENT: "ข้อมูลที่ส่งไม่ถูกต้อง",
  PROJECT_NOT_FOUND: "ไม่พบโครงการ",
  RECORDING_NOT_FOUND: "ไม่พบการบันทึก",
  SCOPE_MISMATCH: "ขอบเขตการบันทึกไม่ตรงกัน",
  STORAGE_READ_FAILED: "อ่านข้อมูลในเครื่องไม่สำเร็จ",
  INVALID_RECORDING_METADATA: "ข้อมูลการบันทึกไม่สมบูรณ์",
  RESOURCE_LIMIT: "ข้อมูลเกินขอบเขตที่รองรับ",
  CURSOR_INVALID: "รายการประวัติไม่ถูกต้อง",
  CURSOR_EXPIRED: "รายการประวัติหมดอายุ",
  NATIVE_UNAVAILABLE: "ความสามารถนี้ใช้ได้ในแอปเดสก์ท็อปเท่านั้น",
  LEGACY_COMMAND_FAILED: "คำสั่งเดสก์ท็อปทำงานไม่สำเร็จ",
  NO_EVIDENCE: "ยังไม่มีหลักฐานเพียงพอ",
  PROVIDER_UNAVAILABLE: "ยังไม่มีตัวประมวลผลในเครื่อง",
  PROVIDER_FAILED: "ตัวประมวลผลในเครื่องทำงานไม่สำเร็จ",
  MODEL_OUTPUT_INVALID: "ผลลัพธ์จากโมเดลไม่ถูกต้อง",
  PLAYBACK_OUTPUT_UNSUPPORTED: "อุปกรณ์เสียงไม่รองรับอัตราสุ่มนี้",
  PLAYBACK_FORMAT_UNSUPPORTED: "ยังเล่นรูปแบบเสียงนี้ไม่ได้",
  PLAYBACK_TIMELINE_INVALID: "ลำดับเวลาของเสียงไม่ถูกต้อง",
  PLAYBACK_SOURCE_MISSING: "ไม่พบไฟล์เสียงของการบันทึก",
  PLAYBACK_PATH_DENIED: "ไฟล์เสียงอยู่นอกพื้นที่ที่อนุญาต",
  PLAYBACK_BUFFER_UNDERRUN: "เสียงมาไม่ทัน จึงหยุดการเล่น",
  PLAYBACK_STALE_EPOCH: "สถานะเครื่องเล่นเสียงเก่าเกินไป",
  PLAYBACK_HANDLE_INVALID: "เครื่องเล่นเสียงนี้ไม่พร้อมใช้งาน",
  PLAYBACK_IO_FAILED: "อ่านเสียงไม่สำเร็จ",
  PLAYBACK_DEVICE_LOST: "อุปกรณ์เสียงหลุดการเชื่อมต่อ",
  PLAYBACK_CAPTURE_ACTIVE: "เล่นเสียงไม่ได้ระหว่างบันทึก",
  PLAYBACK_BUSY: "กรุณาปิดเครื่องเล่นเสียงก่อน",
  PLAYBACK_OPEN_TIMEOUT: "เตรียมเครื่องเล่นเสียงไม่ทันเวลา",
};

let requestSequence = 0;

function newRequestId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  requestSequence += 1;
  return "live-question-" + requestSequence;
}

function formatClock(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return Math.floor(seconds / 60) + ":" + String(seconds % 60).padStart(2, "0");
}

function channelLabel(channel: string): string {
  if (channel === "mic") return "ไมโครโฟน";
  if (channel === "system") return "เสียงระบบ";
  if (channel === "file") return "ไฟล์";
  return "เสียงที่บันทึกไว้";
}

function describeError(error: ReviewError): string {
  const label = ERROR_LABELS[error.code];
  return label + (error.message ? " — " + error.message : "");
}

function isRunning(phase: LiveWorkspaceProps["phase"]): boolean {
  return phase === "starting" || phase === "listening" || phase === "degraded";
}

export function getLiveSummaryPresentation(
  summaries: MeetingSummaries,
  recordingId: string | null,
): LiveSummaryPresentation {
  const summaryKinds = Array.from(new Set(summaries.rows.map((row) => row.kind)));
  const currentRows = summaryKinds.flatMap((kind) => {
    const current = summaries.rows.find((row) => (
      row.recordingId === recordingId &&
      row.kind === kind &&
      !row.superseded
    ));
    return current ? [current] : [];
  });
  const disclosures: LiveSummaryDisclosure[] = [];
  if (summaries.otherRecordings > 0) {
    disclosures.push({
      key: "other-recordings",
      tone: "muted",
      message: "มีสรุปของการบันทึกอื่นในโครงการ " + summaries.otherRecordings + " รายการ — ไม่แสดงเป็นของรายการนี้",
    });
  }
  if (summaries.unattributable > 0 && summaries.attributionComplete) {
    disclosures.push({
      key: "unattributable",
      tone: "notice",
      message: "มีสรุป " + summaries.unattributable + " รายการที่ไม่ทราบว่ามาจากการบันทึกใด",
    });
  }
  if (!summaries.attributionComplete) {
    disclosures.push({
      key: "incomplete-attribution",
      tone: "notice",
      message: "การตรวจสอบที่มาของสรุปยังไม่ครบ จึงไม่สรุปว่าข้อมูลที่หายเป็นข้อมูลเสีย",
    });
  }
  return { currentRows, disclosures };
}

function sourceTime(source: RecordingAnswerSource): string {
  return formatClock(source.startMs) + "–" + formatClock(source.endMs);
}

function parseSummaryItems(content: string): {
  items: Array<Record<string, unknown>>;
  error: boolean;
  raw: string;
} {
  try {
    const value: unknown = JSON.parse(content || "[]");
    if (!Array.isArray(value)) {
      return { items: [], error: true, raw: content };
    }
    return {
      items: value.filter((item): item is Record<string, unknown> => (
        typeof item === "object" && item !== null
      )),
      error: false,
      raw: content,
    };
  } catch {
    return { items: [], error: true, raw: content };
  }
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function SummaryRowView({ row }: { row: SummaryRow }) {
  if (row.kind === "whole_story") {
    return (
      <div className="live-workspace__summary-section">
        <h4>ภาพรวม</h4>
        <p>{row.content}</p>
      </div>
    );
  }

  const parsed = parseSummaryItems(row.content);
  const title = row.kind === "timeline" ? "ประเด็นสำคัญ" : "งานที่ต้องทำ";
  if (parsed.error) {
    return (
      <div className="live-workspace__summary-section live-workspace__summary-section--error">
        <h4>{title} — อ่านข้อมูลไม่สำเร็จ</h4>
        <p>ข้อมูลส่วนนี้มีรูปแบบไม่สมบูรณ์ จึงไม่ตีความเป็นรายการ</p>
        <details>
          <summary>ดูข้อความดิบอย่างปลอดภัย</summary>
          <pre>{parsed.raw}</pre>
        </details>
      </div>
    );
  }

  return (
    <div className="live-workspace__summary-section">
      <h4>{title}</h4>
      {parsed.items.length > 0 ? (
        <ul>
          {parsed.items.map((item, index) => {
            const text = stringValue(item.point) || stringValue(item.item);
            const owner = stringValue(item.owner);
            return (
              <li key={String(row.id) + "-" + String(index)}>
                {text || "รายการที่ไม่มีข้อความ"}
                {owner ? <strong> — {owner}</strong> : null}
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="live-workspace__muted">ยังไม่มีรายการในส่วนนี้</p>
      )}
    </div>
  );
}

function CapabilityNotice({
  capability,
  children,
}: {
  capability: { available: boolean; reasonCode: string | null };
  children: ReactNode;
}) {
  if (capability.available) return null;
  return (
    <p className="live-workspace__disabled-reason" role="status">
      {children}
      {capability.reasonCode ? " (" + capability.reasonCode + ")" : null}
    </p>
  );
}

export function LiveWorkspace({
  selection,
  phase,
  elapsedMs,
  devices,
  captureDevices,
  liveAudioLevels,
  captureDevicesLoading,
  captureDevicesError,
  refreshCaptureDevices,
  segmentFeed,
  topic,
  summaries: summaryState,
  ask,
  capabilities,
  operationErrors,
  actions,
  closeReviewPlayer,
  transcriptLoading = false,
  transcriptIncomplete = false,
  visible = true,
}: LiveWorkspaceProps & LiveWorkspaceAdapterProps) {
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const feedRef = useRef<HTMLDivElement | null>(null);
  const nearBottomRef = useRef(true);
  const previousSegmentCountRef = useRef(segmentFeed.length);
  const [activeTab, setActiveTab] = useState<"capture" | "transcript" | "insights">("capture");
  const [captureSourceMode, setCaptureSourceMode] = useState<"mic" | "system" | "application">("mic");
  const [applicationProcessId, setApplicationProcessId] = useState<number | null>(null);
  const [waveformHistory, setWaveformHistory] = useState<{ mic: number[]; system: number[] }>({
    mic: [],
    system: [],
  });
  const lastWaveformSampleRef = useRef<{ mic: number | null; system: number | null }>({
    mic: null,
    system: null,
  });
  const [micDeviceId, setMicDeviceId] = useState("");
  const [systemDeviceId, setSystemDeviceId] = useState("");
  const [language, setLanguage] = useState("auto");
  const [transcriptProfile, setTranscriptProfile] = useState<"chunked" | "revisioned">("chunked");
  const [privacyAcknowledged, setPrivacyAcknowledged] = useState(true);
  const [question, setQuestion] = useState("");
  const [leavePrompt, setLeavePrompt] = useState(false);
  const [showReturnToLatest, setShowReturnToLatest] = useState(false);

  const handleTabKeyDown = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
    current: "capture" | "transcript" | "insights",
  ) => {
    const tabs = ["capture", "transcript", "insights"] as const;
    const currentIndex = tabs.indexOf(current);
    const nextIndex = event.key === "ArrowRight"
      ? (currentIndex + 1) % tabs.length
      : event.key === "ArrowLeft"
        ? (currentIndex - 1 + tabs.length) % tabs.length
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? tabs.length - 1
            : -1;
    if (nextIndex < 0) return;
    event.preventDefault();
    const next = tabs[nextIndex];
    if (!next) return;
    setActiveTab(next);
    document.getElementById(`live-tab-${next}`)?.focus();
  };

  const handleSourceTabKeyDown = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
    current: "mic" | "system" | "application",
  ) => {
    const tabs = ["mic", "system", "application"] as const;
    const currentIndex = tabs.indexOf(current);
    const nextIndex = event.key === "ArrowRight"
      ? (currentIndex + 1) % tabs.length
      : event.key === "ArrowLeft"
        ? (currentIndex - 1 + tabs.length) % tabs.length
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? tabs.length - 1
            : -1;
    if (nextIndex < 0) return;
    event.preventDefault();
    const next = tabs[nextIndex];
    if (!next) return;
    setCaptureSourceMode(next);
    document.getElementById(`live-source-tab-${next}`)?.focus();
  };

  useEffect(() => {
    if (!captureDevices) return;
    setMicDeviceId(captureDevices.selectedMicDeviceId ?? "");
    setSystemDeviceId(captureDevices.selectedSystemDeviceId ?? "");
  }, [captureDevices?.selectedMicDeviceId, captureDevices?.selectedSystemDeviceId]);

  useEffect(() => {
    setWaveformHistory({ mic: [], system: [] });
    lastWaveformSampleRef.current = { mic: null, system: null };
  }, [selection?.projectId, selection?.recordingId]);

  useEffect(() => {
    for (const channel of ["mic", "system"] as const) {
      const sample = liveAudioLevels[channel];
      if (!sample || lastWaveformSampleRef.current[channel] === sample.observedAt) continue;
      lastWaveformSampleRef.current[channel] = sample.observedAt;
      setWaveformHistory((current) => ({
        ...current,
        [channel]: sample.waveform.reduce(
          (history, peak) => appendLivePeak(history, scaleLivePeak(peak), 144),
          current[channel],
        ),
      }));
    }
  }, [liveAudioLevels.mic?.observedAt, liveAudioLevels.system?.observedAt]);

  const captureCapability = capabilities.capture ?? { available: true, reasonCode: null };
  const askCapability = capabilities.recordingAsk ?? {
    available: Boolean(selection),
    reasonCode: selection ? null : "RECORDING_NOT_FOUND",
  };
  const summaryCapability = capabilities.summary ?? {
    available: Boolean(selection),
    reasonCode: selection ? null : "RECORDING_NOT_FOUND",
  };
  const latestError = operationErrors.length > 0
    ? operationErrors[operationErrors.length - 1]
    : null;
  const running = isRunning(phase);
  const stopPending = phase === "stopping" || phase === "transcribing";
  const hasSelection = selection !== null;

  useEffect(() => {
    if (visible) headingRef.current?.focus();
  }, [visible]);

  useEffect(() => {
    const node = feedRef.current;
    if (!node) return;
    const receivedNewSegments = segmentFeed.length > previousSegmentCountRef.current;
    previousSegmentCountRef.current = segmentFeed.length;
    if (nearBottomRef.current) {
      node.scrollTop = node.scrollHeight;
      setShowReturnToLatest(false);
    } else if (receivedNewSegments) {
      setShowReturnToLatest(true);
    }
  }, [segmentFeed.length]);

  useEffect(() => {
    if (!leavePrompt) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setLeavePrompt(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [leavePrompt]);

  const handleFeedScroll = () => {
    const node = feedRef.current;
    if (!node) return;
    const nearBottom = node.scrollHeight - node.scrollTop - node.clientHeight < 48;
    nearBottomRef.current = nearBottom;
    setShowReturnToLatest(!nearBottom && segmentFeed.length > 0);
  };

  const returnToLatest = () => {
    const node = feedRef.current;
    if (node) node.scrollTop = node.scrollHeight;
    nearBottomRef.current = true;
    setShowReturnToLatest(false);
  };

  const handleStart = async () => {
    if (!privacyAcknowledged || !captureCapability.available || !canStartSelectedSource) return;
    try {
      await actions.start(
        {
          captureSystem: captureSourceMode !== "mic",
          language: language === "auto" ? undefined : language,
          transcriptProfile,
          micDeviceId: micDeviceId || undefined,
          systemDeviceId: systemDeviceId || undefined,
          applicationProcessId: captureSourceMode === "application" ? applicationProcessId ?? undefined : undefined,
      },
        closeReviewPlayer ?? (async () => ({ closed: false })),
      );
      setActiveTab("transcript");
    } catch {
      // The panel owns the safe error state. Keep the form and question intact.
    }
  };

  const handleStop = async () => {
    if (stopPending) return;
    try {
      await actions.stop();
    } catch {
      // The panel owns the safe error state.
    }
  };

  const handleAsk = async () => {
    const trimmed = question.trim();
    if (!selection || !trimmed || ask.status === "loading" || !askCapability.available) return;
    try {
      await actions.ask(selection, trimmed, newRequestId());
    } catch {
      // The panel keeps the question and renders the normalized error.
    }
  };

  const handleGenerateSummary = async () => {
    if (!summaryCapability.available) return;
    try {
      await actions.generateSummary();
    } catch {
      // The panel owns the safe error state.
    }
  };

  const handleClose = () => {
    if (running || stopPending) {
      setLeavePrompt(true);
      return;
    }
    actions.closeView();
  };

  const handleContinueAndLeave = () => {
    setLeavePrompt(false);
    actions.closeView();
  };

  const handleStopAndLeave = async () => {
    try {
      await actions.stopAndLeave();
      setLeavePrompt(false);
      actions.closeView();
    } catch {
      // Keep the dialog open and the capture surface reachable.
    }
  };

  const summaries = summaryState.data ?? {
    rows: [] as SummaryRow[],
    otherRecordings: 0,
    unattributable: 0,
    attributionComplete: true,
  };
  const summaryPresentation = getLiveSummaryPresentation(
    summaries,
    selection?.recordingId ?? null,
  );
  const currentSummaryRows = summaryPresentation.currentRows;
  const topicData = topic.status === "ready" ? topic.data : null;
  const askData = ask.status === "ready" ? ask.data : null;
  const micOptions = captureDevices?.inputs ?? [];
  const systemOptions = captureDevices?.loopbackOutputs ?? [];
  const applicationOptions = captureDevices?.applications ?? [];
  const micSelectionUnavailable = Boolean(
    micDeviceId && !micOptions.some((device) => device.id === micDeviceId),
  );
  const systemSelectionUnavailable = Boolean(
    systemDeviceId && !systemOptions.some((device) => device.id === systemDeviceId),
  );
  const applicationSelectionUnavailable = applicationProcessId !== null && !applicationOptions.some(
    (application) => application.processId === applicationProcessId,
  );
  const applicationCaptureAvailable = captureDevices?.applicationCaptureAvailable ?? false;
  const canStartSelectedSource = captureSourceMode !== "application" || (
    applicationCaptureAvailable && applicationProcessId !== null && !applicationSelectionUnavailable
  );

  return (
    <div className="live-workspace">
      <header className="live-workspace__header">
        <div>
          <p className="live-workspace__eyebrow">พื้นที่ประชุมสด · Live Meeting</p>
          <h2 ref={headingRef} tabIndex={-1} id="live-workspace-title">
            บันทึกเสียงการประชุม
          </h2>
          <div className="live-workspace__status" role="status" aria-live="polite">
            <span className={"live-workspace__phase live-workspace__phase--" + phase}>
              {phase === "idle" && !capabilities.capture.available
                ? "เปิด FUNG Desktop เพื่อบันทึก"
                : PHASE_LABELS[phase] ?? "สถานะการบันทึก"}
            </span>
            {(running || stopPending) ? (
              <span className="live-workspace__elapsed">{formatClock(elapsedMs)}</span>
            ) : null}
          </div>
        </div>
        <div className="live-workspace__header-actions">
          {running ? (
            <button type="button" className="live-btn live-btn-danger" onClick={() => void handleStop()}>
              หยุดบันทึก
            </button>
          ) : null}
          <button type="button" className="live-btn" onClick={handleClose}>
            กลับไปหน้าหลัก
          </button>
        </div>
      </header>

      {latestError ? (
        <div className="live-workspace__error" role="alert">
          {describeError(latestError)}
        </div>
      ) : null}

      <div className="live-workspace__tabs" role="tablist" aria-label="พื้นที่ประชุมสด">
        <button
          id="live-tab-capture"
          type="button"
          role="tab"
          aria-selected={activeTab === "capture"}
          aria-controls="live-panel-capture"
          tabIndex={activeTab === "capture" ? 0 : -1}
          className={`live-workspace__tab${activeTab === "capture" ? " is-active" : ""}`}
          onKeyDown={(event) => handleTabKeyDown(event, "capture")}
          onClick={() => setActiveTab("capture")}
        >
          ตั้งค่าบันทึก
        </button>
        <button
          id="live-tab-transcript"
          type="button"
          role="tab"
          aria-selected={activeTab === "transcript"}
          aria-controls="live-panel-transcript"
          tabIndex={activeTab === "transcript" ? 0 : -1}
          className={`live-workspace__tab${activeTab === "transcript" ? " is-active" : ""}`}
          onKeyDown={(event) => handleTabKeyDown(event, "transcript")}
          onClick={() => setActiveTab("transcript")}
        >
          ข้อความ <span>{segmentFeed.length}</span>
        </button>
        <button
          id="live-tab-insights"
          type="button"
          role="tab"
          aria-selected={activeTab === "insights"}
          aria-controls="live-panel-insights"
          tabIndex={activeTab === "insights" ? 0 : -1}
          className={`live-workspace__tab${activeTab === "insights" ? " is-active" : ""}`}
          onKeyDown={(event) => handleTabKeyDown(event, "insights")}
          onClick={() => setActiveTab("insights")}
        >
          ข้อมูลและถาม
        </button>
      </div>

      <section
        id="live-panel-capture"
        className="live-workspace__capture-card live-workspace__tab-panel"
        role="tabpanel"
        aria-labelledby="live-tab-capture"
        tabIndex={0}
        hidden={activeTab !== "capture"}
      >
        <div>
          <p className="live-workspace__eyebrow">การบันทึกในเครื่อง</p>
          <h3 id="live-capture-title">
            {running ? "กำลังเก็บเสียงอย่างต่อเนื่อง" : "เริ่มการบันทึกเมื่อพร้อม"}
          </h3>
          <p className="live-workspace__copy">
            เสียงถูกบันทึกและประมวลผลในเครื่องนี้เท่านั้น โปรดแจ้งผู้ร่วมประชุมก่อนเริ่มอัด
          </p>
        </div>
        <div className="live-workspace__capture-controls">
          {!running ? (
            <>
              <label className="live-workspace__check">
                <input
                  type="checkbox"
                  checked={privacyAcknowledged}
                  onChange={(event) => setPrivacyAcknowledged(event.target.checked)}
                />
                <span>ฉันจะแจ้งผู้ร่วมประชุมก่อนบันทึกเสียง</span>
              </label>
              <div className="live-workspace__device-routing" aria-label="เลือกแหล่งเสียง">
                <div className="live-workspace__device-routing-header">
                  <span>เสียงที่บันทึก</span>
                  <button
                    type="button"
                    className="live-btn live-btn-subtle"
                    onClick={() => void refreshCaptureDevices()}
                    disabled={captureDevicesLoading}
                  >
                    {captureDevicesLoading ? "กำลังอ่านรายการ…" : "รีเฟรชอุปกรณ์"}
                  </button>
                </div>
                <div className="live-workspace__source-tabs" role="tablist" aria-label="เลือกช่องเสียงเพิ่มเติม">
                  <button
                    id="live-source-tab-mic"
                    type="button"
                    role="tab"
                    aria-selected={captureSourceMode === "mic"}
                    aria-controls="live-source-route-options"
                    tabIndex={captureSourceMode === "mic" ? 0 : -1}
                    className={captureSourceMode === "mic" ? "is-active" : ""}
                    onKeyDown={(event) => handleSourceTabKeyDown(event, "mic")}
                    onClick={() => setCaptureSourceMode("mic")}
                  >ไมค์อย่างเดียว</button>
                  <button
                    id="live-source-tab-system"
                    type="button"
                    role="tab"
                    aria-selected={captureSourceMode === "system"}
                    aria-controls="live-source-route-options"
                    tabIndex={captureSourceMode === "system" ? 0 : -1}
                    className={captureSourceMode === "system" ? "is-active" : ""}
                    onKeyDown={(event) => handleSourceTabKeyDown(event, "system")}
                    onClick={() => setCaptureSourceMode("system")}
                  >เสียงระบบ</button>
                  <button
                    id="live-source-tab-application"
                    type="button"
                    role="tab"
                    aria-selected={captureSourceMode === "application"}
                    aria-controls="live-source-route-options"
                    tabIndex={captureSourceMode === "application" ? 0 : -1}
                    className={captureSourceMode === "application" ? "is-active" : ""}
                    onKeyDown={(event) => handleSourceTabKeyDown(event, "application")}
                    onClick={() => setCaptureSourceMode("application")}
                  >เลือกรายแอป</button>
                </div>
                <label className="live-workspace__field live-workspace__field--stacked">
                  <span>ไมโครโฟน</span>
                  <select
                    aria-label="อุปกรณ์ไมโครโฟน"
                    value={micDeviceId}
                    onChange={(event) => setMicDeviceId(event.target.value)}
                    disabled={captureDevicesLoading}
                  >
                    <option value="">ค่าเริ่มต้นระบบ</option>
                    {micSelectionUnavailable ? (
                      <option value={micDeviceId}>อุปกรณ์เดิม — ไม่พร้อมใช้งาน</option>
                    ) : null}
                    {micOptions.map((device) => (
                      <option key={device.id} value={device.id} disabled={!device.available}>
                        {device.name}{device.isDefault ? " · ค่าเริ่มต้น" : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <div
                  id="live-source-route-options"
                  role="tabpanel"
                  aria-labelledby={`live-source-tab-${captureSourceMode}`}
                >
                {captureSourceMode === "mic" ? (
                  <p className="live-workspace__muted">บันทึกเสียงจากไมโครโฟนที่เลือกเท่านั้น</p>
                ) : null}
                {captureSourceMode === "system" ? (
                  <label className="live-workspace__field live-workspace__field--stacked">
                    <span>อุปกรณ์เสียงระบบ</span>
                    <select
                      aria-label="อุปกรณ์เสียงระบบ"
                      value={systemDeviceId}
                      onChange={(event) => setSystemDeviceId(event.target.value)}
                      disabled={captureDevicesLoading}
                    >
                      <option value="">ค่าเริ่มต้นระบบ</option>
                      {systemSelectionUnavailable ? (
                        <option value={systemDeviceId}>อุปกรณ์เดิม — ไม่พร้อมใช้งาน</option>
                      ) : null}
                      {systemOptions.map((device) => (
                        <option key={device.id} value={device.id} disabled={!device.available}>
                          {device.name}{device.isDefault ? " · ค่าเริ่มต้น" : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : null}
                </div>
                {captureSourceMode === "application" ? (
                  <label className="live-workspace__field live-workspace__field--stacked">
                    <span>แอปที่กำลังส่งเสียง</span>
                    <select
                      aria-label="แอปที่กำลังส่งเสียง"
                      value={applicationProcessId === null ? "" : String(applicationProcessId)}
                      onChange={(event) => setApplicationProcessId(event.target.value ? Number(event.target.value) : null)}
                      disabled={!applicationCaptureAvailable || captureDevicesLoading}
                    >
                      <option value="">เลือกแอปที่กำลังส่งเสียง</option>
                      {applicationSelectionUnavailable ? (
                        <option value={String(applicationProcessId)}>แอปเดิม — ไม่ได้ส่งเสียงแล้ว</option>
                      ) : null}
                      {applicationOptions.map((application) => (
                        <option key={application.processId} value={String(application.processId)}>
                          {application.name} · PID {application.processId}
                        </option>
                      ))}
                    </select>
                    {applicationCaptureAvailable && !captureDevices?.applicationCaptureIssue && applicationOptions.length === 0 ? (
                      <span className="live-workspace__muted">ไม่พบแอปที่กำลังส่งเสียง · รีเฟรชรายการหลังเปิดเสียง</span>
                    ) : null}
                    {captureDevices?.applicationCaptureIssue ? (
                      <span className="live-workspace__device-issue" role="status">
                        {captureDevices.applicationCaptureIssue}
                      </span>
                    ) : null}
                  </label>
                ) : null}
                {captureDevicesError ? (
                  <p className="live-workspace__device-issue" role="alert">{captureDevicesError}</p>
                ) : null}
                {captureDevices?.issue ? (
                  <p className="live-workspace__device-issue" role="status">{captureDevices.issue}</p>
                ) : null}
              </div>
              <label className="live-workspace__field">
                <span>ภาษา</span>
                <select value={language} onChange={(event) => setLanguage(event.target.value)}>
                  <option value="auto">ตรวจอัตโนมัติ</option>
                  <option value="th">ไทย</option>
                  <option value="en">อังกฤษ</option>
                </select>
              </label>
              <label className="live-workspace__field">
                <span>รูปแบบ transcript สด</span>
                <select
                  value={transcriptProfile}
                  onChange={(event) => setTranscriptProfile(event.target.value as "chunked" | "revisioned")}
                >
                  <option value="chunked">ช่วงเสียงเดิม (เสถียร)</option>
                  <option value="revisioned">แสดงข้อความระหว่างพูด (revisioned)</option>
                </select>
              </label>
              {transcriptProfile === "revisioned" ? (
                <p className="live-workspace__device-issue" role="note">
                  ข้อความระหว่างพูดอาจเปลี่ยนได้ · ข้อความยืนยันจะแสดงหลังบันทึกสำเร็จ
                </p>
              ) : null}
              <button
                type="button"
                className="live-btn live-btn-primary"
                disabled={!privacyAcknowledged || !captureCapability.available || !canStartSelectedSource}
                onClick={() => void handleStart()}
              >
                ● เริ่มประชุม
              </button>
              {!privacyAcknowledged ? (
                <p className="live-workspace__disabled-reason">ต้องยืนยันการแจ้งผู้ร่วมประชุมก่อนเริ่ม</p>
              ) : null}
              {!canStartSelectedSource && captureSourceMode === "application" ? (
                <p className="live-workspace__disabled-reason">เลือกแอปที่กำลังส่งเสียงและรองรับการจับเสียงรายแอปก่อนเริ่ม</p>
              ) : null}
              <CapabilityNotice capability={captureCapability}>
                เริ่มบันทึกไม่ได้ในสถานะนี้
              </CapabilityNotice>
            </>
          ) : (
            <div className="live-workspace__capture-monitor" aria-label="เสียงที่กำลังบันทึก">
              <LiveChannelWaveform label="ไมโครโฟน" sample={liveAudioLevels.mic} history={waveformHistory.mic} />
              {devices.system ? (
                <LiveChannelWaveform
                  label={`เสียงที่เลือก · ${devices.system}`}
                  sample={liveAudioLevels.system}
                  history={waveformHistory.system}
                />
              ) : null}
              <div className="live-workspace__device-list" aria-label="อุปกรณ์ที่ใช้บันทึก">
                <span>ไมค์: {devices.mic ?? "กำลังตรวจสอบ"}</span>
                <span>ระบบ: {devices.system ?? "ไม่ได้จับเสียงระบบ"}</span>
              </div>
            </div>
          )}
        </div>
      </section>

      <div
        id="live-panel-transcript"
        className="live-workspace__tab-panel live-workspace__tab-panel--transcript"
        role="tabpanel"
        aria-labelledby="live-tab-transcript"
        tabIndex={0}
        hidden={activeTab !== "transcript"}
      >
        <section className="live-workspace__transcript" aria-labelledby="live-transcript-title">
          <div className="live-workspace__section-heading">
            <div>
              <p className="live-workspace__eyebrow">หลักฐานเสียง</p>
              <h3 id="live-transcript-title">ข้อความที่ยืนยันแล้ว</h3>
            </div>
            <span className="live-workspace__count">{segmentFeed.length} รายการ</span>
          </div>
          {transcriptIncomplete ? (
            <p className="live-workspace__notice" role="status">
              ข้อความสดบางส่วนมาเร็วเกินขอบเขตที่เก็บไว้ — ข้อมูลที่แสดงอาจไม่ครบ
            </p>
          ) : null}
          <div
            className="live-workspace__feed"
            ref={feedRef}
            onScroll={handleFeedScroll}
            aria-busy={transcriptLoading}
            aria-live="polite"
          >
            {transcriptLoading ? (
              <p className="live-workspace__empty">กำลังอ่านข้อความที่บันทึกไว้...</p>
            ) : segmentFeed.length === 0 ? (
              <p className="live-workspace__empty">
                {running ? "ยังไม่มีข้อความที่ยืนยันจากเสียงในขณะนี้" : "ยังไม่มีข้อความของการบันทึกนี้"}
              </p>
            ) : (
              segmentFeed.map((segment: LiveSegmentEvent) => (
                <article
                  key={segment.recordingId + ":" + segment.segmentId}
                  className="live-workspace__segment"
                >
                  <div className="live-workspace__segment-meta">
                    <span>{formatClock(segment.startMs)} · {channelLabel(segment.channel)}</span>
                    <span>{segment.speaker || "ไม่ระบุชื่อ"}</span>
                  </div>
                  <p>{segment.text}</p>
                </article>
              ))
            )}
          </div>
          {showReturnToLatest ? (
            <button type="button" className="live-btn live-workspace__return" onClick={returnToLatest}>
              กลับไปข้อความล่าสุด
            </button>
          ) : null}
        </section>
      </div>

      <div
        id="live-panel-insights"
        className="live-workspace__tab-panel live-workspace__tab-panel--insights"
        role="tabpanel"
        aria-labelledby="live-tab-insights"
        tabIndex={0}
        hidden={activeTab !== "insights"}
      >
        <aside className="live-workspace__side" aria-label="ข้อมูลเสริมของการประชุม">
          <section className="live-workspace__card" aria-labelledby="live-topic-title" aria-busy={topic.status === "loading"}>
            <div className="live-workspace__section-heading">
              <div>
                <p className="live-workspace__eyebrow">ข้อมูลชั่วคราว</p>
                <h3 id="live-topic-title">ตอนนี้กำลังคุยเรื่อง</h3>
              </div>
            </div>
            {topic.status === "error" && topic.error ? (
              <p className="live-workspace__error-text">{describeError(topic.error)}</p>
            ) : topic.status === "loading" ? (
              <p className="live-workspace__empty">กำลังรอข้อมูลสด...</p>
            ) : topicData ? (
              <>
                <p className="live-workspace__topic">{topicData.topic}</p>
                {topicData.openPoints.length > 0 ? (
                  <>
                    <h4>ประเด็นค้าง</h4>
                    <ul>
                      {topicData.openPoints.map((point, index) => <li key={"open-" + String(index)}>{point}</li>)}
                    </ul>
                  </>
                ) : null}
                {topicData.actionItems.length > 0 ? (
                  <>
                    <h4>งานที่พูดถึง</h4>
                    <ul>
                      {topicData.actionItems.map((item, index) => <li key={"action-" + String(index)}>{item}</li>)}
                    </ul>
                  </>
                ) : null}
                <p className="live-workspace__muted">โมเดล: {topicData.model} · ข้อมูลนี้ไม่ถูกเก็บเป็นประวัติ</p>
              </>
            ) : (
              <p className="live-workspace__empty">รอข้อมูลสด — หัวข้อเป็นข้อมูลชั่วคราว ไม่ถูกสร้างย้อนหลัง</p>
            )}
          </section>

          <section className="live-workspace__card" aria-labelledby="live-ask-title" aria-busy={ask.status === "loading"}>
            <div className="live-workspace__section-heading">
              <div>
                <p className="live-workspace__eyebrow">ถามจากหลักฐานเดียวกัน</p>
                <h3 id="live-ask-title">ถามเฉพาะการบันทึกนี้</h3>
              </div>
            </div>
            <p id="live-ask-help" className="live-workspace__scope-note">
              ใช้เฉพาะบทถอดเสียงที่บันทึกไว้ของรายการนี้ กราฟและบริบทสดไม่รวมอยู่ในการถาม
            </p>
            <label className="live-workspace__sr-only" htmlFor="live-recording-question">
              คำถามเกี่ยวกับการบันทึกนี้
            </label>
            <textarea
              id="live-recording-question"
              value={question}
              rows={3}
              maxLength={4000}
              placeholder="เช่น ใครรับผิดชอบขั้นตอนถัดไป"
              aria-describedby="live-ask-help"
              onChange={(event) => setQuestion(event.target.value)}
            />
            <div className="live-workspace__ask-actions">
              <span className="live-workspace__muted">{question.length}/4000</span>
              <button
                type="button"
                className="live-btn"
                disabled={!hasSelection || !askCapability.available || ask.status === "loading" || question.trim().length === 0}
                onClick={() => void handleAsk()}
              >
                {ask.status === "loading" ? "กำลังค้น..." : "ถาม"}
              </button>
            </div>
            <CapabilityNotice capability={askCapability}>
              เลือกการบันทึกที่กำลังทำงานก่อนถาม
            </CapabilityNotice>
            {ask.status === "error" && ask.error ? (
              <p className="live-workspace__error-text" role="alert">{describeError(ask.error)}</p>
            ) : null}
            {askData ? (
              <div className="live-workspace__answer">
                {askData.status === "insufficient_evidence" ? (
                  <p>ยังไม่มีหลักฐานเพียงพอในบันทึกนี้</p>
                ) : (
                  <p>{askData.answer}</p>
                )}
                {askData.sources.length > 0 ? (
                  <ol className="live-workspace__sources">
                    {askData.sources.map((source) => (
                      <li key={source.segmentId}>
                        <span>{sourceTime(source)} · อ้างอิง {source.citationIndex}</span>
                        <p>{source.text}</p>
                      </li>
                    ))}
                  </ol>
                ) : null}
                <p className="live-workspace__muted">
                  คำตอบนี้เป็นการอนุมานจากข้อมูลในเครื่อง · แหล่งอ้างอิง {askData.sources.length} ช่วงเสียง
                </p>
              </div>
            ) : null}
          </section>

          <section
            className="live-workspace__card"
            aria-labelledby="live-summary-title"
            aria-busy={summaryState.status === "loading"}
          >
            <div className="live-workspace__section-heading">
              <div>
                <p className="live-workspace__eyebrow">ขอบเขตการบันทึก</p>
                <h3 id="live-summary-title">สรุปของการบันทึกนี้</h3>
              </div>
              <button
                type="button"
                className="live-btn live-btn-subtle"
                disabled={!summaryCapability.available || summaryState.status === "loading"}
                onClick={() => void handleGenerateSummary()}
              >
                สรุปใหม่
              </button>
            </div>
            <CapabilityNotice capability={summaryCapability}>
              ยังไม่มีรายการบันทึกให้สรุป
            </CapabilityNotice>
            {summaryState.status === "error" && summaryState.error ? (
              <p className="live-workspace__error-text" role="alert">{describeError(summaryState.error)}</p>
            ) : summaryState.status === "loading" ? (
              <p className="live-workspace__empty">กำลังอ่านหรือสร้างสรุปในเครื่อง...</p>
            ) : summaryState.status === "idle" || summaryState.status === "unavailable" ? (
              <p className="live-workspace__empty">
                {summaryState.status === "unavailable"
                  ? "การสรุปยังไม่พร้อมใช้งานในสถานะนี้"
                  : "ยังไม่ได้โหลดสรุปของการบันทึกนี้"}
              </p>
            ) : (
              <>
                {currentSummaryRows.length > 0 ? (
                  currentSummaryRows.map((row) => <SummaryRowView key={row.id} row={row} />)
                ) : (
                  <p className="live-workspace__empty">ยังไม่มีสรุปสำหรับการบันทึกนี้</p>
                )}
                {summaryPresentation.disclosures.map((disclosure) => (
                  <p
                    key={disclosure.key}
                    className={disclosure.tone === "notice" ? "live-workspace__notice" : "live-workspace__muted"}
                  >
                    {disclosure.message}
                  </p>
                ))}
              </>
            )}
          </section>

          <ExternalMeetingToolsPanel
            projectId={selection?.projectId ?? null}
            recordingId={selection?.recordingId ?? null}
            segments={Array.from(segmentFeed).map((segment) => ({
              segmentId: segment.segmentId,
              startMs: segment.startMs,
              speaker: segment.speaker,
              text: segment.text,
            }))}
          />
        </aside>
      </div>

      {leavePrompt ? (
        <div className="live-workspace__leave-dialog" role="dialog" aria-modal="true" aria-labelledby="live-leave-title">
          <div className="live-workspace__leave-card">
            <p className="live-workspace__eyebrow">กำลังบันทึกเสียงอยู่</p>
            <h3 id="live-leave-title">ยังบันทึกเสียงอยู่</h3>
            <p>การปิดหน้านี้จะไม่หยุดเสียงโดยอัตโนมัติ เลือกการกระทำที่ต้องการ</p>
            <div className="live-workspace__leave-actions">
              <button type="button" className="live-btn" autoFocus onClick={handleContinueAndLeave}>
                อัดต่อและออกจากหน้านี้
              </button>
              <button type="button" className="live-btn live-btn-danger" onClick={() => void handleStopAndLeave()}>
                หยุดแล้วออก
              </button>
              <button type="button" className="live-btn live-btn-subtle" onClick={() => setLeavePrompt(false)}>
                อยู่หน้านี้
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
