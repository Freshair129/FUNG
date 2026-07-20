---
version: "0.1.0b"
created_at: "2026-07-20T03:40:11+07:00,ATHER"
last_update: "2026-07-20T04:08:39+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-system"
  scope: "FUNG Mobile visual concepts"
  language: "Thai-first"
---

# FUNG Mobile Visual Concept Review

## Review Status

This light-theme concept set is approved as the beta visual contract. Code implementation remains blocked until the required dark-theme and critical-state concepts complete their review gate.

## Source Contract

- `docs/Mobile/PRODUCT_UX_SPEC.md` version `0.1.0b`, status `beta`.
- Existing FUNG Desktop screenshot used only as visual-DNA reference: `output/playwright/fung-skeuo-1280x720.png`.
- Image generation mode: built-in Image Gen.
- Target aspect: neutral modern mobile portrait, approximately `390 x 844`.
- Generated artifacts: `852–853 x 1844–1846` pixels, matching the target portrait ratio.

## Concept Inventory

| ID | Surface | Canonical concept | Contract coverage |
| --- | --- | --- | --- |
| C1 | Voice Home | `concepts/01-voice-home.png` | Standalone local-ready state, voice-first primary action, recent work. |
| C2 | Voice Listening and Confirmation | `concepts/02-voice-listening-confirmation.png` | Listening, local interpretation, visible command confirmation. |
| C3 | Live Capture | `concepts/03-live-capture.png` | Timer, waveform, durable chunk status, storage, marker, pause, stop. |
| C4 | Notes Library | `concepts/04-notes-library.png` | Search, source types, Mobile/Desktop runtime labels, open list model. |
| C5 | Note Detail and Evidence | `concepts/05-note-detail-evidence-v2.png` | Playback, transcript evidence, inferred relation proposal, confirmation. |
| C6 | Graph Explorer | `concepts/06-graph-explorer.png` | Calm relation graph, confirmed/inferred distinction, evidence sheet. |
| C7 | Devices and Runtime Job | `concepts/07-devices-runtime-job.png` | Standalone Mobile, optional LAN Desktop, local LLM job provenance. |
| C8 | MCP and Privacy | `concepts/08-mcp-privacy.png` | Disabled-by-default MCP, capability consent, LAN scope, lifecycle, revocation. |

`concepts/05-note-detail-evidence.png` is retained as the first-pass comparison. It is not canonical because it selected Home instead of Notes in the bottom dock.

## Final Image Gen Prompt Set

All prompts used the `ui-mockup` taxonomy except the C5 selected-state correction, which used `precise-object-edit`.

### Shared Visual Prompt

Create a complete edge-to-edge FUNG Mobile screen at a neutral `390 x 844` portrait viewport. Inherit the existing FUNG Desktop visual DNA without copying its dense HUD layout. Use Apple Japan, Quiet Luxury, and tactile porcelain adapted for touch: warm near-white porcelain, warm-white reading surfaces, ink and graphite typography, muted sage for local-safe states, deep indigo for focus, warm metal only for inferred relations, and signal red only for recording or destructive states. Use Thai-compatible modern sans typography, generous whitespace, shallow bevels, minimal shadows, safe areas, and touch targets of at least 44 pt/dp. Keep app text and controls code-native for later implementation. Do not use a phone frame, marketing hero, bento grid, nested cards, dominant purple gradients, glassmorphism, neon, fake metrics, cloud-first language, or unrelated features.

### Screen-Specific Prompt Briefs

| Concept | Required focus |
| --- | --- |
| C1 | Idle Voice Home with structurally-owned central microphone, local-ready status, record/note actions, recent work, and four navigation destinations. |
| C2 | Listening waveform, recognized Thai command, local interpretation state, and anchored confirmation sheet with confirm/edit/cancel. |
| C3 | Long-running recording with large timer, live waveform, last durable chunk, remaining storage, timestamp marker, pause, voice note, and stop. |
| C4 | Searchable Notes Library using open rows, source icons, timestamps, local/Desktop provenance, and selected Notes navigation. |
| C5 | Evidence-first Note Detail with audio scrubber, note, transcript excerpt, evidence range, AI-inferred relation proposal, and user confirmation. |
| C6 | Spatial note graph with one focused node, four nearby nodes, confirmed sage relations, inferred warm-metal relation, and evidence detail sheet. |
| C7 | Mobile standalone capability, optional LAN Desktop capability, Local LLM availability, delegated job progress, and local source-data safety. |
| C8 | MCP off by default, per-project capability toggles, LAN-only scope, foreground/background lifecycle truth, audit activity, and revocation. |

## Extracted Design System

### Color Tokens

These values are implementation candidates and must be sampled again from the accepted image set before coding.

| Token | Candidate | Role |
| --- | --- | --- |
| `porcelain.canvas` | `#F8F5F0` | Main app background. |
| `porcelain.surface` | `#FCFAF7` | Reading and control surfaces. |
| `ink.primary` | `#202126` | Primary text. |
| `graphite.secondary` | `#6F706F` | Secondary text and inactive icons. |
| `indigo.focus` | `#394B78` | Selected state and primary action. |
| `sage.local` | `#708D7E` | Local-safe, connected, confirmed. |
| `metal.inferred` | `#B68A3C` | AI-inferred relation pending review. |
| `signal.recording` | `#C93631` | Active recording and stop. |
| `line.quiet` | `#E3DED6` | Dividers and quiet outlines. |

### Typography

- Brand/UI heading: system sans with a quiet geometric character.
- Thai content: `Noto Sans Thai`, `Leelawadee UI`, or platform system Thai fallback.
- Body minimum: 16 px equivalent.
- Controls: deliberate 15–17 px equivalent, not inherited defaults.
- Timer: tabular numerals.
- Letter spacing: zero for Thai content.

