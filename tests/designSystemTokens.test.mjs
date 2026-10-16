import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";
import { dirname, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const tokenPath = resolve(root, "docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.json");
const allowedTypes = new Set([
  "color", "dimension", "fontFamily", "fontWeight", "duration", "cubicBezier", "number", "shadow",
]);

function walk(node, path = [], inheritedType = undefined, tokens = []) {
  assert.ok(node && typeof node === "object" && !Array.isArray(node), `${path.join(".") || "root"} must be an object`);
  const type = node.$type ?? inheritedType;
  if ("$value" in node) {
    assert.ok(type, `${path.join(".")} must declare or inherit $type`);
    assert.ok(allowedTypes.has(type), `${path.join(".")} has unsupported type ${type}`);
    assert.ok("$description" in node, `${path.join(".")} should explain its purpose`);
    tokens.push({ path: path.join("."), type, value: node.$value });
    return tokens;
  }
  for (const [name, value] of Object.entries(node)) {
    if (name.startsWith("$")) {
      assert.ok(["$type", "$description", "$extensions"].includes(name), `unexpected DTCG property ${name}`);
      continue;
    }
    walk(value, [...path, name], type, tokens);
  }
  return tokens;
}

test("canonical FUNG tokens use typed DTCG groups and FUNG namespaced extensions", async () => {
  const data = JSON.parse(await readFile(tokenPath, "utf8"));
  assert.equal("$schemaNote" in data, false);
  assert.ok(data.$extensions?.["com.fung.design-system"]);
  const extension = data.$extensions["com.fung.design-system"];
  assert.equal(extension.metadata?.format, "DTCG 2025.10");
  assert.equal(extension.metadata?.implementationStatus?.tokenCssIntegration, "verified-in-repository");
  assert.equal(extension.metadata?.implementationStatus?.componentCoverage, "partial");
  assert.equal(extension.metadata?.implementationStatus?.accessibilityCertification, false);
  const tokens = walk(data);
  assert.ok(tokens.length >= 100, `expected a complete shared token set, found ${tokens.length}`);

  for (const token of tokens) {
    if (token.type === "color") {
      const value = token.value;
      assert.equal(value.colorSpace, "srgb", `${token.path} must use sRGB`);
      assert.equal(value.components.length, 3, `${token.path} must have three color components`);
      assert.ok(value.components.every((component) => component >= 0 && component <= 1), `${token.path} components must be normalized`);
      assert.ok(value.alpha === undefined || (value.alpha >= 0 && value.alpha <= 1), `${token.path} alpha must be normalized`);
    } else if (token.type === "dimension") {
      assert.ok(Number.isFinite(token.value.value), `${token.path} must have a numeric dimension`);
      assert.ok(["px", "rem"].includes(token.value.unit), `${token.path} must use px or rem`);
    } else if (token.type === "duration") {
      assert.ok(Number.isFinite(token.value.value), `${token.path} must have a numeric duration`);
      assert.ok(["ms", "s"].includes(token.value.unit), `${token.path} must use ms or s`);
    } else if (token.type === "fontFamily") {
      assert.ok(typeof token.value === "string" || (Array.isArray(token.value) && token.value.every((family) => typeof family === "string")));
    } else if (token.type === "fontWeight") {
      assert.ok((typeof token.value === "number" && token.value >= 1 && token.value <= 1000) || typeof token.value === "string");
    } else if (token.type === "cubicBezier") {
      assert.equal(token.value.length, 4, `${token.path} must have four control points`);
      assert.ok(token.value[0] >= 0 && token.value[0] <= 1 && token.value[2] >= 0 && token.value[2] <= 1, `${token.path} x control points must be in [0, 1]`);
    } else if (token.type === "number") {
      assert.ok(Number.isFinite(token.value), `${token.path} must be a finite number`);
    } else if (token.type === "shadow") {
      const shadows = Array.isArray(token.value) ? token.value : [token.value];
      for (const shadow of shadows) {
        for (const part of ["color", "offsetX", "offsetY", "blur", "spread"]) assert.ok(part in shadow, `${token.path} is missing ${part}`);
      }
    }
  }

  assert.ok(Array.isArray(extension.contrastChecks) && extension.contrastChecks.length > 0);
  assert.ok(Array.isArray(extension.board.components) && extension.board.components.length >= 8);
  assert.ok(Array.isArray(extension.board.surfaces) && extension.board.surfaces.length >= 4);
});

test("legacy Mobile source is a pointer, not a competing token payload", async () => {
  const mobile = await readFile(resolve(root, "docs/Mobile/DESIGN_SYSTEM.md"), "utf8");
  assert.match(mobile, /fung\.tokens\.json/);
  assert.doesNotMatch(mobile, /fung-mobile-design-system:data:start/);
  assert.doesNotMatch(mobile, /single editable source of truth/i);
});

test("indexes and desktop glass point to the shared token authority", async () => {
  const files = [
    "docs/README.md",
    "docs/Mobile/README.md",
    "docs/Desktop/02-tokens.md",
    "docs/Desktop/README.md",
    "docs/brand-kit/README.md",
    "docs/design/FRONTEND_REDESIGN_BRIEF.md",
  ];
  const contents = await Promise.all(files.map((file) => readFile(resolve(root, file), "utf8")));
  for (const [index, content] of contents.entries()) {
    assert.match(content, /FUNG_Design_System_v0\.1\.0\/tokens\/fung\.tokens\.json|fung\.tokens\.json/i, `${files[index]} must point to the canonical token file`);

    for (const [, target] of content.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
      if (/^(?:https?:|mailto:|#)/i.test(target)) continue;
      const fileTarget = target.split(/[?#]/, 1)[0].replace(/^<|>$/g, "");
      if (!fileTarget) continue;
      await assert.doesNotReject(
        access(resolve(root, dirname(files[index]), fileTarget)),
        `${files[index]} links to missing file ${fileTarget}`,
      );
    }
  }

  const glassCss = await readFile(resolve(root, "src/components/desktop/LiquidGlass.css"), "utf8");
  assert.match(glassCss, /var\(--fung-glass-canvas\)/);
  assert.doesNotMatch(glassCss, /--fung-glass-canvas\s*:/, "LiquidGlass.css must consume generated material values, not define a duplicate palette");
});
