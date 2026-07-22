---
version: "0.1.1b"
created_at: "2026-07-22T00:50:19+07:00,ATHER"
last_update: "2026-07-22T01:59:15+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "web-experience"
  doc_type: "product-design-spec"
  scope: "Vercel landing page"
  language: "Thai-first"
---

# FUNG Web Landing Page Proposal

## Decision request

Approve a new public, scroll-driven landing page for the Vercel deployment while preserving the existing FUNG web app and desktop entry paths.

## Complexity and risk

- Complexity: **C-2 — Documentation-Driven Implementation**
- Risk: **MEDIUM**
- Reason: the work introduces a new public route, changes the web entry decision, adds animation and responsive behavior, and must not break the Tauri desktop or existing mobile/desktop surfaces.

## Assumptions

1. The requested sentence is the primary brand idea and hero headline, preserving the informal Thai spelling `เค้า` until the product owner requests editorial normalization to `เขา`.
2. The landing page explains what FUNG can infer from observable context, pauses, transcript, and linked evidence. It must not claim literal mind-reading or unsupported emotion detection.
3. GenesisBlockDB remains embedded/local-first. Supabase remains the optional cloud control plane, and Vercel hosts the web experience.
4. The existing application remains functional as a separate route and as the direct Tauri surface.

## Parent and peer alignment

- Aligns with `docs/Desktop/DESIGN_SYSTEM.md`: disciplined hierarchy, porcelain surfaces, ink typography, muted sage, restrained indigo, warm-metal detail, and no dominant purple gradient.
- Aligns with `docs/Mobile/DESIGN_SYSTEM.md`: Thai-first typography, tactile restraint, meaningful motion, and light/dark accessibility.
- Aligns with `docs/Mobile/PRODUCT_UX_SPEC.md`: local-first operation and account-optional behavior.
- Aligns with `docs/WEB_PRODUCTION_DEPLOYMENT.md`: Vercel remains the web deployment target.
- Does not move local audio, transcript, graph, vector, or provenance data into Supabase.

## Brand idea

### Hero headline

> FUNG ในสิ่งที่เค้าไม่ได้พูด
> แล้วเราจะได้ยินสิ่งที่เค้าต้องการ

### Supporting copy

> จับบริบทจากเสียง ความเงียบ และสิ่งที่เชื่อมโยงกัน — โดยข้อมูลหลักยังอยู่บนอุปกรณ์ของคุณ

### Meaning boundary

The promise means that FUNG helps people notice context that was not stated directly by connecting pauses, timestamps, transcript segments, notes, and provenance. Copy must avoid claims that FUNG knows a person's private intention without evidence.

## Visual direction

The page uses one continuous porcelain “memory ribbon” as its narrative object. It begins as captured audio, reveals pauses and transcript, unfolds into linked knowledge, shows the local architecture, and resolves into the FUNG memory mark at the closing CTA.

- Warm porcelain: `#F5F2EB`
- Deep ink: `#17181B`
- Graphite: secondary text and linework
- Muted sage: `#71847C`
- Deep indigo: `#3D4F82`
- Warm metal: provenance markers and fine details only
- Typography: large Thai editorial display paired with a highly legible Thai-compatible sans for UI copy
- Exclusions: purple SaaS gradients, neon glow, generic bento grids, glass cards, stock photos, fake metrics, and decorative motion without meaning

### Approved typography patch

- Display and Thai-first UI: `IBM Plex Sans Thai` at 300, 400, 500, and 600.
- Latin wordmark and compact UI chrome: `DM Sans` at 400, 500, and 600.
- Font files are bundled locally through `@fontsource`; the landing page does not request Google Fonts or another third-party font CDN at runtime.
- Retired from the landing surface: `Th Sarabun New`, `Leelawadee UI`, and `Georgia` fallback treatment.

## Concept references

1. [Hero and first transition](references/landing-concept/01-hero.png)
2. [Voice-to-knowledge narrative](references/landing-concept/02-voice-to-knowledge.png)
3. [Local-first architecture](references/landing-concept/03-local-architecture.png)
4. [Closing manifesto and footer](references/landing-concept/04-closing.png)

The concept images define composition and art direction. Production copy and product UI will be implemented as accessible HTML, CSS, SVG, and React rather than baking text into raster images.

## Page narrative

### 1. Hero — Hear the unsaid

- Large brand headline and short proof-oriented supporting copy
- Primary CTA: `เปิด FUNG`
- Secondary CTA: `ดูวิธีทำงาน`
- The porcelain ribbon enters the viewport with waveform, pause, timestamp, and source markers

### 2. From voice to verifiable knowledge

Pinned scroll sequence with three states on one continuous object:

