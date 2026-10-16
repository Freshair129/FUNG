import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = resolve(root, "src");
const tokenCssPath = resolve(root, "docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.css");
const reportPath = resolve(root, "docs/FUNG_Design_System_v0.1.0/qa/COMPONENT_COVERAGE.md");
const mode = process.argv[2] ?? "render";
if (!new Set(["render", "check"]).has(mode)) {
  console.error(`Unknown coverage mode: ${mode}. Use render or check.`);
  process.exit(2);
}

async function filesUnder(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...await filesUnder(path));
    else files.push(path);
  }
  return files;
}

const cssFiles = (await filesUnder(sourceRoot)).filter((path) => path.endsWith(".css")).sort();
const tokenCss = await readFile(tokenCssPath, "utf8");
const canonicalNames = new Set([...tokenCss.matchAll(/^\s*(--fung-[\w-]+)\s*:/gm)].map((match) => match[1]));
const directColor = /\b(color|background(?:-color)?|border(?:-(?:top|right|bottom|left))?(?:-color)?|outline(?:-color)?|fill|stroke)\s*:\s*(#[\da-f]{3,8}\b|rgba?\([^;{}]+?\))\s*(?:!important\s*)?;/gi;
const colorSyntax = /#[\da-f]{3,8}\b|rgba?\(|hsla?\(/gi;
const records = [];
let rawColorSyntax = 0;
let directLiteralDeclarations = 0;

for (const path of cssFiles) {
  // Ignore block comments so deleted palettes and documentation examples do not
  // inflate the source inventory.
  const source = (await readFile(path, "utf8")).replace(/\/\*[\s\S]*?\*\//g, "");
  const references = new Set([...source.matchAll(/var\(\s*(--fung-[\w-]+)/g)].map((match) => match[1]));
  const canonicalReferences = [...references].filter((name) => canonicalNames.has(name));
  const declarations = [...source.matchAll(directColor)].length;
  const colorSyntaxOccurrences = [...source.matchAll(colorSyntax)].length;
  rawColorSyntax += colorSyntaxOccurrences;
  directLiteralDeclarations += declarations;
  records.push({
    path: relative(root, path).replaceAll("\\", "/"),
    canonicalReferences: canonicalReferences.sort(),
    directLiteralDeclarations: declarations,
    colorSyntaxOccurrences,
  });
}

const integrated = records.filter((record) => record.canonicalReferences.length > 0);
const uncovered = records.filter((record) => record.canonicalReferences.length === 0);
const uniqueColors = new Set();
for (const path of cssFiles) {
  const source = (await readFile(path, "utf8")).replace(/\/\*[\s\S]*?\*\//g, "");
  for (const match of source.matchAll(/#[\da-f]{3,8}\b/gi)) uniqueColors.add(match[0].toUpperCase());
}

const lines = [
  "# FUNG — Component token coverage",
  "",
  "Generated from CSS under `src/` and the canonical generated token CSS. This is a source inventory, not a visual acceptance report.",
  "",
  "## Findings",
  "",
  `- Stylesheets scanned: **${records.length}**.` ,
  `- Stylesheets using at least one canonical ` + "`--fung-*`" + ` variable: **${integrated.length}/${records.length}**. A locally invented ` + "`--fung-*`" + ` alias does not count as a canonical token reference.`,
  `- Stylesheets with no canonical token reference: **${uncovered.length}**.`,
  `- Flat, single-color CSS declarations written as direct literals: **${directLiteralDeclarations}**. Block comments are excluded; semantic UI colors should use a canonical token.`,
  `- Embedded CSS color syntax occurrences: **${rawColorSyntax}** across **${uniqueColors.size}** unique hex values. This inventory includes token fallbacks, gradients, shadows, translucent materials, illustrations and third-party marks; inspect context before changing these effects.`,
  "- The board's rendered component specimens demonstrate token intent; they do not prove production component adoption.",
  "",
  "## Canonical token references",
  "",
  "| Stylesheet | Referenced tokens | Flat literal declarations | Embedded color syntax |",
  "| --- | --- | ---: | ---: |",
  ...integrated.map((record) => `| \`${record.path}\` | ${record.canonicalReferences.map((name) => `\`${name}\``).join(", ")} | ${record.directLiteralDeclarations} | ${record.colorSyntaxOccurrences} |`),
  "",
  "## Stylesheets without canonical token references",
  "",
  "| Stylesheet | Flat literal declarations | Embedded color syntax |",
  "| --- | ---: | ---: |",
  ...uncovered.map((record) => `| \`${record.path}\` | ${record.directLiteralDeclarations} | ${record.colorSyntaxOccurrences} |`),
  "",
  "## Acceptance boundary",
  "",
  "Flat semantic colors are expected to use canonical tokens. Embedded colors remain visible in the inventory because gradients, shadows, translucent materials and illustration layers need contextual review; this source report does not verify their rendered appearance. Source inspection cannot establish rendered contrast, alpha-over-image contrast, clipping, text scaling, native focus, screen-reader announcements or reduced-motion behavior. See the package verification report for the separate runtime evidence and limits.",
  "",
];
const report = lines.join("\n");

if (mode === "render") {
  await mkdir(dirname(reportPath), { recursive: true });
  await writeFile(reportPath, report, "utf8");
  console.log(`Rendered ${relative(root, reportPath)} (${integrated.length}/${records.length} stylesheets use canonical tokens).`);
} else {
  const current = await readFile(reportPath, "utf8").catch(() => "");
  if (current !== report) {
    console.error(`${relative(root, reportPath)} is stale. Run npm run design-system:render.`);
    process.exit(1);
  }
  console.log(`Checked ${relative(root, reportPath)} (${integrated.length}/${records.length} stylesheets use canonical tokens).`);
}
