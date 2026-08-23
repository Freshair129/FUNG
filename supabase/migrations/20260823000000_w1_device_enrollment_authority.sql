-- W1-A-F4-S1: server-owned device authority and enrollment boundary.
-- This migration is project-agnostic. It contains no project reference and
-- must be applied only after a separately approved, read-only preflight.

DO $$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'devices'
      AND column_name = 'authority_state'
      AND (
        data_type <> 'text'
        OR is_nullable <> 'NO'
        OR COALESCE(column_default, '') NOT ILIKE '%legacy%'
      )
  ) THEN
    RAISE EXCEPTION 'incompatible public.devices.authority_state';
  END IF;
  IF EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'devices'
      AND column_name = 'enrollment_source'
      AND (
        data_type <> 'text'
        OR is_nullable <> 'NO'
        OR COALESCE(column_default, '') NOT ILIKE '%legacy%'
      )
  ) THEN
    RAISE EXCEPTION 'incompatible public.devices.enrollment_source';
  END IF;
  IF EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'devices'
      AND column_name IN ('enrolled_at', 'approved_at')
      AND (
        udt_name <> 'timestamptz'
        OR is_nullable <> 'YES'
        OR column_default IS NOT NULL
      )
  ) THEN
    RAISE EXCEPTION 'incompatible public.devices enrollment timestamp';
  END IF;
  IF EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'devices'
      AND column_name IN ('revoked_by', 'revocation_reason')
      AND (
        udt_name <> 'text'
        OR is_nullable <> 'YES'
        OR column_default IS NOT NULL
      )
  ) THEN
    RAISE EXCEPTION 'incompatible public.devices revocation metadata';
  END IF;
END;
$$;

ALTER TABLE public.devices ADD COLUMN IF NOT EXISTS authority_state text NOT
  NULL DEFAULT 'legacy', ADD COLUMN IF NOT EXISTS enrollment_source text NOT
  NULL DEFAULT 'legacy', ADD COLUMN IF NOT EXISTS enrolled_at timestamptz, ADD
  COLUMN IF NOT EXISTS approved_at timestamptz, ADD COLUMN IF NOT EXISTS
  revoked_by text, ADD COLUMN IF NOT EXISTS revocation_reason text;

-- The default above makes every pre-existing row legacy. This guarded update
-- keeps the invariant explicit without downgrading rows on a re-run.
UPDATE public.devices
SET authority_state = 'legacy', enrollment_source = 'legacy'
WHERE authority_state IS NULL OR enrollment_source IS NULL;

do $$
begin
  if exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_authority_state_values'
      and (
        contype <> 'c'
        or pg_get_constraintdef(oid) not ilike '%authority_state%'
        or pg_get_constraintdef(oid) not ilike '%legacy%'
        or pg_get_constraintdef(oid) not ilike '%pairing_only%'
        or pg_get_constraintdef(oid) not ilike '%drive_trusted%'
        or pg_get_constraintdef(oid) not ilike '%revoked%'
      )
  ) then
    raise exception 'incompatible devices_authority_state_values';
  end if;

  if exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_enrollment_source_values'
      and (
        contype <> 'c'
        or pg_get_constraintdef(oid) not ilike '%enrollment_source%'
        or pg_get_constraintdef(oid) not ilike '%legacy%'
        or pg_get_constraintdef(oid) not ilike '%pairing%'
        or pg_get_constraintdef(oid) not ilike '%boss_bootstrap%'
        or pg_get_constraintdef(oid) not ilike '%approved_rebind%'
      )
  ) then
    raise exception 'incompatible devices_enrollment_source_values';
  end if;

  if exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_revocation_state_consistent'
      and (
        contype <> 'c'
        or pg_get_constraintdef(oid) not ilike '%authority_state%'
        or pg_get_constraintdef(oid) not ilike '%revoked_at%'
      )
  ) then
    raise exception 'incompatible devices_revocation_state_consistent';
  end if;

  if exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_trusted_authority_shape'
      and (
        contype <> 'c'
        or pg_get_constraintdef(oid) not ilike '%drive_trusted%'
        or pg_get_constraintdef(oid) not ilike '%windows%'
        or pg_get_constraintdef(oid) not ilike '%boss_bootstrap%'
        or pg_get_constraintdef(oid) not ilike '%approved_rebind%'
        or pg_get_constraintdef(oid) not ilike '%revoked_at%'
        or pg_get_constraintdef(oid) not ilike '%public_key%'
        or pg_get_constraintdef(oid) not ilike '%enrolled_at%'
        or pg_get_constraintdef(oid) not ilike '%approved_at%'
      )
  ) then
    raise exception 'incompatible devices_trusted_authority_shape';
  end if;

  if not exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_authority_state_values'
  ) then
    alter table public.devices
      add constraint devices_authority_state_values
      check (authority_state in ('legacy', 'pairing_only', 'drive_trusted', 'revoked'));
  end if;

  if not exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_enrollment_source_values'
  ) then
    alter table public.devices
      add constraint devices_enrollment_source_values
      check (enrollment_source in ('legacy', 'pairing', 'boss_bootstrap', 'approved_rebind'));
  end if;

  if not exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_revocation_state_consistent'
  ) then
    alter table public.devices
      add constraint devices_revocation_state_consistent
      check (
        (authority_state = 'revoked' and revoked_at is not null)
        or authority_state <> 'revoked'
      );
  end if;

  if not exists (
    select 1 from pg_constraint
    where conrelid = 'public.devices'::regclass
      and conname = 'devices_trusted_authority_shape'
  ) then
    alter table public.devices
      add constraint devices_trusted_authority_shape
      check (
        authority_state <> 'drive_trusted'
        or (
          platform = 'windows'
          and enrollment_source in ('boss_bootstrap', 'approved_rebind')
          and revoked_at is null
          and public_key is not null
          and enrolled_at is not null
          and approved_at is not null
        )
      );
  end if;
