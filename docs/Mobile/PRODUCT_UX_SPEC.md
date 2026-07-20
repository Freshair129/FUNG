---
version: "0.1.0b"
created_at: "2026-07-20T03:09:52+07:00,ATHER"
last_update: "2026-07-20T03:24:45+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "product-spec"
  scope: "FUNG Mobile"
  language: "Thai-first"
---

# FUNG Mobile Product and UX Specification

## Approval Status

This document is the approved beta product and UX contract. It defines the mobile product boundary for visual concept generation and implementation planning.

## Complexity and Risk

| Item | Classification | Reason |
| --- | --- | --- |
| Complexity | C-3 Architecture-Driven | Mobile becomes a standalone runtime with audio durability, local graph data, MCP, and optional Desktop compute. |
| Change risk | HIGH | Recording durability, mobile OS lifecycle, privacy, network pairing, and cross-device provenance affect multiple system boundaries. |

## Source Alignment

Parent documents reviewed:

- `AIOS/CORE/SHARED_CONTEXT.md`
- `docs/Desktop/PRODUCT_SPEC.md`
- `docs/Desktop/ARCHITECTURE.md`
- `docs/Desktop/LEGAL_PRIVACY.md`

Peer documents and contracts reviewed:

- `docs/Desktop/DESIGN_SYSTEM.md`
- `docs/Desktop/AUDIO_AI_PIPELINE.md`
- `docs/Desktop/IMPLEMENTATION_SURFACES.md`
- `contracts/local-api-v1.yaml`
- `contracts/local-mcp-v1.yaml`
- `contracts/stateful-job-model-v1.yaml`
- `contracts/genesisblockdb-entities-v1.yaml`

## Product Decision

FUNG Mobile is a standalone local-first audio and note intelligence product. It must remain useful when no Desktop, Internet, cloud provider, or external model endpoint is available.

FUNG Desktop is an optional LAN-connected compute companion. It may provide heavier features such as local LLM processing, speaker diarization, audio cleanup, source separation, and large export jobs. Mobile must never present Desktop availability as a requirement for core capture, notes, playback, retrieval, or graph navigation.

## Product Principles

1. Voice is the primary command surface; touch is always available as an equivalent and recovery path.
2. Local operation is the normal state, not an offline error state.
3. Recording durability and recovery outrank decorative interaction.
4. Every AI-derived result exposes its runtime location, provenance, evidence, and uncertainty.
5. GenesisBlockDB relations help users retrieve context; the graph must not fabricate relationships silently.
6. Pairing with Desktop is explicit, reversible, and limited to the local network by default.
7. Destructive or privacy-sensitive voice commands require visible confirmation.

## Standalone Core Features

The following features must work entirely on Mobile:

- Create, rename, archive, search, and reopen projects.
- Record microphone audio into durable chunks.
- Pause, resume, stop, and recover interrupted recording sessions.
- Add a timestamped voice note or typed note while recording or reviewing.
- Play, seek, and mark audio locally.
- Run on-device command recognition for the bounded voice-command grammar.
- Produce on-device timestamped transcription when a supported local speech model is installed; otherwise retain playable audio and queued transcription without data loss.
- Edit transcript text and speaker labels.
- Store notes, transcripts, recordings, evidence spans, and relations locally.
- Build and browse a GenesisBlockDB-backed relation graph for notes and evidence.
- Export original audio, note text, and structured JSON locally.
- Expose bounded MCP tools while the app is foregrounded and the user has enabled local-network access.

Full LLM-based summary or intent inference is not a standalone-core dependency. When unavailable, Mobile must show the original evidence and a clear optional processing action rather than a fake or placeholder result.

## Optional Desktop-Enhanced Features

When paired over the local network, Mobile may delegate:

- Large or higher-accuracy transcription jobs.
- Speaker diarization.
- Noise reduction and speech enhancement.
- Source separation.
- Local LLM summary, action extraction, and intent inference.
- Large graph enrichment jobs.
- Batch export or archival work.

Delegated work must return a `job_id`, remain queryable, show its execution device, and retain the last known progress if the connection drops. The original Mobile audio and notes remain available locally during processing.

## MCP Boundary

Mobile supports both MCP roles:

