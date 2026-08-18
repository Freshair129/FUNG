---
version: "0.2.8b"
created_at: "2026-08-13T00:00:00+07:00,ATHER"
last_update: "2026-08-19T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "implementation-plan"
  scope: "FUNG Phase 4"
---

# Phase 4 — Filesystem Test Backup and Mobile Account Implementation Plan

## Goal

Deliver one opt-in, encrypted desktop backup target using a user-selected
filesystem root for development/test only, prove clean-target Genesis restore,
and reconcile the existing mobile Supabase session/device registration without
creating a second identity model. Google Drive production transport is TODO.

## Inputs

- Requirements: `docs/superpowers/specs/2026-08-13-phase-4-cloud-backup-mobile-account-requirements.md`
- Design: `docs/superpowers/specs/2026-08-13-phase-4-cloud-backup-mobile-account-design.md`
- Existing account UI: `src/web/AccountSettings.tsx`
- Existing device/session logic: `src/mobile/MobileApp.tsx`,
  `src/components/AccountLoginPanel.tsx`, and `src/lib/authFlow.ts`

## Global Constraints

- Create an isolated `codex/phase-4-filesystem-test-backup` worktree/branch only
  after this plan is approved. Do not mix code with Phase 3 controller UAT.
- GenesisBlockDB is the only export, restore, and persistence authority. FUNG
  must not read or write a SQLite/graph/vector/blob projection directly.
- Provider tokens, recovery secrets, plaintext archive bytes, and data keys
  never enter Supabase, GenesisBlockDB, browser `localStorage`, telemetry, or
  logs. The filesystem test transport has no provider credential.
- The filesystem test transport receives its root only from the native folder
  picker, writes only encrypted archives/non-secret manifests beneath that
  root, and is labelled development/test in the UI. Google Drive remains TODO;
  when separately approved, its scope is exactly `drive.appdata`.
- No Supabase migration is planned. If an ownership/RLS defect is found, stop
  and submit a separately reviewed migration proposal; do not alter production
  Supabase state during implementation.
- Use `cargo test -j 1 --manifest-path src-tauri/Cargo.toml` on this host and
  run `npx tsc --noEmit` after every TypeScript task. Do not force
  `--test-threads=1` for the full suite; keep serial execution only for the
  focused tests that need it.

## Controller Gates (must pass before code)

1. GenesisBlockDB owner supplies and approves a versioned full-export and
   clean-target restore contract covering relational, graph, vector, and managed
   blob artifacts, with deterministic manifest verification.
2. Boss confirms a safe user-selected filesystem test root and a separate,
   empty clean-target restore location; no real user archive is used for first
   restore proof.
3. Google Drive OAuth/client configuration is TODO for a later production
   adapter and is not a gate for the filesystem test transport.

## Controller-Gate Audit (2026-08-13)

**Execution status: Gate 1 and the bounded local-root decision are available;
implementation remains development/test-only and Google Drive stays TODO.**

| Gate | Evidence | Result | Required next action |
| --- | --- | --- | --- |
| Genesis full-export + clean-target restore | FUNG pins `origin/agent/u9-backup-restore` commit `27cbb285aea635e31311ef2053d21f16e915f1fb`. The FUNG fixture commits two notes, one graph relation, and one `audio_chunks` metadata row; it exports through `Storage::export_backup`, restores through `Storage::restore_backup` into a non-existing target, and verifies source frontier, nodes, relation, and metadata. | Proven in focused automated fixture | Keep U9/release closure open pending encrypted FUNG transport and clean-install evidence. |
| Filesystem test transport | Dedicated empty roots now exist at `D:\FUNG-Phase4-TestStorage\FUNG-DEV-TEST` and `D:\FUNG-Phase4-TestRestore`. | Approved development/test proof locations | Keep all final archives encrypted and create each `restore-<archive-id>` only when restoring to a clean target. |
| Google Drive OAuth | No Drive adapter, Drive client ID, or Drive callback configuration exists in the FUNG workspace. | TODO — deferred production work | Do not implement until a later approved production-adapter slice. |

The approved plan prohibits a mock archive, direct Genesis projection access, or
generic OAuth/token-exchange implementation while these gates are absent.

