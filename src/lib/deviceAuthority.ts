import { supabase } from "./supabase";

/**
 * Client side of the W1 device authority boundary.
 *
 * `supabase/migrations/20260823000000_w1_device_enrollment_authority.sql`
 * made `public.devices` server-owned: a signed-in client keeps SELECT only,
 * so registering and revoking go through the `device-enrollment` Edge
 * function, which derives the user from the verified session and calls the
 * service-role RPCs (`register_pairing_device`, `revoke_device_for_user`).
 * Revocation is soft — the row stays with `revoked_at` set — so readers
 * filter on `revoked_at`, not on existence.
 *
 * This is the one place the web and mobile surfaces do either;
 * `tests/deviceAuthority.test.mjs` pins that nothing in `src/` writes the
 * table directly.
 */

export type DeviceAuthorityErrorCode =
  | "unauthenticated"
  | "device_not_found"
  | "invalid_device_key"
  | "invalid_request"
  | "enrollment_unavailable"
  | "network"
  | (string & {});

export class DeviceAuthorityError extends Error {
  code: DeviceAuthorityErrorCode;
  status: number | null;

  constructor(code: DeviceAuthorityErrorCode, status: number | null, message: string) {
    super(message);
    this.name = "DeviceAuthorityError";
    this.code = code;
    this.status = status;
  }
}

type EnrollmentResponse = {
  ok?: boolean;
  code?: string;
  deviceId?: string;
  authorityState?: string;
};

async function callDeviceEnrollment(body: Record<string, unknown>): Promise<EnrollmentResponse> {
  const { data, error } = await supabase.functions.invoke<EnrollmentResponse>("device-enrollment", {
    body,
  });
  if (error) {
    // A non-2xx reply surfaces as FunctionsHttpError with the Response in
    // `context`; the function body carries the reason as `{ code }`.
    const context = (error as { context?: unknown }).context;
    if (context instanceof Response) {
      let code: DeviceAuthorityErrorCode = "enrollment_unavailable";
      try {
        const parsed = (await context.clone().json()) as { code?: string };
        if (parsed?.code) code = parsed.code;
      } catch {
        // Non-JSON body (gateway 401 before the function ran, for instance).
      }
      if (context.status === 401 && code === "enrollment_unavailable") code = "unauthenticated";
      throw new DeviceAuthorityError(code, context.status, error.message);
    }
    throw new DeviceAuthorityError("network", null, error.message);
  }
  if (!data?.ok) {
    throw new DeviceAuthorityError(data?.code ?? "enrollment_unavailable", null, "device-enrollment refused");
  }
  return data;
}

/** Soft-revokes one of the signed-in user's devices (`revoke_device_for_user`). */
export async function revokeCloudDevice(deviceId: string): Promise<void> {
  await callDeviceEnrollment({ action: "revoke", deviceId });
}

export type PairingDeviceInput = {
  deviceLabel: string;
  platform: string;
  /** Standard base64 of the raw 32-byte ed25519 verifying key. */
  publicKey: string;
  /** Lower-case hex sha256 of that raw key; the function checks they match. */
  publicKeyFingerprint: string;
};

/**
 * Registers (or, when the fingerprint already exists, refreshes) a
 * `pairing_only` device under the signed-in user and returns its row id
 * (`register_pairing_device`). Never creates a trusted device and never
 * resurrects a revoked one — the server raises for both.
 */
export async function registerPairingDevice(input: PairingDeviceInput): Promise<{ deviceId: string }> {
  const data = await callDeviceEnrollment({ action: "pairing_only", ...input });
  if (!data.deviceId) {
    throw new DeviceAuthorityError("enrollment_unavailable", null, "device-enrollment returned no device id");
  }
  return { deviceId: data.deviceId };
}
