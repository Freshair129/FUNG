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

Install the Supabase CLI, authenticate it, link the project ref `nqnrvqnijzovkrhxslfp`, then review and push the migration through the normal deployment workflow:

```powershell
supabase login
supabase link --project-ref nqnrvqnijzovkrhxslfp
supabase db push
```

Do not run the migration from a browser SQL editor without first reviewing it. Before production, run the Supabase Database Linter/Advisors and verify all four tables have RLS enabled.

## OAuth exchange Edge Function (not created yet)

No Edge Function is included because the first external OAuth provider, exact issuer, registered redirect URIs, and approved scopes are still unresolved. Shipping a generic token-exchange endpoint before those values exist would create an unsafe, non-functional production surface.

Once a provider is approved, the server-side Edge Function may require these secrets, set only with `supabase secrets set` or the Supabase dashboard:

```text
OAUTH_PROVIDER_ISSUER
OAUTH_PROVIDER_CLIENT_ID
OAUTH_PROVIDER_CLIENT_SECRET
OAUTH_PROVIDER_TOKEN_ENDPOINT
OAUTH_PROVIDER_REVOCATION_ENDPOINT
OAUTH_PROVIDER_ALLOWED_REDIRECT_URI
```

Never put these secrets in a `VITE_*` variable, Desktop bundle, repository file, browser client, or chat. The function must validate its caller session, use a provider allowlist and exact redirect URI, redact all logs, and write only redacted metadata to `oauth_connections` and `oauth_audit_events`.

## RLS model

- `profiles`: users can read/update only their own row; provisioning occurs through a trusted server-side path.
- `devices`: users can manage only their own device records.
- `oauth_connections` and `oauth_audit_events`: users are read-only and see only their own records; a controlled server-side path writes them.

No `anon` grants are provided. Service-role use is server-side only.
