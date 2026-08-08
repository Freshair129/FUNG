-- Pairing sessions: short-lived brokered handshakes between two of a user's devices.
create table if not exists public.pairing_sessions (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.profiles (id) on delete cascade,
  initiator_device_id uuid not null references public.devices (id) on delete cascade,
  responder_device_id uuid references public.devices (id) on delete set null,
  code_hash text not null,
  status text not null default 'pending'
    check (status in ('pending','confirmed','expired','cancelled','locked')),
  attempt_count integer not null default 0,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '5 minutes',
  confirmed_at timestamptz
);

alter table public.pairing_sessions enable row level security;

create policy "pairing_sessions_select_own" on public.pairing_sessions
  for select using ((select auth.uid()) = user_id);
create policy "pairing_sessions_insert_own" on public.pairing_sessions
  for insert with check ((select auth.uid()) = user_id);
create policy "pairing_sessions_update_own" on public.pairing_sessions
  for update using ((select auth.uid()) = user_id);

grant select, insert, update on public.pairing_sessions to authenticated;

create index pairing_sessions_user_pending_idx
  on public.pairing_sessions (user_id, created_at desc)
  where status = 'pending';

-- Atomic code verification with attempt limiting. security invoker: RLS applies.
create or replace function public.confirm_pairing(
  p_session_id uuid,
  p_code text,
  p_responder_device_id uuid
) returns text
language plpgsql
security invoker
as $$
declare
  v_session public.pairing_sessions%rowtype;
begin
  select * into v_session from public.pairing_sessions
    where id = p_session_id for update;

  if not found then return 'not_found'; end if;
  if v_session.status = 'locked' then return 'locked'; end if;
  if v_session.status <> 'pending' then return v_session.status; end if;
  if v_session.expires_at < now() then
    update public.pairing_sessions set status = 'expired' where id = p_session_id;
    return 'expired';
  end if;

  if v_session.code_hash = encode(sha256((p_session_id::text || ':' || p_code)::bytea), 'hex') then
    update public.pairing_sessions
      set status = 'confirmed', confirmed_at = now(),
          responder_device_id = p_responder_device_id
      where id = p_session_id;
    return 'confirmed';
  end if;

  update public.pairing_sessions
    set attempt_count = attempt_count + 1,
        status = case when attempt_count + 1 >= 5 then 'locked' else status end
    where id = p_session_id;
  return case when v_session.attempt_count + 1 >= 5 then 'locked' else 'wrong_code' end;
end;
$$;

-- Device lifecycle audit trail (user-scoped).
create table if not exists public.device_audit_events (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.profiles (id) on delete cascade,
  device_id uuid,
  event_type text not null check (char_length(event_type) between 1 and 60),
  metadata jsonb not null default '{}',
  created_at timestamptz not null default now()
);
alter table public.device_audit_events enable row level security;
create policy "device_audit_select_own" on public.device_audit_events
  for select using ((select auth.uid()) = user_id);
create policy "device_audit_insert_own" on public.device_audit_events
  for insert with check ((select auth.uid()) = user_id);
grant select, insert on public.device_audit_events to authenticated;
