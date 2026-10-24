import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  closeWindow,
  createTauriMeetingIntelligenceService,
  createProject,
  getHealth,
  hideWindowToTray,
  importAndTranscribe,
  listJobs,
  listProjects,
  minimizeWindow,
  nativeInvoke,
  openExternalAccountPortal,
  pickAudioOrVideoFile,
  startLocalApi,
  type Health,
  type Job,
  type LiveStatusOutput,
  type Project,
} from "./tauri";
import {
  LiveMeetingPanel,
  type LiveControllerSnapshot,
  type LiveMeetingController,
} from "./components/LiveMeetingPanel";
import { MeetingIntelligencePanel } from "./components/MeetingIntelligencePanel";
import { DesktopShell, getCaptureLifecycle } from "./components/desktop/DesktopShell";
import { RecordingReview, type RecoveryRefresh } from "./components/desktop/RecordingReview";
import { RecordingOutputPanel } from "./components/desktop/RecordingOutputPanel";
import {
  normalizeReviewError,
  type CloseReviewPlayer,
  type DesktopSurface,
  type ReadState,
  type ReviewError,
  type ThemeChoice,
} from "./components/desktop/contracts";
import type { InvokeFn, RecoveredRecording, RecoveryReport } from "./lib/recoveryFlow";
import { brokerSessionStatus, type SessionStatus } from "./lib/desktopSessionBroker";

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

const NATIVE_UNAVAILABLE_ERROR: ReviewError = {
  code: "NATIVE_UNAVAILABLE",
  message: "การอ่านข้อมูลส่วนนี้ต้องใช้ FUNG Desktop",
  retryable: false,
};

function createReadState<T>(
  status: ReadState<T>["status"],
  data: T | null = null,
  error: ReviewError | null = null,
): ReadState<T> {
  return { status, identity: null, data, error };
}

const INITIAL_LIVE_SNAPSHOT: LiveControllerSnapshot = {
  liveStatus: createReadState<LiveStatusOutput>(
    nativeInvoke ? "idle" : "unavailable",
    null,
    nativeInvoke ? null : NATIVE_UNAVAILABLE_ERROR,
  ),
  phase: "idle",
  selection: null,
};

type ThemeMode = ThemeChoice;
type ResolvedTheme = "light" | "dark";

const SYSTEM_THEME_QUERY = "(prefers-color-scheme: dark)";

export function resolveEffectiveTheme(theme: ThemeMode, systemTheme: ResolvedTheme): ResolvedTheme {
  return theme === "system" ? systemTheme : theme;
}

export function readSystemTheme(): ResolvedTheme {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return "light";
  return window.matchMedia(SYSTEM_THEME_QUERY).matches ? "dark" : "light";
}

export function subscribeToSystemTheme(onChange: (theme: ResolvedTheme) => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return () => {};
  const media = window.matchMedia(SYSTEM_THEME_QUERY);
  const update = (matches: boolean) => onChange(matches ? "dark" : "light");
  update(media.matches);
  const listener = (event: MediaQueryListEvent) => update(event.matches);
  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}

