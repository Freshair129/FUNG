import { withSupabase } from "npm:@supabase/server";

const PROVIDER = "google_drive";
const DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.appdata";
const CLIENT_TYPE = "desktop";
const AUTHORIZATION_TTL_MS = 90_000;
const OPERATIONS = new Set([
  "connection.authorize",
  "connection.activate",
  "connection.read",
  "connection.revoke",
  "backup.read",
  "backup.write",
  "backup.restore",
]);

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers":
    "authorization, x-client-info, apikey, content-type",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type AuthorizationRequest = {
  operation?: string;
  devicePublicKey?: string;
  deviceFingerprint?: string;
  signature?: string;
  timestampMs?: number;
  nonce?: string;
};

const replayedNonces = new Map<string, number>();

function response(body: Record<string, unknown>, status = 200): Response {
  return Response.json(body, { status, headers: corsHeaders });
}

function withCors(result: Response): Response {
  const headers = new Headers(result.headers);
  for (const [key, value] of Object.entries(corsHeaders)) {
    headers.set(key, value);
  }
  return new Response(result.body, {
    status: result.status,
    statusText: result.statusText,
    headers,
  });
}

function isHexFingerprint(value: string): boolean {
  return /^[0-9a-f]{64}$/.test(value);
}

function decodeBase64(value: string): Uint8Array | null {
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(value)) return null;
  try {
    const binary = atob(value);
    return Uint8Array.from(binary, (character) => character.charCodeAt(0));
  } catch {
    return null;
  }
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length &&
    left.every((byte, index) => byte === right[index]);
}

function asArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

function isUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
    .test(value);
}

function canonicalRequest(
  operation: string,
  timestampMs: number,
  nonce: string,
  fingerprint: string,
): Uint8Array {
  return new TextEncoder().encode(
    `fung-drive-auth-v1\n${operation}\n${timestampMs}\n${nonce}\n${fingerprint}`,
  );
}

function purgeReplay(nowMs: number): void {
  for (const [key, expiresAt] of replayedNonces) {
    if (expiresAt <= nowMs) replayedNonces.delete(key);
  }
}

function exactDriveScope(value: unknown): boolean {
  return Array.isArray(value) && value.length === 1 && value[0] === DRIVE_SCOPE;
}

function isProviderOperation(operation: string): boolean {
  return operation === "backup.read" || operation === "backup.write" ||
    operation === "backup.restore";
}