end;
$$;

-- A pending request is not a device and is never directly client-readable.
CREATE TABLE public.device_enrollment_requests (id uuid PRIMARY KEY DEFAULT
  gen_random_uuid(), user_id uuid NOT NULL REFERENCES public.profiles (id) ON
  DELETE CASCADE, device_label text NOT NULL, platform text NOT NULL, public_key
  text NOT NULL, public_key_fingerprint text NOT NULL, native_proof text NOT
  NULL, requested_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz
  NOT NULL, status text NOT NULL DEFAULT 'pending', approved_by text,
  approved_role text, approved_at timestamptz, consumed_at timestamptz,
  consumed_device_id uuid REFERENCES public.devices (id) ON DELETE SET NULL,
  CONSTRAINT device_enrollment_requests_status CHECK (status IN ('pending',
  'approved', 'consumed', 'expired', 'rejected')), CONSTRAINT
  device_enrollment_requests_expiry CHECK (expires_at > requested_at),
  CONSTRAINT device_enrollment_requests_label_length CHECK
  (char_length(device_label) BETWEEN 1 AND 120), CONSTRAINT
  device_enrollment_requests_platform_length CHECK (char_length(platform)
  BETWEEN 1 AND 40), CONSTRAINT device_enrollment_requests_fingerprint_length
  CHECK (char_length(public_key_fingerprint) = 64), CONSTRAINT
  device_enrollment_requests_proof_length CHECK (char_length(native_proof)
  BETWEEN 1 AND 8192));

CREATE UNIQUE INDEX device_enrollment_requests_active_identity_idx ON
  public.device_enrollment_requests (user_id, public_key_fingerprint)
WHERE status IN ('pending', 'approved', 'consumed');

CREATE INDEX device_enrollment_requests_user_expiry_idx ON
  public.device_enrollment_requests (user_id, expires_at DESC);

ALTER TABLE public.device_enrollment_requests enable ROW level security;
revoke ALL ON TABLE public.device_enrollment_requests
FROM public, anon, authenticated, service_role;

-- Direct table mutation is no longer an enrollment or revocation path. The
-- Edge server may read authoritative rows, but it cannot mutate them directly.
DROP policy IF EXISTS "devices: user registers own device" ON public.devices;
DROP policy IF EXISTS "devices: user updates own device" ON public.devices;
DROP policy IF EXISTS "devices: user removes own device" ON public.devices;
ALTER TABLE public.devices enable ROW level security;
revoke ALL ON TABLE public.devices
FROM public, anon, authenticated, service_role;
grant
SELECT ON TABLE public.devices TO authenticated, service_role;

