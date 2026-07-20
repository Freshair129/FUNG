import { useEffect, useMemo, useRef, useState } from "react";
import { BookOpenText, Check, ChevronDown, Merge, Pause, Play, Scissors, SlidersHorizontal, Sparkles, UserRoundPen, ZoomIn, ZoomOut } from "lucide-react";
import { confirmSpeakerTurn, mergeSpeakers, queryTimeline, renameSpeaker, splitSpeakerTurn, startDiarization } from "./bridge";
import type { Speaker, SpeakerTurn, TimelineData } from "./model";
import { ProcessingStudio, StoryEditor, useCreativeStudio } from "./CreativeStudio";

const PREVIEW_DURATION = 180_000;
const TIMELINE_PREVIEW_KEY = "fung.mobile.timeline.preview.v1";
const previewData = (projectId: string): TimelineData => {
  const speakers: Speaker[] = [
    { id: "preview-speaker-1", projectId, key: "speaker-1", displayName: "ผู้พูด 1", confidence: 0.94 },
    { id: "preview-speaker-2", projectId, key: "speaker-2", displayName: "ผู้พูด 2", confidence: 0.86 },
    { id: "preview-speaker-3", projectId, key: "speaker-3", displayName: "ผู้พูด 3", confidence: 0.78 },
  ];
  const ranges = [[4, 31, 0], [34, 57, 1], [51, 68, 0], [72, 99, 2], [103, 132, 1], [127, 151, 2], [155, 176, 0]];
  return {
    recordingId: "preview-recording",
    durationMs: PREVIEW_DURATION,
    speakers,
    turns: ranges.map(([start, end, speaker], index) => ({
      id: `preview-turn-${index + 1}`, projectId, recordingId: "preview-recording",
      speakerId: speakers[speaker].id, startMs: start * 1000, endMs: end * 1000,
      confidence: speakers[speaker].confidence, status: "proposed", overlap: index === 2 || index === 5, revision: 1,
    })),
    source: "preview",
  };
};

const timeLabel = (milliseconds: number) => {
  const total = Math.max(0, Math.floor(milliseconds / 1000));
  return `${Math.floor(total / 60).toString().padStart(2, "0")}:${(total % 60).toString().padStart(2, "0")}`;
};

const waveBars = Array.from({ length: 34 }, (_, index) => 25 + ((index * 17) % 58));

type Props = { projectId: string; desktopAvailable: boolean; onRecord: () => void };

