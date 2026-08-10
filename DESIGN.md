---
version: "0.1.0b"
created_at: "2026-08-09T00:00:00+07:00,ATHER"
last_update: "2026-08-09T00:00:00+07:00,ATHER"
status: "draft"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-system"
  scope: "FUNG desktop and Meeting Intelligence UI"
name: "FUNG"
description: "A calm, local-first instrument for voice, evidence and deliberate meeting intelligence."
colors:
  porcelain: "#f4f1ea"
  porcelain-top: "#fffaf0"
  porcelain-mid: "#eee5d6"
  porcelain-low: "#d8ccba"
  ink: "#191a1d"
  graphite: "#5f6268"
  graphite-soft: "#848990"
  local-sage: "#6e897d"
  evidence-indigo: "#3d4f82"
  provenance-metal: "#9a8260"
  destructive-signal: "#b34b4b"
typography:
  ui:
    fontFamily: "SF Pro Display, SF Pro Text, Hiragino Sans, Yu Gothic UI, Segoe UI, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.35
    letterSpacing: "0"
  thai-web:
    fontFamily: "IBM Plex Sans Thai, DM Sans, Segoe UI, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0"
rounded:
  surface: "8px"
  dock: "14px"
  pill: "999px"
spacing:
  1: "4px"
  2: "8px"
  3: "10px"
  4: "12px"
  5: "14px"
  6: "18px"
components:
  button-primary:
    backgroundColor: "{colors.evidence-indigo}"
    textColor: "#ffffff"
    rounded: "{rounded.pill}"
    height: "42px"
    padding: "0 12px"
  signal-card:
    backgroundColor: "{colors.porcelain-top}"
    textColor: "{colors.graphite}"
    rounded: "{rounded.surface}"
    padding: "12px"
  segmented-active:
    backgroundColor: "{colors.evidence-indigo}"
    textColor: "#ffffff"
    rounded: "{rounded.pill}"
    height: "28px"
    padding: "0 8px"
---

# Design System: FUNG

## Overview

**Creative North Star: "The Quiet Evidence Desk"**

FUNG is a tactile desktop instrument for work that must remain understandable after the moment has passed. The working surface is a porcelain command deck, not a dashboard collage. It uses molded surfaces, restrained semantic color and compact familiar controls so capture, transcript and evidence remain readable for long sessions.

Call.md may inform information hierarchy, such as an immediately legible meeting setup, live transcript, metrics strip, nudge area and results panel. FUNG must implement these patterns in its own Tauri/React system and local-first policy. No Call.md logo, copy, screenshots, assets, Tailwind bundle or component code is part of this system.

**Key Characteristics:**

- Fixed, operational hierarchy: current work, proof of state, then supporting detail.
- Local and evidence-backed states are explicit and calm.
- Shallow physical depth explains pressability and ownership, never decoration.
- One clear primary action per active Meeting Mode stage.
- Product UI behaves as a familiar working tool, not a sales copilot.

## Colors

The palette is a restrained porcelain field with semantic color reserved for information that changes a decision.

### Primary

- **Evidence Indigo:** Use `evidence-indigo` for selected transcript/evidence context, the primary action and active segmented control. It is a focus signal, not a decorative brand wash.

### Secondary

- **Local Sage:** Use `local-sage` for confirmed local processing, healthy capture and privacy-safe states.
- **Provenance Metal:** Use `provenance-metal` for durable provenance, export readiness and small premium markers only.

### Neutral

- **Porcelain Field:** `porcelain`, `porcelain-top`, `porcelain-mid` and `porcelain-low` form the existing molded surface ramp.
- **Ink and Graphite:** `ink` is the long-reading foreground. `graphite` is supporting text. `graphite-soft` is metadata only, never body copy where it would miss contrast.

### Semantic State

- **Destructive Signal:** `destructive-signal` is reserved for recording-critical failure, data-risk and destructive action confirmation. It must not represent ordinary AI uncertainty.
- Inference uses evidence-indigo with an explicit text label and source span. It never receives a personality, confidence theatre or green “approved” treatment.

**The Semantic Accent Rule.** A semantic color appears only when it explains current state, action priority or provenance. Inactive UI remains neutral.

## Typography

**Desktop UI Font:** SF Pro Display/SF Pro Text with Hiragino Sans, Yu Gothic UI and Segoe UI fallbacks.

**Thai Web Font:** IBM Plex Sans Thai with DM Sans and Segoe UI fallbacks.

**Character:** Desktop UI is compact and deliberately quiet. Thai copy is reading-first, with zero letter spacing and enough line height to keep transcript review comfortable.

### Hierarchy

- **Workspace title** (600, 20px, 1): current project or active Meeting Mode context.
- **Panel title** (600, 18px, 1.15): a work surface or focused task.
- **Action label** (600, 13–14px, 1.2): a button, signal card or status control.
- **Body / transcript** (400, 14–16px, 1.35–1.5): readable evidence and instructions. Prose is constrained to roughly 65–75ch when it is not a transcript or table.
- **Metadata** (400–600, 11–12px, 1.2): timestamp, provider, state and source label. Uppercase is allowed only for short system labels, never Thai sentences or user-authored transcript text.