-- The exact Drive predicate is server-owned and deliberately excludes legacy,
-- pairing-only, revoked, Android, FUNGWIRE, and Genesis identities.
CREATE function public.is_drive_authorized_desktop(p_user_id uuid, p_device_id
  uuid) returns boolean language sql stable SET search_path = pg_catalog,
  public, pg_temp AS $$
  select exists (
    select 1
    from public.devices d
    where d.id = p_device_id
      and d.user_id = p_user_id
      and d.platform = 'windows'
      and d.authority_state = 'drive_trusted'
      and d.enrollment_source in ('boss_bootstrap', 'approved_rebind')
      and d.revoked_at is null
      and d.public_key is not null
      and pg_catalog.encode(
        pg_catalog.sha256(pg_catalog.decode(d.public_key, 'base64')),
        'hex'
      ) = d.public_key_fingerprint
  );
$$;

revoke execute ON function public.is_drive_authorized_desktop(uuid, uuid)
FROM public, anon, authenticated;
grant execute ON function public.is_drive_authorized_desktop(uuid, uuid) TO
  service_role;

-- Server-only creation of a non-authoritative pending request. The verified
-- Edge session supplies p_user_id; no caller can mark the request trusted.
CREATE function public.create_device_enrollment_request(p_user_id uuid,
  p_device_label text, p_platform text, p_public_key text,
  p_public_key_fingerprint text, p_native_proof text) returns table(request_id
  uuid, request_status text, request_expires_at timestamptz) language plpgsql
  security definer SET search_path = pg_catalog, public, pg_temp AS $$
begin
  if p_user_id is null
    or p_device_label is null
    or char_length(p_device_label) not between 1 and 120
    or p_platform is null
    or char_length(p_platform) not between 1 and 40
    or p_public_key is null
    or p_public_key_fingerprint is null
    or p_public_key_fingerprint !~ '^[0-9a-f]{64}$'
    or p_native_proof is null
    or char_length(p_native_proof) not between 1 and 8192 then
    raise exception 'invalid_enrollment_request';
  end if;

  if exists (
    select 1
    from public.devices d
    where d.user_id = p_user_id
      and d.public_key_fingerprint = p_public_key_fingerprint
  ) then
    raise exception 'device_identity_already_registered';
  end if;

  return query
  insert into public.device_enrollment_requests (
    user_id,
    device_label,
    platform,
    public_key,
    public_key_fingerprint,
    native_proof,
    expires_at
  ) values (
    p_user_id,
    p_device_label,
    p_platform,
    p_public_key,
    p_public_key_fingerprint,
    p_native_proof,
    pg_catalog.now() + pg_catalog.make_interval(mins => 5)
  )
  returning id, status, expires_at;
end;
$$;

revoke execute ON function public.create_device_enrollment_request(uuid, text,
  text, text, text, text)
FROM public, anon, authenticated;
grant execute ON function public.create_device_enrollment_request(uuid, text,
  text, text, text, text) TO service_role;

-- Server-only pairing registration. It can create or refresh pairing_only
-- metadata, but it never creates a drive_trusted row or resurrects a revoked
-- identity. A trusted key is never mutated in place.
CREATE function public.register_pairing_device(p_user_id uuid, p_device_label
  text, p_platform text, p_public_key text, p_public_key_fingerprint text)
  returns uuid language plpgsql security definer SET search_path = pg_catalog,
  public, pg_temp AS $$
declare
  v_device public.devices%rowtype;
  v_device_id uuid;
begin
  if p_user_id is null
    or p_device_label is null
    or char_length(p_device_label) not between 1 and 120
    or p_platform is null
    or char_length(p_platform) not between 1 and 40
    or p_public_key is null
    or p_public_key_fingerprint is null
    or p_public_key_fingerprint !~ '^[0-9a-f]{64}$' then
    raise exception 'invalid_pairing_device';
  end if;

  select * into v_device
  from public.devices
  where user_id = p_user_id
    and public_key_fingerprint = p_public_key_fingerprint
  for update;

  if found then
    if v_device.revoked_at is not null
      or v_device.authority_state = 'revoked' then
      raise exception 'device_revoked';
    end if;
    if v_device.authority_state = 'drive_trusted' then
      raise exception 'trusted_device_rebind_required';
    end if;

    update public.devices
    set device_label = p_device_label,
        platform = p_platform,
        public_key = p_public_key,
        last_seen_at = pg_catalog.now(),
        authority_state = 'pairing_only',
        enrollment_source = 'pairing'
    where id = v_device.id;
    return v_device.id;
  end if;

  insert into public.devices (
    user_id,
    device_label,
    platform,
    public_key_fingerprint,
    public_key,
    authority_state,
    enrollment_source
  ) values (
    p_user_id,
    p_device_label,
    p_platform,
    p_public_key_fingerprint,
    p_public_key,
    'pairing_only',
    'pairing'
  )
  returning id into v_device_id;

  return v_device_id;
