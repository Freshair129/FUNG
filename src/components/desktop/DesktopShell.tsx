import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MutableRefObject,
} from "react";
import type { LiveStatusOutput } from "../../tauri.ts";
import {
  normalizeReviewError,
  type DesktopShellProps,
  type DesktopSurface,
  type LivePhase,
  type ReadState,
  type ReviewError,
  type ThemeChoice,
} from "./contracts.ts";
import "./DesktopShell.css";

export type CaptureLifecycle = "inactive" | "starting" | "active" | "stopping";

export type ProjectPanelState =
  | "idle"
  | "loading"
  | "ready"
  | "empty"
  | "error"
  | "unavailable";

export const SURFACE_ITEMS: ReadonlyArray<{
  id: DesktopSurface;
  href: string;
  label: string;
}> = [
  { id: "home", href: "#home", label: "หน้าหลัก" },
  { id: "live", href: "#live", label: "ประชุมสด" },
  { id: "review", href: "#history", label: "บันทึกย้อนหลัง" },
];

export const THEME_ITEMS: ReadonlyArray<{ id: ThemeChoice; label: string }> = [
  { id: "system", label: "ตามระบบ" },
  { id: "light", label: "สว่าง" },
  { id: "dark", label: "มืด" },
];

const FUNG_MARK_PATH =
  "M32 8H68A24 24 0 0 1 92 32V68A24 24 0 0 1 68 92H32A24 24 0 0 1 8 68V32A24 24 0 0 1 32 8ZM47 22H53A14 14 0 0 1 67 36V64A14 14 0 0 1 53 78H47A14 14 0 0 1 33 64V36A14 14 0 0 1 47 22Z";

const FUNG_BRAND_MARK = (
  <svg
    className="desktop-shell__brand-mark"
    width={40}
    height={40}
    viewBox="0 0 100 100"
    aria-hidden="true"
    focusable="false"
  >
    <path fill="currentColor" fillRule="evenodd" d={FUNG_MARK_PATH} />
  </svg>
);

export const NAVIGATION_GUARD_CHOICES = [
  "continue",
  "stop",
  "stay",
] as const;

export type NavigationGuardChoice = (typeof NAVIGATION_GUARD_CHOICES)[number];

export function getCaptureLifecycle(
  liveStatus: Pick<ReadState<LiveStatusOutput>, "status" | "data">,
  livePhase: LivePhase = "idle",
): CaptureLifecycle {
  if (liveStatus.data?.stopping) return "stopping";
  if (livePhase === "stopping") return "stopping";
  if (liveStatus.data?.active) return "active";
  if (livePhase === "starting") return "starting";
  if (livePhase === "listening" || livePhase === "degraded") return "active";
  return "inactive";
}

export function shouldGuardNavigation(
  lifecycle: CaptureLifecycle,
  leavesCaptureContext: boolean,
): boolean {
  return leavesCaptureContext && lifecycle !== "inactive";
}

export function isInactiveCapture(status: LiveStatusOutput | null | undefined): boolean {
  return status?.active === false && status.stopping === false;
}

export function resolveProjectPanelState(
  project: Pick<ReadState<unknown[]>, "status" | "data">,
): ProjectPanelState {
  if (project.status === "loading") return "loading";
  if (project.status === "error") return "error";
  if (project.status === "unavailable") return "unavailable";
  if (project.status === "ready" && Array.isArray(project.data)) {
    return project.data.length === 0 ? "empty" : "ready";
  }
  return "idle";
}

export function normalizeShellError(error: unknown): ReviewError {
  return normalizeReviewError(error);
}

const STOP_NOT_CONFIRMED_ERROR = normalizeReviewError({
  code: "LEGACY_COMMAND_FAILED",
  message: "ยังยืนยันไม่ได้ว่าการบันทึกหยุดแล้ว",
  retryable: true,
});

export function resolveStopAndLeaveError(
  status: LiveStatusOutput | null | undefined,
): ReviewError | null {
  return isInactiveCapture(status) ? null : STOP_NOT_CONFIRMED_ERROR;
}

type NavigationIntent = {
  label: string;
  leavesCaptureContext: boolean;
  action: () => void | Promise<void>;
};

