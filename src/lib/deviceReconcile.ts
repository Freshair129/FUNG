/**
 * Mobile device-row reconciliation (Phase 4, R4-11..R4-13).
 *
 * The Supabase PKCE session and `devices` table stay authoritative; this
 * module only decides how the single Android row for this physical device is
 * reused, refreshed, or re-registered — and when the locally cached
 * `fung.device.id` must be replaced or cleared. It never creates a second
 * identity model and performs no backup work.
 */

export type OwnedDeviceRow = { id: string };

export type DevicesPort = {
  /** Row for this physical device owned by the CURRENT user, or null. The
   * implementation must filter by both user id and fingerprint so a row
   * belonging to another account is never reused. */
  findOwnedDevice(userId: string, fingerprint: string): Promise<OwnedDeviceRow | null>;
  /** Register exactly one Android row under the current user. */
  insertDevice(userId: string, fingerprint: string): Promise<OwnedDeviceRow>;
  /** Refresh last-seen + public key on an existing owned row. */
  refreshDevice(deviceId: string, publicKey: string | null): Promise<void>;
  auditRegistered(userId: string, deviceId: string): Promise<void>;
};

export type ReconcileOutcome = {
  deviceId: string;
  /** True when the cached id was missing, stale, or belonged to a different
   * row and must be rewritten by the caller. */
  cacheStale: boolean;
  registered: boolean;
};

/**
 * Reconcile the device row for a valid session. The cached id is never
 * trusted on its own: the row is always resolved by (user, fingerprint)
 * first, and the cache is only a mirror of that result.
 */
export async function reconcileMobileDevice(
  port: DevicesPort,
  userId: string,
  fingerprint: string,
  publicKey: string | null,
  cachedDeviceId: string | null,
): Promise<ReconcileOutcome> {
  if (!userId) throw new Error("missing_session");
  if (!fingerprint) throw new Error("missing_device_identity");

  const existing = await port.findOwnedDevice(userId, fingerprint);
  if (existing) {
    await port.refreshDevice(existing.id, publicKey);
    return {
      deviceId: existing.id,
      cacheStale: cachedDeviceId !== existing.id,
      registered: false,
    };
  }

  // Missing row: first run, or this device was revoked — register one row
  // under the current session either way. A stale cached id pointing at the
  // revoked/foreign row is reported stale so the caller replaces it.
  const inserted = await port.insertDevice(userId, fingerprint);
  await port.refreshDevice(inserted.id, publicKey);
  await port.auditRegistered(userId, inserted.id);
  return { deviceId: inserted.id, cacheStale: cachedDeviceId !== inserted.id, registered: true };
}

/**
 * Cache policy on auth transitions: any signed-out/revoked state clears the
 * cached device id; only a valid session keeps it (until reconciliation
 * confirms or replaces it).
 */
export function deviceCacheActionForSession(hasValidSession: boolean): "keep" | "clear" {
  return hasValidSession ? "keep" : "clear";
}
