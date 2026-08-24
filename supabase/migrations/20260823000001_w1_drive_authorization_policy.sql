-- W1-A-F4-S1: independent Drive operation grants and durable authorization
-- reservation. This migration contains no project reference and no deploy step.

BEGIN;

CREATE TABLE public.oauth_operation_grants (id uuid PRIMARY KEY DEFAULT
  gen_random_uuid(), user_id uuid NOT NULL REFERENCES public.profiles (id) ON
  DELETE CASCADE, connection_id uuid NOT NULL REFERENCES
  public.oauth_connections (id) ON DELETE CASCADE, operation text NOT NULL,
  status text NOT NULL DEFAULT 'active', granted_by text NOT NULL, granted_role
  text NOT NULL DEFAULT 'database_owner', granted_at timestamptz NOT NULL
  DEFAULT now(), revoked_by text, revoked_at timestamptz, CONSTRAINT
  oauth_operation_grants_operation CHECK (operation IN ('backup.write',
  'backup.restore')), CONSTRAINT oauth_operation_grants_status CHECK (status IN
  ('active', 'revoked', 'expired')), CONSTRAINT
  oauth_operation_grants_revocation_state CHECK ((status = 'active' AND
  revoked_at IS NULL) OR (status <> 'active' AND revoked_at IS NOT NULL)));

CREATE UNIQUE INDEX oauth_operation_grants_active_unique_idx ON
  public.oauth_operation_grants (connection_id, operation)
WHERE status = 'active';

CREATE INDEX oauth_operation_grants_user_status_idx ON
  public.oauth_operation_grants (user_id, status, operation);

ALTER TABLE public.oauth_operation_grants enable ROW level security;
revoke ALL ON TABLE public.oauth_operation_grants
FROM public, anon, authenticated, service_role;
grant
SELECT ON TABLE public.oauth_operation_grants TO service_role;

-- A signed request is reserved by nonce before its native/provider-capable
-- operation continues. The unique nonce is the lock; audit is not the lock.
CREATE TABLE public.oauth_authorization_reservations (id uuid PRIMARY KEY
  DEFAULT gen_random_uuid(), nonce uuid NOT NULL, user_id uuid NOT NULL
  REFERENCES public.profiles (id) ON DELETE CASCADE, device_id uuid NOT NULL
  REFERENCES public.devices (id) ON DELETE restrict, connection_id uuid
  REFERENCES public.oauth_connections (id) ON DELETE SET NULL, operation text
  NOT NULL, status text NOT NULL DEFAULT 'reserved', expires_at timestamptz NOT
  NULL, created_at timestamptz NOT NULL DEFAULT now(), CONSTRAINT
  oauth_authorization_reservations_operation CHECK (operation IN
  ('connection.authorize', 'connection.activate', 'connection.read',
  'connection.revoke', 'backup.read', 'backup.write', 'backup.restore')),
  CONSTRAINT oauth_authorization_reservations_nonce_key UNIQUE (nonce),
  CONSTRAINT oauth_authorization_reservations_status CHECK (status IN
  ('reserved', 'allowed', 'denied', 'expired')), CONSTRAINT
  oauth_authorization_reservations_expiry CHECK (expires_at > created_at));

CREATE INDEX oauth_authorization_reservations_user_created_idx ON
  public.oauth_authorization_reservations (user_id, created_at DESC);

ALTER TABLE public.oauth_authorization_reservations enable ROW level security;
revoke ALL ON TABLE public.oauth_authorization_reservations
FROM public, anon, authenticated, service_role;

-- This table is an authoritative server decision linkage. Client-authored
-- device_audit_events and oauth_audit_events remain informational only.
CREATE TABLE public.oauth_authorization_decisions (id uuid PRIMARY KEY DEFAULT
  gen_random_uuid(), reservation_id uuid NOT NULL UNIQUE REFERENCES
  public.oauth_authorization_reservations (id) ON DELETE CASCADE, user_id uuid
  NOT NULL REFERENCES public.profiles (id) ON DELETE CASCADE, decision text NOT
  NULL CHECK (decision IN ('allowed', 'denied')), denial_code text, decided_at
  timestamptz NOT NULL DEFAULT now(), CONSTRAINT
  oauth_authorization_decisions_denial_code CHECK ((decision = 'allowed' AND
  denial_code IS NULL) OR (decision = 'denied' AND denial_code IS NOT NULL)));

ALTER TABLE public.oauth_authorization_decisions enable ROW level security;
revoke ALL ON TABLE public.oauth_authorization_decisions
FROM public, anon, authenticated, service_role;

-- Connection revocation invalidates both grants. Reactivation does not restore
-- them; the operator must issue fresh independent grant rows.
CREATE function public.revoke_oauth_operation_grants_on_connection_change()
  returns trigger language plpgsql security definer SET search_path =
  pg_catalog, public, pg_temp AS $$
