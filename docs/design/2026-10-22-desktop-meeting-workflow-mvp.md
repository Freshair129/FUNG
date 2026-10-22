---
version: "0.1.0b"
created_at: "2026-10-22"
last_update: "2026-10-22"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "specification"
  scope: "FUNG Desktop capture, review and support surfaces"
  language: "Thai-first"
---

# FUNG Desktop meeting workflow MVP — design and behavior amendment

## Source and authority

This implementation follows the approved Home → Live capture → Archive review
journey in the [current Desktop flow contract](2026-10-16-desktop-navigation-and-flows.md).
The [mockup evidence set](mockups/2026-10-22-desktop-mvp/README.md) is a visual
reference. Existing semantic tokens remain owned by the
[Quiet Archive Design System](../FUNG_Design_System_v0.1.0/README.md).
This document amends the former application-audio exclusion in the
[capture routing contract](2026-09-20-live-capture-device-routing.md) for the
bounded implementation below.

## User and outcome

Desktop user starts a meeting capture, chooses a microphone and either machine
audio or one currently active audio-producing application, sees independent
measured levels while recording, then reviews the exact recording and its
persisted source channels.

## Scope

- Keep the persistent task navigation at Home, Live, and Archive. Files/import
  remain support actions; Companion remains a utility panel; Settings remains a
  right-side drawer.
- Preserve current project, recording, transcript, playback and summary owners.
- In Live setup, offer `เสียงระบบ` or `เลือกแอป` as mutually exclusive sources
  for the second recording channel. Microphone remains its own required source.
- For app capture on Windows, enumerate applications with an active render audio
  session, capture the chosen process tree using Windows process loopback, and
  revalidate the selected PID/session at start. If the process is stale, the OS
  lacks support, or capture cannot start, fail closed and explain why.
  Windows process-loopback activation is supported from Windows 10 build 20348;
  see Microsoft's [Application loopback audio capture sample](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/).
- Show live measured waveform envelopes as separate microphone and remote-source
  lanes. Native code reduces each PCM sample batch to 24 bounded peak bins and an
  RMS percentage. Raw audio bytes never enter the visual event; each event is
  scoped to `recordingId` and a durable source channel.
- Reuse the existing durable `mic` and `system` transcript/playback channel IDs
  for compatibility. Record source mode and application identity in capture
  provenance so `system` can be distinguished as render-loopback or app-loopback.
- Keep settings categories in a visible horizontal tab grid with keyboard
  support. The settings content reader may scroll internally; the tab strip and
  main shell do not gain another scroll container.
- Keep sample meeting dates, names, task counts, and generated waveform bars in
  the mockups only. Production values must come from current application data.

## Non-goals

- Multiple microphones or multiple application sources in one meeting.
- Cross-platform application loopback outside supported Windows builds.
- New database channel IDs, audio mixing, diarization claims, or transcript
  quality changes.
- New primary navigation pages for Files or Companion.
- Calendar integration; an unavailable calendar keeps its truthful empty state.
- General repository-wide removal of valid constants, product copy, or schema
  defaults. The audit target is mock data, inert controls, and unreachable code
  in affected Desktop surfaces.

## Acceptance criteria

1. The source control selects system loopback or one active app session; an
   unavailable/stale choice never silently falls back to another source.
2. Native audio capture persists each selected source under its actual capture
   provenance, and existing mic/system playback and transcript behavior remains
   compatible.
3. The Live surface renders two labeled 24-bin waveform envelopes from native
   measured PCM, scopes events to the active `recordingId`, resets on new
   sessions, and shows an explicit waiting/error state when no current sample
   exists.
4. The settings drawer presents every category without a scrolling tab rail;
   Left/Right/Home/End keyboard handling, focus visibility, Escape and close
   focus restoration continue to work.
5. Home, Review, Files support actions and Companion use live application state;
   no generated sample meeting/person/task data ships in runtime components.
6. The saved PNGs have SHA-256 evidence. The design QA report compares the
   browser preview to the approved references and distinguishes source/build
   checks from native/device acceptance.
7. Run focused UI/native tests, `npm run build`, Rust formatting and the targeted
   Rust library suite. Record exact commands, outcomes, environment and any
   remaining app-loopback device gate in Desktop progress.