| Role | Behavior |
| --- | --- |
| MCP client | Connect to an approved FUNG Desktop MCP endpoint on the local network and invoke bounded tools. |
| Bounded MCP server | Expose user-approved Mobile projects, notes, recordings, search, and jobs while the app is foregrounded or within OS-permitted background execution. |

MCP rules:

- Disabled by default.
- Local-network scope by default.
- Pairing and capability consent are required.
- Tools wrap the same domain and job invariants as the app UI.
- MCP must not bypass GenesisBlockDB provenance, audit events, recording locks, or destructive confirmations.
- The UI must not imply that Mobile can host an always-on server when the operating system has suspended the app.

## GenesisBlockDB Note Graph

### Initial Node Types

- Project
- Recording
- AudioChunk
- Note
- TranscriptSegment
- Speaker
- Topic
- Decision
- ActionItem
- EvidenceSpan
- ModelRun
- Device

### Initial Relation Types

- `BELONGS_TO`
- `RECORDED_DURING`
- `MENTIONS`
- `RELATED_TO`
- `SUPPORTS`
- `CONTRADICTS`
- `SPOKEN_BY`
- `DERIVED_FROM`
- `CREATED_ON`
- `PROCESSED_BY`

### Relation Rules

- User-created relations are stored as explicit facts.
- AI-suggested relations remain proposals until accepted or are visibly labelled as inferred.
- Every inferred relation stores its source evidence, model run, confidence, and runtime location.
- Graph deletion must not silently delete source audio, transcript, or note content.
- Cross-device merges preserve stable identities and append audit events; overwrite-by-last-write is not an acceptable default for graph truth.

## Voice-First Interaction Model

### Default Activation

Push-to-talk is the default. Always-listening or wake-word behavior is out of scope for the first release because it materially changes battery, background execution, consent, and privacy expectations.

Touching the primary voice control and using the equivalent visible button must produce the same command outcome.

### Voice State Machine

```text
idle
  -> listening
  -> interpreting
  -> confirmation_required | executing
  -> success | failed | needs_clarification
  -> idle
```

Required visible states:

| State | Visual feedback | Voice/audio feedback |
| --- | --- | --- |
| Idle | Quiet microphone control and current local status | None |
| Listening | Live waveform and elapsed listening time | Soft start cue, optional haptic |
| Interpreting | Stable transcript preview and progress | No repeated speech |
| Needs clarification | One concise question with tappable choices | Question may be spoken once |
| Confirmation required | Exact action, target, and consequence | Spoken confirmation prompt |
| Executing | Job state, device, and progress | Short acknowledgement |
| Success | Result and undo/open action where safe | Optional completion cue |
| Failed | Cause, data-safety state, and recovery action | Short failure cue |

### Initial Voice Command Families

- “เริ่มบันทึก [ชื่อโปรเจกต์]”
- “หยุดชั่วคราว” / “บันทึกต่อ” / “หยุดและบันทึก”
- “ทำโน้ตว่า …”
- “ทำเครื่องหมายตรงนี้”
- “เปิดโน้ตล่าสุด”
- “ค้นหาเรื่อง …”
- “เชื่อมต่อ FUNG Desktop”
- “ส่งงานนี้ไปถอดเสียงที่ Desktop”
- “สรุปการประชุมนี้ด้วย local LLM”
- “แสดงความสัมพันธ์ของโน้ตนี้”
- “ยกเลิกงานล่าสุด”

Ambiguous commands must show the interpreted action before execution. Stop, delete, replace, share, external-provider, and permission-changing actions require confirmation.

## Information Architecture

### Primary Command Dock

The bottom command dock contains five stable destinations/actions:

1. Home
2. Notes
3. Voice / Capture — central primary action
4. Graph
5. Devices

The central Voice / Capture control is visually dominant but belongs to the navigation dock; it must not float without structural ownership.

### Screen Inventory

| ID | Screen | Primary job | Required states |
| --- | --- | --- | --- |
| M1 | Voice Home | Start a command, recording, note, or resume recent work | local-ready, listening, interpreting, disconnected, active job |
| M2 | Live Capture | Trust long-running recording and add timestamped notes | recording, paused, saving chunk, low storage, interrupted, recovered |
| M3 | Notes Library | Find projects and notes quickly | populated, empty, search, filter, processing |
| M4 | Note Detail | Read evidence, edit text, play source, and follow relations | source-only, transcript-ready, edited, inferred relation proposal |
| M5 | Graph Explorer | Explore note, speaker, topic, decision, and evidence relations | overview, focused node, relation detail, no relations |
| M6 | Devices and Runtime | Pair Desktop and choose execution location | unpaired, pairing, connected, degraded, permission denied |
| M7 | Job Detail | Track Mobile or Desktop work with provenance | queued, running, paused, failed, retrying, completed |
| M8 | MCP and Privacy | Enable bounded tools and inspect access | disabled, enabled, active session, suspended by OS, revoked |

