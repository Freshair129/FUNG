import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import * as typescript from "typescript";

const worktreeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runtimeDir = await mkdtemp(path.join(os.tmpdir(), "fung-desktop-shell-"));

function transpile(source, compilerOptions = {}) {
  return typescript.transpileModule(source, {
    compilerOptions: {
      target: typescript.ScriptTarget.ES2020,
      module: typescript.ModuleKind.ESNext,
      jsx: typescript.JsxEmit.ReactJSX,
      moduleResolution: typescript.ModuleResolutionKind.NodeJs,
      esModuleInterop: true,
      ...compilerOptions,
    },
  }).outputText;
}

const reactModuleUrl = import.meta.resolve("react");
const jsxRuntimeModuleUrl = import.meta.resolve("react/jsx-runtime");
const lucideModuleUrl = import.meta.resolve("lucide-react");

function resolveRuntimeImports(source) {
  return source
    .replaceAll('from "react/jsx-runtime"', `from "${jsxRuntimeModuleUrl}"`)
    .replaceAll('from "react"', `from "${reactModuleUrl}"`)
    .replaceAll('from "lucide-react"', `from "${lucideModuleUrl}"`);
}

const [
  shellSource,
  contractsSource,
  brandAssetSource,
  shellCssSource,
  designTokensCssSource,
  glassCssSource,
  reviewCssSource,
  appCssSource,
  studioSource,
  studioCssSource,
] = await Promise.all([
  readFile(path.join(worktreeRoot, "src/components/desktop/DesktopShell.tsx"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/contracts.ts"), "utf8"),
  readFile(path.join(worktreeRoot, "docs/brand-kit/logo/fung-mark.svg"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/DesktopShell.css"), "utf8"),
  readFile(path.join(worktreeRoot, "docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.css"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/LiquidGlass.css"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/RecordingReview.css"), "utf8"),
  readFile(path.join(worktreeRoot, "src/styles.css"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/StudioHome.tsx"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/DesktopStudio.css"), "utf8"),
]);

test("shell rows are assigned by region rather than optional child count", () => {
  assert.match(studioCssSource, /grid-template-areas:\s*"capture"\s*"body"\s*"dock"/);
  assert.match(studioCssSource, /grid-template-rows:\s*auto minmax\(0, 1fr\) 52px/);
  assert.match(shellCssSource, /grid-template-areas:\s*"header"\s*"capture"\s*"body"/);
  for (const [className, area] of [["header", "header"], ["capture-strip", "capture"], ["body", "body"]]) {
    assert.match(shellCssSource, new RegExp(`\\.desktop-shell__${className}\\s*\\{[^}]*grid-area:\\s*${area}`));
  }
  const main = shellCssSource.match(/\.desktop-shell\[data-fung-root\] \.desktop-shell__main\s*\{([^}]+)\}/)?.[1];
  assert.match(main ?? "", /display:\s*flex/);
  assert.match(main ?? "", /overflow-y:\s*auto/);
  assert.doesNotMatch(main ?? "", /grid-template-rows/);
});

test("glass stacking cannot turn skip links or fixed overlays into grid rows", () => {
  assert.doesNotMatch(glassCssSource, /\.desktop-shell\[data-material="glass"\]\s*>\s*\*\s*\{/);
  assert.match(glassCssSource, />\s*:is\(\.desktop-shell__header, \.desktop-shell__capture-strip, \.desktop-shell__body\)/);
  assert.match(shellCssSource, /\.desktop-shell > #fung-companion-panel\s*\{\s*display:\s*contents/);
  assert.match(shellSource, /<details ref=\{projectMenuRef\} className="desktop-shell__sidebar-context desktop-shell__project-menu"/);
  assert.match(shellSource, /<summary className="desktop-shell__project-toggle"/);
});

test("glass effects stay inside the rounded shell and clear the rectangular native backdrop", () => {
  assert.match(shellSource, /desktop_window_set_glass/);
  assert.match(shellSource, /desktop_window_set_glass", \{ enabled: false \}/);
  assert.match(studioCssSource, /backdrop-filter:\s*blur\(30px\) saturate\(155%\)/);
  assert.match(studioCssSource, /#root:has\(> \.desktop-shell\[data-layout="studio"\]\)\s*\{[^}]*z-index:\s*1/);
  assert.match(studioCssSource, /background-color:\s*var\(--fung-glass-solid\)/);
});

test("hosted review and live pages defer page scrolling to main", () => {
  assert.match(shellCssSource, /\.desktop-shell__surface-content\s*\{[^}]*overflow:\s*visible/);
  assert.match(reviewCssSource, /\.desktop-shell \.recording-review\s*\{[^}]*max-height:\s*none;[^}]*overflow:\s*visible/);
  assert.match(appCssSource, /\.desktop-shell \.callmd-surface-stack \.live-panel\s*\{[^}]*min-width:\s*0;[^}]*overflow:\s*visible/);
  assert.match(appCssSource, /\.desktop-shell \.callmd-surface-stack\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/);
});

await writeFile(path.join(runtimeDir, "contracts.mjs"), resolveRuntimeImports(transpile(contractsSource)), "utf8");
await writeFile(
  path.join(runtimeDir, "tauri.mjs"),
  "export const nativeInvoke = null; export const listMeetingActionItems = async () => ({ items: [], totalCount: 0 });\n",
  "utf8",
);
await writeFile(
  path.join(runtimeDir, "DesktopShell.mjs"),
  resolveRuntimeImports(transpile(shellSource))
    .replace('from "../../tauri.ts"', 'from "./tauri.mjs"')
    .replace('from "./contracts.ts"', 'from "./contracts.mjs"')
    .replace('from "./CompanionPanel"', 'from "./CompanionPanel.mjs"')
    .replace('from "./StudioHome"', 'from "./StudioHome.mjs"')
    .replace('import "./DesktopShell.css";', "")
    .replace('import "./LiquidGlass.css";', "")
    .replace('import "./DesktopStudio.css";', ""),
  "utf8",
);
await writeFile(path.join(runtimeDir, "StudioHome.mjs"), resolveRuntimeImports(transpile(studioSource)), "utf8");
await writeFile(
  path.join(runtimeDir, "CompanionPanel.mjs"),
  "export function CompanionPanel() { return null; }\n",
  "utf8",
);

const authoritativeMarkPath = brandAssetSource.match(/<path\b[^>]*\bd="([^"]+)"/)?.[1];
assert.ok(authoritativeMarkPath, "the brand-kit SVG must expose an authoritative path");

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const shellModule = await import(`${pathToFileURL(path.join(runtimeDir, "DesktopShell.mjs"))}?shell-test=1`);
const {
  DesktopShell,
  NAVIGATION_GUARD_CHOICES,
  NavigationGuardDialog,
  getCaptureLifecycle,
  isInactiveCapture,
  normalizeShellError,
  resolveProjectPanelState,
  resolveStopAndLeaveError,
  shouldGuardNavigation,
} = shellModule;

function state(status, data = null, error = null) {
  return { status, identity: null, data, error };
}

const projectA = {
  id: "project-a",
  name: "โครงการทดลอง A",
  storagePath: "not-rendered",
  activeRecordingId: null,
  createdAt: "2026-09-17T00:00:00.000Z",
  updatedAt: "2026-09-17T00:00:00.000Z",
};

function createProps(overrides = {}) {
  const calls = [];
  const actions = {
    selectProject: (projectId) => calls.push(["selectProject", projectId]),
    selectRecording: (selection) => calls.push(["selectRecording", selection]),
    showHome: () => calls.push(["showHome"]),
    showLive: () => calls.push(["showLive"]),
    showReview: () => calls.push(["showReview"]),
    showOutput: () => calls.push(["showOutput"]),
    showAppearance: () => calls.push(["showAppearance"]),
    startRecording: () => calls.push(["startRecording"]),
    stopRecording: () => calls.push(["stopRecording"]),
    openReview: () => calls.push(["openReview"]),
    stopAndLeave: async () => ({ active: false, stopping: false, projectId: null, recordingId: null, elapsedMs: null }),
    openSettings: () => calls.push(["openSettings"]),
    openAccount: () => calls.push(["openAccount"]),
    openPairing: () => calls.push(["openPairing"]),
    importMedia: () => calls.push(["importMedia"]),
    exportMedia: () => calls.push(["exportMedia"]),
    exportDisabled: false,
    exportTitle: "ส่งออก",
    setTheme: (theme) => calls.push(["setTheme", theme]),
    minimizeWindow: () => calls.push(["minimizeWindow"]),
    hideWindowToTray: () => calls.push(["hideWindowToTray"]),
    closeWindow: () => calls.push(["closeWindow"]),
  };

  return {
    scopeChoice: "B",
    project: state("ready", [projectA]),
    selectedProjectId: projectA.id,
    selection: null,
    activeSurface: "home",
    liveStatus: state("ready", {
      active: false,
      stopping: false,
      projectId: null,
      recordingId: null,
      elapsedMs: null,
    }),
    theme: "light",
    mainContent: React.createElement("p", { "data-testid": "surface-content" }, "Surface content"),
    settingsSlot: null,
    pairingSlot: null,
    recoverySlot: null,
    actions,
    ...overrides,
    _calls: calls,
  };
}

function renderShellWithProps(overrides = {}) {
  return renderToStaticMarkup(React.createElement(DesktopShell, createProps(overrides)));
}

test("production shell helpers keep capture guards, stop confirmation, and safe failures explicit", () => {
  assert.equal(getCaptureLifecycle(state("loading"), "starting"), "starting");
  assert.equal(getCaptureLifecycle(state("ready", { active: true, stopping: false })), "active");
  assert.equal(getCaptureLifecycle(state("ready", { active: true, stopping: true })), "stopping");
  assert.equal(getCaptureLifecycle(state("ready", { active: false, stopping: false })), "inactive");

  assert.deepEqual([...NAVIGATION_GUARD_CHOICES], ["continue", "stop", "stay"]);
  assert.equal(shouldGuardNavigation("active", true), true);
  assert.equal(shouldGuardNavigation("stopping", true), true);
  assert.equal(shouldGuardNavigation("active", false), false);
  assert.equal(shouldGuardNavigation("inactive", true), false);

  assert.equal(isInactiveCapture({ active: false, stopping: false }), true);
  assert.equal(isInactiveCapture({ active: false, stopping: true }), false);
  assert.equal(resolveStopAndLeaveError({ active: false, stopping: false }), null);

  const unconfirmed = resolveStopAndLeaveError({ active: true, stopping: true });
  assert.equal(unconfirmed.code, "LEGACY_COMMAND_FAILED");
  assert.match(unconfirmed.message, /ยังยืนยันไม่ได้/);

  const safeFailure = normalizeShellError(new Error("C:\\private\\recording.wav token=secret stderr=hidden"));
  assert.equal(safeFailure.code, "LEGACY_COMMAND_FAILED");
  assert.equal(safeFailure.message, "Desktop command failed.");
  assert.doesNotMatch(safeFailure.message, /private|secret|hidden/);
});

test("capture lifecycle precedence separates read bootstrap from the existing owner phase", () => {
  const cases = [
    ["native stopping wins over an owner start", state("ready", { active: true, stopping: true }), "starting", "stopping"],
    ["owner stopping wins over an active read", state("ready", { active: true, stopping: false }), "stopping", "stopping"],
    ["native active wins over an owner start", state("loading", { active: true, stopping: false }), "starting", "active"],
    ["owner starting is the only start phase", state("loading"), "starting", "starting"],
    ["owner listening is active", state("loading"), "listening", "active"],
    ["owner degraded is active", state("loading"), "degraded", "active"],
    ["bootstrap loading without a phase is unknown", state("loading"), undefined, "inactive"],
    ["an authoritative inactive pair is inactive", state("ready", { active: false, stopping: false }), "idle", "inactive"],
  ];

  for (const [label, liveStatus, livePhase, expected] of cases) {
    assert.equal(getCaptureLifecycle(liveStatus, livePhase), expected, label);
    assert.equal(shouldGuardNavigation(expected, true), expected !== "inactive", label);
  }
});

test("bootstrap and failed reads do not fabricate capture while known capture survives read errors", () => {
  const readError = { code: "STORAGE_READ_FAILED", message: "อ่านสถานะไม่ได้", retryable: true };
  const unknownReadCases = [
    ["loading", state("loading"), "กำลังอ่านสถานะ"],
    ["unavailable", state("unavailable"), "สถานะการบันทึกยังไม่พร้อมอ่าน"],
    ["error", state("error", null, readError), "อ่านสถานะการบันทึกไม่สำเร็จ"],
  ];

  for (const [label, liveStatus, expectedNotice] of unknownReadCases) {
    assert.equal(getCaptureLifecycle(liveStatus), "inactive", label);
    assert.equal(shouldGuardNavigation(getCaptureLifecycle(liveStatus), true), false, label);
    const markup = renderShellWithProps({ liveStatus });
    assert.doesNotMatch(markup, /aria-label="สถานะการบันทึก"/, label);
    assert.doesNotMatch(markup, /มีเซสชันบันทึกอยู่/, label);
    assert.doesNotMatch(markup, /พร้อมตรวจสอบ/, label);
    if (label !== "loading") assert.match(markup, new RegExp(expectedNotice), label);
  }

  const startingStatus = state("error", null, readError);
  assert.equal(getCaptureLifecycle(startingStatus, "starting"), "starting");
  assert.equal(shouldGuardNavigation(getCaptureLifecycle(startingStatus, "starting"), true), true);
  const startingMarkup = renderShellWithProps({ liveStatus: startingStatus, livePhase: "starting" });
  assert.match(startingMarkup, /aria-label="สถานะการบันทึก"/);
  assert.match(startingMarkup, /กำลังเตรียมการบันทึก/);
  assert.match(startingMarkup, /อ่านสถานะการบันทึกไม่สำเร็จ/);
  assert.match(startingMarkup, /มีเซสชันบันทึกอยู่/);

  for (const data of [
    { active: true, stopping: false },
    { active: true, stopping: true },
  ]) {
    const liveStatus = state("error", data, readError);
    const markup = renderShellWithProps({ liveStatus });
    assert.match(markup, /aria-label="สถานะการบันทึก"/);
    assert.match(markup, /อ่านสถานะการบันทึกไม่สำเร็จ/);
    assert.match(markup, /มีเซสชันบันทึกอยู่/);
  }
});

test("SSR keeps Home, Live, and History as primary navigation with Storage under More", () => {
  const homeMarkup = renderToStaticMarkup(React.createElement(DesktopShell, createProps()));
  assert.match(homeMarkup, /href="#home"[^>]*aria-current="page"/);
  assert.match(homeMarkup, /href="#live"/);
  assert.match(homeMarkup, /href="#history"/);
  assert.doesNotMatch(homeMarkup, /href="#output"/);
  assert.match(homeMarkup, /aria-label="พื้นที่ทำงานหลัก"/);
  assert.match(homeMarkup, /ตำแหน่งจัดเก็บ/);
  assert.match(homeMarkup, /ข้ามไปยังเนื้อหาหลัก/);
  assert.match(homeMarkup, /เริ่มประชุม/);
  assert.match(homeMarkup, /เปิดพื้นที่ประชุมก่อนเริ่มบันทึกเสียง/);
  assert.match(homeMarkup, /<section hidden="" class="desktop-shell__surface-content"/);
  assert.match(homeMarkup, /Surface content/);
  assert.doesNotMatch(homeMarkup, /พื้นที่ทำงานเดิม|callmd-legacy-workspace|fab-topbar|power-dock/);
  assert.match(homeMarkup, /data-surface="home"/);
  assert.match(homeMarkup, /aria-label="ย่อหน้าต่าง"/);
  assert.match(homeMarkup, /aria-label="ซ่อนลงถาดระบบ"/);
  assert.match(homeMarkup, /aria-label="ปิดแอป"/);

  const reviewMarkup = renderToStaticMarkup(
    React.createElement(
      DesktopShell,
      createProps({
        activeSurface: "review",
        selection: { projectId: projectA.id, recordingId: "recording-a" },
        theme: "dark",
      }),
    ),
  );
  assert.match(reviewMarkup, /href="#history"[^>]*aria-current="page"/);
  assert.match(reviewMarkup, /ทบทวนบันทึกที่เลือก/);
  assert.match(reviewMarkup, /บันทึกที่เลือก/);
  assert.match(reviewMarkup, /data-theme="dark"/);
  assert.doesNotMatch(reviewMarkup, /เริ่มประชุม/);

  const outputMarkup = renderToStaticMarkup(
    React.createElement(DesktopShell, createProps({ activeSurface: "output" })),
  );
  assert.doesNotMatch(outputMarkup, /href="#output"/);
  assert.match(outputMarkup, /ตำแหน่งจัดเก็บไฟล์บันทึก/);
  assert.match(outputMarkup, /aria-current="page"/);
});

test("appearance is a separate active surface and not an inline dock disclosure", () => {
  const markup = renderToStaticMarkup(
    React.createElement(DesktopShell, createProps({ activeSurface: "appearance" })),
  );
  assert.match(markup, /data-surface="appearance"/);
  assert.match(markup, /ลักษณะและธีม/);
  assert.match(markup, /id="desktop-shell-theme"/);
  assert.match(markup, /id="desktop-shell-material"/);
  assert.match(markup, /id="desktop-shell-transparency"/);

  const sidebar = markup.match(/<footer[^>]*class="desktop-shell__dock"[\s\S]*?<\/footer>/)?.[0];
  assert.ok(sidebar, "appearance render should keep the bottom dock");
  assert.match(sidebar, /ลักษณะ/);
  assert.doesNotMatch(sidebar, /desktop-shell-theme|desktop-shell-material|desktop-shell-transparency/);
  assert.doesNotMatch(sidebar, /<button[^>]*(เริ่มบันทึก|หยุดบันทึก)/);
});

test("SSR renders the authoritative unboxed 40px FUNG mark and currentColor wordmark", () => {
  const markup = renderToStaticMarkup(React.createElement(DesktopShell, createProps()));
  const mark = markup.match(/<svg\b[^>]*class="desktop-shell__brand-mark"[^>]*>[\s\S]*?<\/svg>/)?.[0];

  assert.ok(mark, "the shell should SSR an identifiable brand mark");
  assert.match(markup, /class="desktop-shell__brand" role="img" aria-label="FUNG Quiet Archive"/);
  assert.match(mark, /width="40"/);
  assert.match(mark, /height="40"/);
  assert.match(mark, /viewBox="0 0 100 100"/);
  assert.match(mark, /aria-hidden="true"/);
  assert.match(mark, /<path[^>]+fill="currentColor"[^>]+fill-rule="evenodd"/);
  assert.match(mark, new RegExp(`d="${escapeRegExp(authoritativeMarkPath)}"`));
  assert.match(markup, /class="desktop-shell__brand-wordmark"[^>]*>FUNG<\/span>/);
  assert.doesNotMatch(markup, /linearGradient|porcelainGrad|fung-logo-container/);
});

test("native close control uses the guarded close action and drag boundaries stay explicit", () => {
  assert.match(shellSource, /<footer className="desktop-shell__dock" data-tauri-drag-region>/);
  assert.match(
    shellSource,
    /const requestCloseWindow = \(initiator: HTMLElement\) => \{[\s\S]*?action: actions\.closeWindow/,
  );
  assert.match(
    shellSource,
    /className="desktop-shell__close-button"[\s\S]*?aria-label="ปิดแอป"[\s\S]*?onClick=\{\(event\) => requestCloseWindow\(event\.currentTarget\)\}/,
  );
  assert.match(shellSource, /aria-label="ย่อหน้าต่าง"[\s\S]*?actions\.minimizeWindow/);
  assert.match(shellSource, /aria-label="ซ่อนลงถาดระบบ"[\s\S]*?actions\.hideWindowToTray/);
  assert.doesNotMatch(shellSource, /className="panel-glass" data-tauri-drag-region/);
  assert.doesNotMatch(shellSource, /className="fab fab-topbar" data-tauri-drag-region/);
  assert.doesNotMatch(shellSource, /desktop-shell__legacy-workspace|callmd-legacy-workspace|power-dock/);
  assert.doesNotMatch(shellSource, /HomeScreen|InstrumentRail|fab-topbar|power-dock/);
  assert.match(shellCssSource, /\.desktop-shell__header :where\(a, button, select, input, textarea\)/);
  assert.doesNotMatch(shellCssSource, /\.desktop-shell__header-button/);
});

test("Studio home keeps desktop utilities in the dock overflow and account separate from close", () => {
  const home = renderToStaticMarkup(React.createElement(DesktopShell, createProps()));
  assert.match(home, /data-layout="studio"/);
  assert.match(home, /class="desktop-shell__utility-actions"/);
  assert.match(home, /aria-label="บัญชี"/);
  assert.match(home, /aria-label="ย่อหน้าต่าง"/);
  assert.match(home, /aria-label="ซ่อนลงถาดระบบ"/);
  assert.match(home, /aria-label="ปิดแอป"/);
  assert.doesNotMatch(home, /class="desktop-shell__sidebar"/);
  assert.doesNotMatch(home, /<button[^>]*(เริ่มบันทึก|หยุดบันทึก)/);
  assert.doesNotMatch(home, /desktop-shell__appearance/);
});

test("studio dashboard uses real project metadata and preserves mounted surface owners", () => {
  const ready = renderShellWithProps({ project: state("ready", [projectA]) });
  assert.match(ready, /data-layout="studio"/);
  assert.match(ready, /studio-hero/);
  assert.match(ready, /studio-meeting/);
  assert.match(ready, /studio-activity/);
  assert.match(ready, /studio-task-pool/);
  assert.match(ready, /การประชุมครั้งถัดไป/);
  assert.match(ready, /วันและเวลา/);
  assert.match(ready, /ผู้เข้าร่วม/);
  assert.match(ready, /หัวข้อ/);
  assert.match(ready, /ยังไม่มีข้อมูลปฏิทิน/);
  assert.match(ready, /งานจากทุกการประชุม/);
  assert.match(ready, /<section hidden=""[^>]*>[\s\S]*?Surface content[\s\S]*?<\/section>/);
  assert.match(ready, /<strong>1<\/strong><small>โครงการทั้งหมด/);
  const unavailable = renderShellWithProps({ project: state("unavailable") });
  assert.match(unavailable, /<strong>—<\/strong><small>โครงการทั้งหมด/);
  const anotherProject = { ...projectA, id: "project-b", name: "Second project" };
  const populated = renderShellWithProps({ project: state("ready", [projectA, anotherProject]) });
  assert.match(populated, /<strong>2<\/strong><small>โครงการทั้งหมด/);
  assert.doesNotMatch(populated, /<meter/);
});

test("brand tint stays CSS-owned for explicit light/dark and system-effective themes", () => {
  assert.match(shellCssSource, /\.desktop-shell\s*\{[\s\S]*?--shell-brand:\s*var\(--fung-logo-mark\);/);
  assert.match(
    shellCssSource,
    /\.desktop-shell\[data-fung-root\]\.desktop-shell--dark\s*\{[\s\S]*?--shell-brand:\s*var\(--fung-logo-mark\);/,
  );
  const systemDarkRules = shellCssSource.match(
    /@media\s*\(prefers-color-scheme:\s*dark\)\s*\{[\s\S]*?\.desktop-shell--system\s*\{([\s\S]*?)\n  \}/,
  )?.[1];

  assert.ok(systemDarkRules, "system theme needs a dark media-query branch");
  assert.match(systemDarkRules, /color-scheme:\s*dark;/);
  const darkSystemTokens = designTokensCssSource.match(
    /@media\s*\(prefers-color-scheme:\s*dark\)\s*\{[\s\S]*?\[data-fung-root\]:not\(\[data-theme\]\),\s*\[data-fung-root\]\[data-theme="system"\]\s*\{([\s\S]*?)\n  \}/,
  )?.[1];
  assert.ok(darkSystemTokens, "system theme tokens need a dark media-query branch");
  assert.match(darkSystemTokens, /--fung-logo-mark:\s*#FAF8F3;/);
  const lightTokens = designTokensCssSource.match(
    /\[data-fung-root\]\[data-theme="light"\]\s*\{([\s\S]*?)\n\}/,
  )?.[1];
  const darkTokens = designTokensCssSource.match(
    /\[data-fung-root\]\[data-theme="dark"\]\s*\{([\s\S]*?)\n\}/,
  )?.[1];
  assert.ok(lightTokens, "light theme tokens need an explicit theme block");
  assert.ok(darkTokens, "dark theme tokens need an explicit root block");
  assert.match(lightTokens, /--fung-logo-mark:\s*#171918;/);
  assert.match(darkTokens, /--fung-logo-mark:\s*#FAF8F3;/);
  assert.match(shellCssSource, /\.desktop-shell__brand\s*\{[\s\S]*color:\s*var\(--shell-brand\);/);
  assert.match(shellCssSource, /\.desktop-shell__brand-mark\s*\{[\s\S]*color:\s*currentColor;/);
  assert.match(shellCssSource, /\.desktop-shell__brand-wordmark\s*\{[\s\S]*color:\s*currentColor;/);

  const systemMarkup = renderToStaticMarkup(
    React.createElement(DesktopShell, createProps({ theme: "system" })),
  );
  assert.match(systemMarkup, /data-theme="system"/);
  assert.doesNotMatch(systemMarkup, /wordmarkColor|variant=|style="[^"]*color/);
});

test("project loading, error, empty, and unavailable states remain distinct", () => {
  const error = { code: "STORAGE_READ_FAILED", message: "อ่านข้อมูลไม่ได้", retryable: true };
  const cases = [
    ["loading", state("loading"), "กำลังอ่านโครงการในเครื่อง"],
    ["error", state("error", null, error), "อ่านโครงการไม่สำเร็จ"],
    ["empty", state("ready", []), "ยังไม่มีโครงการในเครื่อง"],
    ["unavailable", state("unavailable"), "รายการโครงการยังไม่พร้อมใช้งาน"],
  ];

  for (const [name, projectState, expectedText] of cases) {
    assert.equal(resolveProjectPanelState(projectState), name);
    const markup = renderToStaticMarkup(
      React.createElement(DesktopShell, createProps({ project: projectState })),
    );
    assert.match(markup, new RegExp(expectedText));
  }

  const errorMarkup = renderToStaticMarkup(
    React.createElement(DesktopShell, createProps({ project: state("error", null, error) })),
  );
  assert.doesNotMatch(errorMarkup, /ยังไม่มีโครงการในเครื่อง/);
});

test("navigation guard presents the three choices, focuses Stay by default, and keeps normalized failure visible", () => {
  const dialogMarkup = renderToStaticMarkup(
    React.createElement(NavigationGuardDialog, {
      dialogRef: { current: null },
      destinationLabel: "บันทึกย้อนหลัง / ทบทวน",
      busy: false,
      error: null,
      onKeyDown: () => {},
      onContinue: () => {},
      onStop: () => {},
      onStay: () => {},
    }),
  );
  assert.match(dialogMarkup, /role="dialog"/);
  assert.match(dialogMarkup, /aria-modal="true"/);
  assert.match(dialogMarkup, /data-guard-default="true"/);
  assert.match(dialogMarkup, /อยู่หน้านี้/);
  assert.match(dialogMarkup, /อัดต่อและออกจากหน้านี้/);
  assert.match(dialogMarkup, /หยุดแล้วออก/);
  assert.match(dialogMarkup, /Esc จะอยู่หน้านี้และคืนโฟกัสให้ปุ่มเดิม/);

  const failedMarkup = renderToStaticMarkup(
    React.createElement(NavigationGuardDialog, {
      dialogRef: { current: null },
      destinationLabel: "ตั้งค่า",
      busy: false,
      error: normalizeShellError(new Error("raw path token stderr")),
      onKeyDown: () => {},
      onContinue: () => {},
      onStop: () => {},
      onStay: () => {},
    }),
  );
  assert.match(failedMarkup, /หยุดบันทึกยังไม่สำเร็จ/);
  assert.match(failedMarkup, /Desktop command failed\./);
  assert.doesNotMatch(failedMarkup, /raw path token stderr/);
  assert.match(failedMarkup, /เซสชันยังอยู่หน้านี้/);
});

await rm(runtimeDir, { recursive: true, force: true });
