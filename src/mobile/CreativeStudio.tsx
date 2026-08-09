import { useEffect, useMemo, useState } from "react";
import {
  ArrowLeft, AudioLines, Bookmark, Bot, Check, ChevronRight, CloudDownload, Crop,
  GripVertical, Leaf, Mic2, Monitor, Move, Pause, Play, Plus, Redo2, Scissors,
  Send, Settings2, ShieldCheck, SlidersHorizontal, Sparkles, Square, Undo2, Upload,
  VolumeX, WandSparkles, X,
} from "lucide-react";
import type {
  CreativeStudioState, DelegatedJob, EffectKind, EffectNode, ModelPackage, StoryClip, StorySequence,
  TimelineData,
} from "./model";
import { createStory, delegateTranscription, desktopEndpoint, desktopReachable, moveStoryClip, onDeviceAiStatus, pollDelegatedJob, queryStory, reviewRefinement, setAgentVoiceGrant, splitStoryClip, startProcessingJob, storyHistory, trimStoryClip, updateEffectChain, type OnDeviceAiStatus } from "./bridge";
import { listModelProviders, type ModelProvider } from "../tauri";

const STUDIO_KEY = "fung.mobile.creative-studio.v1";
// Must match MobileApp.tsx's DEVICE_ID_KEY — this mobile device's Supabase
// `devices.id`, cached at registration time.
const DEVICE_ID_KEY = "fung.device.id";
// Mirrors `on_device_ai::Admission::Deferred`'s reason strings
// (src-tauri/src/on_device_ai.rs) — anything else (`model_package_pending_approval`,
// meaning admitted but no package installed yet, or `android_profile_unavailable`,
// meaning the probe itself couldn't run) is not a resource-driven deferral and
// isn't treated as "recommend the desktop" here.
const DEFERRED_ADMISSION_REASONS = new Set([
  "device_tier_core", "capture_active", "thermal_severe", "memory_pressure", "battery_low", "device_tier_insufficient",
]);
const DELEGATE_POLL_MS = 1_500;
const delegateStateLabel = (state: DelegatedJob["state"]) => ({
  queued: "รอคิว", running: "กำลังถอดเสียง", paused: "หยุดชั่วคราว",
  completed: "เสร็จสิ้น", failed: "ล้มเหลว", cancelled: "ยกเลิกแล้ว",
})[state];
const effectLabels: Record<EffectKind, string> = {
  pitch_shift: "Pitch", reverb: "Reverb", delay: "Delay", compressor: "Compressor", low_pass: "Low-pass",
};
const effectDefaults: Record<EffectKind, Record<string, number>> = {
  pitch_shift: { semitones: 0 }, reverb: { room: 0.7 }, delay: { milliseconds: 120 }, compressor: { threshold: -18 }, low_pass: { hertz: 8000 },
};
const waveBars = Array.from({ length: 28 }, (_, index) => 24 + ((index * 19) % 62));

const timeLabel = (milliseconds: number) => {
  const total = Math.max(0, Math.floor(milliseconds / 1000));
  return `${Math.floor(total / 60).toString().padStart(2, "0")}:${(total % 60).toString().padStart(2, "0")}`;
};

const makeStory = (timeline: TimelineData, projectId: string): StorySequence => ({
  id: `story-${projectId}`,
  projectId,
  title: "บทสนทนาวันศุกร์",
  durationMs: Math.max(timeline.durationMs, 180_000),
  revision: 1,
  clips: timeline.turns.map((turn) => ({
    id: `story-${turn.id}`,
    sourceTurnId: turn.id,
    sourceRecordingId: turn.recordingId,
    sourceStartMs: turn.startMs,
    sourceEndMs: turn.endMs,
    timelineStartMs: turn.startMs,
    speakerId: turn.speakerId,
    effectChainId: null,
    revision: 1,
  })),
});

