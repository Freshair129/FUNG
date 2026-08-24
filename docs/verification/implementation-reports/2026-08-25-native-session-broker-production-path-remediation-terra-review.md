---
version: "0.1.0b"
created_at: "2026-08-25T04:18:25+07:00,Terra 5.6"
last_update: "2026-08-25T04:18:25+07:00,Terra 5.6"
status: "need review"
superseded_by: null
attributes:
  domain: "application-security"
  doc_type: "independent-document-review"
  scope: "Desktop/Tauri Native Session Broker production-path remediation candidate; Browser unchanged; Mobile deferred; review only"
  risk: "HIGH"
  complexity: "C-3"
  review_target_commit: "52104b9c06dda3ed89aca41c8f9b285e71d0b761"
  candidate_path: "docs/specs/2026-08-25-native-session-broker-production-path-remediation-amendment.md"
  candidate_sha256: "D590ABB67C13FC02A1AD96B2E0D6E895DCA49321C30E09F098DB5DFFF74C0172"
  candidate_sha256_length: 64
  verdict: "FAIL"
  recommendation: "BLOCK approval; revise candidate and obtain a fresh exact-byte Terra review"
---

# Native Session Broker Production-Path Remediation — Terra 5.6 Independent Document Review

## Verdict

**FAIL — BLOCK APPROVAL.** The review-target commit and candidate hash are
correct, the scope remains Desktop/Tauri-only, and the candidate retains the
existing local evidence and external gates. However, it is not safe or complete
enough for HIGH-risk C-3 approval: the credential commit protocol has no
failure-atomic recovery contract, the single-engine rule does not define the
Drive/session lifecycle boundary, and the custody rule makes an unqualified
claim that is not currently testable through the Tauri/Serde/HTTP boundaries.
The C-3 candidate also lacks the required architecture diagram and reviewable
command-to-engine/port mapping. Each requires candidate-byte changes before
exact-hash approval.

This review changes no candidate, writer report, code, tests, configuration,
Browser, Mobile, dirty path, or external system. It does not claim
implementation readiness.

## Review target and provenance

| Item | Independent result |
|---|---|
| Review target commit | `52104b9c06dda3ed89aca41c8f9b285e71d0b761` exists and is a commit: `docs(auth): draft production-path remediation amendment`. |
| Target manifest | `git show --name-status` contains exactly the candidate and Luna writer report, both added; no other path is in the target commit. |
| Candidate bytes | `Get-FileHash -Algorithm SHA256` returned `D590ABB67C13FC02A1AD96B2E0D6E895DCA49321C30E09F098DB5DFFF74C0172` (64 uppercase hexadecimal characters). `git diff --exit-code <target> -- <candidate> <writer-report>` returned 0, so working bytes equal the reviewed commit. |
| Prior approved candidate | Candidate lines 15-16 and 60 cite `7d48aa01c243ce5f32af1005b95b71082c5a5984` and `41B91DCC09DDC5856F47A8BDFB2E1ACE021934BC696D97FFC59236C6122AA3D4`; both match the approval record. |
| Latest implementation / prior Terra review | Candidate lines 17-18 and 61-62 cite `36fa29412fc46a764e1bccae94e44bf0d4d7a6e5` and `07649e7526243446f719a2dcab63e6bba5b94285`; both commits exist and their parent relation is correct. |

## Independent command evidence

| Command | Result | Limitation |
|---|---|---|
| `git cat-file -t <target>`; `git show --name-status <target>` | PASS | Confirms commit existence and its two-path manifest only. |
| `Get-FileHash -Algorithm SHA256 <candidate>` | PASS | Exact expected 64-character candidate SHA-256. |
| `git diff --exit-code <target> -- <candidate> <writer-report>` | PASS | Confirms the reviewed working bytes equal the commit. |
| `git diff --check <target>^ <target>` | PASS | No whitespace error in the review target. |
| `git diff --name-only 36fa294..<target> -- <authorized source/tests>` | PASS | No source/test change exists between fix3 and this documentation-only target. |
| Read-only source/test audit with `rg` and line inspection | PASS for finding reproduction | Current registered commands still dispatch to `auth_session` and `drive_oauth`; current `LifecycleCore` behavioral tests remain fake-core tests, as recorded by Terra fix3. |
| Target-diff secret-value pattern scan | PASS | Zero bearer/API-key/password/JWT-like value matches; this is a bounded scan, not proof that no secret can exist. |

