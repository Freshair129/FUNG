---
version: "0.1.0b"
created_at: "2026-07-20T10:18:02+07:00,ATHER"
last_update: "2026-07-20T10:57:33+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "design-system"
  scope: "FUNG Mobile dark appearance and critical states"
  language: "Thai-first"
---

# FUNG Mobile Dark Appearance and Critical-State Review

## Review Status

This approved package extends the light-theme contract with four dark-appearance concepts and four critical-state concepts. It is the visual-state baseline for Mobile technical design. Code implementation remains blocked by the technical-design approval gate.

## Source Contract

- `docs/Mobile/PRODUCT_UX_SPEC.md` version `0.1.0b`, status `beta`.
- `docs/Mobile/CONCEPT_REVIEW.md` version `0.1.0b`, status `beta`.
- Built-in Image Gen was used in `precise-object-edit` mode to preserve approved layout and information architecture.
- All canonical outputs are `852–853 x 1844–1846` pixels and match the accepted mobile portrait ratio.

## Dark-Appearance Inventory

| ID | Surface | Canonical concept | Purpose |
| --- | --- | --- | --- |
| D1 | Voice Home | `concepts/dark/01-voice-home-dark.png` | Validate primary voice hierarchy and command-dock ownership in dark appearance. |
| D2 | Live Capture | `concepts/dark/02-live-capture-dark.png` | Validate timer, waveform, durable-chunk trust signal, and recording red in darkness. |
| D3 | Note Detail | `concepts/dark/03-note-detail-dark.png` | Validate long-reading contrast, evidence, transcript, and inference distinction. |
| D4 | Graph Explorer | `concepts/dark/04-graph-explorer-dark.png` | Validate confirmed/inferred edge semantics and graph spatial orientation. |

## Critical-State Inventory

| ID | State | Canonical concept | Required user truth |
| --- | --- | --- | --- |
| S1 | Capture low storage | `concepts/states/01-capture-low-storage.png` | Recording continues, saved chunks are safe, about 12 minutes remain, automatic stop is expected. |
| S2 | Recording recovery | `concepts/states/02-recording-recovery.png` | Forty-two chunks recovered, audio is safe through `01:23:58`, final 20 seconds remain uncertain. |
| S3 | Desktop job disconnected | `concepts/states/03-desktop-job-disconnected.png` | Mobile remains ready, job pauses at 68%, source remains on Mobile, resume occurs after reconnect. |
| S4 | MCP suspended | `concepts/states/04-mcp-suspended.png` | OS suspended MCP in background, no tool currently accesses data, previous consent remains visible but inactive. |

## Dark-Appearance Design Extension

### Candidate Tokens

Exact values must be sampled and contrast-tested from the accepted concepts before implementation.

| Token | Candidate | Role |
| --- | --- | --- |
| `dark.canvas` | `#171918` | Charcoal porcelain background. |
| `dark.surface` | `#202321` | Primary molded surface. |
| `dark.surfaceRaised` | `#272A27` | Controls and anchored sheets. |
| `dark.textPrimary` | `#F2EFE8` | Primary text and key values. |
| `dark.textSecondary` | `#A9AAA5` | Secondary text. |
| `dark.lineQuiet` | `#40423F` | Dividers and quiet boundaries. |
| `dark.indigoFocus` | `#6679AD` | Selected controls and focused graph node. |
| `dark.sageLocal` | `#8FA88F` | Local-safe and confirmed states. |
| `dark.metalInferred` | `#C39A4A` | Inferred and attention states. |
| `dark.signalRecording` | `#E34338` | Active recording and stop only. |

### Dark Rules

- Dark appearance uses charcoal porcelain, not pure black.
- Surfaces remain distinguishable through tone, outline, and shallow bevel direction.
- Primary content uses warm off-white rather than cool blue-white.
- Recording red appears only in active recording and stop controls.
- Muted sage communicates local-safe or user-confirmed state.
- Warm metal communicates inference, uncertainty, pause, or non-destructive attention.
- Selected state must remain visible without depending only on color.
- Final implementation must pass measured contrast checks; generated imagery is not contrast evidence by itself.

## Critical-State Contracts

### S1 — Capture Low Storage

State conditions:

- Recording remains active.
- Last durable chunk write succeeded.
- Remaining capacity is approximately 12 minutes.
- App will stop safely when capacity is exhausted.

Required actions:

- `หยุดและบันทึก`
- `จัดการพื้นที่`
- `บันทึกต่อ`

The UI must not claim that already-written audio is at risk.

### S2 — Recording Recovery

State conditions:

- Recording is no longer active.
- Recovery scan found 42 durable chunks.
- Confirmed-safe audio ends at `01:23:58`.
- The final approximate 20 seconds are uncertain and must not be described as recovered fact.

Required actions:

- `ตรวจสอบเสียง`
- `บันทึกต่อ`
- `จบเซสชัน`

The timeline distinguishes confirmed-safe and uncertain ranges through label, pattern, boundary, and color.

### S3 — Desktop Job Disconnected