const initialStudio = (timeline: TimelineData, projectId: string): CreativeStudioState => ({
  story: makeStory(timeline, projectId), undo: [], redo: [], selectedModelId: "whisper-large-v3",
  proposals: [{
    id: "proposal-friday", originalText: "ผมว่าเราน่าจะส่งของวันศุกร์นะ",
    proposedText: "ผมว่าเราน่าจะส่งของวันศุกร์นะครับ", status: "proposed", policy: "แก้คำฟุ่มเฟือยและวรรคตอนเท่านั้น",
  }],
  effects: [
    { id: "fx-pitch", kind: "pitch_shift", label: "Pitch", bypassed: false, parameters: effectDefaults.pitch_shift },
    { id: "fx-reverb", kind: "reverb", label: "Reverb", bypassed: false, parameters: effectDefaults.reverb },
    { id: "fx-compressor", kind: "compressor", label: "Compressor", bypassed: true, parameters: effectDefaults.compressor },
    { id: "fx-low-pass", kind: "low_pass", label: "Low-pass", bypassed: true, parameters: effectDefaults.low_pass },
  ],
  voiceProfile: { id: "owned-voice-1", displayName: "เสียงของฉัน", rightsBasis: "owned_recording", rightsState: "valid", providerAvailable: false },
  agentVoiceGrant: false,
  updatedAt: new Date().toISOString(),
});

export function useCreativeStudio(timeline: TimelineData, projectId: string) {
  const [state, setState] = useState<CreativeStudioState>(() => {
    try {
      const raw = localStorage.getItem(STUDIO_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as CreativeStudioState;
        if (parsed.story.projectId === projectId) return parsed;
      }
    } catch { /* Ignore malformed local state. */ }
    return initialStudio(timeline, projectId);
  });
  useEffect(() => localStorage.setItem(STUDIO_KEY, JSON.stringify(state)), [state]);
  useEffect(() => {
    if (timeline.source === "preview") return;
    void queryStory(projectId).then((story) => story ?? createStory(projectId, "บทสนทนาวันศุกร์")).then((story) => {
      if (story) setState((current) => ({ ...current, story, undo: [], redo: [], updatedAt: new Date().toISOString() }));
    }).catch(() => undefined);
  }, [projectId, timeline.recordingId, timeline.source]);
  return [state, setState] as const;
}

type StoryProps = {
  timeline: TimelineData;
  state: CreativeStudioState;
  setState: React.Dispatch<React.SetStateAction<CreativeStudioState>>;
  onBack: () => void;
  onEffects: () => void;
};