## File Map (implementation targets after gates)

| Area | Expected files | Responsibility |
| --- | --- | --- |
| Backup boundary | `src-tauri/src/backup.rs` (new), `src-tauri/src/lib.rs` | Tauri commands and Genesis-only export/restore bridge |
| Crypto/archive | `src-tauri/src/backup_archive.rs` (new), `Cargo.toml` | Versioned manifest, authenticated streaming archive, recovery-key envelope |
| Filesystem transport | `src-tauri/src/filesystem_backup.rs` (new) | Selected-root validation and atomic encrypted archive write/read |
| Desktop UI | `src/web/AccountSettings.tsx`, `src/web/AccountSettings.css` | Connection, backup, restore, truthful unavailable/error states |
| Mobile reconciliation | `src/mobile/MobileApp.tsx`, `tests/authFlow.test.mjs` or new focused test | Existing-session/device-row reconciliation only |
| Tests/docs | focused Rust/Node tests, status docs | Contract, crypto, adapter, UI, and UAT evidence |

## Tasks

- [x] 1. Establish the Genesis backup/restore contract gate.
  - Record the exact upstream API/version, archive inventory semantics,
    clean-target restore behavior, and deterministic manifest fields in the
    Phase 4 plan before calling it from FUNG.
  - Add contract fixtures that include notes, graph entities/relations, and at
    least one managed blob artifact.
  - Verify: upstream owner review plus a fixture export → clean-target restore
    proves source and destination manifest identity.
  - _Requirements: R4-02, R4-05, R4-06_

- [x] 2. Add a non-secret backup state model and unavailable path.
  - Implement status/command DTOs that contain only archive ID, digest, byte
    count, timestamp, selected-root identifier, relative archive name, and
    terminal state; never key material.
  - Register a fail-closed `unavailable` result while the archive envelope and
    filesystem transport are absent, rather than creating a mock or partial
    archive.
  - Verify: focused Rust unit tests show unavailable state and a static
    response-field guard rejects recovery-secret, data-key, and provider-token
    fields in the new Rust status boundary. This task adds no TypeScript file.
  - _Requirements: R4-01, R4-04, R4-06_

- [x] 3. Implement and test the archive envelope.
  - Select pinned, maintained Rust dependencies for the approved AEAD,
    Argon2id, archive stream, and 24-word encoding; document versions, test
    vectors, nonce strategy, and recovery-secret UX before adding them.
  - Build a versioned encrypted archive with random data key, authenticated
    manifest binding, versioned KDF parameters, and recoverable key envelope.
  - Verify: RustCrypto XChaCha20-Poly1305 and Argon2id known vectors, BIP-39
    entropy round-trip, wrong-secret/tamper/truncation rejection, nonce
    uniqueness, and manifest serialization without recovery/data/provider
    secret fields. Interrupted-write preservation remains a Task 4 transport
    gate.
  - _Requirements: R4-03, R4-04, R4-05_


- [x] 4. Implement the bounded filesystem test adapter.
  - Obtain the root from the native folder picker; canonicalize all paths and
    reject traversal, symlink escape, and any archive outside the selected
    root. Use a FUNG-owned child folder and atomic same-filesystem writes.
  - Persist only non-secret root/archive metadata. Do not accept filesystem
    paths from the web UI or describe this destination as cloud backup.
  - Verify: root-boundary, traversal/symlink rejection, interrupted-write,
    digest mismatch, and missing-root truth-state tests. The native picker
    command returns only an opaque selected-root identifier; no WebView2 path
    command or provider transport was added.
  - _Requirements: R4-01, R4-04, R4-07, R4-07a, R4-10_

- [x] 5. Wire the Genesis export → encrypt → filesystem backup job.
  - Invoke only the Task 1 Genesis contract, produce a local manifest, encrypt,
    write through Task 4, and mark a backup verified only after the final file
    bytes and manifest digest match.
  - Reuse the durable job model where compatible; do not add a direct database
    authority or a second cross-store identity mapping.
  - Verify: full fixture backup succeeds; failed export/encryption/upload keeps
    the previous verified backup and reports a terminal non-secret reason.
  - _Requirements: R4-02, R4-03, R4-04, R4-06_

