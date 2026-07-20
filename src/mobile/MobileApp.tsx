import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowLeft,
  AudioLines,
  Check,
  ChevronRight,
  CircleStop,
  FileText,
  HardDrive,
  Home,
  Link2,
  LockKeyhole,
  Mic,
  Monitor,
  Moon,
  Network,
  Pause,
  Play,
  Plus,
  Radio,
  Search,
  Server,
  ShieldCheck,
  Smartphone,
  Sparkles,
  Sun,
  Trash2,
  Users,
  Wifi,
  WifiOff,
  X,
} from "lucide-react";
import { addNote, loadSnapshot, pairDevice, removeDevice, saveSnapshot } from "./mobileStore";
import { appendCaptureSegment, controlNativeRecorder, finishCapture, loadPlaybackSegment, nativeRecorderStatus, pairDesktop, persistNote, queryGraph, reconcileNativeCapture, setMcpEnabled, startCapture, startNativeRecorder } from "./bridge";
import { acquireCaptureBackend, CaptureStartError, resumeCaptureClock } from "./captureOrchestration";
import type { CaptureState, EpistemicStatus, MobileNote, MobileSnapshot, MobileTab, ThemePreference } from "./model";
import { TimelineScreen } from "./TimelineScreen";
import "./mobile.css";

const idleCapture: CaptureState = {
  state: "idle",
  recordingId: null,
  startedAt: null,
  pausedAt: null,
  elapsedMs: 0,
  safeOffsetMs: 0,
  segmentCount: 0,
  storageWarning: false,
  backend: null,
  error: null,
};

