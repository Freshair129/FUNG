---
version: "0.1.0b"
created_at: "2026-08-10T00:00:00+07:00,Agent: ATHER"
last_update: "2026-08-10T00:00:00+07:00,Agent: ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: Desktop Tauri UI starts blank during Call.md reference validation

## Symptom

The detached Desktop UI candidate launched successfully, but the Tauri window showed a black/blank surface and no React app content. The same candidate passed build and Rust tests.

## Evidence

- UI candidate branch: `feature/phase-3-byom-cloud-keys`, detached test HEAD `4fcc264`.
- `npm run build` passed with 1,747 modules.
- `cargo test -j 1 --manifest-path src-tauri/Cargo.toml` passed with 171/171 tests.
- WebView2 inspection showed `<div id="root"></div>` after startup.
- Dynamic import of `src/main.tsx` initially failed with `Error: supabaseUrl is required`, then `Error: supabaseKey is required` after only `VITE_SUPABASE_URL` was present in the local `.env`.
- `src/lib/supabase.ts` constructs the client at module load and requires both Vite variables.
- `src/main.tsx` assigns `body[data-surface="landing"]` for `/` before the Tauri runtime is given precedence.
- `src/landing/landing.css` sets `body[data-surface="landing"] #root` to `height: auto`; the `.app-shell { height: 100% }` then resolves to a zero-height layout and its content is offscreen.
- With test-only placeholder Supabase variables and an ephemeral DOM surface override to `app`, the real Tauri WebView rendered the FUNG-native P1/P2/P3 Meeting Mode flow. No recording, provider call, or external Supabase call was performed.

## Root Cause

Two startup contracts fail independently:

1. Supabase is an eager module-level dependency for the Desktop bootstrap. Missing `VITE_SUPABASE_ANON_KEY` aborts React initialization before the root can render.
2. The route-based landing-surface rule wins for the Tauri `/` route. Landing CSS therefore collapses the application root even when the native app is otherwise mounted.

The Call.md material is not the runtime source of this defect. It is a visual/interaction reference; the candidate remains FUNG-native and should be integrated selectively after the startup contract is fixed.

## Why The Issue Escaped Detection

Build and Rust tests do not execute the real WebView bootstrap with the workstation `.env`. Existing checks also did not assert that a Tauri launch without Supabase credentials still renders a non-zero root/app-shell layout. The candidate UI was validated primarily through source-level and test-suite evidence, not a clean runtime smoke path.

## Proposed Prevention

- Make the Desktop bootstrap independent of eager Supabase credentials: lazy-load the client or render a bounded unavailable state when credentials are absent.
- Give `isTauriRuntime` precedence over the `/` route when assigning `body[data-surface]`.
- Add a Tauri startup smoke test with missing Supabase variables that asserts `#root` and `.app-shell` have positive dimensions and visible app text.
- Keep the Call.md reference boundary explicit: copy validated information architecture and interaction patterns into FUNG-native components; do not merge the reference branch wholesale.
- Run the smoke test on the exact candidate commit before selective integration into the main branch.

## Validation Boundary

This RCA records diagnosis only. No source fix, branch merge, provider call, recording, or real Supabase request was performed in this session. The temporary detached test worktree `D:\FUNG-wt-callmd-ui-test` remains on disk with no running process because junction cleanup was not safe to force through the shell; it is not part of the commit.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-08-10 | beta | Recorded reproducible Desktop blank-screen causes and safe prevention gates. | pending | ATHER |
