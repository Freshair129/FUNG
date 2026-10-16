import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(root, "docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.json");
const outputs = {
  system: resolve(root, "docs/design-system/index.html"),
  brand: resolve(root, "docs/design-system/brand/index.html"),
  legacySystem: resolve(root, "docs/Mobile/design-system/index.html"),
  legacyBrand: resolve(root, "docs/Mobile/design-system/brand/index.html"),
};
const extensionKey = "com.fung.design-system";

const escapeHtml = (value) => String(value)
  .replaceAll("&", "&amp;")
  .replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;")
  .replaceAll('"', "&quot;")
  .replaceAll("'", "&#39;");

function collectTokens(group, path = [], inheritedType = undefined, tokens = []) {
  if (!group || typeof group !== "object" || Array.isArray(group)) {
    throw new Error(`Token group ${path.join(".") || "root"} must be an object.`);
  }
  const type = group.$type ?? inheritedType;
  if ("$value" in group) {
    if (!type || !group.$description) throw new Error(`Token ${path.join(".")} has no type or description.`);
    tokens.push({ path, name: path.join("."), type, value: group.$value, description: group.$description, extensions: group.$extensions ?? {} });
    return tokens;
  }
  for (const [name, child] of Object.entries(group)) {
    if (!name.startsWith("$")) collectTokens(child, [...path, name], type, tokens);
  }
  return tokens;
}

function colorCss(value) {
  if (value.alpha === undefined || value.alpha === 1) {
    return value.hex?.toUpperCase() ?? `#${value.components.map((channel) => Math.round(channel * 255).toString(16).padStart(2, "0")).join("").toUpperCase()}`;
  }
  const channels = value.components.map((channel) => Math.round(channel * 255)).join(", ");
  return `rgba(${channels}, ${value.alpha})`;
}

const dimensionCss = (value) => `${value.value}${value.unit}`;
const numberText = (value) => Number.isInteger(value) ? String(value) : String(value);

function cssValue(token) {
  const { type, value } = token;
  if (type === "color") return colorCss(value);
  if (type === "dimension") return dimensionCss(value);
  if (type === "fontFamily") {
    const families = Array.isArray(value) ? value : [value];
    return families.map((family) => /\s/.test(family) ? `"${family}"` : family).join(", ");
  }
  if (type === "fontWeight") return String(value);
  if (type === "duration") return dimensionCss(value);
  if (type === "cubicBezier") return `cubic-bezier(${value.join(", ")})`;
  if (type === "number") {
    const custom = token.extensions?.[extensionKey];
    return numberText(value) + (custom?.cssUnit ?? "");
  }
  if (type === "shadow") {
    const shadows = Array.isArray(value) ? value : [value];
    return shadows.map((shadow) => [
      shadow.inset ? "inset" : "",
      dimensionCss(shadow.offsetX),
      dimensionCss(shadow.offsetY),
      dimensionCss(shadow.blur),
      dimensionCss(shadow.spread),
      colorCss(shadow.color),
    ].filter(Boolean).join(" ")).join(", ");
  }
  throw new Error(`Unsupported board token type: ${type}`);
}

function cssVariable(path) {
  if (path[0] === "brand") return `--fung-brand-${path.slice(1).join("-")}`;
  if (path[0] === "shared") return `--fung-${path.slice(1).join("-")}`;
  if (path[0] === "themes") return `--fung-${path.slice(2).join("-")}`;
  throw new Error(`No CSS variable mapping for ${path.join(".")}.`);
}

function validate(data, tokens) {
  const extension = data.$extensions?.[extensionKey];
  if (!extension?.metadata || !extension?.board || !Array.isArray(extension.contrastChecks)) {
    throw new Error(`Token source needs metadata, board content and contrast checks in $extensions.`);
  }
  if (!data.themes?.light || !data.themes?.dark) throw new Error("Both light and dark themes are required.");
  if (extension.board.brand?.status !== "selected-beta") throw new Error("The Quiet Archive beta identity must be explicitly selected.");
  for (const key of ["mark", "markInk", "markWhite", "appIcon"]) {
    if (!extension.board.brand.assets?.[key]) throw new Error(`Missing selected brand asset: ${key}.`);
  }
  if (!Array.isArray(extension.board.components) || extension.board.components.length < 8) {
    throw new Error("The shared board needs the documented component state set.");
  }
  if (!Array.isArray(extension.board.surfaces) || extension.board.surfaces.length < 4) {
    throw new Error("The shared board needs guidance for all product surfaces.");
  }
  if (tokens.length < 100) throw new Error(`Expected a complete shared token catalog, found ${tokens.length}.`);
}

