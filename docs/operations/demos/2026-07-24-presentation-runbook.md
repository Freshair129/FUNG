---
version: "0.1.0b"
created_at: "2026-07-24T18:10:00+07:00,ATHER"
last_update: "2026-07-24T18:10:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "presentation-readiness"
  doc_type: "technical-design"
  scope: "FUNG launch and demo runbook"
  language: "Thai-first"
---

# FUNG Presentation Runbook

## Goal

Use one web surface to present the product story, then move into the live app or desktop demo without changing the narrative.

## What Is Ready Now

- Public landing page on Vercel for the story and product framing.
- Web app at `/app` for product-surface walkthrough.
- Desktop installer artifacts already present on this machine.
- Frontend production build passes.
- Desktop build prerequisites pass.

## Verified Assets On This Machine

- Production web: `https://fung-seven.vercel.app`
- Product app: `https://fung-seven.vercel.app/app`
- Desktop installer: `D:\FUNG\src-tauri\target\release\bundle\nsis\FUNG_0.1.0_x64-setup.exe`
- Alternate bundle: `D:\FUNG\src-tauri\target\release\bundle\msi\FUNG_0.1.0_x64_en-US.msi`
- Local launcher fallback: `D:\FUNG\RUN_FUNG.bat`

## Suggested Demo Flow

1. Open the landing page and scroll through the four-part story.
2. Stop at the local-first architecture section and make the boundary clear:
   FUNG Desktop + GenesisBlockDB is the primary runtime.
   Supabase is optional control-plane/auth metadata.
   Vercel hosts the web surface only.
3. Jump to `/app` and walk through:
   Capture
   Transcript
   Summary
   Runtime
4. If the room wants the native app, run the NSIS setup file or use `RUN_FUNG.bat` as the fallback.

## Claims To Keep Honest

- Safe to say:
  - local-first
  - account optional
  - web landing is live
  - web app route is live
  - desktop installer artifact exists on this machine
- Do not say yet:
  - OAuth sign-in is live in production
  - cloud account is required
  - GenesisBlockDB runs in Supabase
  - the web page downloads the installer directly from production

## Last-Minute Checklist

- Keep these three tabs ready:
  - landing
  - `/app`
  - local file explorer opened at `D:\FUNG\src-tauri\target\release\bundle\nsis`
- If installer flow is too slow, use `RUN_FUNG.bat`.
- If asked about data location: answer "on-device by default; cloud is optional by choice."

## Validation Snapshot

- `npm run build` passed on 2026-07-24.
- `BUILD_FUNG_EXE.bat --check` passed on 2026-07-24.
- Existing installer artifacts were found on 2026-07-24.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-24 | beta | First presentation and demo runbook for the current production/web-desktop handoff. | N/A | ATHER |
