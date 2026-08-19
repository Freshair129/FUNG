// @req FR-102
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

/**
 * GAP-20 was not a missing feature — the worker and the merge path were
 * written and unit-tested. It was that `diarize.py` was absent from
 * `bundle.resources`, so no installed build had the script to run. That is
 * exactly the kind of defect no unit test catches and no developer hits,
 * because the source tree always has the file.
 */

const tauriConf = JSON.parse(
  readFileSync("src-tauri/tauri.conf.json", "utf8"),
);
const resources = tauriConf.bundle.resources;

test("every worker script the backend shells out to is in the bundle", () => {
  // Derived from the Rust side rather than listed here, so a fourth worker
  // added later fails this test instead of silently shipping without its
  // script — which is the whole of GAP-20.
  // Production code only: test modules shell out to stub scripts
  // (`fake_transcribe.py`) that deliberately never ship.
  const productionCode = (path) => {
    const source = readFileSync(path, "utf8");
    // Cut at the test *module*, not at any `#[cfg(test)]` attribute —
    // test-only constructors appear near the top of lib.rs, and cutting
    // there would hide the production references this test exists to find.
    const testModule = source.search(/#\[cfg\(test\)\]\s*\nmod /);
    return testModule === -1 ? source : source.slice(0, testModule);
  };
  const rust = [
    "src-tauri/src/lib.rs",
    "src-tauri/src/diarization.rs",
    "src-tauri/src/fungwire_server.rs",
  ]
    .map(productionCode)
    .join("\n");
  const referenced = new Set(
    [...rust.matchAll(/join\("(\w+\.py)"\)/g)].map((match) => match[1]),
  );
  assert.ok(referenced.size >= 2, "expected to find worker script references");

  const bundled = new Set(
    Object.values(resources)
      .filter((target) => target.endsWith(".py"))
      .map((target) => target.split("/").pop()),
  );
  for (const script of referenced) {
    assert.ok(
      bundled.has(script),
      `${script} is shelled out to but not in bundle.resources — an installed build cannot run it`,
    );
  }
});

test("the diarization worker ships", () => {
  assert.equal(resources["../scripts/diarize.py"], "scripts/diarize.py");
  assert.ok(existsSync("scripts/diarize.py"));
});

test("the dependency tree is staged separately from the default bundle", () => {
  // torch is hundreds of megabytes and the model is gated per user, so
  // neither can go in the installer. The opt-in script is what makes the
  // feature reachable at all.
  assert.ok(existsSync("scripts/stage_diarization_runtime.ps1"));
  const staging = readFileSync("scripts/stage_diarization_runtime.ps1", "utf8");
  assert.match(
    staging,
    /--require-hashes/,
    "installs must be hash-pinned like the whisper runtime's",
  );
  assert.match(
    staging,
    /GenerateLock/,
    "the lockfile has to be generatable — its digests cannot be hand-written",
  );
});

test("the staging script refuses to install without a reviewed lockfile", () => {
  // Fabricated or absent hashes would be worse than none: the point of
  // --require-hashes is that the tree was resolved and reviewed once.
  const staging = readFileSync("scripts/stage_diarization_runtime.ps1", "utf8");
  assert.match(staging, /No pinned lockfile at/);
  assert.equal(
    existsSync("scripts/diarization-runtime-requirements.txt"),
    false,
    "the lockfile must be generated on a machine with network access, not committed unresolved",
  );
});

test("the app and the staging script agree on where the model cache lives", () => {
  // If they disagree, the operator downloads the model into a directory the
  // readiness probe never looks at, and the app reports it as missing.
  const staging = readFileSync("scripts/stage_diarization_runtime.ps1", "utf8");
  const rust = readFileSync("src-tauri/src/diarization.rs", "utf8");
  for (const source of [staging, rust]) {
    assert.match(source, /FUNG_HF_HOME/);
    assert.match(source, /huggingface/);
  }
});

test("diarizing is a real action, not a disabled button with a reason", () => {
  // It was dark because diarization was reachable only from Zoom import.
  // Now that a local capture can be diarized, a reason string would be a
  // leftover claiming a limit that no longer exists.
  const actions = readFileSync("src/lib/jobActions.ts", "utf8");
  assert.doesNotMatch(actions, /"speakers\.diarize":\s*"/);
  assert.match(actions, /"speakers\.diarize",/);
});

test("the shell checks readiness before queueing a diarization", () => {
  // The dependencies are opt-in and the model is gated, so this job can be
  // unrunnable for reasons the user can fix. Queueing anyway would surface
  // that as a job failure minutes later instead of at the click.
  const app = readFileSync("src/App.tsx", "utf8");
  assert.match(app, /plan\.jobType === "speakers\.diarize"/);
  assert.match(app, /await diarizationStatus\(\)/);
  assert.match(
    app,
    /readiness && !readiness\.available/,
    "a probe that could not run is not the same as one that said no",
  );
});

test("the readiness contract matches what Rust serialises", () => {
  const rust = readFileSync("src-tauri/src/diarization.rs", "utf8");
  const struct = rust.slice(
    rust.indexOf("pub(crate) struct DiarizationReadiness {"),
    rust.indexOf("/// Decides the single blocker"),
  );
  const camel = (name) =>
    name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
  const fields = [...struct.matchAll(/pub\(crate\) (\w+):/g)]
    .map((match) => camel(match[1]))
    .sort();

  const api = readFileSync("src/tauri.ts", "utf8");
  const type = api.slice(
    api.indexOf("export type DiarizationReadiness = {"),
    api.indexOf("export async function diarizationStatus"),
  );
  const declared = [...type.matchAll(/^  (\w+)[?]?:/gm)]
    .map((match) => match[1])
    .sort();
  assert.deepEqual(declared, fields);
});
