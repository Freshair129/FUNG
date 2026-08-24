-- W1-A-F4-S1 staging evidence only.
-- This file is intentionally read-only: it must be run after a reviewed,
-- transactional migration in the explicitly approved staging project.
-- It never applies migrations, grants, policies, or bootstrap approvals.

BEGIN;

do $$
declare
  v_table text;
  v_function regprocedure;
  v_config text[];
begin
  foreach v_table in array array[
    'public.device_enrollment_requests',
    'public.device_enrollment_proof_reservations',
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
    'public.device_enrollment_proof_reservations',
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
      'anon', 'public.approve_bootstrap_enrollment(uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.approve_bootstrap_enrollment(uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.approve_bootstrap_enrollment(uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'public', 'public.approve_bootstrap_enrollment(uuid)'::regprocedure, 'EXECUTE'
    ) then
    raise exception 'database-owner-only bootstrap is exposed';
  end if;

  if has_function_privilege(
      'public', 'public.approve_rebind_enrollment(uuid, uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.approve_rebind_enrollment(uuid, uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.approve_rebind_enrollment(uuid, uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.approve_rebind_enrollment(uuid, uuid)'::regprocedure, 'EXECUTE'
    ) then
    raise exception 'database-owner-only rebind is exposed';
  end if;

  if has_function_privilege(
      'public', 'public.is_drive_authorized_desktop(uuid, uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.is_drive_authorized_desktop(uuid, uuid)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.is_drive_authorized_desktop(uuid, uuid)'::regprocedure,
      'EXECUTE'
    )
    or not has_function_privilege(
      'service_role', 'public.is_drive_authorized_desktop(uuid, uuid)'::regprocedure,
      'EXECUTE'
    ) then
    raise exception 'Drive predicate function privilege posture is unsafe';
  end if;

  if has_function_privilege(
      'public',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)'::regprocedure,
      'EXECUTE'
    )
    or has_function_privilege(
      'anon',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)'::regprocedure,
      'EXECUTE'
    )
    or has_function_privilege(
      'authenticated',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)'::regprocedure,
      'EXECUTE'
    )
    or not has_function_privilege(
      'service_role',
      'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)'::regprocedure,
      'EXECUTE'
    ) then
    raise exception 'atomic authorization function privilege posture is unsafe';
  end if;

  if has_function_privilege(
      'public', 'public.grant_oauth_operation(uuid, text)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.grant_oauth_operation(uuid, text)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.grant_oauth_operation(uuid, text)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.grant_oauth_operation(uuid, text)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'public', 'public.revoke_oauth_operation_grant(uuid, text)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'anon', 'public.revoke_oauth_operation_grant(uuid, text)'::regprocedure, 'EXECUTE'
    )
    or has_function_privilege(
      'authenticated', 'public.revoke_oauth_operation_grant(uuid, text)'::regprocedure,
      'EXECUTE'
    )
    or has_function_privilege(
      'service_role', 'public.revoke_oauth_operation_grant(uuid, text)'::regprocedure,
      'EXECUTE'
    ) then
    raise exception 'operator-only grant functions are exposed';
  end if;

  if has_function_privilege(
      'public',
      'public.create_device_enrollment_request(uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text)'::regprocedure,
      'EXECUTE'
    )
    or has_function_privilege(
      'anon',
      'public.create_device_enrollment_request(uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text)'::regprocedure,
      'EXECUTE'
    )
    or has_function_privilege(
      'authenticated',
      'public.create_device_enrollment_request(uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text)'::regprocedure,
      'EXECUTE'
    )
    or not has_function_privilege(
      'service_role',
      'public.create_device_enrollment_request(uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text)'::regprocedure,
      'EXECUTE'
    ) then
    raise exception 'enrollment proof function privilege posture is unsafe';
  end if;

  foreach v_function in array ARRAY[
    'public.create_device_enrollment_request(uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text)'::regprocedure,
    'public.register_pairing_device(uuid, text, text, text, text)'::regprocedure,
    'public.revoke_device_for_user(uuid, uuid)'::regprocedure,
    'public.is_drive_authorized_desktop(uuid, uuid)'::regprocedure,
    'public.authorize_oauth_request(uuid, uuid, text, text, text, uuid, timestamptz)'::regprocedure,
    'public.approve_bootstrap_enrollment(uuid)'::regprocedure,
    'public.approve_rebind_enrollment(uuid, uuid)'::regprocedure,
    'public.create_pairing_session(uuid, text, uuid)'::regprocedure,
    'public.confirm_pairing(uuid, text, uuid)'::regprocedure,
    'public.handle_new_user()'::regprocedure,
    'public.revoke_oauth_operation_grants_on_connection_change()'::regprocedure,
    'public.revoke_oauth_operation_grants_on_device_change()'::regprocedure,
    'public.grant_oauth_operation(uuid, text)'::regprocedure,
    'public.revoke_oauth_operation_grant(uuid, text)'::regprocedure
  ] loop
    select p.proconfig
      into v_config
      from pg_catalog.pg_proc p
      where p.oid = v_function::oid;
    if v_config is null
      or not exists (
        select 1
        from unnest(v_config) config
        where config = 'search_path=pg_catalog, public, pg_temp'
      ) then
      raise exception 'fixed search_path missing for %', v_function::text;
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

  if not exists (
    select 1
    from pg_proc
    where oid = 'public.create_device_enrollment_request(uuid,text,text,text,text,integer,text,text,bigint,bigint,text,text)'::regprocedure
      and pg_get_functiondef(oid) ilike '%on conflict%nonce_hash%do nothing%returning%'
      and pg_get_functiondef(oid) ilike '%proof_replayed%'
      and pg_get_functiondef(oid) ilike '%device_enrollment_proof_reservations%'
  ) then
    raise exception 'atomic enrollment proof reservation is missing';
  end if;
