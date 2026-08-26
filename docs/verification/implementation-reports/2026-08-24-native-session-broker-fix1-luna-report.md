---
version: "0.1.0b"
created_at: "2026-08-24T14:38:07+07:00,Luna 5.6"
last_update: "2026-08-24T14:38:07+07:00,Luna 5.6"
status: "beta"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "implementation-report"
  scope: "Desktop/Tauri native session broker candidate fix1"
  candidate_path: "docs/specs/2026-08-24-native-session-broker-amendment.md"
  candidate_version: "0.2.0b"
  candidate_commit: "7d48aa01c243ce5f32af1005b95b71082c5a5984"
  candidate_sha256: "41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D"
  review_base: "360c494fb1c03fbc74910dbe5ded88ef689ebb8b"
  writer: "Luna 5.6"
---

# Native Session Broker Fix1 — Luna Writer Report

## Status

`DONE_WITH_CONCERNS`: the candidate documentation corrections are complete and
committed within the exact two-file write set. The candidate remains a
documentation-only `candidate`; no implementation, runtime, deployment, or
promotion claim is made.

## Finding closure

| Terra finding | Candidate correction | Evidence location |
|---|---|---|
| P0-NSB-01 | Added exact Desktop auth, enrollment, pairing, device, Drive, local-backup, and restore operation contracts; each has inputs, native derivation, authority/grant/intent, action, redacted result/errors, cancellation/idempotency, and legacy disposition. Added the strict allowlist/no-duplicate rule and inventory-test requirements. | Candidate operation matrices and command-inventory section |
| P0-NSB-02 | Added the native session state machine, request ownership, startup/restart behavior, keyring persistence/readback order, refresh rotation and single-flight rules, failure handling, logout, shutdown linearization, zeroization, and negative-test requirements. | Candidate session-lifecycle and custody sections |
| P0-NSB-03 | Added D-GDA2/D-GDA3 inheritance, the Drive operation matrix, `drive_trusted` enforcement, separate `backup.write`/`backup.restore` grants, one-use durable replay context, archive/target-bound restore intents, deny-before-secret/provider ordering, and the 50-client evidence gate. | Candidate provenance and Drive-inheritance sections |
| P1-NSB-01 | Added named Desktop/native and browser/Mobile adapter boundaries, forbidden Desktop imports and session paths, retained browser/Mobile compatibility, and static import/build acceptance tests. | Candidate platform-boundary section |
| P1-NSB-02 | Added immutable review base, prior and inherited hashes, discovery evidence, approval-state wording, supersession/version diff, and the exact final-hash approval rule. | Candidate front matter, provenance, and version sections |

## Provenance and approval boundary

- Review base: Terra native-session-broker FAIL commit
  `360c494fb1c03fbc74910dbe5ded88ef689ebb8b`.
- Upstream security FAIL reference: `3e2b38c9d8eed0e638a93b8bd67dc8dad873c373`.
- Previous candidate SHA-256: `B2C89EBAFEE7CB0AF1648F656A802DE8CF921203AA418A1351E459382010935B`.
- Luna discovery evidence is cited only as discovery evidence; its SHA-256 is
  `426579C9E34ACACA67401258841074A89CDA9BDD6DD30A2F6BF4D9E7F0E09879`.
- D-GDA2 amendment SHA-256:
  `0655B004FF60F7B802799E50BE8CF5BC1F7026297E8BE7A5A294873E25DE98ED`.
- D-GDA3 amendment SHA-256:
  `1430552C7ACCB1D04AC1411032AC0B8EBF44A5773AB5822E14A53455D5F67792`.
- D-GDA3 approval-record SHA-256:
  `E16573B3E15FA67020598C6C0A31EF2B8BEA0DD40A64BC7415CC4CA22FB31A1F`;
  the candidate correctly records it as a working-tree record outside the
  `360c494` review base, not as a committed approval.
- D-GDA4 approval remains pending. Boss approval must bind the exact candidate
  SHA-256 below; any later content or hash change requires fresh Terra review.

## Exact write set and commits

Only these paths were authorized and changed:

1. `docs/specs/2026-08-24-native-session-broker-amendment.md`
2. `docs/verification/implementation-reports/2026-08-24-native-session-broker-fix1-luna-report.md`

The candidate was committed separately as:

`7d48aa01c243ce5f32af1005b95b71082c5a5984` —
`docs: correct native session broker candidate`

The final candidate version is `0.2.0b` and its SHA-256 is:

`41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D`

## Verification

- Candidate front matter reports version `0.2.0b` and status `candidate`.
- SHA-256 was recomputed after the final candidate write with PowerShell
  `Get-FileHash -Algorithm SHA256`.
- Candidate staging contained exactly one path; cached whitespace checks passed.
- The current source consumers were audited for the Desktop/browser/Mobile
  boundary and the old command mappings; no source consumer was edited.
- No code, configuration, tests, migrations, other documentation, deployment,
  push, merge, PR, delete, or external message was performed.
- Pre-existing dirty and untracked paths remain outside both commits.

## Concerns and open gates

This report does not claim that the broker is implemented or runtime-verified.
Fresh Terra review and exact-hash Boss approval remain open. The 50-client
proof, keyring/provider behavior, device/staging integration, platform static
checks, and production/release evidence are future acceptance gates. The
candidate's D-GDA2/D-GDA3 inheritance is documented, while D-GDA4 approval is
not implied.

## Version diff

- Candidate: `0.1.0b` → `0.2.0b`.
- Writer report: new `0.1.0b` report for candidate `0.2.0b`, hash-bound to the
  exact candidate artifact above.

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-24 | beta | Recorded Luna fix1 closure, provenance, exact write set, candidate commit, and SHA-256; external gates remain open. | report commit | Luna 5.6 |
