import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  DESKTOP_RELEASE_DOWNLOAD_URL,
  DESKTOP_RELEASE_SIZE_LABEL,
  DESKTOP_RELEASE_VERSION,
} from "../src/lib/release.ts";

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));

test("desktop release metadata points to the stable latest Windows asset", () => {
  assert.equal(DESKTOP_RELEASE_VERSION, "0.1.1");
  assert.equal(
    DESKTOP_RELEASE_DOWNLOAD_URL,
    "https://github.com/Freshair129/FUNG-Releases/releases/latest/download/FUNG-windows-x64-setup.exe",
  );
  assert.doesNotMatch(DESKTOP_RELEASE_DOWNLOAD_URL, /Freshair129\/FUNG\/releases/);
});

test("Tauri release resources include the live worker and portable runtime", async () => {
  const config = await readJson(new URL("../src-tauri/tauri.conf.json", import.meta.url));
  const resources = config.bundle.resources;

  assert.equal(resources["../.venv-whisper"], ".venv-whisper");
  assert.equal(resources["../.venv-whisper-transformers-candidate"], undefined);
  assert.equal(resources["../scripts/transformers-candidate-requirements.txt"], undefined);
  assert.equal(resources["../scripts/transcribe.py"], "scripts/transcribe.py");
  assert.equal(resources["../scripts/transcribe_live.py"], "scripts/transcribe_live.py");
  assert.equal(
    resources["../scripts/transcribe_transformers.py"],
    "scripts/transcribe_transformers.py",
  );
  assert.equal(
    resources["../scripts/transcribe_transformers_live.py"],
    "scripts/transcribe_transformers_live.py",
  );
});

test("package and Tauri versions agree with the public release", async () => {
  const packageJson = await readJson(new URL("../package.json", import.meta.url));
  const tauriConfig = await readJson(new URL("../src-tauri/tauri.conf.json", import.meta.url));
  const cargoToml = await readFile(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");

  assert.equal(packageJson.version, DESKTOP_RELEASE_VERSION);
  assert.equal(tauriConfig.version, DESKTOP_RELEASE_VERSION);
  assert.match(cargoToml, /^version = "0\.1\.1"$/m);
});

test("release.ts version matches src-tauri/tauri.conf.json (no drift between the two sources of truth)", async () => {
  const tauriConfig = await readJson(new URL("../src-tauri/tauri.conf.json", import.meta.url));

  assert.equal(
    DESKTOP_RELEASE_VERSION,
    tauriConfig.version,
    `DESKTOP_RELEASE_VERSION ("${DESKTOP_RELEASE_VERSION}") in src/lib/release.ts is out of sync with ` +
      `"version" ("${tauriConfig.version}") in src-tauri/tauri.conf.json`,
  );
});

test("landing page exposes a Windows download CTA and unsigned beta notice", async () => {
  const source = await readFile(new URL("../src/landing/LandingPage.tsx", import.meta.url), "utf8");

  assert.match(source, /DESKTOP_RELEASE_DOWNLOAD_URL/);
  assert.match(source, /DESKTOP_RELEASE_SIZE_LABEL/);
  assert.match(source, /ดาวน์โหลด FUNG สำหรับ Windows/);
  assert.match(source, /SmartScreen/);
  // The download size must come from the shared constant, not a hardcoded literal.
  assert.doesNotMatch(source, /\d+(\.\d+)?\s*MB/);
});

test("desktop release size label is defined once, next to the version", async () => {
  assert.match(DESKTOP_RELEASE_SIZE_LABEL, /^\d+(\.\d+)?\s*(MB|GB)$/);
});

test("portable runtime staging is pinned and bundles a local model", async () => {
  const source = await readFile(new URL("../scripts/stage_whisper_runtime.ps1", import.meta.url), "utf8");

  assert.match(source, /PythonVersion\s*=\s*'3\.11\.9'/);
  assert.match(
    source,
    /\$pythonSha256 = '009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b'/,
    "operational staging must pin the verified Python 3.11.9 embedded archive",
  );
  assert.match(source, /FasterWhisperVersion\s*=\s*'1\.2\.1'/);
  assert.match(source, /\[string\]\$Model\s*=\s*'large-v3-turbo'/);
  assert.match(source, /\[ValidateSet\(\s*'small'\s*,\s*'large-v3-turbo'\s*,\s*'medium'\s*,\s*'large-v3'\s*\)\]/);
  assert.match(source, /manifest\.json/);
  assert.match(source, /SHA256/);
});

test("Thai Transformers candidate staging is pinned, transactional, and space-aware", async () => {
  const source = await readFile(
    new URL("../scripts/stage_whisper_transformers_candidate.ps1", import.meta.url),
    "utf8",
  );

  assert.match(source, /b751db1e8dbfee6561de22ca99fe070282fcf459/);
  assert.match(source, /pytorch_model\.bin/);
  assert.match(source, /e1e0b5b4c9a89d7d60fb795448c3102e07af87fa73c5fce7c0206c6bd99a7e7b/);
  assert.match(source, /pythonVersion\s*=\s*'3\.11\.9'/);
  assert.match(source, /python311\._pth/);
  assert.match(source, /transformers-candidate-requirements\.txt/);
  assert.match(source, /--require-hashes/);
  assert.match(source, /SafetyMarginBytes/);
  assert.match(source, /Move-Item -LiteralPath \$stagingRoot/);
  assert.match(source, /Remove-Item -LiteralPath \$resolvedStagingRoot -Recurse -Force/);
  assert.match(source, /torch = \$dependencyInfo\.torch/);
  assert.match(source, /runtimePackages = \$dependencyInfo\.packages/);
  assert.match(source, /lockfileSha256/);
  assert.match(source, /ConvertTo-Json/);

  const worker = await readFile(
    new URL("../scripts/transcribe_transformers.py", import.meta.url),
    "utf8",
  );
  assert.match(worker, /low_cpu_mem_usage=True/);

  const requirements = await readFile(
    new URL("../scripts/transformers-candidate-requirements.txt", import.meta.url),
    "utf8",
  );
  const rust = await readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const pythonArchiveSha256 =
    "009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b";
  const lockHash = createHash("sha256").update(requirements).digest("hex");
  assert.ok(
    source.includes(`$pythonSha256 = '${pythonArchiveSha256}'`),
    "candidate staging must retain the verified embedded-Python archive digest",
  );
  assert.ok(
    rust.includes(pythonArchiveSha256),
    "Rust readiness must pin the staged embedded-Python archive digest",
  );
  assert.match(requirements, /torch==2\.14\.0\+cpu/);
  assert.match(requirements, /transformers==5\.17\.0/);
  assert.match(requirements, /accelerate==1\.15\.0/);
  assert.match(requirements, /--hash=sha256:/);
  assert.ok(rust.includes(lockHash), "Rust readiness must pin the candidate dependency lock hash");
});

test("the isolated PDF parser installs the fixed wheel from its SHA-256 pin", async () => {
  const requirements = await readFile("scripts/knowledge-extraction-requirements.txt", "utf8");
  const staging = await readFile("scripts/stage_knowledge_parser_runtime.ps1", "utf8");
  const rust = await readFile("src-tauri/src/meeting_knowledge_windows.rs", "utf8");

  assert.match(requirements, /^pypdf==6\.16\.1 --hash=sha256:[0-9a-f]{64}$/m);
  assert.match(staging, /--require-hashes/);
  assert.match(staging, /pypdfVersion = "6\.16\.1"/);
  assert.match(rust, /manifest\.pypdf_version != "6\.16\.1"/);
});
