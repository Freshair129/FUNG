import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  Archive,
  AudioLines,
  Cloud,
  Download,
  Home,
  Loader2,
  Minimize2,
  Moon,
  Power,
  Search,
  ShieldCheck,
  Sparkles,
  Sun,
  TimerReset,
  Volume2,
  Wifi,
} from "lucide-react";
import {
  cancelJob,
  closeWindow,
  createJob,
  diarizationStatus,
  createProject,
  getHealth,
  graphBuildStart,
  importAndTranscribe,
  listExportArtifacts,
  listJobs,
  listModelProviders,
  listProjects,
  listTranscriptSegments,
  minimizeWindow,
  nativeInvoke,
  openExternalAccountPortal,
  pickAudioOrVideoFile,
  renameSpeaker,
  startLocalApi,
  ttsSynthesizeText,
  type Health,
  type Job,
  type ModelProvider,
  type Project,
  type TranscriptLoadState,
  beginTranscriptLoad,
  settleTranscriptLoad,
} from "./tauri";
import { LiveMeetingPanel } from "./components/LiveMeetingPanel";
import { InstrumentRail } from "./components/InstrumentRail";
import { HomeScreen } from "./components/HomeScreen";
import type { SettingsTab } from "./components/SettingsPanel";
import {
  isJobActionEnabled,
  jobActionBlockedReason,
  resolveJobAction,
} from "./lib/jobActions";

const DevicePairingPanel = lazy(() =>
  import("./components/DevicePairingPanel").then((module) => ({ default: module.DevicePairingPanel })),
);
// Rendered at launch: an interrupted recording that nobody is told about is
// indistinguishable from lost audio.
const RecoveryNotice = lazy(() =>
  import("./components/RecoveryNotice").then((module) => ({ default: module.RecoveryNotice })),
);
const SettingsPanel = lazy(() =>
  import("./components/SettingsPanel").then((module) => ({ default: module.SettingsPanel })),
);