const formatClock = (milliseconds: number) => {
  const seconds = Math.floor(milliseconds / 1000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  return [hours, minutes, rest].map((value) => value.toString().padStart(2, "0")).join(":");
};

function Waveform({ active = false }: { active?: boolean }) {
  const bars = [12, 24, 38, 18, 31, 17, 45, 29, 20, 42, 23, 34, 16, 27, 39, 19, 31, 14];
  return (
    <div className={`m-waveform ${active ? "is-active" : ""}`} aria-hidden="true">
      {bars.map((height, index) => <i key={index} style={{ height }} />)}
    </div>
  );
}

function StatusMark({ children }: { children: React.ReactNode }) {
  return <span className="m-status"><ShieldCheck size={16} />{children}</span>;
}

function BottomNavigation({ tab, onChange }: { tab: MobileTab; onChange: (tab: MobileTab) => void }) {
  const items = [
    { id: "home" as const, label: "หน้าหลัก", icon: Home },
    { id: "notes" as const, label: "โน้ต", icon: FileText },
    { id: "voice" as const, label: "พูด", icon: Mic, primary: true },
    { id: "graph" as const, label: "กราฟ", icon: Network },
    { id: "devices" as const, label: "อุปกรณ์", icon: Server },
  ];
  return (
    <nav className="m-bottom-nav" aria-label="เมนูหลัก">
      {items.map(({ id, label, icon: Icon, primary }) => (
        <button
          key={id}
          type="button"
          className={`${primary ? "m-nav-voice" : "m-nav-item"} ${tab === id ? "is-active" : ""}`}
          onClick={() => onChange(id)}
          aria-current={tab === id ? "page" : undefined}
        >
          <Icon size={primary ? 31 : 23} strokeWidth={1.8} />
          <span>{label}</span>
        </button>
      ))}
    </nav>
  );
}

type ScreenProps = {
  snapshot: MobileSnapshot;
  setSnapshot: (snapshot: MobileSnapshot) => void;
  capture: CaptureState;
  setCapture: React.Dispatch<React.SetStateAction<CaptureState>>;
  go: (tab: MobileTab) => void;
  theme: ThemePreference;
  cycleTheme: () => void;
};

function HomeScreen({ snapshot, capture, go }: ScreenProps) {
  const recent = snapshot.notes[0];
  const isRecording = capture.state === "recording";
  return (
    <main className="m-screen m-home-screen">
      <header className="m-header">
        <span className="m-wordmark">FUNG</span>
        <StatusMark>พร้อมใช้งานบนมือถือ</StatusMark>
      </header>
      <section className="m-home-intro">
        <h1>วันนี้</h1>
        <div className={`m-voice-orbit ${isRecording ? "is-recording" : ""}`}>
          <Waveform active={isRecording} />
          <button className="m-voice-core" type="button" onClick={() => go("voice")} aria-label="เปิดการสั่งงานด้วยเสียง">
            {isRecording ? <Radio size={54} /> : <Mic size={61} strokeWidth={1.7} />}
            <strong>{isRecording ? "กำลังบันทึก" : "กดค้างเพื่อพูด"}</strong>
            {isRecording && <span>{formatClock(capture.elapsedMs)}</span>}
          </button>
        </div>
      </section>
      <section className="m-quick-actions" aria-label="คำสั่งด่วน">
        <button type="button" onClick={() => go("voice")}><CircleStop size={29} /><span>เริ่มบันทึก</span></button>
        <button type="button" onClick={() => go("notes")}><FileText size={29} /><span>สร้างโน้ต</span></button>
      </section>
      <section className="m-recent">
        <div className="m-section-title"><h2>งานล่าสุด</h2><button onClick={() => go("notes")}>ดูทั้งหมด <ChevronRight size={18} /></button></div>
        {recent && (
          <button className="m-recent-row" type="button" onClick={() => go("notes")}>
            <span className="m-recent-icon"><Users size={25} /></span>
            <span><strong>{recent.title}</strong><small>{recent.evidenceLabel}</small></span>
            <time>{new Date(recent.updatedAt).toLocaleTimeString("th-TH", { hour: "2-digit", minute: "2-digit" })}</time>
            <ChevronRight size={21} />
          </button>
        )}
      </section>
    </main>
  );
}

function CaptureScreen({ snapshot, capture, setCapture, go }: ScreenProps) {
  const mediaRecorder = useRef<MediaRecorder | null>(null);
  const stream = useRef<MediaStream | null>(null);
  const chunkStartedAt = useRef(0);
  const pendingWrites = useRef<Promise<void>[]>([]);
  const playback = useRef<HTMLAudioElement | null>(null);
  const playbackUrl = useRef<string | null>(null);
  const [playing, setPlaying] = useState(false);

  useEffect(() => () => {
    playback.current?.pause();
    if (playbackUrl.current) URL.revokeObjectURL(playbackUrl.current);
  }, []);

  const playSequence = async (sequence: number): Promise<void> => {
    if (!capture.recordingId) return;
    const segment = await loadPlaybackSegment(capture.recordingId, sequence);
    if (!segment) throw new Error("playback is available in the installed app");
    if (playbackUrl.current) URL.revokeObjectURL(playbackUrl.current);
    const url = URL.createObjectURL(new Blob([Uint8Array.from(segment.bytes)], { type: segment.mimeType }));
    playbackUrl.current = url;
    const audio = new Audio(url);
    playback.current = audio;
    audio.onended = () => { if (segment.hasNext) void playSequence(sequence + 1); else setPlaying(false); };
    audio.onerror = () => setPlaying(false);
    await audio.play();
    setPlaying(true);
  };

  const togglePlayback = () => {
    if (playing) { playback.current?.pause(); setPlaying(false); return; }
    if (playback.current?.paused && playback.current.currentTime > 0 && !playback.current.ended) { void playback.current.play().then(() => setPlaying(true)); return; }
    void playSequence(1).catch(() => setPlaying(false));
  };

  const syncNative = async (recordingId: string) => {
    const status = await nativeRecorderStatus(recordingId);
    const session = await reconcileNativeCapture(recordingId, status.segments);
    setCapture((current) => ({
      ...current,
      state: status.state === "recovery_required" ? "recovery_required" : current.state,
      safeOffsetMs: session?.safeOffsetMs ?? status.safeOffsetMs,
      segmentCount: session?.segmentCount ?? status.segmentCount,
    }));
    return status;
  };

  useEffect(() => {
    if (capture.state !== "recording" || !capture.startedAt) return;
    const timer = window.setInterval(() => {
      setCapture((current) => ({ ...current, elapsedMs: Date.now() - (current.startedAt ?? Date.now()) }));
    }, 250);
    return () => window.clearInterval(timer);
  }, [capture.state, capture.startedAt, setCapture]);

  useEffect(() => {
    if (capture.backend !== "android-native" || !capture.recordingId || (capture.state !== "recording" && capture.state !== "paused")) return;
    const recordingId = capture.recordingId;
    const timer = window.setInterval(() => void syncNative(recordingId).catch((error) => {
      setCapture((current) => ({
        ...current,
        state: "recovery_required",
        error: { stage: "native-sync", detail: error instanceof Error ? error.message : String(error) },
      }));
    }), 1000);
    return () => window.clearInterval(timer);
  }, [capture.backend, capture.recordingId, capture.state]);

  const begin = async () => {
    setCapture((current) => ({ ...current, state: "preparing", error: null }));
    try {
      const acquired = await acquireCaptureBackend({
        createSession: () => startCapture(snapshot.projectId),
        startNative: startNativeRecorder,
        openWebAudio: () => navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } }),
        createRecordingId: () => crypto.randomUUID(),
      });
      const { recordingId } = acquired;
      if (acquired.backend === "android-native") {
        setCapture({ ...idleCapture, backend: "android-native", state: "recording", recordingId, startedAt: Date.now() });
        return;
      }
      const media = acquired.media;
      if (!media) throw new Error("WebView media stream is unavailable");
      const recorder = new MediaRecorder(media, { audioBitsPerSecond: 128000 });
      stream.current = media;
      mediaRecorder.current = recorder;
      chunkStartedAt.current = Date.now();
      recorder.ondataavailable = (event) => {
        if (!event.data.size) return;
        const write = (async () => {
          const bytes = new Uint8Array(await event.data.arrayBuffer());
          const durationMs = Math.max(1, Date.now() - chunkStartedAt.current);
          chunkStartedAt.current = Date.now();
          const result = await appendCaptureSegment(recordingId, bytes, durationMs);
          setCapture((current) => ({
            ...current,
            safeOffsetMs: result?.safeOffsetMs ?? current.elapsedMs,
            segmentCount: result?.segmentCount ?? current.segmentCount + 1,
          }));
        })();
        pendingWrites.current.push(write);
      };
      recorder.start(5000);
      setCapture({ ...idleCapture, backend: "web", state: "recording", recordingId, startedAt: Date.now() });
    } catch (error) {
      const failure = error instanceof CaptureStartError
        ? { stage: error.stage, detail: error.detail }
        : { stage: "web-permission" as const, detail: error instanceof Error ? error.message : String(error) };
      setCapture({ ...idleCapture, state: "recovery_required", error: failure });
    }
  };

  const stop = async () => {
    const id = capture.recordingId;
    setCapture((current) => ({ ...current, state: "finalizing" }));
    if (id && capture.backend === "android-native") {
      let settled = await controlNativeRecorder(id, "stop");
      for (let attempt = 0; attempt < 10 && settled.state !== "completed"; attempt += 1) {
        await new Promise((resolve) => window.setTimeout(resolve, 100));
        settled = await nativeRecorderStatus(id);
      }
      if (settled.state !== "completed") throw new Error("native recorder did not finalize safely");
      await reconcileNativeCapture(id, settled.segments);
    } else if (mediaRecorder.current) {
      const recorder = mediaRecorder.current;
      const stopped = new Promise<void>((resolve) => recorder.addEventListener("stop", () => resolve(), { once: true }));
      recorder.requestData();
      recorder.stop();
      await stopped;
      await Promise.all(pendingWrites.current);
      pendingWrites.current = [];
      stream.current?.getTracks().forEach((track) => track.stop());
    }
    if (id) await finishCapture(id);
    window.setTimeout(() => setCapture((current) => ({ ...current, state: "completed", safeOffsetMs: current.elapsedMs })), 350);
  };

  const pause = async () => {
    if (capture.backend === "android-native" && capture.recordingId) {
      const action = capture.state === "paused" ? "resume" : "pause";
      const status = await controlNativeRecorder(capture.recordingId, action);
      await reconcileNativeCapture(capture.recordingId, status.segments);
      const changedAt = Date.now();
      setCapture((current) => ({
        ...current,
        state: action === "pause" ? "paused" : "recording",
        startedAt: action === "resume" ? resumeCaptureClock(current.startedAt, current.pausedAt, changedAt) : current.startedAt,
        pausedAt: action === "pause" ? changedAt : null,
        elapsedMs: action === "pause" && current.startedAt ? changedAt - current.startedAt : current.elapsedMs,
        safeOffsetMs: status.safeOffsetMs,
        segmentCount: status.segmentCount,
      }));
      return;
    }
    const recorder = mediaRecorder.current;
    if (!recorder) return;
    if (recorder.state === "recording") {
      recorder.pause();
      const pausedAt = Date.now();
      setCapture((current) => ({
        ...current,
        state: "paused",
        pausedAt,
        elapsedMs: current.startedAt ? pausedAt - current.startedAt : current.elapsedMs,
      }));
    } else if (recorder.state === "paused") {
      recorder.resume();
      const resumedAt = Date.now();
      setCapture((current) => ({
        ...current,
        state: "recording",
        startedAt: resumeCaptureClock(current.startedAt, current.pausedAt, resumedAt),
        pausedAt: null,
      }));
    }
  };

  const active = capture.state === "recording" || capture.state === "paused";
  return (
    <main className="m-screen m-capture-screen">
      <header className="m-titlebar"><button onClick={() => go("home")} aria-label="กลับ"><ArrowLeft /></button><h1>บันทึกเสียง</h1><span /></header>
      <section className="m-capture-stage">
        <span className={`m-capture-state ${active ? "is-live" : ""}`}><i />{capture.state === "paused" ? "หยุดชั่วคราว" : active ? "กำลังบันทึกบนอุปกรณ์" : "พร้อมบันทึกบนอุปกรณ์"}</span>
        <strong className="m-timer">{formatClock(capture.elapsedMs)}</strong>
        <Waveform active={capture.state === "recording"} />
        <p>{active ? `บันทึกปลอดภัยถึง ${formatClock(capture.safeOffsetMs)}` : "เสียงจะถูกเก็บไว้ในอุปกรณ์นี้ก่อนเสมอ"}</p>
      </section>
      {capture.state === "completed" && <><button className="m-context-action" type="button" onClick={togglePlayback}>{playing ? <Pause size={20} /> : <Play size={20} />}{playing ? "หยุดเล่นเสียง" : "เล่นเสียงต้นฉบับ"}</button><button className="m-context-action" type="button" onClick={() => go("timeline")}><AudioLines size={20} />เปิดไทม์ไลน์ของเสียงนี้</button></>}
      <section className="m-capture-controls">
        {!active && capture.state !== "finalizing" ? (
          <button className="m-record-button" type="button" onClick={begin}><Mic size={34} /><span>เริ่มบันทึก</span></button>
        ) : (
          <>
            <button className="m-round-control" onClick={pause}>{capture.state === "paused" ? <Play /> : <Pause />}</button>
            <button className="m-stop-button" onClick={stop} disabled={capture.state === "finalizing"}><CircleStop size={35} /></button>
            <button className="m-round-control" onClick={() => go("notes")}><FileText /></button>
          </>
        )}
      </section>
      <section className="m-safe-card">
        <HardDrive size={24} />
        <div><strong>{capture.segmentCount || "—"} ส่วนเสียงที่ยืนยันแล้ว</strong><span>ทุก checkpoint ตรวจสอบได้และกู้คืนแยกส่วนได้</span></div>
        <Check size={20} />
      </section>
      {capture.state === "recovery_required" && <div className="m-inline-alert" role="alert"><strong>ยังไม่เริ่มบันทึกเสียง</strong><span>{capture.error?.stage === "native-start" && capture.error.detail.toLowerCase().includes("permission") ? "Android กำลังขอสิทธิ์ไมโครโฟน โปรดอนุญาตแล้วแตะเริ่มบันทึกอีกครั้ง" : capture.error?.stage === "native-sync" ? "การยืนยัน checkpoint จาก native recorder ขัดข้อง เสียงที่ยืนยันแล้วจะยังคงอยู่" : "ไม่สามารถเปิดตัวบันทึกเสียงได้ กรุณาลองใหม่หรือตรวจสิทธิ์ไมโครโฟน"}</span><small>ขั้นตอน: {capture.error?.stage ?? "unknown"}</small></div>}
    </main>
  );
}

