# RCA: meeting-intelligence People, recovery, and PDF boundaries

Date: 2026-09-25 ICT<br>
Risk: HIGH<br>
Scope: approved local M1–M5 work, including the M2 People and Windows PDF amendment

## Symptom

- Mounting the Desktop meeting-intelligence panel could evaluate the People
  scope before the transcript snapshot state was initialized.
- Archiving a People profile advanced the row revision while retaining an
  encrypted payload whose authenticated revision was older. The archived
  profile could no longer be opened or exported in a private backup.
- A People key newly written to the OS keyring was returned before a
  read-back verified the stored bytes.
- Editing or archiving a profile made a confirmed speaker link ineligible for
  resolution, but the People list did not show the link as stale.
- PDF and private-asset paths checked file size before reading but used
  unbounded reads afterward. A concurrent file change could exceed the
  declared memory bound.
- Recovery wrote each decrypted key into the OS keyring before validating all
  packages and assets in the archive, so a later failure could leave a partial
  restore.
- The AppContainer parser probe could not start the parser because its staged
  executable was under the host user's temporary directory.

## Evidence

- `src/components/MeetingIntelligencePanel.tsx` derived `peopleScope` before
  the `snapshot` state declaration.
- `archive_meeting_people_profile` incremented `participant_profiles.revision`
  without creating a ciphertext bound to that new revision.
- `OsPeopleMetadataKeyBackend::ensure_key` returned its generated key
  immediately after `set_secret`, without checking a subsequent read.
- `list_meeting_people` compared manual links with current audio evidence but
  did not compare their encrypted profile revision with the current profile.
- PDF parsing, source re-check, and private asset retrieval used file reads
  whose allocation size was not bounded by the initial metadata result.
- `verify_recovered_private_meeting_key` restored package keys inside the
  package loop, before all asset ciphertexts had been authenticated.
- `CreateProcessW` returned Win32 error 2 for the AppContainer child. The host
  `%TEMP%` ACL did not grant the AppContainer SID path access, while staging was
  performed under that host-only directory; generic production errors had hidden
  the failing operation.
- A restricted test runner also returned Win32 `ERROR_FILE_NOT_FOUND` from
  `CreateAppContainerProfile` because it could not access the per-user Packages
  profile store. The same AppContainer probe passed when run with host access.

## Root Cause

The approved implementation introduced connected UI, encrypted profile
lifecycle, PDF isolation, and multi-key recovery before its consolidated
verification freeze. The UI hook order, profile AAD revision, mutable-file
read bounds, and recovery write ordering were not maintained as explicit
cross-layer invariants during implementation. The parser runtime was staged in
the host's `%TEMP%`, which the AppContainer could not resolve when Windows
started the child executable.

## Why the issue escaped detection

The user requested one final integrated test campaign after implementation.
No test, build, or lint command had run at the time these issues were found;
the defects were identified by source review before that campaign.

## Proposed Prevention

- Keep state dependencies initialized before derived hooks and stabilize
  scope values so live revisions do not trigger redundant People reads.
- Reseal profile payloads whenever the AAD-bound profile revision changes;
  supersede old ciphertext and update metadata atomically.
- Use a bounded, zeroizing file reader for parser input and private encrypted
  assets; test the declared byte limit.
- Decrypt and validate the complete recovery package and asset set before
  mutating any keyring entry.
- Read back newly provisioned People keys and surface profile-revision drift
  as a stale link before it can be presented as confirmed.
- Prefer the AppContainer's profile path; if Windows has not materialized that
  path, fail closed. Do not fall back to host application data or relax the
  host `%TEMP%` ACL. Keep `LOCALAPPDATA`, `TEMP`, and `TMP` inside each unique
  staged tree under the profile path, and remove that tree after the child exits.
- Add a Windows AppContainer probe for user-file and loopback access, plus
  stdin and oversized-input fixtures. Keep untested hardware, provider,
  installed-artifact, Thai-accuracy, and release gates explicit.

## Verification

The consolidated local campaign passed on 2026-09-25: Rust library 573 passed
with 1 ignored; the meeting-knowledge integration suite passed 15/15; the
AppContainer probe denied host-file and loopback access; all-target check,
clippy with warnings as errors, formatting, diff check and local `cargo build`
passed. The AppContainer test needed access to the Windows per-user profile
store and remains fail-closed when that store is unavailable. These results do
not establish Thai accuracy, real meetings, other desktop targets, external
provider behavior, installed-artifact behavior or release acceptance.
