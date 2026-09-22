---
version: "0.1.0b"
created_at: "2026-09-21T11:19:35+07:00"
last_update: "2026-09-21T11:19:35+07:00"
status: "need review"
superseded_by: null
actual_identity: "Banach / 01a0c225-a1ae-75d1-889c-23aaae57c9a8"
agent: "Banach"
model: "gpt-5.6-luna"
reasoning_effort: "max"
risk_level: "HIGH"
workflow_level: "C-2"
base_commit: "b336f33ec400a38f003a0665c121069a87a543ac"
version_diff: "Initial bounded environment RCA; no source, test, lock, configuration, or generated-artifact changes were applied."
attributes:
  domain: "environment-rca"
  doc_type: "rca"
  scope: "FUNG meeting-intelligence Genesis shared-target reproducibility"
  custody: "frozen inputs immutable; review-only handoff"
  cargo_slot: "RELEASED"
  verdict: "FAIL reproduced; exact causal attribution unresolved"
---

# RCA: Genesis shared-target Rust crate identity collision

## Decision summary

- **Reproduction status: FAIL.** The approved combined Genesis command exited 101 during test compilation. \`relational_u2_contract_tests\` and \`relational_u2_tests\` produced no test rows.
- **RCA status: UNRESOLVED.** The evidence establishes a mixed compiled-crate-identity condition in the shared target and makes target-artifact provenance contamination the leading environment cause. It does not prove the exact artifact overwrite or feature-unification event that created it.
- **No product conclusion.** This is an environment/build-custody finding, not evidence that the Genesis schema change or the meeting-intelligence product contract is incorrect. Explicit unlock and alternate-attempt replay remain Bohr R2 scope.
- **Cargo handoff: \`CARGO_SLOT_RELEASED\`.** The bounded Cargo process set was checked after the reproducer and was empty. No further Cargo command is authorized in this RCA.

## Scope and immutable inputs

This is a report-only C-2 environment RCA under the HIGH evidence/custody risk lane. The following inputs were read but not modified:

- FUNG frozen R1: \`C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-meeting-intelligence-repair-r1-20260921\`, main \`b336f33ec400a38f003a0665c121069a87a543ac\`.
- Genesis frozen R1: \`C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-genesis-schema-limit-r1-20260921\`, detached \`79b41a3f4ae4026d086b634c631f4f4a7ccbd142\`, with only the approved table64-to-128 source change and regression test.
- Original cached Genesis checkout: \`C:\\Users\\pc\\.cargo\\git\\checkouts\\genesisblock-88970819a8b18a23\\79b41a3\`; it remained untouched and uncleaned.
- Shared diagnostic target: \`C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-meeting-intelligence-contract-20260921-target\`.
- G1 final report: \`C:\\Users\\pc\\workspace\\fung\\docs\\verification\\implementation-reports\\2026-09-21-meeting-intelligence-g1-contract-r1.md\`, SHA256 \`19E41145C5869C67B314E4398D0831D7679B46E7F6B44DFB3B766FB20F70E9E6\`.

The Cargo process environment for the reproducer was process-scoped to \`CARGO_TARGET_DIR=C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-meeting-intelligence-contract-20260921-target\`, \`CARGO_PROFILE_DEV_DEBUG=0\`, \`CARGO_PROFILE_TEST_DEBUG=0\`, and \`CARGO_INCREMENTAL=0\`. Cargo used \`--offline --locked\`. No FUNG full-suite command was rerun.

## Symptom

The exact approved command was run against the frozen Genesis manifest, with \`-vv\` added only to capture compiler artifact provenance:

\`\`\`text
cargo test --manifest-path C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-genesis-schema-limit-r1-20260921\\Cargo.toml --offline --locked --no-default-features --features mobile --test relational_schema_resource_limit_tests --test relational_u2_contract_tests --test relational_u2_tests --test schema_version_tests -vv
\`\`\`

Result: **exit 101** while compiling the integration-test crates. The compiler emitted E0308/E0277 and the diagnostic note that multiple different \`serde_json\` crate identities were present. The failures included:

- \`tests\\relational_u2_contract_tests.rs:87:39\` and \`:93\` where \`json!(true)\` and \`json!({})\` did not match the expected \`serde_json::Value\`.
- \`tests\\relational_u2_tests.rs:243:21\` where the test value did not match the \`Value\` accepted by \`RelationalFilter::equal\`.
- \`src\\lib.rs:1084\`, the Genesis \`RelationalFilter::equal(column: &str, value: Value)\` API boundary.

The compiler reported 12 previous errors for the contract test. The two U2 test binaries therefore did not execute. The 4 resource-limit and 3 schema-version successes recorded earlier by the reviewer do not convert this combined compile failure into a pass.

## Evidence

### Manifest and feature identity

Read-only Cargo metadata resolved the frozen FUNG package to the frozen local Genesis path through this patch:

\`\`\`toml
genesis-block-native = { git = "https://github.com/Freshair129/GenesisBlock.git", rev = "79b41a3f4ae4026d086b634c631f4f4a7ccbd142", default-features = false, features = ["mobile"] }

[patch."https://github.com/Freshair129/GenesisBlock.git"]
genesis-block-native = { path = "C:/Users/pc/AppData/Local/Temp/codex-fung-genesis-schema-limit-r1-20260921" }
\`\`\`

The Genesis manifest is package \`genesis-block-native\` 0.2.5, with \`mobile\` enabled and \`default\` disabled. Its library declares \`cdylib\`, \`staticlib\`, and \`rlib\` crate types. FUNG also declares \`staticlib\`, \`cdylib\`, and \`rlib\`. Both manifests declare \`serde_json = "1"\`/\`"1.0"\`.

Both lockfiles resolve \`serde_json\` to version \`1.0.150\`, the same registry source, and checksum \`e8014e44b4736ed0538adeecded0fce2a272f22dc9578a7eb6b2d9993c74cfb9\`. \`cargo tree --duplicates\` for the frozen Genesis mobile graph exited 0 and showed no \`serde_json\` version duplicate. This rules out a simple lockfile version split; it does not rule out distinct compiled crate identities in a reused target.

### Failing compiler inputs

The verbose rustc commands for both failing tests passed the same un-hashed Genesis library and a hashed direct \`serde_json\` library:

\`\`\`text
--extern genesis_block_native=C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-meeting-intelligence-contract-20260921-target\\debug\\deps\\libgenesis_block_native.rlib
--extern serde_json=C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-meeting-intelligence-contract-20260921-target\\debug\\deps\\libserde_json-fdbb3628868d0550.rlib
\`\`\`

The \`relational_u2_contract_tests\` rustc invocation had metadata \`878e4bf77bb1a2d2\` and extra filename \`526f61952e6848fb\`. The \`relational_u2_tests\` invocation had metadata \`88ac32dc82e83499\` and extra filename \`981e4bb8becba399\`. The preserved Cargo fingerprint output for both invocations contains the same identity-mismatch diagnostics; this is direct evidence from the failing compiler, not an inference from test counts.

### Shared-target provenance

The shared \`debug\\deps\` directory contains multiple Genesis artifact families and dep-info files. The dep-info source roots are decisive for provenance:

| Artifact family | Observed provenance | Interpretation |
|---|---|---|
| \`genesis_block_native.d\`, \`libgenesis_block_native.rlib\`, \`genesis_block_native.dll/.lib/.pdb\` | Dep-info points to the frozen local Genesis path | Un-hashed local artifact family consumed by the failing rustc command |
| \`genesis_block_native-475780d309c903da.*\` | Dep-info points to \`C:\\Users\\pc\\.cargo\\git\\checkouts\\genesisblock-88970819a8b18a23\\79b41a3\` | Separate cached-checkout artifact family; rlib SHA256 \`A1630E6A47A634EE853C1A3EC194287EFD45AA0E56F5CDB4FE8AE44AC5520397\` |
| \`genesis_block_native-56a4f7aeb08ad74a.rmeta\`, \`genesis_block_native-442713ddeb579c30.rmeta\` | Dep-info/fingerprint entries point to the frozen local Genesis path | Separate local metadata generations, including a fresh rmeta generation after the reproducer began |
| \`serde_json-fdbb3628868d0550.rlib/.rmeta\` and \`serde_json-454d6cf9a41e64f9.rmeta\` | Same \`serde_json\` 1.0.150 graph identity by version/source, distinct compiled artifacts/fingerprints | Target contains more than one compiled identity candidate even though Cargo resolution has one version |

The un-hashed local Genesis rlib SHA256 after reproduction was \`ACD1F097759485AA858F28CA898D36DF4DEB0C5D9ACB4306594DA7AEABE94636\`. The directly passed \`serde_json-fdbb3628868d0550.rlib\` SHA256 was \`610B1A749689B40668F89D7619B1484D0C16502B1A530BEC53D40A26CE2EFBB8\`. Multiple artifacts alone are normal and are not proof of corruption; the material evidence is the combination of two source-root families, the un-hashed crate input, and the compiler’s distinct-identity error at the shared \`Value\` type boundary.

### Frozen-input and report hashes

The required hashes remained unchanged at handoff:

| Input | SHA256 |
|---|---|
| Genesis \`src\\lib.rs\` | \`2D17B3914BC5DD9AE5CB0EE22BF3BB5B5751FBE8EACD8DC467E190591ED94F53\` |
| Genesis \`tests\\relational_schema_resource_limit_tests.rs\` | \`494CB6A4A63C79934653C879F8DB8024D966A8AF84E05D903A424F25763F9704\` |
| FUNG \`src-tauri\\Cargo.toml\` | \`ED5CCA795367A1B31577DEAE3C81B0712B7DD13762577DD8F8702D8F653BF86E\` |
| FUNG \`src-tauri\\Cargo.lock\` | \`2952DB9D3E62281980847E941C2F89E594A502E08397AA888459AA6065FDCF4E\` |
| Final G1 report | \`19E41145C5869C67B314E4398D0831D7679B46E7F6B44DFB3B766FB20F70E9E6\` |

### Concurrent root inventory

At the final read-only inventory at \`2026-09-21T11:19:35+07:00\`, Bohr’s announced paths were absent:

- \`.brain\\rca\\2026-09-21-meeting-unlock-replay-r2.md\`
- \`docs\\verification\\implementation-reports\\2026-09-21-meeting-remediation-r2-bootstrap.md\`
- \`docs\\verification\\implementation-reports\\2026-09-21-meeting-remediation-r2-bootstrap.json\`

Their later appearance is an expected concurrent documentary addition and must not be treated as a frozen-source change. Existing untracked RCA files \`2026-09-21-meeting-identity-custody-plaintext.md\` and \`2026-09-21-meeting-workflow-lease-deadlock.md\` were left untouched. The targeted status query emitted only Git global-ignore permission warnings; no source, test, Cargo manifest, or lockfile change was made by this worker.

## Root Cause

### Confirmed causal facts

1. The failing test crates were compiled against a Genesis \`Value\` type and a direct-test \`serde_json::Value\` type that rustc treated as different crate identities, even though both diagnostics point at the semantic \`serde_json\` 1.0.150 source path.
2. The failing rustc commands consumed the un-hashed \`libgenesis_block_native.rlib\` while the shared target retained distinct hashed Genesis families whose dep-info points to both the frozen local checkout and the original cached Git checkout.
3. The Cargo lock graph and manifest feature selection do not explain the failure as a normal dependency-version duplicate. They also do not record the complete provenance of every generated artifact already present in the target.

### Leading environment cause, with attribution limit

The leading cause is **shared-target compiled-artifact identity/provenance contamination across the FUNG workspace and the standalone frozen Genesis package**. The un-hashed multi-crate-type Genesis rlib is the likely collision boundary: it can be selected together with a direct \`serde_json\` artifact whose compiled disambiguator does not match the \`serde_json\` identity embedded in that Genesis library.

The supplied hypothesis that FUNG feature unification overwrote or reused a shared Genesis rlib is consistent with the observed artifact families and the sequence “Sartre recorded 17/17, then FUNG built, then the combined command failed.” It is **not proven**. No clean isolated target or package-scoped invalidation was authorized, so this RCA cannot distinguish an overwrite from stale Cargo selection, multi-crate-type artifact naming, profile/fingerprint reuse, or another target-state transition. The exact mutation event remains **UNRESOLVED**.

## Why the issue escaped detection

- Sartre’s 17/17 record was authored before the FUNG build and was not an independent post-build rerun by this worker. It is historical evidence, not a current pass.
- The reviewer’s post-build rerun covered the 4 resource tests and 3 schema-version tests, which do not exercise the failing cross-crate \`Value\` boundaries in the two U2 test crates.
- \`Cargo.lock\`, \`cargo metadata\`, and \`cargo tree --duplicates\` describe the resolved graph, not the source-root and compiled-crate identity of every artifact in a reused target.
- A shared target contained both local frozen-Genesis and cached-Git-checkout dep-info families. The presence of multiple hashed artifacts is normally valid, so an artifact inventory without the rustc \`--extern\` paths would not by itself expose the collision.
- The first decisive check was the combined command after the FUNG build, and it failed at test compilation before the U2 runtime rows could provide a misleading partial pass.

## Proposed prevention

1. Give each distinct repository/source-root and revision lineage an exclusive Cargo target directory for the duration of its verification lease. Do not reuse a FUNG workspace target for a patched local Genesis package and its original cached Git checkout.
2. Record, with every cross-repository Cargo result, the exact \`CARGO_TARGET_DIR\`, manifest path, feature set, \`--extern\` paths, and dep-info source roots. Treat target provenance as part of the evidence packet.
3. After any upstream build or workspace feature change, rerun the decisive combined command rather than promoting an earlier partial test set or an independently authored count.
4. If a target must be reused, require package-scoped Cargo invalidation after capturing artifact hashes and dep-info; never use whole-target cleanup as the first diagnostic step.
5. Keep the distinction explicit in reports: lock-graph uniqueness is not compiled-crate identity uniqueness, and a prior 17/17 result cannot erase a later compile failure.

## Proposed recovery for parent review — not applied

No recovery or cleanup was executed. A parent-owned follow-up may choose one of these bounded probes after issuing a new Cargo lease:

### Minimal same-target collision probe

After capturing a fresh target inventory, use Cargo’s package-scoped invalidation for the exact frozen Genesis manifest and package \`genesis-block-native\`, with \`CARGO_TARGET_DIR\` still resolving to:

\`\`\`text
C:\\Users\\pc\\AppData\\Local\\Temp\\codex-fung-meeting-intelligence-contract-20260921-target
\`\`\`

The candidate un-hashed generated family identified in this RCA is:

\`\`\`text
debug\\deps\\genesis_block_native.d
debug\\deps\\genesis_block_native.dll
debug\\deps\\genesis_block_native.dll.exp
debug\\deps\\genesis_block_native.dll.lib
debug\\deps\\genesis_block_native.lib
debug\\deps\\genesis_block_native.pdb
debug\\deps\\libgenesis_block_native.rlib
\`\`\`

These are generated artifacts only. Do not manually edit binaries and do not remove hashed cached-checkout artifacts as part of this minimal probe. Prefer the package-scoped Cargo operation over manual deletion; it must not be executed until the parent explicitly leases the slot and approves the exact target effect.

Then rerun the same four-test command with \`--offline --locked --no-default-features --features mobile -vv\`, and compare the resulting \`--extern\` paths plus dep-info source roots. A pass would support, but still not fully prove, stale package artifacts as the trigger. A repeat failure leaves the RCA unresolved and requires the next minimal isolation step.

### Strong confirmation probe

If the parent later authorizes a task-owned isolated target, run the unchanged Genesis command with the same manifest, features, lock mode, and environment while resolving \`CARGO_TARGET_DIR\` to that newly leased target. This is the cleanest test of whether the failure is target-state dependent. It was not run here because a second target and a new Cargo lease were not authorized.

### Rollback and stop condition

The proposed recovery affects only regenerable Cargo outputs. Frozen source, tests, manifests, locks, and report evidence remain unchanged. If the bounded probe fails or produces a new provenance split, stop; do not expand to whole-target cleanup, dependency changes, compiler-error suppression, assertion changes, or source/schema edits. Preserve the original exit-101 failure in all subsequent reports.

## Impact, risk, and evidence status

- **Impact:** blocks the combined Genesis U2 compile gate and therefore blocks a clean G1 environment handoff. It does not establish a product defect or a schema-contract failure.
- **Risk:** HIGH because shared generated artifacts and cross-repository provenance can invalidate otherwise plausible local evidence. The exact causal transition is not proven.
- **Source/test/lock change:** none.
- **Explicit unlock and alternate-attempt replay:** out of scope; assigned to Bohr’s future R2 work.

| Check | Status | Evidence boundary |
|---|---|---|
| Combined four-test Genesis command | **FAIL** | Exit 101 at integration-test compilation; U2 tests did not run |
| Resource-limit and schema-version subsets | **PASS (reviewer-recorded)** | Partial post-build evidence only; not a combined pass |
| Sartre 17/17 | **REVIEW_ONLY** | Authored before FUNG build; not independently rerun here |
| Lockfile/manifest frozen hashes | **PASS** | Exact mandated hashes match |
| \`cargo metadata\` and \`cargo tree --duplicates\` | **PASS** | Graph resolves one serde_json version; does not validate target identity |
| Clean/isolated-target confirmation | **NOT_RUN** | Requires parent lease and explicit recovery approval |
| FUNG full suite | **NOT_RUN** | Intentionally not rerun; known \`.venv-whisper\` exclusion remains a separate boundary |

## Version diff

\`0.1.0b\` is the initial documentary RCA version. It adds only this report. No source, test, manifest, lockfile, configuration, cache, binary, checkout, or generated artifact was changed. The status remains \`need review\` because the environment contamination hypothesis is evidence-backed but the exact root-cause transition is unresolved.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-21 | need review | Initial bounded shared-target type-identity RCA; failure preserved and recovery deferred | b336f33ec400a38f003a0665c121069a87a543ac | Banach / 01a0c225-a1ae-75d1-889c-23aaae57c9a8 |

