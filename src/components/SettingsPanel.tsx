import { lazy, Suspense, useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { Activity, Cloud, Link2, LockKeyhole, SlidersHorizontal, UserCircle, Volume2, X } from "lucide-react";
import * as QRCode from "qrcode";
import type { InvokeFn } from "../lib/backupFlow";
import { setLocalApiLan, startLocalApi, type Job, type LocalApiInfo } from "../tauri";
import { supabaseConfigured } from "../lib/bootstrap";
import { FungLogo } from "./FungLogo";
import "./SettingsPanel.css";
// Eagerly imported (not lazy) because the "Supabase not configured" fallback
// below reuses these class names directly, and that fallback can render
// without ever mounting the lazy AccountLoginPanel that normally pulls this
// stylesheet in. DevicePairingPanel also loads it for its own overlay.
import "./AccountLoginPanel.css";

const ExternalAccountPanel = lazy(() =>
  import("./ExternalAccountPanel").then((m) => ({ default: m.ExternalAccountPanel })),
);
const TtsProviderPanel = lazy(() =>
  import("./TtsProviderPanel").then((m) => ({ default: m.TtsProviderPanel })),
);
const CloudProvidersPanel = lazy(() =>
  import("./CloudProvidersPanel").then((m) => ({ default: m.CloudProvidersPanel })),
);
const MediaFetchPanel = lazy(() =>
  import("./MediaFetchPanel").then((m) => ({ default: m.MediaFetchPanel })),
);
const ZoomPanel = lazy(() => import("./ZoomPanel").then((m) => ({ default: m.ZoomPanel })));
const AccountLoginPanel = lazy(() =>
  import("./AccountLoginPanel").then((m) => ({ default: m.AccountLoginPanel })),
);
const BackupPanel = lazy(() => import("./BackupPanel").then((m) => ({ default: m.BackupPanel })));

export type SettingsTab = "account" | "tts" | "cloud" | "fetch" | "zoom" | "runtime" | "account-login";

const TABS: { id: SettingsTab; label: string; icon: typeof SlidersHorizontal }[] = [
  { id: "account", label: "การเชื่อมต่อ", icon: SlidersHorizontal },
  { id: "account-login", label: "บัญชีและสำรองข้อมูล", icon: UserCircle },
  { id: "tts", label: "เสียงพูด", icon: Volume2 },
  { id: "cloud", label: "ผู้ให้บริการคลาวด์", icon: Cloud },
  { id: "fetch", label: "นำเข้าจากลิงก์", icon: Link2 },
  { id: "zoom", label: "นำเข้า Zoom", icon: Cloud },
  { id: "runtime", label: "ระบบและประสิทธิภาพ", icon: Activity },
];

interface SettingsPanelProps {
  onClose: () => void;
  invoke: InvokeFn | null;
  projectId: string | null;
  onStartApi: () => void;
  apiRunning: boolean;
  onFetchStarted: (job: Job) => void;
  onOpenExternalAccountPortal: () => Promise<void>;
  initialTab?: SettingsTab;
}

/**
 * Copies `text`, preferring the async clipboard API and falling back to
 * selecting `field` and issuing the legacy copy command, which WebView2
 * honours under a user gesture even when `navigator.clipboard` is refused.
 * Returns whether either path reported success.
 */
async function copyText(text: string, field: HTMLInputElement | null): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // fall through to the selection path
  }
  if (!field) return false;
  try {
    field.focus();
    field.select();
    return document.execCommand("copy");
  } catch {
    return false;
  }
}

