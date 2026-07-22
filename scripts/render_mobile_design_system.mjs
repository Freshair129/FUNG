import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(root, "docs/Mobile/DESIGN_SYSTEM.md");
const outputs = {
  system: resolve(root, "docs/Mobile/design-system/index.html"),
  brand: resolve(root, "docs/Mobile/design-system/brand/index.html"),
};
const start = "<!-- fung-mobile-design-system:data:start -->";
const end = "<!-- fung-mobile-design-system:data:end -->";

const escapeHtml = (value) => String(value)
  .replaceAll("&", "&amp;")
  .replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;")
  .replaceAll('"', "&quot;")
  .replaceAll("'", "&#39;");

function extractPayload(markdown) {
  const from = markdown.indexOf(start);
  const to = markdown.indexOf(end);
  if (from === -1 || to === -1 || to <= from) {
    throw new Error("DESIGN_SYSTEM.md must contain one fung-mobile-design-system payload block.");
  }
  const body = markdown.slice(from + start.length, to).trim();
  const match = body.match(/^```json\s*([\s\S]*?)\s*```$/);
  if (!match) throw new Error("The design-system payload must be fenced as json.");
  try {
    return JSON.parse(match[1]);
  } catch (error) {
    throw new Error(`Invalid design-system JSON: ${error.message}`);
  }
}

function validate(data) {
  const required = ["version", "sourceUpdatedAt", "brand", "themes", "semantic", "type", "spacing", "radii", "elevation", "components", "brandNarrative"];
  for (const key of required) if (!(key in data)) throw new Error(`Missing token payload key: ${key}`);
  if (data.brand.markStatus !== "selected-beta") throw new Error("The selected beta identity must be explicitly marked before its production vector can render.");
  for (const key of ["markAsset", "markInkAsset", "markWhiteAsset", "appIconAsset"]) {
    if (!data.brand[key]) throw new Error(`Missing selected brand asset: ${key}`);
  }
  if (!data.themes.light?.canvas || !data.themes.dark?.canvas) throw new Error("Both light and dark theme tokens are required.");
  if (!Array.isArray(data.spacing) || !Array.isArray(data.components) || !Array.isArray(data.brandNarrative)) {
    throw new Error("spacing, components and brandNarrative must be arrays.");
  }
}

