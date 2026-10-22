---
version: "0.1.0b"
created_at: "2026-10-23"
last_update: "2026-10-23"
status: "blocked"
superseded_by: null
attributes:
  domain: "desktop-release-acceptance"
  doc_type: "implementation-verification"
  scope: "FUNG Desktop workflow candidate build, CI, installer, isolated launch, and native audio acceptance"
  language: "Thai-first"
---

# Desktop release candidate and native UAT status

## Outcome

The Desktop workflow candidate builds and installs as an unsigned Windows NSIS
package in an isolated app identity. Source tests, TypeScript/Vite build, Rust
library regression, and Clippy passed locally. The installed app launched and
created its isolated local profile.

This is **not release acceptance**. The candidate has not completed a fresh
microphone, system-loopback, or application-process-loopback recording followed
by saved-audio review and close/reopen verification. PR #91 is still Draft and
its first hosted CI run failed on an outdated `desktop-bootstrap` assertion;
the assertion was corrected locally and the focused suite passed, but hosted CI
has not yet rerun against that correction. No merge, version bump, release tag,
or GitHub release was created.

## Evidence identity

| Field | Value |
| --- | --- |
| Date | 2026-10-23 Asia/Bangkok |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| Source/package commit | `dd9134e0c1167eae39f83151aadf05975402af80` |
| Pull request | [#91](https://github.com/Freshair129/FUNG/pull/91), open Draft |
| Candidate app identity | `dev.fung.uat.release20261023` |
| Candidate product/version | `FUNG Release UAT 20261023`, `0.1.1` |
| Installer | `src-tauri/target/release/bundle/nsis/FUNG Release UAT 20261023_0.1.1_x64-setup.exe` |
| Installer size/hash | 1,538,782,433 bytes; SHA-256 `9CB3CB6CC485FC4F67AF22F35672165058BAB7D65099503941E017296A5A3B0B` |
| Signature | Unsigned |
| Existing latest tag | `v0.1.1` still points to its historical commit; it was not moved |

The test correction and this status report are follow-up changes after the
source/package commit above. A release package must be rebuilt from the final
merged `main` commit after the release version is selected.

## Verification

| Check | Result |
| --- | --- |
| `npm run build` | PASS; TypeScript and Vite production build, 1,820 modules |
| `npm run test:ci-coverage` | PASS, 4/4 |
| Focused frontend suites for Live routing, meeting capture/review/contracts/integration, audio visualization/output, and Product MCP | PASS; individual results are recorded in the 2026-10-22 MVP report and current task run |
| `npm run design-system:check` | PASS; 202 tokens, 58 contrast pairs, 25/25 stylesheets |
| `npm run test:design-system` | PASS, 5/5 |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | PASS, 651 passed, 0 failed, 6 ignored |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | PASS |
| Product MCP focused Rust tests | PASS; 9 `product_mcp` and 12 `product_mcp_transport` tests |
| `npm run test:desktop-bootstrap` on initial PR head | FAIL, 10/11; its recording-entry assertion expected the removed `setLiveMeetingOpen`/workspace contract |
| Updated `npm run test:desktop-bootstrap` | PASS, 11/11; verifies that recording entry navigates to the active Live surface |
| PR #91 hosted frontend CI at `dd9134e` | FAIL at `test:desktop-bootstrap`; all earlier frontend steps passed |
| PR #91 hosted Rust CI at `dd9134e` | In progress at the time of this report; native custody target compilation was running |
| Isolated NSIS installation and executable process launch | PASS; installed executable started and created only `dev.fung.uat.release20261023` profile data |
| Visible non-empty app window and uninstall/reinstall smoke | NOT RUN |
| Packaged microphone/system/application capture and saved-output review | NOT RUN; waiting for the requested live microphone test |

The initial Rust full-suite run at unrestricted test concurrency had one
100-ms timeout. The exact test passed in isolation, and the complete library
suite passed on rerun with `--test-threads=4`; the first timeout is retained as
evidence rather than hidden.

## Release gates still open

- Push the stale-test correction and this report; obtain fresh green frontend
  and Rust checks on the exact PR head.
- Complete an actual microphone-only capture in the isolated installed app;
  verify nonzero saved audio, recording identity/channel provenance, review
  playback, normal close, and relaunch.
- Exercise system loopback and one actively producing application capture on
  the installed Windows package, confirming source isolation and saved output.
- Verify the installed app window is non-empty and uninstall/reinstall does not
  depend on development files.
- Resolve the release dependency/security gate: the repository has open
  Dependabot alerts for `aiohttp` 3.14.3 and PR #90 has a failed frontend check.
  Re-check the alert and PR state before release promotion.
- Merge only after the release acceptance checks are complete. Bump all package
  version declarations and add the release note, then rebuild and verify the
  final package from the exact merged `main` commit before creating a new tag.

Until these gates pass, the candidate remains `blocked`; the correct action is
to retain PR #91 as Draft and not create a tag or GitHub release.