begin
  if new.status = 'revoked' or new.revoked_at is not null then
    update public.oauth_operation_grants
    set status = 'revoked',
        revoked_by = session_user,
        revoked_at = coalesce(revoked_at, pg_catalog.now())
    where connection_id = new.id
      and status = 'active';
  end if;
  return new;
end;
$$;

revoke execute ON function
  public.revoke_oauth_operation_grants_on_connection_change()
FROM public, anon, authenticated, service_role;

DO $$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM pg_trigger
    WHERE tgrelid = 'public.oauth_connections'::regclass
      AND tgname = 'oauth_connection_revokes_operation_grants'
      AND NOT tgisinternal
  ) THEN
    RAISE EXCEPTION 'incompatible oauth connection grant trigger already exists';
  END IF;
END;
$$;

CREATE trigger oauth_connection_revokes_operation_grants after UPDATE of status,
  revoked_at ON public.oauth_connections FOR each ROW execute function
  public.revoke_oauth_operation_grants_on_connection_change();

-- Device revocation invalidates operation grants for the account as well.
-- Rebind uses this trigger before inserting its replacement identity.
CREATE function public.revoke_oauth_operation_grants_on_device_change() returns
  trigger language plpgsql security definer SET search_path = pg_catalog,
  public, pg_temp AS $$
begin
  if new.authority_state = 'revoked' or new.revoked_at is not null then
    update public.oauth_operation_grants
    set status = 'revoked',
        revoked_by = session_user,
        revoked_at = coalesce(revoked_at, pg_catalog.now())
    where user_id = new.user_id
      and status = 'active';
  end if;
  return new;
end;
$$;

revoke execute ON function
  public.revoke_oauth_operation_grants_on_device_change()
FROM public, anon, authenticated, service_role;

DO $$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM pg_trigger
    WHERE tgrelid = 'public.devices'::regclass
      AND tgname = 'device_revokes_oauth_operation_grants'
      AND NOT tgisinternal
  ) THEN
    RAISE EXCEPTION 'incompatible device grant trigger already exists';
  END IF;
END;
$$;

CREATE trigger device_revokes_oauth_operation_grants after UPDATE of
  authority_state, revoked_at ON public.devices FOR each ROW execute function
  public.revoke_oauth_operation_grants_on_device_change();

-- Operator-only grant issuance. A connection proves provider scope, not an
-- operation; every grant is separately issued and separately revocable.
CREATE function public.grant_oauth_operation(p_connection_id uuid, p_operation
  text) returns uuid language plpgsql security definer SET search_path =
  pg_catalog, public, pg_temp AS $$
declare
  v_connection public.oauth_connections%rowtype;
  v_grant_id uuid;
begin
  if p_operation not in ('backup.write', 'backup.restore') then
    raise exception 'invalid_oauth_operation';
  end if;

  select * into v_connection
  from public.oauth_connections
  where id = p_connection_id
    and provider = 'google_drive'
    and status = 'active'
    and revoked_at is null
    and approved_scopes = array['https://www.googleapis.com/auth/drive.appdata']
  for update;
  if not found then
    raise exception 'connection_not_active_or_exact_scope';
  end if;

  select id into v_grant_id
  from public.oauth_operation_grants
  where connection_id = p_connection_id
    and operation = p_operation
    and status = 'active';
  if found then
    return v_grant_id;
  end if;

  insert into public.oauth_operation_grants (
    user_id,
    connection_id,
    operation,
    granted_by,
    granted_role
  ) values (
    v_connection.user_id,
    p_connection_id,
    p_operation,
    session_user,
    'database_owner'
  )
  returning id into v_grant_id;
  return v_grant_id;
end;
$$;

revoke execute ON function public.grant_oauth_operation(uuid, text)
FROM public, anon, authenticated, service_role;

CREATE function public.revoke_oauth_operation_grant(p_connection_id uuid,
  p_operation text) returns boolean language plpgsql security definer SET
  search_path = pg_catalog, public, pg_temp AS $$
begin
  if p_operation not in ('backup.write', 'backup.restore') then
    raise exception 'invalid_oauth_operation';
  end if;

  update public.oauth_operation_grants
  set status = 'revoked',
      revoked_by = session_user,
      revoked_at = coalesce(revoked_at, pg_catalog.now())
  where connection_id = p_connection_id
    and operation = p_operation
    and status = 'active';
  return found;
end;
$$;

revoke execute ON function public.revoke_oauth_operation_grant(uuid, text)
FROM public, anon, authenticated, service_role;

