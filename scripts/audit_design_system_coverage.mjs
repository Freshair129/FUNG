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
  const source = await readFile(path, "utf8");
  const references = new Set([...source.matchAll(/var\(\s*(--fung-[\w-]+)/g)].map((match) => match[1]));
  const canonicalReferences = [...references].filter((name) => canonicalNames.has(name));
  const declarations = [...source.matchAll(directColor)].length;
  rawColorSyntax += [...source.matchAll(colorSyntax)].length;
  directLiteralDeclarations += declarations;
  records.push({
    path: relative(root, path).replaceAll("\\", "/"),
    canonicalReferences: canonicalReferences.sort(),
    directLiteralDeclarations: declarations,
  });
}

const integrated = records.filter((record) => record.canonicalReferences.length > 0);
const uncovered = records.filter((record) => record.canonicalReferences.length === 0);
const uniqueColors = new Set();
for (const path of cssFiles) {
  const source = await readFile(path, "utf8");
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
  `- Simple single-color CSS declarations written as a literal: **${directLiteralDeclarations}**. This includes visual states and surface-specific colors and is a triage count, not a claim that every occurrence is a defect.`,
  `- CSS color syntax occurrences: **${rawColorSyntax}** across **${uniqueColors.size}** unique hex values. This count includes fallbacks, translucent materials, charts, illustrations and third-party marks.`,
  "- The board's rendered component specimens demonstrate token intent; they do not prove production component adoption.",
  "",
  "## Canonical token references",
  "",
  "| Stylesheet | Referenced tokens | Literal declarations |",
  "| --- | --- | ---: |",
  ...integrated.map((record) => `| \`${record.path}\` | ${record.canonicalReferences.map((name) => `\`${name}\``).join(", ")} | ${record.directLiteralDeclarations} |`),
  "",
  "## Stylesheets without canonical token references",
  "",
  "| Stylesheet | Literal declarations |",
  "| --- | ---: |",
  ...uncovered.map((record) => `| \`${record.path}\` | ${record.directLiteralDeclarations} |`),
  "",
  "## Acceptance boundary",
  "",
  "New semantic colors should use a canonical token or include a documented, reviewed exception. Existing uncovered styles need an owner and a migration decision before being represented as complete. Source inspection cannot establish rendered contrast, alpha-over-image contrast, clipping, text scaling, native focus, screen-reader announcements or reduced-motion behavior. See the package verification report for the separate runtime evidence and limits.",
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