function NotesScreen({ snapshot, setSnapshot, go }: ScreenProps) {
  const [query, setQuery] = useState("");
  const [editorOpen, setEditorOpen] = useState(false);
  const [selected, setSelected] = useState<MobileNote | null>(null);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const notes = snapshot.notes.filter((note) => `${note.title} ${note.body}`.toLowerCase().includes(query.toLowerCase()));

  const save = async () => {
    const next = addNote(snapshot, title, body);
    const note = next.notes[0];
    setSnapshot(next);
    await persistNote(note);
    const graph = await queryGraph(note.projectId);
    if (graph) setSnapshot({ ...next, nodes: graph.nodes, edges: graph.edges });
    setTitle("");
    setBody("");
    setEditorOpen(false);
  };

  return (
    <main className="m-screen m-notes-screen">
      <header className="m-page-header"><div><span>พื้นที่ส่วนตัว</span><h1>โน้ต</h1></div><button className="m-icon-action" aria-label="สร้างโน้ตใหม่" onClick={() => setEditorOpen(true)}><Plus /></button></header>
      <label className="m-search"><Search size={19} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="ค้นหาโน้ตและหลักฐาน" /></label>
      <div className="m-filter-line"><button className="is-active">ล่าสุด</button><button>ยืนยันแล้ว</button><button>ข้อเสนอ</button></div>
      <section className="m-note-list">
        {notes.map((note) => (
          <button className="m-note-row" type="button" key={note.id} onClick={() => setSelected(note)}>
            <span className="m-note-date">{new Date(note.updatedAt).toLocaleDateString("th-TH", { day: "2-digit", month: "short" })}</span>
            <span className="m-note-copy"><strong>{note.title}</strong><p>{note.body}</p><small><Link2 size={13} /> {note.evidenceLabel}</small></span>
            <ChevronRight size={20} />
          </button>
        ))}
      </section>
      {editorOpen && (
        <div className="m-sheet-backdrop" role="presentation">
          <section className="m-sheet" role="dialog" aria-modal="true" aria-label="สร้างโน้ต">
            <header><h2>สร้างโน้ต</h2><button onClick={() => setEditorOpen(false)}><X /></button></header>
            <input autoFocus value={title} onChange={(event) => setTitle(event.target.value)} placeholder="ชื่อโน้ต" />
            <textarea value={body} onChange={(event) => setBody(event.target.value)} placeholder="เขียนสิ่งที่ต้องจำ…" />
            <button className="m-primary-button" onClick={save} disabled={!title.trim()}>บันทึกบนอุปกรณ์</button>
          </section>
        </div>
      )}
      {selected && (
        <div className="m-detail-layer">
          <header className="m-titlebar"><button onClick={() => setSelected(null)}><ArrowLeft /></button><span>รายละเอียดโน้ต</span><button><Sparkles size={20} /></button></header>
          <article className="m-note-detail">
            <time>{new Date(selected.updatedAt).toLocaleString("th-TH")}</time>
            <h1>{selected.title}</h1>
            <p>{selected.body}</p>
            <aside><strong><Link2 size={16} /> หลักฐานต้นทาง</strong><span>{selected.evidenceLabel}</span></aside>
            <aside className="is-inference"><strong><Sparkles size={16} /> สถานะความรู้</strong><span>{selected.evidenceLabel?.includes("ข้อเสนอ") ? "ข้อเสนอจากระบบ — รอการยืนยัน" : "ยืนยันแล้ว"}</span></aside>
            <button className="m-context-action" type="button" onClick={() => go("timeline")}><AudioLines size={20} />เปิดไทม์ไลน์ที่เกี่ยวข้อง</button>
          </article>
        </div>
      )}
    </main>
  );
}