end;
$$;

-- Database-level adversarial evidence for the exact S2-F2 proof path. Every
-- row is inside this transaction and disappears with the final ROLLBACK.
do $$
declare
  v_user_id uuid := '00000000-0000-0000-0000-000000000001';
  v_public_bytes bytea := pg_catalog.decode(repeat('ab', 32), 'hex');
  v_public_key text;
  v_fingerprint text;
  v_nonce bytea := pg_catalog.decode(repeat('cd', 32), 'hex');
  v_nonce_hash text;
  v_envelope_hash text := encode(pg_catalog.decode(repeat('ef', 32), 'hex'), 'hex');
  v_signature text := repeat('12', 64);
  v_issued_at_ms bigint := floor(extract(epoch from clock_timestamp()) * 1000)::bigint;
  v_expires_at_ms bigint;
  v_request record;
  v_count bigint;
  v_replay_nonce bytea := pg_catalog.decode(repeat('34', 32), 'hex');
  v_replay_nonce_hash text;
  v_wrong_field_nonce_hash text := encode(
    pg_catalog.sha256(pg_catalog.decode(repeat('56', 32), 'hex')), 'hex'
  );
  v_expired_nonce_hash text := encode(
    pg_catalog.sha256(pg_catalog.decode(repeat('67', 32), 'hex')), 'hex'
  );
  v_skew_nonce_hash text := encode(
    pg_catalog.sha256(pg_catalog.decode(repeat('78', 32), 'hex')), 'hex'
  );
  v_tamper_nonce_hash text := encode(
    pg_catalog.sha256(pg_catalog.decode(repeat('89', 32), 'hex')), 'hex'
  );
  v_foreign_nonce_hash text := encode(
    pg_catalog.sha256(pg_catalog.decode(repeat('9a', 32), 'hex')), 'hex'
  );