No build, Rust, Node, Browser, provider, database, VM, device, signing, or
release command was run by this documentation-only review. Historical local
test results in the fix3 review were read as evidence, not re-executed or
treated as implementation readiness.

## Finding table

| ID | Severity | Candidate/report location | Finding and required disposition |
|---|---|---|---|
| T5-P0-01 | P0 BLOCK | Candidate §4.1 lines 120-126; §4.2 lines 133-140; AC-GDA5-03 line 222 | The staged-write/readback → active-write/readback → staged-delete sequence has an order but no failure-atomic rule. It does not say what happens if any write, readback, delete, or verify step fails after an earlier credential write, nor require compensating cleanup, retained `cleanup_failed` state, retry/recovery semantics, or fault-injection tests at every transition. A fence prevents concurrent interleaving but cannot make two keyring slots atomic. The candidate must define the partial-commit recovery and prove it before approval. |
| T5-P0-02 | P0 BLOCK | Candidate metadata line 12; §3 D-GDA5-01 line 97; §4 lines 104-161; AC-GDA5-01 lines 220-221 | This is C-3 yet has no architecture diagram or architecture-review artifact. More importantly, it says Drive credential operations use the one `SessionLifecycle` but supplies protocol steps only for login/refresh/logout/shutdown. It does not define the Drive connection begin/complete/refresh/disconnect operation namespace, their generation/operation-ID ownership, or their interleavings with session logout/shutdown. The candidate must add a diagram and a normative mapping of every registered auth/Drive command to one engine, ports, lock/commit fence, ownership, and terminal/cleanup path. |
| T5-P0-03 | P0 BLOCK | Candidate D-GDA5-03 line 99; §5 lines 170-176; AC-GDA5-04 line 223 | The custody acceptance claim is under-specified and presently untestable as written. It prohibits ordinary secret-bearing command parameters and `String::deserialize`, while current native source uses `String::deserialize(...).map(Zeroizing::new)` at `auth_session.rs:307-310`, and Tauri/Serde plus HTTP response parsing may already own temporary input buffers. The generic exception permits only a future unnamed adapter. The candidate must name each unavoidable Tauri InvokeBody/Serde/provider-response boundary, define its lifetime/disposal/no-log constraints, distinguish a hard prohibition from an unavoidable boundary, and state a testable proof. Otherwise the requirement can be technically impossible to satisfy or merely bypassed by assertion. |
| T5-P1-01 | P1 BLOCK | Candidate §3 D-GDA5-01/02 lines 97-98; §4 lines 112-161 | The one-engine requirement is sound, as is the login/refresh stale-completion rule, but the exact credential domain is ambiguous. The current registered Drive commands are distinct paths in `lib.rs:3101-3109`; the candidate neither states whether session logout deletes Drive credentials nor specifies a separate Drive domain whose pending completion cannot resurrect a Drive credential after Drive disconnect or app shutdown. This must be explicit in the required mapping from T5-P0-02. |
| T5-P2-01 | P2 WARN | Candidate metadata lines 19-20; changelog line 282; writer report lines 19-21, 46-48, 127 | `pending post-commit` is now temporally stale, but it is acceptable as a non-self-referential drafting note: embedding this candidate's own final commit ID/hash would alter the bytes it attempts to bind. Candidate §10 correctly requires external exact-byte binding. Before Boss approval, an immutable approval record (or equivalent exact conversation record) must bind target commit `52104b9...`, this SHA-256, and this fresh Terra review. This warning does not itself require changing candidate bytes. |
| T5-P2-02 | P2 WARN | Writer report metadata line 9; title/lines 24-32; candidate §7 lines 204-205 | The Luna file is labelled `implementation-report` although its declared scope is documentation-only, and the candidate calls the already-existing Luna draft a “future implementation report.” The text makes no false implementation claim, so this is a clarity/provenance warning only. A later implementation needs a distinct, accurately classified evidence report or an explicit revision after authorization; do not imply one already exists. |

## Decision and acceptance-criteria disposition

