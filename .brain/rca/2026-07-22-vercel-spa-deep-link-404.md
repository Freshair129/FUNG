---
version: "0.1.0b"
created_at: "2026-07-22T01:19:48+07:00,ATHER"
last_update: "2026-07-22T01:19:48+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "web-deployment"
  doc_type: "rca"
  scope: "Vercel SPA routes"
---

# RCA — Vercel SPA deep links return 404

## Symptom

The Vercel preview serves `/` successfully, but a direct request to `/app` returns `404 Not Found`. This blocks the landing-page CTA and would also block `/auth/callback` when opened directly.

## Evidence

- Deployment `dpl_7x6kBvcHLhFse1Wn4Dc31rhayvUC` reached `Ready` and returned the generated `index.html` for `/`.
- Authenticated `vercel curl .../app --head` returned `HTTP/1.1 404 Not Found` with `X-Vercel-Error: NOT_FOUND`.
- Local Vite preview serves `/app` because its development/preview server automatically falls back to the SPA entry.
- `vercel.json` declares the Vite build and `dist` output but has no rewrite contract for client-side routes.

## Root Cause

The application selects Landing, App, and OAuth surfaces in client-side React using `window.location.pathname`, while Vercel static hosting resolves `/app` and `/auth/callback` as physical files. Without explicit rewrites, those files do not exist and React never starts.

## Why the issue escaped detection

The first verification used Vite preview, whose SPA fallback behavior differs from the deployed Vercel static route behavior. The initial Vercel check covered deployment readiness and `/`, but not direct deep-link responses until the CTA route gate.

## Proposed prevention

1. Add explicit rewrites for `/app` and `/auth/callback` to `/index.html`.
2. Keep the rewrite scope limited to approved SPA routes rather than masking every unknown path.
3. Add authenticated post-deploy HTTP checks for `/`, `/app`, and `/auth/callback` before promotion.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-07-22 | beta | Confirmed missing Vercel SPA rewrites as the deep-link 404 root cause | — | ATHER |