export function StoryEditor({ timeline, state, setState, onBack, onEffects }: StoryProps) {
  const [selectedId, setSelectedId] = useState(state.story.clips[0]?.id ?? "");
  const [playing, setPlaying] = useState(false);
  const [playhead, setPlayhead] = useState(42_000);
  const selected = state.story.clips.find((clip) => clip.id === selectedId) ?? null;
  const ticks = useMemo(() => Array.from({ length: 7 }, (_, i) => state.story.durationMs / 6 * i), [state.story.durationMs]);

  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => setPlayhead((value) => value >= state.story.durationMs ? 0 : value + 250), 250);
    return () => window.clearInterval(timer);
  }, [playing, state.story.durationMs]);

  const commit = (operation: (story: StorySequence) => StorySequence) => setState((current) => ({
    ...current,
    undo: [...current.undo.slice(-29), current.story], redo: [],
    story: operation(current.story), updatedAt: new Date().toISOString(),
  }));

  const mutateSelected = (mutate: (clip: StoryClip) => StoryClip) => {
    if (!selected) return;
    commit((story) => ({ ...story, revision: story.revision + 1, clips: story.clips.map((clip) => clip.id === selected.id ? mutate(clip) : clip) }));
  };

  const split = () => {
    if (!selected || selected.sourceEndMs - selected.sourceStartMs < 2_000) return;
    const midpoint = Math.round((selected.sourceStartMs + selected.sourceEndMs) / 2);
    commit((story) => ({
      ...story, revision: story.revision + 1,
      clips: story.clips.flatMap((clip) => clip.id !== selected.id ? [clip] : [
        { ...clip, sourceEndMs: midpoint, revision: clip.revision + 1 },
        { ...clip, id: crypto.randomUUID(), sourceStartMs: midpoint, timelineStartMs: clip.timelineStartMs + (midpoint - clip.sourceStartMs), revision: clip.revision + 1 },
      ]),
    }));
    void splitStoryClip(selected.id, midpoint).then((story) => story && setState((current) => ({ ...current, story }))).catch(() => undefined);
  };

  const undo = () => setState((current) => {
    const previous = current.undo[current.undo.length - 1]; if (!previous) return current;
    return { ...current, story: previous, undo: current.undo.slice(0, -1), redo: [current.story, ...current.redo].slice(0, 30), updatedAt: new Date().toISOString() };
  });
  const redo = () => setState((current) => {
    const next = current.redo[0]; if (!next) return current;
    return { ...current, story: next, undo: [...current.undo, current.story].slice(-30), redo: current.redo.slice(1), updatedAt: new Date().toISOString() };
  });

  const undoWithNative = () => { undo(); void storyHistory(state.story.id, "undo").then((story) => story && setState((current) => ({ ...current, story }))).catch(() => undefined); };
  const redoWithNative = () => { redo(); void storyHistory(state.story.id, "redo").then((story) => story && setState((current) => ({ ...current, story }))).catch(() => undefined); };

  const exportManifest = () => {
    const manifest = JSON.stringify({ format: "fung-story-v1", exportedAt: new Date().toISOString(), story: state.story, sourceImmutable: true }, null, 2);
    const url = URL.createObjectURL(new Blob([manifest], { type: "application/json" }));
    const anchor = document.createElement("a"); anchor.href = url; anchor.download = "fung-story-manifest.json"; anchor.click(); URL.revokeObjectURL(url);
  };

  return <main className="m-screen m-story-screen">
    <header className="m-studio-titlebar"><button onClick={onBack} aria-label="กลับ"><ArrowLeft /></button><h1>เรื่องเล่า</h1><button aria-label="ตัวเลือก"><Settings2 /></button></header>
    <div className="m-story-name"><h2>{state.story.title}</h2><span>revision {state.story.revision}</span></div>
    <section className="m-story-transport"><button onClick={() => setPlaying((value) => !value)}>{playing ? <Pause /> : <Play />}</button><strong>{timeLabel(playhead)} / {timeLabel(state.story.durationMs)}</strong><span /><button onClick={undoWithNative} disabled={!state.undo.length}><Undo2 /></button><button onClick={redoWithNative} disabled={!state.redo.length}><Redo2 /></button></section>
    <section className="m-story-timeline" style={{ "--story-playhead": playhead / state.story.durationMs } as React.CSSProperties}>
      <div className="m-story-ruler">{ticks.map((tick) => <time key={tick}>{timeLabel(tick)}</time>)}</div><i className="m-story-playhead" />
      {timeline.speakers.map((speaker, laneIndex) => <div className={`m-story-lane story-lane-${laneIndex % 3}`} key={speaker.id}>
        <div className="m-story-speaker"><span /><strong>{speaker.displayName}</strong><GripVertical size={16} /></div>
        <div className="m-story-track">{state.story.clips.filter((clip) => clip.speakerId === speaker.id).map((clip) => {
          const width = (clip.sourceEndMs - clip.sourceStartMs) / state.story.durationMs * 100;
          const left = clip.timelineStartMs / state.story.durationMs * 100;
          return <button key={clip.id} className={`m-story-clip ${selectedId === clip.id ? "is-selected" : ""}`} style={{ left: `${left}%`, width: `${Math.max(5, width)}%` }} onClick={() => { setSelectedId(clip.id); setPlayhead(clip.timelineStartMs); }}><span>{waveBars.map((height, index) => <i key={index} style={{ height: `${height}%` }} />)}</span>{selectedId === clip.id && <><b className="is-left" /><b className="is-right" /></>}</button>;
        })}</div>
      </div>)}
    </section>
    {selected && <section className="m-story-inspector"><div className="m-story-selection"><span className="m-lane-dot" /><strong>{timeline.speakers.find((speaker) => speaker.id === selected.speakerId)?.displayName}</strong><time>{timeLabel(selected.sourceStartMs)}–{timeLabel(selected.sourceEndMs)}</time><button onClick={onEffects}><SlidersHorizontal size={18} /> เอฟเฟกต์ <ChevronRight size={17} /></button></div><p>เราควรยืนยันแผนก่อนส่งให้ทีม</p><div className="m-story-mini-wave">{waveBars.concat(waveBars).map((height, index) => <i key={index} style={{ height: `${height}%` }} />)}</div><div className="m-story-tools"><button onClick={split}><Scissors />แยก</button><button onClick={() => { const nextStart = Math.min(selected.sourceEndMs - 500, selected.sourceStartMs + 500); mutateSelected((clip) => ({ ...clip, sourceStartMs: nextStart, revision: clip.revision + 1 })); void trimStoryClip(selected.id, nextStart, selected.sourceEndMs).then((story) => story && setState((current) => ({ ...current, story }))).catch(() => undefined); }}><Crop />ตัดขอบ</button><button onClick={() => { const nextStart = Math.min(state.story.durationMs - 500, selected.timelineStartMs + 5_000); mutateSelected((clip) => ({ ...clip, timelineStartMs: nextStart, revision: clip.revision + 1 })); void moveStoryClip(selected.id, nextStart).then((story) => story && setState((current) => ({ ...current, story }))).catch(() => undefined); }}><Move />ย้าย</button><button onClick={exportManifest}><Upload />ส่งออก</button></div><aside><ShieldCheck /><span><strong>ต้นฉบับเสียงยังคงเดิม</strong> การตัดและย้ายเปลี่ยนเฉพาะข้อมูลลำดับ ไม่แก้ไฟล์ต้นฉบับ</span></aside></section>}
  </main>;
}

