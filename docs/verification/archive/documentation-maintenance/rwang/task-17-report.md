---
version: "0.1.1b"
created_at: "2026-08-12T07:00:00+07:00,ATHER"
last_update: "2026-08-12T08:00:00+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "phase3-regression"
  doc_type: "verification-report"
  scope: "codex/phase3-integration sync candidate on origin/main"
---

# T17 — Phase 3 candidate regression and runtime hardening

## Verdict

This rerun is on the post-merge `origin/main` base and includes the Live
Meeting test surface that is absent from the earlier 186-test candidate.

**PASS — automated candidate gates green.** The candidate remains unmerged and
Controller-gated; this report does not claim real provider, physical-device,
merge, post-merge CI or release acceptance.

## Root-cause closure

The first standard Rust run exposed a test-runtime gap rather than a Phase 3
policy failure: Windows exposed Python through `py.exe`, while
`resolve_test_python` searched only `python`/`python3`; the fallback worktree
venv junction was empty. Six worker tests therefore failed before the fixture
could run. After adding `py` discovery, the concat-only test exposed that the
real worker imported `faster_whisper` even when no model was needed. The worker
now lazily imports the model and uses a fail-closed stdlib WAV path only when
the decoder package is unavailable; non-WAV decoding still requires the
approved decoder.

RCA: `.brain/rca/2026-08-12-rust-test-python-launcher-gap.md`.

## Verification matrix

| Gate | Command | Result |
|---|---|---|
| Rust full regression | `cargo test -j 1 --manifest-path src-tauri/Cargo.toml` | **195 passed, 0 failed** |
| Python concat contract | `py -3 tests/transcribeConcatOnly.test.py` | **5 passed, 0 failed** |
| Mobile capture suite | `npm run test:mobile` | **4 passed, 0 failed** |
| Auth flow | `npm run test:auth` | **5 passed, 0 failed** |
| Design system | `npm run test:design-system` and `npm run design-system:check` | **2 passed; check passed** |
| Frontend build | `npm run build` | **TypeScript/Vite passed** |

## Scope and remaining gates

- Changed only test-runtime discovery, concat-only import/decode behavior,
  dated evidence ledgers and the RCA.
- No key, endpoint, provider body, transcript, provenance or policy-storage
  boundary was widened.
- Real Windows keyring save/status/delete, paired mobile/Desktop OpenAI STT
  Local/Cloud with reconnect/restart/revocation, real Anthropic fallback,
  physical Android/Desktop UAT, merge authorization, post-merge CI and
  canonical graph/preflight truth-sync remain **OPEN**.

## Version Diff

| Version | Change |
|---|---|
| 0.1.0b | Recorded the Phase 3 candidate regression matrix and closed the Windows test-runtime gaps. |
| 0.1.1b | Re-ran the regression matrix on the post-merge `origin/main` base: Rust 195/195, Python 5/5, mobile 4/4, auth 5/5, design-system 2/2 and Vite build pass. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-08-12 | candidate | Post-merge-base regression passed and the scoped follow-up merged in PR #10; external/provider/device gates remain open. | `cea2d93` | ATHER |
| 0.1.0b | 2026-08-12 | candidate | Full local regression passed after test-runtime hardening; external/controller gates remain open. | same commit | ATHER |
