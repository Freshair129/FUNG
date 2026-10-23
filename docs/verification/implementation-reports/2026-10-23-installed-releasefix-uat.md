---
version: "0.1.0b"
created_at: "2026-10-23"
last_update: "2026-10-23"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-release-acceptance"
  doc_type: "verification"
  scope: "FUNG 0.1.2 rebuilt installed-package acceptance and release gates"
  language: "Thai-first"
---

# Rebuilt Desktop package acceptance

## Source preparation

Repository `G:\AI-Studio\development\FUNG`, branch
`codex/desktop-workflow-mcp-20261020`, starting revision `6e6067c26c91bbd1a880ab259fc1021fd4eff56a`.
The source changes accompanying this record fix two findings from independent
whole-PR review: preserve native application packet positions/discontinuities and
persist captured tails when packet reads fail. Quiet event timeouts keep polling.
Re-review found no additional actionable findings in the remediation.

Three regressions exercise packet position normalization and invalid timing,
actual WAV persistence on a packet failure, and a native-position jump producing
separate pre/post-gap chunks plus exact `application_loopback_discontinuity` evidence.
They were added before the missing helper implementations; focused tests and the
full library passed after implementation.

The package version is prepared as `0.1.2` across package manifests/locks, Cargo,
Tauri and frontend release metadata. Approximate public download size is corrected
to 1.5 GB. This version preparation does not create or publish a release.

## Source checks

| Check | Result |
| --- | --- |
| Full Rust library with application-capture corrections | 658 passed, 0 failed, 7 ignored; 197.83 s; compiled at 0.1.1 before metadata-only version preparation |
| Clippy, all library targets, warnings denied | PASS at 0.1.2 |
| Release distribution metadata checks | 11 passed at 0.1.2 |
| Live workspace, routing and recording-output Node checks | 26 passed |
| TypeScript/Vite production build | PASS at 0.1.2 |
| Hosted CI on starting revision `6e6067c` | Frontend and Rust PASS; superseded by the new packet-handling changes |

## Installed acceptance in progress

The user authorized rebuilding, installing and completing package acceptance,
then merging and releasing only if gates pass. The UAT package uses alternate
identity `dev.fung.uat.releasefix20261023` and product name `FUNG Capture UAT 20261023`.
Local evidence root: `G:\AI-Studio\verification\fung-releasefix-uat-20261023`.

The UAT NSIS uses no compression to reduce iteration time; the distributable must
be rebuilt from merged `main` with normal compression. The installed executable,
bundled worker scripts and manifest-listed runtime files must be verified, rather
than inferring payload equality from installer compression settings.

| Gate | State |
| --- | --- |
| Rebuilt installer and hash | IN PROGRESS |
| Install / non-empty app | NOT RUN |
| Bundled runtime manifest / offline model | NOT RUN |
| 30–60 s spoken microphone → Stop → transcript | NOT RUN |
| System loopback / selected application audio | NOT RUN |
| Review playback / durable close-reopen | NOT RUN |
| Uninstall-reinstall preserving UAT data | NOT RUN |
| Hosted CI on new release candidate | PENDING PUSH |
| Merge / final-main package / tag / publication | NOT RUN; gated by acceptance |

The superseded 0.1.1 packaging attempt was cancelled after review found packet
handling defects; it was not installed or accepted. Earlier headless microphone
evidence remains in the [capture follow-up](2026-10-23-capture-backpressure-remediation.md)
and does not replace the installed-package gates above.
