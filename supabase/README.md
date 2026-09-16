# FUNG Supabase control plane

This directory contains the Supabase-owned cloud control plane. It is separate from the FUNG Desktop runtime and **must not** become a second operational database for GenesisBlockDB.

## What is stored

- Supabase Auth identity references and a small user profile
- registered-device metadata and a public-key fingerprint
- historical OAuth connection and authorization-audit schema retained for migration provenance

## What is never stored

- access tokens, refresh tokens, authorization codes, device codes, client secrets, API keys, or JWTs
- source audio, transcripts, notes, embeddings, graph data, WAL data, or GenesisBlockDB exports

The native client stores credentials only in OS secure storage. GenesisBlockDB remains embedded and local to the Desktop runtime.

## Apply the migration

Install the Supabase CLI, authenticate it, link only the separately approved project ref, then review and push the migration through the normal deployment workflow:

```powershell
supabase login
supabase link --project-ref <approved-project-ref>
supabase db push
```

Do not run the migration from a browser SQL editor without first reviewing it.
The applied Google Drive migration is historical schema provenance, not a
request to activate a provider. Before any new deployment, review migration
history, run the Supabase Database Linter/Advisors, and verify all four tables
have RLS enabled.

## Current provider scope

FUNG no longer uses Google Drive. The provider adapter, Drive-specific Edge
Functions, deployment instructions, and provider/UAT gates were removed from
the active product on 2026-09-17. The cancellation record is
`docs/decisions/2026-09-17-google-drive-scope-cancellation.md`.

The remaining active cloud function is `device-enrollment`. Local encrypted
filesystem backup/restore remains the Phase 4 backup target. The historical
Drive migration and schema evidence remain in the repository only so an
already-applied database history is not rewritten; they are not a deployment
or runtime activation instruction.

## RLS model

- `profiles`: users can read/update only their own row; provisioning occurs through a trusted server-side path.
- `devices`: users can manage only their own device records.
- `oauth_connections` and `oauth_audit_events`: users are read-only and see only their own records; a controlled server-side path writes them.

No `anon` grants are provided. Service-role use is server-side only.

## Live state (2026-09-13)

The historical W1 migrations and the later profile backfill were previously
applied to the production project `nqnrvqnijzovkrhxslfp`. Current source
scope is limited to the `device-enrollment` Edge Function; the former
Google Drive functions are no longer part of this repository's active
deployment surface. `verify_jwt` and CORS settings must be re-verified before
any future function deployment; this document does not claim a remote
provider deletion or revocation.
They were applied through the Supabase management API rather than the CLI, so
the recorded migration versions are the apply timestamps
(`20260913005747`…`20260913010251`), not the file names — `supabase db push`
from a linked CLI will want to reconcile that history before pushing anything
new. Post-apply, the first read-only block of
`supabase/tests/w1_authority_schema.sql` passed and the Database Linter
reported no critical findings (the two `authenticated`-callable
`SECURITY DEFINER` pairing RPCs are intentional; they enforce ownership
themselves). Bootstrap approval and provider testing remain owner ceremonies.

Auth → URL Configuration → Redirect URLs must list every callback the
clients use, or GoTrue falls back to the Site URL (which on this shared
project is another app's `http://localhost:3000`): the web
`https://fung-seven.vercel.app/auth/callback`, the mobile deep link
`fung://auth/callback`, and the desktop's loopback listener on an
OS-assigned port, `http://127.0.0.1:*/auth/callback` (`*` matches the port;
`auth_session.rs` `CALLBACK_PATH`). The last one was missing until
2026-09-13, which is why desktop sign-in surfaced as
`bad_oauth_state` on the other app's page.

## W1 server authority boundary

The W1-A-F4-S1 implementation is project-agnostic (see "Live state" above for
where it is applied).
It adds a server-controlled authority state to `devices`; every pre-existing
device starts as `legacy` and cannot be promoted automatically. Authenticated
clients retain owner-scoped read access only. Pending enrollment requests are
non-authoritative, and the authenticated `device-enrollment` Edge function can
create only pending or `pairing_only` state and can request a server-owned soft
revocation. It cannot call the bootstrap approval function.

`approve_bootstrap_enrollment(uuid)` and the explicit
`approve_rebind_enrollment(uuid, uuid)` ceremony are database-owner-only. Their
execute privilege is revoked from `PUBLIC`, `anon`, `authenticated`, and
`service_role`; the database owner must verify the request out-of-band before
consuming it. Rebind soft-revokes the selected old trusted row before creating
a new identity. Only the resulting Windows row with `drive_trusted`,
`boss_bootstrap` or `approved_rebind`, an unrevoked identity, and a matching
public-key fingerprint satisfies the historical provider predicate. The
provider-specific authority states and operation-grant tables are retained for
database provenance; no active FUNG client issues provider backup grants after
the Google Drive cancellation. Signed request and device-enrollment evidence
remains separate from any future cloud-provider decision.

The committed `deno.lock` pins the Edge dependency used by the enrollment
function. `supabase/tests/w1_authority_schema.sql` contains read-only
privilege, RLS, fixed-search-path, and historical reservation evidence. Live
migration and grant verification were performed on 2026-09-13; no current
Google Drive deployment or provider test is claimed. Bootstrap approval
remains an owner ceremony performed out-of-band.

Note for clients: after W1, `authenticated` holds only `SELECT` on
`public.devices`. A direct `delete()`/`insert()`/`update()` on that table from
the web or mobile client is refused; revocation and registration go through
the `device-enrollment` Edge function (`action: "revoke"` / `"pairing_only"`,
wrapped by `src/lib/deviceAuthority.ts`), and the desktop publishes its
FUNGWIRE LAN endpoint through the owner-scoped
`public.publish_device_endpoint(uuid, text)` RPC
(`20260913000001_publish_device_endpoint.sql`, `authenticated`-callable
`SECURITY DEFINER` like the pairing RPCs; it refuses revoked rows and never
changes authority state). `tests/deviceAuthority.test.mjs` pins this.
