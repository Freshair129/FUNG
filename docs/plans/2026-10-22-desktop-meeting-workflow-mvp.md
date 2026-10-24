---
version: "0.1.0b"
created_at: "2026-10-22"
last_update: "2026-10-22"
status: "implemented-awaiting-native-uat"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "plan"
  scope: "FUNG Desktop capture/review MVP implementation"
  language: "Thai-first"
---

# Desktop meeting workflow MVP implementation plan

Spec: [Desktop meeting workflow MVP](../design/2026-10-22-desktop-meeting-workflow-mvp.md)
Design authority: [Current Desktop flow contract](../design/2026-10-16-desktop-navigation-and-flows.md)
Evidence: [Mockup set](../design/mockups/2026-10-22-desktop-mvp/README.md)

## Preconditions

- Worktree `codex/desktop-workflow-mcp-20261020` is clean before this change.
- Existing local Product MCP commits are retained and not rewritten.
- Main navigation, Genesis/audio source ownership, semantic tokens, and current
  capture stop/recovery guarantees remain authoritative.
- Current device-routing contract explicitly excludes application capture; this
  plan is the approved product-scope amendment and updates its source/index.

## Tasks

- [x] Save final mockup PNGs and iteration images with hash/provenance notes.
- [x] Add failing focused UI and source-routing tests before implementation.
- [x] Extend Windows capture inventory with active audio applications; validate
  process loopback support and selected audio session before recording.
- [x] Capture app audio through WASAPI process loopback and persist it through
  the existing system-source chunk path with truthful provenance.
- [x] Emit recording-scoped, throttled level events and render two real waveform
  lanes in Live; no sample data or synthetic bars in the event/UI path.
- [x] Convert settings categories to an accessible horizontal tab grid; audit
  affected Home/Live/Review/More/Companion surfaces for mock values and inert
  controls, fixing only verified defects in scope.
- [x] Update design/spec/index and Desktop progress with current truth and exact
  verification boundaries.
- [x] Build app and run focused UI/native source tests, browser visual QA, Rust
  format and targeted Rust tests; record remaining native audio acceptance
  separately.
- [ ] Run installed native app UAT for microphone, system loopback and selected
  application process-loopback, then confirm saved recording and review.

## Verification commands

- `npm run test:live-capture-routing`
- `npm run test:callmd-live`
- `npm run test:callmd-shell`
- `npm run test:audio-viz`
- `npm run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --check`
- `cargo test --lib live_meeting::tests`
- `git diff --check`

Run commands from `src-tauri` when testing Rust. Browser screenshot review uses
the user's Codex in-app browser on the local Vite preview. No physical-device,
installer, or real user meeting claim follows from these checks.

## Exit gate

Source and production-build checks pass, the mockups and design QA report are
in-repo, and no sample meeting data is hard-coded into the affected runtime
surfaces. The implementation is an MVP candidate; the installed native capture
and saved-recording UAT above remains open before calling the full workflow
accepted.
