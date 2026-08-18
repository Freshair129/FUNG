---
version: "0.1.0b"
created_at: "2026-08-14T11:18:00+07:00,ATHER"
last_update: "2026-08-14T11:18:00+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "website-release"
  doc_type: "root-cause-analysis"
  scope: "Vercel production bootstrap environment"
---

# RCA — Production website cannot render the download landing page

## Symptom

The production-style local build renders the missing-Supabase configuration
notice before the Landing Page, so the Windows download CTA is unavailable.

## Evidence

1. `src/main.tsx` gates web surfaces on both `VITE_SUPABASE_URL` and
   `VITE_SUPABASE_ANON_KEY` before rendering the requested surface.
2. The Vercel Production environment inventory contains only
   `VITE_SUPABASE_URL`; the pulled value is empty and
   `VITE_SUPABASE_ANON_KEY` is absent.
3. The currently deployed production JavaScript contains no legacy anon JWT or
   modern `sb_publishable_` client key.
4. The authenticated Supabase project API reports one publishable key for the
   project identified by the local project URL. Secret key material is not
   required for the browser client and must not be exposed.

## Root Cause

The Vercel Production build environment was provisioned with an empty project
URL and without a public client key. The bootstrap contract correctly fails
closed, but that configuration prevents the public Landing Page from rendering.

## Why the issue escaped detection

The website deployment gate checked deployment readiness and HTTP availability,
but did not build locally from the exact Production environment or assert that
the download CTA rendered. The protected preview URL also hid the application
behind Vercel authentication during anonymous inspection.

## Proposed prevention

- Set the exact Supabase project URL and its publishable client key in Vercel
  Production; never use a secret or service-role key in a `VITE_` variable.
- Rebuild from the pulled Production environment and assert CTA text, stable
  release URL, version, and SmartScreen disclosure in a real browser.
- Promote only after the GitHub release asset exists and the CTA resolves.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Initial evidence-backed RCA for the missing Vercel bootstrap environment. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-08-14 | candidate | Documented the empty URL, absent publishable key, and production verification gate. | pending | ATHER |
