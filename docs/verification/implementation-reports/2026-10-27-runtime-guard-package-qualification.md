---
version: "0.1.1b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Native session lifecycle runtime guard and isolated package qualification"
  language: "Thai-first"
---

# Runtime guard and package qualification — execution checkpoint

Authority: [lifecycle subplan](../../plans/2026-10-27-native-capture-session-lifecycle-plan.md),
[dedicated endpoint design](../../specs/2026-10-25-dedicated-endpoint-routing-design.md)
and [parent implementation plan](../../plans/2026-10-25-dedicated-endpoint-routing-plan.md).

## Revision and scope

Checkout: `G:\AI-Studio\development\FUNG`; branch
`fix/issue-96-app-loopback-transition`; starting committed revision
`df4b12879915a39f7686513506954fdd37043f2e`. The resulting implementation commit
contains this report; external `package/source-build-inputs.json` records the
reviewed worktree inputs used by the UAT build. No merge, tag or release is
authorized by source tests alone.

The implementation introduces a production native group factory, MTA-local
capture-client ownership, revocable weak observers, real Storage binding reads,
a production routed capture coordinator and native Tauri start/status/stop.
The public initial/preview admission policy stays strict. Audit JSON is evidence,
not authority to create a live capture exception. Capture infrastructure is not
an authorized renderer. Default-instance/same-PID exceptions are not introduced.

Legacy media consumers reject pending and finalized routed recordings until
source-aware access is qualified; this is a staging fence, not source export or
playback implementation. No HTTP/LAN recording-control endpoint is added.

## Evidence inventory

External evidence root: `G:\AI-Studio\verification\fung-runtime-guard-20261027`.
Subdirectories `owner`, `storage`, `service`, `start-stop`, `media-fence` and
`pid-only-input` retain scoped RED/GREEN logs, source hashes and review notes.
Earlier failures remain intact. Selected-test counts are not summed into a claim
that the whole current tree passed.

- `integrated-full-library.log`: historical composed snapshot **829 passed,
  0 failed, 12 ignored**, before later fixes. It is not the final-tree result.
- `final-library.log`: the first full current-tree run reported **848 passed,
  1 failed, 12 ignored** in 307.08 seconds. The unchanged playback delayed-install
  test timed out waiting 100 ms for a post-cleanup notification; the owned worker
  join was already required immediately afterward. An isolated run of the frozen
  unchanged test passed in 40 ms (`playback-first-failure-isolated.log`). The
  scheduler-sensitive notification was replaced by the existing deterministic
  join, preserving cancellation, rejected late install and cleanup guard assertions.
  Product deadlines and audio acceptance thresholds are not changed.
- `service/round2-targeted-final.txt`: **19 passed**, but independent review
  subsequently found cancellation-spanning peer attribution and masked final
  admission evidence. A passing selected suite did not close those findings.
- `service/round3-targeted-final.txt`: **26 passed**; scoped independent review
  accepts cancellation-intent publication before revocation and final per-source
  checks after joins on all execution exits. A later test-only Clippy correction
  replaces a redundant clone with `std::slice::from_ref`.
- `start-stop/2026-10-27-started-capture-owner-publication.md`: two ownership
  regressions pass and scoped review accepts handle publication before shutdown
  or response failure. Pure ownership evidence is not native shutdown acceptance.
- `storage/task-a-review-custody-regression-green.log`: **22 passed** against
  actual local Storage. Deterministic frontier interleaving remains unqualified.
- `pid-only-input`: optional microphone-disabled input has policy, production
  bridge and frontend evidence; it is not PID transport repair or hardware UAT.
- `legacy-guard-owner`: final review found that empty legacy shutdown and an old
  post-join cleanup could release a newer routed owner's shared guard. Two
  deterministic regressions reproduced both resets (**1 passed/2 failed**), then
  **3/3 passed** after removing both unowned releases and the old cleanup's
  unconditional slot clear. The actual `CaptureRuntimeLease` remains the owner.
  Live regression: **41 passed/2 native ignored**. Independent review accepts
  `live_meeting.rs` SHA-256
  `8B5EFD1976EDB786E454CC6679B9CCF2A650A918BC6B8753918189F052977CFE`.