const edgeClass = (status: EpistemicStatus) => status === "inferred" ? "is-inferred" : status === "disputed" ? "is-disputed" : "is-confirmed";

function GraphScreen({ snapshot }: ScreenProps) {
  const [selectedId, setSelectedId] = useState(snapshot.nodes[0]?.id ?? "");
  const selected = snapshot.nodes.find((node) => node.id === selectedId);
  return (
    <main className="m-screen m-graph-screen">
      <header className="m-page-header"><div><span>GenesisBlockDB</span><h1>ความสัมพันธ์</h1></div><button className="m-icon-action" aria-label="ค้นหาในกราฟ"><Search /></button></header>
      <section className="m-graph-canvas" aria-label="กราฟความสัมพันธ์ของโน้ต">
        <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          {snapshot.edges.map((edge) => {
            const source = snapshot.nodes.find((node) => node.id === edge.sourceId);
            const target = snapshot.nodes.find((node) => node.id === edge.targetId);
            if (!source || !target) return null;
            return <line key={edge.id} x1={source.x} y1={source.y} x2={target.x} y2={target.y} className={edgeClass(edge.status)} />;
          })}
        </svg>
        {snapshot.nodes.map((node) => (
          <button
            key={node.id}
            className={`m-graph-node is-${node.kind} ${selectedId === node.id ? "is-selected" : ""}`}
            style={{ left: `${node.x}%`, top: `${node.y}%` }}
            onClick={() => setSelectedId(node.id)}
          >{node.label}</button>
        ))}
      </section>
      <div className="m-graph-legend"><span><i />ยืนยันแล้ว</span><span><i className="is-dotted" />ข้อเสนอ</span></div>
      {selected && <section className="m-graph-inspector"><span>โหนดที่เลือก</span><h2>{selected.label}</h2><p>{snapshot.edges.filter((edge) => edge.sourceId === selected.id || edge.targetId === selected.id).length} ความสัมพันธ์ที่ตรวจสอบย้อนกลับได้</p></section>}
    </main>
  );
}

