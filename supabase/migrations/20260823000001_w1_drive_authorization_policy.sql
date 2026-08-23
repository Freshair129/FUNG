-- W1-A-F4-S1: independent Drive operation grants and durable authorization
-- reservation. This migration contains no project reference and no deploy step.

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
  DEFAULT gen_random_uuid(), nonce uuid NOT NULL UNIQUE, user_id uuid NOT NULL
  REFERENCES public.profiles (id) ON DELETE CASCADE, device_id uuid NOT NULL
  REFERENCES public.devices (id) ON DELETE restrict, connection_id uuid
  REFERENCES public.oauth_connections (id) ON DELETE SET NULL, operation text
  NOT NULL, status text NOT NULL DEFAULT 'reserved', expires_at timestamptz NOT
  NULL, created_at timestamptz NOT NULL DEFAULT now(), CONSTRAINT
  oauth_authorization_reservations_operation CHECK (operation IN
  ('connection.authorize', 'connection.activate', 'connection.read',
  'connection.revoke', 'backup.read', 'backup.write', 'backup.restore')),
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

-- One atomic durable reservation primitive. A conflict returns won=false;
-- it never falls back to an audit read or an isolate-local replay map.
CREATE function public.reserve_oauth_authorization(p_user_id uuid, p_device_id
  uuid, p_connection_id uuid, p_operation text, p_nonce uuid, p_expires_at
  timestamptz) returns table(reservation_id uuid, won boolean) language plpgsql
  security definer SET search_path = pg_catalog, public, pg_temp AS $$
declare
  v_reservation_id uuid;
begin
  if p_user_id is null
    or p_device_id is null
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
    raise exception 'invalid_authorization_reservation';
  end if;

  insert into public.oauth_authorization_reservations (
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
    p_connection_id,
    p_operation,
    p_expires_at
  )
  on conflict (nonce) do nothing
  returning id into v_reservation_id;

  if v_reservation_id is not null then
    return query select v_reservation_id, true;
    return;
  end if;

  select id into v_reservation_id
  from public.oauth_authorization_reservations
  where nonce = p_nonce;
  return query select v_reservation_id, false;
end;
$$;

revoke execute ON function public.reserve_oauth_authorization(uuid, uuid, uuid,
  text, uuid, timestamptz)
FROM public, anon, authenticated;
grant execute ON function public.reserve_oauth_authorization(uuid, uuid, uuid,
  text, uuid, timestamptz) TO service_role;

-- The winner records one server decision and closes its reservation. This is
-- intentionally separate from oauth_audit_events, which is never a lock.
CREATE function public.record_oauth_authorization_decision(p_reservation_id
  uuid, p_user_id uuid, p_decision text, p_denial_code text DEFAULT NULL)
  returns uuid language plpgsql security definer SET search_path = pg_catalog,
  public, pg_temp AS $$
declare
  v_reservation public.oauth_authorization_reservations%rowtype;
  v_decision_id uuid;
begin
  if p_decision not in ('allowed', 'denied')
    or (p_decision = 'denied' and p_denial_code is null)
    or (p_decision = 'allowed' and p_denial_code is not null) then
    raise exception 'invalid_authorization_decision';
  end if;

  select * into v_reservation
  from public.oauth_authorization_reservations
  where id = p_reservation_id
    and user_id = p_user_id
  for update;
  if not found or v_reservation.status <> 'reserved' then
    raise exception 'authorization_reservation_unavailable';
  end if;
  if v_reservation.expires_at <= pg_catalog.now() then
    update public.oauth_authorization_reservations
    set status = 'expired'
    where id = p_reservation_id;
    raise exception 'authorization_reservation_expired';
  end if;

  insert into public.oauth_authorization_decisions (
    reservation_id,
    user_id,
    decision,
    denial_code
  ) values (
    p_reservation_id,
    p_user_id,
    p_decision,
    p_denial_code
  )
  returning id into v_decision_id;

  update public.oauth_authorization_reservations
  set status = p_decision
  where id = p_reservation_id;
  return v_decision_id;
end;
$$;

revoke execute ON function public.record_oauth_authorization_decision(uuid,
  uuid, text, text)
FROM public, anon, authenticated;
grant execute ON function public.record_oauth_authorization_decision(uuid, uuid,
  text, text) TO service_role;

comment ON TABLE public.oauth_operation_grants IS
  'Operator-issued independent backup.write and backup.restore grants; connection state is not a grant.';
comment ON TABLE public.oauth_authorization_reservations IS
  'Durable unique nonce reservation. It is the replay lock; audit rows are not.';
comment ON TABLE public.oauth_authorization_decisions IS
  'Server-created authorization outcome linkage for a winning reservation.';
