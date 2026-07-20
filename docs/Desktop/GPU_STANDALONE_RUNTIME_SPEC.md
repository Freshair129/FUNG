---
version: "0.1.1b"
created_at: "2026-07-19T00:00:00+07:00,ATHER"
last_update: "2026-07-19T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "runtime-packaging-spec"
  scope: "FUNG"
---

# GPU Standalone Runtime Specification

## Decision

FUNG's packaged Windows desktop build must run faster-whisper on a CUDA-capable NVIDIA GPU without loading CUDA libraries from another product, development checkout, or user-configured `PATH` entry.

This specification applies to the packaged application. NVIDIA display drivers remain a host prerequisite and are not bundled.

## Complexity and Risk

| Item | Classification |
| --- | --- |
| Execution complexity | C-3 — Architecture-Driven Implementation |
| Change risk | High |
| Reason | Changes affect packaged-runtime layout, native DLL loading, Python worker launch, GPU capability diagnostics, and release verification. |

## Evidence and Root Cause

### Symptom

GPU transcription succeeds only when FUNG inherits a `PATH` that exposes CUDA 12 DLLs from `D:\G-Music\...\torch\lib`. Without that path, Windows reports a missing `cublas64_12.dll`.

### Evidence

- The transcription worker uses `faster-whisper`.
- The Rust worker launch currently invokes the Python executable without prepending a FUNG-owned CUDA runtime directory to the child process environment.
- The current runtime path is source-checkout-relative (`.venv-whisper`), not packaged-app-relative.

### Root Cause

FUNG does not yet own or resolve the CUDA/cuDNN runtime required by its GPU worker. DLL discovery therefore falls through to inherited process environment paths, which makes G-Music an undeclared runtime dependency.

### Why It Escaped Detection

Existing validation was performed on a workstation where the G-Music Torch library directory was already reachable through the environment. No isolated packaged-build smoke test excluded that path.

### Prevention

Package the compatible native runtime, resolve it from the FUNG install directory for each worker process, and gate release on a clean-room GPU smoke test.

## Runtime Ownership Contract

### Packaged Layout

The release bundle must contain FUNG-owned equivalents of the following paths:

```text
FUNG/
  .venv-whisper/
    Scripts/python.exe
    Lib/site-packages/
  runtime/
    cuda12/
      bin/
        cudart64_12.dll
        cublas64_12.dll
        cublasLt64_12.dll
        cudnn*.dll
    manifest.json
```

`manifest.json` must record the exact package/version/build source for Python, faster-whisper/CTranslate2, CUDA runtime, cuDNN, and the files expected by the launcher. It must also record license or redistribution evidence for the bundled NVIDIA components.

### Host Prerequisites

- Windows version supported by the FUNG release.
- NVIDIA display driver compatible with the bundled CUDA 12 runtime.
- An NVIDIA GPU when the `gpu` execution profile is selected.

FUNG must not require a CUDA Toolkit installation, PyTorch installation, G-Music installation, or a development checkout.

## Runtime Resolution Contract

1. Resolve the installed application/resource directory at runtime; do not use compile-time source paths for a packaged launch.
2. Resolve Python, worker script, and `runtime/cuda12/bin` from that application directory.
3. Before spawning Whisper, set the child process `PATH` to `FUNG CUDA runtime directory + existing PATH`; do not mutate the parent process environment.
4. Verify the required DLL manifest before launch and return a diagnostic that identifies each missing file.
5. Start the selected execution profile explicitly:
   - `gpu`: invoke faster-whisper with `--device cuda` and a compatible GPU compute type.
   - `cpu`: invoke a documented CPU configuration.
6. If the requested GPU profile is unavailable, fail with an actionable diagnostic. CPU fallback requires an explicit user or caller choice and must never occur silently.

## Diagnostics Contract

Runtime diagnostics must report:

- requested and effective execution profile;
- FUNG-owned CUDA runtime directory used for the worker;
- required DLL presence check result;
- NVIDIA GPU/CUDA provider availability;
- clear distinction between missing FUNG files, missing/unsupported host driver, and model/runtime execution failure.

Diagnostics must not disclose unrelated absolute paths from the host environment in end-user output.

## Acceptance Criteria

| ID | Requirement |
| --- | --- |
| AC-1 | Packaged FUNG includes its Python worker, compatible CUDA 12 runtime (`cudart`, `cuBLAS`, `cuBLASLt`), compatible cuDNN, and a version/license manifest. |
| AC-2 | The FUNG launcher resolves all worker paths from the installed application location. |
| AC-3 | The child Whisper process receives a `PATH` beginning with FUNG's runtime directory. |
| AC-4 | GPU mode explicitly uses CUDA; CPU mode is explicit and never a silent fallback. |
| AC-5 | Removing G-Music from the test machine or process `PATH` does not prevent GPU transcription. |
| AC-6 | A missing bundled DLL, incompatible driver, or absent GPU produces a distinct actionable error. |
| AC-7 | Release evidence records a clean-room GPU smoke-test result and the exact runtime manifest. |

## Clean-Room GPU Smoke Test

1. Build the FUNG release bundle.
2. Copy only the bundle to a temporary test directory on a GPU-capable machine.
3. Launch with a process environment whose `PATH` excludes `D:\G-Music`, other Python/Torch installations, and CUDA Toolkit `bin` directories.
4. Confirm runtime diagnostics name only the FUNG-owned CUDA directory.
5. Run a short known audio fixture with the explicit `gpu` profile.
6. Verify a successful transcript and GPU/CUDA provider evidence.
7. Repeat with the FUNG CUDA runtime deliberately unavailable; verify the expected FUNG diagnostic rather than an accidental dependency resolution.

The smoke test must preserve the command, sanitized environment evidence, runtime manifest, application version, driver version, GPU model, fixture checksum, exit status, and transcript checksum.

## Out of Scope

- Bundling NVIDIA display drivers.
- Supporting non-NVIDIA GPU acceleration in this change.
- Changing transcription quality, model selection, or UI design beyond exposing the required explicit runtime profile and diagnostics.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.1b | Corrected the packaged Python layout to match the implemented `.venv-whisper` resource. |
| 0.1.0b | Initial standalone CUDA 12/cuDNN runtime, launcher, diagnostic, and clean-room GPU smoke-test contract. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.1b | 2026-07-19 | beta | Corrected packaged Python runtime layout. | N/A | ATHER |
| 0.1.0b | 2026-07-19 | beta | Added standalone GPU runtime packaging specification. | N/A | ATHER |
