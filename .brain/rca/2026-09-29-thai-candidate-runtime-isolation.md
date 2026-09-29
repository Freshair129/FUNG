---
version: "0.2.3b"
created_at: "2026-09-29T21:39:00+07:00,RWANG"
last_update: "2026-09-29T23:25:00+07:00,RWANG"
status: "runtime qualification passed; stale egress test corrected; hosted CI rerun pending"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "root-cause-analysis"
  scope: "Detailed Thai Transformers candidate runtime qualification"
  language: "Thai"
  risk: "HIGH"
---

# RCA — Thai candidate runtime is not isolated from the operational Whisper runtime

## Symptom

The `ละเอียด` workflow has a candidate worker and model-staging script, but its
runtime cannot yet be qualified through the Python interpreter used by FUNG.
Detailed readiness and the worker both use the existing `WhisperRuntime.python`
interpreter. That runtime currently lacks the candidate's Transformers
dependencies. A dependency overlay supplied through `PYTHONPATH` is not visible
to it.

## Evidence

- The application runtime is
  `C:\Users\pc\workspace\fung\.venv-whisper\Scripts\python.exe`. It reports
  Python isolation flags `isolated=1` and `ignore_environment=1`; its import
  path is limited to the embedded runtime's zip, Scripts and `Lib/site-packages`.
- The operational runtime imports Torch `2.4.1+cpu`. With
  `PYTHONPATH=.candidate-transformers-gpu-deps`, the same interpreter fails
  with `ModuleNotFoundError: No module named 'transformers'` because it ignores
  environment-based path injection.
- A test-only explicit `sys.path` insertion imports the isolated candidate
  dependencies as Torch `2.14.0+cpu` and Transformers `4.57.1`. The prior GPU
  comparison used a separate Torch `2.14.0+cu130` dependency overlay and a
  low-CPU-memory loading harness; it did not run through FUNG's bundled worker
  runtime.
- At discovery, `detailed_transcription_readiness_for_job` and
  `run_detailed_candidate_worker` used the shared operational interpreter, and
  the initial candidate stage depended on that same runtime.
- The first candidate staging attempt stopped at Python archive hash
  validation. The candidate pin copied from the operational staging script did
  not match the downloaded 11,249,023-byte Python 3.11.9 archive. Its MD5
  matched the official [Python 3.11.9 release page](https://www.python.org/downloads/release/python-3119/).
  The candidate-only SHA-256 pin was corrected to the computed archive digest;
  the operational staging script remains unchanged and out of scope.
- The first hosted frontend CI run failed at the offline-transcription source
  assertion. The shared Rust worker runner still sets both
  `HF_HUB_OFFLINE=1` and `TRANSFORMERS_OFFLINE=1` in its `None` cache arm;
  local `npm run test:egress` reproduced the test failure before correction.
- The Tauri bundle currently includes `.venv-whisper` and the candidate worker
  scripts, but not a candidate Python environment or candidate model directory.
- The pinned model checkpoint is 6,173,655,480 bytes. The temporary GPU
  dependency overlay measured 3,421,544,814 bytes. These are qualification
  artifacts and not an approved packaged-runtime size or distribution plan.

## Root Cause

The candidate backend has a separate model path but not a separate Python
runtime. FUNG's embedded operational interpreter is isolated from environment
path injection, so candidate-only Transformers/PyTorch dependencies cannot be
made available to the candidate worker without changing the shared runtime.
The temporary benchmark bypassed this boundary with its own dependency/loading
harness, so it proves comparative local inference only, not FUNG runtime
readiness.
The subsequent CI failure had a separate, test-only root cause: the offline
`match hf_home` branch had moved from `run_python_worker` into
`run_python_worker_with_interpreter`, but the egress test still sliced source
around the old location and therefore inspected an empty branch.

## Why the issue escaped detection

Before the approved isolated runtime was added, source and fixture tests
covered candidate routing, readiness contracts and worker output shape but did
not launch the candidate worker with a fully staged interpreter and model. The
A/B harness loaded the model with temporary dependencies and a low-memory shim.
The first stage attempt also did not have an archive-hash test against the
Python.org artifact, so the copied malformed pin was discovered only when
staging executed. The first hosted frontend run exposed that the egress source
test had not been updated with the shared-worker refactor.

## Implemented fix and prevention

The approved model-profile contract now requires a dedicated, opt-in Python
3.11.9 candidate runtime and hash-locked CPU dependencies beside the pinned
model and manifest. The staging script builds that root transactionally and
records Python, lock, dependency and model-file provenance. Readiness and
Detailed batch jobs resolve and use the candidate interpreter directly,
compare its dependency versions and lock hash, and remain offline/fail-closed.
The candidate model load uses `low_cpu_mem_usage=True`. General/medium and live
meeting transcription continue using the operational interpreter. The
candidate runtime remains excluded from the installer. Candidate staging,
dependency-import checks, and the offline Rust readiness/worker smoke pass.
The candidate worker returned valid `WhisperOutput` for a 30-second clip. The
candidate/Turbo CPU comparison took 76.4/57.7 seconds and looked more
continuous for the candidate on this one clip; without a reference transcript,
Thai accuracy remains unqualified. The source campaign passed 594 Rust tests
(2 ignored), 2 Python tests, 8 release/resource tests, the frontend build,
dependency check, formatting and diff checks. The original operational archive
pin was not changed; its mismatch remains an out-of-scope follow-up.

Prevention now includes an executable staging-time archive digest check and a
release contract test that pins the candidate archive digest in both the
staging script and Rust readiness. Keep the operational runtime pin separate
until that runtime is reviewed under its own scope.
The egress test now inspects the shared worker helper and verifies both offline
flags; its focused local suite passes 8/8. Hosted CI is being rerun.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.2.2b → 0.2.3b | 2026-09-29 | stale egress assertion fixed; hosted CI pending | Updated the source contract to inspect the shared worker helper and verify both offline flags after the Rust refactor. | working-tree | RWANG |
| 0.2.1b → 0.2.2b | 2026-09-29 | Rust-worker runtime passed; accuracy unqualified | Verified pinned readiness, offline candidate inference, valid worker output and one same-audio Turbo CPU comparison. | working-tree | RWANG |
| 0.2.0b → 0.2.1b | 2026-09-29 | staged; source validation passed; model load pending | Corrected the candidate-only Python archive pin, staged the hash-locked runtime/model and passed source tests; production model load and same-audio comparison remain open. | working-tree | RWANG |
| 0.1.0b → 0.2.0b | 2026-09-29 | fix implemented; validation pending | Added the isolated candidate runtime, lock/manifest checks and candidate-only Detailed batch interpreter; full local qualification remains pending. | working-tree | RWANG |
| 0.1.0b | 2026-09-29 | candidate | Documented the isolated-Python boundary; no source or runtime files changed. | working-tree | RWANG |