export function App() {
  const meetingIntelligenceService = useMemo(createTauriMeetingIntelligenceService, []);
  const [health, setHealth] = useState<Health | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectState, setProjectState] = useState<ReadState<Project[]>>(() =>
    createReadState<Project[]>(
      nativeInvoke ? "loading" : "unavailable",
      null,
      nativeInvoke ? null : NATIVE_UNAVAILABLE_ERROR,
    ),
  );
  // Why a Desktop action could not run; keep failures visible instead of
  // silently treating an unavailable native command as success.
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemeMode>("light");
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() => readSystemTheme());
  const [accountStatus, setAccountStatus] = useState<SessionStatus | null>(null);
  const [activeSurface, setActiveSurface] = useState<DesktopSurface>("home");
  const [reviewSelection, setReviewSelection] = useState<{
    projectId: string;
    recordingId: string;
  } | null>(null);
  const [meetingIntelligenceOpen, setMeetingIntelligenceOpen] = useState(false);
  const [liveController, setLiveController] = useState<LiveMeetingController | null>(null);
  const [liveSnapshot, setLiveSnapshot] = useState<LiveControllerSnapshot>(INITIAL_LIVE_SNAPSHOT);
  const [devicePairingPanelOpen, setDevicePairingPanelOpen] = useState(false);
  const [settingsPanelOpen, setSettingsPanelOpen] = useState(false);

  const reviewClosePlayerRef = useRef<CloseReviewPlayer | null>(null);
  const recoveryRefreshRef = useRef<RecoveryRefresh | null>(null);
  const recoveryProjectByRecordingRef = useRef(new Map<string, string>());

  useEffect(() => subscribeToSystemTheme(setSystemTheme), []);

  useEffect(() => {
    if (!nativeInvoke) return;
    let cancelled = false;
    const refreshAccount = async () => {
      try {
        const next = await brokerSessionStatus();
        if (!cancelled) setAccountStatus(next);
      } catch {
        if (!cancelled) setAccountStatus(null);
      }
    };
    void refreshAccount();
    const timer = window.setInterval(() => void refreshAccount(), 1000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const registerClosePlayer = useCallback((closePlayer: CloseReviewPlayer | null) => {
    reviewClosePlayerRef.current = closePlayer;
    return () => {
      if (reviewClosePlayerRef.current === closePlayer) reviewClosePlayerRef.current = null;
    };
  }, []);

  const closeReviewPlayer = useCallback(async () => {
    if (!reviewClosePlayerRef.current) return { closed: true };
    return reviewClosePlayerRef.current();
  }, []);

  const registerRecoveryRefresh = useCallback((refreshRecovered: RecoveryRefresh | null) => {
    recoveryRefreshRef.current = refreshRecovered;
    return () => {
      if (recoveryRefreshRef.current === refreshRecovered) recoveryRefreshRef.current = null;
    };
  }, []);

  const recoveryInvoke = useMemo<InvokeFn | null>(() => {
    const invoke = nativeInvoke;
    if (!invoke) return null;

    const invokeWithRecoveryObserver: InvokeFn = async <T,>(
      command: string,
      args?: Record<string, unknown>,
    ) => {
      const result = await invoke<T>(command, args);

      if (command === "recovery_scan") {
        const report = result as RecoveryReport;
        const nextMapping = new Map<string, string>();
        if (report && Array.isArray(report.interrupted)) {
          for (const item of report.interrupted) {
            if (item.recordingId && item.projectId) {
              nextMapping.set(item.recordingId, item.projectId);
            }
          }
        }
        recoveryProjectByRecordingRef.current = nextMapping;
      } else if (command === "recovery_recover") {
        const recovered = result as RecoveredRecording;
        const recordingId = recovered?.adopted?.recordingId;
        const projectId = recordingId
          ? recoveryProjectByRecordingRef.current.get(recordingId)
          : undefined;
        const refreshRecovered = recoveryRefreshRef.current;
        if (recordingId && projectId && refreshRecovered) {
          try {
            await refreshRecovered({ projectId, recordingId });
          } catch {
            // Recovery itself already succeeded; a review refresh is advisory.
          }
        }
      }

      return result;
    };

    return invokeWithRecoveryObserver;
  }, []);

  const refresh = useCallback(async () => {
    if (!nativeInvoke) {
      setProjectState(createReadState<Project[]>("unavailable", null, NATIVE_UNAVAILABLE_ERROR));
      setHealth(null);
      setProjects([]);
      return;
    }

    setProjectState((current) => ({ ...current, status: "loading", error: null }));
    const [healthResult, projectsResult] = await Promise.allSettled([
      getHealth(),
      listProjects(),
    ]);

    if (healthResult.status === "fulfilled") setHealth(healthResult.value);
    if (projectsResult.status === "fulfilled") {
      setProjects(projectsResult.value);
      setProjectState(createReadState<Project[]>("ready", projectsResult.value));
    } else {
      setProjectState((current) => ({
        ...current,
        status: "error",
        error: normalizeReviewError(projectsResult.reason),
      }));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleLiveControllerChange = useCallback((controller: LiveMeetingController | null) => {
    setLiveController(controller);
  }, []);

  useEffect(() => {
    if (!liveController) {
      setLiveSnapshot(INITIAL_LIVE_SNAPSHOT);
      return undefined;
    }

    setLiveSnapshot(liveController.getSnapshot());
    return liveController.subscribe(setLiveSnapshot);
  }, [liveController]);

  const [selectedRecording, setSelectedRecording] = useState<string>("");

  useEffect(() => {
    if (!projects.some((project) => project.id === selectedRecording)) {
      setSelectedRecording(projects[0]?.id ?? "");
    }
  }, [projects, selectedRecording]);

  const selectedProjectId = useMemo(
    () => (projects.some((project) => project.id === selectedRecording) ? selectedRecording : null),
    [projects, selectedRecording],
  );

  useEffect(() => {
    if (reviewSelection && reviewSelection.projectId !== selectedProjectId) {
      setReviewSelection(null);
    }
  }, [reviewSelection, selectedProjectId]);

  const handleReviewSelection = useCallback((selection: { projectId: string; recordingId: string } | null) => {
    if (selection && !projects.some((project) => project.id === selection.projectId)) {
      return;
    }
    setMeetingIntelligenceOpen(false);
    if (selection) setSelectedRecording(selection.projectId);
    setReviewSelection(selection);
  }, [projects]);

  const liveStatus: ReadState<LiveStatusOutput> = nativeInvoke ? liveSnapshot.liveStatus : {
    status: "unavailable",
    identity: null,
    data: null,
    error: { code: "NATIVE_UNAVAILABLE", message: "การอ่านสถานะการบันทึกต้องใช้ FUNG Desktop", retryable: false },
  };
  const stopAndLeave = useCallback(async (): Promise<LiveStatusOutput> => {
    if (!liveController) throw NATIVE_UNAVAILABLE_ERROR;
    return liveController.stopAndLeave();
  }, [liveController]);

  const handleStopCapture = useCallback(async () => {
    try {
      await stopAndLeave();
      setActionNotice("หยุดการบันทึกแล้ว — ตรวจสอบสถานะล่าสุดจาก Desktop");
    } catch {
      setActionNotice("หยุดการบันทึกไม่สำเร็จ — เซสชันยังอยู่ในพื้นที่ประชุมสด");
    }
  }, [stopAndLeave]);

  const openReviewSurface = () => {
    setMeetingIntelligenceOpen(false);
    setActiveSurface("review");
  };

  const openAppearanceSurface = () => {
    setActiveSurface("appearance");
  };

  const openOutputSurface = () => {
    setActiveSurface("output");
  };

  const returnToHome = () => {
    setActiveSurface("home");
  };

  const handleMinimizeWindow = async () => {
    await minimizeWindow();
  };

  const handleHideWindowToTray = async () => {
    await hideWindowToTray();
  };

  const handleCloseWindow = async () => {
    await closeWindow();
  };

  const handleNewProject = async () => {
    const name = `ประชุมใหม่ ${projects.length + 1}`;
    const project = await createProject(name);
    setSelectedRecording(project.id);
    setReviewSelection(null);
    await refresh();
    return project;
  };

  // Returns the job as it finished, so a caller can tell "done" from "failed"
  // rather than refreshing and hoping. `null` means it stopped being visible
  // or outlasted the poll.
  const pollJobUntilDone = async (jobId: string): Promise<Job | null> => {
    for (let attempt = 0; attempt < 600; attempt += 1) {
      const nextJobs = await listJobs();
      const job = nextJobs.find((entry) => entry.id === jobId);
      if (!job) return null;
      if (job.status === "completed" || job.status === "failed") return job;
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    return null;
  };

  const handleImportAndTranscribe = async () => {
    setActionNotice(null);
    if (!nativeInvoke) {
      setActionNotice("เปิด FUNG Desktop เพื่อนำเข้าไฟล์เสียงหรือวิดีโอ");
      return;
    }
    try {
      const filePath = await pickAudioOrVideoFile();
      if (!filePath) return;

      let projectId = selectedProjectId;
      if (!projectId) {
        const project = await handleNewProject();
        projectId = project.id;
      }

      setActionNotice("กำลังนำเข้าและถอดเสียงในเครื่อง…");
      const job = await importAndTranscribe(filePath, projectId);
      const finished = await pollJobUntilDone(job.id);
      if (finished?.status === "failed") {
        setActionNotice("นำเข้าและถอดเสียงไม่สำเร็จ — ตรวจสอบไฟล์และ local model แล้วลองใหม่");
      } else if (finished?.status === "completed") {
        setActionNotice("นำเข้าและถอดเสียงเสร็จแล้ว — เปิดบันทึกย้อนหลังเพื่อตรวจผล");
      } else {
        setActionNotice("งานนำเข้ายังไม่รายงานผลสุดท้าย — ตรวจสถานะจาก FUNG Desktop");
      }
      await refresh();
    } catch {
      setActionNotice("นำเข้าและถอดเสียงไม่สำเร็จ — ตรวจสอบไฟล์และ local model แล้วลองใหม่");
    }
  };

  const handleStartApi = async () => {
    await startLocalApi();
    await refresh();
  };

  const effectiveTheme = resolveEffectiveTheme(theme, systemTheme);

  return (
    <DesktopShell
      scopeChoice="B"
      project={projectState}
      selectedProjectId={selectedProjectId}
      selection={reviewSelection}
      activeSurface={activeSurface}
      liveStatus={liveStatus}
      livePhase={liveSnapshot.phase}
      theme={theme}
      accountStatus={accountStatus}
      actions={{
        selectProject: (projectId) => {
          if (!projects.some((project) => project.id === projectId)) return;
          setSelectedRecording(projectId);
          setReviewSelection(null);
        },
        selectRecording: handleReviewSelection,
        showHome: returnToHome,
        showLive: () => {
          setActiveSurface("live");
        },
        showReview: openReviewSurface,
        showOutput: openOutputSurface,
        showAppearance: openAppearanceSurface,
        startRecording: () => {
          setActiveSurface("live");
        },
        stopRecording: handleStopCapture,
        openReview: openReviewSurface,
        stopAndLeave,
        openSettings: () => setSettingsPanelOpen(true),
        openAccount: () => setSettingsPanelOpen(true),
        openPairing: () => setDevicePairingPanelOpen(true),
        importMedia: async () => {
          setMeetingIntelligenceOpen(false);
          setActiveSurface("review");
          await handleImportAndTranscribe();
        },
        exportMedia: () => {
          openReviewSurface();
          setActionNotice("เลือกบันทึก แล้วกด “ส่งออกบันทึกนี้” ในงานของการบันทึก");
        },
        exportDisabled: !selectedProjectId,
        exportTitle: selectedProjectId
          ? "เลือกบันทึกในประวัติเพื่อส่งออก transcript และไฟล์เสียงของรายการนั้น"
          : "เลือกโครงการก่อนส่งออกบันทึก",
        setTheme,
        minimizeWindow: handleMinimizeWindow,
        hideWindowToTray: handleHideWindowToTray,
        closeWindow: handleCloseWindow,
      }}
      recoverySlot={(
        <Suspense fallback={null}>
          <RecoveryNotice invoke={recoveryInvoke} />
        </Suspense>
      )}
      settingsSlot={settingsPanelOpen ? (
        <Suspense fallback={null}>
          <SettingsPanel
            onClose={() => setSettingsPanelOpen(false)}
            invoke={nativeInvoke}
            projectId={selectedProjectId}
            onStartApi={() => void handleStartApi()}
            apiRunning={Boolean(health?.localApi.running)}
            onFetchStarted={(job) => {
              void pollJobUntilDone(job.id).then(() => void refresh());
            }}
            onOpenExternalAccountPortal={openExternalAccountPortal}
          />
        </Suspense>
      ) : null}
      pairingSlot={devicePairingPanelOpen ? (
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
      ) : null}
      mainContent={activeSurface === "output" ? (
        <div className={`callmd-desktop-content theme-${effectiveTheme}`}>
          <RecordingOutputPanel captureActive={getCaptureLifecycle(liveStatus, liveSnapshot.phase) !== "inactive"} />
        </div>
      ) : (
        <div className="callmd-desktop-content">
          {actionNotice ? <p className="action-notice" role="status" aria-live="polite">{actionNotice}</p> : null}
          <div className={`callmd-surface-stack theme-${effectiveTheme}`} aria-label="พื้นที่บันทึกและทบทวน">
            <LiveMeetingPanel
              onClose={() => {
                setActiveSurface("home");
              }}
              projectId={selectedProjectId}
              visible={activeSurface === "live"}
              closeReviewPlayer={closeReviewPlayer}
              onControllerChange={handleLiveControllerChange}
            />
            <RecordingReview
              selectedProjectId={selectedProjectId}
              selection={reviewSelection}
              onSelect={handleReviewSelection}
              onOpenMeetingIntelligence={() => setMeetingIntelligenceOpen(true)}
              visible={activeSurface === "review" && !meetingIntelligenceOpen}
              registerClosePlayer={registerClosePlayer}
              registerRecoveryRefresh={registerRecoveryRefresh}
            />
            {activeSurface === "review" && meetingIntelligenceOpen && reviewSelection && meetingIntelligenceService ? (
              <MeetingIntelligencePanel
                projectId={reviewSelection.projectId}
                recordingId={reviewSelection.recordingId}
                service={meetingIntelligenceService}
                onClose={() => setMeetingIntelligenceOpen(false)}
              />
            ) : null}
          </div>
        </div>
      )}
    />
  );
}