- Final independent source review reports **no remaining MUST findings in the
  reviewed tree**. This is source review, not native or installed capture approval.
  Playback test synchronization is accepted at SHA-256
  `CABFF528B43A161D434872BEA89F1E552C090392B87D5FF5AB60AF39E4D53487`.

Local source verification before the installed lazy-project-path finding:

| Command | Result | External evidence |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | **851 passed, 0 failed, 12 ignored**, 602.05 s | `final-library-bounded.log` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | **PASS**, exit 0 | `final-clippy-after-fixes.log` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all --check` | **PASS**, exit 0 | executed after final source freeze |
| `npm run test:native-session-custody` | **12 passed** | `final-native-custody-contract.log` |
| `npm run test:callmd-contracts` / `test:callmd-live` / `test:live-capture-routing` / `test:callmd-integration` | **15 / 19 / 8 / 13 passed** | `frontend-{contracts,live,routing,integration}.log` |
| `npm run build` as the actual Tauri `beforeBuildCommand` | **PASS** | `package/build.log` |
| Changed documentation link check | **147 local links, 0 missing** | `docs-link-check.json` |

Four test threads bound concurrent SDK/scheduler load; every ordinary library
test still ran. The original unrestricted parallel failure remains preserved,
and its deterministic synchronization fix is separately reviewed above. Ignored
native/manual tests were not converted into passes. The final checks are local;
hosted CI for the resulting commit is a separate gate.

## Owned native attempt n05

Frozen component test executable:
`G:\AI-Studio\verification\fung-endpoint-routing-20261026-native-probe\frozen-n05\fung-native-component-tests.exe`.
SHA-256: `8CCA27C34BA2191ADB3C6E8E833E17F6DA2F46A2A2DD41EFA3F8D23F1288B050`.

The ordinary `routed_native_probe::probe_environment` filter ran **2 passed,
0 failed** on that frozen artifact. These are validation-only tests with no
capture. The reviewed controller was invoked as:

```powershell
& 'G:\AI-Studio\verification\fung-endpoint-routing-20261026-native-probe\controller-draft\run-owned-native-probe.ps1' `
  -RunName n05-runtime-owned `
  -TestExe 'G:\AI-Studio\verification\fung-endpoint-routing-20261026-native-probe\frozen-n05\fung-native-component-tests.exe' `
  -TestExeSha256 '8CCA27C34BA2191ADB3C6E8E833E17F6DA2F46A2A2DD41EFA3F8D23F1288B050' `
  -EnvironmentValidationReceipt 'G:\AI-Studio\verification\fung-endpoint-routing-20261026-native-probe\frozen-n05\environment-validation.json' `
  -RendererSeconds 60 -Scope Normal
```

Result: **FAIL at read-only idle-target preflight**, `Idle target inactive: a`.
Fresh native inventory reports both LM22-B200S and Mi Monitor HDMI endpoints as
`notPresent`. No renderer or capture was started; no microphone, routing, default
device or volume setting was changed. The controller report and original native
inventory are retained in `controller-runs\n05-runtime-owned` under that evidence
root. This is an unavailable-device gate, not a new PCM failure or a successful
runtime exception test.

User disposition at n05: HDMI is unavailable; continue source/package work.
Do not substitute the default Focusrite or communications ONIKUMA endpoints for
the approved two-endpoint test.

## Actual installed package A and lazy project-root correction

The actual unsigned NSIS package was built, hash-verified and silently installed
at `G:\AI-Studio\verification\fung-runtime-guard-20261027\package\installed`
with application identifier `dev.fung.uat.guard20261027` and separate
`fung-guard-uat` deep-link scheme. The normal `dev.fung.local` profile was not used.