end;
$$;

revoke execute ON function public.register_pairing_device(uuid, text, text,
  text, text)
FROM public, anon, authenticated;
grant execute ON function public.register_pairing_device(uuid, text, text, text,
  text) TO service_role;

-- Server-owned soft revocation. The row remains for audit and uniqueness;
-- no client or Edge role receives direct UPDATE/DELETE on devices.
CREATE function public.revoke_device_for_user(p_user_id uuid, p_device_id uuid)
  returns boolean language plpgsql security definer SET search_path =
  pg_catalog, public, pg_temp AS $$
begin
  update public.devices
  set revoked_at = coalesce(revoked_at, pg_catalog.now()),
      revoked_by = session_user,
      authority_state = 'revoked'
  where id = p_device_id
    and user_id = p_user_id
    and revoked_at is null;
  return found;
end;
$$;

revoke execute ON function public.revoke_device_for_user(uuid, uuid)
FROM public, anon, authenticated;
grant execute ON function public.revoke_device_for_user(uuid, uuid) TO
  service_role;

-- Manual Boss ceremony. The request carries the candidate identity; this
-- function accepts only its immutable request id and has no client execute
-- grant. The database owner verifies the request out-of-band before calling.
CREATE function public.approve_bootstrap_enrollment(p_request_id uuid) returns
  uuid language plpgsql security definer SET search_path = pg_catalog, public,
  pg_temp AS $$
declare
  v_request public.device_enrollment_requests%rowtype;
  v_device_id uuid;
  v_fingerprint text;
begin
  if session_user <> (
    select pg_catalog.pg_get_userbyid(datdba)
    from pg_catalog.pg_database
    where datname = pg_catalog.current_database()
  ) then
    raise exception 'database_owner_required';
  end if;

  select * into v_request
  from public.device_enrollment_requests
  where id = p_request_id
  for update;

  if not found then
    raise exception 'enrollment_request_not_found';
  end if;
  if v_request.status <> 'pending' then
    raise exception 'enrollment_request_not_pending';
  end if;
  if v_request.expires_at <= pg_catalog.now() then
    raise exception 'enrollment_request_expired';
  end if;
  if v_request.platform <> 'windows' then
    raise exception 'drive_bootstrap_requires_windows';
  end if;

  begin
    v_fingerprint := pg_catalog.encode(
      pg_catalog.sha256(pg_catalog.decode(v_request.public_key, 'base64')),
      'hex'
    );
  exception when others then
    raise exception 'enrollment_public_key_invalid';
  end;
  if v_fingerprint <> v_request.public_key_fingerprint then
    raise exception 'enrollment_fingerprint_mismatch';
  end if;

  if exists (
    select 1
    from public.devices d
    where d.user_id = v_request.user_id
      and d.public_key_fingerprint = v_request.public_key_fingerprint
  ) then
    raise exception 'enrollment_identity_reused';
  end if;

  insert into public.devices (
    user_id,
    device_label,
    platform,
    public_key_fingerprint,
    public_key,
    authority_state,
    enrollment_source,
    enrolled_at,
    approved_at
  ) values (
    v_request.user_id,
    v_request.device_label,
    v_request.platform,
    v_request.public_key_fingerprint,
    v_request.public_key,
    'drive_trusted',
    'boss_bootstrap',
    pg_catalog.now(),
    pg_catalog.now()
  )
  returning id into v_device_id;

  update public.device_enrollment_requests
  set status = 'consumed',
      approved_by = session_user,
      approved_role = 'database_owner',
      approved_at = pg_catalog.now(),
      consumed_at = pg_catalog.now(),
      consumed_device_id = v_device_id
  where id = p_request_id;

  return v_device_id;
end;
$$;

revoke execute ON function public.approve_bootstrap_enrollment(uuid)
FROM public, anon, authenticated, service_role;