1. `บันทึก` — waveform capture
2. `เข้าใจ` — timestamped Thai transcript and speaker context
3. `เชื่อมโยง` — knowledge relationships that retain provenance

### 3. Local by default. Connected by choice.

- The local core is visually dominant: FUNG Desktop + GenesisBlockDB
- On-device data: audio, notes, transcript, graph, vectors, and provenance
- Optional cloud is visually lighter: Supabase for Auth, metadata, and RLS; Vercel for web UI
- Required truth statement: `ใช้งานแบบ Local ได้โดยไม่ต้องมีบัญชี`

### 4. Closing manifesto

- Headline: `เก็บความคิดไว้ใกล้ตัว`
- Primary CTA: `เปิด FUNG`
- Secondary CTA: `ดาวน์โหลด Desktop`
- Spacious dark-ink footer with Product, Privacy, Documentation, and deployment status links

## Route contract

- `/` — public landing page on Vercel
- `/app` — existing interactive web application
- `/auth/callback` — authorization callback; unchanged by landing navigation
- Tauri runtime — opens the existing desktop application directly, never the marketing landing page
- Existing explicit surface selection remains supported under the app route

## Motion contract

- The ribbon moves by transform and opacity only; no layout-driven scroll jank.
- Scroll progress controls reveal state, never essential information availability.
- Keyboard and touch users can access the complete narrative without precise scroll gestures.
- `prefers-reduced-motion: reduce` renders stable sections without pinning or parallax.
- No autoplay audio.
- Implementation should prefer native CSS/SVG, `IntersectionObserver`, and a small requestAnimationFrame scroll-progress layer. A WebGL/Three.js dependency is out of scope unless a later proof shows it is necessary.

## Responsive behavior

- Desktop: cinematic pinned transitions and wide editorial composition
- Tablet: reduced depth and shorter pinned ranges
- Mobile: linear narrative, no sticky overlap, full-width readable Thai text, and simplified ribbon geometry
- Navigation collapses without hiding Privacy or the primary CTA

## Accessibility, SEO, and performance

- Semantic heading order and landmark structure
- WCAG AA text contrast and visible keyboard focus
- Decorative ribbon hidden from assistive technology; meaningful transcript/provenance available as text
- Thai title, description, Open Graph metadata, canonical URL, and share image
- No horizontal overflow at 390, 768, 1024, and 1440 CSS pixels
- Target LCP <= 2.5 s and CLS <= 0.1 on the production landing page
- Concept PNGs are references only and must not become multi-megabyte above-the-fold payloads

## Implementation plan after approval

1. Introduce route-safe landing/app entry separation -> verify Vercel `/`, `/app`, auth callback, and Tauri startup.
2. Build semantic page structure and tokens -> verify responsive layout and Thai typography.
3. Build the code-native ribbon and scroll states -> verify reduced-motion and keyboard/touch behavior.
4. Connect CTAs and production metadata -> verify links, SEO, and optional-account wording.
5. Run unit/build/browser checks and visual comparison against all four concepts -> verify acceptance criteria.
6. Deploy a Vercel preview, review the full scroll story, then promote to production only after the preview passes.

## Acceptance criteria

- The four-section story is present and visually continuous from hero to footer.
- The approved hero sentence is shown exactly.
- The page does not imply that cloud use or account creation is required.
- GenesisBlockDB is represented as local/embedded, not as a hosted Supabase replacement.
- Existing web app, OAuth callback, mobile/desktop surface selection, and Tauri startup still work.
- Reduced-motion, keyboard navigation, responsive layouts, and production build pass.
- Browser screenshots at desktop and mobile materially match the approved concept direction.
- A Vercel preview is verified before production promotion.

## Out of scope

- Pricing, testimonials, fabricated adoption metrics, CMS, blog, localization beyond Thai-first copy, and a public GenesisBlockDB service
- Changes to the Supabase database schema or OAuth contract
- Refactoring unrelated desktop/mobile UI

## Version diff

### 0.1.1b

- Approved and implemented a self-hosted modern Thai typography system: IBM Plex Sans Thai + DM Sans.
- Preserved the two-line desktop hero rhythm and verified a non-overflowing mobile treatment.

### 0.1.0b

- Added the candidate landing-page narrative, visual system, route contract, motion contract, and acceptance gates.
- Added four full-page section concepts.
- Established the product-owner sentence as the hero brand idea.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-07-22 | beta | Replaced dated serif/system fallback with self-hosted IBM Plex Sans Thai + DM Sans | — | ATHER |
| 0.1.0b | 2026-07-22 | candidate | Initial scroll-driven FUNG landing-page proposal | — | ATHER |
