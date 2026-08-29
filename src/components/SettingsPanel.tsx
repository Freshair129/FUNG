import { lazy, Suspense, useState } from "react";
import { Activity, Cloud, Link2, SlidersHorizontal, Volume2, X } from "lucide-react";
import type { InvokeFn } from "../lib/backupFlow";
import type { Job } from "../tauri";
import "./SettingsPanel.css";

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

type SettingsTab = "account" | "tts" | "cloud" | "fetch" | "zoom" | "runtime";

const TABS: { id: SettingsTab; label: string; icon: typeof SlidersHorizontal }[] = [
  { id: "account", label: "Account and connection", icon: SlidersHorizontal },
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
}

export function SettingsPanel({
  onClose,
  invoke,
  projectId,
  onStartApi,
  apiRunning,
  onFetchStarted,
  onOpenExternalAccountPortal,
}: SettingsPanelProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("account");

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
            {activeTab === "account" && <ExternalAccountPanel onClose={() => {}} onOpenPortal={onOpenExternalAccountPortal} embedded />}
            {activeTab === "tts" && <TtsProviderPanel onClose={() => {}} embedded />}
            {activeTab === "cloud" && <CloudProvidersPanel onClose={() => {}} embedded />}
            {activeTab === "fetch" && (
              <MediaFetchPanel projectId={projectId} onClose={() => {}} onStarted={onFetchStarted} embedded />
            )}
            {activeTab === "zoom" && <ZoomPanel onClose={() => {}} embedded />}
            {activeTab === "runtime" && (
              <div className="settings-runtime">
                <p>Local API runtime status: <strong>{apiRunning ? "Running" : "Stopped"}</strong></p>
                <button type="button" className="settings-runtime-start" onClick={onStartApi} disabled={apiRunning}>
                  Start local API
                </button>
              </div>
            )}
          </Suspense>
        </div>
      </div>
    </div>
  );
}
