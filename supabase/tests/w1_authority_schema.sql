-- W1-A-F4-S1 staging evidence only.
-- This file is intentionally read-only: it must be run after a reviewed,
-- transactional migration in the explicitly approved staging project.
-- It never applies migrations, grants, policies, or bootstrap approvals.

BEGIN;

do $$
declare
  v_table text;
  v_function text;
  v_config text[];
begin
  foreach v_table in array array[
    'public.device_enrollment_requests',
    'public.oauth_operation_grants',
    'public.oauth_authorization_reservations',
    'public.oauth_authorization_decisions'
  ] loop
    if not exists (
      select 1
      from pg_class c
      join pg_namespace n on n.oid = c.relnamespace
      where n.nspname || '.' || c.relname = v_table
        and c.relrowsecurity
    ) then
      raise exception 'W1 table is missing RLS: %', v_table;
    end if;
  end loop;

  if has_table_privilege('public', 'public.devices', 'INSERT, UPDATE, DELETE')
    or has_table_privilege('anon', 'public.devices', 'INSERT, UPDATE, DELETE')
    or has_table_privilege(
      'authenticated', 'public.devices', 'INSERT, UPDATE, DELETE'
    )
    or has_table_privilege(
      'service_role', 'public.devices', 'INSERT, UPDATE, DELETE'
    ) then
    raise exception 'direct device mutation remains granted';
  end if;

  foreach v_table in array array[
    'public.device_enrollment_requests',
    'public.oauth_authorization_reservations',
    'public.oauth_authorization_decisions'
  ] loop
    if has_table_privilege('public', v_table, 'SELECT, INSERT, UPDATE, DELETE')
      or has_table_privilege('anon', v_table, 'SELECT, INSERT, UPDATE, DELETE')
      or has_table_privilege(
        'authenticated', v_table, 'SELECT, INSERT, UPDATE, DELETE'
      )
      or has_table_privilege(
        'service_role', v_table, 'SELECT, INSERT, UPDATE, DELETE'
      ) then
      raise exception 'direct W1 table access remains granted: %', v_table;
    end if;
  end loop;

  if has_table_privilege(
      'public', 'public.oauth_operation_grants', 'SELECT, INSERT, UPDATE, DELETE'
    )
    or has_table_privilege(
      'anon', 'public.oauth_operation_grants', 'SELECT, INSERT, UPDATE, DELETE'
    )
    or has_table_privilege(
      'authenticated', 'public.oauth_operation_grants',
      'SELECT, INSERT, UPDATE, DELETE'
    )
    or has_table_privilege(
      'service_role', 'public.oauth_operation_grants', 'INSERT, UPDATE, DELETE'
    )
    or not has_table_privilege(
      'service_role', 'public.oauth_operation_grants', 'SELECT'
    ) then
    raise exception 'operation-grant table privilege posture is unsafe';
  end if;

  if has_function_privilege(
      'anon', 'public.approve_bootstrap_enrollment(uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.approve_bootstrap_enrollment(uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.approve_bootstrap_enrollment(uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'public', 'public.approve_bootstrap_enrollment(uuid)', 'EXECUTE'
    ) then
    raise exception 'database-owner-only bootstrap is exposed';
  end if;

  if has_function_privilege(
      'public', 'public.approve_rebind_enrollment(uuid, uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.approve_rebind_enrollment(uuid, uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.approve_rebind_enrollment(uuid, uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.approve_rebind_enrollment(uuid, uuid)', 'EXECUTE'
    ) then
    raise exception 'database-owner-only rebind is exposed';
  end if;

  if has_function_privilege(
      'public', 'public.is_drive_authorized_desktop(uuid, uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.is_drive_authorized_desktop(uuid, uuid)', 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.is_drive_authorized_desktop(uuid, uuid)',
      'EXECUTE'
    )
    or not has_function_privilege(
      'service_role', 'public.is_drive_authorized_desktop(uuid, uuid)',
      'EXECUTE'
    ) then
    raise exception 'Drive predicate function privilege posture is unsafe';
  end if;

  if has_function_privilege(
      'public',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)',
      'EXECUTE'
    )
    or has_function_privilege(
      'anon',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)',
      'EXECUTE'
    )
    or has_function_privilege(
      'authenticated',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)',
      'EXECUTE'
    )
    or not has_function_privilege(
      'service_role',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)',
      'EXECUTE'
    ) then
    raise exception 'atomic authorization function privilege posture is unsafe';
  end if;

  if has_function_privilege(
      'public', 'public.grant_oauth_operation(uuid, text)', 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.grant_oauth_operation(uuid, text)', 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.grant_oauth_operation(uuid, text)', 'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.grant_oauth_operation(uuid, text)', 'EXECUTE'
    )
    or has_function_privilege(
      'public', 'public.revoke_oauth_operation_grant(uuid, text)', 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.revoke_oauth_operation_grant(uuid, text)', 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.revoke_oauth_operation_grant(uuid, text)',
      'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.revoke_oauth_operation_grant(uuid, text)',
      'EXECUTE'
    ) then
    raise exception 'operator-only grant functions are exposed';
  end if;

  foreach v_function in array array[
    'public.create_device_enrollment_request(uuid, text, text, text, text, text)',
    'public.register_pairing_device(uuid, text, text, text, text)',
    'public.revoke_device_for_user(uuid, uuid)',
    'public.is_drive_authorized_desktop(uuid, uuid)',
    'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)',
    'public.approve_bootstrap_enrollment(uuid)',
    'public.approve_rebind_enrollment(uuid, uuid)',
    'public.create_pairing_session(uuid, text, uuid)',
    'public.confirm_pairing(uuid, text, uuid)',
    'public.handle_new_user()',
    'public.revoke_oauth_operation_grants_on_connection_change()',
    'public.revoke_oauth_operation_grants_on_device_change()',
    'public.grant_oauth_operation(uuid, text)',
    'public.revoke_oauth_operation_grant(uuid, text)'
  ] loop
    select p.proconfig
      into v_config
      from pg_proc p
      join pg_namespace n on n.oid = p.pronamespace
      where n.nspname || '.' || p.proname || '(' ||
        pg_get_function_identity_arguments(p.oid) || ')' = v_function;
    if v_config is null
      or not exists (
        select 1
        from unnest(v_config) config
        where config = 'search_path=pg_catalog, public, pg_temp'
      ) then
      raise exception 'fixed search_path missing for %', v_function;
    end if;
  end loop;

  if not exists (
    select 1
    from pg_constraint
    where conrelid = 'public.oauth_authorization_reservations'::regclass
      and contype = 'u'
      and pg_get_constraintdef(oid) ilike '%nonce%'
  ) then
    raise exception 'durable nonce uniqueness is missing';
  end if;

  if not exists (
    select 1
    from pg_proc
    where oid = 'public.authorize_oauth_request(uuid,uuid,text,text,text,uuid,timestamptz)'::regprocedure
      and pg_get_functiondef(oid) ilike '%on conflict%do nothing%returning%'
      and pg_get_functiondef(oid) ilike '%for update%'
      and pg_get_functiondef(oid) ilike '%oauth_authorization_decisions%'
  ) then
    raise exception 'atomic authorization transaction is missing';
  end if;