function DevicesScreen({ snapshot, setSnapshot, theme, cycleTheme }: ScreenProps) {
  const [pairing, setPairing] = useState(false);
  const [name, setName] = useState("FUNG Desktop");
  const [endpoint, setEndpoint] = useState("192.168.1.20:8765");
  const [code, setCode] = useState("");
  const [mcpEnabled, setMcpState] = useState(false);
  const [mcpBind, setMcpBind] = useState<string | null>(null);
  const ThemeIcon = theme === "system" ? Monitor : theme === "dark" ? Moon : Sun;
  const themeLabel = theme === "system" ? "ตามระบบ" : theme === "dark" ? "มืด" : "สว่าง";

  const connect = async () => {
    await pairDesktop(name, endpoint, code);
    setSnapshot(pairDevice(snapshot, name, endpoint));
    setPairing(false);
  };

  const toggleMcp = async () => {
    const result = await setMcpEnabled(!mcpEnabled, false);
    setMcpState(result.enabled);
    setMcpBind(result.bind);
  };

  return (
    <main className="m-screen m-devices-screen">
      <header className="m-page-header"><div><span>เครือข่ายภายใน</span><h1>อุปกรณ์</h1></div><button className="m-icon-action" aria-label="เพิ่ม Desktop" onClick={() => setPairing(true)}><Plus /></button></header>
      <section className="m-device-summary"><Smartphone size={30} /><div><strong>มือถือเครื่องนี้</strong><span>พร้อมใช้งานแบบ standalone</span></div><StatusMark>Local</StatusMark></section>
      <h2 className="m-subheading">FUNG Desktop ที่เชื่อถือ</h2>
      <section className="m-device-list">
        {snapshot.devices.length === 0 ? (
          <div className="m-empty-device"><WifiOff size={31} /><strong>ยังไม่ได้เชื่อม Desktop</strong><p>คุณยังบันทึก สร้างโน้ต และเปิดกราฟได้ตามปกติ</p><button onClick={() => setPairing(true)}>เชื่อมต่อผ่าน LAN</button></div>
        ) : snapshot.devices.map((device) => (
          <article className="m-device-row" key={device.id}><span className="m-device-icon"><Server /></span><div><strong>{device.name}</strong><span><Wifi size={14} /> {device.endpoint}</span><small>{device.capabilities.join(" · ")}</small></div><button onClick={() => setSnapshot(removeDevice(snapshot, device.id))} aria-label="เพิกถอนอุปกรณ์"><Trash2 size={19} /></button></article>
        ))}
      </section>
      <section className="m-appearance-panel">
        <div><strong>รูปแบบหน้าจอ</strong><span>ใช้ธีมตามระบบ หรือเลือกสว่าง/มืดสำหรับแอปนี้</span></div>
        <button type="button" onClick={cycleTheme} aria-label={`ธีม ${themeLabel}`}><ThemeIcon size={19} /><span>{themeLabel}</span></button>
      </section>
      <section className="m-mcp-panel"><div className="m-panel-heading"><span><LockKeyhole size={22} /></span><div><strong>MCP บนมือถือ</strong><p>{mcpEnabled ? `เปิดเฉพาะเครื่องนี้ · ${mcpBind}` : "ปิดอยู่ ไม่มีเครื่องมือเข้าถึงข้อมูล"}</p></div><button className={`m-toggle ${mcpEnabled ? "is-on" : ""}`} aria-label={mcpEnabled ? "ปิด MCP" : "เปิด MCP"} aria-pressed={mcpEnabled} onClick={toggleMcp}><i /></button></div><small>ทุก capability ต้องได้รับอนุญาตเป็นรายโปรเจกต์ และระบบจะหยุดเมื่อ lifecycle ไม่อนุญาต</small></section>
      {pairing && <div className="m-sheet-backdrop"><section className="m-sheet"><header><h2>เชื่อม FUNG Desktop</h2><button onClick={() => setPairing(false)}><X /></button></header><label>ชื่ออุปกรณ์<input value={name} onChange={(event) => setName(event.target.value)} /></label><label>LAN endpoint<input value={endpoint} onChange={(event) => setEndpoint(event.target.value)} /></label><label>รหัสยืนยัน<input inputMode="numeric" maxLength={6} value={code} onChange={(event) => setCode(event.target.value.replace(/\D/g, ""))} placeholder="6 หลัก" /></label><p className="m-consent-copy"><ShieldCheck size={17} /> ยืนยันรหัสเดียวกันบน Desktop ก่อนให้สิทธิ์</p><button className="m-primary-button" onClick={connect} disabled={code.length !== 6}>ยืนยันและเชื่อมต่อ</button></section></div>}
    </main>
  );
}