export function TimelineScreen({ projectId, desktopAvailable, onRecord }: Props) {
  const [data, setData] = useState<TimelineData>(() => {
    try {
      const stored = localStorage.getItem(TIMELINE_PREVIEW_KEY);
      if (stored) return JSON.parse(stored) as TimelineData;
    } catch { /* Ignore malformed preview state. */ }
    return previewData(projectId);
  });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const [playhead, setPlayhead] = useState(42_000);
  const [zoom, setZoom] = useState(1);
  const [busy, setBusy] = useState(false);
  const [surface, setSurface] = useState<"timeline" | "story" | "processing">("timeline");
  const pointers = useRef(new Map<number, number>());
  const pinch = useRef<{ distance: number; zoom: number } | null>(null);
  const selected = data.turns.find((turn) => turn.id === selectedId) ?? null;
  const [studio, setStudio] = useCreativeStudio(data, projectId);

  useEffect(() => {
    void queryTimeline(projectId).then((result) => {
      if (result) setData(result);
    }).catch(() => undefined);
  }, [projectId]);

  useEffect(() => {
    if (data.source === "preview") localStorage.setItem(TIMELINE_PREVIEW_KEY, JSON.stringify(data));
  }, [data]);

  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => setPlayhead((current) => {
      if (current >= data.durationMs) { setPlaying(false); return 0; }
      return Math.min(data.durationMs, current + 250);
    }), 250);
    return () => window.clearInterval(timer);
  }, [playing, data.durationMs]);

  const effectiveDuration = Math.max(data.durationMs, 1);
  const visibleDuration = effectiveDuration / zoom;
  const viewportStart = Math.min(Math.max(0, playhead - visibleDuration / 2), Math.max(0, effectiveDuration - visibleDuration));
  const position = (ms: number) => ((ms - viewportStart) / visibleDuration) * 100;
  const ticks = useMemo(() => Array.from({ length: 7 }, (_, index) => viewportStart + (visibleDuration / 6) * index), [viewportStart, visibleDuration]);

  const updateSpeaker = async (speaker: Speaker) => {
    const name = window.prompt("ชื่อผู้พูด", speaker.displayName)?.trim();
    if (!name) return;
    setData((current) => ({ ...current, speakers: current.speakers.map((item) => item.id === speaker.id ? { ...item, displayName: name } : item) }));
    await renameSpeaker(speaker.id, name);
  };

  const splitSelected = async () => {
    if (!selected || playhead <= selected.startMs || playhead >= selected.endMs) return;
    const result = await splitSpeakerTurn(selected.id, playhead);
    if (result) setData(result);
    else setData((current) => ({ ...current, turns: current.turns.flatMap((turn) => turn.id !== selected.id ? [turn] : [
      { ...turn, endMs: playhead, revision: turn.revision + 1 },
      { ...turn, id: crypto.randomUUID(), startMs: playhead, revision: turn.revision + 1 },
    ]) }));
  };

  const mergeSelected = async () => {
    if (!selected || data.speakers.length < 2) return;
    const target = data.speakers.find((speaker) => speaker.id !== selected.speakerId);
    if (!target || !window.confirm(`รวมช่วงของผู้พูดนี้เข้า “${target.displayName}”? การแก้ไขจะถูกบันทึกเป็น revision ใหม่`)) return;
    const result = await mergeSpeakers(projectId, selected.speakerId, target.id);
    if (result) setData(result);
    else setData((current) => ({
      ...current,
      speakers: current.speakers.filter((speaker) => speaker.id !== selected.speakerId),
      turns: current.turns.map((turn) => turn.speakerId === selected.speakerId ? { ...turn, speakerId: target.id, revision: turn.revision + 1 } : turn),
    }));
    setSelectedId(null);
  };

  const confirmSelected = async () => {
    if (!selected) return;
    setData((current) => ({ ...current, turns: current.turns.map((turn) => turn.id === selected.id ? { ...turn, status: "confirmed" } : turn) }));
    await confirmSpeakerTurn(selected.id);
  };

  const runDiarization = async () => {
    if (!data.recordingId) return;
    setBusy(true);
    try { await startDiarization(projectId, data.recordingId); }
    finally { setBusy(false); }
  };

  const seekFromTrack = (element: HTMLElement, clientX: number) => {
    const bounds = element.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - bounds.left) / bounds.width));
    setPlayhead(Math.round(viewportStart + visibleDuration * ratio));
  };

  const trackPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    pointers.current.set(event.pointerId, event.clientX);
    if (pointers.current.size === 1) seekFromTrack(event.currentTarget, event.clientX);
    if (pointers.current.size === 2) {
      const [first, second] = [...pointers.current.values()];
      pinch.current = { distance: Math.abs(second - first), zoom };
    }
  };

  const trackPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!pointers.current.has(event.pointerId)) return;
    pointers.current.set(event.pointerId, event.clientX);
    if (pointers.current.size === 2 && pinch.current) {
      const [first, second] = [...pointers.current.values()];
      const scale = Math.abs(second - first) / Math.max(1, pinch.current.distance);
      setZoom(Math.min(8, Math.max(1, pinch.current.zoom * scale)));
    }
  };

  const trackPointerUp = (event: React.PointerEvent<HTMLDivElement>) => {
    pointers.current.delete(event.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
  };

  if (!data.recordingId) {
    return <main className="m-screen m-timeline-screen"><header className="m-page-header m-timeline-header"><div><span>เสียงและผู้พูด</span><h1>ไทม์ไลน์ผู้พูด</h1></div></header><section className="m-timeline-empty"><Sparkles size={29} /><strong>ยังไม่มีเสียงสำหรับสร้างไทม์ไลน์</strong><p>เริ่มบันทึกเสียงบนมือถือก่อน แล้วคุณจะติดป้ายผู้พูดด้วยตนเองหรือส่งให้ FUNG Desktop แยกผู้พูดได้</p><button type="button" onClick={onRecord}>เริ่มบันทึกเสียง</button></section></main>;
  }

  if (surface === "story") return <StoryEditor timeline={data} state={studio} setState={setStudio} onBack={() => setSurface("timeline")} onEffects={() => setSurface("processing")} />;
  if (surface === "processing") return <ProcessingStudio state={studio} setState={setStudio} desktopAvailable={desktopAvailable} onBack={() => setSurface("story")} />;

  return (
    <main className="m-screen m-timeline-screen">
      <header className="m-page-header m-timeline-header">
        <div><span>เสียงและผู้พูด</span><h1>ไทม์ไลน์ผู้พูด</h1></div>
        <div className="m-timeline-header-actions"><button onClick={() => setSurface("story")}><BookOpenText />เรื่องเล่า</button><button onClick={() => setSurface("processing")}><SlidersHorizontal />ประมวลผล</button></div>
      </header>
      <div className="m-timeline-truth"><i className={data.source === "desktop" ? "is-connected" : ""} />{data.source === "desktop" ? "ผลจาก FUNG Desktop · ตรวจสอบก่อนยืนยัน" : "โหมด standalone · แก้ป้ายผู้พูดได้บนเครื่อง"}</div>
      <button className="m-diarize-button m-diarize-inline" onClick={runDiarization} disabled={busy || !desktopAvailable}><Sparkles size={17} />{busy ? "กำลังส่งงาน" : desktopAvailable ? "แยกผู้พูด" : "ใช้ Desktop เพื่อแยก"}</button>
      <section className="m-transport" aria-label="ตัวควบคุมการเล่นเสียง">
        <button onClick={() => setPlaying((value) => !value)} aria-label={playing ? "หยุดชั่วคราว" : "เล่น"}>{playing ? <Pause /> : <Play />}</button>
        <strong>{timeLabel(playhead)}</strong><span>/ {timeLabel(data.durationMs)}</span>
        <button onClick={() => setZoom(Math.max(1, zoom / 1.5))} aria-label="ย่อไทม์ไลน์"><ZoomOut size={19} /></button>
        <span className="m-zoom-label">{zoom.toFixed(1)}×</span>
        <button onClick={() => setZoom(Math.min(8, zoom * 1.5))} aria-label="ขยายไทม์ไลน์"><ZoomIn size={19} /></button>
      </section>
      <section className="m-timeline" style={{ "--playhead": position(playhead) / 100 } as React.CSSProperties}>
        <div className="m-time-ruler">{ticks.map((tick) => <time key={tick}>{timeLabel(tick)}</time>)}</div>
        <div className="m-playhead" aria-hidden="true"><i /></div>
        {data.speakers.map((speaker, speakerIndex) => (
          <div className={`m-speaker-lane lane-${speakerIndex % 3}`} key={speaker.id}>
            <button className="m-speaker-label" onClick={() => void updateSpeaker(speaker)}><span>{speaker.displayName}</span><small>{speaker.confidence == null ? "กำหนดเอง" : `${Math.round(speaker.confidence * 100)}% · ข้อเสนอ`}</small><ChevronDown size={14} /></button>
            <div className="m-lane-track" onPointerDown={trackPointerDown} onPointerMove={trackPointerMove} onPointerUp={trackPointerUp} onPointerCancel={trackPointerUp}>
              {data.turns.filter((turn) => turn.speakerId === speaker.id && turn.endMs >= viewportStart && turn.startMs <= viewportStart + visibleDuration).map((turn) => {
                const left = Math.max(0, position(turn.startMs));
                const right = Math.min(100, position(turn.endMs));
                return <button key={turn.id} className={`m-speaker-clip ${selectedId === turn.id ? "is-selected" : ""} ${turn.overlap ? "is-overlap" : ""}`} style={{ left: `${left}%`, width: `${Math.max(2, right - left)}%` }} onClick={() => { setSelectedId(turn.id); setPlayhead(turn.startMs); }} aria-label={`${speaker.displayName} ${timeLabel(turn.startMs)} ถึง ${timeLabel(turn.endMs)}`}>
                  <span aria-hidden="true">{waveBars.map((height, index) => <i key={index} style={{ height: `${height}%` }} />)}</span>{turn.status === "confirmed" && <Check size={13} />}
                </button>;
              })}
            </div>
          </div>
        ))}
      </section>
      {selected && <section className="m-turn-inspector">
        <div><span>ช่วงที่เลือก</span><strong>{data.speakers.find((speaker) => speaker.id === selected.speakerId)?.displayName}</strong><small>{timeLabel(selected.startMs)}–{timeLabel(selected.endMs)} · {selected.confidence == null ? "กำหนดเอง" : `${Math.round(selected.confidence * 100)}% ข้อเสนอ`}</small></div>
        <div className="m-turn-actions">
          <button onClick={() => void updateSpeaker(data.speakers.find((speaker) => speaker.id === selected.speakerId)!)}><UserRoundPen size={18} />เปลี่ยนชื่อ</button>
          <button onClick={splitSelected} disabled={playhead <= selected.startMs || playhead >= selected.endMs}><Scissors size={18} />แยกช่วง</button>
          <button onClick={mergeSelected}><Merge size={18} />รวมผู้พูด</button>
          <button className="is-confirm" onClick={confirmSelected} disabled={selected.status === "confirmed"}><Check size={18} />{selected.status === "confirmed" ? "ยืนยันแล้ว" : "ยืนยัน"}</button>
        </div>
      </section>}
      <p className="m-timeline-note">ป้ายผู้พูดเป็นการจัดกลุ่มเสียง ไม่ใช่การยืนยันตัวบุคคล · ต้นฉบับเสียงไม่ถูกแก้ไข</p>
    </main>
  );
}