-- Manual Boss rebind ceremony. The old trusted row is soft-revoked and the
-- newly approved request receives a distinct identity; no trusted key is
-- mutated in place. The database-owner check is repeated inside the definer.
CREATE function public.approve_rebind_enrollment(p_request_id uuid,
  p_old_device_id uuid) returns uuid language plpgsql security definer SET
  search_path = pg_catalog, public, pg_temp AS $$
declare
  v_request public.device_enrollment_requests%rowtype;
  v_old_device public.devices%rowtype;
  v_device_id uuid;
  v_fingerprint text;
begin
  if session_user <> (
    select pg_catalog.pg_get_userbyid(datdba)
    from pg_catalog.pg_database
    where datname = pg_catalog.current_database()
  ) then
    raise exception 'database_owner_required';
  end if;

  select * into v_request
  from public.device_enrollment_requests
  where id = p_request_id
  for update;

  if not found then
    raise exception 'enrollment_request_not_found';
  end if;
  if v_request.status <> 'pending' then
    raise exception 'enrollment_request_not_pending';
  end if;
  if v_request.expires_at <= pg_catalog.now() then
    raise exception 'enrollment_request_expired';
  end if;
  if v_request.platform <> 'windows' then
    raise exception 'drive_rebind_requires_windows';
  end if;

  select * into v_old_device
  from public.devices
  where id = p_old_device_id
    and user_id = v_request.user_id
  for update;

  if not found
    or v_old_device.authority_state <> 'drive_trusted'
    or v_old_device.revoked_at is not null then
    raise exception 'trusted_device_not_available_for_rebind';
  end if;

  begin
    v_fingerprint := pg_catalog.encode(
      pg_catalog.sha256(pg_catalog.decode(v_request.public_key, 'base64')),
      'hex'
    );
  exception when others then
    raise exception 'enrollment_public_key_invalid';
  end;
  if v_fingerprint <> v_request.public_key_fingerprint then
    raise exception 'enrollment_fingerprint_mismatch';
  end if;

  if exists (
    select 1
    from public.devices d
    where d.user_id = v_request.user_id
      and d.public_key_fingerprint = v_request.public_key_fingerprint
  ) then
    raise exception 'enrollment_identity_reused';
  end if;

  update public.devices
  set revoked_at = pg_catalog.now(),
      revoked_by = session_user,
      authority_state = 'revoked',
      revocation_reason = 'approved_rebind'
  where id = v_old_device.id
    and revoked_at is null;
  if not found then
    raise exception 'trusted_device_rebind_raced';
  end if;

  insert into public.devices (
    user_id,
    device_label,
    platform,
    public_key_fingerprint,
    public_key,
    authority_state,
    enrollment_source,
    enrolled_at,
    approved_at
  ) values (
    v_request.user_id,
    v_request.device_label,
    v_request.platform,
    v_request.public_key_fingerprint,
    v_request.public_key,
    'drive_trusted',
    'approved_rebind',
    pg_catalog.now(),
    pg_catalog.now()
  )
  returning id into v_device_id;

  update public.device_enrollment_requests
  set status = 'consumed',
      approved_by = session_user,
      approved_role = 'database_owner',
      approved_at = pg_catalog.now(),
      consumed_at = pg_catalog.now(),
      consumed_device_id = v_device_id
  where id = p_request_id;

  return v_device_id;
end;
$$;

revoke execute ON function public.approve_rebind_enrollment(uuid, uuid)
FROM public, anon, authenticated, service_role;

-- Pairing remains account-owned but non-authoritative. A pairing session can
-- only use caller-owned, non-revoked device IDs and never promotes authority.
CREATE OR replace function public.create_pairing_session(p_session_id uuid,
  p_code_hash text, p_initiator_device_id uuid) returns void language plpgsql
  security definer SET search_path = pg_catalog, public, pg_temp AS $$
declare
  v_user_id uuid := (select auth.uid());
begin
  if v_user_id is null then
    raise exception 'unauthenticated';
  end if;
  if not exists (
    select 1
    from public.devices d
    where d.id = p_initiator_device_id
      and d.user_id = v_user_id
      and d.revoked_at is null
      and d.authority_state <> 'revoked'
  ) then
    raise exception 'initiator_device_not_owned_or_revoked';
  end if;

  delete from public.pairing_sessions
  where user_id = v_user_id
    and expires_at < pg_catalog.now() - pg_catalog.make_interval(days => 1);

  update public.devices
  set authority_state = case
        when authority_state = 'legacy' then 'pairing_only'
        else authority_state
      end,
      enrollment_source = case
        when authority_state = 'legacy' then 'pairing'
        else enrollment_source
      end
  where id = p_initiator_device_id
    and user_id = v_user_id
    and revoked_at is null;

  insert into public.pairing_sessions (id, user_id, initiator_device_id, code_hash)
  values (p_session_id, v_user_id, p_initiator_device_id, p_code_hash);