### Spacing and Geometry

- Spacing scale: `4, 8, 12, 16, 20, 24, 32, 40, 48`.
- Compact control radius: `10–14`.
- Sheet and dock radius: `20–28`.
- Circular voice and record controls use a stable pressed state without layout shift.
- The bottom command dock owns the central voice control.
- Active Capture replaces global navigation with a focused recording dock.

### Icon System

- Rounded outline icons with approximately 2 px optical stroke at base size.
- Selected destinations may use filled deep-indigo icons.
- Recording uses filled signal red only where state is active or destructive.
- Confirmation, local-safe, and verified relations use muted sage.
- Chevrons and disclosure controls remain SVG or icon components, never text glyph approximations.

### Component Families

- `MobileAppShell`
- `LocalStatus`
- `VoiceDial`
- `CommandDock`
- `CaptureDock`
- `WaveformWell`
- `SegmentedFilter`
- `SearchField`
- `NoteRow`
- `AudioScrubber`
- `EvidenceLink`
- `RelationProposal`
- `GraphCanvas`
- `RelationDetailSheet`
- `DeviceSection`
- `RuntimeCapabilityList`
- `JobProgressPanel`
- `PermissionGroup`
- `PermissionSwitchRow`
- `DestructiveAction`

## Container Rules

- Prefer open reading surfaces and dividers over card stacks.
- One primary sheet or control surface may dominate a screen; secondary information should use rows or rails.
- Do not place cards inside cards.
- Status labels may use a compact contained treatment only when they communicate local/runtime truth.
- The voice control must be owned by Home content or the command dock; it must not become an unrelated floating action button.
- Graph controls belong to the graph canvas; relation details belong to the anchored sheet.

## Interaction Inventory

| Interaction | Required behavior |
| --- | --- |
| Hold voice control | Enter listening state with waveform, timer, visible mic state, and haptic cue. |
| Release voice control | Interpret locally and present confirmation when ambiguous or mutating. |
| Start recording | Enter focused Capture screen and begin durable chunk writes. |
| Add marker/note | Attach the current timestamp without interrupting capture. |
| Open evidence | Seek audio and transcript to the referenced range. |
| Confirm relation | Change inferred proposal to user-confirmed relation and append audit event. |
| Delegate to Desktop | Show exact destination, job ID/progress, and preserve local source access. |
| Enable MCP | Require project and capability consent before activation. |

## Concept Fidelity Ledger

| Comparison point | Product contract evidence | Concept evidence | Result |
| --- | --- | --- | --- |
| Standalone truth | Mobile core must work without Desktop. | C1, C3, C4, and C7 explicitly show Mobile/local state. | Pass |
| Voice-first hierarchy | Voice is primary with touch equivalent. | C1–C3 use the voice control as the main action while retaining touch controls. | Pass |
| Recording durability | Last safe chunk must be visible. | C3 exposes last durable chunk and remaining storage near the primary controls. | Pass |
| Evidence and inference | AI relation must not appear as fact. | C5 and C6 distinguish evidence, inferred proposal, and user-confirmed relation. | Pass |
| Desktop enhancement | Desktop is optional LAN compute. | C7 visually separates Mobile capability from FUNG Desktop and labels execution location. | Pass |
| MCP lifecycle truth | MCP is opt-in and constrained by Mobile OS lifecycle. | C8 shows disabled default, LAN scope, foreground behavior, possible background suspension, audit, and revoke. | Pass |
| Navigation ownership | Central voice control belongs to command dock. | C1, C2, C4–C7 consistently integrate the control into the dock. | Pass |
| Note Detail selection | Note Detail should retain Notes context. | First pass selected Home; C5 v2 changes only the bottom-dock selection to Notes. | Fixed |

## Known Concept Boundaries

- Generated Thai text is a visual reference only. Implementation must use the exact approved code-native copy and validate Thai line wrapping.
- Light appearance is represented. Dark appearance remains gated until this light concept set is approved.
- Error, low-storage, interrupted-write, recovered, disconnected, and failed-job states still require dedicated state concepts before implementation if their behavior cannot be expressed unambiguously from this system.
- Platform-specific iOS and Android permission sheets are not represented; native permission UX must remain platform-owned.

## Approval Decision

Approval of this set makes C1–C8 and the extracted design system the production visual specification for the next documentation and implementation-planning phase. Any requested visual change must be applied to the concepts before code begins.

## Acceptance Criteria

- The user approves or requests changes to the complete C1–C8 set.
- The selected concept files remain readable at native portrait scale.
- Voice, recording, notes, graph, Desktop job, and MCP surfaces share one coherent design language.
- Runtime location, evidence, inference, privacy, and durability states are visible in context.
- No screen is a marketing surface or static screenshot substitute for real UI.

## Success Criteria

- A reviewer can understand the primary voice, recording, evidence, graph, Desktop, and MCP workflows from the concepts without additional product invention.
- The visual system can be decomposed into reusable production components without a monolithic screen implementation.
- No fixable product-boundary or navigation contradiction remains in the canonical set.

## Definition of Done

- User approves the concept set.
- Dark appearance and required error-state concept scope are agreed.
- Exact visual tokens are sampled from accepted concepts.
- Implementation inventory is reviewed against the target Mobile stack.
- No code is written before this approval gate is complete.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Approved the eight-screen FUNG Mobile light-theme concept set, extracted design system, prompt set, and fidelity ledger. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-20 | beta | Approved complete light-theme visual concept review package. | N/A | ATHER |
