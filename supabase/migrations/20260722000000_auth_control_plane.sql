-- FUNG cloud control plane: identity, registered devices, and redacted OAuth metadata.
--
-- This migration intentionally does NOT store access tokens, refresh tokens,
-- authorization codes, device codes, client secrets, source audio, transcripts,
-- notes, or GenesisBlockDB data. OAuth credential material stays in the native
-- client's OS secure storage (or the provider) and is never persisted here.

create table if not exists public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  display_name text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint profiles_display_name_length check (char_length(display_name) <= 120)
);

create table if not exists public.devices (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.profiles (id) on delete cascade,
  device_label text not null,
  platform text not null,
  public_key_fingerprint text not null,
  registered_at timestamptz not null default now(),
  last_seen_at timestamptz,
  revoked_at timestamptz,
  constraint devices_label_length check (char_length(device_label) between 1 and 120),
  constraint devices_platform_length check (char_length(platform) between 1 and 40),
  constraint devices_fingerprint_length check (char_length(public_key_fingerprint) between 16 and 255),
  constraint devices_unique_fingerprint_per_user unique (user_id, public_key_fingerprint)
);

create table if not exists public.oauth_connections (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.profiles (id) on delete cascade,
  provider text not null,
  provider_subject_reference text,
  approved_scopes text[] not null default '{}',
  status text not null default 'active',
  connected_at timestamptz not null default now(),
  revoked_at timestamptz,
  last_authorized_at timestamptz not null default now(),
  constraint oauth_connections_provider_length check (char_length(provider) between 1 and 80),
  constraint oauth_connections_status check (status in ('active', 'revoked', 'expired', 'error')),
  constraint oauth_connections_scope_values check (
    array_position(approved_scopes, '') is null
    and cardinality(approved_scopes) <= 32
  ),
  constraint oauth_connections_revocation_state check (
    (status = 'revoked' and revoked_at is not null)
    or (status <> 'revoked' and revoked_at is null)
  ),
  constraint oauth_connections_unique_provider_per_user unique (user_id, provider)
);

create table if not exists public.oauth_audit_events (
  id bigint generated always as identity primary key,
  user_id uuid not null references public.profiles (id) on delete cascade,
  connection_id uuid references public.oauth_connections (id) on delete set null,
  provider text not null,
  client_type text not null,
  event_type text not null,
  outcome text not null,
  scopes text[] not null default '{}',
  correlation_id uuid not null,
  occurred_at timestamptz not null default now(),
  constraint oauth_audit_events_provider_length check (char_length(provider) between 1 and 80),
  constraint oauth_audit_events_client_type check (client_type in ('web', 'desktop', 'mobile', 'cli', 'mcp')),
  constraint oauth_audit_events_event_type check (event_type in (
    'authorization_started',
    'authorization_completed',
    'authorization_denied',
    'authorization_expired',
    'connection_revoked',
    'token_refresh_failed'
  )),
  constraint oauth_audit_events_outcome check (outcome in ('success', 'denied', 'expired', 'failed')),
  constraint oauth_audit_events_scope_values check (
    array_position(scopes, '') is null
    and cardinality(scopes) <= 32
  )
);

create index if not exists devices_user_id_active_idx
  on public.devices (user_id, registered_at desc)
  where revoked_at is null;

create index if not exists oauth_connections_user_id_active_idx
  on public.oauth_connections (user_id, connected_at desc)
  where status = 'active';

create index if not exists oauth_audit_events_user_id_occurred_at_idx
  on public.oauth_audit_events (user_id, occurred_at desc);

alter table public.profiles enable row level security;
alter table public.devices enable row level security;
alter table public.oauth_connections enable row level security;
alter table public.oauth_audit_events enable row level security;

-- Profiles are created by the trusted server-side provisioning path. Users can
-- read and edit only their own display name; id and timestamps cannot be changed.
create policy "profiles: user reads own profile"
  on public.profiles for select to authenticated
  using ((select auth.uid()) = id);

create policy "profiles: user updates own profile"
  on public.profiles for update to authenticated
  using ((select auth.uid()) = id)
  with check ((select auth.uid()) = id);

-- Device enrolment is user-owned, but no authenticated user can see or alter
-- another user's device identity.
create policy "devices: user reads own devices"
  on public.devices for select to authenticated
  using ((select auth.uid()) = user_id);

create policy "devices: user registers own device"
  on public.devices for insert to authenticated
  with check ((select auth.uid()) = user_id);

create policy "devices: user updates own device"
  on public.devices for update to authenticated
  using ((select auth.uid()) = user_id)
  with check ((select auth.uid()) = user_id);

create policy "devices: user removes own device"
  on public.devices for delete to authenticated
  using ((select auth.uid()) = user_id);

-- OAuth connection state and audit history are server-written. The authenticated
-- user is deliberately read-only and only sees records belonging to that user.
-- Edge Functions/service role bypass RLS for the controlled write path.
create policy "oauth_connections: user reads own connections"
  on public.oauth_connections for select to authenticated
  using ((select auth.uid()) = user_id);

create policy "oauth_audit_events: user reads own audit history"
  on public.oauth_audit_events for select to authenticated
  using ((select auth.uid()) = user_id);

revoke all on table public.profiles from anon;
revoke all on table public.devices from anon;
revoke all on table public.oauth_connections from anon;
revoke all on table public.oauth_audit_events from anon;

grant select, update (display_name) on table public.profiles to authenticated;
grant select,
  insert (user_id, device_label, platform, public_key_fingerprint),
  update (device_label, last_seen_at),
  delete on table public.devices to authenticated;
grant select on table public.oauth_connections to authenticated;
grant select on table public.oauth_audit_events to authenticated;

comment on table public.profiles is
  'Cloud account profile only. This is not a GenesisBlockDB data store.';
comment on table public.devices is
  'User-owned enrolled device metadata. The full private key is never stored here.';
comment on table public.oauth_connections is
  'Redacted OAuth connection metadata only; contains no token or authorization-code material.';
comment on table public.oauth_audit_events is
  'Redacted authorization audit metadata only; contains no token, device code, user content, or provider response.';
