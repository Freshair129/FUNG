// @req NFR-101
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

/**
 * The egress register (docs/appendices/E-egress-register.md) is only worth
 * anything if it stays true. A document listing outbound paths rots the first
 * time someone adds one, and it rots silently — nothing about writing a new
 * `reqwest` call reminds you a register exists.
 *
 * So these tests derive the paths from the source rather than restating them,
 * and fail when the code grows a way out that the register does not name.
 */

const REGISTER_PATH = "docs/appendices/E-egress-register.md";
const register = readFileSync(REGISTER_PATH, "utf8");

/**
 * Rust source with the test module cut off. Tests bind loopback listeners and
 * point clients at them constantly; counting those would drown the production
 * signal. Cut at the test *module*, not at any `#[cfg(test)]` attribute —
 * test-only constructors appear above it.
 */
function productionRust(path) {
  const source = readFileSync(path, "utf8");
  const testModule = source.search(/#\[cfg\(test\)\]\s*\nmod /);
  return testModule === -1 ? source : source.slice(0, testModule);
}

const rustModules = readdirSync("src-tauri/src")
  .filter((name) => name.endsWith(".rs"))
  .map((name) => ({ name: name.replace(/\.rs$/, ""), source: productionRust(`src-tauri/src/${name}`) }));

test("every module that can reach the network is named in the register", () => {
  // An HTTP client or an outbound socket connect. Anything matching this is a
  // way for bytes to leave the machine, and the register must account for it.
  const reaches = rustModules.filter(
    ({ source }) =>
      /reqwest::blocking::Client/.test(source) ||
      /TcpStream::connect/.test(source) ||
      /UdpSocket::bind/.test(source),
  );
  assert.ok(reaches.length >= 5, "expected to find the known outbound modules");

  const undeclared = reaches
    .map(({ name }) => name)
    .filter((name) => !register.includes(`${name}.rs`) && !register.includes(`${name}::`));
  assert.deepEqual(
    undeclared,
    [],
    `${undeclared.join(", ")} can reach the network but ${REGISTER_PATH} does not mention it — ` +
      "declare the path (payload, destination, consent gate) or explain why it is not egress",
  );
});

test("the webview cannot reach a remote host", () => {
  // The register's claim that all egress is Rust-side rests on two things:
  // no client-side HTTP anywhere, and a CSP that would stop one if it appeared.
  const frontend = readdirSync("src", { recursive: true })
    .filter((name) => /\.tsx?$/.test(String(name)))
    .map((name) => readFileSync(`src/${name}`, "utf8"))
    .join("\n");
  assert.doesNotMatch(frontend, /\bfetch\(/, "the frontend must not make its own network calls");
  assert.doesNotMatch(frontend, /new WebSocket\(/);
  assert.doesNotMatch(frontend, /XMLHttpRequest/);

  const csp = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).app.security.csp;
  const connect = csp
    .split(";")
    .map((directive) => directive.trim())
    .find((directive) => directive.startsWith("connect-src"));
  assert.ok(connect, "connect-src must be set explicitly, not left to default-src");
  for (const source of connect.split(/\s+/).slice(1)) {
    assert.ok(
      source === "ipc:" || source === "'self'" || /^https?:\/\/127\.0\.0\.1(:\*)?$/.test(source),
      `connect-src allows ${source}, which is not loopback or IPC`,
    );
  }
});

test("TLS is verified and cannot be turned off", () => {
  const manifest = readFileSync("src-tauri/Cargo.toml", "utf8");
  const reqwest = manifest.split("\n").find((line) => line.startsWith("reqwest ="));
  assert.match(reqwest, /default-features = false/);
  assert.match(reqwest, /"rustls-tls"/, "the TLS backend must be pinned, not inherited");

  for (const { name, source } of rustModules) {
    assert.doesNotMatch(
      source,
      /danger_accept_invalid/,
      `${name}.rs disables certificate verification`,
    );
  }
});

test("cloud dispatch is reachable only through the tier policy", () => {
  // The register says cloud is off by default and gated. That holds only if
  // the dispatch functions have no second caller that skips the decision.
  const defaults = productionRust("src-tauri/src/policy.rs");
  const impl = defaults.slice(defaults.indexOf("impl Default for TierPolicy"));
  for (const field of ["stt_cloud_enabled", "llm_cloud_enabled"]) {
    assert.ok(
      new RegExp(`${field}: false`).test(impl),
      `${field} must default to false — cloud is opt-in, not opt-out`,
    );
  }

  const gated = rustModules
    .filter(({ source }) => /policy::decide_cloud_tier/.test(source))
    .map(({ name }) => name)
    .sort();
  assert.deepEqual(
    gated,
    ["cloud_executor", "fungwire_server"],
    "a new module consults the cloud tier policy — add it to the register",
  );

  const callers = rustModules
    .filter(({ name, source }) => name !== "cloud_executor" && /cloud_executor::dispatch_(stt|llm)\b/.test(source))
    .map(({ name }) => name);
  assert.deepEqual(
    callers,
    ["fungwire_server"],
    "dispatch_stt/dispatch_llm must not be called from anywhere that skips decide_cloud_tier",
  );
});

test("the transcription worker is pinned offline, not merely expected to be", () => {
  // Register §3.1. faster-whisper's own default is to download `small` from
  // huggingface.co, so "it loads a bundled model" has to be enforced by the
  // environment rather than by a comment.
  const lib = productionRust("src-tauri/src/lib.rs");
  assert.match(lib, /HF_HUB_OFFLINE/, "a worker given no HF cache must be told it has none");
  const branch = lib.slice(lib.indexOf("match hf_home {"), lib.indexOf("HF_HUB_OFFLINE"));
  assert.match(
    branch,
    /None =>/,
    "the offline pin must be the None arm — the diarization worker legitimately needs the hub",
  );
});

test("the Zoom bearer is never sent to a host from a response body", () => {
  // Register §3.3. `download_url` is attacker-shaped input if Zoom is ever
  // wrong; every call must be preceded by the host check.
  const zoom = productionRust("src-tauri/src/zoom_sync.rs");
  const downloads = [...zoom.matchAll(/download_to_file\(/g)];
  assert.ok(downloads.length >= 2, "expected the mixed and per-participant downloads");
  for (const match of downloads) {
    if (zoom.slice(match.index - 40, match.index).includes("fn ")) continue; // the definition
    const preceding = zoom.slice(Math.max(0, match.index - 300), match.index);
    assert.match(
      preceding,
      /is_zoom_download_url\(/,
      "a download_to_file call is not guarded by is_zoom_download_url",
    );
  }
});
