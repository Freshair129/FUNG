import { lazy, Suspense, useState } from "react";
import { Activity, Cloud, Link2, SlidersHorizontal, UserCircle, Volume2, X } from "lucide-react";
import type { InvokeFn } from "../lib/backupFlow";
import { startLocalApi, type Job, type LocalApiInfo } from "../tauri";
import { supabaseConfigured } from "../lib/bootstrap";
import "./SettingsPanel.css";
// Eagerly imported (not lazy) because the "Supabase not configured" fallback
// below reuses these class names directly, and that fallback can render
// without ever mounting the lazy AccountLoginPanel that normally pulls this
// stylesheet in — mirrors the same eager import in App.tsx.
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
  { id: "account", label: "External Connections", icon: SlidersHorizontal },
  { id: "account-login", label: "Sign In and Backup", icon: UserCircle },
  { id: "tts", label: "TTS Providers", icon: Volume2 },
  { id: "cloud", label: "Cloud Providers", icon: Cloud },
  { id: "fetch", label: "Fetch from URL", icon: Link2 },
  { id: "zoom", label: "Zoom import", icon: Cloud },
  { id: "runtime", label: "Runtime", icon: Activity },
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

  const copyConnectUrl = async () => {
    if (!localApi?.connectUrl) return;
    try {
      await navigator.clipboard.writeText(localApi.connectUrl);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard denied: the URL is still shown in the field to select by hand.
    }
  };

  return (
    <div className="settings-overlay" role="presentation" onClick={onClose}>
      <div className="settings-stack" onClick={(event) => event.stopPropagation()}>
        <header className="settings-header">
          <h2>Settings</h2>
          <button type="button" className="settings-close" onClick={onClose} aria-label="Close settings">
            <X size={18} />
          </button>
        </header>
        <nav className="settings-tabs" aria-label="Settings sections">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              className={`settings-tab ${activeTab === tab.id ? "is-active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              <tab.icon size={16} />
              {tab.label}
            </button>
          ))}
        </nav>
        <div className="settings-body">
          <Suspense fallback={<p className="settings-loading">Loading…</p>}>
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
              </div>
            )}
          </Suspense>
        </div>
      </div>
    </div>
  );
}