export function MobileApp() {
  const [snapshot, setSnapshotState] = useState(loadSnapshot);
  const [tab, setTab] = useState<MobileTab>("home");
  const [capture, setCapture] = useState<CaptureState>(idleCapture);
  const [theme, setTheme] = useState<ThemePreference>(() => {
    const stored = localStorage.getItem("fung.mobile.theme.v1");
    return stored === "light" || stored === "dark" || stored === "system" ? stored : "system";
  });
  const [systemDark, setSystemDark] = useState(() => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false);
  const dark = theme === "dark" || (theme === "system" && systemDark);

  useEffect(() => {
    document.title = "FUNG Mobile";
  }, []);

  useEffect(() => {
    void queryGraph(snapshot.projectId).then((graph) => {
      if (graph && graph.nodes.length > 0) {
        setSnapshotState((current) => ({ ...current, nodes: graph.nodes, edges: graph.edges }));
      }
    }).catch(() => undefined);
  }, [snapshot.projectId]);

  useEffect(() => {
    localStorage.setItem("fung.mobile.theme.v1", theme);
  }, [theme]);

  useEffect(() => {
    const media = window.matchMedia?.("(prefers-color-scheme: dark)");
    if (!media) return;
    const update = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  const setSnapshot = (next: MobileSnapshot) => {
    setSnapshotState(next);
    saveSnapshot(next);
  };

  const cycleTheme = () => setTheme((current) => current === "system" ? "light" : current === "light" ? "dark" : "system");
  const props = useMemo<ScreenProps>(() => ({ snapshot, setSnapshot, capture, setCapture, go: setTab, theme, cycleTheme }), [snapshot, capture, theme]);
  const screen = tab === "home" ? <HomeScreen {...props} /> : tab === "notes" ? <NotesScreen {...props} /> : tab === "timeline" ? <TimelineScreen projectId={snapshot.projectId} desktopAvailable={snapshot.devices.some((device) => device.trustState === "paired")} onRecord={() => setTab("voice")} /> : tab === "graph" ? <GraphScreen {...props} /> : tab === "devices" ? <DevicesScreen {...props} /> : <CaptureScreen {...props} />;

  return (
    <div className={`m-app ${dark ? "m-theme-dark" : ""}`}>
      {screen}
      <BottomNavigation tab={tab} onChange={setTab} />
    </div>
  );
}
