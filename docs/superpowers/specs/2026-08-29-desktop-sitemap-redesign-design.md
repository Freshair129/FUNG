---
status: "draft"
created_at: "2026-08-29"
supersedes: "docs/Desktop/05-sitemap-ia.md (navigation/rail sections only)"
keeps_intact: "docs/Desktop/07-meeting-mode.md (P1-P4 content model is unchanged)"
scope: "FUNG desktop shell — navigation structure and left rail"
---

# Desktop Sitemap Redesign — Home + Notched Instrument Rail

## 1. Problem

Two concrete problems surfaced while debugging the login flow and reviewing the shell against its own IA docs:

1. **No real "home."** The app boots straight into a P-page. There is no landing view with a hero record action and a list of recent meetings — every session starts by picking a P first.
2. **The left rail is overloaded and undiscoverable.** Two separate rails stack vertically: the `PAGES` card (P1-P4) and an 11-icon `fab-sidebar` (record, import, pair, playback, export, runtime, settings, TTS providers, cloud providers, account, Zoom import). The bottom five icons require scrolling to reach and carry no labels — this is what made the account/login button (`UserCircle`, "บัญชี & อุปกรณ์") nearly impossible to find during the earlier debugging session. Account, Backup, TTS, Cloud, and Zoom currently open as stacked modal overlays rather than living anywhere in the sitemap, which also contradicts `05-sitemap-ia.md`'s own rule: *"Deep settings stay within P4 unless a future architecture introduces separate modal pages."*

Additionally, the topbar segmented control (Capture / Transcript / Summary / Runtime) already mirrors the `PAGES` list per `05-sitemap-ia.md`: *"P rail owns page identity; topbar segmented control mirrors it for fast access."* The two navigation surfaces are redundant.

## 2. Goals

- Give the app a real first screen: hero "start recording" action + recent meetings, in one view.
- Cut the left rail from 11 unlabeled icons (plus a separate 4-item PAGES card) down to 6 buttons + a live L/R level meter.
- Remove the redundant `PAGES` list — the topbar segmented control becomes the sole P1-P4 switcher.
- Consolidate Settings, TTS Providers, Cloud Providers, Account & Devices, and Zoom import behind one entry point instead of five separate modal triggers.
- **Preserve the shell's existing visual identity** — the "subtract HUD" notch silhouette (`PANEL_PATH` in `App.tsx`) and its double-stroke rim treatment — rather than flattening it into a generic rounded bar.

## 3. Non-goals

- No change to the P1-P4 *content* model. Every tile, agent-card behavior, and signal defined in `docs/Desktop/07-meeting-mode.md` stays exactly as specified.
- No change to the `power-dock` / `power-radial` island (bottom-left close/minimize control). It keeps its current position, shape, and behavior untouched.
- No visual redesign of the center deck, agent card, Sector C, or signal cards.
- Live audio metering wiring (actually driving the VU meter from the mic input stream) is scoped separately — see §8 Open Questions.

## 4. Navigation structure

### 4.1 Before

```
PAGES card (P1-P4)          <- left rail, top
fab-sidebar (11 icons)       <- left rail, below it
Topbar segmented control     <- mirrors PAGES, redundant
```

### 4.2 After

```
Topbar segmented control     <- sole P1-P4 switcher (Capture / Transcript / Summary / Runtime)
Notched instrument rail       <- VU meter + 6 buttons, left edge, floating in the outer gutter
```

- **Home** is a new top-level screen, entered on launch and via a persistent 🏠 breadcrumb in the topbar once inside a meeting. It is *not* one of P1-P4 — it has no segmented-control state of its own.
- Selecting "Start recording" or opening a past meeting from Home transitions into the existing P1-P4 workflow (content unchanged from `07-meeting-mode.md`), with the topbar segmented control now the only way to move between P1-P4.
- The `PAGES` card component is removed. Its screen-space is absorbed by the rail redesign (§5).

### 4.3 Home screen content