const handler = withSupabase({ auth: "user" }, async (request, ctx) => {
  if (request.method !== "POST") {
    return response({ code: "method_not_allowed" }, 405);
  }

  const userId = ctx.userClaims?.id;
  if (!userId || !isUuid(userId)) {
    return response({ code: "unauthenticated" }, 401);
  }

  let body: AuthorizationRequest;
  try {
    body = await request.json() as AuthorizationRequest;
  } catch {
    return response({ code: "invalid_request" }, 400);
  }

  const operation = body.operation ?? "";
  const devicePublicKeyValue = body.devicePublicKey ?? "";
  const deviceFingerprint = body.deviceFingerprint ?? "";
  const signatureValue = body.signature ?? "";
  const timestampMs = body.timestampMs;
  const nonce = body.nonce ?? "";
  if (
    !OPERATIONS.has(operation) ||
    !isHexFingerprint(deviceFingerprint) ||
    !Number.isSafeInteger(timestampMs) ||
    !nonce ||
    nonce.length > 128 ||
    !isUuid(nonce)
  ) {
    return response({ code: "invalid_request" }, 400);
  }

  const nowMs = Date.now();
  if (Math.abs(nowMs - (timestampMs as number)) > AUTHORIZATION_TTL_MS) {
    return response({ code: "authorization_expired" }, 401);
  }
  purgeReplay(nowMs);
  const replayKey = `${userId}:${nonce}`;
  if (replayedNonces.has(replayKey)) {
    return response({ code: "authorization_replayed" }, 409);
  }

  const devicePublicKey = decodeBase64(devicePublicKeyValue);
  const signature = decodeBase64(signatureValue);
  if (
    !devicePublicKey || devicePublicKey.length !== 32 || !signature ||
    signature.length !== 64
  ) {
    return response({ code: "authorization_denied" }, 403);
  }
  if (
    hex(
      new Uint8Array(
        await crypto.subtle.digest("SHA-256", asArrayBuffer(devicePublicKey)),
      ),
    ) !== deviceFingerprint
  ) {
    return response({ code: "authorization_denied" }, 403);
  }

  const admin = ctx.supabaseAdmin as any;
  const { data: devices, error: deviceError } = await admin
    .from("devices")
    .select("id,user_id,public_key,public_key_fingerprint,revoked_at")
    .eq("user_id", userId)
    .eq("public_key_fingerprint", deviceFingerprint)
    .limit(2);
  if (deviceError || !Array.isArray(devices) || devices.length !== 1) {
    return response({ code: "authorization_denied" }, 403);
  }
  const device = devices[0] as {
    id: string;
    user_id: string;
    public_key: string | null;
    public_key_fingerprint: string;
    revoked_at: string | null;
  };
  const registeredPublicKey = device.public_key
    ? decodeBase64(device.public_key)
    : null;
  if (
    device.user_id !== userId ||
    device.revoked_at ||
    device.public_key_fingerprint !== deviceFingerprint ||
    !registeredPublicKey ||
    registeredPublicKey.length !== 32 ||
    !sameBytes(registeredPublicKey, devicePublicKey)
  ) {
    return response({ code: "authorization_denied" }, 403);
  }

  const verifiedKey = await crypto.subtle.importKey(
    "raw",
    asArrayBuffer(devicePublicKey),
    { name: "Ed25519" },
    false,
    ["verify"],
  );
  const validSignature = await crypto.subtle.verify(
    { name: "Ed25519" },
    verifiedKey,
    asArrayBuffer(signature),
    asArrayBuffer(
      canonicalRequest(
        operation,
        timestampMs as number,
        nonce,
        deviceFingerprint,
      ),
    ),
  );
  if (!validSignature) return response({ code: "authorization_denied" }, 403);
  replayedNonces.set(replayKey, nowMs + AUTHORIZATION_TTL_MS);

  const { data: priorAudit, error: replayReadError } = await admin
    .from("oauth_audit_events")
    .select("id")
    .eq("correlation_id", nonce)
    .limit(1)
    .maybeSingle();
  if (replayReadError) {
    return response({ code: "authorization_unavailable" }, 500);
  }
  if (priorAudit) return response({ code: "authorization_replayed" }, 409);

  const { data: existingConnection, error: connectionReadError } = await admin
    .from("oauth_connections")
    .select("id,status,approved_scopes")
    .eq("user_id", userId)
    .eq("provider", PROVIDER)
    .maybeSingle();
  if (connectionReadError) {
    return response({ code: "authorization_unavailable" }, 500);
  }

  if (
    isProviderOperation(operation) &&
    (!existingConnection ||
      existingConnection.status !== "active" ||
      !exactDriveScope(existingConnection.approved_scopes))
  ) {
    return response({ code: "authorization_denied" }, 403);
  }

  let connectionId = (existingConnection?.id as string | undefined) ?? null;
  if (operation === "connection.activate") {
    const { data, error } = await admin
      .from("oauth_connections")
      .upsert({
        user_id: userId,
        provider: PROVIDER,
        approved_scopes: [DRIVE_SCOPE],
        status: "active",
        connected_at: new Date(nowMs).toISOString(),
        revoked_at: null,
        last_authorized_at: new Date(nowMs).toISOString(),
      }, { onConflict: "user_id,provider" })
      .select("id")
      .single();
    if (error || !data?.id) {
      return response({ code: "authorization_unavailable" }, 500);
    }
    connectionId = data.id as string;
  } else if (operation === "connection.revoke" && existingConnection?.id) {
    const { error } = await admin
      .from("oauth_connections")
      .update({ status: "revoked", revoked_at: new Date(nowMs).toISOString() })
      .eq("id", existingConnection.id)
      .eq("user_id", userId)
      .eq("provider", PROVIDER);
    if (error) return response({ code: "authorization_unavailable" }, 500);
  }

  const auditEvent = operation === "connection.activate"
    ? "authorization_completed"
    : operation === "connection.revoke"
    ? "connection_revoked"
    : "authorization_started";
  const { error: auditError } = await admin
    .from("oauth_audit_events")
    .insert({
      user_id: userId,
      connection_id: connectionId,
      provider: PROVIDER,
      client_type: CLIENT_TYPE,
      event_type: auditEvent,
      outcome: "success",
      scopes: [DRIVE_SCOPE],
      correlation_id: nonce,
      occurred_at: new Date(nowMs).toISOString(),
    });
  if (auditError) return response({ code: "authorization_unavailable" }, 500);

  return response({
    authorized: true,
    provider: PROVIDER,
    userId,
    deviceId: device.id,
    deviceFingerprint,
    connectionId,
    operation,
    expiresAtMs: nowMs + AUTHORIZATION_TTL_MS,
    nonce,
  });
});

export default {
  fetch: async (request: Request) => {
    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }
    return withCors(await handler(request));
  },
};