**The Evidence-First Type Rule.** A transcript timestamp, speaker label and inference boundary must remain visually distinct from generated prose at all zoom levels.

## Elevation

FUNG uses shallow, directional elevation to communicate a molded porcelain work surface. Inset depth indicates a pressed or selected control. Raised depth is reserved for the command deck rim and the notch-owned floating controls defined in `docs/Desktop/03_LAYOUT.md`. Tonal layers, not broad soft shadows, establish most grouping.

### Shadow Vocabulary

- **Panel Material:** existing `--shadow` gives the outer command deck a shallow, two-direction material edge.
- **Pressed Control:** existing `--pressed-shadow` communicates selection without moving the surrounding layout.
- **Inset Surface:** bevel highlights and low-contrast borders distinguish a readable work zone from the enclosing deck.

**The Ownership Before Polish Rule.** If a surface looks detached, correct its zone or grid ownership before changing blur, opacity, shadow or color. Signal cards remain inside the panel-owned signal sector.

## Components

### Buttons

- **Shape:** primary working actions use a compact pill (`999px`) only when the control is an action. Panels, transcript rows and data surfaces stay gently curved (`8px`).
- **Primary:** Evidence Indigo, white text, minimum `42px` height. Label must be verb + object, such as “Start recording” or “Export bundle”.
- **Secondary:** Porcelain surface with an explicit neutral border and graphite text. It must not visually compete with the current primary action.
- **Focus / disabled / loading:** Focus remains visible against porcelain. Disabled controls state why when the capability is unavailable. Loading reserves the control width and does not replace durable job state.

### Signal Cards

- **Role:** a 2 × 2 in-panel summary of capture, transcript, intelligence and export health.
- **Shape and material:** `8px` surface, `12px` padding, low-contrast structural border and optional pressed state.
- **Content:** status label, short value and one sentence explaining what changed. Do not render decorative metrics or a generic “health score”.
- **Reference adaptation:** retain Call.md’s at-a-glance live status concept, but bind cards to FUNG jobs, source evidence and local policy instead of a hosted copilot state.

### Segmented Controls and Navigation

- **Role:** switch P1–P4 Meeting Mode stages or tightly bounded modes without adding route depth.
- **Shape:** a `4px` enclosing track with pill items. Active is Evidence Indigo; inactive items are graphite on a neutral field.
- **Behavior:** keyboard-operable, selected state announced, and never used as a substitute for unrelated navigation.

### Transcript and Evidence Rows

- **Role:** show time, editable speaker label, transcript text, source state and evidence marker in a stable reading order.
- **Surface:** unframed working material by default. Add an `8px` porcelain selection surface only for focus, edit, conflict or evidence pinning.
- **States:** pending transcript, low-confidence span, selected evidence, speaker-editing, model-unavailable and export-ready. Every state uses text plus color.

### Live Assistance and MCP Result Surface

- **Role:** a bounded panel for a nudge, suggested next question or an approved external-tool result.
- **Rule:** show origin, policy state, timestamp and evidence span before generated content. External action controls use an explicit confirmation button; they never run from transcript keywords alone.
- **Rendering:** tool output is untrusted content. Links, Markdown and embedded data require sanitization and a visible source label.

### Empty, Error and Offline States

- **Empty:** teach the next local action, for example “Choose an audio file to create a transcript job.”
- **Error:** state whether source audio and chunks remain safe, then offer recover, retry or diagnostics.
- **Offline:** local mode is normal. Hide only unavailable cloud-provider controls, not capture/review work.

## Do's and Don'ts

### Do:

- **Do** preserve the existing FUNG porcelain, bevel and compact-control identity when adding meeting intelligence.
- **Do** use Call.md only as a reference for task order and information hierarchy: preparation, live proof, transcript, intelligence, export.
- **Do** show source, timestamp, provider and uncertainty wherever generated output can change a user decision.
- **Do** maintain a single dominant action per Meeting Mode stage and reserve colour for meaningful state.
- **Do** support keyboard focus, reduced motion and a visible equivalent for every audio-only state.
- **Do** use a controlled result panel for MCP/tool output, with policy and user-approval state visible.

### Don't:

- **Don't** copy Call.md’s logo, screenshots, product copy, Tailwind bundle, Electron IPC code, tRPC bindings or VideoDB-specific UI text.
- **Don't** add a card inside a card, colored side stripes, gradient text, decorative glass layers or a hero-metric dashboard pattern.
- **Don't** move signal cards outside their panel-owned sector or reintroduce a floating signal FAB.
- **Don't** use dark blue/slate-heavy inactive UI, purple gradients, neon accents, decorative gradient blobs or a marketing landing page as the desktop first screen.
- **Don't** use colour alone to communicate recording, privacy, inference, failure or accessibility-critical state.
- **Don't** label an AI suggestion, speaker identity or conversation metric as fact without its evidence boundary.
- **Don't** animate layout during recording or gate readable content behind an animation. Respect `prefers-reduced-motion`.

### Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Captured FUNG’s current desktop tokens and defined a lawful, local-first design adaptation boundary for Call.md-inspired meeting UI patterns. |

### CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-08-09 | draft | Added root machine-readable design system and component rules for FUNG Meeting Intelligence UI. | N/A — uncommitted | ATHER |
