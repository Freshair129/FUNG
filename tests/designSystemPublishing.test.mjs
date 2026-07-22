import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import test from "node:test";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const renderer = resolve(root, "scripts/render_mobile_design_system.mjs");
const system = resolve(root, "docs/Mobile/design-system/index.html");
const brand = resolve(root, "docs/Mobile/design-system/brand/index.html");

function run(mode) {
  execFileSync(process.execPath, [renderer, mode], { cwd: root, encoding: "utf8" });
}

test("renderer creates both interactive pages from the Markdown SOT", async () => {
  run("render");
  const [systemHtml, brandHtml] = await Promise.all([readFile(system, "utf8"), readFile(brand, "utf8")]);
  assert.match(systemHtml, /generated from Markdown SOT/);
  assert.match(brandHtml, /Quiet Archive selected/);
  assert.match(brandHtml, /quiet-archive-mark-white\.svg/);
  assert.match(brandHtml, /Export \/ Print/);
});

test("validation detects manually stale generated HTML", async () => {
  run("render");
  const original = await readFile(system, "utf8");
  await writeFile(system, `${original}\n<!-- stale -->\n`, "utf8");
  assert.throws(() => run("check"), /Mobile design system check failed/);
  await writeFile(system, original, "utf8");
  run("check");
});
