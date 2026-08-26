BEGIN;

-- S2-F2 forward-only metadata. Legacy rows remain readable, while every new
-- pending request carries the exact proof material needed for review.
ALTER TABLE public.device_enrollment_requests
  ADD COLUMN IF NOT EXISTS proof_version integer,
  ADD COLUMN IF NOT EXISTS proof_operation text,
  ADD COLUMN IF NOT EXISTS proof_nonce_hash bytea,
  ADD COLUMN IF NOT EXISTS proof_issued_at_ms bigint,
  ADD COLUMN IF NOT EXISTS proof_expires_at_ms bigint,
  ADD COLUMN IF NOT EXISTS proof_envelope_hash bytea,
  ADD COLUMN IF NOT EXISTS proof_signature bytea;

ALTER TABLE public.device_enrollment_requests
  DROP CONSTRAINT IF EXISTS device_enrollment_requests_proof_metadata_shape,
  ADD CONSTRAINT device_enrollment_requests_proof_metadata_shape CHECK (
    (proof_version IS NULL
      AND proof_operation IS NULL
      AND proof_nonce_hash IS NULL
      AND proof_issued_at_ms IS NULL
      AND proof_expires_at_ms IS NULL
      AND proof_envelope_hash IS NULL
      AND proof_signature IS NULL)
    OR (proof_version = 1
      AND proof_operation = 'device.enrollment.request'
      AND proof_nonce_hash IS NOT NULL
      AND octet_length(proof_nonce_hash) = 32
      AND proof_issued_at_ms IS NOT NULL
      AND proof_expires_at_ms IS NOT NULL
      AND proof_expires_at_ms > proof_issued_at_ms
      AND proof_expires_at_ms - proof_issued_at_ms <= 300000
      AND proof_envelope_hash IS NOT NULL
      AND octet_length(proof_envelope_hash) = 32
      AND proof_signature IS NOT NULL
      AND octet_length(proof_signature) = 64)
  );

CREATE TABLE public.device_enrollment_proof_reservations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nonce_hash bytea NOT NULL UNIQUE,
  user_id uuid NOT NULL REFERENCES public.profiles (id) ON DELETE RESTRICT,
  public_key_fingerprint text NOT NULL,
  envelope_hash bytea NOT NULL,
  issued_at_ms bigint NOT NULL,
  expires_at_ms bigint NOT NULL,
  request_id uuid NOT NULL,
  decision text NOT NULL DEFAULT 'pending',
  created_at timestamptz NOT NULL DEFAULT pg_catalog.now(),
  CONSTRAINT device_enrollment_proof_reservations_nonce_length
    CHECK (octet_length(nonce_hash) = 32),
  CONSTRAINT device_enrollment_proof_reservations_fingerprint_length
    CHECK (public_key_fingerprint ~ '^[0-9a-f]{64}$'),
  CONSTRAINT device_enrollment_proof_reservations_envelope_length
    CHECK (octet_length(envelope_hash) = 32),
  CONSTRAINT device_enrollment_proof_reservations_time_order
    CHECK (expires_at_ms > issued_at_ms),
  CONSTRAINT device_enrollment_proof_reservations_decision
    CHECK (decision IN ('pending', 'accepted', 'rejected'))
);

CREATE INDEX IF NOT EXISTS device_enrollment_proof_reservations_user_idx
  ON public.device_enrollment_proof_reservations (user_id, created_at DESC);

ALTER TABLE public.device_enrollment_proof_reservations ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON TABLE public.device_enrollment_proof_reservations
  FROM public, anon, authenticated, service_role;

-- W1 retains nonce reservations indefinitely. There is no cleanup path: the
-- unique nonce hash is an append-only replay barrier, not an expiring cache.
COMMENT ON TABLE public.device_enrollment_proof_reservations IS
  'Indefinite append-only W1 nonce reservations; never clean up or mutate for replay prevention.';

DROP FUNCTION IF EXISTS public.create_device_enrollment_request(
  uuid, text, text, text, text, text
);

CREATE FUNCTION public.create_device_enrollment_request(
  p_user_id uuid,
  p_device_label text,
  p_platform text,
  p_public_key text,
  p_public_key_fingerprint text,
  p_proof_version integer,
  p_proof_operation text,
  p_nonce_hash_hex text,
  p_issued_at_ms bigint,
  p_expires_at_ms bigint,
  p_envelope_hash_hex text,
  p_proof_signature_hex text
) RETURNS TABLE(request_id uuid, request_status text, request_expires_at timestamptz)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public, pg_temp
AS $$
DECLARE
  v_now_ms bigint := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
  v_nonce_hash bytea;
  v_envelope_hash bytea;
  v_signature bytea;
  v_public_key bytea;
  v_reservation_id uuid;
  v_request_id uuid := gen_random_uuid();
  v_expires_at timestamptz;
