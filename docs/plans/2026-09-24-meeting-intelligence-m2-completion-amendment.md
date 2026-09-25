---
version: "0.1.2b"
created_at: "2026-09-24T23:25:01+07:00,RWANG,working-tree"
last_update: "2026-09-25T03:13:29+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "meeting-intelligence"
  doc_type: "implementation-plan"
  scope: "Close accepted local M2 People and PDF gaps; external activation remains deferred"
  complexity: "C-3"
  risk: "HIGH"
  language: "Thai/English"
---

# M2 completion amendment: local People review and sandboxed PDF

## Decision requested

This approved amendment amends the approved
[M1–M5 implementation plan](2026-09-24-meeting-intelligence-local-adapters.md).
The user approved this amendment on 2026-09-24. Implementation remains local;
tests stay deferred until the integrated M1–M5 source freeze.

The user-approved working tree currently supports local TXT/Markdown knowledge,
selected retrieval and encrypted typed metrics. Native PDF import returns
`PDF_PARSER_SANDBOX_REQUIRED`. The v12 identity tables and encrypted identity
resolver exist, but no populated People profile lifecycle or review UI is
available.

## Evidence and root causes

- `genesis_adapter.rs` intentionally rejects PDF before invoking the parser;
  the extractor has byte/page/time/output bounds but no OS sandbox. An
  in-process deadline cannot contain a parser compromise.
- `participant_profiles` and `speaker_identity_links` store opaque encrypted
  references, and transcript commit can resolve a confirmed link. There is no
  production `people_metadata` key creation/recovery, profile query/create,
  link review mutation, or renderer flow to create the prerequisite state.
- Existing tests cover identity-envelope validation and resolver boundaries,
  but not a complete local People lifecycle or sandbox behavior.

The root cause is an intentionally incomplete integration boundary: storage
shapes and consumers were added before the owner-controlled profile/key
lifecycle, while PDF parsing was kept out of the app until process isolation is
qualified.

## Proposed bounded implementation

### M2-People: local manual identity review

- Add vault-scoped local profile query, create, edit and archive operations.
  Profiles are user-entered labels and are not verification of legal identity.
- Store profile payloads in the existing encrypted private-asset custody path
  under a purpose-separated `people_metadata:<vault-id>` key. Do not reuse the
  `knowledge:` key. Extend the allowlisted recovery-package set so People and
  Knowledge keys stay separate and can be recovered only through the existing
  explicit owner recovery flow.
- Add recording-scoped manual link proposal, confirm, reject and unlink. Every
  mutation binds vault, project, recording, meeting/source session, speaker and
  current evidence revision; it requires the expected link revision and the
  active native owner session. Stale audio/diarization evidence invalidates the
  candidate. A name or display label alone never creates a confirmed link.
- Expose only decrypted, authorized display labels and opaque IDs to the
  Desktop renderer. Keep voice enrollment, voice matching, directory merging,
  provider identity and external access out of this amendment.
- Preserve vault-level People reuse. Review which current `owner_scope`
  comparison must change from project equality to vault ownership before
  implementation; do not silently narrow a profile to one project.

### M2-PDF: isolated text-bearing PDF extraction

- The current desktop target is Windows. Implement the PDF parser in a
  Windows AppContainer with no network capabilities, plus a Job Object for
  process-tree termination and memory/CPU limits. Android, macOS, and Linux
  fail closed with an explicit unsupported-sandbox result until separately
  qualified.
- The sandboxed child receives only one user-selected PDF through a bounded
  stdin pipe; it receives no source path, has no network capability, and
  cannot read unrelated user files. Apply enforceable CPU, memory, wall-time,
  page-count and output limits. Grant the stable parser AppContainer
  read/execute only to its pinned parser runtime; capture output through
  bounded pipes.
- The native caller must fail closed when the sandbox is unavailable or cannot
  prove its restriction. Scanned-only PDFs remain text-empty with an explicit
  warning; no OCR is claimed. DOCX, XLSX and automatic CSV remain unsupported.
- Record parser version, dependency license and source fingerprint. Keep
  import local and provider-independent.
- Before a Windows development or bundle build, run
  `scripts/stage_knowledge_parser_runtime.ps1` to create the ignored,
  minimal `.knowledge-parser-runtime` resource. The CI empty-directory
  placeholder is compile plumbing only and is not parser-runtime evidence.

## Parent and peer impact

- Parent: the v12 private-asset and backup contracts must preserve purpose
  separation, selected-vault ownership and restore behavior. No auth, device,
  signing or vendor keys enter backups.
