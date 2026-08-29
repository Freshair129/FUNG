# Desktop Sitemap Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the desktop shell's dual PAGES-card + 11-icon FAB rail with a single notched instrument rail (VU meter + 6 buttons), consolidate five scattered settings triggers into one Settings surface, and add a real Home screen (hero record action + recent meetings) as the app's landing view.

**Architecture:** Three new presentational components (`InstrumentRail`, `SettingsPanel`, `HomeScreen`) replace JSX currently inlined in `src/App.tsx`. `App.tsx` keeps owning all state and handlers; the new components receive callbacks as props (no new global state beyond what's listed per task). The topbar `Segmented` control (already wired to `onViewChange`/`activateAnchor`) becomes the sole P1-P4 switcher — no new nav logic needed there, only removal of the now-redundant `anchor-rail` JSX.

**Tech Stack:** React 18 + TypeScript, Vite, no component test framework present (`node --test` covers business-logic contract tests only — see `package.json`). Verification for UI tasks in this plan is: `npm run build` (tsc catches type errors) + visual check via `npm run desktop` (Tauri dev) or `npm run dev` (Vite, browser).

## Global Constraints

- Canonical stage coordinate space is `1280×720` (see `PANEL_PATH` and `.stage` in `src/styles.css`); all new absolute positioning must be expressed in this space, not eyeballed screenshot pixels.
- The panel's real notch sits at `y: 330→350`, stepping the left edge from `x:72` (upper) to `x:92` (lower) — see `src/App.tsx:97` (`PANEL_PATH`). The new rail's notch must fall at this same seam.
- Do not modify `power-dock` / `power-radial` (`src/App.tsx:1768-1799`, `src/styles.css:1253-1310`) — position, markup, and behavior stay bit-for-bit unchanged.
- Do not modify any P1-P4 tile content, Agent Card copy, Sector C, or Signal definitions (`pageContent`/`agentContent` etc. in `src/App.tsx:190-545`).
- Follow existing color tokens only — no new hex values. Reuse: `--bg-porcelain #f4f1ea`, `--sage #6e897d`, `--indigo #3d4f82`, `--metal #9a8260`, `--signal #b34b4b`, `--graphite #5f6268`, `.sidebar-action` cream gradient `linear-gradient(145deg, #fffdf7, #e5dac7)`, `.sidebar-action.is-active` indigo gradient `linear-gradient(145deg, #6573a0, #34456c)`.
- Icons come from the already-installed `lucide-react` package — no new icon dependency.

---

## Task 1: Extract device pairing into its own rail trigger

Today, clicking the `UserCircle` rail button opens one bundled overlay containing `AccountLoginPanel` + `DevicePairingPanel` + `BackupPanel` together (`src/App.tsx:1278-1326`, state `accountLoginPanelOpen`). The redesigned rail needs Device Pairing as its own one-tap button (§5.3 of the design spec), separate from Account/Backup (which move into the consolidated Settings surface in Task 2). This task splits `DevicePairingPanel` out onto its own state and trigger, with no visual change yet — it's a pure extraction, verified before the rail JSX is touched in Task 4.

**Files:**
- Modify: `src/App.tsx:691-698` (state block), `src/App.tsx:1278-1326` (bundled overlay JSX)

**Interfaces:**
- Produces: `devicePairingPanelOpen: boolean`, `setDevicePairingPanelOpen: (v: boolean | ((v: boolean) => boolean)) => void` — consumed by Task 4's rail wiring.

- [ ] **Step 1: Add the new state variable**

In `src/App.tsx`, immediately after line 697 (`const [accountLoginPanelOpen, setAccountLoginPanelOpen] = useState(false);`), add:

```tsx
  const [devicePairingPanelOpen, setDevicePairingPanelOpen] = useState(false);
```

- [ ] **Step 2: Remove `DevicePairingPanel` from the bundled overlay**

In `src/App.tsx`, find this block (currently lines 1284-1295):

```tsx
          <div className="account-login-stack" onClick={(event) => event.stopPropagation()}>
            {supabaseConfigured ? (
              <Suspense
                fallback={(
                  <section className="account-login-panel" aria-label="กำลังเปิดบัญชี FUNG">
                    <p className="account-login-status">กำลังเปิดบัญชีและอุปกรณ์…</p>
                  </section>
                )}
              >
                <AccountLoginPanel onClose={() => setAccountLoginPanelOpen(false)} />
                <DevicePairingPanel onClose={() => setAccountLoginPanelOpen(false)} />
              </Suspense>
            ) : (
```

Remove the `<DevicePairingPanel ... />` line so it reads:

```tsx
          <div className="account-login-stack" onClick={(event) => event.stopPropagation()}>
            {supabaseConfigured ? (
              <Suspense
                fallback={(
                  <section className="account-login-panel" aria-label="กำลังเปิดบัญชี FUNG">
                    <p className="account-login-status">กำลังเปิดบัญชีและอุปกรณ์…</p>
                  </section>
                )}
              >
                <AccountLoginPanel onClose={() => setAccountLoginPanelOpen(false)} />
              </Suspense>
            ) : (
```

- [ ] **Step 3: Render `DevicePairingPanel` on its own, standalone**

Immediately after the closing `)}` of the `accountLoginPanelOpen && (...)` block (end of the block that was at lines 1278-1326), add a new standalone conditional render, using the same lazy import that already exists at `src/App.tsx:75-77`:

```tsx
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
```

- [ ] **Step 4: Verify the build compiles**

Run: `npm run build`
Expected: no TypeScript errors, no unused-variable warnings for `devicePairingPanelOpen`/`setDevicePairingPanelOpen` (both are used by the JSX added above; they will also be consumed by Task 4).

- [ ] **Step 5: Visual check**

Run: `npm run dev` (or use the `run` skill to launch the Tauri dev build), open the app, and confirm:
- The existing `UserCircle` rail button still opens Account + Backup (pairing no longer appears there).
- No button yet opens the new standalone pairing overlay — that wiring lands in Task 4. This step only confirms nothing broke.

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx
git commit -m "refactor: extract DevicePairingPanel onto its own overlay state"
```

---

## Task 2: Build the consolidated SettingsPanel component

Six rail buttons collapse into one: `SlidersHorizontal` (Settings → `ExternalAccountPanel`), `Volume2` (TTS), `Cloud` (Cloud Providers), `Link2` (Fetch from URL → `MediaFetchPanel`), `Cloud` (Zoom import), and `Activity` (Runtime — currently `handleStartApi()` with no panel). This task builds a new tabbed shell that wraps the five existing panel components plus a Runtime status tab, as one component. Wiring it to a rail button happens in Task 4.

**Files:**
- Create: `src/components/SettingsPanel.tsx`
- Create: `src/components/SettingsPanel.css`

**Interfaces:**
- Consumes (props passed in from `App.tsx`): `onClose: () => void`, `invoke: InvokeFn | null` (the established shared type from `src/lib/backupFlow.ts:68`, already used by `BackupPanel`/`RecoveryNotice`/`GoogleDrivePanel` — `nativeInvoke` itself is typed `InvokeFn | null`, see `src/tauri.ts:106-108`), `projectId: string | null` (same type already passed to `MediaFetchPanel`/`BackupPanel`), `onStartApi: () => void` (existing `handleStartApi` from `App.tsx`), `apiRunning: boolean` (derive from existing runtime/health state — see Task 4 Step 2 for exact source), `onFetchStarted: (job: Job) => void` (the real `Job` type exported from `src/tauri.ts:47-53`, matching `MediaFetchPanel`'s existing `onStarted` prop).
- Produces: default export `SettingsPanel` — consumed by Task 4.

- [ ] **Step 1: Check the exact `invoke` and `onStarted` prop types `MediaFetchPanel`/`BackupPanel` already expect**

Run: `grep -n "interface.*Props" src/components/MediaFetchPanel.tsx src/components/BackupPanel.tsx`
Expected output: prop interfaces naming `invoke: InvokeFn | null`, `projectId`, `onClose`, `onStarted: (job: Job) => void` — `SettingsPanel`'s own prop types (below) already match these; this step just confirms no drift before wiring them together in Step 2.

- [ ] **Step 2: Write `src/components/SettingsPanel.tsx`**

```tsx
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
  onOpenExternalAccountPortal: () => void;
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
```

- [ ] **Step 3: Check whether the wrapped panels accept an `embedded` prop**

Run: `grep -n "interface.*Props" src/components/ExternalAccountPanel.tsx src/components/TtsProviderPanel.tsx src/components/CloudProvidersPanel.tsx src/components/MediaFetchPanel.tsx src/components/ZoomPanel.tsx`

None of them currently declare `embedded`. Add `embedded?: boolean` to each of these five prop interfaces (default unused — a no-op flag reserved for a follow-up pass that suppresses each panel's own backdrop/close-button chrome when shown inside `SettingsPanel`'s shared shell). For this task, just add the optional prop to each interface so `SettingsPanel.tsx` compiles; do not change each panel's internal rendering yet — that visual de-duplication (removing double backdrops) is follow-up work, noted in this plan's Task 4 Step 6 verification.

For each of the five files, find the line matching `interface .*Props {` and add `embedded?: boolean;` as the last field before the closing `}`.

- [ ] **Step 4: Write `src/components/SettingsPanel.css`**

```css
/* src/components/SettingsPanel.css */
.settings-overlay {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(25, 26, 29, 0.45);
}

.settings-stack {
  width: min(720px, 92vw);
  max-height: 84vh;
  display: flex;
  flex-direction: column;
  background: linear-gradient(145deg, #fffdf8, #ebe0cf);
  border-radius: 16px;
  border: 1px solid var(--line-soft);
  box-shadow: var(--shadow);
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--line-soft);
}

.settings-header h2 {
  margin: 0;
  font-size: 16px;
  color: var(--ink);
}

.settings-close {
  width: 30px;
  height: 30px;
  border-radius: 999px;
  background: linear-gradient(145deg, #fffdf7, #e5dac7);
  border: none;
  color: var(--graphite);
  cursor: pointer;
}

.settings-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 12px 20px;
  border-bottom: 1px solid var(--line-soft);
}

.settings-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 12px;
  border-radius: 999px;
  border: 1px solid var(--line-soft);
  background: linear-gradient(145deg, #fffdf7, #e5dac7);
  color: var(--graphite);
  font-size: 12px;
  cursor: pointer;
}

.settings-tab.is-active {
  background: linear-gradient(145deg, #6573a0, #34456c);
  color: white;
  border-color: transparent;
}

.settings-body {
  padding: 20px;
  overflow-y: auto;
}

.settings-loading {
  color: var(--graphite-soft);
  font-size: 13px;
}

.settings-runtime {
  display: flex;
  flex-direction: column;
  gap: 12px;
  color: var(--ink);
  font-size: 13px;
}

.settings-runtime-start {
  align-self: flex-start;
  height: 34px;
  padding: 0 16px;
  border-radius: 999px;
  border: none;
  background: linear-gradient(145deg, #6573a0, #34456c);
  color: white;
  cursor: pointer;
}

.settings-runtime-start:disabled {
  opacity: 0.5;
  cursor: default;
}
```

- [ ] **Step 5: Verify the build compiles**

Run: `npm run build`
Expected: no TypeScript errors. `SettingsPanel` is not yet imported anywhere, so this only validates the new files are self-consistent.

- [ ] **Step 6: Commit**

```bash
git add src/components/SettingsPanel.tsx src/components/SettingsPanel.css src/components/ExternalAccountPanel.tsx src/components/TtsProviderPanel.tsx src/components/CloudProvidersPanel.tsx src/components/MediaFetchPanel.tsx src/components/ZoomPanel.tsx
git commit -m "feat: add consolidated SettingsPanel wrapping 5 existing panels as tabs"
```

---

## Task 3: Build the InstrumentRail component (notched shape, VU meter, 6 buttons)

This is the core visual piece from the design spec — replacing the `anchor-rail` (PAGES card, `src/App.tsx:1342-1357`) and `fab-sidebar` (11 icons, `src/App.tsx:1660-1766`) with one notched rail. The notch geometry mirrors `PANEL_PATH`'s own left-edge notch at the same seam (canonical `y: 330→350`, stepping `x:72→92`, as established in the design spec §5.1).

**Files:**
- Create: `src/components/InstrumentRail.tsx`
- Create: `src/components/InstrumentRail.css`

**Interfaces:**
- Consumes: nothing beyond its own props (pure presentational component).
- Produces (props): `recording: boolean`, `onRecord: () => void`, `onImport: () => void`, `importDisabled: boolean`, `onPlayback: () => void`, `onExport: () => void`, `exportTitle: string`, `onPairDevice: () => void`, `onOpenSettings: () => void`, `levelLeft: number` (0-1), `levelRight: number` (0-1) — consumed by Task 4.

- [ ] **Step 1: Write `src/components/InstrumentRail.css`**

The rail sits at the same canonical position as the two components it replaces (`anchor-rail` origin `16,22`; `fab-sidebar` origin `26,354`), spanning from the old PAGES card's top down to just above `power-dock` (`top: 668px`).

```css
/* src/components/InstrumentRail.css */
.instrument-rail {
  position: absolute;
  left: 16px;
  top: 22px;
  width: 74px;
  height: 638px; /* 660 - 22, clears power-dock (top: 668px) by 8px */
  z-index: 5;
}

.instrument-rail__shape {
  position: absolute;
  inset: 0;
  filter: drop-shadow(4px 5px 10px rgba(112, 95, 74, 0.16));
}

.instrument-rail__fill {
  fill: url(#instrumentRailGradient);
}

.instrument-rail__stroke-outer {
  fill: none;
  stroke: rgba(82, 70, 54, 0.24);
  stroke-width: 1.4;
}

.instrument-rail__stroke-inner {
  fill: none;
  stroke: rgba(255, 255, 255, 0.72);
  stroke-width: 1;
  transform: scale(0.985);
  transform-origin: center;
}

.instrument-rail__content {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.instrument-rail__vu {
  display: flex;
  gap: 6px;
  align-items: flex-end;
  height: 96px;
  margin-top: 14px;
}

.instrument-rail__bar {
  width: 12px;
  height: 100%;
  border-radius: 4px;
  background: rgba(232, 224, 208, 0.9);
  display: flex;
  flex-direction: column-reverse;
  overflow: hidden;
  box-shadow: inset 1px 1px 2px rgba(92, 78, 58, 0.2);
}

.instrument-rail__bar-seg {
  width: 100%;
}

.instrument-rail__vu-label {
  margin-top: 4px;
  font-size: 8px;
  letter-spacing: 1px;
  color: var(--graphite-soft);
}

.instrument-rail__buttons {
  margin-top: 22px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 0 11px;
}

.instrument-rail__button {
  width: 48px;
  height: 40px;
  border-radius: 12px;
  border: 1px solid var(--line-soft);
  background: linear-gradient(145deg, #fffdf7, #e5dac7);
  box-shadow:
    inset 1px 1px 0 var(--bevel-light),
    inset -1px -1px 0 rgba(117, 99, 74, 0.1);
  color: var(--graphite);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.instrument-rail__button.is-active {
  background: linear-gradient(145deg, #6573a0, #34456c);
  color: white;
  box-shadow: inset 3px 3px 8px rgba(21, 29, 51, 0.32), inset -2px -2px 6px rgba(255, 255, 255, 0.18);
  border-color: transparent;
}

.instrument-rail__button:disabled {
  opacity: 0.45;
  cursor: default;
}
```

- [ ] **Step 2: Compute the notch path and write `src/components/InstrumentRail.tsx`**

The rail's local coordinate space is its own `0,0 → 74,638` box (matching the CSS width/height above, i.e. the outer canonical bounds `16,22` through `90,660` shifted to a local origin). Upper zone spans local `x:0→70` (matching `anchor-rail` width 70), lower zone spans local `x:10→74` (matching `fab-sidebar`'s 10px-further-right start), with the notch seam at local `y:308→328` (canonical `330-22=308` through `350-22=328`).

```tsx
import { useId } from "react";
import { Circle, Download, Play, Settings, Upload, Wifi } from "lucide-react";
import "./InstrumentRail.css";

interface InstrumentRailProps {
  recording: boolean;
  onRecord: () => void;
  onImport: () => void;
  importDisabled: boolean;
  onPlayback: () => void;
  onExport: () => void;
  exportTitle: string;
  onPairDevice: () => void;
  onOpenSettings: () => void;
  levelLeft: number;
  levelRight: number;
}

const NOTCH_PATH =
  "M 16,0 H 54 A 16 16 0 0 1 70,16 V 292 A 16 16 0 0 0 86,308 V 308 A 16 16 0 0 1 74,328 V 622 A 16 16 0 0 1 58,638 H 16 A 16 16 0 0 1 0,622 V 16 A 16 16 0 0 1 16,0 Z";

function VuBar({ level, id }: { level: number; id: string }) {
  const clamped = Math.max(0, Math.min(1, level));
  const segments = 6;
  const litSegments = Math.round(clamped * segments);
  const segEls = [];
  for (let i = 0; i < segments; i += 1) {
    const lit = i < litSegments;
    let color = "#6e897d"; // sage — normal
    if (i === segments - 1) color = "#b34b4b"; // signal — peak/overload
    else if (i >= segments - 2) color = "#9a8260"; // metal — near-peak
    segEls.push(
      <div
        key={`${id}-${i}`}
        className="instrument-rail__bar-seg"
        style={{ height: `${100 / segments}%`, background: lit ? color : "transparent" }}
      />,
    );
  }
  return <div className="instrument-rail__bar">{segEls}</div>;
}

export function InstrumentRail({
  recording,
  onRecord,
  onImport,
  importDisabled,
  onPlayback,
  onExport,
  exportTitle,
  onPairDevice,
  onOpenSettings,
  levelLeft,
  levelRight,
}: InstrumentRailProps) {
  const gradientId = useId();

  return (
    <div className="instrument-rail no-drag" aria-label="Instrument rail">
      <svg className="instrument-rail__shape" viewBox="0 0 74 638" aria-hidden="true">
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="#fffdf8" />
            <stop offset="100%" stopColor="#e6dac8" />
          </linearGradient>
        </defs>
        <path d={NOTCH_PATH} fill={`url(#${gradientId})`} />
        <path d={NOTCH_PATH} className="instrument-rail__stroke-outer" />
        <path d={NOTCH_PATH} className="instrument-rail__stroke-inner" />
      </svg>
      <div className="instrument-rail__content">
        <div className="instrument-rail__vu" role="meter" aria-label="Input level">
          <VuBar level={levelLeft} id="vu-l" />
          <VuBar level={levelRight} id="vu-r" />
        </div>
        <div className="instrument-rail__vu-label">L&nbsp;&nbsp;R</div>

        <div className="instrument-rail__buttons">
          <button
            type="button"
            className={`instrument-rail__button ${recording ? "is-active" : ""}`}
            aria-label={recording ? "Pause recording" : "Start recording"}
            onClick={onRecord}
          >
            <Circle size={18} fill={recording ? "currentColor" : "none"} />
          </button>
          <button
            type="button"
            className="instrument-rail__button"
            aria-label="Import audio"
            title="Import audio or video and transcribe locally"
            disabled={importDisabled}
            onClick={onImport}
          >
            <Upload size={18} />
          </button>
          <button type="button" className="instrument-rail__button" aria-label="Playback" onClick={onPlayback}>
            <Play size={18} />
          </button>
          <button
            type="button"
            className="instrument-rail__button"
            aria-label="Export subtitles"
            title={exportTitle}
            onClick={onExport}
          >
            <Download size={18} />
          </button>
          <button type="button" className="instrument-rail__button" aria-label="Pair device" onClick={onPairDevice}>
            <Wifi size={18} />
          </button>
          <button type="button" className="instrument-rail__button" aria-label="Settings" onClick={onOpenSettings}>
            <Settings size={18} />
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Verify the build compiles**

Run: `npm run build`
Expected: no TypeScript errors. `InstrumentRail` is not yet imported anywhere, so this only checks internal consistency (unused-export warnings are fine at this stage).

- [ ] **Step 4: Commit**

```bash
git add src/components/InstrumentRail.tsx src/components/InstrumentRail.css
git commit -m "feat: add InstrumentRail component with notched shell and VU meter"
```

---

## Task 4: Wire InstrumentRail + SettingsPanel + pairing trigger into App.tsx, remove old rail JSX

**Files:**
- Modify: `src/App.tsx` (imports, state, JSX at `1342-1357` and `1660-1766`)

**Interfaces:**
- Consumes: `InstrumentRail` (Task 3), `SettingsPanel` (Task 2), `devicePairingPanelOpen`/`setDevicePairingPanelOpen` (Task 1).

- [ ] **Step 1: Add imports**

In `src/App.tsx`, near the other lazy component imports (around line 75), add:

```tsx
import { InstrumentRail } from "./components/InstrumentRail";
const SettingsPanel = lazy(() =>
  import("./components/SettingsPanel").then((module) => ({ default: module.SettingsPanel })),
);
```

- [ ] **Step 2: Add `settingsPanelOpen` state and find the existing API-running signal**

Run: `grep -n "handleStartApi\|apiHealth\|apiRunning\|health\b" src/App.tsx | head -20`

Confirm which existing variable reflects whether the local API is running (used elsewhere for the `Activity`/Runtime badge). Add, next to the other panel-open state declarations (after the `devicePairingPanelOpen` line added in Task 1):

```tsx
  const [settingsPanelOpen, setSettingsPanelOpen] = useState(false);
```

- [ ] **Step 3: Remove the old panel-open triggers now folded into Settings**

The following state declarations become unused by the rail (still used internally by `SettingsPanel` only if you choose to lift state up — this plan keeps them local to `App.tsx` and passes them through, since `ExternalAccountPanel`/`TtsProviderPanel`/`CloudProvidersPanel`/`ZoomPanel`/`MediaFetchPanel` are now only ever rendered *inside* `SettingsPanel`). Remove these four standalone conditional renders from the JSX (they are replaced by `SettingsPanel` in Step 5):

Delete these lines (currently `src/App.tsx:1258-1270` and `1274`):

```tsx
      {accountPanelOpen && <ExternalAccountPanel onClose={() => setAccountPanelOpen(false)} onOpenPortal={openExternalAccountPortal} />}
      {zoomPanelOpen && <ZoomPanel onClose={() => setZoomPanelOpen(false)} />}
      {mediaFetchOpen && (
        <MediaFetchPanel
          projectId={selectedProjectId ?? null}
          onClose={() => setMediaFetchOpen(false)}
          onStarted={(job) => {
            setJobs((current) => [job, ...current.filter((entry) => entry.id !== job.id)]);
            void pollJobUntilDone(job.id).then(() => void refresh());
          }}
        />
      )}
      {ttsPanelOpen && <TtsProviderPanel onClose={() => setTtsPanelOpen(false)} />}
```

and (currently line 1274):

```tsx
      {cloudProvidersPanelOpen && <CloudProvidersPanel onClose={() => setCloudProvidersPanelOpen(false)} />}
```

`SettingsPanel` (Step 4 below) replaces all five conditionals with one `settingsPanelOpen && <SettingsPanel ... />`, so the five old booleans (`accountPanelOpen`, `zoomPanelOpen`, `mediaFetchOpen`, `ttsPanelOpen`, `cloudProvidersPanelOpen`) become fully unused. Delete their `useState` declarations too (`src/App.tsx:691-694`). Run `grep -n "setAccountPanelOpen\|setZoomPanelOpen\|setMediaFetchOpen\|setTtsPanelOpen\|setCloudProvidersPanelOpen" src/App.tsx` afterward and confirm zero remaining references before moving on.

- [ ] **Step 4: Insert `SettingsPanel` render**

In the same area where the deleted conditionals were, add:

```tsx
      {settingsPanelOpen && (
        <Suspense fallback={null}>
          <SettingsPanel
            onClose={() => setSettingsPanelOpen(false)}
            invoke={nativeInvoke}
            projectId={selectedProjectId ?? null}
            onStartApi={() => void handleStartApi()}
            apiRunning={apiRunning}
            onFetchStarted={(job) => {
              setJobs((current) => [job, ...current.filter((entry) => entry.id !== job.id)]);
              void pollJobUntilDone(job.id).then(() => void refresh());
            }}
            onOpenExternalAccountPortal={openExternalAccountPortal}
          />
        </Suspense>
      )}
```

Replace `apiRunning` with whatever exact variable name Step 2's grep found (e.g. it may be `health.apiRunning` or similar — use the real expression, not a placeholder).

- [ ] **Step 5: Remove the `anchor-rail` JSX**

Delete the entire block at `src/App.tsx:1342-1357`:

```tsx
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
```

`activateAnchor` (defined at `src/App.tsx:976-979`) stays — it is reused by Task 6's Home screen navigation.

- [ ] **Step 6: Replace the `fab-sidebar` JSX with `InstrumentRail`**

Delete the entire block at `src/App.tsx:1660-1766` (the `<div className="fab fab-sidebar no-drag">...</div>`, all 11 buttons) and replace it with:

```tsx
          <InstrumentRail
            recording={liveMeetingOpen}
            onRecord={() => {
              setActiveAnchor("P1");
              setActiveView(viewByAnchor.P1);
              setActiveTileByAnchor((current) => ({ ...current, P1: "live-capture" }));
              setLiveMeetingOpen(true);
            }}
            onImport={() => void handleImportAndTranscribe()}
            importDisabled={transcribing}
            onPlayback={() => void handleCreateJob("transcript.transcribe")}
            onExport={() => void handleCreateJob("export.render")}
            exportTitle={
              jobActionBlockedReason("export.render", Boolean(activeRecordingId)) ??
              "ส่งออกซับไตเติล .srt และ .vtt ของการบันทึกนี้"
            }
            onPairDevice={() => setDevicePairingPanelOpen((open) => !open)}
            onOpenSettings={() => setSettingsPanelOpen(true)}
            levelLeft={0}
            levelRight={0}
          />
```

`levelLeft`/`levelRight` are hardcoded to `0` (idle meter) here — live audio-level wiring is out of scope for this plan (see design spec §8, Open Question 2). The meter renders correctly at rest; a follow-up plan wires it to the real input stream.

- [ ] **Step 7: Verify the build compiles**

Run: `npm run build`
Expected: no TypeScript errors, no references to removed state (`accountPanelOpen`, `zoomPanelOpen`, `mediaFetchOpen`, `ttsPanelOpen`, `cloudProvidersPanelOpen`) remaining anywhere in the file.

- [ ] **Step 8: Visual check**

Launch the app (`npm run desktop` or the `run` skill) and confirm:
- The old `PAGES` card and 11-icon black rail are gone; one notched rail with a VU meter (idle/empty) and 6 buttons is in their place, in the same screen region.
- The topbar segmented control (Capture/Transcript/Summary/Runtime) still switches P1-P4 correctly (unchanged — it was never touched).
- Clicking the rail's record icon starts Live Meeting exactly as the old mic button did.
- Clicking the pair-device icon opens only the pairing overlay (not Account/Backup).
- Clicking the settings icon opens the new tabbed `SettingsPanel`, and each of its 6 tabs renders its wrapped panel without crashing.
- `power-dock` (bottom-left) is visually untouched.

- [ ] **Step 9: Commit**

```bash
git add src/App.tsx
git commit -m "feat: replace PAGES card + fab-sidebar with InstrumentRail and consolidated SettingsPanel"
```

---

## Task 5: Build the HomeScreen component

**Files:**
- Create: `src/components/HomeScreen.tsx`
- Create: `src/components/HomeScreen.css`

**Interfaces:**
- Consumes: `LibraryItem` type (already defined in `src/App.tsx:120-125` — re-declare identically in this file, or export it from `App.tsx` and import it; this plan re-declares to keep `HomeScreen` free of an `App.tsx` import cycle).
- Produces (props): `items: LibraryItem[]`, `onStartRecording: () => void`, `onImport: () => void`, `onOpenItem: (id: string) => void` — consumed by Task 6.

- [ ] **Step 1: Write `src/components/HomeScreen.css`**

```css
/* src/components/HomeScreen.css */
.home-screen {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  padding: 32px 40px;
  gap: 24px;
}

.home-screen__brand {
  font-size: 11px;
  letter-spacing: 2px;
  color: var(--graphite-soft);
  text-transform: uppercase;
}

.home-screen__actions {
  display: flex;
  align-items: center;
  gap: 16px;
}

.home-screen__hero {
  height: 52px;
  padding: 0 28px;
  border-radius: 14px;
  border: none;
  background: linear-gradient(145deg, #6573a0, #34456c);
  color: white;
  font-size: 15px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
}

.home-screen__secondary {
  height: 44px;
  padding: 0 20px;
  border-radius: 14px;
  border: 1px solid var(--line-soft);
  background: transparent;
  color: var(--ink);
  font-size: 13px;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.home-screen__list-label {
  font-size: 12px;
  letter-spacing: 1px;
  color: var(--graphite-soft);
  text-transform: uppercase;
}

.home-screen__list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
}

.home-screen__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-radius: 12px;
  background: linear-gradient(145deg, #fffdf8, #ebe0cf);
  box-shadow: var(--shadow);
  border: none;
  cursor: pointer;
  text-align: left;
  font-size: 13px;
  color: var(--ink);
}

.home-screen__row-subtitle {
  color: var(--graphite-soft);
  font-size: 12px;
}
```

- [ ] **Step 2: Write `src/components/HomeScreen.tsx`**

```tsx
import { Mic, Upload } from "lucide-react";
import "./HomeScreen.css";

type LibraryItem = {
  id: string;
  title: string;
  subtitle: string;
  state: string;
};

interface HomeScreenProps {
  items: LibraryItem[];
  onStartRecording: () => void;
  onImport: () => void;
  onOpenItem: (id: string) => void;
}

export function HomeScreen({ items, onStartRecording, onImport, onOpenItem }: HomeScreenProps) {
  return (
    <div className="home-screen" data-tauri-drag-region>
      <div className="home-screen__brand">FUNG</div>

      <div className="home-screen__actions">
        <button type="button" className="home-screen__hero" onClick={onStartRecording}>
          <Mic size={18} />
          เริ่มบันทึกประชุม
        </button>
        <button type="button" className="home-screen__secondary" onClick={onImport}>
          <Upload size={16} />
          นำเข้าไฟล์เสียง
        </button>
      </div>

      <div className="home-screen__list-label">การประชุมล่าสุด</div>
      <div className="home-screen__list">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            className="home-screen__row"
            onClick={() => onOpenItem(item.id)}
          >
            <span>{item.title}</span>
            <span className="home-screen__row-subtitle">
              {item.subtitle} · {item.state}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Verify the build compiles**

Run: `npm run build`
Expected: no TypeScript errors. `HomeScreen` is not yet imported anywhere at this point.

- [ ] **Step 4: Commit**

```bash
git add src/components/HomeScreen.tsx src/components/HomeScreen.css
git commit -m "feat: add HomeScreen component with hero record action and recent list"
```

---

## Task 6: Wire HomeScreen into App.tsx as the initial view

**Files:**
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: `HomeScreen` (Task 5), existing `libraryItems` (`src/App.tsx:733-...`), existing `activateAnchor` (`src/App.tsx:976-979`), existing `setSelectedRecording` (`src/App.tsx:755`).

- [ ] **Step 1: Add `showHome` state**

Near the other top-level UI state (after the `settingsPanelOpen` line added in Task 4 Step 2), add:

```tsx
  const [showHome, setShowHome] = useState(true);
```

`true` by default so the app lands on Home at launch, matching the design spec's acceptance criterion: *"Launching the app lands on Home... with no P selected."*

- [ ] **Step 2: Import `HomeScreen`**

Near the other component imports (alongside `InstrumentRail` added in Task 4 Step 1):

```tsx
import { HomeScreen } from "./components/HomeScreen";
```

- [ ] **Step 3: Add navigation handlers**

Immediately after `activateAnchor` (`src/App.tsx:976-979`), add:

```tsx
  const enterMeetingWorkspace = (anchor: Anchor) => {
    setShowHome(false);
    activateAnchor(anchor);
  };

  const returnToHome = () => setShowHome(true);
```

- [ ] **Step 4: Wire Home's callbacks to existing handlers**

In the JSX, find the `.stage-wrap` root (`src/App.tsx:1338`). Wrap the existing stage content so it only renders when `!showHome`, and render `HomeScreen` when `showHome` is true. The existing structure is:

```tsx
      <div className="stage-wrap" style={{ transform: `scale(${scale})` }}>
        <main className="stage" aria-label="FUNG review workspace">
```

Change it to:

```tsx
      <div className="stage-wrap" style={{ transform: `scale(${scale})` }}>
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
        <main className="stage" aria-label="FUNG review workspace">
```

...and at the matching end of the `<main className="stage">` block, close the added `)}`. Find the closing tag for `<main className="stage">` — run `grep -n "</main>" src/App.tsx` to locate it — and change:

```tsx
        </main>
      </div>
```

to:

```tsx
        </main>
        )}
      </div>
```

- [ ] **Step 5: Add a "return to Home" breadcrumb in the topbar**

Find the topbar `Segmented` control block (`src/App.tsx:1634-1639`, inside the `score-header` or adjacent topbar zone — run `grep -n "Segmented" src/App.tsx` to confirm the exact surrounding JSX). Immediately before the `<Segmented ... />` element, add:

```tsx
            <button type="button" className="icon-button no-drag" aria-label="Back to Home" onClick={returnToHome}>
              <Home size={16} />
            </button>
```

Add `Home` to the `lucide-react` import list at the top of `src/App.tsx` (alongside `HardDriveDownload`, `Link2`, etc., added in alphabetical position per the existing list's ordering).

- [ ] **Step 6: Verify the build compiles**

Run: `npm run build`
Expected: no TypeScript errors.

- [ ] **Step 7: Visual check**

Launch the app and confirm:
- On first load, `HomeScreen` renders (hero record button + import button + recent list), no P1-P4 content visible.
- Clicking "เริ่มบันทึกประชุม" navigates into P1 with Live Meeting active, exactly as the old mic rail button did.
- Clicking "นำเข้าไฟล์เสียง" navigates into P1 and starts the import flow.
- Clicking a recent-list row navigates into P2 with that recording selected.
- The new Home icon in the topbar returns to `HomeScreen` from anywhere in the P1-P4 workflow.
- The topbar segmented control and `InstrumentRail` both continue to work correctly once inside the workflow (unaffected by this task).

- [ ] **Step 8: Commit**

```bash
git add src/App.tsx
git commit -m "feat: add Home screen as the app's landing view"
```

---

## Self-Review Notes

**Spec coverage:**
- §4 Navigation structure (Home + topbar-only P1-P4 switching) → Tasks 5, 6.
- §5 Notched instrument rail (VU meter + 6 buttons, real notch geometry) → Task 3, wired in Task 4.
- §6 Consolidated Settings → Task 2, wired in Task 4.
- §7 Preserved items (`power-dock`, P1-P4 content, material tokens) → explicitly untouched by every task; called out in Global Constraints.
- §8 Open Questions (live VU data, modal-vs-screen for Settings, icon source, exact rail pixel math) → resolved where the plan needed a concrete answer (Settings = modal overlay; icons = `lucide-react`; rail math re-derived in canonical `1280×720` space in Task 3) or explicitly deferred with a note (live VU data — Task 4 Step 6 hardcodes `levelLeft`/`levelRight` to `0` and states this is follow-up work).

**Known follow-up work not in this plan** (flagged inline where relevant, repeated here for visibility):
- Live audio-level wiring for the VU meter.
- Removing each wrapped settings panel's own backdrop/close-button chrome now that `SettingsPanel` provides a shared shell (Task 2 Step 3 adds the `embedded` prop but doesn't yet use it to suppress duplicate chrome — first pass will show a panel-within-a-panel look that a follow-up visual pass should clean up).