- [x] 6. Wire clean-target restore and post-restore verification.
  - Read to a temporary target, authenticate/decrypt before any mutation,
    invoke Genesis clean-target restore, then compare source and restored
    manifests, notes, graph identities, and managed artifacts.
  - Refuse in-place overwrite in v1 and retain the current local target on all
    verification or restore failures.
  - Verify: clean fixture restore succeeds; wrong secret, tamper, wrong digest,
    unavailable filesystem archive, and Genesis restore error preserve current
    state.
  - _Requirements: R4-04, R4-05, R4-06_

- [x] 7. Replace the desktop Cloud Storage placeholder with a bounded test UI.
  - Add selected-root status, development/test label, recovery-secret acknowledgement, explicit backup
    action/progress/result, archive list, and clean-target restore confirmation
    to `AccountSettings`; do not render or log a secret after entry.
  - Clearly state that filesystem storage is local development/test only and
    distinguish verified, failed, and unavailable backup states.
  - Verify: component/interaction tests cover root selection, unavailable state,
    unavailable state, progress, failure truth, and restore confirmation.
  - _Requirements: R4-01, R4-04, R4-10_

- [x] 8. Harden mobile account/device reconciliation without changing the model.
  - Verify the cached `fung.device.id` belongs to the current authenticated user
    before reuse; refresh one existing Android row or create one row only when
    the session is valid; clear stale cache on sign-out/revocation.
  - Recheck existing Supabase `devices` ownership/RLS behavior. If it requires
    a schema or policy change, stop at a separately reviewed migration draft.
  - Verify: valid-session reuse, stale cache, duplicate avoidance, revoked and
    missing-session paths; Dashboard shows only current-account devices.
  - _Requirements: R4-11, R4-12, R4-13_

- [x] 9. Run closure verification and record truthful release boundaries.
  - Observed evidence (2026-08-19): full Rust library suite 217/217 with the
    exact plan command; `npx tsc --noEmit` clean; `git diff --check` clean;
    focused Node suites auth 5/5, backup-flow 10/10, device-reconcile 6/6,
    desktop-bootstrap 5/5, external-tools 5/5; secret scan found no
    recovery-phrase persistence path in web/localStorage/console.
  - Open gates recorded truthfully: real clean-install desktop restore on the
    approved `D:\FUNG-Phase4-TestStorage` / `D:\FUNG-Phase4-TestRestore` roots
    and the physical Android/Dashboard identity check have not been run in
    this environment. U9 and release gates therefore stay open; the automated
    fixture restore is implementation evidence, not clean-install proof.
  - Run focused Rust/Node suites, TypeScript build, `git diff --check`, and a
    credential/secret scan. Run a real clean-install restore using the approved
    local test archive and an Android/mobile account identity check.
  - Update Mobile implementation status, Desktop real-progress, Phase 4 ledger,
    and master plan only with observed evidence. Keep U9/release gates open if
    clean restore or physical-device evidence is absent.
  - Verify: acceptance evidence maps to R4-01 through R4-13; no unverified
    production/release claim remains.
  - _Requirements: R4-01 through R4-13_

## Completion Criteria

- Clean-target restore reproduces fixture/source notes, graph, and managed
  artifacts with matching manifest identity.
- The filesystem test archive is encrypted before final write; wrong recovery secret and
  tampering fail before Genesis mutation.
- Archive-secret scans find no prohibited persistence path.
- Desktop, mobile, and Dashboard agree on the signed-in user's device rows.
- Real provider/device/release evidence is labeled separately from automated
  tests; U9 closes only after the clean-install proof.
- Google Drive production OAuth and transport remain TODO and are excluded from
  this filesystem test completion claim.

## Deferred TODO — Google Drive Production Adapter

- [ ] Create/approve installed-app OAuth client, callback, consent text, and
  `drive.appdata` scope.
- [ ] Implement keyring-only PKCE credential lifecycle and `appDataFolder`
  resumable transport in a separately approved production plan.
