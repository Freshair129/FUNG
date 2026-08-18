---
version: "0.1.0b"
created_at: "2026-08-14T09:54:30+07:00,ATHER"
last_update: "2026-08-14T09:54:30+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-live-meeting"
  doc_type: "root-cause-analysis"
  scope: "Windows public release runtime"
---

# RCA — Packaged Live Meeting cannot transcribe on a clean Windows machine

## Symptom

The source application can route microphone capture into Live Meeting, but the
current repository cannot produce a Windows installer that is proven to start
the live transcription worker on a machine without the development Python and
model cache.

## Evidence

1. `src-tauri/tauri.conf.json` bundles `.venv-whisper`, `runtime`, and
   `scripts/transcribe.py`, but does not bundle `scripts/transcribe_live.py`.
2. `LiveWorker::spawn` resolves `transcribe_live.py` beside
   `transcribe.py`; the packaged resource is therefore absent even when the
   Python interpreter exists.
3. The local `.venv-whisper` directory is empty and system Python cannot import
   `faster_whisper` in the current workspace.
4. `WhisperRuntime` expects `.venv-whisper/Scripts/python.exe`. A normal Windows
   virtual environment can retain a dependency on the build machine's base
   Python and is not, by itself, clean-machine release evidence.
5. The worker defaults to model name `small`; without a packaged model, its
   first start may download model data during the 180-second ready window.
6. The existing GPU specification requires redistribution evidence for bundled
   NVIDIA components; that release gate is still open.

## Root Cause

The development runtime layout was reused as the release layout without a
complete, reproducible runtime contract. The bundle declaration omits the live
worker, does not prove Python portability, and does not own a pinned local model.
Consequently the application code can capture audio while a public installer
cannot guarantee live transcription.

## Why the issue escaped detection

Previous smoke evidence ran from a workstation that already had development
runtime dependencies and model/cache history. The release-layout test copied
files on the same machine; it did not validate an installer against a runtime
with system Python, CUDA paths, and model caches excluded.

## Proposed prevention

- Build a pinned, FUNG-owned portable CPU runtime from reproducible inputs.
- Bundle `transcribe.py`, `transcribe_live.py`, runtime packages, model files,
  licenses, and a SHA-256 manifest.
- Default the first public Windows release to explicit CPU mode; retain GPU as
  a later separately approved artifact after redistribution and clean-GPU gates.
- Add an automated release-layout probe and a real microphone UAT before tag.
- Make the public website download a stable asset name from the latest GitHub
  release, then verify the production URL after deployment.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Initial evidence-backed RCA for the clean-machine Live Meeting release failure. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-08-14 | candidate | Documented the incomplete worker/runtime/model release layout and prevention gates. | pending | ATHER |
