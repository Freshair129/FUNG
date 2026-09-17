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

const stripYamlComment = (line) => {
  let singleQuoted = false;
  let doubleQuoted = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (character === "'" && !doubleQuoted) {
      if (singleQuoted && line[index + 1] === "'") {
        index += 1;
      } else {
        singleQuoted = !singleQuoted;
      }
    } else if (character === '"' && !singleQuoted && line[index - 1] !== "\\") {
      doubleQuoted = !doubleQuoted;
    } else if (character === "#" && !singleQuoted && !doubleQuoted && (index === 0 || /\s/.test(line[index - 1]))) {
      return line.slice(0, index);
    }
  }
  return line;
};

const extractNpmRunCommands = (workflowText) => {
  const commands = [];
  const unresolved = [];
  const runPrefix = /\bnpm\s+run\b/g;
  for (const rawLine of workflowText.split(/\r?\n/)) {
    const line = stripYamlComment(rawLine);
    for (const match of line.matchAll(runPrefix)) {
      const tail = line.slice(match.index + match[0].length).trim();
      const script = tail.match(/^(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9][A-Za-z0-9:_-]*))(?=\s|$)/);
      if (!script) {
        unresolved.push(tail);
        continue;
      }
      commands.push(script[1] ?? script[2] ?? script[3]);
    }
  }
  return { commands, unresolved };
};

const findMissingScripts = (commands, availableScripts) =>
  [...new Set(commands)].filter((name) => !(name in availableScripts));

const findOrphanTestFiles = (testFiles, availableScripts) => {
  const commandText = Object.values(availableScripts).join("\n");
  return testFiles
    .filter((name) => /\.test\.\w+$/.test(name))
    .filter((name) => !commandText.includes(`tests/${name}`));
};

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
  const orphans = findOrphanTestFiles(readdirSync("tests"), scripts);
  assert.deepEqual(orphans, [], `${orphans.join(", ")} is never run by any npm script`);
});

test("every npm run command in CI resolves to a package script", () => {
  const { commands, unresolved } = extractNpmRunCommands(workflow);
  assert.deepEqual(unresolved, [], "CI contains an npm run command without a static script name");
  const missing = findMissingScripts(commands, scripts);
  assert.deepEqual(
    missing,
    [],
    `${missing.join(", ")} invoked by .github/workflows/ci.yml but not defined in package.json`,
  );
});

test("CI command inventory keeps exact names across shell, quoting, and comments", () => {
  const fixture = [
    "steps:",
    "  - run: npm run 'test:auth' -- --watch && npm run build",
    "  - run: npm run test:stale # npm run test:commented",
    "  - run: |",
    "      # npm run test:commented",
    "      npm run test:auth-extra",
    "      npm run ${DYNAMIC_SCRIPT}",
  ].join("\n");
  const inventory = extractNpmRunCommands(fixture);
  assert.deepEqual(inventory.commands, ["test:auth", "build", "test:stale", "test:auth-extra"]);
  assert.deepEqual(inventory.unresolved, ["${DYNAMIC_SCRIPT}"]);
  assert.deepEqual(
    findMissingScripts(inventory.commands, { "test:auth": "node auth", build: "vite build" }),
    ["test:stale", "test:auth-extra"],
  );
  assert.deepEqual(
    findOrphanTestFiles(["auth.test.mjs", "nativeSessionCustody.test.mjs"], {
      "test:auth": "tests/auth.test.mjs",
    }),
    ["nativeSessionCustody.test.mjs"],
  );
});
