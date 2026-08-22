---
version: "0.1.2b"
created_at: "2026-08-14T09:54:30+07:00,ATHER"
last_update: "2026-08-14T11:42:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-release"
  doc_type: "technical-design"
  scope: "FUNG Windows x64 Live Meeting public beta"
---

# Desktop Live Meeting Public Release Specification

## Decision request

Approve the first public Windows x64 release of FUNG as version `0.1.0`, with a
self-contained CPU transcription runtime, a GitHub release asset named
`FUNG-windows-x64-setup.exe`, and a production Vercel landing-page download
link that always targets the latest GitHub release.

## Complexity and risk

| Item | Classification |
| --- | --- |
| Complexity | C-3 — Architecture-Driven Implementation |
| Risk | HIGH |
| Reason | Runtime redistribution, installer size, real microphone behavior, unsigned Windows distribution, public tag/release, and production web deployment. |

## Parent and peer alignment

- `docs/Desktop/ARCHITECTURE.md` keeps Desktop as the primary runtime for
  capture, local storage, and model execution.
- `docs/Desktop/GPU_STANDALONE_RUNTIME_SPEC.md` remains authoritative for a
  future GPU artifact. This release does not bundle NVIDIA CUDA/cuDNN files and
  therefore does not claim that gate.
- `docs/WEB_PRODUCTION_DEPLOYMENT.md` keeps Vercel as a static frontend only;
  installers remain GitHub Release assets.
- GenesisBlockDB remains the single local persistence boundary. This release
  does not add cloud persistence or browser capture.
- Phase 4 backup work is excluded from the release branch and artifact.

## Release architecture

```text
GitHub tag v0.1.0
  -> GitHub Release (public beta)
     -> FUNG-windows-x64-setup.exe
        -> FUNG.exe
        -> portable CPython x64 runtime
        -> pinned faster-whisper CPU dependencies
        -> pinned faster-whisper small model
        -> transcribe.py + transcribe_live.py
        -> runtime/license/SHA-256 manifest

Vercel production landing page
  -> https://github.com/Freshair129/FUNG-Releases/releases/latest/download/
     FUNG-windows-x64-setup.exe
```

## Functional requirements

| ID | Requirement |
| --- | --- |
| DLR-01 | The installer must run on supported Windows x64 without system Python, CUDA Toolkit, PyTorch, G-Music, or a pre-existing model cache. |
| DLR-02 | Live Meeting must complete start → microphone capture → local transcript event → stop → durable chunk and transcript rows. |
| DLR-03 | The release must bundle both transcription workers and a pinned local `small` model; normal first use must not download model data. |
| DLR-04 | CPU `int8` is the explicit default for this release. GPU acceleration is out of scope and must not be claimed. |
| DLR-05 | The runtime manifest must record component versions, source URLs/identities, licenses, file sizes, and SHA-256 digests. |
| DLR-06 | Failure to load the runtime/model must produce a visible degraded status while audio capture continues durably. |
| DLR-07 | The GitHub release asset must use the stable name `FUNG-windows-x64-setup.exe`. |
| DLR-08 | The production landing page must show version `0.1.0`, Windows x64 support, local-first behavior, approximate download size, and an unsigned/SmartScreen notice. |
| DLR-09 | The download CTA must resolve from the production Vercel domain to the published GitHub asset with HTTP success. |
| DLR-10 | The release branch must not include the unrelated Phase 4 dirty working-tree changes. |

## Build and packaging contract

1. Add a PowerShell staging script that creates the portable runtime only under
   the release worktree's ignored `.venv-whisper` directory.
2. Pin the Python runtime, `faster-whisper`, transitive package set, and model
   snapshot. Verify every downloaded build input before packaging.
3. Package CPU dependencies only. Do not call `stage_gpu_runtime.ps1` for this
   artifact.
4. Add `transcribe_live.py` and the runtime manifest/license directory to Tauri
   resources.