- Peers: the knowledge picker stays TXT/Markdown-only until sandbox
  qualification; transcript attribution stays anonymous unless a current
  reviewed link exists; account/vault fences remain authoritative.
- Expected schema impact: reuse existing v12 People/private-asset rows if
  their current fields satisfy the proposal. Version the private backup
  envelope for the additional key purpose. If a required column/table or
  incompatible format is discovered, amend and approve that schema change
  before implementing it.

## Acceptance and one-campaign verification

- A locked or wrong vault cannot query, mutate, decrypt or resolve People data.
- Profile names and identity canaries do not appear in plaintext in Genesis
  rows, WAL, logs, temporary files or ordinary exports; backup/recovery restores
  only through the explicit owner flow.
- Confirm/reject/unlink require current evidence and expected revision; stale,
  replayed, cross-project and concurrent mutations fail closed.
- UI supports manual review without a voice model; anonymous/unknown remains a
  valid result and no cross-recording name inference is performed.
- Sandbox fixtures prove selected-file-only access, no network, enforced
  resource bounds, timeout/kill behavior and fail-closed unsupported targets.
- Run these checks only in the single final integrated campaign after M1–M5
  source freeze, then publish a result ledger with exact hashes and explicit
  NOT_RUN boundaries. No provider activation, real meeting, device, deployment
  or release evidence is implied.

## Risk and out-of-scope

Risk is **HIGH** because the amendment changes encrypted key lifecycle,
recovery format, identity review authority and process isolation. The change is
local-only; external provider/service activation stays deferred. No voice
recognition, enrollment, real Meet joining, sending, OCR, file upload, mobile
identity UI or deployment is proposed.

## Approval

- [x] User approved this M2 completion amendment on 2026-09-24.
- [x] Supported OS matrix and sandbox selection documented: Windows desktop
  AppContainer + Job Object; Android/macOS/Linux fail closed.
- [x] Parent/peer review completed against the v12 schemas, private-asset
  custody, backup/recovery, People identity design, and Desktop authority
  boundaries; implementation may begin within this amendment.

## Implementation status at source freeze

The source now includes vault-scoped People key provisioning with read-back,
profile query/create/update/archive, revision-bound manual speaker-link review,
encrypted recovery-package validation before keyring writes, and Desktop People
controls. Archiving reseals the profile payload against its new revision and
marks links stale when their reviewed profile revision changes.

Windows PDF parsing uses a pinned CPython 3.11.9 / pypdf 6.10.0 runtime inside
an AppContainer with no capabilities and a memory/CPU/process-lifetime Job
Object. The child receives the selected bytes over bounded stdin and no source
path. The local AppContainer/file/network-denial and parser fixtures are authored;
the consolidated Windows campaign passed. Other desktop targets remain
fail-closed for PDF import.

## Final verification — 2026-09-25

- Rust library: 573 passed, 0 failed, 1 ignored.
- Meeting-knowledge behavioral integration: 15/15 passed.
- AppContainer probe: passed; the child could not read the host-only file or
  connect to loopback. The full probe ran with access to the per-user Windows
  profile store; under the restricted sandbox, profile creation returned
  `ERROR_FILE_NOT_FOUND` and production code failed closed.
- Rust format, all-target check, all-target clippy with warnings as errors,
  diff check and local `cargo build`: passed.
- The test-only lifecycle witness race found during the campaign was repaired
  with a reader acknowledgement and passed in the final full suite.

See the [local adapter implementation report](../verification/implementation-reports/2026-09-24-meeting-intelligence-local-adapters.md)
for source hashes and the full results ledger. Real meeting, non-Windows PDF,
Thai speech accuracy, external providers, device, installed-artifact and release
acceptance remain NOT_RUN.

## Version Diff

0.1.1b → 0.1.2b: recorded the passing consolidated People, recovery and Windows
AppContainer fixtures while retaining the unrun device, Thai accuracy, provider
and release gates.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.2b | 2026-09-25 | beta | People/recovery and Windows AppContainer local tests passed; external/device/release acceptance remains open | working-tree | RWANG |
| 0.1.1b | 2026-09-25 | beta | Implemented the approved local People/review and Windows AppContainer PDF scope; campaign pending | working-tree | RWANG |
| 0.1.0 | 2026-09-24 | beta | Approved M2 local People/review and Windows AppContainer PDF implementation; tests deferred to final integrated campaign | working-tree | RWANG |