State conditions:

- Mobile remains locally ready.
- Desktop is unavailable on LAN.
- Delegated summary job retains last-known progress at 68% and does not restart silently.
- Source audio and notes remain on Mobile.

Required actions:

- `เชื่อมต่อใหม่`
- `ดูข้อมูลต้นฉบับ`
- `ยกเลิกงาน`

Paused attention uses warm metal. Red remains reserved for cancelling the job.

### S4 — MCP Suspended

State conditions:

- Mobile OS suspended MCP after the app moved to background.
- No MCP client is actively accessing data.
- Previously approved project and capabilities remain visible but inactive.
- Resuming requires explicit user action.

Required actions:

- `เปิด MCP อีกครั้ง`
- `ดูบันทึกกิจกรรม`
- `เพิกถอนทุกสิทธิ์`

The UI must not imply that Mobile MCP can remain continuously reachable outside OS lifecycle limits.

## Final Image Gen Prompt Set

### Dark Transformation Prompt

Convert the approved light screen to a refined dark tactile-porcelain appearance while preserving exact layout, hierarchy, copy, controls, icon metaphors, spacing, component ownership, and viewport. Use charcoal and graphite porcelain surfaces, warm off-white text, muted sage local-safe state, softened deep indigo focus, restrained warm metal inference/attention, and signal red only for recording or destructive actions. Maintain accessible hierarchy without pure-black voids, neon, purple gradients, glassmorphism, or new UI.

### Critical-State Prompt Pattern

Transform the approved light screen into one explicit operational state while preserving its product anatomy. State what happened, what data is safe, what remains uncertain or paused, where execution runs, and which recovery actions are available. Use icon, text, structure, and color together. Do not hide the core context behind a generic modal, invent cloud fallback, claim data loss without evidence, or restart work silently.

## Fidelity Ledger

| Comparison point | Accepted contract | New concept evidence | Result |
| --- | --- | --- | --- |
| Layout preservation | Dark mode must not change information architecture. | D1–D4 retain light-theme screen structure and navigation ownership. | Pass |
| Dark semantic color | Red is recording/destructive; sage is local-safe; metal is inference/attention. | D1–D4 preserve semantic roles. | Pass |
| Reading evidence | Note Detail must keep evidence and inference visibly distinct. | D3 retains timestamp evidence, transcript, inference label, and confirmation actions. | Pass |
| Graph semantics | Confirmed and inferred relations need more than color. | D4 preserves solid confirmed edges, dotted inferred edge, icons, and labels. | Pass |
| Low-storage truth | User needs remaining time and safe-write state. | S1 shows safe saved data, 12-minute estimate, automatic stop, and recovery choices. | Pass |
| Recovery boundary | Recovery cannot overclaim uncertain tail. | S2 labels confirmed safe boundary and uncertain final 20 seconds. | Pass |
| Disconnected job | Job progress and local source access remain visible. | S3 retains 68%, Desktop execution location, local source safety, and reconnect action. | Pass |
| MCP lifecycle | Background suspension must be honest. | S4 states OS suspension, no current access, inactive capabilities, audit, and explicit resume. | Pass |
| Paused-job color | Non-destructive pause should not use error red. | First output used red; canonical S3 was edited to warm metal while cancel remains red. | Fixed |

## Known Boundaries

- Dark concepts require measured WCAG contrast verification in code; visual inspection alone is insufficient.
- Dynamic type, Thai line wrapping, and platform-specific accessibility settings remain implementation tests.
- Native OS low-storage and background-execution notifications are platform-owned and not replaced by these in-app surfaces.
- Microphone-denied, model-download failure, database corruption, and pairing-authentication failure are not included in this bounded state pass.

## Acceptance Criteria

- D1–D4 preserve the accepted light-theme information architecture.
- S1–S4 make data safety, uncertainty, execution location, and recovery action explicit.
- No state depends only on color or sound.
- No screen claims cloud fallback, full recovery, or always-on Mobile MCP without evidence.
- Canonical files exist in the workspace and are readable at native portrait ratio.

## Success Criteria

- A reviewer can identify what happened and the safest next action within the first viewport.
- Dark appearance preserves FUNG's tactile porcelain identity without becoming neon, glassy, or visually flat.
- Critical states reduce ambiguity without obscuring the user's recording, source data, job progress, or consent boundary.

## Definition of Done

- User approved this package on 2026-07-20.
- Candidate dark tokens are sampled and contrast-tested during implementation.
- Exact Thai copy is moved into code-native strings.
- Critical-state transitions are added to the implementation plan and test plan.
- No code begins before this approval gate closes.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Added four dark-appearance concepts and four critical-state concepts with state contracts, prompt set, and fidelity ledger. |
| 0.1.0b approval update | Promoted review status from `candidate` to `beta` after user approval. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-20 | candidate | Added dark appearance and critical-state review package. | N/A | ATHER |
| 0.1.0b | 2026-07-20 | beta | User approved the visual-state package for technical-design input. | N/A | ATHER |
