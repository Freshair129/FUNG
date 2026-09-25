---
version: "0.1.0b"
created_at: "2026-09-24T00:06:35+07:00,RWANG,2c2559f"
last_update: "2026-09-24T00:06:35+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "runtime-diagnostics"
  doc_type: "rca"
  scope: "FUNG staged Whisper import under restricted execution"
---

# Whisper import: execution boundary, not a confirmed broken installation

## Symptom

The restricted-shell probe raises `ImportError: cannot import name
'WhisperModel' from 'faster_whisper' (unknown location)`.

## Evidence

- `find_spec` under the restricted shell returns `origin=null` for
  faster_whisper, huggingface_hub and ctranslate2 with site-packages locations.
- `Get-Item` on their `__init__.py` returns Access denied in that context.
- The same staged interpreter outside the sandbox imports WhisperModel and
  HfApi successfully, reports Python 3.11.9 / faster-whisper 1.2.1 and one CUDA
  device. No installation repair was performed between the two probes.
- Both contexts find only the existing small model before staging; a separate
  host-level probe confirms turbo/medium and root runtime manifest absent.

## Root Cause

The observed import failure is caused by the restricted execution context's
file-access boundary. Python sees package directories but cannot resolve their
initialization files. Host import success rules out the need to reinstall this
runtime for this symptom. The underlying OS ACL or sandbox implementation was
not changed or audited; this conclusion is limited to the compared probes.

## Why the issue escaped detection

An import-only failure can look like a missing/broken dependency when the
execution identity is omitted. Historical successful imports were performed
in a different execution context. Missing operational models are a separate
prerequisite and must not be confused with this import symptom.

## Proposed prevention

Record execution context with import/model evidence. On this exact error,
inspect module origin and file readability, then compare a permitted host
read-only probe before reinstalling. Keep runtime preservation and model
staging separate. Do not widen filesystem ACLs as a diagnostic workaround.

## Separate test-runner mismatch found during qualification

The existing `transcribeConcatOnly.test.py` suite injects a fake faster_whisper
package through child-process `PYTHONPATH`. Running that suite with the
production embedded interpreter produced 5 passes / 1 failure: the fake was
not loaded, the real model attempted a default Hugging Face download, and
decoding subsequently failed on the deliberately nonexistent `first.wav`.
The embedded `python311._pth` configuration isolates environment search paths.
This is a test-runner mismatch, distinct from the sandbox import problem.

The unchanged suite passed 6/6 under the bundled Python 3.12.14 test runtime,
whose `ignore_environment=0` and `isolated=0`, with Hugging Face offline mode
enabled. Production worker probes still use the embedded Python and real
local models. Prevention: use the ordinary test interpreter for PYTHONPATH
mock suites and local pinned paths/offline mode for real-runtime probes.
The first run's Hugging Face cache was left intact; no unrelated cache cleanup
was performed. This failure is retained as evidence, not reported as a pass.

## Version Diff

No document → 0.1.0b: documented the observed execution boundary; no code fix.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-09-24 | beta | Compared restricted and host imports without modifying dependencies | base 2c2559f; working-tree | RWANG |