BEGIN
  IF p_user_id IS NULL
    OR p_device_label IS NULL
    OR octet_length(p_device_label) NOT BETWEEN 1 AND 80
    OR p_device_label ~ '[[:cntrl:]]'
    OR p_platform <> 'windows'
    OR p_public_key IS NULL
    OR p_public_key_fingerprint IS NULL
    OR p_public_key_fingerprint !~ '^[0-9a-f]{64}$'
    OR p_proof_version <> 1
    OR p_proof_operation <> 'device.enrollment.request'
    OR p_nonce_hash_hex IS NULL
    OR p_nonce_hash_hex !~ '^[0-9a-f]{64}$'
    OR p_envelope_hash_hex IS NULL
    OR p_envelope_hash_hex !~ '^[0-9a-f]{64}$'
    OR p_proof_signature_hex IS NULL
    OR p_proof_signature_hex !~ '^[0-9a-f]{128}$'
    OR p_issued_at_ms IS NULL
    OR p_expires_at_ms IS NULL
    OR p_issued_at_ms > v_now_ms + 30000
    OR p_expires_at_ms <= v_now_ms
    OR p_expires_at_ms <= p_issued_at_ms
    OR p_expires_at_ms - p_issued_at_ms > 300000 THEN
    RAISE EXCEPTION 'invalid_enrollment_proof';
  END IF;

  BEGIN
    v_nonce_hash := pg_catalog.decode(p_nonce_hash_hex, 'hex');
    v_envelope_hash := pg_catalog.decode(p_envelope_hash_hex, 'hex');
    v_signature := pg_catalog.decode(p_proof_signature_hex, 'hex');
    v_public_key := pg_catalog.decode(p_public_key, 'base64');
  EXCEPTION WHEN OTHERS THEN
    RAISE EXCEPTION 'invalid_enrollment_proof';
  END;

  IF octet_length(v_nonce_hash) <> 32
    OR octet_length(v_envelope_hash) <> 32
    OR octet_length(v_signature) <> 64
    OR octet_length(v_public_key) <> 32
    OR pg_catalog.encode(pg_catalog.sha256(v_public_key), 'hex') <> p_public_key_fingerprint THEN
    RAISE EXCEPTION 'invalid_enrollment_proof';
  END IF;

  v_expires_at := pg_catalog.to_timestamp(p_expires_at_ms / 1000.0);

  INSERT INTO public.device_enrollment_proof_reservations (
    nonce_hash,
    user_id,
    public_key_fingerprint,
    envelope_hash,
    issued_at_ms,
    expires_at_ms,
    request_id,
    decision
  ) VALUES (
    v_nonce_hash,
    p_user_id,
    p_public_key_fingerprint,
    v_envelope_hash,
    p_issued_at_ms,
    p_expires_at_ms,
    v_request_id,
    'pending'
  )
  ON CONFLICT (nonce_hash) DO NOTHING
  RETURNING id INTO v_reservation_id;

  IF v_reservation_id IS NULL THEN
    RAISE EXCEPTION 'proof_replayed';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM public.devices d
    WHERE d.user_id = p_user_id
      AND d.public_key_fingerprint = p_public_key_fingerprint
  ) THEN
    RAISE EXCEPTION 'device_identity_already_registered';
  END IF;

  RETURN QUERY
  INSERT INTO public.device_enrollment_requests (
    id,
    user_id,
    device_label,
    platform,
    public_key,
    public_key_fingerprint,
    native_proof,
    expires_at,
    proof_version,
    proof_operation,
    proof_nonce_hash,
    proof_issued_at_ms,
    proof_expires_at_ms,
    proof_envelope_hash,
    proof_signature
  ) VALUES (
    v_request_id,
    p_user_id,
    p_device_label,
    p_platform,
    p_public_key,
    p_public_key_fingerprint,
    p_proof_signature_hex,
    v_expires_at,
    p_proof_version,
    p_proof_operation,
    v_nonce_hash,
    p_issued_at_ms,
    p_expires_at_ms,
    v_envelope_hash,
    v_signature
  )
  RETURNING id, status, expires_at;
END;
$$;

REVOKE EXECUTE ON FUNCTION public.create_device_enrollment_request(
  uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text
) FROM public, anon, authenticated, service_role;
GRANT EXECUTE ON FUNCTION public.create_device_enrollment_request(
  uuid, text, text, text, text, integer, text, text, bigint, bigint, text, text
) TO service_role;

COMMIT;