const modelPackages: ModelPackage[] = [
  { id: "whisper-base", label: "Whisper Base", sizeLabel: "74M", installed: true, compatible: true, runtimeLocation: "desktop", languages: 99 },
  { id: "whisper-small", label: "Whisper Small", sizeLabel: "244M", installed: true, compatible: true, runtimeLocation: "desktop", languages: 99 },
  { id: "whisper-medium", label: "Whisper Medium", sizeLabel: "769M", installed: false, compatible: true, runtimeLocation: "desktop", languages: 99 },
  { id: "whisper-large-v3", label: "Whisper Large", sizeLabel: "1.5B", installed: true, compatible: true, runtimeLocation: "desktop", languages: 99 },
  { id: "whisper-turbo", label: "Whisper Turbo", sizeLabel: "809M", installed: false, compatible: true, runtimeLocation: "desktop", languages: 99 },
];

type ProcessingProps = {
  state: CreativeStudioState;
  setState: React.Dispatch<React.SetStateAction<CreativeStudioState>>;
  desktopAvailable: boolean;
  pairedDesktopId: string | null;
  recordingId: string | null;
  onBack: () => void;
};

export function ProcessingStudio({ state, setState, desktopAvailable, pairedDesktopId, recordingId, onBack }: ProcessingProps) {
  const [tab, setTab] = useState<"transcribe" | "refine" | "effects" | "voice">("transcribe");
  const [jobState, setJobState] = useState<"idle" | "submitting" | "queued" | "unavailable" | "failed">("idle");
  const [localAi, setLocalAi] = useState<OnDeviceAiStatus | null>(null);
  const [ttsProviders, setTtsProviders] = useState<ModelProvider[]>([]);
  const [selectedTtsId, setSelectedTtsId] = useState<string>("");
  const [desktopEndpointValue, setDesktopEndpointValue] = useState<string | null>(null);
  const [desktopReachableValue, setDesktopReachableValue] = useState<boolean | null>(null);
  const [delegatedJob, setDelegatedJob] = useState<DelegatedJob | null>(null);
  const [delegating, setDelegating] = useState(false);
  const [delegateError, setDelegateError] = useState<string | null>(null);
  useEffect(() => { void onDeviceAiStatus().then(setLocalAi).catch(() => setLocalAi(null)); }, []);
  useEffect(() => {
    void listModelProviders().then((all) => {
      const tts = all.filter((p) => p.kind === "tts" && p.enabled);
      setTtsProviders(tts);
    }).catch(() => setTtsProviders([]));
  }, []);

  // Resolve the paired desktop's LAN endpoint and probe reachability
  // whenever the pairing changes — gates the "ถอดเสียงบน FUNG Desktop" button.
  useEffect(() => {
    if (!pairedDesktopId) { setDesktopEndpointValue(null); setDesktopReachableValue(false); return; }
    let cancelled = false;
    setDesktopReachableValue(null);
    void (async () => {
      const endpoint = await desktopEndpoint(pairedDesktopId);
      if (cancelled) return;
      setDesktopEndpointValue(endpoint);
      const ownDeviceId = localStorage.getItem(DEVICE_ID_KEY);
      if (!endpoint || !ownDeviceId) { setDesktopReachableValue(false); return; }
      const reachable = await desktopReachable(pairedDesktopId, endpoint, ownDeviceId).catch(() => false);
      if (!cancelled) setDesktopReachableValue(reachable);
    })();
    return () => { cancelled = true; };
  }, [pairedDesktopId]);

  // Poll the delegated job every 1.5s until it leaves queued/running.
  useEffect(() => {
    if (!delegatedJob || (delegatedJob.state !== "queued" && delegatedJob.state !== "running")) return;
    const jobId = delegatedJob.id;
    const timer = window.setInterval(() => {
      void pollDelegatedJob(jobId).then((job) => {
        if (job) setDelegatedJob((current) => (current && current.id === jobId ? { ...job, executorDeviceId: current.executorDeviceId } : current));
      }).catch(() => undefined);
    }, DELEGATE_POLL_MS);
    return () => window.clearInterval(timer);
  }, [delegatedJob?.id, delegatedJob?.state]);

  const desktopJobBusy = delegating || (delegatedJob != null && (delegatedJob.state === "queued" || delegatedJob.state === "running"));
  const showDelegateRecommendation = !!localAi && DEFERRED_ADMISSION_REASONS.has(localAi.reason);

  const delegateToDesktop = async () => {
    // Double-submit guard (I8): a job already in flight, or a submit already
    // in progress, must not be able to fire a second delegate.
    if (desktopJobBusy || !pairedDesktopId || !recordingId || !desktopEndpointValue) return;
    const ownDeviceId = localStorage.getItem(DEVICE_ID_KEY);
    if (!ownDeviceId) { setDelegateError("ยังไม่พบตัวระบุอุปกรณ์นี้ — เข้าสู่ระบบก่อน"); return; }
    setDelegating(true);
    setDelegateError(null);
    try {
      const result = await delegateTranscription(state.story.projectId, recordingId, pairedDesktopId, desktopEndpointValue, ownDeviceId);
      if (result) {
        setDelegatedJob({ id: result.jobId, operation: "transcript.transcribe", state: "queued", progress: 0, executorDeviceId: pairedDesktopId });
      } else {
        setDelegateError("ส่งงานไม่สำเร็จ ลองใหม่");
      }
    } catch {
      setDelegateError("ส่งงานไม่สำเร็จ ลองใหม่");
    } finally {
      setDelegating(false);
    }
  };
  const proposal = state.proposals[0];
  const persistEffects = (effects: EffectNode[]) => void updateEffectChain(state.story.projectId, "project", state.story.projectId, "Story processing", effects).catch(() => undefined);
  const toggleEffect = (id: string) => setState((current) => { const effects = current.effects.map((effect) => effect.id === id ? { ...effect, bypassed: !effect.bypassed } : effect); persistEffects(effects); return { ...current, effects, updatedAt: new Date().toISOString() }; });
  const removeEffect = (id: string) => setState((current) => { const effects = current.effects.filter((effect) => effect.id !== id); persistEffects(effects); return { ...current, effects, updatedAt: new Date().toISOString() }; });
  const addEffect = () => setState((current) => {
    const kinds = Object.keys(effectLabels) as EffectKind[];
    const kind = kinds.find((item) => !current.effects.some((effect) => effect.kind === item));
    if (!kind) return current;
    const node: EffectNode = { id: crypto.randomUUID(), kind, label: effectLabels[kind], bypassed: false, parameters: effectDefaults[kind] };
    const effects = [...current.effects, node]; persistEffects(effects); return { ...current, effects, updatedAt: new Date().toISOString() };
  });
  const reviewProposal = (status: "accepted" | "rejected") => { setState((current) => ({ ...current, proposals: current.proposals.map((item) => item.id === proposal.id ? { ...item, status } : item), updatedAt: new Date().toISOString() })); void reviewRefinement(proposal.id, status).catch(() => undefined); };
  const setGrant = () => {
    if (ttsProviders.length === 0) {
      window.alert("ยังไม่ได้ลงทะเบียน TTS provider — ไปตั้งค่าที่ Settings ก่อน");
      return;
    }
    if (!selectedTtsId) {
      window.alert("เลือก TTS provider ก่อนเปิดสิทธิ์");
      return;
    }
    const next = !state.agentVoiceGrant;
    setState((current) => ({ ...current, agentVoiceGrant: next, updatedAt: new Date().toISOString() }));
    void setAgentVoiceGrant(state.story.projectId, state.voiceProfile.id, next).catch(() => undefined);
  };
  const startTranscription = async () => {
    if (!desktopAvailable || jobState === "submitting") return;
    setJobState("submitting");
    try {
      const job = await startProcessingJob(state.story.projectId, "transcript.refine", `model:${state.selectedModelId}`);
      setJobState(job?.state === "queued" ? "queued" : "unavailable");
    } catch { setJobState("failed"); }
  };

  return <main className="m-screen m-processing-screen">
    <header className="m-studio-titlebar"><button onClick={onBack} aria-label="กลับ"><ArrowLeft /></button><h1>ประมวลผลเสียง</h1><span /></header>
    <nav className="m-processing-tabs" aria-label="ส่วนประมวลผล">{[
      ["transcribe", "ถอดเสียง", AudioLines], ["refine", "ปรับข้อความ", WandSparkles], ["effects", "เอฟเฟกต์", SlidersHorizontal], ["voice", "เสียง Agent", Bot],
    ].map(([id, label, Icon]) => <button key={id as string} className={tab === id ? "is-active" : ""} onClick={() => setTab(id as typeof tab)}><Icon size={19} />{label as string}</button>)}</nav>
    <section className="m-runtime-truth"><Monitor /><span><strong>FUNG Desktop</strong> · ภายในเครือข่าย</span><small className={desktopAvailable ? "is-ready" : ""}>{desktopAvailable ? "พร้อมประมวลผล" : "ใช้งานได้แม้ไร้ Desktop"}</small></section>
    <section className="m-runtime-truth"><Settings2 /><span><strong>AI บนมือถือ</strong> · {localAi ? localAi.tier.replace("ai_", "AI ").replace("core", "Core") : "กำลังตรวจสอบ"}</span><small>{localAi?.reason === "model_package_pending_approval" ? "ยังไม่มี package ที่อนุมัติ" : localAi?.reason === "device_tier_core" ? "อุปกรณ์ยังไม่ถึง AI Lite" : localAi?.reason === "android_profile_unavailable" ? "เปิดใน Android app เพื่อตรวจสอบ" : "ต้องเปิดใน Android app เพื่อตรวจสอบ"}</small></section>
    <section className={`m-processing-panel ${tab === "transcribe" ? "is-focus" : ""}`}><header><h2>เลือกโมเดล Whisper</h2><button>เกี่ยวกับโมเดล <ChevronRight /></button></header><div className="m-model-list">{modelPackages.map((model) => <button key={model.id} className={state.selectedModelId === model.id ? "is-selected" : ""} onClick={() => model.installed && setState((current) => ({ ...current, selectedModelId: model.id, updatedAt: new Date().toISOString() }))}><i /><span>{model.label}</span><small>{model.installed ? "ติดตั้งแล้ว" : "ยังไม่ติดตั้ง"}</small><em>{model.sizeLabel}</em>{!model.installed && <CloudDownload size={16} />}</button>)}</div><div className="m-model-balance"><Leaf /><span>สมดุลคุณภาพ–ความเร็ว · 99 ภาษา · snapshot จาก Desktop</span><div><i /><i /><i /><i className="is-on" /><i /></div></div><button className="m-process-start" disabled={!desktopAvailable || jobState === "submitting"} onClick={startTranscription}><Play />{jobState === "queued" ? "ส่งงานเข้าคิวแล้ว" : jobState === "submitting" ? "กำลังส่งงาน…" : jobState === "unavailable" ? "เปิดในแอปเพื่อส่งงาน" : jobState === "failed" ? "ส่งงานไม่สำเร็จ · ลองใหม่" : desktopAvailable ? "เริ่มประมวลผล" : "เชื่อม Desktop เพื่อเริ่ม"}</button>
      {pairedDesktopId && recordingId && (
        <div className="m-delegate-panel">
          {showDelegateRecommendation && <p className="m-delegate-recommend"><Sparkles size={14} />อุปกรณ์นี้ประมวลผลเองไม่ไหว — ส่งไปที่ FUNG Desktop</p>}
          {desktopReachableValue && (
            <button type="button" className="m-delegate-button" disabled={desktopJobBusy} onClick={() => void delegateToDesktop()}>
              <Send size={16} />{delegating ? "กำลังส่งงาน…" : "ถอดเสียงบน FUNG Desktop"}
            </button>
          )}
          {delegatedJob && (
            <div className="m-delegate-progress">
              <div className="m-delegate-progress-bar"><i style={{ width: `${delegatedJob.progress}%` }} /></div>
              <span>{delegateStateLabel(delegatedJob.state)} · {delegatedJob.progress}%</span>
            </div>
          )}
          {(delegatedJob?.error ?? delegateError) && <p className="m-delegate-error">{delegatedJob?.error ?? delegateError}</p>}
        </div>
      )}
    </section>
    <section className={`m-processing-panel ${tab === "refine" ? "is-focus" : ""}`}><header><h2>ปรับแต่งการถอดเสียง</h2><Sparkles /></header><div className="m-refinement-diff"><div><span>ต้นฉบับ</span><p>{proposal.originalText}</p></div><div className="is-proposed"><span>ข้อเสนอ · {proposal.status === "proposed" ? "ยังไม่ยืนยัน" : proposal.status === "accepted" ? "ยืนยันแล้ว" : "ปฏิเสธแล้ว"}</span><p>{proposal.proposedText}</p></div><footer><small><WandSparkles /> {proposal.policy}</small>{proposal.status === "proposed" && <span><button onClick={() => reviewProposal("rejected")}>ปฏิเสธ</button><button onClick={() => reviewProposal("accepted")}>ยืนยันการปรับ</button></span>}</footer></div></section>
    <section className={`m-processing-panel ${tab === "effects" ? "is-focus" : ""}`}><header><h2>เอฟเฟกต์เสียง</h2><SlidersHorizontal /></header><div className="m-effect-chain">{state.effects.map((effect) => <button key={effect.id} className={`${effect.bypassed ? "is-bypassed" : ""} is-${effect.kind}`} onClick={() => toggleEffect(effect.id)}>{effect.label}<AudioLines size={15} /><X size={14} onClick={(event) => { event.stopPropagation(); removeEffect(effect.id); }} /></button>)}</div><div className="m-effect-footer"><button onClick={addEffect}><Plus />เพิ่มเอฟเฟกต์</button><button><Bookmark />พรีเซ็ตของคุณ <ChevronRight /></button></div><p className="m-provider-gate">Preview/render ต้องใช้ DSP provider บน Desktop · ต้นฉบับไม่ถูกแก้ไข</p></section>
    <section className={`m-processing-panel ${tab === "voice" ? "is-focus" : ""}`}><header><h2>เสียง Agent</h2><Bot /></header><div className="m-agent-policy">
      <div style={{ marginBottom: 12 }}>
        <label style={{ fontSize: 12, color: "#999", display: "block", marginBottom: 4 }}>TTS Provider</label>
        {ttsProviders.length > 0 ? (
          <select
            value={selectedTtsId}
            onChange={(event) => setSelectedTtsId(event.target.value)}
            style={{
              width: "100%", padding: "6px 8px", borderRadius: 6,
              background: "rgba(255,255,255,0.04)", color: "#e0e0e0",
              border: "1px solid rgba(255,255,255,0.1)", fontSize: 13,
            }}
          >
            <option value="">— เลือก provider —</option>
            {ttsProviders.map((provider) => <option key={provider.id} value={provider.id}>{provider.label}</option>)}
          </select>
        ) : (
          <p style={{ fontSize: 12, color: "#999" }}>ยังไม่มี TTS provider — ตั้งค่าที่ Settings</p>
        )}
      </div>
      <button onClick={setGrant}><span>การอนุญาต MCP</span><strong>{state.agentVoiceGrant ? "เปิด · อนุญาตแล้ว" : "ปิด · default deny"}</strong><ChevronRight /></button><div><span>โปรไฟล์เสียง</span><strong>{state.voiceProfile.displayName}</strong><small>candidate · รอ provider และหลักฐานสิทธิ์ที่อนุมัติ</small></div><div className="m-agent-session"><i /><span><strong>ยังไม่มี session ที่กำลังพูด</strong><small>{state.voiceProfile.providerAvailable ? "provider พร้อม" : "ยังไม่มี voice synthesis provider"}</small></span><button disabled><Square /></button><button disabled><VolumeX /></button></div><footer><ShieldCheck /> บันทึกสิทธิ์และคำสั่งหยุดไว้ในเครื่องเท่านั้น</footer></div></section>
  </main>;
}