function formatMs(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

// Plain rounded rectangle: the earlier notched silhouette let the instrument
// rail and command-deck bar float half outside the panel, which read as
// elements overlapping each other. Same outer bounds (12..1268 x 12..708) so
// every absolutely-positioned zone keeps its coordinates.
const PANEL_PATH =
  "M 40,12 H 1240 A 28 28 0 0 1 1268,40 V 680 A 28 28 0 0 1 1240,708 H 40 A 28 28 0 0 1 12,680 V 40 A 28 28 0 0 1 40,12 Z";

const navItems = [
  { id: "capture", label: "Capture" },
  { id: "review", label: "Transcript" },
  { id: "summary", label: "Summary" },
  { id: "runtime", label: "Runtime" },
] as const;

const pageAnchors = [
  { id: "P1", domain: "Capture", label: "Capture" },
  { id: "P2", domain: "Review", label: "Review" },
  { id: "P3", domain: "Intelligence", label: "Intel" },
  { id: "P4", domain: "Runtime", label: "Runtime" },
] as const;

type Anchor = (typeof pageAnchors)[number]["id"];
type NavLabel = (typeof navItems)[number]["label"];
type ViewId = (typeof navItems)[number]["id"];
type Tone = "sage" | "indigo" | "metal";
type SignalId = "health" | "privacy" | "queue" | "focus";
type ThemeMode = "light" | "dark";

type LibraryItem = {
  id: string;
  title: string;
  subtitle: string;
  state: string;
};

type ActivityEntry = {
  time: string;
  title: string;
  detail: string;
  speakerId?: string | null;
  speakerName?: string | null;
};

type EventEntry = {
  type: string;
  detail: string;
  state: string;
};

type TileAction = {
  kind: "anchor" | "job" | "record" | "api";
  label: string;
  value: string;
};

type FocusTile = {
  id: string;
  eyebrow: string;
  title: string;
  detail: string;
  action: string;
  status: string;
  currentLabel: string;
  tone: Tone;
  primaryAction: TileAction;
  secondaryAction: TileAction;
};

const viewByAnchor: Record<Anchor, ViewId> = {
  P1: "capture",
  P2: "review",
  P3: "summary",
  P4: "runtime",
};

const anchorByView: Record<ViewId, Anchor> = {
  capture: "P1",
  review: "P2",
  summary: "P3",
  runtime: "P4",
};

const pageContent: Record<
  Anchor,
  {
    agent: string;
    domain: string;
    eventsTitle: string;
    focus: string;
    primary: string;
    signalTitle: string;
    tiles: readonly FocusTile[];
  }
> = {
  P1: {
    agent: "Capture console",
    domain: "Capture and Library",
    eventsTitle: "Capture events",
    focus: "Prepare and sustain a private meeting capture.",
    primary: "Meeting setup",
    signalTitle: "Capture readiness",
    tiles: [
      {
        id: "meeting-setup",
        eyebrow: "P1 / Setup",
        title: "Meeting setup",
        detail: "Name the meeting, confirm the source input, and arm a long-take preset before anyone starts talking.",
        action: "Start recording",
        status: "Ready to arm",
        currentLabel: "Next step",
        tone: "sage",
        primaryAction: { kind: "record", label: "Start recording", value: "start" },
        secondaryAction: { kind: "anchor", label: "Open transcript", value: "P2" },
      },
      {
        id: "live-capture",
        eyebrow: "P1 / Live",
        title: "Live capture",
        detail: "Watch elapsed time, chunk sealing, clipping, and silence so the meeting can run without babysitting.",
        action: "Pause or resume",
        status: "Recording watch",
        currentLabel: "Current focus",
        tone: "indigo",
        primaryAction: { kind: "record", label: "Pause or resume", value: "toggle" },
        secondaryAction: { kind: "job", label: "Add marker", value: "capture.marker" },
      },
      {
        id: "side-notes",
        eyebrow: "P1 / Notes",
        title: "Side notes",
        detail: "Drop quick flags for agenda shifts, decisions, and moments worth exporting before details disappear.",
        action: "Add marker",
        status: "Marker lane open",
        currentLabel: "Capture aid",
        tone: "metal",
        primaryAction: { kind: "job", label: "Add marker", value: "capture.marker" },
        secondaryAction: { kind: "anchor", label: "Go to transcript", value: "P2" },
      },
    ],
  },
  P2: {
    agent: "Review workspace",
    domain: "Transcript and Speakers",
    eventsTitle: "Review events",
    focus: "Turn meeting audio into readable, speaker-aware text.",
    primary: "Transcript pass",
    signalTitle: "Transcript readiness",
    tiles: [
      {
        id: "transcript-pass",
        eyebrow: "P2 / Transcript",
        title: "Transcript pass",
        detail: "Read the latest meeting transcript, keep timestamps visible, and fix obvious wording before analysis begins.",
        action: "Review transcript",
        status: "Transcript ready",
        currentLabel: "Review step",
        tone: "indigo",
        primaryAction: { kind: "job", label: "Review transcript", value: "transcript.transcribe" },
        secondaryAction: { kind: "job", label: "Rerun transcript", value: "transcript.retry" },
      },
      {
        id: "speaker-pass",
        eyebrow: "P2 / Speakers",
        title: "Speaker pass",
        detail: "Merge, rename, and lock speaker lanes only where the meeting recap actually depends on who said what.",
        action: "Lock speakers",
        status: "Speaker map in review",
        currentLabel: "Speaker task",
        tone: "sage",
        primaryAction: { kind: "job", label: "Lock speakers", value: "speakers.lock" },
        secondaryAction: { kind: "job", label: "Rerun diarization", value: "speakers.diarize" },
      },
      {
        id: "highlight-pass",
        eyebrow: "P2 / Evidence",
        title: "Highlight pass",
        detail: "Mark decisions, questions, and quotes worth carrying into summary, intent, and export bundles.",
        action: "Mark evidence",
        status: "Evidence pass open",
        currentLabel: "Highlight layer",
        tone: "metal",
        primaryAction: { kind: "job", label: "Mark evidence", value: "review.evidence" },
        secondaryAction: { kind: "anchor", label: "Open summary", value: "P3" },
      },
    ],
  },
  P3: {
    agent: "Intelligence deck",
    domain: "Summary and Intent",
    eventsTitle: "AI events",
    focus: "Produce a recap with evidence, intent, and next steps.",
    primary: "Meeting recap",
    signalTitle: "Summary confidence",
    tiles: [
      {
        id: "meeting-recap",
        eyebrow: "P3 / Recap",
        title: "Meeting recap",
        detail: "Generate a short, evidence-backed story of what happened, what changed, and what still needs attention.",
        action: "Generate recap",
        status: "Recap available",
        currentLabel: "Summary layer",
        tone: "sage",
        primaryAction: { kind: "job", label: "Generate recap", value: "summary.recap" },
        secondaryAction: { kind: "job", label: "Compare summaries", value: "summary.compare" },
      },
      {
        id: "people-intent",
        eyebrow: "P3 / Intent",
        title: "People and intent",
        detail: "Separate what each speaker likely wanted, worried about, or committed to, without presenting inference as fact.",
        action: "Analyze intent",
        status: "Intent in review",
        currentLabel: "Inference layer",
        tone: "indigo",
        primaryAction: { kind: "job", label: "Analyze intent", value: "summary.intent" },
        secondaryAction: { kind: "anchor", label: "Review speakers", value: "P2" },
      },
      {
        id: "action-register",
        eyebrow: "P3 / Actions",
        title: "Action register",
        detail: "Pull follow-ups, decisions, owners, and unresolved questions into a package people can actually use after the call.",
        action: "Extract actions",
        status: "Actions ready",
        currentLabel: "Follow-up layer",
        tone: "metal",
        primaryAction: { kind: "job", label: "Extract actions", value: "summary.actions" },
        secondaryAction: { kind: "anchor", label: "Prepare export", value: "P4" },
      },
    ],
  },
  P4: {
    agent: "Runtime control",
    domain: "Runtime and Governance",
    eventsTitle: "Runtime events",
    focus: "Finish the meeting with local policy, export, and provenance intact.",
    primary: "Export bundle",
    signalTitle: "Bundle readiness",
    tiles: [
      {
        id: "export-bundle",
        eyebrow: "P4 / Export",
        title: "Export bundle",
        detail: "Render the meeting package as WAV, MP3, transcript, recap, and structured metadata without breaking local provenance.",
        action: "Export bundle",
        status: "Bundle queued",
        currentLabel: "Delivery step",
        tone: "metal",
        primaryAction: { kind: "job", label: "Export bundle", value: "export.render" },
        secondaryAction: { kind: "job", label: "Open export queue", value: "export.queue" },
      },
      {
        id: "privacy-policy",
        eyebrow: "P4 / Policy",
        title: "Privacy and policy",
        detail: "Confirm local-only mode, provider posture, and inference labeling before the meeting leaves your machine.",
        action: "Review policy",
        status: "Policy guarded",
        currentLabel: "Governance step",
        tone: "sage",
        primaryAction: { kind: "anchor", label: "Review policy", value: "P4" },
        secondaryAction: { kind: "api", label: "Start local API", value: "start" },
      },
      {
        id: "runtime-archive",
        eyebrow: "P4 / Runtime",
        title: "Runtime and archive",
        detail: "Check provider health, storage path, and archive safety so this meeting can be reopened or audited later.",
        action: "Archive project",
        status: "Runtime stable",
        currentLabel: "System step",
        tone: "indigo",
        primaryAction: { kind: "job", label: "Archive project", value: "archive.project" },
        secondaryAction: { kind: "api", label: "Start local API", value: "start" },
      },
    ],
  },
};

const signalTileMap: Record<Anchor, Record<SignalId, string>> = {
  P1: {
    health: "meeting-setup",
    privacy: "live-capture",
    queue: "side-notes",
    focus: "meeting-setup",
  },
  P2: {
    health: "transcript-pass",
    privacy: "speaker-pass",
    queue: "highlight-pass",
    focus: "transcript-pass",
  },
  P3: {
    health: "meeting-recap",
    privacy: "people-intent",
    queue: "action-register",
    focus: "meeting-recap",
  },
  P4: {
    health: "export-bundle",
    privacy: "privacy-policy",
    queue: "runtime-archive",
    focus: "export-bundle",
  },
};

function useStageScale() {
  const computeScale = () => {
    const safeWidth = Math.max(window.innerWidth - 24, 1);
    const safeHeight = Math.max(window.innerHeight - 24, 1);
    return Math.min(safeWidth / 1304, safeHeight / 744, 1.4);
  };

  const [scale, setScale] = useState<number>(computeScale);

  useEffect(() => {
    const onResize = () => setScale(computeScale());
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return scale;
}

function Segmented<T extends string>({
  compact = false,
  items,
  onChange,
  value,
}: {
  compact?: boolean;
  items: readonly T[];
  onChange: (item: T) => void;
  value?: T;
}) {
  return (
    <div className={`segmented ${compact ? "segmented--compact" : ""}`} role="tablist">
      {items.map((item) => (
        <button
          key={item}
          type="button"
          className={`segmented__item ${item === value ? "is-active" : ""}`}
          onClick={() => onChange(item)}
        >
          {item}
        </button>
      ))}
    </div>
  );
}

function SpeakerLabel({
  speakerId,
  speakerName,
  onRename,
}: {
  speakerId: string;
  speakerName: string;
  onRename: (speakerId: string, displayName: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(speakerName);

  useEffect(() => {
    setValue(speakerName);
  }, [speakerName]);

  if (editing) {
    return (
      <input
        className="log-item__speaker-input"
        autoFocus
        value={value}
        aria-label="แก้ไขชื่อผู้พูด"
        onChange={(event) => setValue(event.target.value)}
        onBlur={() => {
          setEditing(false);
          const trimmed = value.trim();
          if (trimmed && trimmed !== speakerName) onRename(speakerId, trimmed);
          else setValue(speakerName);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
          if (event.key === "Escape") {
            setValue(speakerName);
            setEditing(false);
          }
        }}
      />
    );
  }

  return (
    <button
      type="button"
      className="log-item__speaker"
      title="แก้ไขชื่อผู้พูด"
      onClick={(event) => {
        event.stopPropagation();
        setEditing(true);
      }}
    >
      {speakerName}
    </button>
  );
}

export function App() {
  const scale = useStageScale();
  const [health, setHealth] = useState<Health | null>(null);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [providers, setProviders] = useState<ModelProvider[]>([]);
  const transcriptRequestId = useRef(0);
  const [transcriptRefreshToken, setTranscriptRefreshToken] = useState(0);
  const [transcriptLoad, setTranscriptLoad] = useState<TranscriptLoadState>(() =>
    beginTranscriptLoad(null, 0),
  );
  const [transcribing, setTranscribing] = useState(false);
  /// Why the last action could not run. Shown instead of the silent
  /// no-op the inert job buttons used to produce.
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [activeAnchor, setActiveAnchor] = useState<Anchor>("P2");
  const [activeView, setActiveView] = useState<ViewId>("review");
  const [theme, setTheme] = useState<ThemeMode>("light");
  const [powerMenuOpen, setPowerMenuOpen] = useState(false);
  const [liveMeetingOpen, setLiveMeetingOpen] = useState(false);
  const [devicePairingPanelOpen, setDevicePairingPanelOpen] = useState(false);
  const [settingsPanelOpen, setSettingsPanelOpen] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>("account");
  const [showHome, setShowHome] = useState(true);
  const [recording, setRecording] = useState(false);
  const [ttsPlaying, setTtsPlaying] = useState(false);
  const [ttsLoading, setTtsLoading] = useState(false);
  const [ttsAudio, setTtsAudio] = useState<HTMLAudioElement | null>(null);
  const [signals, setSignals] = useState<Record<SignalId, boolean>>({
    focus: true,
    health: true,
    privacy: true,
    queue: false,
  });
  const [activeTileByAnchor, setActiveTileByAnchor] = useState<Record<Anchor, string>>({
    P1: "meeting-setup",
    P2: "transcript-pass",
    P3: "meeting-recap",
    P4: "export-bundle",
  });

  const refresh = async () => {
    const [nextHealth, nextProjects, nextJobs, nextProviders] = await Promise.all([
      getHealth(),
      listProjects(),
      listJobs(),
      listModelProviders(),
    ]);

    setHealth(nextHealth);
    setProjects(nextProjects);
    setJobs(nextJobs);
    setProviders(nextProviders);
  };

  useEffect(() => {
    void refresh();
  }, []);

  // State reflects what the backend actually reports for each project, not
  // its position in the list: an active recording, then a running/queued
  // job, then a plain "Saved" for a project with neither.
  const libraryItems = useMemo<LibraryItem[]>(() => {
    return projects.slice(0, 5).map((project) => {
      const hasActiveRecording = project.activeRecordingId !== null;
      const hasPendingJob = jobs.some(
        (job) => job.projectId === project.id && (job.status === "running" || job.status === "queued"),
      );
      const state = hasActiveRecording ? "Live" : hasPendingJob ? "Queued" : "Saved";
      return {
        id: project.id,
        title: project.name,
        subtitle: `Updated ${new Date(project.updatedAt).toLocaleDateString()}`,
        state,
      };
    });
  }, [projects, jobs]);

  const [selectedRecording, setSelectedRecording] = useState<string>("");

  useEffect(() => {
    if (!libraryItems.some((item) => item.id === selectedRecording)) {
      setSelectedRecording(libraryItems[0]?.id ?? "");
    }
  }, [libraryItems, selectedRecording]);

  const selectedCard = useMemo(
    () => libraryItems.find((item) => item.id === selectedRecording) ?? libraryItems[0],
    [libraryItems, selectedRecording],
  );

  const selectedProjectId = useMemo(
    () => (projects.some((project) => project.id === selectedRecording) ? selectedRecording : undefined),
    [projects, selectedRecording],
  );

  const activeRecordingId = useMemo(
    () =>
      projects.find((project) => project.id === selectedProjectId)
        ?.activeRecordingId ?? null,
    [projects, selectedProjectId],
  );

  const transcriptView =
    transcriptLoad.status === "ready" && transcriptLoad.recordingId === activeRecordingId
      ? transcriptLoad.view
      : null;
  const segments = transcriptView?.segments ?? [];
  const transcriptCapped = transcriptView?.capped ? transcriptView : null;

  useEffect(() => {
    const requestId = transcriptRequestId.current + 1;
    transcriptRequestId.current = requestId;
    setTranscriptLoad(beginTranscriptLoad(activeRecordingId, requestId));
    if (!selectedProjectId || !activeRecordingId) return;

    const request = { requestId, recordingId: activeRecordingId };
    void listTranscriptSegments(selectedProjectId, activeRecordingId)
      .then((view) => {
        setTranscriptLoad((state) =>
          settleTranscriptLoad(state, {
            ...request,
            outcome: { status: "fulfilled", view },
          }),
        );
      })
      .catch(() => {
        setTranscriptLoad((state) =>
          settleTranscriptLoad(state, {
            ...request,
            outcome: { status: "rejected" },
          }),
        );
      });
  }, [activeRecordingId, jobs, selectedProjectId, transcriptRefreshToken]);

  const currentPage = pageContent[activeAnchor];
  const currentTile =
    currentPage.tiles.find((tile) => tile.id === activeTileByAnchor[activeAnchor]) ?? currentPage.tiles[0];
  const meetingTitle = selectedCard?.title ?? "ยังไม่ได้เลือกการบันทึก";
  const meetingSubtitle = selectedCard?.subtitle ?? "เลือกการบันทึกจากหน้าแรกเพื่อเริ่มรีวิว";

  const providerSummary = useMemo(() => {
    if (providers.length === 0) return "BYOM not configured";

    const localProviders = providers.filter(
      (provider) => provider.runtimeLocation !== "cloud" && provider.enabled,
    ).length;

    return `${localProviders}/${providers.length} local providers`;
  }, [providers]);

  useEffect(() => {
    if (!currentPage.tiles.some((tile) => tile.id === activeTileByAnchor[activeAnchor])) {
      setActiveTileByAnchor((current) => ({ ...current, [activeAnchor]: currentPage.tiles[0]?.id ?? "" }));
    }
  }, [activeAnchor, activeTileByAnchor, currentPage.tiles]);

  const speakerCount = useMemo(
    () => new Set(segments.map((segment) => segment.speakerId).filter((id) => id != null)).size,
    [segments],
  );

  const runtimeStats = useMemo(() => {
    const apiState = health?.localApi.running ? "API live" : "Offline";
    const providerCount = providers.filter((provider) => provider.enabled).length;

    switch (activeAnchor) {
      case "P1":
        return [
          { label: "Session", value: recording ? "Live" : "Armed", meta: selectedCard?.state ?? "—" },
          { label: "Capture", value: recording ? "กำลังอัด" : "หยุดอยู่", meta: "Chunked" },
          { label: "Queue", value: jobs.length ? `${jobs.length} งาน` : "ว่าง", meta: "Jobs" },
          { label: "Next", value: "Transcript", meta: "After stop" },
        ];
      case "P2":
        return [
          {
            label: "Transcript",
            value: segments.length ? `${segments.length} ท่อน` : "ยังไม่มี",
            meta: segments.length ? (transcriptCapped ? "ไม่ครบ" : "โหลดแล้ว") : "รอถอดเสียง",
          },
          { label: "Speakers", value: speakerCount ? `${speakerCount} คน` : "ยังไม่ระบุ", meta: "Editable" },
          { label: "Evidence", value: "ยังไม่มี", meta: "ยังไม่รองรับ" },
          { label: "Next", value: "Summary", meta: "After review" },
        ];
      case "P3":
        return [
          { label: "Recap", value: "ยังไม่มี", meta: "รอสร้าง" },
          { label: "Intent", value: "ยังไม่มี", meta: "รอสร้าง" },
          { label: "Actions", value: "ยังไม่มี", meta: "ยังไม่รองรับ" },
          { label: "Export", value: jobs.length ? `${jobs.length} queued` : "Ready", meta: "Bundle" },
        ];
      case "P4":
        return [
          { label: "Providers", value: providerCount ? `${providerCount} ready` : "Setup", meta: "BYOM" },
          { label: "Privacy", value: "Local", meta: "No cloud" },
          { label: "Queue", value: jobs.length ? `${jobs.length} pending` : "Clear", meta: "Export" },
          { label: "Runtime", value: apiState, meta: health?.sqliteWal ? "WAL" : "Idle" },
        ];
    }
  }, [activeAnchor, health, jobs.length, providers, recording, segments.length, selectedCard, speakerCount, transcriptCapped]);

  const activityFeed = useMemo<ActivityEntry[]>(() => {
    if (activeAnchor === "P2" && currentTile.id === "transcript-pass" && segments.length > 0) {
      const lines = segments.slice(0, 16).map((segment) => ({
        time: formatMs(segment.startMs),
        title: segment.text.length > 60 ? `${segment.text.slice(0, 60)}…` : segment.text,
        detail: segment.confidence != null ? `Confidence ${(segment.confidence * 100).toFixed(0)}%` : "faster-whisper",
        speakerId: segment.speakerId,
        speakerName: segment.speakerName,
      }));
      // First, not last: a transcript that stops mid-meeting reads as a
      // meeting that ended there, and the reader has to know before they
      // start rather than after they have drawn a conclusion from it.
      if (transcriptCapped) {
        lines.unshift({
          time: "!",
          title: `transcript ไม่ครบ — อ่านได้สูงสุด ${transcriptCapped.cap} ท่อนต่อการบันทึก`,
          detail: `ยังมีท่อนที่ยังไม่ได้อ่านใน ${transcriptCapped.cappedRecordingIds.length} การบันทึก — เป็นเพดานของ storage engine ไม่ใช่จุดจบของการประชุม`,
          speakerId: null,
          speakerName: null,
        });
      }
      return lines;
    }

    // No per-tile activity source exists beyond the job queue and the
    // transcript above, so every remaining row here has to come from real
    // jobs — never invented copy standing in for activity that never
    // happened.
    if (jobs.length > 0) {
      return jobs.slice(0, 8).map((job) => ({
        time: new Date(job.updatedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        title: `${job.type} ${job.status}`,
        detail: job.errorMessage ?? `Progress ${job.progress}% for ${job.projectId}`,
      }));
    }
    return [
      {
        time: "",
        title: "ยังไม่มีกิจกรรม",
        detail: "เริ่มบันทึกหรือเลือกไฟล์เสียงเพื่อสร้างงานแรก",
      },
    ];
  }, [activeAnchor, currentTile.id, jobs, segments, transcriptCapped]);

  const activeTranscribeJob = jobs.find(
    (job) => job.type === "transcript.transcribe" && job.status === "running",
  );

  const primaryActionLabel =
    transcribing && currentTile.primaryAction.value === "transcript.transcribe"
      ? `Transcribing… ${activeTranscribeJob?.progress ?? 0}%`
      : currentTile.primaryAction.label;

  const eventFeed = useMemo<EventEntry[]>(() => {
    if (activeAnchor === "P4" && providers.length > 0) {
      return providers.slice(0, 4).map((provider) => ({
        type: provider.kind,
        detail: provider.label,
        state: provider.enabled ? provider.runtimeLocation : "Disabled",
      }));
    }
    // Same rule as the activity feed: no per-tile event source exists other
    // than the job queue, so distinct job types stand in for "events" and an
    // honest empty state replaces invented event rows.
    if (jobs.length > 0) {
      const latestByType = new Map<string, Job>();
      for (const job of jobs) {
        if (!latestByType.has(job.type)) latestByType.set(job.type, job);
      }
      return Array.from(latestByType.values())
        .slice(0, 4)
        .map((job) => ({
          type: job.type,
          detail: job.errorMessage ?? `Progress ${job.progress}%`,
          state: job.status,
        }));
    }
    return [{ type: "งาน", detail: "ยังไม่มีเหตุการณ์", state: "รอเริ่ม" }];
  }, [activeAnchor, jobs, providers]);

  const signalCards = useMemo(
    () => {
      const byPage: Record<Anchor, Array<{ id: SignalId; title: string; value: string; foot: string }>> = {
        P1: [
          { id: "health", title: "Capture safety", value: recording ? "Guarded" : "Armed", foot: health?.sqliteWal ? "SQLite WAL พร้อมใช้งาน" : "รอตรวจสถานะ storage" },
          { id: "privacy", title: "Audio level", value: "ไม่มีข้อมูล", foot: "บิลด์นี้ยังไม่วัดระดับเสียงขณะอัด" },
          { id: "queue", title: "Session readiness", value: "Transcript next", foot: "Stop capture to begin transcript review." },
          { id: "focus", title: currentPage.signalTitle, value: currentTile.title, foot: meetingSubtitle },
        ],
        P2: [
          {
            id: "health",
            title: "Transcript",
            value: segments.length ? (transcriptCapped ? "ไม่ครบ" : `${segments.length} ท่อน`) : "ยังไม่มี",
            foot: transcriptCapped
              ? "อ่านได้ไม่ครบเพราะเพดานของ storage engine"
              : segments.length
                ? "โหลดจากการบันทึกจริงแล้ว"
                : "ถอดเสียงก่อนเพื่อเริ่มรีวิว",
          },
          { id: "privacy", title: "Speakers", value: speakerCount ? `${speakerCount} คน` : "ยังไม่ระบุ", foot: "ป้ายชื่อแก้ไขได้และไม่ผูกกับข้อมูลชีวมิติ" },
          { id: "queue", title: "Evidence", value: "ยังไม่มี", foot: "ยังไม่รองรับการปักหลักฐานในบิลด์นี้" },
          { id: "focus", title: currentPage.signalTitle, value: currentTile.title, foot: meetingSubtitle },
        ],
        P3: [
          { id: "health", title: "Recap", value: "ยังไม่มี", foot: "ยังไม่มีการสร้าง recap สำหรับการบันทึกนี้" },
          { id: "privacy", title: "Intent", value: "ยังไม่มี", foot: "ยังไม่มีการอนุมานเจตนา" },
          { id: "queue", title: "Actions", value: "ยังไม่มี", foot: "ยังไม่รองรับการสกัด action item" },
          { id: "focus", title: currentPage.signalTitle, value: currentTile.title, foot: meetingSubtitle },
        ],
        P4: [
          { id: "health", title: "Export queue", value: jobs.length ? `${jobs.length} pending` : "Clear", foot: "Bundle render remains local-only." },
          { id: "privacy", title: "Privacy mode", value: health?.localApi.running ? "On-device" : "Offline", foot: health?.localApi.bind ?? "No remote calls" },
          { id: "queue", title: "Runtime health", value: providers.length ? "Stable" : "Setup", foot: providers.length ? providerSummary : health?.version ? `v${health.version}` : "Local provider check needed." },
          { id: "focus", title: currentPage.signalTitle, value: currentTile.title, foot: meetingSubtitle },
        ],
      };

      return byPage[activeAnchor].map((signal) => ({
        ...signal,
        icon:
          signal.id === "health" ? <Activity size={15} /> :
          signal.id === "privacy" ? <ShieldCheck size={15} /> :
          signal.id === "queue" ? <Archive size={15} /> :
          <TimerReset size={15} />,
      }));
    },
    [activeAnchor, currentPage.signalTitle, currentTile.title, health, jobs.length, meetingSubtitle, providerSummary, providers.length, recording, segments.length, speakerCount, transcriptCapped],
  );

  const viewLabel = navItems.find((item) => item.id === activeView)?.label ?? "Transcript";

  const onViewChange = (label: NavLabel) => {
    const nextView = navItems.find((item) => item.label === label)?.id ?? "review";
    enterMeetingWorkspace(anchorByView[nextView]);
  };

  const activateAnchor = (anchor: Anchor) => {
    setActiveAnchor(anchor);
    setActiveView(viewByAnchor[anchor]);
  };

  const enterMeetingWorkspace = (anchor: Anchor) => {
    setShowHome(false);
    activateAnchor(anchor);
  };

  const returnToHome = () => setShowHome(true);

  const activateTile = (tileId: string) => {
    setActiveTileByAnchor((current) => ({ ...current, [activeAnchor]: tileId }));
  };

  const handleMinimizeWindow = async () => {
    setPowerMenuOpen(false);
    await minimizeWindow();
  };

  const handleCloseWindow = async () => {
    setPowerMenuOpen(false);
    await closeWindow();
  };

  const handleNewProject = async () => {
    const name = `Session ${projects.length + 1}`;
    const project = await createProject(name);
    setSelectedRecording(project.id);
    await refresh();
    return project;
  };

  // Returns the job as it finished, so a caller can tell "done" from "failed"
  // rather than refreshing and hoping. `null` means it stopped being visible
  // or outlasted the poll.
  const pollJobUntilDone = async (jobId: string): Promise<Job | null> => {
    for (let attempt = 0; attempt < 600; attempt += 1) {
      const nextJobs = await listJobs();
      setJobs(nextJobs);
      const job = nextJobs.find((entry) => entry.id === jobId);
      if (!job) return null;
      if (job.status === "completed" || job.status === "failed") return job;
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    return null;
  };

  const handleRenameSpeaker = async (speakerId: string, displayName: string) => {
    await renameSpeaker(speakerId, displayName);
    setTranscriptRefreshToken((current) => current + 1);
  };

  const handleTtsPlay = async (text: string) => {
    // If already playing, stop
    if (ttsAudio) {
      ttsAudio.pause();
      setTtsAudio(null);
      setTtsPlaying(false);
      return;
    }

    // Block TTS during active recording to prevent feedback
    if (recording) {
      return;
    }

    setTtsLoading(true);
    try {
      const result = await ttsSynthesizeText(text);
      const audio = new Audio(`asset://localhost/${result.audioPath}`);
      audio.onended = () => {
        setTtsPlaying(false);
        setTtsAudio(null);
      };
      audio.onerror = () => {
        setTtsPlaying(false);
        setTtsAudio(null);
      };
      await audio.play();
      setTtsAudio(audio);
      setTtsPlaying(true);
    } catch (e: unknown) {
      // If no provider registered, deep-link straight to the TTS tab of
      // Settings rather than leaving the user on whatever tab last opened.
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("ยังไม่ได้ลงทะเบียน")) {
        setSettingsInitialTab("tts");
        setSettingsPanelOpen(true);
      }
    } finally {
      setTtsLoading(false);
    }
  };

  const failedGraphBuilds = useMemo(
    () => jobs.filter((job) => job.type === "graph.build" && job.status === "failed"),
    [jobs],
  );

  const handleRetryGraphBuild = async (job: Job) => {
    const recordingId = job.inputRefs[0];
    if (!recordingId) return;
    await graphBuildStart(job.projectId, recordingId);
    await refresh();
  };

  const handleImportAndTranscribe = async () => {
    const filePath = await pickAudioOrVideoFile();
    if (!filePath) return;

    let projectId = selectedProjectId;
    if (!projectId) {
      const project = await handleNewProject();
      projectId = project.id;
    }

    activateAnchor("P2");
    activateTile("transcript-pass");
    setTranscribing(true);
    try {
      const job = await importAndTranscribe(filePath, projectId);
      setJobs((current) => [job, ...current.filter((entry) => entry.id !== job.id)]);
      const finished = await pollJobUntilDone(job.id);
      if (finished?.status === "failed") {
        setActionNotice("นำเข้าและถอดเสียงไม่สำเร็จ — ตรวจสอบไฟล์และ local model แล้วลองใหม่");
      }
      await refresh();
    } catch {
      setActionNotice("นำเข้าและถอดเสียงไม่สำเร็จ — ตรวจสอบไฟล์และ local model แล้วลองใหม่");
    } finally {
      setTranscribing(false);
    }
  };

  const transcriptBlockedReason = (jobType: string) => {
    if (jobType !== "summary.generate" && jobType !== "export.render") return null;
    if (transcriptLoad.status === "rejected") {
      return "อ่าน transcript ไม่สำเร็จ — ตรวจสอบไฟล์และพื้นที่จัดเก็บในเครื่อง แล้วลองใหม่";
    }
    if (!transcriptView) return "กำลังอ่าน transcript ของการบันทึกนี้…";
    if (transcriptView.capped) {
      return `ยังสร้างผลลัพธ์ไม่ได้ — transcript ชนเพดาน ${transcriptView.cap} ท่อน จึงไม่ใช่ผลลัพธ์ที่ครบ`;
    }
    if (transcriptView.segments.length === 0) {
      return "ยังสร้างผลลัพธ์ไม่ได้ — ยังไม่มี transcript ที่อ่านได้ ให้ตรวจสอบ local model แล้วลองถอดเสียงใหม่";
    }
    return null;
  };

  const handleCreateJob = async (jobType: string) => {
    setActionNotice(null);
    const plan = resolveJobAction(jobType);

    if (plan.kind === "unavailable") {
      // Previously this wrote a job row nothing would ever run. Saying so is
      // the whole improvement.
      setActionNotice(plan.reason);
      return;
    }
    if (plan.kind === "import") {
      await handleImportAndTranscribe();
      return;
    }
    if (!selectedProjectId || !activeRecordingId) {
      setActionNotice("ต้องมีการบันทึกในโปรเจกต์นี้ก่อน");
      return;
    }
    const transcriptBlocker = transcriptBlockedReason(plan.jobType ?? "");
    if (transcriptBlocker) {
      setActionNotice(transcriptBlocker);
      return;
    }
    if (plan.jobType === "speakers.diarize") {
      // The dependencies are opt-in and the model is gated, so this job can
      // be unrunnable for reasons the user can fix. Saying so at the click
      // beats queueing work that fails on the worker minutes later.
      const readiness = await diarizationStatus();
      if (readiness && !readiness.available) {
        setActionNotice(readiness.detail ?? "ยังแยกเสียงผู้พูดไม่ได้");
        return;
      }
    }

    let queued;
    try {
      queued = await createJob(plan.jobType, selectedProjectId, activeRecordingId);
    } catch (error) {
      setActionNotice(`เข้าคิวไม่สำเร็จ: ${String(error)}`);
      return;
    }

    // An export whose files the user cannot find is not an export. The other
    // job kinds change what is already on screen; this one writes to disk and
    // has to say where.
    if (plan.jobType === "export.render") {
      setActionNotice("กำลังส่งออกซับไตเติล…");
      const finished = await pollJobUntilDone(queued.id);
      if (finished?.status === "failed") {
        setActionNotice(finished.errorMessage ?? "ส่งออกซับไตเติลไม่สำเร็จ");
      } else if (finished?.status === "completed") {
        const written = (await listExportArtifacts(selectedProjectId)).filter(
          (artifact) => artifact.kind === "srt" || artifact.kind === "vtt",
        );
        setActionNotice(
          written.length > 0
            ? `ส่งออกแล้ว: ${written
                .slice(0, 2)
                .map((artifact) => artifact.filePath)
                .join(" · ")}`
            : "ส่งออกเสร็จแล้ว แต่ไม่พบไฟล์ที่บันทึกไว้",
        );
      }
    }
    await refresh();
  };

  const handleCancelJob = async (jobId: string) => {
    const outcome = await cancelJob(jobId);
    setActionNotice(
      outcome === "cancelled"
        ? "ยกเลิกงานที่รออยู่แล้ว"
        : outcome === "requestedWhileRunning"
          ? "งานกำลังทำงานอยู่ — จะหยุดเมื่อขั้นตอนปัจจุบันจบ"
          : "ไม่พบงานที่รออยู่",
    );
    await refresh();
  };

  const handleStartApi = async () => {
    await startLocalApi();
    await refresh();
  };

  /// Only `job` actions can be unavailable; anchors, capture, and the local
  /// API are always live, so they are enabled unconditionally rather than
  /// routed through the job vocabulary.
  const tileActionEnabled = (action: TileAction) => {
    if (action.kind !== "job") return true;
    const plan = resolveJobAction(action.value);
    const planJobType = plan.kind === "queue" ? plan.jobType : "";
    return (
      isJobActionEnabled(action.value, Boolean(activeRecordingId)) &&
      !transcriptBlockedReason(planJobType)
    );
  };

  const tileActionTitle = (action: TileAction) => {
    if (action.kind !== "job") return action.label;
    const plan = resolveJobAction(action.value);
    const planJobType = plan.kind === "queue" ? plan.jobType : "";
    return (
      jobActionBlockedReason(action.value, Boolean(activeRecordingId)) ??
      transcriptBlockedReason(planJobType) ??
      action.label
    );
  };

  const performTileAction = async (action: TileAction) => {
    if (action.kind === "anchor") {
      activateAnchor(action.value as Anchor);
      return;
    }

    if (action.kind === "api") {
      await handleStartApi();
      return;
    }

    if (action.kind === "record") {
      // Real capture lives in the Live Meeting panel now — the old
      // setRecording() toggle was UI state with no backend.
      setLiveMeetingOpen(true);
      activateTile("live-capture");
      return;
    }

    await handleCreateJob(action.value);
  };

  const toggleSignal = async (id: SignalId) => {
    setSignals((current) => ({ ...current, [id]: !current[id] }));
    const nextTile = signalTileMap[activeAnchor][id];
    if (nextTile) {
      activateTile(nextTile);
    }
  };

  return (
    <div className={`app-shell theme-${theme}`}>
      <Suspense fallback={null}>
        <RecoveryNotice invoke={nativeInvoke} />
      </Suspense>
      {settingsPanelOpen && (
        <Suspense fallback={null}>
          <SettingsPanel
            onClose={() => setSettingsPanelOpen(false)}
            invoke={nativeInvoke}
            projectId={selectedProjectId ?? null}
            onStartApi={() => void handleStartApi()}
            apiRunning={Boolean(health?.localApi.running)}
            onFetchStarted={(job) => {
              setJobs((current) => [job, ...current.filter((entry) => entry.id !== job.id)]);
              void pollJobUntilDone(job.id).then(() => void refresh());
            }}
            onOpenExternalAccountPortal={openExternalAccountPortal}
            initialTab={settingsInitialTab}
          />
        </Suspense>
      )}
      {liveMeetingOpen && (
        <LiveMeetingPanel onClose={() => setLiveMeetingOpen(false)} projectId={selectedProjectId ?? null} />
      )}
      {devicePairingPanelOpen && (
        <div
          className="account-login-overlay"
          role="presentation"
          onClick={() => setDevicePairingPanelOpen(false)}
        >
          <div className="account-login-stack" onClick={(event) => event.stopPropagation()}>
            <Suspense
              fallback={(
                <section className="account-login-panel" aria-label="กำลังเปิดการจับคู่อุปกรณ์">
                  <p className="account-login-status">กำลังเปิดการจับคู่อุปกรณ์…</p>
                </section>
              )}
            >
              <DevicePairingPanel onClose={() => setDevicePairingPanelOpen(false)} />
            </Suspense>
          </div>
        </div>
      )}
      <div className="ambient-grid" data-tauri-drag-region aria-hidden="true" />

      <svg className="clip-defs" width="0" height="0" aria-hidden="true" focusable="false">
        <defs>
          <path id="subtractPanelPath" d={PANEL_PATH} />
          <clipPath id="panelClip" clipPathUnits="userSpaceOnUse">
            <use href="#subtractPanelPath" />
          </clipPath>
        </defs>
      </svg>

      <div className="stage-wrap" style={{ transform: `scale(${scale})` }}>
        <main className="stage" aria-label="FUNG review workspace">
          <div className="panel-glow" data-tauri-drag-region aria-hidden="true" />
          <div className="panel-glass" data-tauri-drag-region>
            {showHome ? (
              <HomeScreen
                items={libraryItems}
                onStartRecording={() => {
                  enterMeetingWorkspace("P1");
                  setActiveTileByAnchor((current) => ({ ...current, P1: "live-capture" }));
                  setLiveMeetingOpen(true);
                }}
                onImport={() => {
                  enterMeetingWorkspace("P1");
                  void handleImportAndTranscribe();
                }}
                onOpenItem={(id) => {
                  setSelectedRecording(id);
                  enterMeetingWorkspace("P2");
                }}
              />
            ) : (
              <>
            <section className="zone score-header" data-tauri-drag-region aria-label="Score header">
              <div>
                <div className="eyebrow">Meeting Mode / {activeAnchor}</div>
                <div className="score-title">{meetingTitle}</div>
              </div>
              <div className="score-meta">
                <span className="badge badge--sage">
                  <ShieldCheck size={14} />
                  {currentPage.domain}
                </span>
                <span className="badge">
                  <AudioLines size={14} />
                  {currentTile.status}
                </span>
              </div>
            </section>

            <section className="zone stats-bar no-drag" aria-label="Stats">
              {runtimeStats.map((item) => (
                <button key={item.label} type="button" className="stat-pill">
                  <span className="stat-pill__label">{item.label}</span>
                  <strong>{item.value}</strong>
                  <span className="stat-pill__meta">{item.meta}</span>
                </button>
              ))}
            </section>

            <section className="battle-grid no-drag" aria-label="Battle zone">
              <div className="zone focus-workbench">
                <div className="focus-workbench__head">
                  <div>
                    <div className="eyebrow">{currentPage.domain}</div>
                    <div className="zone-title zone-title--large">{currentPage.focus}</div>
                  </div>
                  <span className="badge">{meetingSubtitle}</span>
                </div>
                <div className="focus-tile-grid">
                  {currentPage.tiles.map((tile) => (
                    <button
                      key={tile.id}
                      type="button"
                      className={`focus-tile focus-tile--${tile.tone} ${tile.id === currentTile.id ? "is-active" : ""}`}
                      onClick={() => activateTile(tile.id)}
                    >
                      <span>{tile.eyebrow}</span>
                      <strong>{tile.title}</strong>
                      <p>{tile.detail}</p>
                      <em>{tile.action}</em>
                    </button>
                  ))}
                </div>
                <div className="focus-detail-dock">
                  <div className="focus-detail-dock__copy">
                    <span>{currentTile.currentLabel}</span>
                    <strong>{currentTile.title}</strong>
                    <p>{currentTile.detail}</p>
                  </div>
                  {actionNotice ? (
                    <p className="action-notice" role="status">
                      {actionNotice}
                    </p>
                  ) : null}
                  <div className="focus-detail-dock__actions">
                    <button
                      type="button"
                      className="quick-action quick-action--primary"
                      disabled={transcribing || !tileActionEnabled(currentTile.primaryAction)}
                      title={tileActionTitle(currentTile.primaryAction)}
                      onClick={() => void performTileAction(currentTile.primaryAction)}
                    >
                      {primaryActionLabel}
                    </button>
                    <button
                      type="button"
                      className="quick-action"
                      disabled={!tileActionEnabled(currentTile.secondaryAction)}
                      title={tileActionTitle(currentTile.secondaryAction)}
                      onClick={() => void performTileAction(currentTile.secondaryAction)}
                    >
                      {currentTile.secondaryAction.label}
                    </button>
                  </div>
                </div>
              </div>
            </section>

            <section className="zone agent-card no-drag" aria-label="Agent card">
              <div className="agent-card__head">
                <div>
                  <div className="eyebrow">{activeAnchor} / {currentPage.domain}</div>
                  <div className="zone-title zone-title--large">{currentPage.agent}</div>
                </div>
                <span className="badge badge--metal">
                  <Sparkles size={14} />
                  {currentTile.status}
                </span>
              </div>

              <div className="agent-card__stack">
                <div className="agent-current">
                  <span>{currentTile.currentLabel}</span>
                  <strong>
                    {currentTile.title}
                    {activeAnchor === "P3" && currentTile.id === "meeting-recap" && (
                      <button
                        type="button"
                        className="tts-speak-btn"
                        title={ttsPlaying ? "หยุดฟัง" : "ฟังสรุป"}
                        aria-label={ttsPlaying ? "หยุดฟังสรุป" : "ฟังสรุป"}
                        onClick={() => void handleTtsPlay(currentTile.detail)}
                        disabled={ttsLoading}
                        style={{ marginLeft: 8 }}
                      >
                        {ttsLoading ? (
                          <Loader2 size={14} className="spin" />
                        ) : ttsPlaying ? (
                          <span>⏸</span>
                        ) : (
                          <Volume2 size={14} />
                        )}
                      </button>
                    )}
                  </strong>
                  <p>{currentTile.detail}</p>
                </div>

                {actionNotice ? (
                  <p className="action-notice" role="status">
                    {actionNotice}
                  </p>
                ) : null}
                <div className="quick-actions">
                  <button
                    type="button"
                    className="quick-action quick-action--primary"
                    disabled={transcribing || !tileActionEnabled(currentTile.primaryAction)}
                    title={tileActionTitle(currentTile.primaryAction)}
                    onClick={() => void performTileAction(currentTile.primaryAction)}
                  >
                    {primaryActionLabel}
                  </button>
                  <button
                    type="button"
                    className="quick-action"
                    disabled={!tileActionEnabled(currentTile.secondaryAction)}
                    title={tileActionTitle(currentTile.secondaryAction)}
                    onClick={() => void performTileAction(currentTile.secondaryAction)}
                  >
                    {currentTile.secondaryAction.label}
                  </button>
                </div>

                <div className="agent-footer">
                  <span>
                    <Cloud size={14} />
                    {health?.databasePath ?? "browser-preview"}
                  </span>
                  <span>
                    <Wifi size={14} />
                    {health?.localApi.bind ?? "Background offline"}
                  </span>
                </div>
              </div>
            </section>

            <section className="zone sector-log no-drag" aria-label="Sector C log">
              <div className="log-column">
                <div className="zone-title">Activity</div>
                <div className="log-list">
                  {activityFeed.map((entry) => (
                    <article key={entry.time + entry.title} className="log-item">
                      <span className="log-item__time">{entry.time}</span>
                      <div>
                        {entry.speakerName && entry.speakerId ? (
                          <SpeakerLabel
                            speakerId={entry.speakerId}
                            speakerName={entry.speakerName}
                            onRename={(speakerId, displayName) => void handleRenameSpeaker(speakerId, displayName)}
                          />
                        ) : null}
                        <strong>{entry.title}</strong>
                        <p>{entry.detail}</p>
                      </div>
                    </article>
                  ))}
                  {failedGraphBuilds.map((job) => (
                    <article key={job.id} className="log-item log-item--retry">
                      <span className="log-item__time">
                        {new Date(job.updatedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                      </span>
                      <div>
                        <strong>สร้างกราฟความรู้ไม่สำเร็จ</strong>
                        <p>{job.errorMessage ?? "ลองสร้างกราฟใหม่อีกครั้ง"}</p>
                        <button
                          type="button"
                          className="quick-action log-item__retry"
                          onClick={() => void handleRetryGraphBuild(job)}
                        >
                          ลองใหม่
                        </button>
                      </div>
                    </article>
                  ))}
                </div>
              </div>

              <div className="log-column">
                <div className="zone-title">{currentPage.eventsTitle}</div>
                <div className="event-list">
                  {eventFeed.map((entry) => (
                    <article key={entry.type + entry.detail} className="event-item">
                      <div>
                        <span>{entry.type}</span>
                        <strong>{entry.detail}</strong>
                      </div>
                      <em>{entry.state}</em>
                    </article>
                  ))}
                </div>
              </div>
            </section>

            <section className="zone signals-sector no-drag" aria-label="Signals">
              {signalCards.map((signal) => (
                <button
                  key={signal.id}
                  type="button"
                  className={`signal-card ${signals[signal.id] ? "is-active" : ""}`}
                  onClick={() => void toggleSignal(signal.id)}
                >
                  <div className="signal-card__head">
                    <span>{signal.title}</span>
                    {signal.icon}
                  </div>
                  <strong>{signal.value}</strong>
                  <p>{signal.foot}</p>
                </button>
              ))}
            </section>
              </>
            )}
          </div>

          <svg className="panel-rim" viewBox="0 0 1280 720" aria-hidden="true">
            <defs>
              <filter id="rimShadow" x="-10%" y="-10%" width="120%" height="120%">
                <feDropShadow
                  dx="0"
                  dy="24"
                  stdDeviation="24"
                  floodColor="#181a1f"
                  floodOpacity="0.15"
                />
              </filter>
            </defs>
            <use href="#subtractPanelPath" className="panel-rim__shadow" filter="url(#rimShadow)" />
            <use href="#subtractPanelPath" className="panel-rim__stroke panel-rim__stroke--outer" />
            <use href="#subtractPanelPath" className="panel-rim__stroke panel-rim__stroke--inner" />
          </svg>

          <div className="fab fab-topbar" data-tauri-drag-region>
            <div className="topbar-title">
              <button type="button" className="icon-button no-drag" aria-label="Search">
                <Search size={16} />
              </button>
              <span>Command deck</span>
            </div>
            <Segmented
              compact
              items={navItems.map((item) => item.label)}
              onChange={onViewChange}
              value={showHome ? undefined : viewLabel}
            />
            <div className="topbar-actions">
              <button type="button" className="icon-button no-drag" aria-label="Back to Home" onClick={returnToHome}>
                <Home size={16} />
              </button>
              <button
                type="button"
                className="icon-button no-drag"
                aria-label="Toggle light dark mode"
                onClick={() => setTheme((mode) => (mode === "light" ? "dark" : "light"))}
                title={theme === "light" ? "Dark mode" : "Light mode"}
              >
                {theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
              </button>
              <button type="button" className="action-chip no-drag" onClick={handleNewProject}>
                <Download size={16} />
                New
              </button>
            </div>
          </div>

          <InstrumentRail
            recording={liveMeetingOpen}
            onRecord={() => {
              enterMeetingWorkspace("P1");
              setActiveTileByAnchor((current) => ({ ...current, P1: "live-capture" }));
              setLiveMeetingOpen(true);
            }}
            onImport={() => {
              enterMeetingWorkspace("P1");
              void handleImportAndTranscribe();
            }}
            importDisabled={transcribing}
            onExport={() => {
              enterMeetingWorkspace(activeAnchor);
              void handleCreateJob("export.render");
            }}
            exportTitle={
              jobActionBlockedReason("export.render", Boolean(activeRecordingId)) ??
              "ส่งออกซับไตเติล .srt และ .vtt ของการบันทึกนี้"
            }
            onPairDevice={() => setDevicePairingPanelOpen((open) => !open)}
            onOpenSettings={() => setSettingsPanelOpen(true)}
          />

          <div className={`power-dock no-drag ${powerMenuOpen ? "is-open" : ""}`}>
            <div className="power-radial" aria-hidden={!powerMenuOpen}>
              <button
                type="button"
                className="power-radial__item"
                onClick={() => void handleMinimizeWindow()}
                tabIndex={powerMenuOpen ? 0 : -1}
                aria-label="Minimize window"
              >
                <Minimize2 size={16} />
                <span>พับจอ</span>
              </button>
              <button
                type="button"
                className="power-radial__item power-radial__item--danger"
                onClick={() => void handleCloseWindow()}
                tabIndex={powerMenuOpen ? 0 : -1}
                aria-label="Close app"
              >
                <Power size={16} />
                <span>ปิด</span>
              </button>
            </div>
            <button
              type="button"
              className="fab fab-close power-trigger"
              aria-label="Power menu"
              aria-expanded={powerMenuOpen}
              onClick={() => setPowerMenuOpen((open) => !open)}
            >
              <Power size={18} />
            </button>
          </div>

        </main>
      </div>
    </div>
  );
}