### First-Run Flow

1. Explain local-first storage in one screen.
2. Request microphone permission only when the user starts capture or voice command.
3. Create the first note or recording without requiring an account or Desktop.
4. Offer Desktop pairing after the first successful local result, not as an onboarding blocker.
5. Offer MCP separately with a capability-level consent screen.

## Core User Flows

### Flow A — Standalone Voice Note

1. User holds the central voice control.
2. User says “ทำโน้ตว่า นัดตรวจต้นแบบวันศุกร์”.
3. Mobile shows the recognized text and saves it locally.
4. GenesisBlockDB creates the Note node and provenance.
5. Related-node suggestions appear only when evidence exists and remain visibly inferred until accepted.

### Flow B — Long Recording

1. User says “เริ่มบันทึก ประชุมทีม”.
2. Consent reminder appears before first capture in a new context.
3. Mobile writes durable audio chunks and displays last-safe-write time.
4. User adds voice or touch markers without interrupting capture.
5. On stop, Mobile finalizes local playback immediately and queues optional transcription.

### Flow C — Desktop-Enhanced Processing

1. Mobile detects a previously paired Desktop on LAN.
2. User chooses or says “สรุปด้วย local LLM ที่ Desktop”.
3. UI shows exact data, destination device, runtime location, and action.
4. Desktop returns a stateful `job_id`.
5. Mobile continues to show local source data if the connection drops.
6. Returned result includes evidence references, model provenance, and inference labels.

### Flow D — MCP Access

1. User opens MCP and Privacy.
2. User selects projects and tool capabilities to expose.
3. Mobile shows session identity, network scope, and foreground/background availability.
4. Each mutation appends an audit event.
5. User can revoke the session immediately.

## Visual Direction

### Design Character

- Apple Japan: disciplined hierarchy, quiet typography, generous negative space.
- Quiet Luxury: careful materials and restrained premium detail rather than decoration.
- Tactile porcelain: shallow molded surfaces, precise pressed states, and quiet mechanical confidence.
- Mobile adaptation: fewer bevels, lighter shadows, larger touch targets, and no desktop-HUD imitation.

### Palette

| Token | Direction | Use |
| --- | --- | --- |
| Porcelain | warm near-white | app background |
| Warm white | clean surface white | primary sheets and reading areas |
| Ink | near-black | primary text |
| Graphite | neutral gray | secondary text and icons |
| Muted sage | calm green | local-safe and completed states |
| Deep indigo | restrained blue-indigo | focus, selection, graph traversal |
| Warm metal | muted champagne metal | small premium detail only |
| Signal red | controlled red | destructive, recording, or critical error only |

No dominant purple gradient, decorative glow, generic bento grid, or card-inside-card composition.

### Typography

- Thai-first UI using a system Thai-compatible sans fallback.
- Content and transcript text prioritize long-session reading comfort.
- Control labels use deliberate sizes and must not inherit browser defaults.
- No viewport-scaled type.
- Minimum body size: 16 px equivalent.

### Shape and Material

- Minimum touch target: 44 x 44 pt/dp.
- Primary voice control: tactile circular or softly squared control with stable pressed state.
- Standard surface radius: 10–14 pt; use larger radius only for bottom sheets or the command dock.
- Use open reading surfaces, rails, and sheets; avoid wrapping every region in a card.
- Recording and listening surfaces may use inset waveform wells, but the waveform remains readable and code-native.
- Respect platform safe areas and dynamic text sizing.

### Motion and Haptics

- Listening uses restrained waveform motion and one soft haptic at activation.
- Chunk-save state uses a quiet pulse without distracting from elapsed time.
- Graph focus transitions preserve spatial orientation.
- Processing motion stops or simplifies under reduced-motion settings.
- Haptics never replace visible state.

## Content Rules

