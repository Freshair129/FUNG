// @req NFR-110
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

/**
 * A test suite that exists and never runs is worse than no suite at all: it
 * reads as coverage in `package.json` and in any listing of the tests, while
 * being free to rot. Nothing about writing a new suite reminds you to wire it
 * into CI, and nothing about CI passing tells you a suite was skipped —
 * a green run reports on what it was asked to do, not on what exists.
 *
 * This has now been wrong twice. Six suites were found unwired at once, the
 * comment in `ci.yml` was updated to say "every suite runs", and
 * `test:release` was missed in the very same sweep. The claim needs a check,
 * not a better comment.
 */

const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
const scripts = JSON.parse(readFileSync("package.json", "utf8")).scripts;

test("every test script in package.json runs in CI", () => {
  // Derived from package.json rather than listed here, so a suite added later
  // fails this test instead of quietly never executing.
  const suites = Object.keys(scripts).filter((name) => name.startsWith("test:"));
  assert.ok(suites.length >= 13, "expected to find the known suites");

  const unwired = suites.filter((name) => !workflow.includes(`npm run ${name}`));
  assert.deepEqual(
    unwired,
    [],
    `${unwired.join(", ")} defined in package.json but never invoked by .github/workflows/ci.yml — ` +
      "wire it in, or delete it if it is not meant to gate anything",
  );
});

test("every suite file has a script that runs it", () => {
  // The other direction: a `.test.mjs` nobody can invoke is dead weight that
  // still looks like coverage to anyone reading the directory. Matched by
  // `.test.<ext>` rather than hardcoding `.mjs`, so a suite written in any
  // other language (e.g. `.test.py`) can't slip through the same gap that
  // let `tests/transcribeConcatOnly.test.py` go unwired.
  const commands = Object.values(scripts).join("\n");
  const orphans = readdirSync("tests")
    .filter((name) => /\.test\.\w+$/.test(name))
    .filter((name) => !commands.includes(`tests/${name}`));
  assert.deepEqual(orphans, [], `${orphans.join(", ")} is never run by any npm script`);
});