| Element | Detail |
| --- | --- |
| Hero action | "🎤 เริ่มบันทึกประชุม" (Start recording) — primary button, same visual weight as today's `quick-action--primary` |
| Secondary action | "⬆ นำเข้าไฟล์เสียง" (Import audio file) — secondary button beside the hero |
| Recent list | Reuses the existing `libraryItems` data (`App.tsx`) — meeting title, date, and current stage (Transcript %, Summary status, Exported) per row |
| Rail state | Present but idle: VU meter shows no signal (empty/dim bars), record button not in "active" state |

Clicking a recent-list row or the hero action navigates into P1 (Capture) for that meeting, topbar now showing the segmented control and a "🏠 ← \<meeting title\>" breadcrumb.

## 5. Notched instrument rail

### 5.1 Why notched, not a plain rounded bar

The shell's actual panel silhouette is defined by a literal SVG subtract path, not a plain rectangle:

```
const PANEL_PATH =
  "M 40,12 H 800 A 20 20 0 0 1 820,32 V 54 A 20 20 0 0 0 840,74 H 1248 A 20 20 0 0 1 1268,94 V 688 A 20 20 0 0 1 1248,708 H 112 A 20 20 0 0 1 92,688 V 350 A 20 20 0 0 0 72,330 H 32 A 20 20 0 0 1 12,310 V 40 A 28 28 0 0 1 40,12 Z";
```

This path cuts two deliberate concave notches — one in the top edge, one in the left edge. The **left-edge notch sits at y≈330-350 in the canonical 1280×720 stage**, which is *exactly* the seam between where the old `PAGES` card ended and the old `fab-sidebar` began. The original design already treated these as two distinct zones joined by a notch, not one continuous bar. A flat merged rail erases that intentional cut. The redesigned rail keeps the notch at this same seam.

The panel is also rendered with a double-stroke rim (`panel-rim__stroke--outer`: `rgba(82,70,54,.24)`, `panel-rim__stroke--inner`: `rgba(255,255,255,.72)`, plus a soft drop-shadow glow), not a flat single-color border. The rail should use the same two-stroke treatment.

The outer `.stage-wrap` (1304×744) is wider than the panel silhouette (`.panel-rim`, inset 12px → 1280×720). That 12px gutter is where floating chrome is meant to visually extend past the panel's own edge. The rail's left edge should sit close to the window edge, in that outer gutter, rather than flush against the panel's inner boundary — this is what gives it the "floating instrument dock" feel instead of "another box inside the card."

### 5.2 Two zones, one rail

| Zone | Content | Notes |
| --- | --- | --- |
| Upper (wider, extends further into the outer gutter) | Live L/R VU meter, "REC" label | Segmented vertical bars, green → yellow → red (standard audio metering: green = normal, yellow = near-peak, red = overload/clipping) |
| *(notch here — same seam as the original `PAGES`/`fab-sidebar` boundary)* | | |
| Lower (narrower, stepped in) | 6 buttons | See §5.3 |

### 5.3 The 6 buttons

| # | Icon | Action | Replaces | Rationale |
| --- | --- | --- | --- | --- |
| 1 | ⏺ Record | Start/pause recording | `handleCreateJob("transcript.transcribe")`'s mic trigger | Hero action of the app; stays pinned at the top of the button group, styled with the existing `.sidebar-action.is-active` indigo gradient (`#6573a0` → `#34456c`) |
| 2 | ↑ Import | Import audio file | Upload icon | Second most frequent action |
| 3 | ▶ Playback | Play back current recording | Play icon | Used to verify audio during/after capture |
| 4 | ↓ Export | Export subtitles/bundle | `HardDriveDownload` icon | Workflow endpoint, kept one tap away |
| 5 | 🔗 Pair device | Open device pairing (FUNGWIRE) | `Link2` icon | Multi-device pairing is a core feature, not buried in a modal |
| 6 | ⚙ Settings | Open the consolidated Settings page | `SlidersHorizontal`, `Volume2` (TTS), `Cloud` (Cloud Providers), `UserCircle` (Account), `Cloud` (Zoom) — **5 buttons become 1** | See §6 |

**Removed from the rail entirely:** the standalone "Runtime" (`Activity`) icon. Runtime/API status becomes an indicator inside the consolidated Settings page (or a small status chip) rather than a floating rail icon — it is not a frequent, one-tap action the way record/import/export are.