- Thai is the default concept language; English technical terms may remain when clearer, such as MCP and local LLM.
- Status language states where work runs: “บนมือถือ”, “FUNG Desktop”, or “Cloud provider”.
- Never label inferred relations, summary, intent, or speaker identity as verified fact.
- Errors state whether the recording and last durable chunk are safe.
- Empty states lead to a real action, not marketing copy.

## Privacy and Safety UX

- Microphone activity is always visually apparent.
- Recording starts only after user action and applicable consent reminder.
- Pairing uses explicit device identity and short-lived verification.
- External or cloud processing is opt-in and visually distinct from Mobile/Desktop local processing.
- MCP capability consent is granular by project and tool family.
- Voice transcripts used only for command interpretation are minimized according to the approved retention policy.
- Low-storage, interrupted-write, and recovery states must explain the last known safe audio boundary.

## Responsive and Platform Boundary

The first visual concept targets a neutral modern mobile viewport at approximately `390 x 844` portrait. The design must also adapt to:

- Compact phones around 360 px wide.
- Large phones around 430 px wide.
- Dynamic text and Thai line wrapping.
- Light and dark appearance.
- iOS and Android safe areas.

Tablet and landscape editor layouts are out of scope for the first concept pass.

## Acceptance Criteria

- Mobile can create and recover a local recording without Desktop or Internet.
- Mobile can create and retrieve voice notes without Desktop or Internet.
- Every recording state exposes the last durable-write state.
- Voice commands have equivalent touch paths.
- Destructive and external-processing commands require confirmation.
- Desktop-enhanced work exposes device, runtime location, job progress, and provenance.
- MCP is disabled by default and uses explicit capability consent.
- The graph distinguishes user-created facts from AI-inferred relation proposals.
- All AI-derived results retain evidence references and inference labels.
- The complete visual concept covers M1–M8 or a clearly coordinated subset plus all required dense states.
- No concept is accepted if it is only a home screen or a static marketing surface.

## Success Criteria

- A new user can create the first local voice note without account creation or Desktop pairing.
- A user can start a recording from Home in no more than two deliberate actions after permissions are granted.
- A user can identify whether an active job runs on Mobile or Desktop without opening Settings.
- A user can reach source evidence from any AI-derived summary or graph proposal.
- A disconnected Desktop does not block access to local projects, audio, notes, or graph data.
- Voice, touch, accessibility, and privacy states remain understandable without relying on color or sound alone.

## Definition of Done for UX/UI Concept Phase

- This candidate is approved by the user.
- Image Gen concepts cover the complete primary Mobile surface and required states.
- A design system is extracted from the accepted concepts: tokens, typography, components, states, icon treatment, spacing, and container rules.
- Thai visible copy is reviewed for clarity and line wrapping.
- Voice-state, recording-state, Desktop connection, MCP consent, and graph evidence flows are all represented.
- Accessibility and privacy review pass.
- Parent and peer documentation review finds no unresolved product contradiction.
- Implementation does not begin until the concept is explicitly approved.

## Exit Criteria Before Implementation

- Mobile platform stack and minimum OS targets are approved.
- On-device speech model boundary and download/storage behavior are approved.
- Mobile MCP foreground/background behavior is technically validated against target OS constraints.
- GenesisBlockDB Mobile persistence and cross-device merge contracts are approved.
- Pairing, authentication, capability consent, and revocation are specified.
- Accepted concepts have no unresolved layout, copy, state, or interaction ambiguity.

## Out of Scope for First Concept Pass

- Always-listening wake word.
- Cloud account or mandatory cloud sync.
- Tablet-first editing workspace.
- Full DAW timeline and mixing controls.
- Automatic biometric speaker identity.
- Unattended always-on MCP hosting beyond mobile OS lifecycle limits.

## Proposed Visual Concept Set After Approval

1. Voice Home — idle, listening, and interpreted-command states.
2. Live Capture — recording, chunk-safe, timestamped note, and low-storage states.
3. Notes Library and Note Detail.
4. Graph Explorer and inferred-relation review.
5. Desktop pairing and delegated local-LLM job.
6. MCP capability consent and active-session state.
7. Dark appearance key screens after the light system is accepted.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Approved beta contract for standalone, voice-first FUNG Mobile with optional LAN Desktop compute, MCP, and GenesisBlockDB note relations. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-20 | beta | Approved Mobile product and UX contract for concept generation. | N/A | ATHER |