function shell({ title, data, content, page }) {
  const payload = escapeHtml(JSON.stringify(data));
  const dark = data.themes.dark;
  const light = data.themes.light;
  const homeHref = page === "brand" ? "../index.html" : "./index.html";
  return `<!doctype html>
<html lang="th" data-theme="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta name="color-scheme" content="dark light" />
  <title>${escapeHtml(title)}</title>
  <script>if (location.search.includes("export=1")) document.documentElement.dataset.export="true";</script>
  <style>
    :root { --canvas:${dark.canvas}; --surface:${dark.surface}; --raised:${dark.surfaceRaised}; --ink:${dark.ink}; --muted:${dark.muted}; --line:${dark.line}; --focus:${data.semantic.focusOnDark}; --local:${data.semantic.localOnDark}; --inferred:${data.semantic.inferredOnDark}; --record:${data.semantic.record}; --danger:${data.semantic.danger}; --control:14px; --surface-radius:20px; --dock:28px; --shadow:0 12px 34px rgba(0,0,0,.16); font-family:${escapeHtml(data.type.uiFamily)}; color:var(--ink); background:var(--canvas); }
    html[data-theme="light"] { --canvas:${light.canvas}; --surface:${light.surface}; --raised:${light.surfaceRaised}; --ink:${light.ink}; --muted:${light.muted}; --line:${light.line}; --focus:${data.semantic.focus}; --local:${data.semantic.local}; --inferred:${data.semantic.inferred}; --record:${data.semantic.record}; --danger:${data.semantic.danger}; }
    * { box-sizing:border-box; }
    body { margin:0; background:var(--canvas); color:var(--ink); }
    button { font:inherit; }
    a { color:inherit; }
    .site { min-height:100vh; }
    .topbar { position:sticky; z-index:3; top:0; display:flex; align-items:center; justify-content:space-between; gap:16px; padding:16px clamp(20px,4vw,56px); border-bottom:1px solid var(--line); background:color-mix(in srgb,var(--canvas) 92%,transparent); backdrop-filter:blur(16px); }
    .brand { display:flex; gap:10px; align-items:center; font-weight:700; letter-spacing:.02em; text-decoration:none; }
    .brand-dot { width:10px; height:10px; background:var(--focus); border-radius:999px; box-shadow:0 0 0 5px color-mix(in srgb,var(--focus) 15%,transparent); }
    .controls { display:flex; align-items:center; gap:8px; }
    .button { min-height:40px; border:1px solid var(--line); border-radius:999px; background:var(--surface); color:var(--ink); padding:0 14px; cursor:pointer; }
    .button:hover { border-color:var(--focus); }
    .button.primary { background:var(--focus); border-color:var(--focus); color:#fff; }
    .page { max-width:1200px; margin:0 auto; padding:clamp(40px,8vw,112px) clamp(20px,4vw,56px) 96px; }
    .eyebrow { margin:0 0 12px; color:var(--local); font-size:12px; font-weight:700; letter-spacing:.09em; text-transform:uppercase; }
    h1 { max-width:850px; margin:0; font-size:clamp(40px,7vw,88px); letter-spacing:-.055em; line-height:.98; }
    h2 { margin:0; font-size:clamp(28px,4vw,48px); letter-spacing:-.04em; line-height:1.05; }
    h3 { margin:0; font-size:18px; }
    p { color:var(--muted); line-height:1.65; }
    .lede { max-width:690px; font-size:18px; }
    .section { margin-top:clamp(60px,10vw,136px); }
    .grid { display:grid; gap:16px; }
    .two { grid-template-columns:repeat(2,minmax(0,1fr)); }
    .three { grid-template-columns:repeat(3,minmax(0,1fr)); }
    .panel { padding:24px; border:1px solid var(--line); border-radius:var(--surface-radius); background:var(--surface); box-shadow:var(--shadow); }
    .token { display:flex; align-items:center; gap:14px; min-height:80px; }
    .swatch { width:44px; height:44px; flex:0 0 44px; border-radius:14px; border:1px solid color-mix(in srgb,#fff 24%,transparent); }
    .mono { font:500 12px ui-monospace,SFMono-Regular,Menlo,monospace; color:var(--muted); }
    .copy { margin-left:auto; min-height:32px; padding:0 10px; border:1px solid var(--line); border-radius:999px; background:transparent; color:var(--muted); cursor:pointer; font-size:12px; }
    .copy:hover { color:var(--ink); border-color:var(--focus); }
    .specimen { position:relative; overflow:hidden; min-height:430px; padding:20px; border:1px solid var(--line); border-radius:28px; background:var(--canvas); box-shadow:var(--shadow); }
    .specimen.light { background:${light.canvas}; color:${light.ink}; }
    .specimen.light .mini-card { background:${light.surface}; border-color:${light.line}; }
    .specimen.light .mini-muted { color:${light.muted}; }
    .device { width:min(100%,340px); margin:0 auto; padding:18px; border:1px solid var(--line); border-radius:34px; background:var(--surface); box-shadow:var(--shadow); }
    .device.landscape { width:min(100%,720px); min-height:290px; display:grid; grid-template-columns:60px 1fr; gap:14px; }
    .mini-status { display:flex; justify-content:space-between; color:var(--muted); font-size:12px; }
    .mini-card { padding:16px; border:1px solid var(--line); border-radius:20px; background:var(--raised); }
    .mini-muted { color:var(--muted); font-size:13px; }
    .voice { display:grid; place-items:center; width:108px; height:108px; margin:20px auto; border:7px solid color-mix(in srgb,var(--focus) 30%,transparent); border-radius:999px; background:var(--focus); color:#fff; font-size:26px; box-shadow:inset 0 2px 7px rgba(0,0,0,.18); }
    .wave { display:flex; align-items:center; gap:3px; height:42px; }
    .wave i { display:block; width:3px; background:var(--local); border-radius:99px; }
    .dock { display:flex; justify-content:space-around; align-items:center; min-height:58px; margin-top:18px; border-radius:var(--dock); background:var(--raised); color:var(--muted); font-size:12px; }
    .dock b { color:var(--ink); }
    .rail { display:flex; flex-direction:column; justify-content:space-evenly; padding:8px 0; border-radius:24px; background:var(--raised); color:var(--muted); text-align:center; font-size:11px; }
    .badge { display:inline-flex; align-items:center; gap:6px; border-radius:999px; padding:6px 10px; background:color-mix(in srgb,var(--local) 15%,transparent); color:var(--local); font-size:12px; font-weight:700; }
    .warning { background:color-mix(in srgb,var(--inferred) 15%,transparent); color:var(--inferred); }
    .record { background:color-mix(in srgb,var(--record) 15%,transparent); color:var(--record); }
    .spacers { display:flex; align-items:end; gap:12px; min-height:120px; }
    .space { display:grid; place-items:end center; gap:8px; color:var(--muted); font-size:12px; }
    .space i { display:block; width:24px; background:var(--focus); border-radius:6px; }
    .footer { margin-top:80px; padding-top:22px; border-top:1px solid var(--line); color:var(--muted); font-size:12px; }
    .brand-hero { min-height:70vh; display:grid; align-content:center; gap:24px; }
    .wordmark { display:flex; align-items:center; gap:20px; font-size:clamp(62px,13vw,170px); font-weight:760; letter-spacing:-.09em; line-height:.8; }
    .guide { background-image:linear-gradient(var(--line) 1px,transparent 1px),linear-gradient(90deg,var(--line) 1px,transparent 1px); background-size:32px 32px; }
    .logo-grid { min-height:360px; display:grid; place-items:center; border:1px solid var(--line); border-radius:var(--surface-radius); }
    .rule { height:1px; background:var(--line); margin:18px 0; }
    .quiet-mark { display:block; width:clamp(72px,10vw,132px); height:clamp(72px,10vw,132px); object-fit:contain; }
    .quiet-mark.large { width:150px; height:150px; }
    .quiet-mark.icon { width:96px; height:96px; }
    @media (max-width:760px) {
      .two,.three { grid-template-columns:1fr; }
      .topbar { padding:14px 18px; }
      .button { padding:0 11px; }
      .page { padding-inline:20px; }
      .brand-hero { min-height:58vh; }
      .device.landscape { grid-template-columns:48px 1fr; }
    }
    @media print {
      .topbar,.controls,.copy { display:none !important; }
      body { background:#fff; color:#111; }
      .page { max-width:none; padding:28px; }
      .panel,.device,.specimen,.logo-grid { box-shadow:none; break-inside:avoid; }
      .section { margin-top:48px; }
    }
    html[data-export="true"] .site { min-height:0; }
    html[data-export="true"] .brand-hero { min-height:740px; padding-block:120px; }
    html[data-export="true"] .topbar { position:static; }
    html[data-export="true"] .page { padding-bottom:48px; }
  </style>
</head>
<body data-page="${page}">
  <main class="site">
    <header class="topbar"><a class="brand" href="${homeHref}"><span class="brand-dot"></span>FUNG / design system</a><div class="controls"><button class="button" data-theme-toggle>Light canvas</button><button class="button" onclick="window.print()">Export / Print</button></div></header>
    ${content}
  </main>
  <script id="fung-design-system-data" type="application/json">${payload}</script>
  <script>
    const root=document.documentElement; const toggle=document.querySelector("[data-theme-toggle]");
    toggle?.addEventListener("click",()=>{const next=root.dataset.theme==="dark"?"light":"dark";root.dataset.theme=next;toggle.textContent=next==="dark"?"Light canvas":"Deep Blue canvas";});
    document.querySelectorAll("[data-copy]").forEach(button=>button.addEventListener("click",async()=>{await navigator.clipboard?.writeText(button.dataset.copy||"");const prior=button.textContent;button.textContent="Copied";setTimeout(()=>button.textContent=prior,900);}));
  </script>
</body></html>`;
}