Buttons use the real `.sidebar-action` token values: `48×40` (scaled to the new button size), `border-radius: 14px`→ rendered here as ~11-12px at the smaller footprint, cream gradient `linear-gradient(145deg, #fffdf7, #e5dac7)`, text/icon color `var(--graphite)` (`#5f6268`), active/record state using the indigo gradient above.

### 5.4 What stays untouched

- `power-dock` / `power-radial` (bottom-left close/minimize island) — same position, same "พับจอ" / "ปิด" behavior, no visual change.
- The recovery notice banner at the top of the window — out of scope for this pass.

## 6. Consolidated Settings

Settings, TTS Providers, Cloud Providers, Account & Devices, and Zoom import currently each open as an independent stacked modal (`AccountLoginPanel`, `DevicePairingPanel`, `BackupPanel`, `TtsProviderPanel`, `CloudProvidersPanel`, `ZoomPanel`), reachable only by finding the right icon in the crowded rail.

**New behavior:** the single ⚙ Settings button opens one page (or one modal, TBD — see Open Questions) with the existing panels as tabs/sections inside it:

- Account & Devices (login, device pairing — `AccountLoginPanel` + `DevicePairingPanel` content, currently already co-located per `AccountLoginPanel.css`'s comment: *"Overlay + stack wrapper for AccountLoginPanel and DevicePairingPanel"*)
- TTS Providers
- Cloud Providers (+ Zoom import folded in as a provider)
- Backup
- Runtime/API status (moved here from the removed rail icon)

This directly resolves the IA violation found earlier: `05-sitemap-ia.md` already says deep settings should consolidate rather than spawn separate modal pages — this groups five triggers into one, matching that intent.

## 7. What is explicitly preserved

- All P1-P4 tile content, Agent Card copy, Sector C activity/event definitions, and Signal card meanings from `docs/Desktop/07-meeting-mode.md` — unchanged.
- The `power-radial` island.
- The porcelain/neumorphic material language (`--bg-porcelain`, `--sage`, `--indigo`, `--metal`, `--signal` tokens, embossed shadow system) — extended, not replaced.
- The 1280×720 canonical stage and `PANEL_PATH` shell shape.

## 8. Open questions (for the implementation plan to resolve)

1. **Settings surface: modal or dedicated screen?** This spec assumes a single consolidated surface but doesn't mandate modal vs. full-screen — the implementation plan should pick one and follow the existing `AccountLoginPanel.css` "shared backdrop" pattern if modal.
2. **VU meter data source.** This spec covers the meter's visual design only. Wiring it to a live input-level stream (WASAPI loopback / mic RMS) is separate follow-up work.
3. **Icon set.** The button icons in this spec were prototyped as hand-drawn vectors for mockup purposes. Implementation should pull from the project's existing icon library (`lucide-react`, already used throughout `App.tsx`) for production.
4. **Exact rail pixel dimensions** for the notch geometry should be re-derived from the live 1280×720 canonical stage during implementation, not eyeballed from a screenshot as this spec's mockups were.

## 9. Acceptance criteria

- Launching the app lands on Home, showing a hero record action and a recent-meetings list in one view, with no P selected.
- The `PAGES` card is removed; the topbar segmented control is the only P1-P4 switcher.
- The left rail shows exactly 6 buttons + a live-capable L/R VU meter, with a visible notch at the same seam the original `PANEL_PATH` already cuts.
- Account & Devices, TTS Providers, Cloud Providers, Backup, and Zoom import are reachable from a single Settings entry point, not five separate rail icons.
- `power-radial` behavior and position are bit-for-bit unchanged.
- No P1-P4 tile, Agent Card, Sector C, or Signal content changes from what `07-meeting-mode.md` already specifies.

## Reference mockups

Produced during brainstorming, saved under `.superpowers/brainstorm/` (git-ignored, session-local):

- Navigation structure trade-off comparison (3 options — Home+workflow hybrid chosen)
- Full before/after wireframe (Home screen + in-meeting workflow)
- Rail composite over the real app screenshot, verified pixel-by-pixel against the actual `PANEL_PATH` notch geometry and real `styles.css` tokens