5. Set the application/package version consistently to `0.1.0`.
6. Build NSIS on Windows and rename the published asset to the stable public
   filename. MSI is optional and not a release gate for this slice.

## Verification gates

| Gate | Exit evidence |
| --- | --- |
| Source regression | Frontend build and focused desktop tests pass; full Rust library suite passes. |
| Runtime isolation | Worker/model probe passes with system Python, user model cache, G-Music, Torch, and CUDA paths excluded. |
| Packaged layout | Installed/copied release resources contain the pinned runtime, model, both workers, manifest, and licenses. |
| Real Live Meeting UAT | A 30–60 second microphone run emits at least one transcript segment, stops cleanly, and preserves chunk/transcript rows. |
| Installer smoke | Install and launch succeed; app window is non-empty; uninstall/reinstall does not require development files. |
| Release integrity | Published asset SHA-256 equals the locally verified installer SHA-256. |
| Web production | Preview passes, production deployment is promoted, the CTA is visible, and the final download URL returns success. |

No tag, GitHub release, or production promotion occurs until all gates above
that can be executed on this Windows host pass. If real speech does not produce
a transcript, the task remains blocked and no release claim is made.

## Verification record

| Gate | Result before publication |
| --- | --- |
| Source regression | Frontend/release/Python suites passed; controlled Rust library rerun passed `197/197`. |
| Runtime isolation | Pinned portable CPU `int8` runtime and local `small` model probe passed. |
| Real Live Meeting UAT | 30.082 seconds on real mic plus system loopback produced 8 durable chunks and 10 transcript segments. |
| Installer smoke | Final NSIS installed, launched a non-empty `FUNG` window, and its installed runtime transcribed the speech fixture at 0.9988 confidence. |
| Release integrity | Final local asset is 515,089,576 bytes with SHA-256 `f67a78e0b216628d19335646342eea20575d5c7b5a16cccf7daf32c6b780d414`; remote equality remains a post-upload gate. |
| Web preview | Production-env local browser rendered version, 491 MB size, SmartScreen disclosure, and stable release URL with zero console errors. |

## Release and rollback

- Commit only release-scoped files on `codex/desktop-live-release`.
- Merge the verified release commit into `main` before tagging `v0.1.0`; the tag
  must point to the exact released source commit.
- Publish release notes identifying the artifact as a public beta and disclose
  that the Windows installer is not code-signed, so SmartScreen may warn.
- Deploy and verify a Vercel preview before production promotion.
- Rollback web by promoting the prior Vercel deployment. Rollback binary
  distribution by marking the GitHub release withdrawn and removing the
  landing CTA; do not silently repoint the same tag to different source.

## Out of scope

- Browser microphone capture or browser-side transcription.
- Mobile release or playlist/tag updates for mobile.
- GPU/CUDA/cuDNN redistribution and GPU performance claims.
- Automatic in-app updater and updater signing keys.
- Windows Authenticode signing certificate procurement.
- Phase 4 backup/cloud-account changes.

## Approval gate

Approval authorizes implementation, scoped commit/push/merge, tag `v0.1.0`,
GitHub Release publication, and Vercel production promotion only after the
verification gates pass. It also accepts an explicitly disclosed unsigned
public-beta installer for this release.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.2b | Moved public binary distribution to the public `FUNG-Releases` repository because private source-repository assets require authentication. |
| 0.1.1b | Recorded approval and the completed pre-publication verification gates. |
| 0.1.0b | Initial CPU-baseline Live Meeting packaging, release, and website-download contract. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.2b | 2026-08-14 | beta | Cut public Desktop downloads over to the binary-only release repository and retained the stable latest-asset contract. | pending | ATHER |
| 0.1.1b | 2026-08-14 | beta | Approved specification with local release, hardware, installer, and web-preview evidence. | pending | ATHER |
| 0.1.0b | 2026-08-14 | candidate | Proposed the first self-contained Windows Live Meeting public beta and latest-release download flow. | pending | ATHER |