| Area | Disposition |
|---|---|
| D-GDA5-01 single production engine | BLOCK pending the C-3 architecture mapping and exact Drive/session lifecycle domain. A test-only generic core/dead seam remains prohibited. |
| D-GDA5-02 generation and commit linearization | BLOCK pending partial-commit recovery, exact keyring-failure behavior, and Drive lifecycle interleavings. The stated precommit/commit/postcommit structure is necessary but insufficient. |
| D-GDA5-03 zeroizing custody | BLOCK pending named, bounded and testable unavoidable boundaries. Callback parsing, PKCE generation, OAuth state/challenge, Drive recovery phrase, and token payloads are listed, but ingress/parser/provider allocations are not fully specified. |
| D-GDA5-04 keyring taxonomy | PASS in principle. `NoEntry` is distinct from transient, unavailable, read, write, delete, and verify failures, and the intended result is fail-closed. It must be incorporated into T5-P0-01's partial-commit matrix. |
| D-GDA5-05 evidence matrix | BLOCK because AC-GDA5-01 through -05 cannot yet be exactly proven. Its local/static versus external-gate distinction is correct. |
| D-GDA5-06 approval semantics | PASS in principle. Exact commit + 64-character SHA-256 + fresh Terra same-byte review and invalidation on any byte/line-ending change are safe. |
| AC-GDA5-06 through -09 | PRESERVED. `cleanup_failed`, 50 simultaneous clients (one winner, 49 `proof_replayed`, zero loser mutation), typed closed IPC, ignored `GoogleDrivePanel.tsx` P2 warning, Browser unchanged, and Mobile deferred remain explicit. |
| AC-GDA5-10 through -12 | BLOCK only by the P0 defects above; no local/static command is misrepresented as an external or production gate. |

## Scope and write-set audit

The candidate is correctly bounded to HIGH-risk C-3 Desktop/Tauri remediation;
it makes no implementation, provider, deploy, release, or production claim.
Browser remains unchanged and Mobile remains deferred. D-GDA2/D-GDA3 are
expressly inherited rather than reopened, and supersession is narrow: only the
failed remediation authority for a possible fix4 is superseded; fix1/fix2/fix3
history and prior local passes remain intact.

The six-path future write set excludes `GoogleDrivePanel.tsx`, Cargo/package/
capability/configuration, migrations, Browser, and Mobile as required. It is
not silently expanded by this review. It can reasonably carry the native engine
and tests, but it is insufficient until the candidate specifies the Tauri
ingress/provider-boundary proof and the command/Drive mapping; if either needs
another path, work must stop for the separately approved scope amendment that
the candidate already requires.

## External gates remain open

The candidate correctly keeps these separate from local/static evidence:

- Real Supabase/Edge/RLS authorization, reservations, grants/revocation, audit,
  and deny-before-provider evidence.
- Google installed-app/provider UAT: consent, refresh/revocation, appDataFolder
  upload, digest-bound restore, and cancellation.
- Clean Windows keyring/VM startup, rotation, logout, shutdown, and stale
  completion proof.
- Supported-device UAT, signing, release artifact/publication, deployment,
  promotion, and explicit production approval.

## Workspace-integrity check

The target commit contains only the candidate and writer report, and its bounded
secret-value scan found no secret value. Before this report was written, the
pre-existing dirty paths were unchanged: existing modified documentation,
`BackupPanel.tsx`, `AccountSettings.tsx`, `supabase/README.md`, and the listed
pre-existing untracked RCA, temporary, plan, spec, and CSS paths. No reviewed
native source or focused test path had an uncommitted diff.

## Final recommendation

**Do not approve D-GDA5 and do not begin fix4.** Revise the candidate to close
T5-P0-01 through T5-P0-03, commit the new bytes, recompute its SHA-256, and
obtain a fresh independent Terra review before any Boss exact-hash approval.
After a passing same-byte review, record the exact commit/hash approval
immutably and keep every external gate open until independently evidenced.

## Version Diff

- `new -> 0.1.0b`: independent documentation review of commit
  `52104b9c06dda3ed89aca41c8f9b285e71d0b761`; records exact-hash verification,
  C-3/protocol/custody blockers, scope audit, retained evidence, provenance
  warnings, and the blocking approval recommendation.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-25 | need review | FAIL/BLOCK: candidate needs failure-atomic commit semantics, C-3 command/engine architecture mapping, and bounded testable custody boundaries before exact-hash approval. | recorded by the review commit | Terra 5.6 |
