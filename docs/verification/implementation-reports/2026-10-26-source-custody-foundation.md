---
version: "0.1.0b"
created_at: "2026-10-26"
last_update: "2026-10-26"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Additive source custody and native PID reference-format follow-up"
  language: "Thai-first"
---

# Source custody foundation and PID follow-up

## Disposition

Task 2 of the [routing plan](../../plans/2026-10-25-dedicated-endpoint-routing-plan.md)
is **PARTIAL**. Source custody is integrated into the Rust library; no new capture
executor, IPC command, UI/MCP route or installed package is enabled. Both PID and
endpoint installed acceptance remain required. **#96 is not fixed; no release.**

Baseline: `653be8b` on `fix/issue-96-app-loopback-transition`; the changes and
verification below refer to its working-tree successor. Earlier 708-test/CI
results at `607079f` are not verification of this new source tree.

## Integrated behavior

- Additive schema v16 retains the immutable v15 package and explicit migration
  step. `capture_sources` owns source UUID/recording/epoch and a durable checkpoint;
  `capture_source_chunks` binds actual audio chunks to unique source ordinals.
- Trusted in-process `capture_source_custody` APIs register one to three sources,
  verify bounded finalized PCM16 mono files/headers/hashes in source-owned paths,
  then atomically commit audio, binding, source/global checkpoints and maximum
  recording duration with a fresh Genesis frontier. Exact retries retain identity.
- All sources must share one meeting QPC origin. Each keeps its own native sample
  rate and exact frames; nonempty sub-ms tails survive zero-length ms projection.
- Quarantine is monotonic and audited. Queued audio can remain private evidence
  until closure; closed sources reject new chunks. Completion requires captured
  audio, and quarantine survives storage reopen.
- Aggregate access validation rejects mismatched/rewritten/deleted chunk ledgers,
  missing bindings, uncompleted/quarantined sources and divergent checkpoints.
  Its returned frontier is **not a reusable permission**. Consumers still need
  actual file verification, publication fencing and quarantine cancellation.

These public Rust APIs add no remote authority. Native callers must supply freshly
admitted process/device identities. Existing legacy consumers have not been wired
to the new guard, and no normal-profile data was migrated by a test launch.

## Verification

Evidence root: `G:\AI-Studio\verification\fung-endpoint-routing-20261026-task2`.

The independently reviewed external draft used the real pinned Genesis SDK:
13 tests and strict Clippy passed. Its separate harness omitted the production
meeting-control adoption step; those results are not full FUNG migration proof.

Integrated checks:

| Check | Result | Receipt |
| --- | --- | --- |
| Source custody subset with real FUNG install | 13 passed | `integrated-custody-tests.log` |
| Different-QPC-origin registration regression | Observed RED, then included in the 13-test GREEN subset | Retained command output and SDD ledger |
| Actual v15 → v16 install/reinstall/reopen with unchanged legacy audio metadata | 1 passed | `integrated-v15-v16-reopen.log` |
| Full Rust library regression after correction | 722 passed, 0 failed, 11 ignored (293.65 s) | `integrated-full-library-final.log` |
| Integrated strict Clippy, all targets, warnings denied | PASS | `integrated-clippy.log` |
| Formatting and whitespace | PASS | `cargo fmt --all -- --check`; `git diff --check` |
| Relative documentation file links | 224 checked, none missing | `integrated-doc-links.json`; no anchor/remote URL claim |

Independent integrated review found a stale schema-chain test that skipped v15;
the test list is corrected to match production install. No runtime legacy
regression was identified by that review. Final narrow independent re-review
accepts the correction and scope. The earlier full run (721 passed, 1 stale-test
failure, 11 ignored) remains in `integrated-full-library.log`; it is not overwritten.
The final 11 ignored native/provider/manual checks are not passing acceptance.

## Actual 44.1-kHz PID diagnostic

The previously unexecuted owned MF diagnostic now runs. p07 is an incompatible
controller abort before the independent reference starts; preserve it as an
invalid experiment, not PCM evidence. Its cleanup timeout masked the initiating
failure. Controller v2 retains the first failure separately from cleanup failures.

p08 uses an owned v2 diagnostic that emits an initialization receipt before
readiness and persists PCM/report before MF teardown. Its initialization reports
44,100-Hz stereo PCM16, exact flags `0x80060000`, 441-frame native buffer, target
PID 8840 and probe PID 36368. MF shutdown returns successfully. Binary SHA-256:
`0C038D1CEC464A5994C22B999780019556AAF1037A2277291B34B4FCBF7B82DE`.

Producer/keeper/independent endpoint control exit 0; PID diagnostic exits 2 with
four source gaps. Frozen dual-view analysis qualifies a second-restart discrepancy
of **439–443 common 48-kHz frames**, exceeding the unchanged 48-frame bound.
Pre-Stop estimates and first-restart comparison remain **NOT_PROVEN** under the
predeclared interpolation uncertainty budget; do not quote them as qualified loss.
Raw captured files remain native 44.1 kHz; no repaired PCM or relaxed bound.

Native evidence root: `G:\AI-Studio\verification\fung-endpoint-routing-20261025`:
`p08-wasapi-mf44100-v2`, `p08-independent-pid44100-analysis.json`,
`p08-independent-44100-report.md`, `p08-independent-44100-audit-receipt.json`.
The unchanged production PID reader remains SHA-256
`FFE188C5CE2A7BCCDDACADCD5D9A415D6065A1864FA2082806B18109264C79D6`.
This is a bounded native failure on this host, not proof of a universal OS defect.

## Remaining gates

1. Exact-ID capture adapter with exact frame/QPC events, common meeting origin,
   native runtime monitoring and persistent source-scoped quarantine.
2. Persist-before-enrich execution in both transcript profiles; publication
   cancellation/CAS; backend playback/export/local/mobile/MCP access enforcement.
3. Source-aware recovery, same-basename backup/restore, per-source export/reopen,
   UI/MCP route controls and original PID repair.
4. New isolated installer and actual installed acceptance for both modes,
   independent review, exact-head CI and normal merge before release/tag.

Binary downgrade into a v16 database is not qualified rollback. Preserve additive
tables and captured evidence; never restore an old backup over new recordings.
