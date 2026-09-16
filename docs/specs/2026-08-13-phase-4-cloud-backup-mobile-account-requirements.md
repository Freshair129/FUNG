---
version: "0.2.5b"
created_at: "2026-08-13T00:00:00+07:00,ATHER"
last_update: "2026-09-17T00:00:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "requirements"
  scope: "FUNG Phase 4"
---

# Phase 4 — Local Backup and Mobile Account Requirements

## Status and Boundary

This is the requirements draft for Phase 4 of the master implementation plan.
It authorizes no code, migration, bucket, OAuth-provider, or production
configuration change. Phase 3 controller acceptance remains a separate gate.

Complexity: C-3 — Architecture-Driven Implementation.

Change risk: HIGH — encrypted user archives, external credentials, restore
integrity, and cross-device account identity.

### Google Drive scope cancellation (2026-09-17)

Google Drive is no longer a FUNG product requirement. The Google Drive-specific
provider, OAuth, deployment and UAT portions of this requirements document are
superseded by `docs/decisions/2026-09-17-google-drive-scope-cancellation.md`.
Local filesystem backup/restore and mobile-account requirements remain active
within the Phase 4 scope; no new Google Drive implementation or external-state
operation is authorized.

## Current Integration Facts

- The desktop Account Settings surface can read/update a Supabase profile and
  display historical `oauth_connections` metadata; no active cloud-storage
  provider is exposed.
- Phase 1 already established Supabase PKCE login and a shared `devices` model
  for desktop and mobile. Phase 4 must extend that model, not add a second
  mobile identity or login flow.
- GenesisBlockDB is FUNG's only persistence and backup boundary. Phase 4 must
  not directly open, export, or restore its SQLite projection.
- U9 full closure is not proven. A production backup flow cannot claim success
  until the required Genesis export and restore contract and all acceptance
  evidence are verified.
- The development/test contract candidate is GenesisBlockDB commit
  `27cbb285aea635e31311ef2053d21f16e915f1fb` on
  `origin/agent/u9-backup-restore`. It is available for the bounded FUNG slice;
  it does not close U9 or any release gate by itself.
- Google Drive was previously considered as a production destination, but it
  is canceled by the 2026-09-17 product decision. A user-selected filesystem
  destination is permitted only for development/test proof; it is not a
  production cloud-backup substitute. Any future cloud destination requires a
  new product decision.

## User Stories

1. As an authenticated FUNG user, I want to select a local backup root so that
   I can keep an encrypted development/test archive under my control.
2. As a user, I want a backup operation to state exactly what was included and
   whether upload completed, so that I do not mistake a partial upload for a
   recoverable backup.
3. As a user setting up a clean device, I want to restore a selected backup and
   verify its contents, so that my notes, graph, recordings, and provenance are
   reproduced without silent loss.
4. As a mobile user, I want the existing signed-in account and device
   registration to be recognized consistently, so that Dashboard and pairing
   refer to the same account-owned devices.

## Requirements

### Backup and Restore

- R4-01: WHEN an authenticated user opens local Backup settings, THE SYSTEM
  SHALL show the configured destination status without exposing access tokens,
  secrets, or archive contents.
- R4-02: WHEN a user starts a backup, THE SYSTEM SHALL create the archive only
  through the approved GenesisBlockDB backup/export contract and SHALL record a
  local, auditable manifest with archive identifier, creation time, byte count,
  content version, and integrity digest.
- R4-03: THE SYSTEM SHALL encrypt archive content before final local write;
  provider credentials and archive-encryption secrets SHALL never be serialized
  into GenesisBlockDB, Supabase tables, logs, or browser local storage.
- R4-04: WHEN archive creation, encryption, destination write, or verification fails, THE
  SYSTEM SHALL retain the previous verified backup and report a terminal error;
  it SHALL not mark the new backup as restorable.
- R4-05: WHEN a user restores to a clean install, THE SYSTEM SHALL verify the
  archive digest and authentication material before mutation, invoke the
  approved GenesisBlockDB restore contract, and prove notes and graph identity
  against the source manifest before reporting success.
- R4-06: IF the required GenesisBlockDB backup/restore API is unavailable, THE
  SYSTEM SHALL block archive creation and display that U9 is not yet satisfied.

### Storage Destinations and Credentials

- R4-07: THE SYSTEM SHALL support the approved local filesystem destination
  only in a development/test build and only within a root selected through the
  native folder picker; it shall not be presented as cloud backup or production
  recovery. Any future provider-neutral destination model requires a new
  product decision.
- R4-07a: WHEN development/test filesystem storage is enabled, THE SYSTEM
  SHALL write only encrypted archives and non-secret manifests beneath the
  user-selected root, reject paths outside that root, and require an explicit
  local test label in the UI and evidence.
