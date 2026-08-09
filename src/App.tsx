import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  Archive,
  AudioLines,
  Bell,
  ChevronRight,
  Cloud,
  Download,
  HardDriveDownload,
  Loader2,
  Mic,
  Minimize2,
  Moon,
  Pause,
  Play,
  Power,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Sun,
  TimerReset,
  Upload,
  UserCircle,
  Volume2,
  Wifi,
} from "lucide-react";
import {
  closeWindow,
  createJob,
  createProject,
  getHealth,
  graphBuildStart,
  importAndTranscribe,
  listJobs,
  listModelProviders,
  listProjects,
  listTranscriptSegments,
  minimizeWindow,
  openExternalAccountPortal,
  pickAudioOrVideoFile,
  renameSpeaker,
  startLocalApi,
  ttsSynthesizeText,
  type Health,
  type Job,
  type ModelProvider,
  type Project,
  type TranscriptSegment,
} from "./tauri";
import { ExternalAccountPanel } from "./components/ExternalAccountPanel";
import { ZoomPanel } from "./components/ZoomPanel";
import { TtsProviderPanel } from "./components/TtsProviderPanel";
import { AccountLoginPanel } from "./components/AccountLoginPanel";

function formatMs(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

const PANEL_PATH =
  "M 40,12 H 800 A 20 20 0 0 1 820,32 V 54 A 20 20 0 0 0 840,74 H 1248 A 20 20 0 0 1 1268,94 V 688 A 20 20 0 0 1 1248,708 H 112 A 20 20 0 0 1 92,688 V 350 A 20 20 0 0 0 72,330 H 32 A 20 20 0 0 1 12,310 V 40 A 28 28 0 0 1 40,12 Z";

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
  meterLabel: string;
  meterValue: string;
  currentLabel: string;
  tone: Tone;
  primaryAction: TileAction;
  secondaryAction: TileAction;
  activities: readonly ActivityEntry[];
  events: readonly EventEntry[];
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
        meterLabel: "Input readiness",
        meterValue: "88%",
        currentLabel: "Next step",
        tone: "sage",
        primaryAction: { kind: "record", label: "Start recording", value: "start" },
        secondaryAction: { kind: "anchor", label: "Open transcript", value: "P2" },
        activities: [
          { time: "09:42", title: "Meeting shell armed", detail: "Default local-first preset loaded for this session." },
          { time: "09:46", title: "Input map confirmed", detail: "Primary microphone and fallback device are available." },
          { time: "09:51", title: "Chunk autosave primed", detail: "SQLite WAL and file sink are ready for long recording." },
          { time: "09:57", title: "Consent note inserted", detail: "Meeting opener includes privacy-safe recording notice." },
        ],
        events: [
          { type: "Source", detail: "USB microphone / 48kHz", state: "Ready" },
          { type: "Preset", detail: "Long meeting / chunked autosave", state: "Armed" },
          { type: "Storage", detail: "Local project folder visible", state: "OK" },
          { type: "Privacy", detail: "No cloud route on this mode", state: "Local" },
        ],
      },
      {
        id: "live-capture",
        eyebrow: "P1 / Live",
        title: "Live capture",
        detail: "Watch elapsed time, chunk sealing, clipping, and silence so the meeting can run without babysitting.",
        action: "Pause or resume",
        status: "Recording watch",
        meterLabel: "Capture stability",
        meterValue: "93%",
        currentLabel: "Current focus",
        tone: "indigo",
        primaryAction: { kind: "record", label: "Pause or resume", value: "toggle" },
        secondaryAction: { kind: "job", label: "Add marker", value: "capture.marker" },
        activities: [
          { time: "10:03", title: "Chunk 18 sealed", detail: "Recovered from standby in 190ms and resumed cleanly." },
          { time: "10:06", title: "Noise floor steady", detail: "Room tone stayed below the cleanup threshold." },
          { time: "10:09", title: "Side marker dropped", detail: "Budget discussion tagged for later recap." },
          { time: "10:12", title: "Battery warning dismissed", detail: "External power is holding during the session." },
        ],
        events: [
          { type: "Live meter", detail: "No clipping in the last 12 minutes", state: "Stable" },
          { type: "Chunks", detail: "Autosave sealing every 02:00", state: "On" },
          { type: "Silence", detail: "Short pause windows detected", state: "Normal" },
          { type: "Transcript", detail: "Ready when recording stops", state: "Next" },
        ],
      },
      {
        id: "side-notes",
        eyebrow: "P1 / Notes",
        title: "Side notes",
        detail: "Drop quick flags for agenda shifts, decisions, and moments worth exporting before details disappear.",
        action: "Add marker",
        status: "Marker lane open",
        meterLabel: "Note coverage",
        meterValue: "64%",
        currentLabel: "Capture aid",
        tone: "metal",
        primaryAction: { kind: "job", label: "Add marker", value: "capture.marker" },
        secondaryAction: { kind: "anchor", label: "Go to transcript", value: "P2" },
        activities: [
          { time: "10:14", title: "Decision marker added", detail: "Hiring budget section tagged for action extraction." },
          { time: "10:17", title: "Question marker added", detail: "Pending legal review called out for follow-up." },
          { time: "10:20", title: "Speaker handoff marked", detail: "Useful transition stored for diarization review." },
          { time: "10:24", title: "Noise note attached", detail: "Coffee machine burst flagged for cleanup pass." },
        ],
        events: [
          { type: "Marker", detail: "Decision / question / follow-up presets", state: "Ready" },
          { type: "Agenda", detail: "Three sections logged so far", state: "Tracked" },
          { type: "Evidence", detail: "Markers will surface in recap review", state: "Linked" },
          { type: "Cleanup", detail: "Noise notes preserved for later pass", state: "Held" },
        ],
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
        meterLabel: "Transcript completeness",
        meterValue: "91%",
        currentLabel: "Review step",
        tone: "indigo",
        primaryAction: { kind: "job", label: "Review transcript", value: "transcript.transcribe" },
        secondaryAction: { kind: "job", label: "Rerun transcript", value: "transcript.retry" },
        activities: [
          { time: "10:32", title: "Transcript generated", detail: "Timestamped text arrived for the latest meeting chunk set." },
          { time: "10:36", title: "Punctuation refined", detail: "Japanese and English punctuation pass completed locally." },
          { time: "10:41", title: "Silence spans merged", detail: "Small dead air windows collapsed for easier reading." },
          { time: "10:45", title: "Open issue flagged", detail: "One phrase remains low confidence for manual review." },
        ],
        events: [
          { type: "Transcript", detail: "Current pass covers the full meeting", state: "Ready" },
          { type: "Language", detail: "JP / EN mixed transcript preserved", state: "OK" },
          { type: "Confidence", detail: "One span below confidence threshold", state: "Review" },
          { type: "Evidence", detail: "Timestamps remain attached while editing", state: "Pinned" },
        ],
      },
      {
        id: "speaker-pass",
        eyebrow: "P2 / Speakers",
        title: "Speaker pass",
        detail: "Merge, rename, and lock speaker lanes only where the meeting recap actually depends on who said what.",
        action: "Lock speakers",
        status: "Speaker map in review",
        meterLabel: "Speaker confidence",
        meterValue: "82%",
        currentLabel: "Speaker task",
        tone: "sage",
        primaryAction: { kind: "job", label: "Lock speakers", value: "speakers.lock" },
        secondaryAction: { kind: "job", label: "Rerun diarization", value: "speakers.diarize" },
        activities: [
          { time: "10:48", title: "Speaker lane merged", detail: "Two adjacent voices were merged after a short interruption." },
          { time: "10:52", title: "Display names updated", detail: "Role labels replaced temporary speaker IDs for recap use." },
          { time: "10:55", title: "Overlap detected", detail: "Cross-talk segment flagged for manual listen-through." },
          { time: "10:58", title: "Lock point suggested", detail: "Most major turns are stable enough for summary jobs." },
        ],
        events: [
          { type: "Speaker map", detail: "2 primary speakers / 1 guest lane", state: "Active" },
          { type: "Overlap", detail: "Short interruption at 14:26 requires review", state: "Open" },
          { type: "Identity", detail: "Labels stay editable, never biometric", state: "Safe" },
          { type: "Summary", detail: "Per-person recap unlocks after speaker lock", state: "Next" },
        ],
      },
      {
        id: "highlight-pass",
        eyebrow: "P2 / Evidence",
        title: "Highlight pass",
        detail: "Mark decisions, questions, and quotes worth carrying into summary, intent, and export bundles.",
        action: "Mark evidence",
        status: "Evidence pass open",
        meterLabel: "Evidence coverage",
        meterValue: "74%",
        currentLabel: "Highlight layer",
        tone: "metal",
        primaryAction: { kind: "job", label: "Mark evidence", value: "review.evidence" },
        secondaryAction: { kind: "anchor", label: "Open summary", value: "P3" },
        activities: [
          { time: "11:02", title: "Decision span pinned", detail: "Budget approval exchange linked for the recap card." },
          { time: "11:05", title: "Quote preserved", detail: "One stakeholder concern tagged for intent comparison." },
          { time: "11:09", title: "Action owner noted", detail: "Follow-up responsibility marked beside the transcript turn." },
          { time: "11:12", title: "Open question tagged", detail: "Procurement answer remains unresolved for later review." },
        ],
        events: [
          { type: "Highlights", detail: "Decisions / concerns / follow-ups", state: "Tagged" },
          { type: "Quote bank", detail: "Three spans saved for export bundle", state: "Ready" },
          { type: "Actions", detail: "Ownership hints flow into action extraction", state: "Linked" },
          { type: "Intent", detail: "Pinned spans improve per-person analysis", state: "Useful" },
        ],
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
        meterLabel: "Recap completeness",
        meterValue: "89%",
        currentLabel: "Summary layer",
        tone: "sage",
        primaryAction: { kind: "job", label: "Generate recap", value: "summary.recap" },
        secondaryAction: { kind: "job", label: "Compare summaries", value: "summary.compare" },
        activities: [
          { time: "11:18", title: "Recap draft generated", detail: "A concise story summary was built from pinned evidence spans." },
          { time: "11:22", title: "Timeline tightened", detail: "Non-essential side chatter was collapsed out of the main narrative." },
          { time: "11:25", title: "Decision cluster grouped", detail: "Budget, hiring, and vendor decisions were separated cleanly." },
          { time: "11:28", title: "Evidence audit passed", detail: "Every recap paragraph still points back to transcript time ranges." },
        ],
        events: [
          { type: "Recap", detail: "Short, evidence-backed meeting story", state: "Ready" },
          { type: "Timeline", detail: "Chronology preserved in the draft", state: "Kept" },
          { type: "Proof", detail: "Evidence spans attached to recap blocks", state: "Linked" },
          { type: "Risk", detail: "Low-confidence lines stay outside the lead", state: "Safe" },
        ],
      },
      {
        id: "people-intent",
        eyebrow: "P3 / Intent",
        title: "People and intent",
        detail: "Separate what each speaker likely wanted, worried about, or committed to, without presenting inference as fact.",
        action: "Analyze intent",
        status: "Intent in review",
        meterLabel: "Intent confidence",
        meterValue: "78%",
        currentLabel: "Inference layer",
        tone: "indigo",
        primaryAction: { kind: "job", label: "Analyze intent", value: "summary.intent" },
        secondaryAction: { kind: "anchor", label: "Review speakers", value: "P2" },
        activities: [
          { time: "11:31", title: "Intent map refreshed", detail: "Per-speaker motivations were recomputed from the corrected transcript." },
          { time: "11:35", title: "Concern highlighted", detail: "One stakeholder repeatedly returned to timeline risk." },
          { time: "11:38", title: "Commitment extracted", detail: "A likely owner was inferred for the vendor follow-up thread." },
          { time: "11:41", title: "Weak span isolated", detail: "One uncertain inference was quarantined until speaker review improves." },
        ],
        events: [
          { type: "Intent", detail: "AI inference stays labelled with uncertainty", state: "Visible" },
          { type: "Concerns", detail: "Three recurring concern patterns found", state: "Tracked" },
          { type: "Commitments", detail: "Likely owners surfaced from final turns", state: "Draft" },
          { type: "Evidence", detail: "Intent cards only cite pinned transcript spans", state: "Bounded" },
        ],
      },
      {
        id: "action-register",
        eyebrow: "P3 / Actions",
        title: "Action register",
        detail: "Pull follow-ups, decisions, owners, and unresolved questions into a package people can actually use after the call.",
        action: "Extract actions",
        status: "Actions ready",
        meterLabel: "Action coverage",
        meterValue: "84%",
        currentLabel: "Follow-up layer",
        tone: "metal",
        primaryAction: { kind: "job", label: "Extract actions", value: "summary.actions" },
        secondaryAction: { kind: "anchor", label: "Prepare export", value: "P4" },
        activities: [
          { time: "11:44", title: "Action list compiled", detail: "Follow-ups were grouped by owner and dependency." },
          { time: "11:48", title: "Deadline note added", detail: "One task was linked to the Friday procurement checkpoint." },
          { time: "11:51", title: "Question bucket created", detail: "Open issues stayed separate from confirmed commitments." },
          { time: "11:54", title: "Export tags staged", detail: "Action items are ready for bundle rendering." },
        ],
        events: [
          { type: "Actions", detail: "Owners, next steps, and due cues", state: "Ready" },
          { type: "Questions", detail: "Unresolved topics stay visible after export", state: "Held" },
          { type: "Dependencies", detail: "Cross-team follow-ups grouped together", state: "Mapped" },
          { type: "Bundle", detail: "Action sheet can ride with recap export", state: "Next" },
        ],
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
        meterLabel: "Bundle readiness",
        meterValue: "87%",
        currentLabel: "Delivery step",
        tone: "metal",
        primaryAction: { kind: "job", label: "Export bundle", value: "export.render" },
        secondaryAction: { kind: "job", label: "Open export queue", value: "export.queue" },
        activities: [
          { time: "12:02", title: "Bundle profile selected", detail: "Meeting recap package chosen with transcript and audio outputs." },
          { time: "12:05", title: "Export queue armed", detail: "Primary artifacts are ready to render in one local pass." },
          { time: "12:09", title: "Transcript sidecars staged", detail: "TXT, SRT, VTT, and JSON outputs are aligned to the same source." },
          { time: "12:12", title: "Audio outputs validated", detail: "Original WAV and processed MP3 are both eligible for export." },
        ],
        events: [
          { type: "Bundle", detail: "WAV / MP3 / TXT / SRT / VTT / JSON / MD", state: "Ready" },
          { type: "Queue", detail: "No active export contention", state: "Clear" },
          { type: "Path", detail: "Artifacts stay inside the local project tree", state: "Local" },
          { type: "Proof", detail: "Summary and intent outputs keep provenance", state: "Bound" },
        ],
      },
      {
        id: "privacy-policy",
        eyebrow: "P4 / Policy",
        title: "Privacy and policy",
        detail: "Confirm local-only mode, provider posture, and inference labeling before the meeting leaves your machine.",
        action: "Review policy",
        status: "Policy guarded",
        meterLabel: "Privacy posture",
        meterValue: "96%",
        currentLabel: "Governance step",
        tone: "sage",
        primaryAction: { kind: "anchor", label: "Review policy", value: "P4" },
        secondaryAction: { kind: "api", label: "Start local API", value: "start" },
        activities: [
          { time: "12:16", title: "Policy banner confirmed", detail: "Intent stays labeled as AI inference in this export profile." },
          { time: "12:19", title: "Remote route blocked", detail: "No cloud provider is enabled for this meeting project." },
          { time: "12:22", title: "Consent note preserved", detail: "Meeting metadata keeps the recording disclosure note." },
          { time: "12:25", title: "Retention rule attached", detail: "Local-only archive policy follows the export bundle." },
        ],
        events: [
          { type: "Privacy", detail: "Loopback-only and no remote upload", state: "On-device" },
          { type: "Inference", detail: "Intent remains marked as probabilistic", state: "Clear" },
          { type: "Retention", detail: "Project keeps a local-only archive policy", state: "Attached" },
          { type: "API", detail: "Background API available on demand", state: "Ready" },
        ],
      },
      {
        id: "runtime-archive",
        eyebrow: "P4 / Runtime",
        title: "Runtime and archive",
        detail: "Check provider health, storage path, and archive safety so this meeting can be reopened or audited later.",
        action: "Archive project",
        status: "Runtime stable",
        meterLabel: "Runtime health",
        meterValue: "90%",
        currentLabel: "System step",
        tone: "indigo",
        primaryAction: { kind: "job", label: "Archive project", value: "archive.project" },
        secondaryAction: { kind: "api", label: "Start local API", value: "start" },
        activities: [
          { time: "12:28", title: "Provider check passed", detail: "Enabled local models are reachable from this workspace." },
          { time: "12:31", title: "Archive lane created", detail: "Meeting folder is ready for post-export retention." },
          { time: "12:34", title: "Job history linked", detail: "Model provenance and export steps remain queryable." },
          { time: "12:37", title: "Recovery point saved", detail: "The project can be reopened without reprocessing core artifacts." },
        ],
        events: [
          { type: "Providers", detail: "BYOM route and local runtime check", state: "Ready" },
          { type: "Database", detail: "Project path and WAL state remain visible", state: "WAL" },
          { type: "Archive", detail: "Source, recap, and provenance stay together", state: "Stable" },
          { type: "Recovery", detail: "Job history remains local and queryable", state: "Kept" },
        ],
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
  value: T;
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
  const [segments, setSegments] = useState<TranscriptSegment[]>([]);
  const [transcribing, setTranscribing] = useState(false);
  const [activeAnchor, setActiveAnchor] = useState<Anchor>("P2");
  const [activeView, setActiveView] = useState<ViewId>("review");
  const [theme, setTheme] = useState<ThemeMode>("light");
  const [powerMenuOpen, setPowerMenuOpen] = useState(false);
  const [accountPanelOpen, setAccountPanelOpen] = useState(false);
  const [zoomPanelOpen, setZoomPanelOpen] = useState(false);
  const [ttsPanelOpen, setTtsPanelOpen] = useState(false);
  const [accountLoginPanelOpen, setAccountLoginPanelOpen] = useState(false);
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

  const libraryItems = useMemo<LibraryItem[]>(() => {
    if (projects.length === 0) {
      return [
        { id: "REC-241", title: "Morning notes", subtitle: "24m capture", state: "Armed" },
        { id: "REC-198", title: "Board memo", subtitle: "Import ready", state: "Queued" },
        { id: "REC-212", title: "Field interview", subtitle: "2 speakers", state: "Live" },
        { id: "REC-207", title: "Cafe ambience", subtitle: "Reference bed", state: "Soft" },
        { id: "REC-190", title: "Voice journal", subtitle: "Draft marked", state: "Saved" },
      ];
    }

    return projects.slice(0, 5).map((project, index) => ({
      id: project.id,
      title: project.name,
      subtitle:
        index === 0
          ? `Updated ${new Date(project.updatedAt).toLocaleDateString()}`
          : `Created ${new Date(project.createdAt).toLocaleDateString()}`,
      state: index === 0 ? "Live" : index === 1 ? "Queued" : "Saved",
    }));
  }, [projects]);

  const [selectedRecording, setSelectedRecording] = useState<string>("REC-212");

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

  useEffect(() => {
    if (!selectedProjectId) {
      setSegments([]);
      return;
    }
    void listTranscriptSegments(selectedProjectId).then(setSegments);
  }, [selectedProjectId, jobs]);

  const currentPage = pageContent[activeAnchor];
  const currentTile =
    currentPage.tiles.find((tile) => tile.id === activeTileByAnchor[activeAnchor]) ?? currentPage.tiles[0];
  const meetingTitle = selectedCard?.title ?? "Leadership weekly sync";
  const meetingSubtitle = selectedCard?.subtitle ?? "2 speakers / local review deck";

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

  const runtimeStats = useMemo(() => {
    const apiState = health?.localApi.running ? "API live" : "Offline";
    const providerCount = providers.filter((provider) => provider.enabled).length;

    switch (activeAnchor) {
      case "P1":
        return [
          { label: "Session", value: recording ? "Live" : "Armed", meta: selectedCard?.state ?? "Ready" },
          { label: "Capture", value: recording ? "01:24" : "00:00", meta: "Chunked" },
          { label: "Noise", value: "-34dB", meta: "Room tone" },
          { label: "Next", value: "Transcript", meta: "After stop" },
        ];
      case "P2":
        return [
          { label: "Transcript", value: "91%", meta: "Ready" },
          { label: "Speakers", value: "2 lanes", meta: "Editable" },
          { label: "Evidence", value: "7 spans", meta: "Pinned" },
          { label: "Next", value: "Summary", meta: "After review" },
        ];
      case "P3":
        return [
          { label: "Recap", value: "1 draft", meta: "Cited" },
          { label: "Intent", value: "3 people", meta: "AI inference" },
          { label: "Actions", value: "6 items", meta: "Owners tagged" },
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
  }, [activeAnchor, health, jobs.length, providers, recording, selectedCard]);

  const activityFeed = useMemo(() => {
    if (activeAnchor === "P2" && currentTile.id === "transcript-pass" && segments.length > 0) {
      return segments.slice(0, 16).map((segment) => ({
        time: formatMs(segment.startMs),
        title: segment.text.length > 60 ? `${segment.text.slice(0, 60)}…` : segment.text,
        detail: segment.confidence != null ? `Confidence ${(segment.confidence * 100).toFixed(0)}%` : "faster-whisper",
        speakerId: segment.speakerId,
        speakerName: segment.speakerName,
      }));
    }

    const items = currentTile.activities.map((entry) => ({ ...entry }));
    if (jobs.length > 0) {
      const latestJob = jobs[0];
      items[0] = {
        time: new Date(latestJob.updatedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        title: `${latestJob.type} ${latestJob.status}`,
        detail: latestJob.errorMessage ?? `Progress ${latestJob.progress}% for ${latestJob.projectId}`,
      };
    }
    return items;
  }, [activeAnchor, currentTile.activities, currentTile.id, jobs, segments]);

  const activeTranscribeJob = jobs.find(
    (job) => job.type === "transcript.transcribe" && job.status === "running",
  );

  const primaryActionLabel =
    transcribing && currentTile.primaryAction.value === "transcript.transcribe"
      ? `Transcribing… ${activeTranscribeJob?.progress ?? 0}%`
      : currentTile.primaryAction.label;

  const eventFeed = useMemo(() => {
    if (activeAnchor === "P4" && providers.length > 0) {
      return providers.slice(0, 4).map((provider) => ({
        type: provider.kind,
        detail: provider.label,
        state: provider.enabled ? provider.runtimeLocation : "Disabled",
      }));
    }
    return currentTile.events.map((entry) => ({ ...entry }));
  }, [activeAnchor, currentTile.events, providers]);

  const signalCards = useMemo(
    () => {
      const byPage: Record<Anchor, Array<{ id: SignalId; title: string; value: string; foot: string }>> = {
        P1: [
          { id: "health", title: "Capture safety", value: recording ? "Guarded" : "Armed", foot: "Chunks and WAL are ready." },
          { id: "privacy", title: "Audio cleanliness", value: "Stable", foot: "Noise floor stays under the cleanup threshold." },
          { id: "queue", title: "Session readiness", value: "Transcript next", foot: "Stop capture to begin transcript review." },
          { id: "focus", title: currentPage.signalTitle, value: currentTile.title, foot: meetingSubtitle },
        ],
        P2: [
          { id: "health", title: "Transcript completeness", value: "91%", foot: "Full meeting transcript is available." },
          { id: "privacy", title: "Speaker confidence", value: "82%", foot: "Editable labels remain non-biometric." },
          { id: "queue", title: "Evidence coverage", value: "7 spans", foot: "Pinned spans will feed recap and export." },
          { id: "focus", title: currentPage.signalTitle, value: currentTile.title, foot: meetingSubtitle },
        ],
        P3: [
          { id: "health", title: "Recap completeness", value: "Drafted", foot: "Evidence-backed meeting story is ready." },
          { id: "privacy", title: "Intent confidence", value: "Bounded", foot: "Inference stays labeled with uncertainty." },
          { id: "queue", title: "Action extraction", value: "6 open", foot: "Owners and follow-ups are staged for export." },
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
    [activeAnchor, currentPage.signalTitle, currentTile.title, health, jobs.length, meetingSubtitle, providerSummary, providers.length, recording],
  );

  const viewLabel = navItems.find((item) => item.id === activeView)?.label ?? "Transcript";

  const onViewChange = (label: NavLabel) => {
    const nextView = navItems.find((item) => item.label === label)?.id ?? "review";
    setActiveView(nextView);
    setActiveAnchor(anchorByView[nextView]);
  };

  const activateAnchor = (anchor: Anchor) => {
    setActiveAnchor(anchor);
    setActiveView(viewByAnchor[anchor]);
  };

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

  const pollJobUntilDone = async (jobId: string) => {
    for (let attempt = 0; attempt < 600; attempt += 1) {
      const nextJobs = await listJobs();
      setJobs(nextJobs);
      const job = nextJobs.find((entry) => entry.id === jobId);
      if (!job || job.status === "completed" || job.status === "failed") return;
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  };

  const handleRenameSpeaker = async (speakerId: string, displayName: string) => {
    await renameSpeaker(speakerId, displayName);
    if (selectedProjectId) {
      const nextSegments = await listTranscriptSegments(selectedProjectId);
      setSegments(nextSegments);
    }
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
      // If no provider registered, open TTS settings
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("ยังไม่ได้ลงทะเบียน")) {
        setTtsPanelOpen(true);
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
      await pollJobUntilDone(job.id);
      await refresh();
    } finally {
      setTranscribing(false);
    }
  };

  const handleCreateJob = async (jobType: string) => {
    if (jobType === "transcript.transcribe") {
      await handleImportAndTranscribe();
      return;
    }

    let projectId = selectedProjectId;
    if (!projectId) {
      const project = await createProject(`Session ${projects.length + 1}`);
      projectId = project.id;
      setSelectedRecording(project.id);
    }
    await createJob(jobType, projectId);
    await refresh();
  };

  const handleStartApi = async () => {
    await startLocalApi();
    await refresh();
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
      if (action.value === "start") {
        if (!selectedProjectId) {
          await handleNewProject();
        }
        setRecording(true);
        activateTile("live-capture");
        return;
      }

      setRecording((value) => !value);
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
      {accountPanelOpen && <ExternalAccountPanel onClose={() => setAccountPanelOpen(false)} onOpenPortal={openExternalAccountPortal} />}
      {zoomPanelOpen && <ZoomPanel onClose={() => setZoomPanelOpen(false)} />}
      {ttsPanelOpen && <TtsProviderPanel onClose={() => setTtsPanelOpen(false)} />}
      {accountLoginPanelOpen && <AccountLoginPanel />}
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
            <section className="zone anchor-rail" aria-label="Anchor rail">
              <div className="eyebrow">Pages</div>
              {pageAnchors.map((anchor) => (
                <button
                  key={anchor.id}
                  type="button"
                  className={`anchor-chip ${anchor.id === activeAnchor ? "is-active" : ""}`}
                  onClick={() => activateAnchor(anchor.id)}
                  title={anchor.domain}
                >
                  <span>{anchor.id}</span>
                  <em>{anchor.label}</em>
                  <ChevronRight size={13} />
                </button>
              ))}
            </section>

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
                  <div className="focus-detail-dock__actions">
                    <button
                      type="button"
                      className="quick-action quick-action--primary"
                      disabled={transcribing}
                      onClick={() => void performTileAction(currentTile.primaryAction)}
                    >
                      {primaryActionLabel}
                    </button>
                    <button
                      type="button"
                      className="quick-action"
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
                <div className="agent-meter">
                  <div>
                    <span>{currentTile.meterLabel}</span>
                    <strong>{currentTile.meterValue}</strong>
                  </div>
                  <div className="meter">
                    <span style={{ width: currentTile.meterValue }} />
                  </div>
                </div>

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

                <div className="quick-actions">
                  <button
                    type="button"
                    className="quick-action quick-action--primary"
                    disabled={transcribing}
                    onClick={() => void performTileAction(currentTile.primaryAction)}
                  >
                    {primaryActionLabel}
                  </button>
                  <button
                    type="button"
                    className="quick-action"
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
              value={viewLabel}
            />
            <div className="topbar-actions">
              <button
                type="button"
                className="icon-button no-drag"
                aria-label="Toggle light dark mode"
                onClick={() => setTheme((mode) => (mode === "light" ? "dark" : "light"))}
                title={theme === "light" ? "Dark mode" : "Light mode"}
              >
                {theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
              </button>
              <button type="button" className="icon-button no-drag" aria-label="Notifications">
                <Bell size={16} />
              </button>
              <button type="button" className="action-chip no-drag" onClick={handleNewProject}>
                <Download size={16} />
                New
              </button>
            </div>
          </div>

          <div className="fab fab-sidebar no-drag">
            <button
              type="button"
              className={`sidebar-action ${recording ? "is-active" : ""}`}
              aria-label="New recording"
              onClick={() => setRecording((value) => !value)}
            >
              {recording ? <Pause size={20} /> : <Mic size={20} />}
            </button>
            <button
              type="button"
              className={`sidebar-action ${transcribing ? "is-active" : ""}`}
              aria-label="Import audio"
              title="Import audio or video and transcribe locally"
              disabled={transcribing}
              onClick={() => void handleImportAndTranscribe()}
            >
              <Upload size={20} />
            </button>
            <button
              type="button"
              className="sidebar-action"
              aria-label="Playback"
              onClick={() => void handleCreateJob("transcript.transcribe")}
            >
              <Play size={20} />
            </button>
            <button
              type="button"
              className="sidebar-action"
              aria-label="Storage"
              onClick={() => void handleCreateJob("export.render")}
            >
              <HardDriveDownload size={20} />
            </button>
            <button
              type="button"
              className="sidebar-action"
              aria-label="Runtime"
              onClick={() => void handleStartApi()}
            >
              <Activity size={20} />
            </button>
              <button
                type="button"
                className="sidebar-action"
                aria-label="Settings"
                title="Account and connection settings"
                onClick={() => setAccountPanelOpen(true)}
              >
                <SlidersHorizontal size={20} />
              </button>
              <button
                type="button"
                className="sidebar-action"
                aria-label="TTS Providers"
                title="TTS Providers"
                onClick={() => setTtsPanelOpen(true)}
              >
                <Volume2 size={20} />
              </button>
              <button
                type="button"
                className="sidebar-action"
                aria-label="บัญชี & อุปกรณ์"
                title="บัญชี & อุปกรณ์"
                onClick={() => setAccountLoginPanelOpen((open) => !open)}
              >
                <UserCircle size={20} />
              </button>
              <button
                type="button"
                className="sidebar-action"
                aria-label="Zoom"
                title="Import from Zoom"
                onClick={() => setZoomPanelOpen(true)}
              >
                <Cloud size={20} />
              </button>
          </div>

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
