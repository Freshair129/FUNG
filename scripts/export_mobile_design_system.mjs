import { access, mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn } from "node:child_process";
import { tmpdir } from "node:os";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = new Set(process.argv.slice(2));
const target = args.has("--system") ? "system" : "brand";
const format = args.has("--pdf") ? "pdf" : "png";
const browserCandidates = [
  process.env.CHROME_PATH,
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
].filter(Boolean);
const source = target === "brand"
  ? resolve(root, "docs/Mobile/design-system/brand/index.html")
  : resolve(root, "docs/Mobile/design-system/index.html");
const output = resolve(root, `docs/Mobile/design-system/exports/fung-${target}-presentation.${format}`);

async function locateBrowser() {
  for (const candidate of browserCandidates) {
    try { await access(candidate); return candidate; } catch { /* try next */ }
  }
  throw new Error("Chrome or Edge was not found. Set CHROME_PATH to export the long image/PDF.");
}

function run(command, commandArgs) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, commandArgs, { stdio: "inherit" });
    child.once("error", reject);
    child.once("exit", (code) => code === 0 ? resolveRun() : reject(new Error(`Browser exited with code ${code}.`)));
  });
}

try {
  const browser = await locateBrowser();
  await mkdir(dirname(output), { recursive: true });
  const viewportHeight = target === "brand" ? 4600 : 4200;
  const profileDir = await mkdtemp(resolve(tmpdir(), "fung-design-system-export-"));
  const common = [
    "--headless",
    "--disable-gpu",
    "--disable-gpu-compositing",
    "--disable-3d-apis",
    "--disable-webgl",
    "--use-gl=disabled",
    "--in-process-gpu",
    "--disable-features=UseSkiaRenderer,UseGraphite,UseDawn,Vulkan",
    "--hide-scrollbars",
    "--no-first-run",
    "--no-default-browser-check",
    `--user-data-dir=${profileDir}`,
    `--window-size=1440,${viewportHeight}`,
  ];
  const capture = format === "pdf" ? `--print-to-pdf=${output}` : `--screenshot=${output}`;
  try {
    await run(browser, [...common, capture, `${pathToFileURL(source).href}?export=1`]);
  } finally {
    await rm(profileDir, { recursive: true, force: true });
  }
  console.log(`Exported ${target} ${format}: ${output}`);
} catch (error) {
  console.error(`Design-system export failed: ${error.message}`);
  process.exitCode = 1;
}
