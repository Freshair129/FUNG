---
version: "0.1.0b"
created_at: "2026-07-22T00:00:00+07:00,ATHER"
last_update: "2026-09-16T00:00:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "web-deployment"
  doc_type: "technical-design"
  scope: "FUNG Vite frontend deployment to Vercel"
  language: "English"
---

# FUNG Web Production Deployment

## Scope

Vercel hosts only the static Vite frontend built from `dist/`. Supabase remains the cloud control plane for Auth and metadata; GenesisBlockDB remains embedded in the Desktop runtime. No service-role key, OAuth client secret, project content, or GenesisBlockDB data is deployed to Vercel.

## Deployment Contract

| Item | Value |
| --- | --- |
| Team | `pornpons-projects` |
| Framework | Vite |
| Build | `npm run build` |
| Published directory | `dist/` |
| Required public environment value | `VITE_SUPABASE_URL=https://nqnrvqnijzovkrhxslfp.supabase.co` |
| Supabase client key | Add only the publishable/anon key as `VITE_SUPABASE_ANON_KEY` when the frontend auth client is implemented; never add service-role credentials |

## First Production Release

1. Create or select a clearly named Vercel project under `pornpons-projects` (recommended: `fung`).
2. Add the Production environment values in Vercel. Values with the `VITE_` prefix are included in the browser build, so they must be public by design.
3. Confirm Supabase Auth has the final Vercel production URL and `/auth/callback` in its allowed redirect URLs before enabling sign-in.
4. Run `npm run build` locally and deploy only after it succeeds.
5. Smoke-test the production URL: initial render, Google sign-in through `/auth/callback` (supabase-js PKCE; the callback URL must be in Supabase Auth → Redirect URLs), `/app` after sign-in, and a browser recording on the dashboard.

## What the browser build can and cannot do

The same `dist/` serves the public site and the signed-in dashboard; there is no Rust runtime and no GenesisBlockDB behind it.

| On `/app` (after Google sign-in) | Status |
| --- | --- |
| Record from the microphone, keep the file in this browser (IndexedDB), play it back, download it, delete it | Works. Files stay in that browser profile; nothing is uploaded. Move a file to the desktop by downloading it. |
| List and play recordings from FUNG desktop | Works on the **same machine** only: the desktop's loopback API (`Settings › Runtime → Start local API`, paste the `http://127.0.0.1:PORT/#TOKEN` link). Other machines and phones use the desktop's LAN page instead. |
| Transcribe a browser recording | Works on the **same machine** once the desktop is connected: "ถอดเสียงที่ desktop" uploads the file over loopback, the desktop runs its normal Whisper import job, and the transcript appears under the recording (and the recording appears in the desktop's own list). |
| Paired devices (list, revoke), account profile | Works (Supabase, via the `device-enrollment` Edge function). |
| Summaries, notes, graph, Live Meeting, local backup | Desktop only. The dashboard never links into the desktop shell (`/app?surface=desktop` exists for developers and calls Tauri IPC unguarded). Google Drive is canceled and is not a web capability. |

Any path other than `/app` and `/auth/callback` is rewritten to the SPA (`vercel.json`), which renders the landing page; a build without `VITE_SUPABASE_*` still renders the landing page, just without sign-in controls.

## Is production current?

Vercel does not show a commit on the page. A quick check: the landing page section "04 / Demo-ready" must render Thai text; the build before PR #37 (2 Sep 2026) rendered it as mojibake, and on 16 Sep 2026 production was still that build — everything merged since (desktop-recordings tile, auth fixes, this dashboard) was absent. When the Vercel Git integration has not picked a merge up, redeploy from the Vercel dashboard or with `npx vercel --prod` from a logged-in CLI.

## Rollback

Use the Vercel dashboard to promote the prior verified deployment. This static deployment has no database migration or GenesisBlockDB migration coupled to it.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.0b | 2026-09-16 | beta | Sign-in is live (supabase-js PKCE); documented the browser build's real capability table, the SPA catch-all rewrite, and how to tell a stale production deploy. | working-tree | ATHER |
| 0.1.0b | 2026-07-22 | beta | Initial Vercel frontend deployment contract and secret boundary. | N/A | ATHER |
