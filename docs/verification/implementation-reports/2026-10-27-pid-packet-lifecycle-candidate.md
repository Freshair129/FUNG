---
version: "0.1.0b"
created_at: "2026-10-27"
last_update: "2026-10-27"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Offline PID reader lifecycle candidate and build-only package"
  language: "Thai-first"
---

# PID packet lifecycle — offline candidate checkpoint

Authority: [packet lifecycle plan](../../plans/2026-10-27-pid-packet-lifecycle-plan.md)
and [original #96 repair contract](../../plans/2026-10-25-issue-96-application-transition.md).
Baseline: `6981de467efc1b665563a53774d5a237c43130db`, branch
`fix/issue-96-app-loopback-transition`, checkout `G:\AI-Studio\development\FUNG`.
Evidence root: `G:\AI-Studio\verification\fung-pid-packet-lifecycle-20261027`.

## Claim boundary

The user authorized source correction and building a new package. All actual audio
tests wait for a later user readiness announcement. No capture/render/inventory
apparatus, microphone, system/application audio, audio setting, installed app or
normal profile is used in this stage. No install, merge, tag or release.

Original #96 remains open: previous native studies lose active correlated PCM at
renderer Stop/Resume. Timer delivery, MF callbacks, converter flag/format changes
and 1 ms timeout draining have not repaired that failure. This work does not
reintroduce those experiments as demonstrated fixes, manufacture missing PCM,
broaden source selection or relax the 48-frame edge criterion.

## Diagnosed component defects

The baseline reader checks stop only outside an inner native drain loop. A
continuously nonempty buffer can delay cancellation until exhaustion or queue
overflow. A positive packet-size observation followed by zero-frame acquisition
also loops without returning to its control boundary. Microsoft specifies matching
counts between the two calls; this is an anomaly to handle defensively rather than
an established normal Windows race.

Startup does not check cancellation immediately before Start, and ignores a lost
readiness receiver. A late activation can therefore start after the caller times
out. The dependency's activation wait itself is not hard-cancelable in this patch.
The dependency also maps failed event waits to timeout, hiding terminal wait errors.

Source corrections and scripted regressions are tracked under `implementation/`;
fresh independent results, source review and package build evidence must be read
separately.

## Retained characterization and RED evidence

The current production loop was extracted into a private shared driver without
changing its behavior. Its finite-packet/final-Stop characterization passed **1/1**.
Six bounded scripted regressions then failed against that actual extracted
implementation: continuous-drain cancellation, cancellation before Start,
abandoned readiness, empty-after-positive-peek progress, read plus cleanup failure,
and unwind cleanup/evidence. They fail promptly rather than hanging a test runner.

Commands use Rust **1.99.1** and the `live_application_audio::packet_driver`
test filter recorded in each original log. Evidence: `implementation/01-characterization.log`,
`02-red.log`, and the frozen characterization sources. These are deterministic
component tests. No fake packet is treated as native capture evidence.

## Reviewed source correction

The private shared driver now checks cancellation before Start/readiness and
every inner packet peek. Normal cancellation stops native production once before
draining its final buffer. Error/caught-unwind paths attempt Stop on the same
owner thread, then close the accepted queue and join the original writer; primary,
Stop and writer failures are combined rather than hiding secondary evidence.

An anomalous empty acquisition while running returns to the wait/control boundary;
the same anomaly after Stop fails rather than certifying a complete drain. A
private readiness-cancellation latch survives caller timeout. Lost readiness
delivery stops capture; the dependency's in-progress activation wait is still not
hard-cancelable. The borrowed native event handle is retained by its owner while
the adapter distinguishes signaled/timeout/failed/unexpected wait status.

Format, decoder, packet clock, event flags, backend, 100 ms wait, queue bounds,
application selection and source gap/tail criteria are unchanged. No timeout-drain
workaround, silent-stream injection or system-wide fallback is added.

Frozen source SHA-256:

| File | SHA-256 |
| --- | --- |
| `src-tauri/src/live_application_audio.rs` | `1ADA21F639831CD1FF5683D208040ED37803E3E892E1DBD7FAE7767DADC0BF4F` |
| `src-tauri/src/application_packet_driver.rs` | `D6A38A2F383BACF78A8CE55ED5A7C9DB27A2770FE0666FD9C0C9C1D25953E4CA` |

Independent task review found **no blocking findings** in these frozen sources;
see `review/task1-review.md` and `frozen-review-inputs.json`. The reviewer inspected
the original RED sources/logs and actual native call path. It is source review,
not native or installed acceptance.

Fresh focused command `cargo +1.99.1 test --manifest-path src-tauri/Cargo.toml
--lib live_application_audio -- --nocapture` passes **31 tests, 0 failed,
2 native probes ignored**. The new driver has 15 tests, and a real production
queue/writer fixture preserves exact `[1200; 480] + [2400; 7]` normal-stop PCM and
the accepted `[1200; 480]` tail after an injected read failure. Output is synthetic
fixture WAV data, not device audio. Evidence: `implementation/04-green-focused.log`.
Exact Rust 1.99.1 formatting and strict all-target Clippy pass; see
`verification/format.log` and `implementation/06-clippy.log`.

## Full local verification on frozen source

| Command | Observed result | Receipt |
| --- | --- | --- |
| `cargo +1.99.1 test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | **873 passed, 0 failed, 12 ignored**, 224.16 s | `verification/full-library.log` |
| `cargo +1.99.1 test --manifest-path src-tauri/Cargo.toml --test meeting_knowledge -- --test-threads=4` | **17 passed, 0 failed**, 22.66 s | `verification/integration.log` |
| `RUSTUP_TOOLCHAIN=1.99.1 npm run test:native-session-custody` (process-local environment) | **12 passed, 0 failed** | `verification/native-custody-contract.log` |
| `cargo +1.99.1 fmt --manifest-path src-tauri/Cargo.toml --all --check` | **PASS**, exit 0 | `verification/format.log` |
| Rust 1.99.1 strict all-target Clippy, `-D warnings` | **PASS**, exit 0 | `implementation/06-clippy.log` |

Counts across these suites overlap and must not be summed. The 12 ignored cases
remain unexecuted, including actual capture/active-inventory probes. The COM
apartment regression uses a non-inventory closure; fixture WAVs and test loopback
HTTP sockets are not real device audio. Hosted CI and actual package acceptance
are separate from these local results.

All six owning/new documentation files are indexed; **241 local Markdown links,
0 missing targets** were checked (`verification/doc-links.json`). This is a path
check, not rendered documentation or UI acceptance.

## Actual build-only package

The following command completed with exit 0, including TypeScript/Vite and a
Rust **1.99.1** optimized release build (7m49s), then **one actual NSIS bundle**:

```powershell
$env:RUSTUP_TOOLCHAIN = '1.99.1'
npm run tauri -- build --config G:\AI-Studio\verification\fung-pid-packet-lifecycle-20261027\package\tauri-uat.config.json --bundles nsis
```

The overlay inherits the base bundle's real resources; it does not replace them
with empty fixtures. It uses separate app ID `dev.fung.uat.pidguard20261027`,
product `FUNG PID Guard UAT 20261027` and deep-link scheme `fung-pidguard-uat`.
Building does not create or access that application's profile.

Frozen artifacts under the evidence root's `package/`:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `FUNG-PID-Guard-UAT-20261027-0.1.2-setup.exe` | 2,116,581,508 | `D1A236756D1A5348C8E10E7FB973DEC99825FC101F17E66D58241962A0A32806` |
| `fung-pidguard-candidate.exe` (unpatched build executable) | 34,938,368 | `0F5EE99FB4F03133581675D43C087EE530AB98B2620E06D5D770638400FF256D` |

Configuration SHA-256:
`C66F8C7FCE59882FDE781009A5EC934A0A1DD1EC91CDBE64563DD750CC4028F3`.
`source-build-inputs.json` pins baseline `6981de4`, dirty-at-build source hashes
and Git blobs; `artifact-receipt.json` verifies those inputs remained unchanged
through packaging. Both copies hash identically to their exact build outputs.
Compiled executable inspection finds the isolated app ID and the new lifecycle
diagnostic string. These are identity/provenance checks, not execution acceptance.

The receipt predicts Tauri's NSIS marker-patched installed executable hash using
the existing exact marker rule. That hash is **prediction only**; no payload was
extracted, installed or executed. An independent reviewer separately rehashed
the actual frozen installer/executable and matched the receipt, configuration
and source hashes. Build log and freeze script/log are retained alongside them.

Final independent whole-task review accepts the frozen source, docs and build-only
delivery with **no blocking findings** (`review/final-candidate-review.md`). It
does not approve original #96 closure or waive any remaining gate below.

**Installation, app launch, actual PID/endpoint audio: NOT_RUN.** Previous package
C, its UAT project and the normal profile are not modified by this build. No
merge, tag, release, device routing, volume change or driver installation occurred.
Hosted verification of the resulting delivery commit is tracked separately in
the PR/CI receipts; this package report does not turn pending checks into passes.

## Remaining acceptance

- Original PID renderer-transition loss is not qualified by a packet-pump model.
- Native exact Stop/drain behavior, source isolation and gap/tail completeness
  require deferred audio testing on the new installed candidate.
- Dedicated-endpoint installed audio has its independent gate.
- Existing PID/name selection does not gain process-generation pinning here;
  that source-contract gap stays explicit.
- No merge/tag/release until both actual installed audio modes pass.

## References

- [Microsoft GetNextPacketSize](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getnextpacketsize)
- [Microsoft GetBuffer](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer)
- [Microsoft WaitForSingleObject](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)
- [Historical producer/session evidence](2026-10-25-pid-producer-lifecycle.md)
