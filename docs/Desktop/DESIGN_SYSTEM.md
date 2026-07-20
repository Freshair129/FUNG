---
version: "0.2.0b"
created_at: "2026-07-05T00:00:00+07:00,ATHER"
last_update: "2026-07-05T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-system"
  scope: "FUNG"
---

# Design System - Apple Japan, Quiet Luxury, Minimal Cozy, Skeuomorphic

## Design Intent

FUNG ต้องให้ความรู้สึกเหมือนเครื่องมือบันทึกเสียงและอ่านความหมายที่สงบ น่าเชื่อถือ และใช้งานได้นานโดยไม่เหนื่อยสายตา. ไม่ใช่ landing page, ไม่ใช่ SaaS dashboard ที่แข็ง, และไม่ใช่ DAW ที่รกตั้งแต่หน้าแรก.

Visual direction is tactile skeuomorphism: molded porcelain surfaces, soft bevels, pressed buttons, inset meters, and quiet mechanical affordance. Glass can remain as a subtle material layer, but the primary read must be physical and grounded rather than floating glass cards.

## Product Feel

- Calm.
- Precise.
- Local and private.
- Warm minimal.
- Professional without enterprise heaviness.
- Tactile and grounded.

## First Screen

หน้าแรกต้องเป็น usable workspace:

- Project list.
- New recording button.
- Import audio button.
- Recent recording state.
- Runtime/model health indicator.

ห้ามเริ่มด้วย hero marketing page.

## Visual Language

### Color Direction

| Token | Use |
| --- | --- |
| Porcelain | Main background |
| Warm white | Surface |
| Ink black | Primary text |
| Soft graphite | Secondary text and icons |
| Muted sage | Active local/private state |
| Deep indigo | Focus and selected transcript |
| Warm metal | Small premium accent |
| Signal red | destructive/error only |

Avoid:

- Dominant purple gradients.
- Dark blue/slate-heavy UI.
- Beige-only palette.
- Decorative gradient blobs.

### Shape and Material

- Card radius max 8px.
- Tool controls should be compact and stable.
- Avoid cards inside cards.
- Timeline and transcript should be unframed work surfaces, not decorative panels.
- Primary panels use molded/inset surfaces with visible bevel direction.
- Active controls may look pressed, but must not shift layout.
- Floating controls are allowed only where the layout spec defines FAB notches.

### Typography

- Clear, quiet, readable.
- No viewport-scaled font sizes.
- Letter spacing: 0.
- Long transcript text must prioritize reading comfort.

## Core Views

### Library

Purpose:

- Find and resume projects quickly.

Required UI:

- Project list.
- Search.
- Status chips for recording/transcribed/exported.
- Model/runtime warning if unavailable.

### Recording

Purpose:

- Long-running capture with confidence.

Required UI:

- Large timer.
- Waveform level.
- Chunk/save indicator.
- Pause/stop controls.
- Storage and recovery status.

### Review Workspace

Purpose:

- Edit transcript, speaker labels, timeline, and layers.

Required UI:

- Transcript with timestamps.
- Speaker rail.
- Timeline/layer strip.
- Summary side panel.
- Export controls.

### Model Settings

Purpose:

- BYOM configuration without exposing the user to unnecessary complexity.

Required UI:

- Provider list.
- Capability check.
- Test model button.
- Local/cloud privacy state.

## Interaction Rules

- Icon buttons use recognizable icons with tooltips.
- Binary settings use toggles.
- Numeric settings use sliders or stepper inputs.
- Mode switches use segmented controls.
- Export uses clear format choices.
- Destructive actions require confirmation.
- Signal cards are panel-owned controls, not floating FABs.
- Any sector that looks detached must be checked for layout ownership before changing shadow, blur, or color.

## Accessibility

- Text must not overlap or truncate critical meaning.
- Minimum contrast must support long reading sessions.
- Keyboard navigation required for transcript editing.
- Audio-only states must have visible status.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Updated visual language to skeuomorphic tactile porcelain and added RCA-driven rule that Signal cards must be panel-owned. |
| 0.1.0b | Initial visual and interaction direction for quiet local-first desktop audio app. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.0b | 2026-07-05 | beta | Added skeuomorphic material direction and Signal ownership rule. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Initial design system. | N/A | ATHER |