const wave = () => `<div class="wave" aria-label="Waveform preview">${[14, 25, 39, 18, 33, 42, 23, 35, 17, 40, 28, 16, 37, 24, 42, 19, 32, 14, 30].map((height) => `<i style="height:${height}px"></i>`).join("")}</div>`;

function systemPage(data) {
  const mark = `<img class="quiet-mark" src="${escapeHtml(data.brand.markInkAsset)}" alt="FUNG Quiet Archive mark" />`;
  const tokens = [
    ["Focus / selected", "var(--focus)", data.semantic.focus],
    ["Local / confirmed", "var(--local)", data.semantic.local],
    ["Inference proposal", "var(--inferred)", data.semantic.inferred],
    ["Recording / destructive", "var(--record)", data.semantic.record],
  ];
  const swatches = tokens.map(([label, css, hex]) => `<article class="panel token"><span class="swatch" style="background:${css}"></span><span><strong>${label}</strong><span class="mono">${hex}</span></span><button class="copy" data-copy="${hex}">Copy</button></article>`).join("");
  const spacers = data.spacing.map((value) => `<span class="space"><i style="height:${value * 1.45}px"></i>${value}</span>`).join("");
  const componentRows = data.components.map((component) => `<article class="panel"><span class="badge ${component.state === "inferred" ? "warning" : component.state === "recording" ? "record" : ""}">${component.state}</span><h3 style="margin-top:14px">${escapeHtml(component.name)}</h3><p>${escapeHtml(component.note)}</p></article>`).join("");
  return shell({
    title: "FUNG Mobile Design System",
    data,
    page: "system",
    content: `<div class="page">
    <section><p class="eyebrow">v${data.version} - generated from Markdown SOT</p><h1>Quiet tools for <em>voice, evidence and local work.</em></h1><p class="lede">An interactive reference for FUNG Mobile. Semantic states remain truthful: local is not cloud, inference is not fact, and recording is always visible.</p></section>
    <section class="section"><div class="grid two"><article class="panel"><p class="eyebrow">Source status</p><h2>One editable source.</h2><p>Change <code>docs/Mobile/DESIGN_SYSTEM.md</code>; this page and the brand case study are regenerated from the same payload.</p><span class="badge">Source updated ${data.sourceUpdatedAt}</span></article><article class="panel"><p class="eyebrow">Logo status</p><div style="display:flex;align-items:center;gap:16px">${mark}<span><h2>${data.brand.name}</h2><p>${data.brand.markStatusLabel}.</p><span class="badge">Quiet Archive</span></span></div></article></div></section>
    <section class="section"><p class="eyebrow">Semantic colour</p><h2>Meaning before decoration.</h2><div class="grid two" style="margin-top:22px">${swatches}</div></section>
    <section class="section"><p class="eyebrow">Type, space and material</p><div class="grid two"><article class="panel"><p class="eyebrow">Thai-first type</p><div style="font-size:${data.type.display.size};line-height:${data.type.display.lineHeight};font-weight:${data.type.display.weight}">บันทึกให้ชัด<br>และฟังได้นาน</div><p style="font-size:${data.type.body.size}">อ่านข้อความและ transcript สบายตา โดยไม่ใช้ viewport-scaled type</p><span class="mono">${escapeHtml(data.type.uiFamily)}</span></article><article class="panel"><p class="eyebrow">Spacing scale</p><div class="spacers">${spacers}</div><p>Controls ${data.radii.control} - surfaces ${data.radii.surface} - dock ${data.radii.dock}</p></article></div></section>
    <section class="section"><p class="eyebrow">Interactive states</p><h2>Touch-first, never decorative.</h2><div class="grid two" style="margin-top:22px">${componentRows}</div></section>
    <section class="section"><p class="eyebrow">Responsive mobile previews</p><h2>Portrait and landscape stay Mobile.</h2><div class="grid two" style="margin-top:22px"><article class="specimen"><p class="eyebrow">Portrait / voice home</p><div class="device"><div class="mini-status"><span>9:41</span><span>Local-ready</span></div><div class="voice">⌁</div><div class="mini-card"><strong>เริ่มบันทึกเสียง</strong><p class="mini-muted">บันทึกในเครื่อง - พร้อมใช้งาน</p>${wave()}</div><div class="dock"><span>Home</span><span>Notes</span><b>Voice</b><span>Graph</span><span>Devices</span></div></div></article><article class="specimen"><p class="eyebrow">Landscape / responsive rail</p><div class="device landscape"><aside class="rail"><span>Home</span><span>Notes</span><b>Voice</b><span>Graph</span><span>Device</span></aside><div><div class="mini-status"><span>FUNG Mobile</span><span class="badge">On-device</span></div><div class="mini-card" style="margin-top:14px"><strong>เสียงบันทึกวันนี้</strong>${wave()}<p class="mini-muted">Timeline, note evidence and actions remain within the mobile surface.</p></div></div></div></article></div></section>
    <footer class="footer">Generated from docs/Mobile/DESIGN_SYSTEM.md - Source revision ${data.version} - Do not hand-edit this HTML.</footer>
  </div>`,
  });
}