export function SettingsPanel({
  onClose,
  invoke,
  projectId,
  onStartApi,
  apiRunning,
  onFetchStarted,
  onOpenExternalAccountPortal,
  initialTab,
}: SettingsPanelProps) {
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const closeRef = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
    const initiator = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeRef.current?.focus();
    return () => {
      const target = initiator?.isConnected && initiator !== document.body && initiator.getClientRects().length > 0
        ? initiator : document.querySelector<HTMLElement>('button[aria-label="ตั้งค่า"]');
      target?.focus();
    };
  }, []);

  const handleDialogKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.defaultPrevented) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab" || !dialogRef.current) return;
    const elements = Array.from(dialogRef.current.querySelectorAll<HTMLElement>(
      'button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    )).filter((element) => element.getClientRects().length > 0);
    const first = elements[0];
    const last = elements[elements.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault(); last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first?.focus();
    }
  };
  const [activeTab, setActiveTab] = useState<SettingsTab>(initialTab ?? "account");
  const [localApi, setLocalApi] = useState<LocalApiInfo | null>(null);
  const [localApiError, setLocalApiError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Idempotent on the Rust side: re-reading the URL never rotates the token,
  // so a web tab that already pasted it keeps working.
  const revealConnectUrl = async () => {
    setLocalApiError(null);
    try {
      setLocalApi(await startLocalApi());
    } catch (error) {
      setLocalApiError(error instanceof Error ? error.message : String(error));
    }
  };

  const connectUrlField = useRef<HTMLInputElement | null>(null);

  const handleTabKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>, currentTab: SettingsTab) => {
    const index = TABS.findIndex((tab) => tab.id === currentTab);
    const nextIndex = event.key === "ArrowDown" || event.key === "ArrowRight"
      ? (index + 1) % TABS.length
      : event.key === "ArrowUp" || event.key === "ArrowLeft"
        ? (index - 1 + TABS.length) % TABS.length
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? TABS.length - 1
            : -1;
    if (nextIndex < 0) return;
    event.preventDefault();
    const nextTab = TABS[nextIndex];
    if (!nextTab) return;
    setActiveTab(nextTab.id);
    document.getElementById(`settings-tab-${nextTab.id}`)?.focus();
  };

  const copyConnectUrl = async () => {
    if (!localApi?.connectUrl) return;
    setLocalApiError(null);
    const copied = await copyText(localApi.connectUrl, connectUrlField.current);
    if (copied) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } else {
      // Say so instead of leaving the button looking unpressed: on the
      // owner's machine the async clipboard API was refused inside the
      // webview and the user waited on a copy that never happened.
      setLocalApiError("คัดลอกอัตโนมัติไม่ได้ — คลิกในช่องลิงก์ (จะเลือกทั้งหมดให้) แล้วกด Ctrl+C");
    }
  };

  const [lanBusy, setLanBusy] = useState(false);
  const [lanQr, setLanQr] = useState<string | null>(null);

  // Opt-in LAN sharing: the desktop serves the phone page itself, so the
  // phone never needs the cloud web or a certificate — it scans this and
  // talks to the desktop directly on the same Wi-Fi.
  const toggleLan = async () => {
    setLanBusy(true);
    setLocalApiError(null);
    try {
      setLocalApi(await setLocalApiLan(!(localApi?.lanEnabled ?? false)));
    } catch (error) {
      setLocalApiError(error instanceof Error ? error.message : String(error));
    } finally {
      setLanBusy(false);
    }
  };

  useEffect(() => {
    const url = localApi?.lanUrl ?? null;
    if (!url) {
      setLanQr(null);
      return;
    }
    let cancelled = false;
    QRCode.toDataURL(url, { margin: 1, width: 240, errorCorrectionLevel: "M" })
      .then((dataUrl) => {
        if (!cancelled) setLanQr(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setLanQr(null);
      });
    return () => {
      cancelled = true;
    };
  }, [localApi?.lanUrl]);

  return (
    <div className="settings-overlay" role="presentation" onClick={onClose}>
      <section className="settings-drawer" ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="settings-panel-heading" onKeyDown={handleDialogKeyDown} onClick={(event) => event.stopPropagation()}>
        <header className="settings-drawer__header">
          <div className="settings-drawer__heading-group">
            <div className="settings-drawer__brand" role="img" aria-label="FUNG Quiet Archive">
              <FungLogo size={28} variant="dark" showWordmark wordmarkColor="var(--fung-text-primary)" />
              <span>QUIET ARCHIVE</span>
            </div>
            <div className="settings-drawer__title-group">
              <h2 id="settings-panel-heading">ตั้งค่า</h2>
              <p>ปรับแต่งการทำงานของ FUNG ให้เหมาะกับคุณ</p>
            </div>
          </div>
          <button ref={closeRef} type="button" className="settings-drawer__close" onClick={onClose} aria-label="ปิดการตั้งค่า">
            <X size={18} />
          </button>
        </header>
        <div className="settings-drawer__layout">
          <nav className="settings-drawer__tabs" role="tablist" aria-orientation="vertical" aria-label="หมวดการตั้งค่า">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                id={`settings-tab-${tab.id}`}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.id}
                aria-controls="settings-content"
                tabIndex={activeTab === tab.id ? 0 : -1}
                className={`settings-drawer__tab${activeTab === tab.id ? " is-active" : ""}`}
                onKeyDown={(event) => handleTabKeyDown(event, tab.id)}
                onClick={() => setActiveTab(tab.id)}
              >
                <tab.icon size={18} aria-hidden="true" />
                <span>{tab.label}</span>
              </button>
            ))}
          </nav>
          <div
            id="settings-content"
            className="settings-drawer__body"
            role="tabpanel"
            aria-labelledby={`settings-tab-${activeTab}`}
            tabIndex={0}
          >
            <Suspense fallback={<p className="settings-loading">กำลังโหลดการตั้งค่า…</p>}>
              {activeTab === "account" && <ExternalAccountPanel onClose={onClose} onOpenPortal={onOpenExternalAccountPortal} embedded />}
              {activeTab === "tts" && <TtsProviderPanel onClose={onClose} embedded />}
              {activeTab === "cloud" && <CloudProvidersPanel onClose={onClose} embedded />}
              {activeTab === "fetch" && (
                <MediaFetchPanel projectId={projectId} onClose={onClose} onStarted={onFetchStarted} embedded />
              )}
              {activeTab === "zoom" && <ZoomPanel onClose={onClose} embedded />}
              {activeTab === "account-login" && (
                <div className="settings-account-login">
                  {supabaseConfigured ? (
                    <AccountLoginPanel onClose={onClose} />
                  ) : (
                    <section className="account-login-panel" aria-label="บัญชี FUNG ยังไม่พร้อมใช้งาน">
                      <header className="account-login-header">
                        <UserCircle size={18} />
                        <h3>บัญชี &amp; อุปกรณ์</h3>
                      </header>
                      <p className="account-login-error">
                        ยังไม่ได้ตั้งค่า Supabase — เพิ่ม VITE_SUPABASE_URL และ VITE_SUPABASE_ANON_KEY เพื่อเปิดใช้บัญชีและการจับคู่อุปกรณ์
                      </p>
                    </section>
                  )}
                  <BackupPanel invoke={invoke} projectId={projectId} />
                </div>
              )}
              {activeTab === "runtime" && (
                <div className="settings-runtime">
                  <p>Local API runtime status: <strong>{apiRunning ? "Running" : "Stopped"}</strong></p>
                  <button type="button" className="settings-runtime-start" onClick={onStartApi} disabled={apiRunning}>
                    Start local API
                  </button>
                  <section className="settings-local-api" aria-label="ลิงก์เชื่อมต่อเว็บ">
                    <h4>ฟังไฟล์ที่อัดไว้จากหน้าเว็บ (เครื่องนี้)</h4>
                    <p className="settings-local-api-note">
                      หน้าเว็บ FUNG ที่เปิดบนเครื่องนี้อ่านรายการและเล่นไฟล์ตรงจากแอปนี้ผ่าน 127.0.0.1 — ไม่มีเสียงขึ้น cloud
                      ลิงก์ใช้ได้จนกว่าจะปิดแอป
                    </p>
                    {localApi?.connectUrl ? (
                      <div className="settings-local-api-row">
                        <input
                          ref={connectUrlField}
                          className="settings-local-api-url"
                          type="text"
                          readOnly
                          value={localApi.connectUrl}
                          onFocus={(event) => event.currentTarget.select()}
                          aria-label="ลิงก์เชื่อมต่อเว็บ"
                        />
                        <button type="button" className="settings-runtime-start" onClick={() => void copyConnectUrl()}>
                          {copied ? "คัดลอกแล้ว" : "คัดลอก"}
                        </button>
                      </div>
                    ) : (
                      <button type="button" className="settings-runtime-start" onClick={() => void revealConnectUrl()}>
                        แสดงลิงก์เชื่อมต่อเว็บ
                      </button>
                    )}
                    {localApiError && <p className="settings-local-api-error">{localApiError}</p>}
                  </section>
                  <section className="settings-local-api" aria-label="เปิดจากมือถือในวง Wi-Fi เดียวกัน">
                    <h4>เปิดจากมือถือในวง Wi-Fi เดียวกัน</h4>
                    <p className="settings-local-api-note">
                      เมื่อเปิด desktop จะเสิร์ฟหน้าเล่นไฟล์บนเครือข่ายภายใน ให้มือถือสแกน QR แล้วเปิดใน browser
                      ได้เลย — ไม่ผ่าน cloud ปิดเมื่อไรก็ได้ และปิดเองเมื่อปิดแอป (Windows Firewall อาจถามครั้งแรก
                      → Allow)
                    </p>
                    <button
                      type="button"
                      className="settings-runtime-start"
                      onClick={() => void toggleLan()}
                      disabled={lanBusy}
                    >
                      {localApi?.lanEnabled ? "ปิดแชร์ในวง LAN" : "แชร์ให้มือถือ (LAN)"}
                    </button>
                    {localApi?.lanEnabled && localApi.lanUrl && (
                      <div className="settings-local-api-qr">
                        {lanQr ? <img src={lanQr} alt="QR สำหรับเปิดบนมือถือ" width={240} height={240} /> : null}
                        <input
                          className="settings-local-api-url"
                          type="text"
                          readOnly
                          value={localApi.lanUrl}
                          onFocus={(event) => event.currentTarget.select()}
                          aria-label="ลิงก์สำหรับมือถือ"
                        />
                      </div>
                    )}
                    {localApi?.lanEnabled && !localApi.lanUrl && (
                      <p className="settings-local-api-error">
                        เปิดแล้วที่ {localApi.lanBind} แต่หา IP ในวง LAN ของเครื่องนี้ไม่เจอ — เช็คว่าเชื่อม Wi-Fi/LAN อยู่
                      </p>
                    )}
                  </section>
                </div>
              )}
            </Suspense>
          </div>
        </div>
        <footer className="settings-drawer__footer">
          <LockKeyhole size={17} aria-hidden="true" />
          <div>
            <strong>Local-first</strong>
            <span>ไฟล์เสียง คำถอดความ และโน้ตยังอยู่บนอุปกรณ์นี้</span>
          </div>
        </footer>
      </section>
    </div>
  );
}