-- A signed request is authorized, reserved, and decided in one database
-- transaction. Lock order: device -> connection -> operation grants -> nonce
-- reservation -> decision. The serialization order is device -> connection -> operation
-- grants (write, then restore) -> nonce reservation -> decision. Revocation
-- paths lock the same authoritative rows before changing their state; audit
-- rows are never part of this authority boundary.
CREATE function public.authorize_oauth_request(p_user_id uuid, p_device_id uuid,
  p_device_public_key text, p_device_fingerprint text, p_operation text, p_nonce
  uuid, p_expires_at timestamptz) returns table(reservation_id uuid, operation
  text, nonce uuid, authorized boolean, denial_code text, connection_id uuid,
  connection_status text, write_grant_status text, restore_grant_status text,
  expires_at timestamptz) language plpgsql security definer SET search_path =
  pg_catalog, public, pg_temp AS $$
declare
  v_device public.devices%rowtype;
  v_connection public.oauth_connections%rowtype;
  v_write_grant public.oauth_operation_grants%rowtype;
  v_restore_grant public.oauth_operation_grants%rowtype;
  v_existing_reservation public.oauth_authorization_reservations%rowtype;
  v_reservation_id uuid;
  v_connection_id uuid;
  v_required_operation text;
  v_denial_code text;
  v_connection_status text := 'disconnected';
  v_write_grant_status text := 'missing';
  v_restore_grant_status text := 'missing';
  v_connection_found boolean := false;
  v_connection_active boolean := false;
  v_device_authorized boolean := false;
  v_write_grant_found boolean := false;
  v_restore_grant_found boolean := false;
  v_authorized boolean := false;
