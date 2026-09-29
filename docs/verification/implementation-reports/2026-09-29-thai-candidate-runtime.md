---
version: "0.2.0b"
created_at: "2026-09-29T22:55:00+07:00,RWANG"
last_update: "2026-09-29T23:14:00+07:00,RWANG"
status: "local runtime qualification passed; Thai accuracy not qualified"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "implementation-report"
  scope: "Isolated Thai Transformers candidate runtime"
  risk: "HIGH"
---

# Thai candidate runtime implementation report

## Result

The approved Detailed candidate now has an isolated Python 3.11.9 runtime,
hash-locked CPU dependencies, a staged pinned model, manifest-bound readiness,
and candidate-only interpreter selection for Detailed batch work. General,
medium, and live-meeting workers retain the existing operational interpreter.
The candidate runtime and dependency lock remain excluded from Tauri release
resources.

The initial candidate staging attempt failed closed at Python archive hash
validation. The candidate-only pin differed from the downloaded official
Python 3.11.9 archive; its 11,249,023-byte size and MD5 matched the
[Python.org release page](https://www.python.org/downloads/release/python-3119/).
The candidate pin was corrected and staging then completed. The existing
operational staging-script pin was intentionally left unchanged as an
out-of-scope issue.

## Staging evidence

- Candidate interpreter: Python 3.11.9 at
  `.venv-whisper-transformers-candidate/Scripts/python.exe`.
- Direct imports pass at the pinned versions: Torch `2.14.0+cpu`, Transformers
  `4.57.1`, Accelerate `1.10.1`, faster-whisper `1.2.1`, and AV `18.1.0`.
- `uv pip check` reports no incompatible installed packages.
- Candidate lock SHA-256:
  `e32755cf7e634075a4a7abe3edf8bf7e4e73b006e114e0d309e1235a86e33aba`.
- Model revision `b751db1e8dbfee6561de22ca99fe070282fcf459` staged with a
  6,173,655,480-byte checkpoint whose SHA-256 is
  `e1e0b5b4c9a89d7d60fb795448c3102e07af87fa73c5fce7c0206c6bd99a7e7b`.
- The release-resource contract test confirms the candidate runtime and lock
  are not packaged.

## Consolidated source verification

| Check | Result |
|---|---|
| `tests/transcribeTransformersCandidate.test.py` | 2 passed |
| `tests/releaseDistribution.test.mjs` | 8 passed |
| `npm run build` | TypeScript and Vite production build passed |
| `cargo test --lib` | 594 passed, 0 failed, 2 ignored |
| `cargo fmt -- --check` | passed |
| `uv pip check` | passed; no incompatible packages |
| `git diff --check` | passed |

The Rust test run used a transient `TAURI_CONFIG={"bundle":{"resources":[]}}`
override because `.knowledge-parser-runtime` is absent from this isolated
worktree. No tracked Tauri configuration was changed. One ignored test,
`staged_thai_candidate_runtime_smoke_when_audio_is_configured`, runs Detailed
readiness and the Rust-to-Python candidate worker against an explicitly
provided local clip.

That ignored test passed on the staged model: readiness returned available
with `accuracyQualified=false`, and the worker returned valid `WhisperOutput`
with two segments for 30,000 ms of audio. The end-to-end smoke took 106.6
seconds, including the readiness import check and model load.

## Same-audio comparison

Both workers used the same 30-second mono 16 kHz WAV, cut from
`F:\meeting\2026-09-24 15-53-50.mp4` at 60–90 seconds. The candidate used its
local Transformers model and CPU float32; Turbo used the existing local
`large-v3-turbo` model and CPU int8.

| Worker | Elapsed | Segments | Observation |
|---|---:|---:|---|
| Thai candidate | 76.4 s | 2 | Produced continuous Thai text in this clip; possible recognition errors remain. |
| Turbo | 57.7 s | 10 | Faster on this clip; several segments contained mixed-script fragments. |

The candidate took about 1.3× as long as Turbo for this one run. No reference
transcript is available. These timing and qualitative observations do not
establish WER/CER or improved Thai accuracy; readiness continues to report
`accuracyQualified=false`.

## Remaining qualification

Application packaging, clean-install distribution, native UI, independent
review, device/provider tests and release readiness remain outside this local
runtime qualification. The existing operational Python staging script's
archive SHA-256 pin differed from the verified Python.org archive; it was not
changed because that runtime is outside the approved scope.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b → 0.2.0b | 2026-09-29 | local runtime qualification passed; Thai accuracy not qualified | Recorded candidate staging, Rust-worker model load, source tests and one same-audio Turbo CPU comparison with explicit evidence limits. | working-tree | RWANG |