function brandPage(data) {
  const whiteMark = `../${escapeHtml(data.brand.markWhiteAsset)}`;
  const inkMark = `../${escapeHtml(data.brand.markInkAsset)}`;
  const iconAsset = `../${escapeHtml(data.brand.appIconAsset)}`;
  const colors = [
    ["White", data.themes.light.canvas],
    ["Deep Blue", data.themes.dark.canvas],
    ["Focus", data.semantic.focus],
    ["Local", data.semantic.local],
    ["Inference", data.semantic.inferred],
    ["Record", data.semantic.record],
  ].map(([label, hex]) => `<article class="panel token"><span class="swatch" style="background:${hex}"></span><span><strong>${label}</strong><span class="mono">${hex}</span></span></article>`).join("");
  const chapters = data.brandNarrative.map((item, index) => `<article class="panel"><p class="eyebrow">0${index + 1}</p><h3>${escapeHtml(item)}</h3></article>`).join("");
  return shell({
    title: "FUNG Brand Presentation",
    data,
    page: "brand",
    content: `<div class="page">
    <section class="brand-hero"><p class="eyebrow">${data.brand.tagline} - selected identity</p><div class="wordmark"><img class="quiet-mark" src="${whiteMark}" alt="FUNG Quiet Archive mark" />${data.brand.name}</div><p class="lede">A calm identity for private voice work. Quiet Archive keeps an open centre for the note, recording and evidence that remain available to the user.</p><span class="badge">${data.brand.markStatusLabel}</span></section>
    <section class="section"><div class="grid three">${chapters}</div></section>
    <section class="section"><p class="eyebrow">Quiet Archive construction</p><h2>An open archive, drawn as one ribbon.</h2><div class="grid two" style="margin-top:22px"><article class="logo-grid guide"><img class="quiet-mark large" src="${whiteMark}" alt="Quiet Archive construction mark" /></article><article class="panel"><h3>Master rules</h3><div class="rule"></div><p>The flat master retains a continuous rounded ribbon, an off-centre archive aperture and two diagonal fold cuts. Material texture is presentation-only.</p><p class="mono">Current status: ${data.brand.markStatus}</p><div style="display:flex;gap:14px;align-items:center;margin-top:18px"><img class="quiet-mark" src="${inkMark}" alt="Deep Blue mark on white" /><img class="quiet-mark" src="${whiteMark}" style="background:#28374C;border-radius:24px;padding:12px" alt="White mark on Deep Blue" /><img class="quiet-mark icon" src="${iconAsset}" alt="Android icon preview" /></div></article></div></section>
    <section class="section"><p class="eyebrow">Colour system</p><h2>Semantic, restrained and operational.</h2><div class="grid three" style="margin-top:22px">${colors}</div></section>
    <section class="section"><p class="eyebrow">Typography</p><div class="grid two"><article class="panel"><div style="font-size:${data.type.display.size};line-height:${data.type.display.lineHeight};font-weight:${data.type.display.weight}">ฟังให้ครบ<br>ก่อนสรุป</div><p>Thai content has zero letter-spacing and a reading-first body scale.</p></article><article class="panel"><h3>Icon and material voice</h3><p>Rounded, deliberate outline icons. Surfaces use shallow porcelain elevation; pressed controls use inset depth without layout movement.</p><span class="badge">Reduced motion respected</span></article></div></section>
    <section class="section"><p class="eyebrow">Product application</p><h2>The identity becomes operational UI.</h2><div class="grid two" style="margin-top:22px"><article class="specimen"><div class="device"><div class="mini-status"><span>FUNG</span><span>9:41</span></div><div class="voice">⌁</div><p style="text-align:center;margin:0"><strong>กดค้างเพื่อสั่งงาน</strong><br><span class="mini-muted">ทำงานบนมือถือ</span></p><div class="mini-card" style="margin-top:18px">โน้ตล่าสุด${wave()}</div><div class="dock"><span>Home</span><span>Notes</span><b>Voice</b><span>Graph</span><span>Devices</span></div></div></article><article class="panel"><p class="eyebrow">Truthful states</p><h3>Identity never hides execution context.</h3><p><span class="badge">On-device</span> means local processing. <span class="badge warning">Inference</span> remains pending. <span class="badge record">Recording</span> is visually and textually explicit.</p><p>FUNG does not adopt the reference's cloning claims, neon palette, avatars or subscription language.</p></article></div></section>
    <section class="section"><p class="eyebrow">Reference boundary</p><div class="panel"><h2>Adapt the rhythm, not the identity.</h2><p>Sequential narrative, construction grids and a long-form case-study presentation are permitted inspiration. Brand geometry, marks, copy and type treatment remain independent FUNG work.</p></div></section>
    <footer class="footer">Generated from docs/Mobile/DESIGN_SYSTEM.md - Source revision ${data.version} - Use Export / Print for long image or PDF capture.</footer>
  </div>`,
  });
}

async function compile() {
  const markdown = await readFile(sourcePath, "utf8");
  const data = extractPayload(markdown);
  validate(data);
  return { system: systemPage(data), brand: brandPage(data) };
}

async function render() {
  const rendered = await compile();
  for (const [key, path] of Object.entries(outputs)) {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, rendered[key], "utf8");
  }
  return rendered;
}

async function check() {
  const rendered = await compile();
  const stale = [];
  for (const [key, path] of Object.entries(outputs)) {
    const actual = await readFile(path, "utf8");
    if (actual !== rendered[key]) stale.push(path);
  }
  if (stale.length) throw new Error(`Generated design-system artifact is stale: ${stale.join(", ")}`);
}

const mode = process.argv[2] ?? "render";
try {
  if (mode === "render") await render();
  else if (mode === "check") await check();
  else throw new Error(`Unknown mode: ${mode}. Use render or check.`);
  console.log(`Mobile design system ${mode} passed.`);
} catch (error) {
  console.error(`Mobile design system ${mode} failed: ${error.message}`);
  process.exitCode = 1;
}