type SurfaceNavigationProps = {
  activeSurface: DesktopSurface;
  onNavigate: (surface: DesktopSurface, initiator: HTMLElement) => void;
};

function SurfaceNavigation({ activeSurface, onNavigate }: SurfaceNavigationProps) {
  return (
    <nav className="desktop-shell__nav" aria-label="พื้นที่ทำงาน">
      {SURFACE_ITEMS.map((item) => (
        <a
          key={item.id}
          className={`desktop-shell__nav-link${activeSurface === item.id ? " is-active" : ""}`}
          href={item.href}
          aria-current={activeSurface === item.id ? "page" : undefined}
          onClick={(event) => {
            event.preventDefault();
            onNavigate(item.id, event.currentTarget);
          }}
        >
          {item.label}
        </a>
      ))}
    </nav>
  );
}

type ThemeChoiceProps = {
  theme: ThemeChoice;
  onChange: (theme: ThemeChoice) => void;
};

function ThemeChoice({ theme, onChange }: ThemeChoiceProps) {
  return (
    <label className="desktop-shell__theme-choice" htmlFor="desktop-shell-theme">
      <span>ธีม</span>
      <select
        id="desktop-shell-theme"
        value={theme}
        onChange={(event) => onChange(event.target.value as ThemeChoice)}
      >
        {THEME_ITEMS.map((item) => (
          <option key={item.id} value={item.id}>
            {item.label}
          </option>
        ))}
      </select>
    </label>
  );
}

type ProjectPanelProps = {
  project: DesktopShellProps["project"];
  selectedProjectId: string | null;
  selection: DesktopShellProps["selection"];
  onSelectProject: (projectId: string, initiator: HTMLElement) => void;
};