begin
  v_public_key := encode(v_public_bytes, 'base64');
  v_fingerprint := encode(pg_catalog.sha256(v_public_bytes), 'hex');
  v_nonce_hash := encode(pg_catalog.sha256(v_nonce), 'hex');
  v_replay_nonce_hash := encode(pg_catalog.sha256(v_replay_nonce), 'hex');
  v_expires_at_ms := v_issued_at_ms + 300000;

  select * into v_request
  from public.create_device_enrollment_request(
    v_user_id,
    'W1 S2-F2 SQL proof',
    'windows',
    v_public_key,
    v_fingerprint,
    1,
    'device.enrollment.request',
    v_nonce_hash,
    v_issued_at_ms,
    v_expires_at_ms,
    v_envelope_hash,
    v_signature
  );
  if v_request.request_status is distinct from 'pending'
    or v_request.request_id is null then
    raise exception 'valid enrollment proof was not pending';
  end if;

  select count(*) into v_count
  from public.device_enrollment_proof_reservations
  where nonce_hash = pg_catalog.decode(v_nonce_hash, 'hex');
  if v_count <> 1 then
    raise exception 'valid enrollment proof did not reserve nonce';
  end if;

  begin
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 S2-F2 SQL proof', 'windows', v_public_key,
      v_fingerprint, 1, 'device.enrollment.request', v_nonce_hash,
      v_issued_at_ms, v_expires_at_ms, v_envelope_hash, v_signature
    );
    raise exception 'replayed enrollment proof was accepted';
  exception when others then
    if sqlerrm <> 'proof_replayed' then
      raise;
    end if;
  end;

  select count(*) into v_count
  from public.device_enrollment_proof_reservations
  where nonce_hash = pg_catalog.decode(v_nonce_hash, 'hex');
  if v_count <> 1 then
    raise exception 'replay mutated durable nonce reservation';
  end if;

  begin
    insert into public.devices (
      user_id, device_label, platform, public_key_fingerprint, public_key,
      authority_state, enrollment_source
    ) values (
      v_user_id, 'W1 S2-F2 rollback identity', 'windows', v_fingerprint,
      v_public_key, 'pairing_only', 'pairing'
    );
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 S2-F2 rollback proof', 'windows', v_public_key,
      v_fingerprint, 1, 'device.enrollment.request', v_replay_nonce_hash,
      v_issued_at_ms, v_expires_at_ms, v_envelope_hash, v_signature
    );
    raise exception 'foreign device identity was accepted';
  exception when others then
    if sqlerrm <> 'device_identity_already_registered' then
      raise;
    end if;
  end;

  select count(*) into v_count
  from public.device_enrollment_proof_reservations
  where nonce_hash = pg_catalog.decode(v_replay_nonce_hash, 'hex');
  if v_count <> 0 then
    raise exception 'failed identity validation retained a nonce reservation';
  end if;

  begin
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 wrong version', 'windows', v_public_key, v_fingerprint,
      2, 'device.enrollment.request', v_wrong_field_nonce_hash,
      v_issued_at_ms, v_expires_at_ms, v_envelope_hash, v_signature
    );
    raise exception 'wrong proof version was accepted';
  exception when others then
    if sqlerrm <> 'invalid_enrollment_proof' then
      raise;
    end if;
  end;

  begin
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 wrong platform', 'linux', v_public_key, v_fingerprint,
      1, 'device.enrollment.request', v_wrong_field_nonce_hash,
      v_issued_at_ms, v_expires_at_ms, v_envelope_hash, v_signature
    );
    raise exception 'wrong proof platform was accepted';
  exception when others then
    if sqlerrm <> 'invalid_enrollment_proof' then
      raise;
    end if;
  end;

  begin
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 expired proof', 'windows', v_public_key, v_fingerprint,
      1, 'device.enrollment.request', v_expired_nonce_hash,
      v_issued_at_ms, v_issued_at_ms - 1, v_envelope_hash, v_signature
    );
    raise exception 'expired proof was accepted';
  exception when others then
    if sqlerrm <> 'invalid_enrollment_proof' then
      raise;
    end if;
  end;

  begin
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 future proof', 'windows', v_public_key, v_fingerprint,
      1, 'device.enrollment.request', v_skew_nonce_hash,
      v_issued_at_ms + 60000, v_issued_at_ms + 360000,
      v_envelope_hash, v_signature
    );
    raise exception 'future-skewed proof was accepted';
  exception when others then
    if sqlerrm <> 'invalid_enrollment_proof' then
      raise;
    end if;
  end;

  begin
    perform public.create_device_enrollment_request(
      v_user_id, 'W1 tampered key', 'windows',
      encode(pg_catalog.decode(repeat('ac', 32), 'hex'), 'base64'),
      v_fingerprint, 1, 'device.enrollment.request', v_tamper_nonce_hash,
      v_issued_at_ms, v_expires_at_ms, v_envelope_hash, v_signature
    );
    raise exception 'tampered key proof was accepted';
  exception when others then
    if sqlerrm <> 'invalid_enrollment_proof' then
      raise;
    end if;
  end;

  begin
    perform public.create_device_enrollment_request(
      '00000000-0000-0000-0000-000000000002', 'W1 foreign profile', 'windows',
      v_public_key, v_fingerprint, 1, 'device.enrollment.request',
      v_foreign_nonce_hash, v_issued_at_ms, v_expires_at_ms,
      v_envelope_hash, v_signature
    );
    raise exception 'foreign profile proof was accepted';
  exception when foreign_key_violation then
    null;
  end;

  select count(*) into v_count
  from public.device_enrollment_proof_reservations
  where nonce_hash in (
    pg_catalog.decode(v_wrong_field_nonce_hash, 'hex'),
    pg_catalog.decode(v_expired_nonce_hash, 'hex'),
    pg_catalog.decode(v_skew_nonce_hash, 'hex'),
    pg_catalog.decode(v_tamper_nonce_hash, 'hex'),
    pg_catalog.decode(v_foreign_nonce_hash, 'hex')
  );
  if v_count <> 0 then
    raise exception 'invalid proof path mutated nonce reservations';
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
  v_nonce uuid := gen_random_uuid();
  v_replay_result record;
  v_reservation_count integer;
  v_decision_count integer;
  v_after_status text;
  v_after_revoked_at timestamptz;
  v_after_scopes text[];
  v_after_user_id uuid;
  v_after_grants jsonb;
  v_reservation_status text;
  v_decision text;
  v_denial_code text;
  v_result record;
  v_activation_result record;
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

  select *
    into v_result
    from public.authorize_oauth_request(
      v_user_id,
      v_device_id,
      v_public_key,
      v_fingerprint,
      'backup.write',
      v_nonce,
      pg_catalog.now() + pg_catalog.make_interval(mins => 1)
    );

  if v_result.authorized is distinct from true
    or v_result.denial_code is not null
    or v_result.connection_id is distinct from v_connection_id then
    raise exception 'active backup.write was not allowed';
  end if;

  select count(*)
    into v_reservation_count
    from public.oauth_authorization_reservations r
   where r.nonce = v_nonce;
  select count(*)
    into v_decision_count
    from public.oauth_authorization_decisions d
   where d.reservation_id = v_result.reservation_id;
  if v_reservation_count <> 1 or v_decision_count <> 1 then
    raise exception 'active backup.write did not durably reserve and decide';
  end if;

  select *
    into v_replay_result
    from public.authorize_oauth_request(
      v_user_id,
      v_device_id,
      v_public_key,
      v_fingerprint,
      'backup.write',
      v_nonce,
      pg_catalog.now() + pg_catalog.make_interval(mins => 1)
    );

  if v_replay_result.authorized is distinct from false
    or v_replay_result.denial_code is distinct from 'authorization_replayed'
    or v_replay_result.reservation_id is distinct from v_result.reservation_id then
    raise exception 'repeated nonce was not durably rejected';
  end if;

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
    into v_activation_result
    from public.authorize_oauth_request(
      v_user_id,
      v_device_id,
      v_public_key,
      v_fingerprint,
      'connection.activate',
      gen_random_uuid(),
      pg_catalog.now() + pg_catalog.make_interval(mins => 1)
    );

  if v_activation_result.authorized is distinct from false
    or v_activation_result.denial_code is distinct from 'connection_revoked'
    or v_activation_result.connection_id is distinct from v_connection_id then
    raise exception 'revoked connection.activate was not denied';
  end if;

  select r.status, d.decision, d.denial_code
    into v_reservation_status, v_decision, v_denial_code
    from public.oauth_authorization_reservations r
    join public.oauth_authorization_decisions d
      on d.reservation_id = r.id
   where r.id = v_activation_result.reservation_id;

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