end;
$$;

CREATE OR replace function public.confirm_pairing(p_session_id uuid, p_code
  text, p_responder_device_id uuid) returns text language plpgsql security
  definer SET search_path = pg_catalog, public, pg_temp AS $$
declare
  v_session public.pairing_sessions%rowtype;
  v_user_id uuid := (select auth.uid());
begin
  if v_user_id is null then
    raise exception 'unauthenticated';
  end if;

  select * into v_session
  from public.pairing_sessions
  where id = p_session_id
  for update;

  if not found or v_session.user_id <> v_user_id then
    return 'not_found';
  end if;
  if not exists (
    select 1
    from public.devices d
    where d.id = p_responder_device_id
      and d.user_id = v_user_id
      and d.revoked_at is null
      and d.authority_state <> 'revoked'
  ) then
    return 'not_found';
  end if;
  if v_session.status = 'locked' then
    return 'locked';
  end if;
  if v_session.status <> 'pending' then
    if v_session.status = 'confirmed' then return 'already_confirmed'; end if;
    return v_session.status;
  end if;
  if v_session.expires_at < pg_catalog.now() then
    update public.pairing_sessions
    set status = 'expired'
    where id = p_session_id;
    return 'expired';
  end if;

  if v_session.code_hash = encode(
    pg_catalog.sha256((p_session_id::text || ':' || p_code)::bytea),
    'hex'
  ) then
    update public.pairing_sessions
    set status = 'confirmed',
        confirmed_at = pg_catalog.now(),
        responder_device_id = p_responder_device_id
    where id = p_session_id;

    update public.devices
    set authority_state = case
          when authority_state = 'legacy' then 'pairing_only'
          else authority_state
        end,
        enrollment_source = case
          when authority_state = 'legacy' then 'pairing'
          else enrollment_source
        end
    where id = p_responder_device_id
      and user_id = v_user_id
      and revoked_at is null;
    return 'confirmed';
  end if;

  update public.pairing_sessions
  set attempt_count = attempt_count + 1,
      status = case when attempt_count + 1 >= 5 then 'locked' else status end
  where id = p_session_id;
  return case when v_session.attempt_count + 1 >= 5 then 'locked' else 'wrong_code' end;
end;
$$;

revoke execute ON function public.create_pairing_session(uuid, text, uuid)
FROM public, anon, service_role;
grant execute ON function public.create_pairing_session(uuid, text, uuid) TO
  authenticated;
revoke execute ON function public.confirm_pairing(uuid, text, uuid)
FROM public, anon, service_role;
grant execute ON function public.confirm_pairing(uuid, text, uuid) TO
  authenticated;

-- The profile trigger is a trusted Auth trigger, not a Data API function.
CREATE OR replace function public.handle_new_user() returns trigger language
  plpgsql security definer SET search_path = pg_catalog, public, pg_temp AS $$
begin
  insert into public.profiles (id, display_name)
  values (
    new.id,
    coalesce(
      new.raw_user_meta_data->>'full_name',
      new.raw_user_meta_data->>'name',
      'User'
    )
  );

  if new.raw_app_meta_data->>'provider' = 'google' then
    insert into public.oauth_connections (user_id, provider, status, approved_scopes)
    values (new.id, 'google', 'active', array['openid', 'email', 'profile'])
    on conflict (user_id, provider) do update
    set status = 'active', last_authorized_at = pg_catalog.now();
  end if;

  return new;
end;
$$;

revoke execute ON function public.handle_new_user()
FROM public, anon, authenticated, service_role;

comment ON COLUMN public.devices.authority_state IS
  'Server-controlled authority class: legacy, pairing_only, drive_trusted, or revoked.';
comment ON COLUMN public.devices.enrollment_source IS
  'Server-controlled source. Only boss_bootstrap or approved_rebind can satisfy Drive authority.';
comment ON TABLE public.device_enrollment_requests IS
  'Non-authoritative pending requests; never a trusted device or Drive grant.';