The first hash comparison stopped before launch because the installed executable
differed from the restored post-build executable. Full-file comparison proved
exactly three changed bytes at offsets 28779400–28779402: the unique bundle
marker suffix changed from `UNK` to `NSS`, with every other byte identical.
[Tauri CLI 2.11.4's bundler](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle.rs)
patches that marker for NSIS and restores the unpatched build output afterward.
The comparison was corrected to independently predict the complete installed
binary; no arbitrary hash difference was accepted. Original failure and
`package/nsis-marker-verification.json` remain intact.

- Installer A: **2,116,932,442 bytes**; SHA-256
  `7F832931B8D9157F12048743DF6C17091C49501019D05B12AC88F7040C018371`.
- Unpatched A executable: SHA-256
  `C3B6122C761BBC645C59ABF97F8786221325F6F523B634386E6958B353F24571`.
- Actual installed A executable, exactly matching the marker prediction: SHA-256
  `DDAA444D979EFCC6581CE916788E57F2ECD78009B9704E9A2F9B91A2772801D4`.

The hidden installed app was controlled through its existing native Tauri IPC,
using a loopback-only WebView debugger whose owner PID was verified as a child
of the exact installed process. No mouse/keyboard control was used. A later
inventory showed hardware state had changed; that read-only attempt stopped
before any capture. Subsequent negative tests use deliberately nonexistent
endpoint IDs and never select real hardware. Audio tests remain deferred by the
user's instruction, independently of observed device availability. One empty
owned-directory setup failure is also retained; no data was removed to retry.

Package A then exposed a real integration failure: `create_project` persists
its storage path lazily, but routed `project_root` required the directory to
already exist. The actual command failed with `existing absolute project root
required` before native endpoint validation. The test-created project and all
original failure receipts are preserved.

The correction validates the entire ordinary absolute path before any creation,
retains nearest-ancestor reparse and final canonical checks, creates only the
ledger-derived project root, then performs/removes its write probe. Real SDK and
filesystem RED/GREEN tests cover lazy nested creation, no probe leftovers,
malformed Windows components without side effects and an owned temporary
junction. Scoped review accepts source SHA-256
`2768EE1BDE47549AB6964F412F506DEBCD2D9AD5D280F30BBB8437F7CCE12CD2`;
the routed suite passes **30/30**. R3 ownership/failure semantics are unchanged.

Fresh verification after this actual-package correction:

| Command | Result | External evidence |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | **855 passed, 0 failed, 12 ignored**, 337.52 s | `final-library-rootfix.log` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | **PASS**, exit 0 | `final-clippy-rootfix.log` |
| `npm run tauri -- build --config ...\package\tauri-uat.config.json --bundles nsis` | **PASS**, actual replacement NSIS package built | `package/build-rootfix.log` |
| `npm run test:native-session-custody` after root correction | **12 passed** | `final-native-custody-contract-rootfix.log` |
| Final changed-document link check, including architecture/progress | **214 local links, 0 missing** | `docs-link-check-final.json` |

## Actual installed package B — negative IPC and orderly reopen PASS

Package B was independently frozen, installed over the exact owned UAT package A
at the same G-drive location and tested with the same isolated profile and real
project created by package A. The original failed package and receipts remain.

- Installer B: **2,116,861,274 bytes**; SHA-256
  `B80760184B9EC9CF98551DE42FC0B4FCE60C67F9E2F9758C4CD315C5A6FB4753`.
- Unpatched B executable: SHA-256
  `1AF82079411D0D521FADF9430CC0B1748AB867CBFF2E9B5CD1579C79E7CDF713`.
- Predicted **before installation**, and matching actual installed B: SHA-256
  `B2670C337F09F30738A1EA3DD60591A4053EC18D9A94EF2A8DF17CEFAAF9A6CB`.
- Compiled/runtime source hashes match `package/source-build-inputs-rootfix.json`;
  documentation was subsequently updated. The manifest's Cargo content remains
  unchanged; packaging only refreshed its working-copy representation.

The first B attempt reached the correct native rejection, `explicit endpoint
absent; no default substitution`, after creating the previously missing project
root. Its test apparatus incorrectly expected a synonym (`missing`/`not found`)
and failed. The original receipt is preserved. The expected message was checked
against production `capture_routing.rs` and tightened to that exact literal;
application code and audio criteria were not changed to obtain a pass.

The follow-up test of that exact installed B executable passed **16 native IPC
calls**, then orderly close/relaunch passed **7 calls**, for **23 calls total**:

- Existing project ID/name/storage location survives the actual package upgrade
  and second native process. The normal profile is not accessed.
- Empty audio selection, invalid PID, disabled-microphone/invalid-system intent,
  empty route plan and nonexistent endpoints reject. The endpoint test selects
  deliberately nonexistent IDs; it cannot start a real audio stream.
- The lazy project directory exists, its temporary write probe is removed,
  routed status remains null, Live status is inactive and recording list is empty.
  Rejected route admission releases its reservation, proven by the following
  output-root setter succeeding.
- Both installed processes exit **0** through the existing native close command;
  no forced kill or cleanup error. Final check finds **0 WAV files, 0 FUNG
  processes and 0 temporary debugger listeners**.

Primary external receipts: `package/installed-package-rootfix-verification-report.json`,
`installed-verify-receipt.json`, `installed-reopen-receipt.json` and
`final-installed-summary.json` under `package/`. These are actual installed
negative/metadata checks. They establish no successful capture, PCM separation,
microphone, PID transitions, UI visual acceptance, MCP execution or source export.

## Rust 1.99.1 CI correction and actual installed package C

Hosted run `40398105477` for source `6375631` passed frontend, native compilation,
formatting and 12 custody-contract tests, then **failed Clippy** on three new
`chunks_exact_to_as_chunks` diagnostics. The runner uses Rust **1.99.1**; the
earlier local strict check used **1.96.0**. The hosted library regression was
**SKIPPED**, not passed. The original job log is retained at
`ci/6375631-rust-failure-decoded.log`.

Exactly three iterator expressions were replaced with the stable fixed-array
chunk APIs: one PCM encoder and two test decoders. The 8192-byte buffer, 2-byte
little-endian conversion, write lengths and final short tail are unchanged.
No lint suppression, dependency/toolchain pin or audio criterion was changed.
Scoped independent review accepts source SHA-256
`58088035D1B6186FBC8A483C6974E70904A5539FCC55000DDDFBA771404B58B5`
and tests SHA-256
`BE1AD3B9A42D4E0FB00C4FD3D78E8E62AA9695CB337A5C0BE1CB912F6D8AB219`.

Fresh exact-toolchain checks: `cargo +1.99.1 test --manifest-path
src-tauri/Cargo.toml --lib routed_capture -- --nocapture` passes **30/30**;
`cargo +1.99.1 clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets
-- -D warnings` and `cargo +1.99.1 fmt --manifest-path src-tauri/Cargo.toml
--all --check` pass. The existing 9001-sample oracle retains its final 809-sample
tail and exact bytes. Evidence: `service/ci199-lint/`.

The source correction was rebuilt into actual package C and installed over the
exact owned B installation with the same UAT profile/project:

- Installer C: **2,116,986,714 bytes**; SHA-256
  `52797D028469FE25C364E947641AF06EE5D574784F985597134AEEA5E10FF516`.
- Unpatched C: SHA-256
  `E5A40B9860BA0AB47E29F4D79471C6AD6F2557EE817FA9313EE76BFD71E7619B`.
- Independently predicted before installation and matching installed C: SHA-256
  `BA73B5A6517480A7BA95C8F01718B038787C81379F80A9BCE12C13EAC984BE89`.
- **16 negative/metadata IPC calls and 7 orderly reopen calls pass again**;
  both processes exit 0, no forced termination or cleanup errors, same real
  project remains and no WAVs/debug listener/FUNG process remain.

Primary C receipts: `package/artifact-ci199-receipt.json`,
`nsis-ci199-marker-prediction.json`, `installed-package-ci199-report.json`,
`ci199-installed-verify-receipt.json` and `ci199-installed-reopen-receipt.json`.
The broad 855-pass local snapshot above belongs to the pre-lint-correction source;
current exact-toolchain targeted checks and actual package C evidence are distinct.
Hosted full-library acceptance must be freshly checked on the resulting head.
Neither the style correction nor a successful package negative test qualifies
actual endpoint/PID capture.

## Remaining gates

- Keep the reviewed runtime source frozen; any later code change requires
  relevant fresh review/verification.
- Preserve package C and its verified profile for the next native-audio stage;
  installed negative/reopen checks do not replace successful capture acceptance.
- With the approved HDMI devices present, run native exact-instance and negative
  observations, two-source PCM/separation, transitions, tails and reopen evidence.
- Fix and independently qualify direct PID capture; its earlier transition failure
  remains open. The retained-client diagnostic draft has not run on hardware.
- Qualify product source consumers and actual installed endpoint **and** PID
  workflows. No merge/tag/release until both independent installed gates pass.
