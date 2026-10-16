import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const mode = process.argv[2] ?? "render";
if (!new Set(["render", "check"]).has(mode)) {
  console.error(`Unknown design-system mode: ${mode}. Use render or check.`);
  process.exit(2);
}

function runPython(args) {
  const candidates = process.platform === "win32"
    ? [{ command: "python", prefix: [] }, { command: "py", prefix: ["-3"] }]
    : [{ command: "python3", prefix: [] }, { command: "python", prefix: [] }];
  let lastError;
  for (const candidate of candidates) {
    const result = spawnSync(candidate.command, [...candidate.prefix, ...args], { cwd: root, encoding: "utf8" });
    if (result.error?.code === "ENOENT") {
      lastError = result.error;
      continue;
    }
    if (result.stdout) process.stdout.write(result.stdout);
    if (result.stderr) process.stderr.write(result.stderr);
    return result.status ?? 1;
  }
  console.error(`Python 3 is required to build FUNG design tokens: ${lastError?.message ?? "not found"}`);
  return 1;
}

const tokenScript = resolve(root, "docs/FUNG_Design_System_v0.1.0/tokens/build_tokens.py");
const tokenArgs = [tokenScript, ...(mode === "check" ? ["--check"] : [])];
const tokenStatus = runPython(tokenArgs);
if (tokenStatus !== 0) process.exit(tokenStatus);

const renderer = resolve(root, "scripts/render_design_system.mjs");
const renderResult = spawnSync(process.execPath, [renderer, mode], { cwd: root, encoding: "utf8" });
if (renderResult.stdout) process.stdout.write(renderResult.stdout);
if (renderResult.stderr) process.stderr.write(renderResult.stderr);
if ((renderResult.status ?? 1) !== 0) process.exit(renderResult.status ?? 1);

const coverageAudit = resolve(root, "scripts/audit_design_system_coverage.mjs");
const coverageResult = spawnSync(process.execPath, [coverageAudit, mode], { cwd: root, encoding: "utf8" });
if (coverageResult.stdout) process.stdout.write(coverageResult.stdout);
if (coverageResult.stderr) process.stderr.write(coverageResult.stderr);
process.exit(coverageResult.status ?? 1);
