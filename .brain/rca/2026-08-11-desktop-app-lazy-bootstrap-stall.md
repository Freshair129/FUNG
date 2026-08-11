---
version: "0.1.1b"
created_at: "2026-08-11T09:31:00+07:00,Agent: ATHER"
last_update: "2026-08-11T11:39:19+07:00,Agent: ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  scope: "D:\\FUNG"
  doc_type: "rca"
---

# RCA: Desktop remains on the FUNG bootstrap fallback

## Symptom

The FUNG taskbar icon and native window appeared, but the window remained on the centered message `กำลังเปิด FUNG…` instead of rendering the Desktop UI.

## Evidence

- `fung.exe` remained responsive with `MainWindowTitle=FUNG` and a non-zero native window handle.
- The WebView renderer process existed under `fung.exe` and displayed the React `Suspense` fallback from `src/main.tsx`.
- Vite remained live on IPv6 loopback `::1:1420`; the earlier `127.0.0.1` failure was a probe-address false negative, not a stopped server.
- Requests through `http://localhost:1420` returned `200`: `/` in 101 ms, `/src/main.tsx` in 22 ms and `/src/App.tsx` in 218 ms.
- The Desktop `App` had been moved behind `React.lazy` while fixing the independent eager-Supabase startup defect. Tauri therefore depended on this new async boundary before any application UI could render.

## Root Cause

The critical Tauri Desktop route was unnecessarily placed behind a `React.lazy` boundary. In the real WebView2 runtime that boundary did not settle, so the root stayed on its `Suspense` fallback even though the Vite endpoint and `App.tsx` module were reachable. Supabase-dependent account modules were the only modules that needed delayed loading; delaying the whole Desktop app widened the failure boundary beyond the original RCA requirement.

## Why The Issue Escaped Detection

The unit contract asserted that the Desktop app was lazy, and the production build proved only that chunks could be emitted. Neither gate asserted that the critical Tauri route renders synchronously or that the bootstrap fallback disappears in the real WebView.

## Prevention

- Keep the core Desktop `App` as a static bootstrap dependency.
- Keep only Supabase-dependent account and pairing panels behind lazy loading.
- Assert both conditions in `tests/desktopBootstrap.test.mjs`.
- Treat a bootstrap fallback lasting more than a few seconds as a failed startup gate and verify the real WebView before completion.

## Resolution

- `src/main.tsx` now imports `App` statically while web/mobile/auth routes remain lazy.
- `src/App.tsx` keeps `AccountLoginPanel` and `DevicePairingPanel` lazy and renders a bounded unavailable state when Supabase is not configured.
- The Desktop bootstrap regression suite was changed from requiring a lazy `App` to forbidding that critical async boundary.

## Verification

- `npm run test:desktop-bootstrap` — 5 passed, 0 failed.
- `npm run test:auth` — 5 passed, 0 failed.
- `npm run test:mobile` — 4 passed, 0 failed.
- `npm run build` — passed; 1,748 modules transformed.
- Rebuilt and restarted runtime: `fung.exe` PID 33780 remained responsive beyond 46 seconds with `MainWindowTitle=FUNG` and a non-zero native window handle; Vite listens on `::1:1420`.
- Captured runtime output contains only the normal Mark IX instant-load sequence; stderr is empty.
- Windows UI Automation observed the rendered `RootWebArea`, `FUNG review workspace`, P1–P4 anchor rail, transcript content, and action controls. The bootstrap fallback is no longer the visible document.
- Windows.Graphics.Capture could not add its optional capture border on this Windows build (`0x80004002`), so the runtime evidence is the live accessibility tree rather than a saved screenshot.

## Version Diff

`0.1.0b -> 0.1.1b`: closed the operator-visible startup gate with a rebuilt responsive window and live UI Automation evidence.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.1b | 2026-08-11 | beta | Verified that the rebuilt Tauri window renders the full review workspace instead of the bootstrap fallback. | pending | ATHER |
| 0.1.0b | 2026-08-11 | beta | Removed the unnecessary lazy boundary from the critical Desktop bootstrap path. | pending | ATHER |
