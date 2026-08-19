// @req FR-102, FR-103, FR-104, FR-105, FR-115, NFR-104, NFR-106
import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  generateMeetingSummary,
  liveMeetingStart,
  liveMeetingStatus,
  liveMeetingStop,
  meetingAsk,
  EMPTY_MEETING_SUMMARIES,
  meetingSummaries,
  type AskAnswer,
  type LiveSegmentEvent,
  type LiveStatusEvent,
  type LiveSummaryEvent,
  type LiveTopicEvent,
  type MeetingSummaries,
  type SummaryRow,
} from "../tauri";
import { ExternalMeetingToolsPanel } from "./ExternalMeetingToolsPanel";
import "./LiveMeetingPanel.css";

function clock(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, "0")}`;
}

const PHASE_LABEL: Record<string, string> = {
  idle: "พร้อมเริ่มประชุม",
  starting: "กำลังเริ่ม...",
  listening: "กำลังฟังอยู่",
  degraded: "อัดต่อเนื่อง (ถอดสดมีปัญหา)",
  stopping: "กำลังปิดเซสชัน...",
  stopped: "จบการประชุมแล้ว",
  error: "เกิดข้อผิดพลาด",
};

export function LiveMeetingPanel({
  onClose,
  projectId,
}: {
  onClose: () => void;
  projectId: string | null;
}) {
  const [phase, setPhase] = useState<string>("idle");
  const [detail, setDetail] = useState<string | null>(null);
  const [devices, setDevices] = useState<{ mic: string | null; system: string | null }>({ mic: null, system: null });
  const [activeIds, setActiveIds] = useState<{ projectId: string | null; recordingId: string | null }>({
    projectId: null,
    recordingId: null,
  });
  const [elapsedMs, setElapsedMs] = useState(0);
  const [segments, setSegments] = useState<LiveSegmentEvent[]>([]);
  const [topic, setTopic] = useState<LiveTopicEvent | null>(null);
  const [summaryState, setSummaryState] = useState<LiveSummaryEvent | null>(null);
  const [summaries, setSummaries] = useState<MeetingSummaries>(
    EMPTY_MEETING_SUMMARIES,
  );
  const [captureSystem, setCaptureSystem] = useState(true);
  const [language, setLanguage] = useState<string>("auto");
  const [question, setQuestion] = useState("");
  const [asking, setAsking] = useState(false);
  const [askResult, setAskResult] = useState<AskAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const feedRef = useRef<HTMLDivElement | null>(null);
  const tickRef = useRef<number | null>(null);
  const activeProjectRef = useRef<string | null>(null);

  /**
   * Loads the summaries of one recording.
   *
   * The recording id is now required. Asking by project alone returned every
   * meeting in it, so a second session in the same project showed the
   * previous meeting's recap under the current one's heading.
   */
  const loadSummaries = useCallback(async (pid: string, rid: string) => {
    try {
      setSummaries(await meetingSummaries(pid, rid));
    } catch {
      /* summaries are optional display data */
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    void liveMeetingStatus().then((status) => {
      if (disposed) return;
      if (status.active) {
        setPhase(status.stopping ? "stopping" : "listening");
        setActiveIds({ projectId: status.projectId, recordingId: status.recordingId });
        activeProjectRef.current = status.projectId;
        setElapsedMs(status.elapsedMs ?? 0);
      }
    });

    void listen<LiveStatusEvent>("live-status", (event) => {
      setPhase(event.payload.state);
      if (event.payload.detail) setDetail(event.payload.detail);
      if (event.payload.micDevice || event.payload.systemDevice) {
        setDevices({ mic: event.payload.micDevice, system: event.payload.systemDevice });
      }
    }).then((fn) => unlisteners.push(fn));

    void listen<LiveSegmentEvent>("live-segment", (event) => {
      setSegments((current) => {
        const next = [...current, event.payload];
        return next.length > 200 ? next.slice(next.length - 200) : next;
      });
      queueMicrotask(() => {
        feedRef.current?.scrollTo({ top: feedRef.current.scrollHeight });
      });
    }).then((fn) => unlisteners.push(fn));

    void listen<LiveTopicEvent>("live-topic", (event) => {
      setTopic(event.payload);
    }).then((fn) => unlisteners.push(fn));

    void listen<LiveSummaryEvent>("live-summary", (event) => {
      setSummaryState(event.payload);
      // The event names the recording that just finished, which is more
      // reliable than the panel's own active ids: by the time a queued
      // summary lands the session may already have been cleared.
      if (
        event.payload.state === "ready" &&
        activeProjectRef.current &&
        event.payload.recordingId
      ) {
        void loadSummaries(
          activeProjectRef.current,
          event.payload.recordingId,
        );
      }
    }).then((fn) => unlisteners.push(fn));

    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [loadSummaries]);

  useEffect(() => {
    if (phase === "listening" || phase === "degraded" || phase === "starting") {
      tickRef.current = window.setInterval(() => setElapsedMs((value) => value + 1000), 1000);
      return () => {
        if (tickRef.current !== null) window.clearInterval(tickRef.current);
      };
    }
    return undefined;
  }, [phase]);

  const handleStart = async () => {
    setError(null);
    setSegments([]);
    setTopic(null);
    setSummaryState(null);
    // Starting a new session must clear the previous meeting's summaries,
    // not just its rows: a stale "3 more in this project" count would
    // describe a query that no longer applies.
    setSummaries(EMPTY_MEETING_SUMMARIES);
    setElapsedMs(0);
    try {
      const output = await liveMeetingStart({
        projectId: projectId ?? undefined,
        captureSystem,
        language: language === "auto" ? undefined : language,
      });
      setActiveIds({ projectId: output.projectId, recordingId: output.recordingId });
      activeProjectRef.current = output.projectId;
      setDevices({ mic: output.micDevice, system: output.systemDevice });
      setPhase("starting");
      if (output.warning) setDetail(output.warning);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleStop = async () => {
    setError(null);
    try {
      await liveMeetingStop();
      setPhase("stopping");
    } catch (err) {
      setError(String(err));
    }
  };

  const handleAsk = async () => {
    const trimmed = question.trim();
    if (!trimmed || asking) return;
    setAsking(true);
    setAskResult(null);
    setError(null);
    try {
      setAskResult(await meetingAsk(trimmed, activeIds.projectId ?? projectId ?? undefined));
    } catch (err) {
      setError(String(err));
    } finally {
      setAsking(false);
    }
  };

  const handleRetrySummary = async () => {
    if (!activeIds.projectId || !activeIds.recordingId) return;
    await generateMeetingSummary(activeIds.projectId, activeIds.recordingId);
  };

  const running = phase === "starting" || phase === "listening" || phase === "degraded";
  // Rows arrive newest-first, so the first of each kind is the current one;
  // the filter states that rather than relying on the order alone.
  const summaryByKind = (kind: string) =>
    summaries.rows.find((row) => row.kind === kind && !row.superseded);
  const story = summaryByKind("whole_story");
  const points = summaryByKind("timeline");
  const actions = summaryByKind("decisions_actions");
  const parsedPoints: { point?: string }[] = points ? (JSON.parse(points.content || "[]") as { point?: string }[]) : [];
  const parsedActions: { item?: string; owner?: string | null }[] = actions
    ? (JSON.parse(actions.content || "[]") as { item?: string; owner?: string | null }[])
    : [];

  return (
    <div className="live-overlay" role="presentation" onClick={onClose}>
      <div className="live-panel" onClick={(event) => event.stopPropagation()}>
        <header className="live-header">
          <div>
            <h2>Live Meeting</h2>
            <p className="live-sub">
              {PHASE_LABEL[phase] ?? phase}
              {running ? ` · ${clock(elapsedMs)}` : ""}
            </p>
          </div>
          <div className="live-header-actions">
            {!running ? (
              <button className="live-btn live-btn-primary" onClick={() => void handleStart()}>
                ● เริ่มประชุม
              </button>
            ) : (
              <button className="live-btn live-btn-danger" onClick={() => void handleStop()}>
                ■ จบประชุม
              </button>
            )}
            <button className="live-btn" onClick={onClose}>
              ปิดหน้าต่าง
            </button>
          </div>
        </header>

        {!running && phase === "idle" && (
          <div className="live-options">
            <label>
              <input
                type="checkbox"
                checked={captureSystem}
                onChange={(event) => setCaptureSystem(event.target.checked)}
              />
              จับเสียงระบบด้วย (เสียงอีกฝ่ายในประชุมออนไลน์)
            </label>
            <label>
              ภาษา
              <select value={language} onChange={(event) => setLanguage(event.target.value)}>
                <option value="auto">ตรวจอัตโนมัติ</option>
                <option value="th">ไทย</option>
                <option value="en">อังกฤษ</option>
              </select>
            </label>
            <p className="live-consent">
              เสียงทั้งหมดถูกบันทึกและประมวลผล<strong>ในเครื่องนี้เท่านั้น</strong> — โปรดแจ้งผู้ร่วมประชุมก่อนเริ่มอัด
            </p>
          </div>
        )}

        {(detail || error) && <div className={`live-note ${error ? "live-note-error" : ""}`}>{error ?? detail}</div>}
        {devices.mic && (
          <div className="live-devices">
            🎙 {devices.mic}
            {devices.system ? ` · 🔊 ${devices.system}` : " · (ไม่จับเสียงระบบ)"}
          </div>
        )}

        <div className="live-body">
          <section className="live-feed-wrap">
            <h3>Transcript สด</h3>
            <div className="live-feed" ref={feedRef}>
              {segments.length === 0 && <p className="live-empty">เมื่อเริ่มพูด ข้อความจะขึ้นที่นี่ (หน่วง ~10-20 วินาที)</p>}
              {segments.map((segment) => (
                <div key={segment.segmentId} className={`live-line live-line-${segment.channel}`}>
                  <span className="live-line-meta">
                    {clock(segment.startMs)} · {segment.speaker}
                  </span>
                  <span>{segment.text}</span>
                </div>
              ))}
            </div>
          </section>

          <section className="live-side">
            <div className="live-card">
              <h3>ตอนนี้กำลังคุยเรื่อง</h3>
              {topic ? (
                <>
                  <p className="live-topic">{topic.topic}</p>
                  {topic.openPoints.length > 0 && (
                    <>
                      <h4>ประเด็นค้าง</h4>
                      <ul>
                        {topic.openPoints.map((point, index) => (
                          <li key={index}>{point}</li>
                        ))}
                      </ul>
                    </>
                  )}
                  {topic.actionItems.length > 0 && (
                    <>
                      <h4>งานที่พูดถึง</h4>
                      <ul>
                        {topic.actionItems.map((item, index) => (
                          <li key={index}>{item}</li>
                        ))}
                      </ul>
                    </>
                  )}
                  <p className="live-model-tag">โมเดล: {topic.model} (ในเครื่อง)</p>
                </>
              ) : (
                <p className="live-empty">รอบทสนทนาสะสมก่อน แล้วสรุปหัวข้อทุก ~45 วินาที</p>
              )}
            </div>

            <div className="live-card">
              <h3>ถาม FUNG (ค้นจากข้อมูลในเครื่อง)</h3>
              <div className="live-ask-row">
                <input
                  value={question}
                  placeholder="เช่น เอกสารเรื่องนี้เคยคุยไว้ที่ไหน"
                  onChange={(event) => setQuestion(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void handleAsk();
                  }}
                />
                <button className="live-btn" disabled={asking} onClick={() => void handleAsk()}>
                  {asking ? "กำลังค้น..." : "ถาม"}
                </button>
              </div>
              {askResult && (
                <div className="live-answer">
                  <p>{askResult.answer}</p>
                  {askResult.sources.length > 0 && (
                    <ul className="live-sources">
                      {askResult.sources.map((source) => (
                        <li key={source.n}>
                          <span className="live-source-kind">[{source.n}] {source.kind}</span>
                          {source.projectName ? ` · ${source.projectName}` : ""} — {source.text.slice(0, 120)}
                        </li>
                      ))}
                    </ul>
                  )}
                  {askResult.searchedRowsCapped && (
                    <p className="live-model-tag">หมายเหตุ: คลังใหญ่เกินเพดานค้น 1000 แถว ผลอาจไม่ครบ</p>
                  )}
                </div>
              )}
            </div>

            <ExternalMeetingToolsPanel
              projectId={activeIds.projectId ?? projectId}
              recordingId={activeIds.recordingId}
              segments={segments}
            />
          </section>
        </div>

        {(summaryState || summaries.rows.length > 0) && (
          <section className="live-summary">
            <h3>
              สรุปหลังประชุม
              {summaryState?.state === "running" && " — กำลังสรุป..."}
              {summaryState?.state === "failed" && " — ล้มเหลว"}
            </h3>
            {summaryState?.state === "failed" && (
              <div className="live-note live-note-error">
                {summaryState.detail}
                <button className="live-btn" onClick={() => void handleRetrySummary()}>
                  ลองสรุปใหม่
                </button>
              </div>
            )}
            {story && (
              <>
                <h4>ภาพรวม</h4>
                <p>{story.content}</p>
              </>
            )}
            {parsedPoints.length > 0 && (
              <>
                <h4>ประเด็นสำคัญ</h4>
                <ul>
                  {parsedPoints.map((point, index) => (
                    <li key={index}>{point.point}</li>
                  ))}
                </ul>
              </>
            )}
            {parsedActions.length > 0 && (
              <>
                <h4>งานที่ต้องทำ</h4>
                <ul>
                  {parsedActions.map((action, index) => (
                    <li key={index}>
                      {action.item}
                      {action.owner ? <strong> — {action.owner}</strong> : null}
                    </li>
                  ))}
                </ul>
              </>
            )}
            {summaryState?.exportPath && (
              <p className="live-model-tag">ไฟล์สรุป: {summaryState.exportPath}</p>
            )}
            {summaries.otherRecordings > 0 && (
              <p className="live-model-tag">
                มีสรุปอีก {summaries.otherRecordings} รายการในโปรเจกต์นี้
                ที่เป็นของการบันทึกครั้งอื่น
              </p>
            )}
            {summaries.rows.some((row) => row.superseded) && (
              <p className="live-model-tag">
                มีสรุปรุ่นเก่าของการบันทึกนี้อยู่{" "}
                {summaries.rows.filter((row) => row.superseded).length} รายการ
                — แสดงรุ่นล่าสุด
              </p>
            )}
            {summaries.unattributable > 0 && summaries.attributionComplete && (
              <p className="live-note live-note-error">
                มีสรุป {summaries.unattributable} รายการในโปรเจกต์นี้
                ที่ไม่มีข้อมูลว่ามาจากการบันทึกใด
              </p>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
