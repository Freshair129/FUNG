---
version: "0.1.0b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Truthful routed state, desktop media admission and pre-clock PID diagnostic"
  language: "Thai-first"
---

# Installed audio remediation — source checkpoint

แก้ข้อบกพร่องที่ยืนยันได้สองจุด: สถานะการอัด endpoint ในรายการบันทึก และ
การตรวจ source ก่อนเปิด playback. **ยังไม่ได้แก้ยืนยันเสียงหายของ PID #96**;
การทดสอบใหม่หยุดก่อน capture เพราะ HDMI ทั้งสองเปลี่ยนเป็น `NotPresent`.
No merge/tag/release; the two-mode installed acceptance gate is unchanged.

Authority: [remediation plan](../../plans/2026-10-27-installed-audio-remediation.md),
[original #96 contract](../../plans/2026-10-25-issue-96-application-transition.md),
[previous actual installed failures](2026-10-27-installed-pid-endpoint-audio-uat.md).
Worktree `G:\AI-Studio\development\FUNG`, branch
`fix/issue-96-app-loopback-transition`, source baseline
`a1fac544ccf99a526e6b89287c067bcf38e94946` plus the frozen changes below.
Evidence root: `G:\AI-Studio\verification\fung-installed-remediation-20261027`.

## Demonstrated application defects and correction

1. `recording_review::live_capture_state` previously checked only the legacy
   live-session mutex. A routed session could be actively recording/draining
   while metadata said `inactive`. Both list/get now use the actual routed owner:
   starting/recording → active, stopping → stopping, terminal phases or a
   different recording → inactive. Legacy behavior is retained. Locks are acquired
   sequentially; contention blocks and poison is preserved rather than inventing
   inactive state. Tests use the production projection, including real mutex
   contention, poison and legacy-lock release.
2. Desktop `prepare_playback` relied on filenames to avoid routed audio, while
   other consumers explicitly checked authoritative source ownership. It now
   applies the same fresh Genesis SDK legacy-media guard before project paths,
   chunks or PCM are inspected. Pending, active, quarantined and completed routed
   media remain fenced, including stale caller rows and a legacy source marker
   hiding real source bindings. A valid legacy WAV still prepares without opening
   an audio output device. Source-aware playback/export is still integration work.

No direct SQLite access, capture backend switch, target injection, new driver,
system-wide fallback, missing PCM fabrication or gap/tail waiver was introduced.

## PID diagnosis: add the missing observation boundary

Independent read-only analysis of the previous pid01 still finds near-unity
marker audio until the PID tail, followed by only peak-1 floor while the endpoint
retains another 1,984 active frames. The 441-frame restart difference is the
endpoint's amplitude threshold crossing at marker frame 39 versus PID resuming
at frame 480; substantial correlated endpoint audio in frames 120–480 is absent
from PID. Neither observation is repaired by changing an amplitude threshold.

Historical v14–v16 packet traces **exist** and reconcile queued frame totals to
WAV totals, but contain no retained pre-clock PCM payload or digest. The new
`cfg(all(windows,test))` diagnostic retains decoded/downmixed mono immediately
after successful native read/decode/ReleaseBuffer, before empty filtering,
`ApplicationPacketClock`, queue and writer. Empty reads retain explicit receipts
and do not claim ReleaseBuffer occurred. It is not original stereo buffer bytes.

Only the explicitly ignored 20-second payload probe opts in through a caller-local,
scoped, single-use request; the ordinary 12-second probe does not. Storage is
preallocated and bounded to 1,152,000 mono frames and 8,192 receipts. Overflow
invalidates diagnostic completeness without changing capture flow. No file I/O,
serialization or hashing runs in the native read path. Post-driver/Stop-attempt
and writer-join emission retains PCM, offsets, flags/index/QPC, observer time,
hashes and separate native Stop/capture/writer results. Collector validity alone
is not successful Stop, full capture or a PID repair.

## RED/GREEN and combined source verification

All device/native tests remain ignored in the ordinary CPU suites.

| Scope / actual command | Result |
| --- | --- |
| Collector behavior, standalone `rustc +1.99.1 --test` | RED: 2 intended failures retained; later Cargo GREEN: 4 diagnostic tests |
| Routed production projection | Corrected RED: 4 intended failures; GREEN: 16 recording-review tests |
| Desktop playback source admission | Corrected legacy-marker and routed/pending RED retained; GREEN: 10 routed-media tests and 26 playback tests, 1 native smoke ignored |
| `cargo +1.99.1 test --manifest-path src-tauri/Cargo.toml --lib live_application_audio -- --nocapture` | 35 passed, 3 native cases ignored; overlaps collector counts |
| `cargo +1.99.1 test --manifest-path src-tauri/Cargo.toml --lib` | **885 passed, 0 failed, 13 ignored**, 162.53 s |
| `cargo +1.99.1 test --manifest-path src-tauri/Cargo.toml --test meeting_knowledge` | **17 passed**, 2.00 s |
| `cargo +1.99.1 fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo +1.99.1 clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | PASS, 48.96 s |
| `npm run test:native-session-custody` | 12 passed; its nested unqualified Cargo uses local default Rust 1.96.0, separate from the pinned full-library run |

Counts overlap. The initial wrong integration target `w1_readiness` failed before
running tests; `combined-integration.log` retains that command error. The actual
`meeting_knowledge` target above subsequently passes. Earlier task-2 invalid
fixture metadata/path and wrong-directory invocation, plus the canceled initial
Cargo RED, are retained separately and are not semantic RED or passing evidence.

## Frozen source and independent review

| File | SHA-256 |
| --- | --- |
| `live_application_audio.rs` | `BC602ABD57891680A5D2717D8CAC5E6D4856CB218CD6C410F9D39B0FA2BE6E90` |
| `application_native_payload_diagnostic.rs` | `E02F74B6F8C4B74D8E8CBD3B678ADED06AE7E70B85AAE124C1201DB9097DF3BC` |
| `recording_review.rs` | `F3A8AE0619FF0A8BB051970D6FBF09A1992CE1FAE6335EE35D6F6880EE668FB9` |
| `desktop_playback.rs` | `4E74717B7F7C2FC38E06F9100C378D330F6272DF0EEF0DD2C451FC04A433A4D6` |
| `routed_media_access.rs` | `FD3860B9814FF085BB96457CC89C9889BE7D1886FFAE92D076232EFEABF604B3` |

Independent source/spec review gives **SOURCE GO, no blocking findings** for this
bounded scope. Review Markdown SHA-256:
`EB713F142A7B4EA597CA4BA91CD14E8D371825D3F8B6941D591DECC33B09AB6C`;
JSON source/log receipt SHA-256:
`2C0933F4D5F460D05248AD8C27455CB8CD86FB365D55123D8E2E6EAA3492DAB6`.
Files are in external `reviews\task1-task2-independent-source-review-20261027T130439Z.*`.
The reviewer did not execute hardware or independently rerun the full suites.

## Native attempt n01: NOT_RUN at device admission

The reviewed controller uses owned process handles, exact test filters, frozen
render/reference binaries and repeated read-only endpoint/default/session/mute
checks. An unknown/foreign session invalidates the study; guard-breach cleanup
terminates owned captures before waiting on a renderer. Checks are sampled
observations, not proof of zero transient foreign activity.

At readiness inventory, both HDMI endpoints were active/nondefault/idle. Before
the actual n01 launch they both became **NotPresent (state bits 4)**. The native
attempt therefore stopped at admission with `Endpoint unavailable`, **no children
started, no capture/render, no PCM payload emitted**. The read-only mute guard
also correctly refuses an inactive device even though its stored mute is true.
`n01\controller-report.json`, `inventory-0001.json` and `volume-after.json` retain
the failure. This is not a negative result from the new payload probe or a new
PID/backend failure; the previously measured PID FAIL and endpoint NOT_PROVEN
remain the latest actual audio results.

Frozen native probe SHA-256:
`5E5C75E00A5DEE8F8B0E0E49D7B949C05B5075E5612BFE5C58F21CC0289E9B70`.
Its verified exact test is
`live_application_audio::windows_impl::capture_regression_tests::native_application_loopback_payload_probe`.
The controller's final SHA-256 is
`D0C9E887011FEA0128ACEBFD45119C2128A9B85110AFCAD2488AD01F2FACE085`:
the separately accepted B0ED revision differs only by that exact filter correction,
confirmed by the frozen test binary's `--list`; no test was executed by listing.

## New package checkpoint

Isolated configuration: `dev.fung.uat.remediation20261027`, product
`FUNG Remediation UAT 20261027`; hidden/unfocused main window for background API
checks. Configuration SHA-256:
`4B386C906264585DAB33D9CD8A12040F1AAFD5103D512723A162B14B2788BD70`.
Selected build inputs and source hashes are frozen in external
`package\source-build-inputs.json`; this is not an exhaustive bundled-resource
manifest. Actual package and installed observations follow below.

Actual TypeScript/Vite build passes (6.59 s), then Rust 1.99.1 optimized build
passes (3m 50s) and NSIS produces a fresh installer. The frozen image contains
the new desktop playback admission and isolated app identity; both diagnostic
schema and payload-filename strings are absent, corroborating the test-only
source boundary. `package\artifact-receipt.json` records these checks.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `FUNG-Remediation-UAT-20261027-0.1.2-setup.exe` | 2,116,634,426 | `0C68B368C657A7D98E67A54B5D0431330CD143ED17B39BE145B14C25EF14FCD8` |
| Built executable before official NSIS marker mutation | 34,999,296 | `61A1BCC2B4DAD2A7D82E2A1D86F460CAD96A4C063D0A71644BB9BB3E70968E22` |

The official single `UNK`→`NSS` bundle marker predicts installed SHA-256
`017EC1C8C40C1DC6A742C453F092CF29ECB52EBEB1CD2CBBF5AF6E34283AAFE4`;
prediction is separate from actual installed-file verification.

The fresh exact installer subsequently exits **0**. Its actual installed
`package\installed\fung.exe` hashes to
`017EC1C8C40C1DC6A742C453F092CF29ECB52EBEB1CD2CBBF5AF6E34283AAFE4`,
matching the prediction. `package\installed-first-receipt.json` explicitly records
the dirty source snapshot rather than claiming the baseline alone contains the fix.
Previous UAT and normal profiles are retained.
The immutable `artifact-receipt.json` is a pre-install build snapshot: its
installation/launch NOT_RUN fields are historical, superseded for those actions
by the later actual install and process receipts below.

Actual installed processes **36424** and **28740** use the new isolated database
path confirmed by health and owned loopback WebView ancestry. Initial project
list is empty; recording output is set inside the owned G: evidence directory.
The app creates project `52a1f215-8c33-4993-86fe-9eba8bbefc34`, closes normally,
reopens and reads the exact same project metadata. Both processes then exit **0**,
without force, and their monitors report no failure. Fresh installed inventory
also reports both selected HDMI devices `notPresent`. Legacy status is inactive
and routed status is null; no capture was requested by this smoke check.

Receipts are under `installed-check\` and `installed-check\reopen\`, including
`persistence-comparison.json` and both `orderly-close-result.json` files. This
demonstrates actual installation, boot, command dispatch and orderly project
persistence. It does **not** test an active routed projection, playback UI, crash
recovery or a new PID/endpoint audio recording. The hidden-window configuration
is for background API UAT; visible UI acceptance is not claimed.

## Remaining acceptance

- Both exact HDMI devices must return active, idle, nondefault and muted before
  actual owned native diagnostic and two-endpoint testing.
- Reconcile retained pre-clock mono against the writer and simultaneous control;
  repair demonstrated application-side loss, or establish the upstream boundary
  before proposing an architectural change. Original PID repair remains open.
- Retain the endpoint fading-tail NOT_PROVEN result; resolve the oracle with
  additional valid evidence rather than trimming PCM or enlarging 48 frames.
- Verify the changed installed lifecycle/guard behavior and complete source-aware
  UI/playback/export/recovery/enrichment plus authoritative source/chunk bindings.
- No merge/tag/release until both installed audio modes pass.

## Changelog

| Version | Date | Change |
| --- | --- | --- |
| 0.1.0b | 2026-10-27 | Corrected routed state and desktop source admission; added independently reviewed test-only payload evidence seam; preserved hardware-blocked NOT_RUN and prior audio failures. |