function ProjectPanel({
  project,
  selectedProjectId,
  selection,
  onSelectProject,
}: ProjectPanelProps) {
  const panelState = resolveProjectPanelState(project);
  const projects = project.data ?? [];
  const selectedProject = projects.find((item) => item.id === selectedProjectId) ?? null;
  const selectionBelongsToProject =
    selection !== null && selectedProjectId !== null && selection.projectId === selectedProjectId;

  return (
    <section
      className="desktop-shell__project-panel"
      aria-labelledby="desktop-shell-project-heading"
      aria-busy={panelState === "loading"}
    >
      <div className="desktop-shell__section-kicker">พื้นที่ในเครื่อง</div>
      <h2 id="desktop-shell-project-heading">โครงการ</h2>

      {panelState === "loading" ? (
        <div className="desktop-shell__state-card desktop-shell__state-card--loading" role="status">
          <strong>กำลังอ่านโครงการในเครื่อง</strong>
          <p>รายการโครงการกำลังโหลดอยู่ การเลือกที่ทำงานจะพร้อมใช้เมื่ออ่านเสร็จ</p>
        </div>
      ) : null}

      {panelState === "error" ? (
        <div className="desktop-shell__state-card desktop-shell__state-card--error" role="alert">
          <strong>อ่านโครงการไม่สำเร็จ</strong>
          <p>{normalizeShellError(project.error).message}</p>
          <span className="desktop-shell__error-code">รหัส {normalizeShellError(project.error).code}</span>
        </div>
      ) : null}

      {panelState === "unavailable" ? (
        <div className="desktop-shell__state-card desktop-shell__state-card--unavailable" role="status">
          <strong>รายการโครงการยังไม่พร้อมใช้งาน</strong>
          <p>พื้นที่นี้ต้องอ่านจาก FUNG Desktop จึงจะแสดงโครงการจริงได้</p>
        </div>
      ) : null}

      {panelState === "idle" ? (
        <div className="desktop-shell__state-card" role="status">
          <strong>ยังไม่ได้อ่านรายการโครงการ</strong>
          <p>รอข้อมูลจากเจ้าของพื้นที่ทำงาน</p>
        </div>
      ) : null}

      {panelState === "empty" ? (
        <div className="desktop-shell__state-card" role="status">
          <strong>ยังไม่มีโครงการในเครื่อง</strong>
          <p>เริ่มจากประชุมสดหรือนำเข้าไฟล์เพื่อสร้างงานในพื้นที่ของคุณ</p>
        </div>
      ) : null}

      {panelState === "ready" ? (
        <div className="desktop-shell__project-controls">
          <label htmlFor="desktop-shell-project-select">โครงการที่เลือก</label>
          <select
            id="desktop-shell-project-select"
            value={selectedProjectId ?? ""}
            aria-describedby="desktop-shell-project-help"
            onChange={(event) => {
              if (event.target.value) onSelectProject(event.target.value, event.currentTarget);
            }}
          >
            <option value="" disabled>
              เลือกโครงการ
            </option>
            {projects.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
          <p id="desktop-shell-project-help" className="desktop-shell__field-help">
            {selectedProject ? selectedProject.name : "ยังไม่ได้เลือกโครงการ"}
          </p>
        </div>
      ) : null}

      <div className="desktop-shell__recording-context" aria-live="polite">
        <span>บันทึกที่เลือก</span>
        <strong>
          {selection === null
            ? "ยังไม่ได้เลือกบันทึก"
            : selectionBelongsToProject
              ? "เลือกบันทึกไว้แล้ว"
              : "รอจับคู่กับโครงการที่เลือก"}
        </strong>
        <p>
          {selection === null
            ? "การเลือกโครงการไม่เริ่มบันทึกหรือเล่นเสียงโดยอัตโนมัติ"
            : selectionBelongsToProject
              ? "รายละเอียดและการกระทำจะใช้คู่โครงการกับบันทึกเดียวกัน"
              : "ข้อมูลบันทึกจะไม่ถูกสรุปแทนโครงการอื่น"}
        </p>
      </div>
    </section>
  );
}

type HomeActionsProps = {
  onOpenLive: (initiator: HTMLElement) => void;
  onImport: () => void;
};

function HomeActions({ onOpenLive, onImport }: HomeActionsProps) {
  return (
    <section className="desktop-shell__home-actions" aria-labelledby="desktop-shell-home-action-heading">
      <div>
        <div className="desktop-shell__section-kicker">เริ่มจากเสียง แล้วกลับมาทบทวน</div>
        <h2 id="desktop-shell-home-action-heading">พื้นที่ส่วนตัวสำหรับการฟังและทบทวน</h2>
        <p>
          เลือกโครงการก่อน แล้วเปิดพื้นที่ประชุมสดเพื่อเริ่มขั้นตอนของคุณ การกดปุ่มนี้ยังไม่เริ่มบันทึกเสียง
        </p>
      </div>
      <div className="desktop-shell__button-row">
        <button
          className="desktop-shell__button desktop-shell__button--primary"
          type="button"
          onClick={(event) => onOpenLive(event.currentTarget)}
          aria-describedby="desktop-shell-live-action-help"
        >
          เริ่มประชุม
        </button>
        <button
          className="desktop-shell__button desktop-shell__button--secondary"
          type="button"
          onClick={onImport}
        >
          นำเข้าไฟล์เสียง/วิดีโอ
        </button>
      </div>
      <p id="desktop-shell-live-action-help" className="desktop-shell__button-help">
        จะเปิดประชุมสดก่อนเริ่มการบันทึกจริง
      </p>
    </section>
  );
}

const SURFACE_COPY: Record<DesktopSurface, { kicker: string; title: string; description: string }> = {
  home: {
    kicker: "หน้าหลัก",
    title: "พื้นที่ส่วนตัวสำหรับการฟังและทบทวน",
    description: "ข้อมูลหลักยังอยู่บนอุปกรณ์ของคุณ และทุกหน้าจะบอกขอบเขตของข้อมูลที่อ่านได้",
  },
  live: {
    kicker: "ประชุมสด",
    title: "เปิดพื้นที่ประชุมสด",
    description: "ตรวจดูสถานะและการทำงานของการบันทึกจากเจ้าของเซสชันเดิม",
  },
  review: {
    kicker: "บันทึกย้อนหลัง / ทบทวน",
    title: "ทบทวนบันทึกที่เลือก",
    description: "อ่านเนื้อหาและการกระทำของบันทึกที่จับคู่กับโครงการนี้เท่านั้น",
  },
};

type SurfaceIntroProps = {
  activeSurface: DesktopSurface;
  scopeChoice: DesktopShellProps["scopeChoice"];
  headingRef: MutableRefObject<HTMLHeadingElement | null>;
};

function SurfaceIntro({ activeSurface, scopeChoice, headingRef }: SurfaceIntroProps) {
  const copy = SURFACE_COPY[activeSurface];
  const scopeLabel =
    scopeChoice === "B" ? "ประวัติและการทบทวนตามบันทึก" : "การทบทวนบันทึกปัจจุบัน";

  return (
    <header className="desktop-shell__surface-intro">
      <div className="desktop-shell__section-kicker">{copy.kicker}</div>
      <h1 ref={headingRef} id="desktop-shell-main-heading" tabIndex={-1}>
        {copy.title}
      </h1>
      <p>{copy.description}</p>
      <span className="desktop-shell__scope-note">ขอบเขต: {scopeLabel}</span>
    </header>
  );
}

type CaptureStripProps = {
  lifecycle: CaptureLifecycle;
  stopState: "idle" | "stopping" | "complete" | "error";
  stopError: ReviewError | null;
  onShowLive: () => void;
  onStop: () => void;
};

function CaptureStrip({ lifecycle, stopState, stopError, onShowLive, onStop }: CaptureStripProps) {
  const isStopping = lifecycle === "stopping" || stopState === "stopping";
  const isStarting = lifecycle === "starting";
  const isComplete = stopState === "complete";
  const title = isStopping
    ? "กำลังยืนยันการหยุดบันทึก"
    : isStarting
      ? "กำลังเตรียมการบันทึก"
      : "ยังบันทึกเสียงอยู่";
  const detail = isStopping
    ? "รอข้อมูลจาก Desktop ให้ active=false และ stopping=false ก่อนถือว่าหยุดแล้ว"
    : isStarting
      ? "การเริ่มต้นยังไม่เสร็จ หลีกเลี่ยงการเริ่มซ้ำ"
      : isComplete
        ? "ได้รับการยืนยันการหยุดแล้ว รอหน้าหลักรับสถานะล่าสุด"
        : "สามารถเปิดประชุมสดหรือหยุดจากแถบนี้ได้ทุกหน้า";

  return (
    <aside
      className="desktop-shell__capture-strip"
      aria-label="สถานะการบันทึก"
      aria-live="polite"
      aria-busy={isStarting || isStopping}
    >
      <span className="desktop-shell__capture-dot" aria-hidden="true" />
      <div className="desktop-shell__capture-copy">
        <strong>{title}</strong>
        <span>{detail}</span>
        {stopError ? (
          <span className="desktop-shell__capture-error" role="alert">
            หยุดบันทึกไม่สำเร็จ: {stopError.message} (รหัส {stopError.code})
          </span>
        ) : null}
      </div>
      <div className="desktop-shell__capture-actions">
        <button className="desktop-shell__button desktop-shell__button--quiet" type="button" onClick={onShowLive}>
          กลับไปประชุมสด
        </button>
        <button
          className="desktop-shell__button desktop-shell__button--danger"
          type="button"
          onClick={onStop}
          disabled={isStopping || isComplete}
          aria-describedby={stopError ? "desktop-shell-capture-stop-error" : undefined}
        >
          {isStopping ? "กำลังหยุด..." : isComplete ? "หยุดแล้ว" : "หยุดบันทึก"}
        </button>
      </div>
      {stopError ? <span id="desktop-shell-capture-stop-error" className="desktop-shell__sr-only">{stopError.message}</span> : null}
    </aside>
  );
}

type LiveStatusNoticeProps = {
  liveStatus: DesktopShellProps["liveStatus"];
};

function LiveStatusNotice({ liveStatus }: LiveStatusNoticeProps) {
  if (liveStatus.status !== "error" && liveStatus.status !== "unavailable") return null;
  const error = normalizeShellError(liveStatus.error);
  return (
    <div className="desktop-shell__state-card desktop-shell__state-card--error" role="alert">
      <strong>
        {liveStatus.status === "unavailable" ? "สถานะการบันทึกยังไม่พร้อมอ่าน" : "อ่านสถานะการบันทึกไม่สำเร็จ"}
      </strong>
      <p>{error.message}</p>
      <span className="desktop-shell__error-code">รหัส {error.code}</span>
    </div>
  );
}

type NavigationGuardDialogProps = {
  dialogRef: MutableRefObject<HTMLDivElement | null>;
  destinationLabel: string;
  busy: boolean;
  error: ReviewError | null;
  onKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
  onContinue: () => void;
  onStop: () => void;
  onStay: () => void;
};

export function NavigationGuardDialog({
  dialogRef,
  destinationLabel,
  busy,
  error,
  onKeyDown,
  onContinue,
  onStop,
  onStay,
}: NavigationGuardDialogProps) {
  return (
    <div className="desktop-shell__modal-layer">
      <div className="desktop-shell__modal-backdrop" aria-hidden="true" />
      <div
        ref={dialogRef}
        className="desktop-shell__dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="desktop-shell-guard-title"
        aria-describedby="desktop-shell-guard-description"
        aria-busy={busy}
        onKeyDown={onKeyDown}
      >
        <div className="desktop-shell__section-kicker">ต้องยืนยันก่อนออก</div>
        <h2 id="desktop-shell-guard-title">ยังบันทึกเสียงอยู่</h2>
        <p id="desktop-shell-guard-description">
          ปลายทางคือ “{destinationLabel}” เลือกว่าจะบันทึกต่อ หยุดให้เรียบร้อยก่อนออก หรืออยู่หน้านี้
        </p>
        {error ? (
          <div className="desktop-shell__dialog-error" role="alert">
            <strong>หยุดบันทึกยังไม่สำเร็จ</strong>
            <span>{error.message}</span>
            <small>เซสชันยังอยู่หน้านี้ คุณลองใหม่หรือเลือกอยู่หน้านี้ได้</small>
          </div>
        ) : null}
        <div className="desktop-shell__dialog-actions">
          <button
            className="desktop-shell__button desktop-shell__button--secondary"
            type="button"
            data-guard-default="true"
            onClick={onStay}
          >
            อยู่หน้านี้
          </button>
          <button
            className="desktop-shell__button desktop-shell__button--quiet"
            type="button"
            onClick={onContinue}
            disabled={busy}
          >
            อัดต่อและออกจากหน้านี้
          </button>
          <button
            className="desktop-shell__button desktop-shell__button--danger"
            type="button"
            onClick={onStop}
            disabled={busy}
          >
            {busy ? "กำลังยืนยันการหยุด..." : "หยุดแล้วออก"}
          </button>
        </div>
        <p className="desktop-shell__dialog-help">Esc จะอยู่หน้านี้และคืนโฟกัสให้ปุ่มเดิม</p>
      </div>
    </div>
  );
}

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), select:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.hasAttribute("aria-hidden"));
}

