---
version: "0.1.0b"
created_at: "2026-07-22T00:00:00+07:00,ATHER"
last_update: "2026-07-22T00:00:00+07:00,ATHER"
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
5. Smoke-test the production URL: initial render, local-first workspace entry, and the external-account panel. Do not claim OAuth sign-in is live until its callback and token exchange are implemented and verified.

## Rollback

Use the Vercel dashboard to promote the prior verified deployment. This static deployment has no database migration or GenesisBlockDB migration coupled to it.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.1.0b | 2026-07-22 | beta | Initial Vercel frontend deployment contract and secret boundary. | N/A | ATHER |
