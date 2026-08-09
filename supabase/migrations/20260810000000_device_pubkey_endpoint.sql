-- Phase 2 FUNGWIRE: publish each device's full ed25519 public key (for the Noise
-- KK handshake) and its current LAN endpoint (for discovery). Public keys are not
-- secret; sha256(raw key) must equal the existing public_key_fingerprint.
alter table public.devices
  add column if not exists public_key text,
  add column if not exists lan_endpoint text,
  add column if not exists lan_endpoint_updated_at timestamptz;

comment on column public.devices.public_key is
  'Base64 ed25519 verifying key (44 chars). sha256(raw 32 bytes)=public_key_fingerprint. Public, not secret.';
comment on column public.devices.lan_endpoint is
  'Last-known LAN ip:port of this device''s FUNGWIRE server. Advisory; identity is proven by the Noise handshake.';

-- Phase 1 granted update only on (device_label, last_seen_at). Extend so a device
-- can maintain its own public_key + endpoint. RLS still scopes every row to auth.uid().
grant update (device_label, last_seen_at, public_key, lan_endpoint, lan_endpoint_updated_at)
  on public.devices to authenticated;