export function DesktopShell({
  scopeChoice,
  project,
  selectedProjectId,
  selection,
  activeSurface,
  liveStatus,
  livePhase = "idle",
  theme,
  mainContent,
  settingsSlot,
  pairingSlot,
  recoverySlot,
  actions,
}: DesktopShellProps) {
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const guardGenerationRef = useRef(0);
  const [pendingNavigation, setPendingNavigation] = useState<NavigationIntent | null>(null);
  const [guardBusy, setGuardBusy] = useState(false);
  const [guardError, setGuardError] = useState<ReviewError | null>(null);
  const [actionError, setActionError] = useState<ReviewError | null>(null);
  const [stripStopState, setStripStopState] = useState<"idle" | "stopping" | "complete" | "error">("idle");
  const [stripStopError, setStripStopError] = useState<ReviewError | null>(null);
  const captureLifecycle = getCaptureLifecycle(liveStatus, livePhase);

  useEffect(() => {
    headingRef.current?.focus();
  }, [activeSurface]);

  useEffect(() => {
    if (captureLifecycle === "inactive") {
      setStripStopState("idle");
      setStripStopError(null);
    }
  }, [captureLifecycle]);

  useEffect(() => {
    if (pendingNavigation) {
      const defaultButton = dialogRef.current?.querySelector<HTMLElement>("[data-guard-default='true']");
      defaultButton?.focus();
      return;
    }

    const restoreTarget = restoreFocusRef.current;
    restoreFocusRef.current = null;
    if (restoreTarget?.isConnected) restoreTarget.focus();
  }, [pendingNavigation]);

  const runAction = async (action: () => void | Promise<void>) => {
    setActionError(null);
    try {
      await action();
    } catch (error) {
      setActionError(normalizeShellError(error));
    }
  };

  const runNavigation = (intent: NavigationIntent) => {
    void runAction(intent.action);
  };

  const requestNavigation = (intent: NavigationIntent, initiator: HTMLElement) => {
    if (!shouldGuardNavigation(captureLifecycle, intent.leavesCaptureContext)) {
      runNavigation(intent);
      return;
    }

    guardGenerationRef.current += 1;
    restoreFocusRef.current = initiator;
    setGuardError(null);
    setGuardBusy(false);
    setPendingNavigation(intent);
  };

  const requestSurface = (surface: DesktopSurface, initiator: HTMLElement) => {
    const surfaceAction =
      surface === "home" ? actions.showHome : surface === "live" ? actions.showLive : actions.showReview;
    requestNavigation(
      {
        label: SURFACE_COPY[surface].kicker,
        leavesCaptureContext: surface !== "live",
        action: surfaceAction,
      },
      initiator,
    );
  };

  const requestProject = (projectId: string, initiator: HTMLElement) => {
    if (projectId === selectedProjectId) return;
    requestNavigation(
      {
        label: "โครงการใหม่",
        leavesCaptureContext: true,
        action: () => actions.selectProject(projectId),
      },
      initiator,
    );
  };

  const requestSettings = (initiator: HTMLElement) => {
    requestNavigation(
      { label: "ตั้งค่า", leavesCaptureContext: true, action: actions.openSettings },
      initiator,
    );
  };

  const requestPairing = (initiator: HTMLElement) => {
    requestNavigation(
      { label: "จับคู่อุปกรณ์", leavesCaptureContext: true, action: actions.openPairing },
      initiator,
    );
  };

  const closeGuard = () => {
    guardGenerationRef.current += 1;
    setGuardBusy(false);
    setGuardError(null);
    setPendingNavigation(null);
  };

  const handleContinue = () => {
    const intent = pendingNavigation;
    if (!intent) return;
    closeGuard();
    runNavigation(intent);
  };

  const handleStopAndLeave = async () => {
    const intent = pendingNavigation;
    if (!intent || guardBusy) return;
    const generation = guardGenerationRef.current;
    setGuardBusy(true);
    setGuardError(null);
    try {
      const stoppedStatus = await actions.stopAndLeave();
      if (generation !== guardGenerationRef.current) return;
      const stopError = resolveStopAndLeaveError(stoppedStatus);
      if (stopError) {
        setGuardError(stopError);
        return;
      }
      guardGenerationRef.current += 1;
      setPendingNavigation(null);
      runNavigation(intent);
    } catch (error) {
      if (generation === guardGenerationRef.current) setGuardError(normalizeShellError(error));
    } finally {
      if (generation === guardGenerationRef.current) setGuardBusy(false);
    }
  };

  const handleGuardKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeGuard();
      return;
    }
    if (event.key !== "Tab" || !dialogRef.current) return;
    const focusable = getFocusableElements(dialogRef.current);
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  const handleStripStop = async () => {
    if (stripStopState === "stopping" || stripStopState === "complete") return;
    setStripStopState("stopping");
    setStripStopError(null);
    try {
      const stoppedStatus = await actions.stopAndLeave();
      const stopError = resolveStopAndLeaveError(stoppedStatus);
      if (stopError) {
        setStripStopState("error");
        setStripStopError(stopError);
        return;
      }
      setStripStopState("complete");
    } catch (error) {
      setStripStopState("error");
      setStripStopError(normalizeShellError(error));
    }
  };

  const themeClass = theme === "dark" ? "desktop-shell--dark" : theme === "light" ? "desktop-shell--light" : "desktop-shell--system";
  const surfaceCopy = SURFACE_COPY[activeSurface];

  return (
    <div className={`desktop-shell ${themeClass}`} data-theme={theme} data-surface={activeSurface}>
      <a className="desktop-shell__skip-link" href="#desktop-shell-main">
        ข้ามไปยังเนื้อหาหลัก
      </a>

      <header className="desktop-shell__header" data-tauri-drag-region>
        <div className="desktop-shell__brand" role="img" aria-label="FUNG Quiet Archive">
          <div className="desktop-shell__brand-lockup">
            {FUNG_BRAND_MARK}
            <span className="desktop-shell__brand-wordmark" aria-hidden="true">
              FUNG
            </span>
          </div>
          <span className="desktop-shell__brand-note">QUIET ARCHIVE</span>
        </div>
        <SurfaceNavigation activeSurface={activeSurface} onNavigate={requestSurface} />
        <div className="desktop-shell__header-actions">
          <ThemeChoice theme={theme} onChange={actions.setTheme} />
          <button
            className="desktop-shell__header-button"
            type="button"
            onClick={(event) => requestSettings(event.currentTarget)}
          >
            ตั้งค่า
          </button>
          <button
            className="desktop-shell__header-button"
            type="button"
            onClick={(event) => requestPairing(event.currentTarget)}
          >
            จับคู่อุปกรณ์
          </button>
          <button
            className="desktop-shell__header-button"
            type="button"
            aria-label="ย่อหน้าต่าง"
            title="ย่อหน้าต่าง"
            onClick={() => void runAction(actions.minimizeWindow)}
          >
            ย่อ
          </button>
          <button
            className="desktop-shell__header-button desktop-shell__header-button--danger"
            type="button"
            aria-label="ปิดหน้าต่าง"
            title="ปิดหน้าต่าง"
            onClick={() => void runAction(actions.closeWindow)}
          >
            ปิด
          </button>
        </div>
      </header>

      {captureLifecycle !== "inactive" ? (
        <CaptureStrip
          lifecycle={captureLifecycle}
          stopState={stripStopState}
          stopError={stripStopError}
          onShowLive={() => requestSurface("live", headingRef.current ?? document.body)}
          onStop={() => void handleStripStop()}
        />
      ) : null}

      <div className="desktop-shell__body">
        <aside className="desktop-shell__sidebar" aria-label="บริบทงาน">
          <ProjectPanel
            project={project}
            selectedProjectId={selectedProjectId}
            selection={selection}
            onSelectProject={requestProject}
          />
          <div className="desktop-shell__sidebar-note">
            <span>สถานะพื้นที่</span>
            <strong>{captureLifecycle === "inactive" ? "พร้อมตรวจสอบ" : "มีเซสชันบันทึกอยู่"}</strong>
            <p>การเปลี่ยนธีมไม่เปลี่ยนโครงการหรือสถานะการบันทึก</p>
          </div>
          {recoverySlot ? <div className="desktop-shell__owned-slot">{recoverySlot}</div> : null}
        </aside>

        <main id="desktop-shell-main" className="desktop-shell__main" tabIndex={-1}>
          <SurfaceIntro activeSurface={activeSurface} scopeChoice={scopeChoice} headingRef={headingRef} />
          {actionError ? (
            <div className="desktop-shell__action-error" role="alert">
              <strong>ทำรายการไม่สำเร็จ</strong>
              <span>{actionError.message}</span>
              <small>รหัส {actionError.code}</small>
            </div>
          ) : null}
          <LiveStatusNotice liveStatus={liveStatus} />
          {activeSurface === "home" ? (
            <HomeActions
              onOpenLive={(initiator) => requestSurface("live", initiator)}
              onImport={() => void runAction(actions.importMedia)}
            />
          ) : null}
          <section className="desktop-shell__legacy-workspace" aria-label={`พื้นที่ทำงานเดิม: ${surfaceCopy.kicker}`}>
            <div className="desktop-shell__legacy-heading">
              <span>พื้นที่ทำงานเดิม</span>
              <small>{surfaceCopy.kicker}</small>
            </div>
            <div className="desktop-shell__legacy-content">{mainContent}</div>
          </section>
        </main>
      </div>

      {settingsSlot ? <div className="desktop-shell__owned-slot desktop-shell__owned-slot--settings">{settingsSlot}</div> : null}
      {pairingSlot ? <div className="desktop-shell__owned-slot desktop-shell__owned-slot--pairing">{pairingSlot}</div> : null}

      {pendingNavigation ? (
        <NavigationGuardDialog
          dialogRef={dialogRef}
          destinationLabel={pendingNavigation.label}
          busy={guardBusy}
          error={guardError}
          onKeyDown={handleGuardKeyDown}
          onContinue={handleContinue}
          onStop={() => void handleStopAndLeave()}
          onStay={closeGuard}
        />
      ) : null}
    </div>
  );
}

export default DesktopShell;
