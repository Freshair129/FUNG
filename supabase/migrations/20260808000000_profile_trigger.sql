-- Auto-create profile row when a new user signs up via Supabase Auth.
-- The profiles table and its RLS policies already exist in
-- 20260722000000_auth_control_plane.sql — this migration only adds the
-- trigger that populates it on first login.
--
-- Both the profile row and (for Google sign-ups) the oauth_connections row
-- are created from a single trigger function so we don't depend on
-- PostgreSQL's alphabetical firing order for same-timing triggers.

CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger AS $$
BEGIN
  INSERT INTO public.profiles (id, display_name)
  VALUES (
    NEW.id,
    COALESCE(
      NEW.raw_user_meta_data->>'full_name',
      NEW.raw_user_meta_data->>'name',
      'User'
    )
  );

  IF NEW.raw_app_meta_data->>'provider' = 'google' THEN
    INSERT INTO public.oauth_connections (user_id, provider, status, approved_scopes)
    VALUES (NEW.id, 'google', 'active', ARRAY['openid', 'email', 'profile'])
    ON CONFLICT (user_id, provider) DO UPDATE
    SET status = 'active', last_authorized_at = now();
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE TRIGGER on_auth_user_created
  AFTER INSERT ON auth.users
  FOR EACH ROW EXECUTE FUNCTION public.handle_new_user();
