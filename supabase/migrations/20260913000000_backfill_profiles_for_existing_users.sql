-- Users who signed in before 20260808000000_profile_trigger.sql was applied to
-- the live project get the same rows the trigger would have written. The live
-- project (nqnrvqnijzovkrhxslfp) received every earlier migration on
-- 2026-09-13, after its first Google sign-ins, so this is a real gap there.
-- Idempotent: conflicts are left untouched, and a project where the trigger
-- always existed is a no-op.

insert into public.profiles (id, display_name)
select u.id, coalesce(u.raw_user_meta_data->>'full_name', u.raw_user_meta_data->>'name', 'User')
from auth.users u
on conflict (id) do nothing;

insert into public.oauth_connections (user_id, provider, status, approved_scopes)
select u.id, 'google', 'active', array['openid', 'email', 'profile']
from auth.users u
where u.raw_app_meta_data->>'provider' = 'google'
on conflict (user_id, provider) do nothing;
