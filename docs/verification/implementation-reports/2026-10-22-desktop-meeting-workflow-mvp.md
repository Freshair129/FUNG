---
version: "0.1.0b"
created_at: "2026-10-22"
last_update: "2026-10-22"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui-and-capture"
  doc_type: "implementation-verification"
  scope: "FUNG Desktop meeting workflow UI, audio-source routing, measured levels, and dead-code/mock-data audit"
  language: "Thai-first"
---

# Desktop meeting workflow MVP implementation report

## Outcome

The approved Home → Live → Archive workflow has an implemented source-routing
UI, Windows application-process capture path, recording-scoped PCM level events,
separate measured waveform lanes, and a horizontal Settings category tab grid.
The mockup source images and their SHA-256 values are stored in the repository.
The source tree and production web build pass the checks in this report.

This is an **MVP source candidate**, not accepted native capture. The browser
preview reports `NATIVE_UNAVAILABLE`; no installed-app test of microphone,
system loopback, application process-loopback, saved audio, or restart/review
was run for this change.

## Evidence identity

| Field | Value |
| --- | --- |
| Date | 2026-10-22 |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| Base HEAD | `80d0adb00956ab7e0a191e7bb446e76a590ac329` |
| Candidate state | Uncommitted working tree; no commit, push, PR, merge, tag, or release |
| Design authority | [Desktop workflow amendment](../../design/2026-10-22-desktop-meeting-workflow-mvp.md) |
| Visual evidence | [Mockup set and PNG hashes](../../design/mockups/2026-10-22-desktop-mvp/README.md) |

## Implemented

- Live setup exposes microphone-only, system audio, and one active application
  as explicit source choices. The native start path validates the selected
  application process again and fails closed if it is unavailable or Windows
  does not support process loopback.
- Windows application audio uses WASAPI process-tree loopback and persists
  through the existing `system` source channel with distinct provenance. No
  silent fallback widens an application choice to all system audio.
- Native capture reduces PCM to a bounded RMS value and 24 peak bins. Events
  contain no PCM bytes, carry the recording identity and source channel, and
  feed the two Live waveform lanes. Missing samples render a waiting state.
- Settings categories use a visible horizontal tab grid with keyboard
  navigation. The content area may scroll inside the drawer.
- Home, Archive/Review, Files support actions, and Companion use current app
  state and honest empty/unavailable states.

## Mock-data and dead-code audit

The audit scope was all of `src/` for TypeScript unused declarations and the
affected Desktop runtime surfaces (Home, Live, Archive/Review, Files support,
Companion, and Settings) for sample data, fake UI values, placeholders, and
inert controls.

- `npx tsc --noEmit --noUnusedLocals --noUnusedParameters` passes for the full
  source tree. It found and the change removed the unreachable P1–P4 dashboard
  prototype from `src/App.tsx`, its unused state, derived values, and handlers,
  plus unused imports/helpers in touched panels and related UI modules.
- Search and source review found no generated sample meeting dates, names,
  tasks, or decorative waveform values in the affected Desktop runtime path.
  The Home meeting card shows an empty state until calendar data exists; task
  and project counts come from the current local state.
- Form placeholders and empty-state dashes are prompts/statuses, not seeded
  records. The explicit Mobile timeline preview fixture remains scoped to its
  preview-only mode and is not used by the Desktop runtime.
- This was not a repository-wide deletion of valid constants, product copy,
  schemas, test fixtures, or preview fixtures.

## Verification

All commands ran from the repository root unless a manifest path is shown.

| Check | Result |
| --- | --- |
| `npm run test:live-capture-routing` | PASS, 7/7 |
| `npm run test:callmd-live` | PASS, 16/16 |
| `npm run test:callmd-shell` | PASS, 18/18 |
| `npm run test:callmd-history` | PASS, 15/15 |
| `npm run test:callmd-contracts` | PASS, 13/13 |
| `npm run test:callmd-integration` | PASS, 13/13 |
| `npm run test:audio-viz` | PASS, 6/6 |
| `npm run test:design-system` | PASS, 5/5; stale-artifact diagnostic is an expected assertion inside the test |
| `npm run test:traceability` | PASS, 1/1 |
| `npm run design-system:check` | PASS, 202 tokens, 58 solid contrast pairs, 25/25 stylesheets use canonical tokens |
| `npx tsc --noEmit --noUnusedLocals --noUnusedParameters` | PASS |
| `npm run build` | PASS; TypeScript plus Vite production build, 1,820 modules |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS; native library and desktop binary compile |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib live_meeting::tests` | PASS, 26 passed, 0 failed |
| `git -c core.whitespace=cr-at-eol diff --check` | PASS |

## Remaining acceptance

- Run the native packaged or isolated UAT build on Windows with an actual
  microphone, system loopback, and an actively producing application. Confirm
  source isolation, nonzero saved PCM, channel provenance, review playback,
  orderly stop, and reopen/recovery.
- The Codex browser preview is web-only and correctly refuses to start capture
  with `NATIVE_UNAVAILABLE`. It proves layout and web-side state only.
- Supported application-process capture requires Windows build 20348 or later.
  A source compile and unit tests do not prove a device/session works on this
  machine.
- No signed installer, package acceptance, merge, or release was produced.

---