function makeData(source) {
  const tokens = collectTokens(source);
  validate(source, tokens);
  const byPath = new Map(tokens.map((token) => [token.name, token]));
  const get = (name) => {
    const token = byPath.get(name);
    if (!token) throw new Error(`Missing required token ${name}.`);
    return token;
  };
  const value = (name) => get(name).value;
  const formatted = (name) => cssValue(get(name));
  const extension = source.$extensions[extensionKey];
  const brand = extension.board.brand;
  const theme = (name) => {
    const prefix = `themes.${name}.`;
    return {
      canvas: formatted(prefix + "bg-canvas"),
      surface: formatted(prefix + "bg-surface"),
      surfaceRaised: formatted(prefix + "bg-elevated"),
      ink: formatted(prefix + "text-primary"),
      muted: formatted(prefix + "text-secondary"),
      line: formatted(prefix + "border-subtle"),
      focus: formatted(prefix + "focus"),
      local: formatted(prefix + "state-local-fg"),
      inferred: formatted(prefix + "state-inferred-fg"),
      record: formatted(prefix + "state-recording-fg"),
      danger: formatted(prefix + "state-danger-fg"),
    };
  };
  const groups = [
    { title: "Brand primitives", tokens: tokens.filter((token) => token.path[0] === "brand") },
    { title: "Shared type, spacing, layout and material", tokens: tokens.filter((token) => token.path[0] === "shared") },
    { title: "Light semantic theme", tokens: tokens.filter((token) => token.path[0] === "themes" && token.path[1] === "light") },
    { title: "Dark semantic theme", tokens: tokens.filter((token) => token.path[0] === "themes" && token.path[1] === "dark") },
  ].map((group) => ({
    ...group,
    tokens: group.tokens.map((token) => ({
      path: token.name,
      type: token.type,
      value: cssValue(token),
      description: token.description,
      variable: cssVariable(token.path),
      swatch: token.type === "color" ? colorCss(token.value) : "",
    })),
  }));
  const spacing = tokens
    .filter((token) => token.path[0] === "shared" && /^space-\d+$/.test(token.path[1]))
    .sort((a, b) => Number(a.path[1].slice(6)) - Number(b.path[1].slice(6)))
    .map((token) => Number(token.value.value));
  const data = {
    version: extension.metadata.version,
    sourceUpdatedAt: extension.metadata.lastUpdated,
    format: extension.metadata.format,
    brand: {
      name: brand.name,
      tagline: brand.tagline,
      identity: brand.identity,
      status: brand.status,
      markStatusLabel: "Quiet Archive selected · beta identity",
      assets: brand.assets,
    },
    brandColors: tokens.filter((token) => token.path[0] === "brand").map((token) => ({
      name: token.path[1],
      value: colorCss(token.value),
    })),
    themes: { light: theme("light"), dark: theme("dark") },
    type: {
      uiFamily: formatted("shared.font-ui"),
      display: {
        size: formatted("shared.font-size-display"),
        lineHeight: formatted("shared.line-height-display"),
        weight: formatted("shared.font-weight-semibold"),
      },
      body: {
        size: formatted("shared.font-size-body-mobile"),
        lineHeight: formatted("shared.line-height-body-mobile"),
        weight: formatted("shared.font-weight-regular"),
      },
    },
    spacing,
    radii: {
      control: formatted("shared.radius-md"),
      surface: formatted("shared.radius-lg"),
      dock: formatted("shared.radius-pill"),
    },
    elevation: { rest: formatted("themes.dark.shadow-elevated") },
    components: extension.board.components,
    surfaces: extension.board.surfaces,
    accessibility: extension.board.accessibility,
    brandNarrative: extension.board.brandNarrative,
    breakpoints: extension.breakpoints,
    tokenGroups: groups,
    tokens: tokens.map((token) => ({ path: token.name, variable: cssVariable(token.path), value: cssValue(token), type: token.type })),
  };
  return data;
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
  <meta name="description" content="FUNG shared design tokens, semantic themes and cross-platform interface guidance." />
  <title>${escapeHtml(title)}</title>
  <script>if (location.search.includes("export=1")) document.documentElement.dataset.export="true";</script>
  <style>
    :root { --canvas:${dark.canvas}; --surface:${dark.surface}; --raised:${dark.surfaceRaised}; --ink:${dark.ink}; --muted:${dark.muted}; --line:${dark.line}; --focus:${dark.focus}; --local:${dark.local}; --inferred:${dark.inferred}; --record:${dark.record}; --danger:${dark.danger}; --control:var(--fung-radius-md,14px); --surface-radius:var(--fung-radius-lg,20px); --dock:var(--fung-radius-pill,28px); --shadow:${data.elevation.rest}; font-family:${escapeHtml(data.type.uiFamily)}; color:var(--ink); background:var(--canvas); }
    html[data-theme="light"] { --canvas:${light.canvas}; --surface:${light.surface}; --raised:${light.surfaceRaised}; --ink:${light.ink}; --muted:${light.muted}; --line:${light.line}; --focus:${light.focus}; --local:${light.local}; --inferred:${light.inferred}; --record:${light.record}; --danger:${light.danger}; color-scheme:light; }
    * { box-sizing:border-box; }
    body { margin:0; background:var(--canvas); color:var(--ink); }
    button,input { font:inherit; }
    a { color:inherit; }
    .site { min-height:100vh; }
    .topbar { position:sticky; z-index:3; top:0; display:flex; align-items:center; justify-content:space-between; gap:16px; padding:16px clamp(20px,4vw,56px); border-bottom:1px solid var(--line); background:color-mix(in srgb,var(--canvas) 92%,transparent); backdrop-filter:blur(16px); }
    .brand { display:flex; gap:10px; align-items:center; font-weight:700; letter-spacing:.02em; text-decoration:none; }
    .brand-dot { width:10px; height:10px; background:var(--focus); border-radius:999px; box-shadow:0 0 0 5px color-mix(in srgb,var(--focus) 15%,transparent); }
    .controls { display:flex; align-items:center; gap:8px; }
    .button { min-height:40px; border:1px solid var(--line); border-radius:999px; background:var(--surface); color:var(--ink); padding:0 14px; cursor:pointer; }
    .button:hover,.copy:hover { border-color:var(--focus); }
    .button:focus-visible,.copy:focus-visible,input:focus-visible,a:focus-visible { outline:3px solid var(--focus); outline-offset:3px; }
    .button.primary { background:var(--focus); border-color:var(--focus); color:#fff; }
    .page { max-width:1320px; margin:0 auto; padding:clamp(40px,8vw,112px) clamp(20px,4vw,56px) 96px; }
    .eyebrow { margin:0 0 12px; color:var(--local); font-size:12px; font-weight:700; letter-spacing:.09em; text-transform:uppercase; }
    h1 { max-width:850px; margin:0; font-size:clamp(40px,7vw,88px); letter-spacing:-.055em; line-height:.98; }
    h2 { margin:0; font-size:clamp(28px,4vw,48px); letter-spacing:-.04em; line-height:1.05; }
    h3 { margin:0; font-size:18px; }
    p { color:var(--muted); line-height:1.65; }
    .lede { max-width:760px; font-size:18px; }
    .section { margin-top:clamp(60px,10vw,136px); }
    .grid { display:grid; gap:16px; }
    .two { grid-template-columns:repeat(2,minmax(0,1fr)); }
    .three { grid-template-columns:repeat(3,minmax(0,1fr)); }
    .five { grid-template-columns:repeat(5,minmax(0,1fr)); }
    .panel { padding:24px; border:1px solid var(--line); border-radius:var(--surface-radius); background:var(--surface); box-shadow:var(--shadow); }
    .token { display:flex; align-items:center; gap:14px; min-height:80px; }
    .swatch { width:44px; height:44px; flex:0 0 44px; border-radius:14px; border:1px solid color-mix(in srgb,#fff 24%,transparent); }
    .mono { font:500 12px ui-monospace,SFMono-Regular,Menlo,monospace; color:var(--muted); }
    .copy { margin-left:auto; min-height:32px; padding:0 10px; border:1px solid var(--line); border-radius:999px; background:transparent; color:var(--muted); cursor:pointer; font-size:12px; }
    .specimen { position:relative; overflow:hidden; min-height:300px; padding:20px; border:1px solid var(--line); border-radius:28px; background:var(--canvas); box-shadow:var(--shadow); }
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
    .spacers { display:flex; align-items:end; gap:12px; min-height:120px; flex-wrap:wrap; }
    .space { display:grid; place-items:end center; gap:8px; color:var(--muted); font-size:12px; }
    .space i { display:block; width:24px; background:var(--focus); border-radius:6px; }
    .catalog-tools { display:flex; gap:12px; align-items:center; margin:22px 0; }
    .catalog-search { min-height:44px; flex:1; padding:0 14px; border:1px solid var(--line); border-radius:12px; background:var(--surface); color:var(--ink); }
    .catalog-group { margin:12px 0; border:1px solid var(--line); border-radius:16px; background:var(--surface); overflow:hidden; }
    .catalog-group summary { padding:16px 20px; cursor:pointer; font-weight:700; }
    .catalog-list { display:grid; }
    .token-row { display:grid; grid-template-columns:38px minmax(130px,1.1fr) minmax(130px,1fr) minmax(100px,.8fr) minmax(180px,1.5fr) auto; align-items:center; gap:12px; padding:12px 18px; border-top:1px solid var(--line); }
    .token-row[hidden] { display:none; }
    .token-swatch { width:28px; height:28px; border:1px solid var(--line); border-radius:8px; }
    .token-name,.token-variable,.token-value { overflow-wrap:anywhere; }
    .token-description { color:var(--muted); font-size:13px; }
    .surface-card { min-height:180px; }
    .surface-link { display:inline-block; margin-top:14px; color:var(--focus); font-size:12px; }
    .desktop-preview { display:grid; grid-template-columns:120px 1fr; min-height:270px; gap:12px; }
    .desktop-preview aside,.desktop-preview main,.desktop-preview header { border:1px solid var(--line); border-radius:12px; padding:14px; background:var(--surface); }
    .desktop-preview aside { display:grid; align-content:start; gap:12px; color:var(--muted); font-size:12px; }
    .desktop-preview main { display:grid; align-content:start; gap:12px; }
    .desktop-preview header { display:flex; justify-content:space-between; gap:8px; align-items:center; background:var(--raised); }
    .footer { margin-top:80px; padding-top:22px; border-top:1px solid var(--line); color:var(--muted); font-size:12px; }
    .brand-hero { min-height:70vh; display:grid; align-content:center; gap:24px; }
    .wordmark { display:flex; align-items:center; gap:20px; font-size:clamp(62px,13vw,170px); font-weight:760; letter-spacing:-.09em; line-height:.8; }
    .guide { background-image:linear-gradient(var(--line) 1px,transparent 1px),linear-gradient(90deg,var(--line) 1px,transparent 1px); background-size:32px 32px; }
    .logo-grid { min-height:360px; display:grid; place-items:center; border:1px solid var(--line); border-radius:var(--surface-radius); }
    .rule { height:1px; background:var(--line); margin:18px 0; }
    .quiet-mark { display:block; width:clamp(72px,10vw,132px); height:clamp(72px,10vw,132px); object-fit:contain; }
    .quiet-mark.large { width:150px; height:150px; }
    .quiet-mark.icon { width:96px; height:96px; }
    @media (max-width:900px) { .five { grid-template-columns:repeat(2,minmax(0,1fr)); } .token-row { grid-template-columns:32px 1fr 1fr auto; } .token-type,.token-description { display:none; } }
    @media (max-width:760px) {
      .two,.three,.five { grid-template-columns:1fr; }
      .topbar { padding:14px 18px; }
      .button { padding:0 11px; }
      .page { padding-inline:20px; }
      .brand-hero { min-height:58vh; }
      .device.landscape { grid-template-columns:48px 1fr; }
      .catalog-tools { align-items:stretch; flex-direction:column; }
      .token-row { grid-template-columns:32px minmax(0,1fr) auto; }
      .token-variable,.token-type,.token-description { display:none; }
    }
    @media (prefers-reduced-motion:reduce) { *,*::before,*::after { scroll-behavior:auto !important; animation-duration:.01ms !important; transition-duration:.01ms !important; } }
    @media print {
      .topbar,.controls,.copy,.catalog-tools { display:none !important; }
      body { background:#fff; color:#111; }
      .page { max-width:none; padding:28px; }
      .panel,.device,.specimen,.logo-grid,.catalog-group { box-shadow:none; break-inside:avoid; }
      .section { margin-top:48px; }
      .catalog-group:not([open]) .catalog-list { display:grid !important; }
    }
    html[data-export="true"] .site { min-height:0; }
    html[data-export="true"] .brand-hero { min-height:740px; padding-block:120px; }
    html[data-export="true"] .topbar { position:static; }
    html[data-export="true"] .page { padding-bottom:48px; }
  </style>
</head>
<body data-page="${page}">
  <main class="site">
    <header class="topbar"><a class="brand" href="${homeHref}"><span class="brand-dot"></span>FUNG / shared design system</a><div class="controls"><button class="button" data-theme-toggle>Light theme</button><button class="button" onclick="window.print()">Export / Print</button></div></header>
    ${content}
  </main>
  <script id="fung-design-system-data" type="application/json">${payload}</script>
  <script>
    const root=document.documentElement;
    const toggle=document.querySelector("[data-theme-toggle]");
    toggle?.addEventListener("click",()=>{const next=root.dataset.theme==="dark"?"light":"dark";root.dataset.theme=next;toggle.textContent=next==="dark"?"Light theme":"Dark theme";});
    document.querySelectorAll("[data-copy]").forEach(button=>button.addEventListener("click",async()=>{try{await navigator.clipboard.writeText(button.dataset.copy||"");button.textContent="Copied";}catch{button.textContent="Copy unavailable";}setTimeout(()=>button.textContent="Copy",1100);}));
    const search=document.querySelector("[data-token-search]");
    search?.addEventListener("input",()=>{const term=search.value.trim().toLowerCase();document.querySelectorAll("[data-token-row]").forEach(row=>{row.hidden=Boolean(term)&&!row.dataset.search.includes(term);});});
  </script>
</body></html>`;
}

const wave = () => `<div class="wave" aria-label="Illustrative waveform">${[14,25,39,18,33,42,23,35,17,40,28,16,37,24,42,19,32,14,30].map((height)=>`<i style="height:${height}px"></i>`).join("")}</div>`;

function catalog(data) {
  return data.tokenGroups.map((group, groupIndex) => `<details class="catalog-group" ${groupIndex === 0 ? "open" : ""}><summary>${escapeHtml(group.title)} <span class="mono">(${group.tokens.length})</span></summary><div class="catalog-list">${group.tokens.map((token)=>`<article class="token-row" data-token-row data-search="${escapeHtml((token.path+" "+token.variable+" "+token.type+" "+token.value+" "+token.description).toLowerCase())}">${token.swatch ? `<span class="token-swatch" style="background:${escapeHtml(token.swatch)}"></span>` : "<span></span>"}<code class="token-name">${escapeHtml(token.path)}</code><code class="mono token-variable">${escapeHtml(token.variable)}</code><span class="mono token-type">${escapeHtml(token.type)}</span><span class="token-description">${escapeHtml(token.description)}</span><button class="copy" data-copy="${escapeHtml(token.value)}" aria-label="Copy value for ${escapeHtml(token.path)}">Copy</button></article>`).join("")}</div></details>`).join("");
}

function systemPage(data) {
  const asset = (name) => `../Mobile/design-system/assets/${escapeHtml(data.brand.assets[name])}`;
  const mark = `<img class="quiet-mark" src="${asset("markInk")}" alt="FUNG Quiet Archive mark" />`;
  const colorTokens = [
    ["Focus", data.themes.light.focus, data.themes.dark.focus],
    ["Local / confirmed", data.themes.light.local, data.themes.dark.local],
    ["Inference / pending", data.themes.light.inferred, data.themes.dark.inferred],
    ["Recording", data.themes.light.record, data.themes.dark.record],
    ["Danger", data.themes.light.danger, data.themes.dark.danger],
  ];
  const swatches = colorTokens.map(([label,light,dark])=>`<article class="panel"><p class="eyebrow">${escapeHtml(label)}</p><div class="grid two"><span><span class="badge">Light</span><br><span class="swatch" style="display:block;background:${light};margin-top:10px"></span><code class="mono">${light}</code></span><span><span class="badge">Dark</span><br><span class="swatch" style="display:block;background:${dark};margin-top:10px"></span><code class="mono">${dark}</code></span></div></article>`).join("");
  const spacers = data.spacing.map((value)=>`<span class="space"><i style="height:${Math.min(value*1.45,120)}px"></i>${value}</span>`).join("");
  const componentRows = data.components.map((component)=>`<article class="panel"><span class="badge ${component.state==="inferred"?"warning":component.state==="recording"?"record":""}">${escapeHtml(component.state)}</span><h3 style="margin-top:14px">${escapeHtml(component.name)}</h3><p>${escapeHtml(component.note)}</p></article>`).join("");
  const surfaceRows = data.surfaces.map((surface)=>`<article class="panel surface-card"><p class="eyebrow">${escapeHtml(surface.name)}</p><p>${escapeHtml(surface.principle)}</p><a class="surface-link" href="../${escapeHtml(surface.reference.replace(/^docs\//,""))}">${escapeHtml(surface.reference)}</a></article>`).join("");
  return shell({
    title: "FUNG Shared Design System",
    data,
    page: "system",
    content: `<div class="page">
      <section><p class="eyebrow">DS-FUNG-001 · v${escapeHtml(data.version)} · ${escapeHtml(data.format)}</p><h1>Quiet tools for <em>voice, evidence and local work.</em></h1><p class="lede">Shared design tokens and cross-surface interface guidance for Desktop, Mobile, Web, Landing and the in-app companion. Semantic states stay explicit: local is not cloud, inference is not fact, and recording remains visible.</p></section>
      <section class="section"><div class="grid two"><article class="panel"><p class="eyebrow">Source status</p><h2>One portable token source.</h2><p>Edit <code>docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.json</code>. CSS, contrast evidence, the Brand Kit export and this board are generated from it.</p><span class="badge">Updated ${escapeHtml(data.sourceUpdatedAt)}</span><p class="mono">Token CSS integration is recorded separately from component completeness and whole-app accessibility.</p></article><article class="panel"><p class="eyebrow">Quiet Archive · selected beta</p><div style="display:flex;align-items:center;gap:16px">${mark}<span><h2>${escapeHtml(data.brand.name)}</h2><p>${escapeHtml(data.brand.tagline)}</p><span class="badge">${escapeHtml(data.brand.identity)} · beta</span></span></div></article></div></section>
      <section class="section"><p class="eyebrow">Semantic color</p><h2>Meaning before decoration.</h2><p>Each state has light and dark values. Keep text labels and shapes alongside color.</p><div class="grid five" style="margin-top:22px">${swatches}</div></section>
      <section class="section"><p class="eyebrow">Type, space and material</p><div class="grid two"><article class="panel"><p class="eyebrow">Thai-first type</p><div style="font-size:${data.type.display.size};line-height:${data.type.display.lineHeight};font-weight:${data.type.display.weight}">บันทึกให้ชัด<br>และฟังได้นาน</div><p style="font-size:${data.type.body.size};line-height:${data.type.body.lineHeight}">อ่านข้อความและ transcript สบายตา โดยไม่ใช้ viewport-scaled type</p><span class="mono">${escapeHtml(data.type.uiFamily)}</span></article><article class="panel"><p class="eyebrow">Spacing scale</p><div class="spacers">${spacers}</div><p>Controls ${data.radii.control} · surfaces ${data.radii.surface} · dock ${data.radii.dock}</p><p class="mono">Breakpoints: ${escapeHtml(Object.entries(data.breakpoints).map(([name,value])=>name+" "+value+"px").join(" · "))}</p></article></div></section>
      <section class="section"><p class="eyebrow">Interactive states</p><h2>State must explain what happened.</h2><div class="grid two" style="margin-top:22px">${componentRows}</div></section>
      <section class="section"><p class="eyebrow">Cross-surface guidance</p><h2>One identity, fitted to each surface.</h2><div class="grid three" style="margin-top:22px">${surfaceRows}</div></section>
      <section class="section"><p class="eyebrow">Illustrative specimens</p><h2>Examples use the shared tokens.</h2><p>These board previews explain layout and state treatment; they are not screenshots or proof that every screen is implemented.</p><div class="grid two" style="margin-top:22px"><article class="specimen"><p class="eyebrow">Desktop · reader-safe surface</p><div class="desktop-preview"><aside><strong>FUNG</strong><span>Home</span><span>Live</span><span>Review</span><span>Appearance</span></aside><main><header><strong>Meeting review</strong><span class="badge">Local</span></header><div class="mini-card"><strong>Transcript and evidence</strong><p class="mini-muted">Readable content stays on a stable surface; source and runtime states remain visible.</p></div><div><span class="badge warning">AI suggestion · review</span></div></main></div></article><article class="specimen"><p class="eyebrow">Mobile · portrait / voice home</p><div class="device"><div class="mini-status"><span>9:41</span><span class="badge">Local-ready</span></div><div class="voice">⌁</div><div class="mini-card"><strong>เริ่มบันทึกเสียง</strong><p class="mini-muted">บันทึกในเครื่อง · พร้อมใช้งาน</p>${wave()}</div><div class="dock"><span>Home</span><span>Notes</span><b>Voice</b><span>Graph</span><span>Devices</span></div></div></article><article class="specimen"><p class="eyebrow">Mobile · landscape / responsive rail</p><div class="device landscape"><aside class="rail"><span>Home</span><span>Notes</span><b>Voice</b><span>Graph</span><span>Device</span></aside><div><div class="mini-status"><span>FUNG Mobile</span><span class="badge">On-device</span></div><div class="mini-card" style="margin-top:14px"><strong>เสียงบันทึก</strong>${wave()}<p class="mini-muted">Timeline, evidence and actions stay within the Mobile surface.</p></div></div></div></article><article class="specimen"><p class="eyebrow">Web · authenticated dashboard</p><div class="mini-card"><div class="mini-status"><strong>Workspace</strong><span class="badge">Connected to local runtime</span></div><div class="rule"></div><strong>Recent work</strong><p class="mini-muted">Use semantic states to show connection and job status; do not fill empty panels with invented activity.</p><button class="button primary">Open project</button></div></article></div></section>
      <section class="section"><p class="eyebrow">Full token catalog</p><h2>Search names, values and usage.</h2><div class="catalog-tools"><input class="catalog-search" data-token-search type="search" aria-label="Search design tokens" placeholder="Search token name, CSS variable, type, value or description" /><span class="badge">${data.tokens.length} tokens</span></div>${catalog(data)}</section>
      <section class="section"><p class="eyebrow">Accessibility and delivery boundary</p><div class="grid three">${data.accessibility.map((note)=>`<article class="panel"><p>${escapeHtml(note)}</p></article>`).join("")}<article class="panel"><p>Native Figma/Penpot library and full inventory PNG exports remain separate deliverables. The local board is ready for review and token handoff.</p></article></div></section>
      <footer class="footer">Generated from ${escapeHtml("docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.json")} · Do not hand-edit this HTML. <a href="brand/index.html">Open Quiet Archive brand presentation</a>.</footer>
    </div>`,
  });
}

function brandPage(data) {
  const asset = (name) => `../../Mobile/design-system/assets/${escapeHtml(data.brand.assets[name])}`;
  const colors = data.brandColors.map(({name,value})=>`<article class="panel token"><span class="swatch" style="background:${value}"></span><span><strong>${escapeHtml(name)}</strong><span class="mono">${escapeHtml(value)}</span></span></article>`).join("");
  const chapters = data.brandNarrative.map((item,index)=>`<article class="panel"><p class="eyebrow">0${index+1}</p><h3>${escapeHtml(item)}</h3></article>`).join("");
  return shell({
    title: "FUNG Quiet Archive Brand Presentation",
    data,
    page: "brand",
    content: `<div class="page">
      <section class="brand-hero"><p class="eyebrow">${escapeHtml(data.brand.tagline)} · selected beta identity</p><div class="wordmark"><img class="quiet-mark" src="${asset("markWhite")}" alt="FUNG Quiet Archive mark" />${escapeHtml(data.brand.name)}</div><p class="lede">A calm identity for private voice work. Quiet Archive keeps an open centre for the note, recording and evidence that remain available to the user.</p><span class="badge">Quiet Archive · beta</span></section>
      <section class="section"><div class="grid three">${chapters}</div></section>
      <section class="section"><p class="eyebrow">Quiet Archive construction</p><h2>An open archive, drawn as one ribbon.</h2><div class="grid two" style="margin-top:22px"><article class="logo-grid guide"><img class="quiet-mark large" src="${asset("markWhite")}" alt="Quiet Archive construction mark" /></article><article class="panel"><h3>Master rules</h3><div class="rule"></div><p>The flat master retains a continuous rounded ribbon, an off-centre archive aperture and two diagonal fold cuts. Material texture is presentation-only.</p><div style="display:flex;gap:14px;align-items:center;margin-top:18px"><img class="quiet-mark" src="${asset("markInk")}" alt="Ink mark on a light surface" /><img class="quiet-mark" src="${asset("markWhite")}" style="background:${data.themes.dark.canvas};border-radius:24px;padding:12px" alt="White mark on an ink surface" /><img class="quiet-mark icon" src="${asset("appIcon")}" alt="Android icon preview" /></div></article></div></section>
      <section class="section"><p class="eyebrow">Brand colors</p><h2>Quiet, semantic and operational.</h2><div class="grid three" style="margin-top:22px">${colors}</div></section>
      <section class="section"><p class="eyebrow">Typography</p><div class="grid two"><article class="panel"><div style="font-size:${data.type.display.size};line-height:${data.type.display.lineHeight};font-weight:${data.type.display.weight}">ฟังให้ครบ<br>ก่อนสรุป</div><p>Thai content has zero letter-spacing and a reading-first body scale.</p></article><article class="panel"><h3>Icon and material voice</h3><p>Shallow material depth explains ownership. Focus remains visible independently of borders and shadows; reduced motion and readable solid fallbacks are documented.</p><span class="badge">Quiet Archive selected · beta</span></article></div></section>
      <section class="section"><p class="eyebrow">Product application</p><h2>The identity becomes operational UI.</h2><div class="grid two" style="margin-top:22px"><article class="specimen"><div class="device"><div class="mini-status"><span>FUNG</span><span>9:41</span></div><div class="voice">⌁</div><p style="text-align:center;margin:0"><strong>กดค้างเพื่อสั่งงาน</strong><br><span class="mini-muted">ทำงานบนมือถือ</span></p><div class="mini-card" style="margin-top:18px">โน้ตล่าสุด${wave()}</div><div class="dock"><span>Home</span><span>Notes</span><b>Voice</b><span>Graph</span><span>Devices</span></div></div></article><article class="panel"><p class="eyebrow">Truthful states</p><h3>Identity keeps execution context visible.</h3><p><span class="badge">On-device</span> means local processing. <span class="badge warning">Inference</span> stays pending. <span class="badge record">Recording</span> remains visually and textually explicit.</p></article></div></section>
      <footer class="footer">Generated from the shared DTCG token source · <a href="../index.html">Open shared design-system board</a>.</footer>
    </div>`,
  });
}

function redirectPage(title, target) {
  const href = escapeHtml(target);
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta http-equiv="refresh" content="0; url=${href}"><link rel="canonical" href="${href}"><title>${escapeHtml(title)}</title></head><body><p>This design-system page moved. <a href="${href}">Open the shared FUNG design system</a>.</p></body></html>`;
}

async function compile() {
  const source = JSON.parse(await readFile(sourcePath, "utf8"));
  const data = makeData(source);
  return {
    system: systemPage(data),
    brand: brandPage(data),
    legacySystem: redirectPage("FUNG Mobile Design System moved", "../../design-system/index.html"),
    legacyBrand: redirectPage("FUNG Brand Presentation moved", "../../../design-system/brand/index.html"),
  };
}

async function run(mode) {
  const rendered = await compile();
  const stale = [];
  for (const [key, path] of Object.entries(outputs)) {
    if (mode === "render") {
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, rendered[key], "utf8");
    } else {
      let actual = "";
      try { actual = await readFile(path, "utf8"); } catch { /* reported below */ }
      if (actual !== rendered[key]) stale.push(path);
    }
  }
  if (stale.length) throw new Error(`Generated design-system artifacts are missing or stale: ${stale.join(", ")}. Run design-system:render.`);
}

const mode = process.argv[2] ?? "render";
try {
  if (!["render", "check"].includes(mode)) throw new Error(`Unknown mode: ${mode}. Use render or check.`);
  await run(mode);
  console.log(`FUNG shared design-system ${mode} passed.`);
} catch (error) {
  console.error(`FUNG shared design-system ${mode} failed: ${error.message}`);
  process.exitCode = 1;
}