- R4-08–R4-10: Provider OAuth, Supabase Storage, and remote-artifact lifecycle
  requirements are historical/non-active; reintroducing any of them requires
  a new product decision and documentation approval.

### Account and Device Unification

- R4-11: WHEN mobile starts with a valid existing Supabase session, THE SYSTEM
  SHALL reuse the Phase 1 PKCE session and register or refresh exactly one
  account-owned mobile `devices` row.
- R4-12: IF the session is missing, expired, or revoked, THE SYSTEM SHALL show
  a signed-out/degraded state and SHALL not create a device row or start remote
  backup work.
- R4-13: WHEN the user signs out or revokes a device, THE SYSTEM SHALL clear
  only local session and device caches permitted by the existing auth contract;
  it SHALL not delete a remote archive without a separate explicit action.

## Required Owner Decisions Before Design Approval

1. No production cloud destination is selected in the current scope. Google
   Drive is canceled; a future OneDrive, S3-compatible, or custom endpoint
   requires a new product decision.
2. Select the encryption and recovery model for clean-install restore:
   user-held recovery secret, user password-derived key, or another approved
   portable key mechanism. Device-only keys cannot satisfy clean-install restore.
3. Confirm the initial archive scope: all Genesis data plus managed audio/blob
   artifacts, or metadata-only. Metadata-only does not satisfy a full U9 backup.
4. Google Drive OAuth/client configuration, redirect URIs, and scopes are
   canceled and must not be implemented under this requirements version.

## Acceptance Evidence

- A clean-install restore reproduces the selected source notes and graph with
  matching manifest identities.
- A tampered archive and an unavailable/unreachable destination both fail closed
  without changing existing local state.
- Credential and secret scans confirm that no provider token or encryption
  secret entered GenesisBlockDB, Supabase, logs, or local storage.
- Desktop, mobile, and Dashboard display the same authenticated account's
  device rows without duplicate registration.

## Out of Scope Until a Later Approval

- Automatic continuous backup, cross-account sharing, public links, and remote
  deletion/retention policy.
- Provider-specific write scopes beyond a single backup destination.
- Production use of filesystem storage or any claim that a local test archive
  is cloud backup.
- Claiming U9, release readiness, or full physical-device UAT from automated
  tests alone.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.6b | Records the 2026-09-17 cancellation and makes local filesystem/mobile-account requirements the active Phase 4 scope; provider requirements are historical. |
| 0.2.4b | Recorded the native fail-closed backup-status boundary: no archive, root path, recovery secret, data key, or provider token is serialized before the envelope/transport tasks. |
| 0.2.3b | Added observed FUNG contract-fixture evidence for Genesis U9; full encrypted transport and restore acceptance remain open. |
| 0.2.2b | Selected dedicated local development/test roots and recorded the reviewed Genesis U9 candidate revision; production and release gates remain open. |
| 0.2.1b | Boss approved the bounded filesystem development/test destination; Google Drive production work remains TODO and Genesis export/restore remains mandatory. |
| 0.2.0b | Google Drive production OAuth is TODO; added a bounded filesystem destination for development/test only. Genesis export/restore remains mandatory. |
| 0.1.1b | Approved Google Drive v1 option set; implementation remains gated by the derived task plan and external controller prerequisites. |
| 0.1.0b | Initial Phase 4 requirements draft with explicit provider, encryption, archive-scope, and OAuth approval gates. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.2.6b | 2026-09-17 | beta | Promoted local filesystem/mobile-account requirements as active Phase 4 scope after removing Google Drive provider requirements from the active contract. | pending | Codex |
| 0.2.5b | 2026-09-17 | beta | Google Drive production/provider work canceled by product decision; no external-state action is authorized. | working-tree | Codex |
| 0.2.4b | 2026-08-14 | beta | Task 2 status DTO returns unavailable with no archive and has a static prohibited-response-field guard. | working-tree | ATHER |
| 0.2.3b | 2026-08-14 | beta | FUNG notes, graph, and audio metadata fixture verified opaque Genesis export and clean-target restore. | working-tree | ATHER |
| 0.2.2b | 2026-08-14 | beta | Selected `D:\FUNG-Phase4-TestStorage` and `D:\FUNG-Phase4-TestRestore`; Genesis U9 candidate is available for bounded integration only. | N/A | ATHER |
| 0.2.1b | 2026-08-13 | beta | Filesystem development/test scope approved; no implementation authority before Genesis U9 contract. | N/A | ATHER |
| 0.2.0b | 2026-08-13 | candidate | Proposed local filesystem test destination; Google Drive production work marked TODO. No implementation authority. | N/A | ATHER |
| 0.1.1b | 2026-08-13 | beta | Requirements approved; Drive, recovery, archive-scope, and permission decisions recorded. | N/A | ATHER |
| 0.1.0b | 2026-08-13 | draft | Requirements proposal; no implementation authority. | N/A | ATHER |
