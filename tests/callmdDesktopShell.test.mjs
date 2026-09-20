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

const [shellSource, contractsSource, brandAssetSource, shellCssSource] = await Promise.all([
  readFile(path.join(worktreeRoot, "src/components/desktop/DesktopShell.tsx"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/contracts.ts"), "utf8"),
  readFile(path.join(worktreeRoot, "docs/brand-kit/logo/fung-mark.svg"), "utf8"),
  readFile(path.join(worktreeRoot, "src/components/desktop/DesktopShell.css"), "utf8"),
]);

await writeFile(path.join(runtimeDir, "contracts.mjs"), resolveRuntimeImports(transpile(contractsSource)), "utf8");
await writeFile(
  path.join(runtimeDir, "DesktopShell.mjs"),
  resolveRuntimeImports(transpile(shellSource))
    .replace('from "../../tauri.ts"', "")
    .replace('from "./contracts.ts"', 'from "./contracts.mjs"')
    .replace('from "./CompanionPanel"', 'from "./CompanionPanel.mjs"')
    .replace('import "./DesktopShell.css";', "")
    .replace('import "./LiquidGlass.css";', ""),
  "utf8",
);
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
    assert.match(markup, /พร้อมตรวจสอบ/, label);
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

test("SSR keeps Home, Live, History, and Output controlled by activeSurface with accessible navigation", () => {
  const homeMarkup = renderToStaticMarkup(React.createElement(DesktopShell, createProps()));
  assert.match(homeMarkup, /href="#home"[^>]*aria-current="page"/);
  assert.match(homeMarkup, /href="#live"/);
  assert.match(homeMarkup, /href="#history"/);
  assert.match(homeMarkup, /href="#output"/);
  assert.match(homeMarkup, /ข้ามไปยังเนื้อหาหลัก/);
  assert.match(homeMarkup, /เริ่มประชุม/);
  assert.match(homeMarkup, /จะเปิดประชุมสดก่อนเริ่มการบันทึกจริง/);
  assert.match(homeMarkup, /Surface content/);
  assert.doesNotMatch(homeMarkup, /พื้นที่ทำงานเดิม|callmd-legacy-workspace|fab-topbar|power-dock/);
  assert.match(homeMarkup, /data-surface="home"/);
  assert.doesNotMatch(homeMarkup, /aria-label="ย่อหน้าต่าง"|aria-label="ปิดหน้าต่าง"/);

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
  assert.match(outputMarkup, /href="#output"[^>]*aria-current="page"/);
  assert.match(outputMarkup, /กำหนดปลายทางไฟล์บันทึก/);
});

test("appearance is a separate active surface and not an inline sidebar disclosure", () => {
  const markup = renderToStaticMarkup(
    React.createElement(DesktopShell, createProps({ activeSurface: "appearance" })),
  );
  assert.match(markup, /data-surface="appearance"/);
  assert.match(markup, /ลักษณะและธีม/);
  assert.match(markup, /id="desktop-shell-theme"/);
  assert.match(markup, /id="desktop-shell-material"/);
  assert.match(markup, /id="desktop-shell-transparency"/);

  const sidebar = markup.match(/<aside[^>]*aria-label="บริบทงาน"[\s\S]*?<\/aside>/)?.[0];
  assert.ok(sidebar, "appearance render should keep the shell sidebar");
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

test("native window controls and drag boundaries stay explicit", () => {
  assert.match(shellSource, /<header className="desktop-shell__header" data-tauri-drag-region>/);
  assert.doesNotMatch(shellSource, /actions\.minimizeWindow|actions\.closeWindow/);
  assert.doesNotMatch(shellSource, /className="panel-glass" data-tauri-drag-region/);
  assert.doesNotMatch(shellSource, /className="fab fab-topbar" data-tauri-drag-region/);
  assert.doesNotMatch(shellSource, /desktop-shell__legacy-workspace|callmd-legacy-workspace|power-dock/);
  assert.doesNotMatch(shellSource, /HomeScreen|InstrumentRail|fab-topbar|power-dock/);
  assert.match(shellCssSource, /\.desktop-shell__header :where\(a, button, select, input, textarea\)/);
  assert.doesNotMatch(shellCssSource, /\.desktop-shell__header-button/);
});

test("the shell keeps truthful profile states and the hover rail owns desktop actions", () => {
  const signedOut = renderToStaticMarkup(React.createElement(DesktopShell, createProps()));
  assert.match(signedOut, /สมัคร \/ เข้าสู่ระบบ/);
  assert.match(signedOut, /desktop-shell__sidebar/);
  assert.doesNotMatch(signedOut, /<button[^>]*(เริ่มบันทึก|หยุดบันทึก)/);
  assert.match(signedOut, /นำเข้าไฟล์/);
  assert.match(signedOut, /ส่งออก/);
  assert.match(signedOut, /จับคู่อุปกรณ์/);
  assert.match(signedOut, /ลักษณะ/);
  assert.doesNotMatch(signedOut, /desktop-shell__appearance/);

  const signedIn = renderToStaticMarkup(
    React.createElement(
      DesktopShell,
      createProps({ accountStatus: { state: "authenticated", email: "owner@example.test" } }),
    ),
  );
  assert.match(signedIn, /owner@example\.test/);
  assert.doesNotMatch(signedIn, /สมัคร \/ เข้าสู่ระบบ/);
});

test("brand tint stays CSS-owned for explicit light/dark and system-effective themes", () => {
  assert.match(shellCssSource, /\.desktop-shell\s*\{[\s\S]*--shell-brand:\s*#171918;/);
  assert.match(shellCssSource, /\.desktop-shell--dark\s*\{[\s\S]*--shell-brand:\s*#faf8f3;/);
  const systemDarkRules = shellCssSource.match(
    /@media\s*\(prefers-color-scheme:\s*dark\)\s*\{[\s\S]*?\.desktop-shell--system\s*\{([\s\S]*?)\n  \}/,
  )?.[1];

  assert.ok(systemDarkRules, "system theme needs a dark media-query branch");
  assert.match(systemDarkRules, /--shell-brand:\s*#faf8f3;/);
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
