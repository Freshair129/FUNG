# FUNG Supabase control plane

This directory contains the Supabase-owned cloud control plane. It is separate from the FUNG Desktop runtime and **must not** become a second operational database for GenesisBlockDB.

## What is stored

- Supabase Auth identity references and a small user profile
- registered-device metadata and a public-key fingerprint
- redacted OAuth connection state and authorization audit metadata

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

Do not run the migration from a browser SQL editor without first reviewing it. Before production, run the Supabase Database Linter/Advisors and verify all four tables have RLS enabled.

## Google Drive metadata Edge Function

The approved Google Drive slice includes
`supabase/functions/google-drive-metadata/index.ts`. It is an authenticated,
metadata-only writer for the native Desktop/Mobile clients. It accepts the
allowlisted event types and the exact `drive.appdata` scope, derives the user
from the verified Supabase JWT, generates the audit correlation ID on the
server, and writes only `oauth_connections` / `oauth_audit_events` metadata.
It does not exchange Google tokens and it never accepts a token, authorization
code, user ID, or provider response from the client.

Deploy only after reviewing the linked migration and project environment:

```powershell
supabase functions deploy google-drive-metadata
```

The native installed-app PKCE flow does not require a Google client secret. If
a future provider-specific server exchange is approved, its secrets must be
set only with `supabase secrets set` or the Supabase dashboard:

```text
OAUTH_PROVIDER_ISSUER
OAUTH_PROVIDER_CLIENT_ID
OAUTH_PROVIDER_CLIENT_SECRET
OAUTH_PROVIDER_TOKEN_ENDPOINT
OAUTH_PROVIDER_REVOCATION_ENDPOINT
OAUTH_PROVIDER_ALLOWED_REDIRECT_URI
```

Never put these secrets in a `VITE_*` variable, Desktop bundle, repository file, browser client, or chat. The function must validate its caller session, use a provider allowlist and exact redirect URI, redact all logs, and write only redacted metadata to `oauth_connections` and `oauth_audit_events`.

Real Google consent, upload/download/revoke, clean-install restore, and
function deployment remain external gates. Local FUNG mode does not depend on
this function or on Google Drive configuration.

## RLS model

- `profiles`: users can read/update only their own row; provisioning occurs through a trusted server-side path.
- `devices`: users can manage only their own device records.
- `oauth_connections` and `oauth_audit_events`: users are read-only and see only their own records; a controlled server-side path writes them.

No `anon` grants are provided. Service-role use is server-side only.

## W1 server authority boundary (local implementation only)

The W1-A-F4-S1 implementation is project-agnostic and has not been deployed.
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
public-key fingerprint satisfies the exact Drive predicate.

An active `google_drive` connection is not an operation grant. The operator
issues `backup.write` and `backup.restore` independently, and revocation of the
connection revokes both without resurrecting them on reconnection. Archive
read follows the restore grant. Signed requests use the unique durable nonce
reservation and server decision tables; `oauth_audit_events` and
`device_audit_events` remain informational and are never replay locks.

The committed `deno.lock` pins the Edge dependency used by the enrollment,
authorizer, and metadata functions. `supabase/tests/w1_authority_schema.sql`
contains read-only privilege, RLS, fixed-search-path, and reservation evidence.
Live migration, RLS/grant verification, bootstrap approval, Edge deployment,
and provider testing remain external gates and must be performed only after an
enumerated project-ref manifest and separate deployment approval.
