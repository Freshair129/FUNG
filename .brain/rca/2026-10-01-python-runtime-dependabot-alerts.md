# RCA: 37 open Python runtime Dependabot alerts

**Date:** 2026-10-01
**Risk:** HIGH
**Status:** Remediation in progress
**Affected scope:** PDF knowledge parser, opt-in Transformers Thai candidate, optional pyannote diarization

## Symptom

GitHub reported 37 open runtime Dependabot alerts on the default branch: 23 for `pypdf`, five for the Transformers/Accelerate candidate runtime, and nine for Torch in the diarization runtime.

## Evidence

- Current pins were `pypdf==6.10.0`, `transformers==4.57.1`, `accelerate==1.10.1`, `pyannote.audio==3.4.0`, `torch==2.4.1`, and `torchaudio==2.4.1`.
- The affected files are three independent runtime requirement sets plus stage scripts, manifest checks, Rust readiness/provenance checks, and packaging/worker tests.
- The diarization contract pins the 2.4.1 Torch/TorchAudio pair because pyannote 3.4 consumes TorchAudio `AudioMetaData`; TorchAudio 2.9 removed that API.
- The Accelerate GHSA lists no first patched version and concerns untrusted sharded checkpoint index paths. FUNG's approved staged Thai model is one offline, pinned `pytorch_model.bin`; the stage allowlist excludes indexes, but the worker had no explicit rejection if an index were later added.
- Existing npm security checks do not audit the Python runtime lockfiles.

## Root Cause

The Python runtimes are intentionally exact-pinned and synchronized across lockfiles, stage scripts, manifests, Rust checks, and tests. Security advisories advanced after those pins were accepted, and no Python lockfile audit or update gate alerted the project before GitHub Dependabot did. The diarization lock's dependency pin also became coupled to an obsolete TorchAudio API, so a mechanical Torch bump would break the optional diarization path.

## Why the issue escaped detection

Tests verified that runtime packages matched reviewed pins and that staging contracts stayed synchronized. They did not compare those pins against vulnerability advisories. The existing TypeScript `npm audit` check covers the JavaScript dependency graph only. The lockfile generator also treats Torch, TorchAudio, and pyannote as one compatibility set, making blind one-package bumps unsafe.

## Proposed prevention

- Keep exact locks and their stage/readiness/version guards synchronized in one reviewed change.
- Add a recurring or CI `pip-audit` check over the three runtime requirement files, and surface advisories against generated hash locks.
- Keep model trust explicit: only the approved local checkpoint format is accepted, and any future sharded or external model requires a separate security review.
- Keep a real gated-model diarization smoke as a distinct acceptance gate; dependency imports must not be promoted to model/runtime acceptance.

## Remediation and verification

See `docs/specs/2026-10-01-python-runtime-dependabot-remediation.md` for the approved migration contract, exact dependency targets, tests, and evidence boundaries. The 3.11.9 embedded parser runtime staged successfully with `pypdf==6.16.1`; its isolated import probe passed. Local checks passed: `npm run build`, Rust tests (594 passed, 2 ignored), candidate tests (4/4), diarization tests (8/8 Node and 3/3 Python), release tests (9/9), and PDF parser tests (7/7). `cargo fmt --check` and `git diff --check` passed. `pip-audit` found no known issues in the three lock groups; its PyPI service skipped the local `torch==2.14.0+cpu` build. Full Transformers checkpoint inference was not run because free RAM was below the 12 GiB qualification threshold, and gated pyannote model inference was not run because the model was absent from the local cache. Those are model-qualification limits, not dependency test failures.

## Additional staging finding: embedded Python archive digest

**Symptom:** The operational Whisper staging script rejected the CPython 3.11.9 x64 embedded archive fetched from its configured `python.org` URL.

**Evidence:** The downloaded archive's SHA-256 is `009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b`; its MD5 matches the Python 3.11.9 release page. The candidate staging script and Rust readiness constant already used that SHA-256, while `stage_whisper_runtime.ps1` had the transposed value `...60e0f0e305...`.

**Root Cause:** One character-order typo in the operational staging SHA-256 constant made the official archive fail closed at integrity verification.

**Why it escaped detection:** Release tests checked that the operational stage script contained a SHA-256 check, but did not assert its value. The exact archive digest was asserted only for candidate staging and Rust readiness.

**Proposed prevention:** Keep the operational stage digest assertion alongside the candidate/Rust digest checks, and verify the actual archive before staging the local parser runtime.

## Additional staging finding: CUDA wheel index

**Symptom:** CUDA diarization lock generation and wheel download selected the PyTorch `cu130` index, but the later installation command did not receive that index argument.

**Evidence:** `stage_diarization_runtime.ps1` passes `@indexArgs` to lock resolution and wheel download, but the install invocation omitted it. The default CPU lock and CPU install path were exercised; a CUDA runtime was not staged in this CPU-only verification environment.

**Root Cause:** The custom index argument was not threaded through from dependency resolution to the hash-locked install phase.

**Why it escaped detection:** Packaging tests verified hash completeness and the CPU runtime pins, but did not assert that the install command used the configured variant index.

**Proposed prevention:** Pass `@indexArgs` to the hash-locked install command and assert the complete install invocation in the diarization packaging test. CUDA staging remains unverified until run on a matching CUDA host.

**Verification:** The install command now passes `@indexArgs`; `npm run test:diarization` passed (8 Node, 3 Python) and the modified PowerShell file parses. This verifies command wiring, not a real cu130 wheel installation or GPU inference.