begin
  if p_user_id is null
    or p_device_id is null
    or p_device_public_key is null
    or p_device_fingerprint is null
    or p_device_fingerprint !~* '^[0-9a-f]{64}$'
    or p_operation not in (
      'connection.authorize',
      'connection.activate',
      'connection.read',
      'connection.revoke',
      'backup.read',
      'backup.write',
      'backup.restore'
    )
    or p_nonce is null
    or p_expires_at <= pg_catalog.now()
    or p_expires_at > pg_catalog.now() + pg_catalog.make_interval(mins => 2) then
    raise exception 'invalid_authorization_request';
  end if;

  -- Lock the device before reading any other authority row. The predicate is
  -- evaluated again here after Edge signature verification, not trusted from
  -- the earlier public-key lookup.
  select * into v_device
  from public.devices as d
  where d.id = p_device_id
    and d.user_id = p_user_id
  for update;
  if found then
    begin
      v_device_authorized := coalesce(
        public.is_drive_authorized_desktop(p_user_id, p_device_id),
        false
      )
      and v_device.public_key = p_device_public_key
      and v_device.public_key_fingerprint = p_device_fingerprint;
    exception when others then
      v_device_authorized := false;
    end;
  end if;

  -- Lock the account's exact-provider connection next. Missing or inactive
  -- connection state is returned as data for status operations and is a deny
  -- for backup operations and revoked activation below.
  select * into v_connection
  from public.oauth_connections as c
  where c.user_id = p_user_id
    and c.provider = 'google_drive'
  for update;
  v_connection_found := found;
  if v_connection_found then
    v_connection_id := v_connection.id;
    v_connection_status := coalesce(v_connection.status, 'unknown');
    if v_connection.status = 'active'
      and v_connection.revoked_at is null
      and v_connection.approved_scopes = ARRAY[
        'https://www.googleapis.com/auth/drive.appdata'
      ]::text[] then
      v_connection_active := true;
      v_connection_status := 'active';
    end if;
  end if;

  -- Lock both independent grants in deterministic order. The latest row is
  -- the current state when historical revoked rows are retained.
  if v_connection_found then
    select * into v_write_grant
    from public.oauth_operation_grants as g
    where g.user_id = p_user_id
      and g.connection_id = v_connection.id
      and g.operation = 'backup.write'
    order by (g.status = 'active') desc, g.granted_at desc, g.id desc
    limit 1
    for update;
    v_write_grant_found := found;
    if v_write_grant_found then
      v_write_grant_status := v_write_grant.status;
    end if;

    select * into v_restore_grant
    from public.oauth_operation_grants as g
    where g.user_id = p_user_id
      and g.connection_id = v_connection.id
      and g.operation = 'backup.restore'
    order by (g.status = 'active') desc, g.granted_at desc, g.id desc
    limit 1
    for update;
    v_restore_grant_found := found;
    if v_restore_grant_found then
      v_restore_grant_status := v_restore_grant.status;
    end if;
  end if;

  v_required_operation := case p_operation
    when 'backup.write' then 'backup.write'
    when 'backup.read' then 'backup.restore'
    when 'backup.restore' then 'backup.restore'
    else null
  end;

  if v_device_authorized is distinct from true then
    v_denial_code := 'device_not_authorized';
  elsif p_operation = 'connection.activate'
    and v_connection_found
    and (
      v_connection.status = 'revoked'
      or v_connection.revoked_at is not null
    ) then
    v_denial_code := 'connection_revoked';
  elsif v_required_operation is not null
    and v_connection_active is distinct from true then
    v_denial_code := 'connection_not_active_or_exact_scope';
  elsif v_required_operation = 'backup.write'
    and (v_write_grant_found is distinct from true
      or v_write_grant.status <> 'active'
      or v_write_grant.revoked_at is not null) then
    v_denial_code := 'backup_write_grant_missing';
  elsif v_required_operation = 'backup.restore'
    and (v_restore_grant_found is distinct from true
      or v_restore_grant.status <> 'active'
      or v_restore_grant.revoked_at is not null) then
    v_denial_code := 'backup_restore_grant_missing';
  end if;
  v_authorized := v_denial_code is null;

  -- The unique nonce insert is the only replay winner. A conflict is locked
  -- and returned as denied; it never falls through to audit or a second RPC.
  insert into public.oauth_authorization_reservations as reservation (
    nonce,
    user_id,
    device_id,
    connection_id,
    operation,
    expires_at
  ) values (
    p_nonce,
    p_user_id,
    p_device_id,
    v_connection_id,
    p_operation,
    p_expires_at
  )
  on conflict on constraint oauth_authorization_reservations_nonce_key do nothing
  returning reservation.id into v_reservation_id;

  if v_reservation_id is null then
    select * into v_existing_reservation
    from public.oauth_authorization_reservations as r
    where r.nonce = p_nonce
    for update;
    if not found then
      raise exception 'authorization_reservation_unavailable';
    end if;
    return query select
      v_existing_reservation.id,
      v_existing_reservation.operation,
      v_existing_reservation.nonce,
      false,
      'authorization_replayed'::text,
      v_connection_id,
      v_connection_status,
      v_write_grant_status,
      v_restore_grant_status,
      v_existing_reservation.expires_at;
    return;
  end if;

  insert into public.oauth_authorization_decisions (
    reservation_id,
    user_id,
    decision,
    denial_code
  ) values (
    v_reservation_id,
    p_user_id,
    case when v_authorized then 'allowed' else 'denied' end,
    v_denial_code
  );

  update public.oauth_authorization_reservations as reservation
  set status = case when v_authorized then 'allowed' else 'denied' end
  where reservation.id = v_reservation_id;

  -- Connection state transitions remain inside the same authorization
  -- transaction. Revoked activation is denied above and never clears state.
  if v_authorized and p_operation = 'connection.activate' then
    insert into public.oauth_connections as connection (
      user_id,
      provider,
      approved_scopes,
      status,
      connected_at,
      revoked_at,
      last_authorized_at
    ) values (
      p_user_id,
      'google_drive',
      ARRAY['https://www.googleapis.com/auth/drive.appdata']::text[],
      'active',
      pg_catalog.now(),
      null,
      pg_catalog.now()
    )
    on conflict (user_id, provider) do update
    set approved_scopes = excluded.approved_scopes,
        status = excluded.status,
        connected_at = excluded.connected_at,
        revoked_at = null,
        last_authorized_at = excluded.last_authorized_at
    returning connection.id into v_connection_id;
    v_connection_active := true;
    v_connection_status := 'active';
  elsif v_authorized and p_operation = 'connection.revoke'
    and v_connection_found then
    update public.oauth_connections as connection
    set status = 'revoked',
        revoked_at = coalesce(revoked_at, pg_catalog.now())
    where connection.id = v_connection.id;
    v_connection_active := false;
    v_connection_status := 'revoked';
    if v_write_grant_found then v_write_grant_status := 'revoked'; end if;
    if v_restore_grant_found then v_restore_grant_status := 'revoked'; end if;
  end if;

  return query select
    v_reservation_id,
    p_operation,
    p_nonce,
    v_authorized,
    v_denial_code,
    v_connection_id,
    v_connection_status,
    v_write_grant_status,
    v_restore_grant_status,
    p_expires_at;
end;
$$;

revoke execute ON function public.authorize_oauth_request(uuid, uuid, text,
  text, text, uuid, timestamptz)
FROM public, anon, authenticated, service_role;
grant execute ON function public.authorize_oauth_request(uuid, uuid, text, text,
  text, uuid, timestamptz) TO service_role;

comment ON TABLE public.oauth_operation_grants IS
  'Operator-issued independent backup.write and backup.restore grants; connection state is not a grant.';
comment ON TABLE public.oauth_authorization_reservations IS
  'Durable unique nonce reservation. It is the replay lock; audit rows are not.';
comment ON TABLE public.oauth_authorization_decisions IS
  'Server-created authorization outcome linkage for a winning reservation.';

COMMIT;
