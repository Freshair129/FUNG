import { Mic, Settings2, Sparkles, X } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { FungLogo } from "../FungLogo";
import type { CaptureLifecycle } from "./DesktopShell";
import type {
  DesktopSurface,
  MaterialChoice,
  RecordingKey,
  ThemeChoice,
  TransparencyChoice,
} from "./contracts";
import "./CompanionPanel.css";

type CompanionPanelProps = {
  activeSurface: DesktopSurface;
  captureLifecycle: CaptureLifecycle;
  selection: RecordingKey | null;
  theme: ThemeChoice;
  material: MaterialChoice;
  transparency: TransparencyChoice;
  onClose: () => void;
  onNavigate: (surface: DesktopSurface, initiator: HTMLElement) => void;
  onOpenSettings: (initiator: HTMLElement) => void;
};

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  );
}

function resolveMode(activeSurface: DesktopSurface, captureLifecycle: CaptureLifecycle): string {
  if (captureLifecycle !== "inactive") return "PEEK";
  if (activeSurface === "review") return "CONVERSATION";
  return "IDLE";
}

export function CompanionPanel({
  activeSurface,
  captureLifecycle,
  selection,
  theme,
  material,
  transparency,
  onClose,
  onNavigate,
  onOpenSettings,
}: CompanionPanelProps) {
  const panelRef = useRef<HTMLElement | null>(null);
  const closeRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    closeRef.current?.focus();
  }, []);

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key !== "Tab" || !panelRef.current) return;
    const focusable = getFocusableElements(panelRef.current);
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

  const statusTitle = captureLifecycle === "inactive" ? "พร้อมอยู่กับคุณ" : "กำลังบันทึกอยู่";
  const statusCopy = selection
    ? "มีการเลือกบันทึกไว้แล้ว ข้อมูลจะยังอยู่ในขอบเขตเดิม"
    : "ยังไม่ได้เลือกบันทึกใหม่ การเปิด Companion ไม่เริ่มการอัดเสียง";

  return (
    <div className="companion-layer" role="presentation" onClick={onClose}>
      <div className="companion-layer__backdrop" aria-hidden="true" />
      <aside
        ref={panelRef}
        className="companion-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="companion-panel-title"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <header className="companion-panel__header">
          <FungLogo size={30} variant="porcelain" showWordmark />
          <button ref={closeRef} type="button" className="companion-panel__close" onClick={onClose} aria-label="ปิด Companion">
            <X size={18} />
          </button>
        </header>

        <div className="companion-panel__tabs" aria-label="Companion state">
          <span className="is-active">{resolveMode(activeSurface, captureLifecycle)}</span>
          <span>IN-APP</span>
          <span>{theme.toUpperCase()}</span>
        </div>

        <section className="companion-panel__body">
          <p className="companion-panel__eyebrow">Quiet companion · {resolveMode(activeSurface, captureLifecycle)}</p>
          <h2 id="companion-panel-title">อยู่กับคุณเสมอ</h2>
          <p className="companion-panel__intro">
            ช่วยเปิดพื้นที่ทำงานและอ่านสถานะตามจริง โดยไม่สร้างคำสั่งหรือผลลัพธ์ใหม่ที่ยังไม่มี handler
          </p>

          <div className="companion-panel__status" role="status" aria-live="polite">
            <span className="companion-panel__status-icon" aria-hidden="true">
              <Mic size={18} />
            </span>
            <div>
              <strong>{statusTitle}</strong>
              <span>{statusCopy}</span>
            </div>
          </div>

          <div className="companion-panel__actions">
            <button
              type="button"
              className="companion-panel__action companion-panel__action--primary"
              onClick={(event) => onNavigate("live", event.currentTarget)}
            >
              <Mic size={18} />
              <span>เปิดประชุมสด</span>
              <small>เริ่มจาก handler เดิม</small>
            </button>
            <button
              type="button"
              className="companion-panel__action"
              onClick={(event) => onOpenSettings(event.currentTarget)}
            >
              <Settings2 size={18} />
              <span>เปิดตั้งค่า</span>
              <small>ธีมและการเชื่อมต่อ</small>
            </button>
          </div>

          <div className="companion-panel__disclosure">
            <Sparkles size={15} aria-hidden="true" />
            <span>วัสดุ {material} · transparency {transparency} · ไม่ใช่หน้าต่างลอยข้ามแอป</span>
          </div>
        </section>

        <footer className="companion-panel__footer">
          <span>Local-first</span>
          <span>ข้อมูลยังอยู่ใน FUNG Desktop</span>
        </footer>
      </aside>
    </div>
  );
}