- [ ] Run real Drive clean-install restore UAT and update production readiness
  only from observed evidence.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.8b | Completed Tasks 5–9: backup job (export → encrypt → atomic write, failure-preserving), clean-target restore with post-restore digest identity and deep fixture verification, bounded desktop test UI with one-time recovery-phrase display and restore confirmation, ownership-verified mobile device reconciliation with sign-out cache clearing, and closure runs (Rust 217/217, tsc clean, focused Node suites green). Clean-install restore UAT and physical Android identity check remain open gates. |
| 0.2.7b | Fixed the full-suite verification procedure: the exact plan command now passes all 212 Rust library tests in 27.19s; the prior serial override exceeded the shell timeout and caused a broken-pipe artifact. |
| 0.2.6b | Implemented the bounded filesystem adapter with canonical root/layout checks, traversal/symlink rejection, atomic create-new staging, non-secret sidecar metadata, digest verification, and interrupted-write preservation tests; no UI/provider/restore orchestration was added. |
| 0.2.5b | Added the approved Task 3 in-memory archive envelope with pinned XChaCha20-Poly1305, Argon2id, and BIP-39 dependencies plus vector and failure tests; filesystem write remains deferred. |
| 0.2.4b | Added the native non-secret `backup_status` command, returning only an unavailable terminal state and null archive until encryption and filesystem transport exist. |
| 0.2.3b | Pinned Genesis U9 in FUNG and added a focused FUNG notes, graph, and audio metadata export-to-clean-restore fixture. |
| 0.2.2b | Recorded remote Genesis U9 candidate revision and named dev/test roots; implementation remains non-production and Google Drive is TODO. |
| 0.2.1b | Boss approved the filesystem-test plan. Execution remains blocked before Task 1 by the Genesis U9 export/restore contract. |
| 0.2.0b | Proposed a bounded filesystem test transport; Google Drive OAuth/transport marked TODO for a later production slice. Genesis export/restore remains the controller blocker. |
| 0.1.1b | Recorded controller-gate audit: Phase 4 is blocked before code because coherent Genesis export/restore and Google Drive OAuth configuration are unavailable or unverified. |
| 0.1.0b | Initial task plan derived from approved Phase 4 requirements and design; implementation remains gated. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.2.8b | 2026-08-19 | beta | Tasks 5–9 implemented and verified with 217/217 Rust plus green focused Node suites; U9/release stay open pending clean-install restore and physical-device evidence. | working-tree | ATHER |
| 0.2.7b | 2026-08-14 | beta | Full exact-plan Rust library suite passed 212/212; serial override timeout RCA recorded and verification command corrected. | working-tree | ATHER |
| 0.2.6b | 2026-08-14 | beta | Task 4 bounded filesystem adapter and nine focused boundary/atomicity/digest/opaque-status tests passed; UI/provider/restore orchestration and clean-install proof remain open. | working-tree | ATHER |
| 0.2.5b | 2026-08-14 | beta | Task 3 archive envelope and crypto/failure tests passed; no filesystem/provider write or restore orchestration exists. | working-tree | ATHER |
| 0.2.4b | 2026-08-14 | beta | Task 2 fail-closed native backup status and secret-response-field guard passed; no archive is created. | working-tree | ATHER |
| 0.2.3b | 2026-08-14 | beta | Task 1 focused fixture passed against the pinned U9 contract; encryption, filesystem adapter, and clean-install proof remain pending. | working-tree | ATHER |
| 0.2.2b | 2026-08-14 | beta | Gate 1 contract candidate and safe filesystem test locations are available for the isolated Phase 4 slice. | N/A | ATHER |
| 0.2.1b | 2026-08-13 | beta | Filesystem-test plan approved; no code started because Genesis U9 remains unavailable. | N/A | ATHER |
| 0.2.0b | 2026-08-13 | need review | Proposed filesystem test scope and deferred Google Drive production work. No code started. | N/A | ATHER |
| 0.1.1b | 2026-08-13 | need review | Controller-gate audit recorded; no code started. | N/A | ATHER |
| 0.1.0b | 2026-08-13 | draft | Sequential Phase 4 implementation plan with external controller gates. | N/A | ATHER |
