import assert from "node:assert/strict";
import { access, readFile, writeFile } from "node:fs/promises";
import test from "node:test";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
const builder = resolve(root, "scripts/design_system.mjs");
const system = resolve(root, "docs/design-system/index.html");
const brand = resolve(root, "docs/design-system/brand/index.html");
const legacySystem = resolve(root, "docs/Mobile/design-system/index.html");
const legacyBrand = resolve(root, "docs/Mobile/design-system/brand/index.html");

function run(mode) {
  execFileSync(process.execPath, [builder, mode], { cwd: root, encoding: "utf8" });
}

test("shared board and brand presentation are generated from the DTCG token source", async () => {
  run("render");
  const [systemHtml, brandHtml, legacySystemHtml, legacyBrandHtml] = await Promise.all([
    readFile(system, "utf8"), readFile(brand, "utf8"), readFile(legacySystem, "utf8"), readFile(legacyBrand, "utf8"),
  ]);
  assert.match(systemHtml, /DTCG 2025\.10/);
  assert.match(systemHtml, /Cross-surface guidance/);
  assert.match(systemHtml, /Full token catalog/);
  assert.match(systemHtml, /data-token-search/);
  assert.match(systemHtml, /Export \/ Print/);
  assert.match(brandHtml, /Quiet Archive · beta/);
  assert.match(brandHtml, /quiet-archive-mark-white\.svg/);
  assert.match(legacySystemHtml, /\.\.\/\.\.\/design-system\/index\.html/);
  assert.match(legacyBrandHtml, /\.\.\/\.\.\/\.\.\/design-system\/brand\/index\.html/);

  for (const html of [systemHtml, brandHtml]) {
    for (const [, asset] of html.matchAll(/(?:src|href)="(\.\.[^"]+\.svg)"/g)) {
      await access(resolve(dirnameFor(html, systemHtml, brandHtml), asset));
    }
  }
});

function dirnameFor(html, systemHtml, brandHtml) {
  return html === systemHtml ? resolve(root, "docs/design-system") : resolve(root, "docs/design-system/brand");
}

test("design-system check detects stale generated HTML without rewriting it", async () => {
  run("render");
  const original = await readFile(system, "utf8");
  await writeFile(system, `${original}\n<!-- stale -->\n`, "utf8");
  assert.throws(() => run("check"));
  assert.match(await readFile(system, "utf8"), /<!-- stale -->/);
  run("render");
  run("check");
});
