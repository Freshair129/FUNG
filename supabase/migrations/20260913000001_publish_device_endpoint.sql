-- The desktop publishes its FUNGWIRE LAN endpoint so a paired phone can find
-- it. W1 (20260823000000_w1_device_enrollment_authority.sql) left signed-in
-- clients SELECT-only on public.devices, which silently refused the direct
-- PATCH the desktop used for this. This is the one server-owned write for
-- that purpose: owner-scoped, refuses revoked rows, touches only the endpoint
-- columns and last_seen_at, and never changes authority state. The phone
-- still proves the desktop's identity with the Noise handshake; the endpoint
-- is advisory discovery data.

BEGIN;

CREATE OR REPLACE FUNCTION public.publish_device_endpoint(
  p_device_id uuid,
  p_lan_endpoint text
) RETURNS timestamptz
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public, pg_temp
AS $$
DECLARE
  v_user_id uuid := (select auth.uid());
  v_updated_at timestamptz;
BEGIN
  IF v_user_id IS NULL THEN
    RAISE EXCEPTION 'unauthenticated';
  END IF;
  -- host:port only (IPv4, hostname, or bracketed IPv6), nothing else rides here.
  IF p_lan_endpoint IS NULL
    OR char_length(p_lan_endpoint) > 64
    OR p_lan_endpoint !~ '^[0-9A-Za-z.\-\[\]:]+:[0-9]{1,5}$' THEN
    RAISE EXCEPTION 'invalid_lan_endpoint';
  END IF;

  UPDATE public.devices
  SET lan_endpoint = p_lan_endpoint,
      lan_endpoint_updated_at = pg_catalog.now(),
      last_seen_at = pg_catalog.now()
  WHERE id = p_device_id
    AND user_id = v_user_id
    AND revoked_at IS NULL
    AND authority_state <> 'revoked'
  RETURNING lan_endpoint_updated_at INTO v_updated_at;

  -- NULL when the row is not the caller's or is revoked: the client treats
  -- that as denied rather than learning which of the two it was.
  RETURN v_updated_at;
END;
$$;

REVOKE EXECUTE ON FUNCTION public.publish_device_endpoint(uuid, text)
  FROM public, anon, service_role;
GRANT EXECUTE ON FUNCTION public.publish_device_endpoint(uuid, text)
  TO authenticated;

COMMIT;
