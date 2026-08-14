import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  DESKTOP_RELEASE_DOWNLOAD_URL,
  DESKTOP_RELEASE_VERSION,
} from "../src/lib/release.ts";

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));

test("desktop release metadata points to the stable latest Windows asset", () => {
  assert.equal(DESKTOP_RELEASE_VERSION, "0.1.0");
  assert.equal(
    DESKTOP_RELEASE_DOWNLOAD_URL,
    "https://github.com/Freshair129/FUNG/releases/latest/download/FUNG-windows-x64-setup.exe",
  );
});

test("Tauri release resources include the live worker and portable runtime", async () => {
  const config = await readJson(new URL("../src-tauri/tauri.conf.json", import.meta.url));
  const resources = config.bundle.resources;

  assert.equal(resources["../.venv-whisper"], ".venv-whisper");
  assert.equal(resources["../scripts/transcribe.py"], "scripts/transcribe.py");
  assert.equal(resources["../scripts/transcribe_live.py"], "scripts/transcribe_live.py");
});

test("package and Tauri versions agree with the public release", async () => {
  const packageJson = await readJson(new URL("../package.json", import.meta.url));
  const tauriConfig = await readJson(new URL("../src-tauri/tauri.conf.json", import.meta.url));
  const cargoToml = await readFile(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");

  assert.equal(packageJson.version, DESKTOP_RELEASE_VERSION);
  assert.equal(tauriConfig.version, DESKTOP_RELEASE_VERSION);
  assert.match(cargoToml, /^version = "0\.1\.0"$/m);
});

test("landing page exposes a Windows download CTA and unsigned beta notice", async () => {
  const source = await readFile(new URL("../src/landing/LandingPage.tsx", import.meta.url), "utf8");

  assert.match(source, /DESKTOP_RELEASE_DOWNLOAD_URL/);
  assert.match(source, /ดาวน์โหลด FUNG สำหรับ Windows/);
  assert.match(source, /491 MB/);
  assert.match(source, /SmartScreen/);
});

test("portable runtime staging is pinned and bundles a local model", async () => {
  const source = await readFile(new URL("../scripts/stage_whisper_runtime.ps1", import.meta.url), "utf8");

  assert.match(source, /PythonVersion\s*=\s*'3\.11\.9'/);
  assert.match(source, /FasterWhisperVersion\s*=\s*'1\.2\.1'/);
  assert.match(source, /Model\s*=\s*'small'/);
  assert.match(source, /manifest\.json/);
  assert.match(source, /SHA256/);
});