end;
$$;

-- Database-level adversarial evidence: a revoked exact-provider connection
-- must remain revoked when connection.activate is denied. All rows are seeded
-- inside this transaction and removed by the final ROLLBACK.
do $$
declare
  v_user_id uuid;
  v_device_id uuid;
  v_connection_id uuid;
  v_public_key text;
  v_fingerprint text;
  v_revoked_status text;
  v_revoked_at timestamptz;
  v_revoked_scopes text[];
  v_revoked_user_id uuid;
  v_revoked_grants jsonb;
  v_after_status text;
  v_after_revoked_at timestamptz;
  v_after_scopes text[];
  v_after_user_id uuid;
  v_after_grants jsonb;
  v_reservation_status text;
  v_decision text;
  v_denial_code text;
  v_result record;
begin
  select p.id
    into v_user_id
    from public.profiles p
   where not exists (
     select 1
       from public.oauth_connections c
      where c.user_id = p.id
        and c.provider = 'google_drive'
   )
   order by p.created_at
   limit 1;

  if v_user_id is null then
    raise exception 'W1 revoked activation evidence needs a profile without google_drive';
  end if;

  v_public_key := encode(convert_to(gen_random_uuid()::text, 'UTF8'), 'base64');
  v_fingerprint := encode(
    pg_catalog.sha256(pg_catalog.decode(v_public_key, 'base64')),
    'hex'
  );

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
    v_user_id,
    'W1 S1 F2 SQL evidence',
    'windows',
    v_fingerprint,
    v_public_key,
    'drive_trusted',
    'boss_bootstrap',
    pg_catalog.now(),
    pg_catalog.now()
  ) returning id into v_device_id;

  insert into public.oauth_connections (
    user_id,
    provider,
    approved_scopes,
    status,
    connected_at,
    last_authorized_at
  ) values (
    v_user_id,
    'google_drive',
    array['https://www.googleapis.com/auth/drive.appdata']::text[],
    'active',
    pg_catalog.now(),
    pg_catalog.now()
  ) returning id into v_connection_id;

  insert into public.oauth_operation_grants (
    user_id,
    connection_id,
    operation,
    granted_by,
    granted_role
  ) values
    (v_user_id, v_connection_id, 'backup.write',
      'w1_s1_f2_sql_evidence', 'database_owner'),
    (v_user_id, v_connection_id, 'backup.restore',
      'w1_s1_f2_sql_evidence', 'database_owner');

  update public.oauth_connections
     set status = 'revoked',
         revoked_at = pg_catalog.now()
   where id = v_connection_id;

  select c.status, c.revoked_at, c.approved_scopes, c.user_id
    into v_revoked_status, v_revoked_at, v_revoked_scopes, v_revoked_user_id
    from public.oauth_connections c
   where c.id = v_connection_id;

  select coalesce(jsonb_agg(to_jsonb(g) order by g.operation), '[]'::jsonb)
    into v_revoked_grants
    from public.oauth_operation_grants g
   where g.connection_id = v_connection_id;

  select *
    into v_result
    from public.authorize_oauth_request(
      v_user_id,
      v_device_id,
      v_public_key,
      v_fingerprint,
      'connection.activate',
      gen_random_uuid(),
      pg_catalog.now() + pg_catalog.make_interval(mins => 1)
    );

  if v_result.authorized is distinct from false
    or v_result.denial_code is distinct from 'connection_revoked'
    or v_result.connection_id is distinct from v_connection_id then
    raise exception 'revoked connection.activate was not denied';
  end if;

  select r.status, d.decision, d.denial_code
    into v_reservation_status, v_decision, v_denial_code
    from public.oauth_authorization_reservations r
    join public.oauth_authorization_decisions d
      on d.reservation_id = r.id
   where r.id = v_result.reservation_id;

  if v_reservation_status is distinct from 'denied'
    or v_decision is distinct from 'denied'
    or v_denial_code is distinct from 'connection_revoked' then
    raise exception 'revoked activation decision was not durably denied';
  end if;

  select c.status, c.revoked_at, c.approved_scopes, c.user_id
    into v_after_status, v_after_revoked_at, v_after_scopes, v_after_user_id
    from public.oauth_connections c
   where c.id = v_connection_id;

  select coalesce(jsonb_agg(to_jsonb(g) order by g.operation), '[]'::jsonb)
    into v_after_grants
    from public.oauth_operation_grants g
   where g.connection_id = v_connection_id;

  if v_after_status is distinct from v_revoked_status
    or v_after_revoked_at is distinct from v_revoked_at
    or v_after_scopes is distinct from v_revoked_scopes
    or v_after_user_id is distinct from v_revoked_user_id
    or v_after_grants is distinct from v_revoked_grants then
    raise exception 'denied revoked activation changed authoritative state';
  end if;
end;
$$;

-- Manual staging adversarial evidence, executed by the DB owner only:
-- run at least 50 identical authorize_oauth_request calls concurrently through
-- separate Edge workers and assert one allowed protected operation at most,
-- no post-lock revocation bypass, and no audit row used as the lock.

ROLLBACK;
