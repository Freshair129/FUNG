import { withSupabase } from "npm:@supabase/server@1.4.1";

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
  deviceId?: string;
  devicePublicKey?: string;
  deviceFingerprint?: string;
  signature?: string;
  timestampMs?: number;
  nonce?: string;
};

type DeviceKeyRow = {
  id: string;
  user_id: string;
  public_key: string | null;
  public_key_fingerprint: string;
};

type AuthorizationDecisionRow = {
  reservation_id?: string;
  operation?: string;
  nonce?: string;
  authorized?: boolean;
  denial_code?: string | null;
  connection_id?: string | null;
  connection_status?: string;
  write_grant_status?: string;
  restore_grant_status?: string;
  expires_at?: string;
};

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
  return new TextEncoder().encode([
    "fung-drive-auth-v1",
    operation,
    String(timestampMs),
    nonce,
    fingerprint,
  ].join("\n"));
}

function grantState(
  operation: string,
  status: string,
): Record<string, unknown> {
  return {
    operation,
    active: status === "active",
    status,
  };
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
  const deviceId = body.deviceId ?? "";
  const devicePublicKeyValue = body.devicePublicKey ?? "";
  const deviceFingerprint = body.deviceFingerprint ?? "";
  const signatureValue = body.signature ?? "";
  const timestampMs = body.timestampMs;
  const nonce = body.nonce ?? "";
  if (
    !OPERATIONS.has(operation) ||
    !isUuid(deviceId) ||
    !isHexFingerprint(deviceFingerprint) ||
    !Number.isSafeInteger(timestampMs) ||
    !isUuid(nonce)
  ) {
    return response({ code: "invalid_request" }, 400);
  }

  const nowMs = Date.now();
  if (Math.abs(nowMs - (timestampMs as number)) > AUTHORIZATION_TTL_MS) {
    return response({ code: "authorization_expired" }, 401);
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
  // The Edge lookup supplies only public-key material for signature
  // verification. Authoritative device state is rechecked and locked by the
  // single database authorization RPC after the signature is verified.
  const { data: deviceRow, error: deviceError } = await admin
    .from("devices")
    .select("id,user_id,public_key,public_key_fingerprint")
    .eq("id", deviceId)
    .eq("user_id", userId)
    .maybeSingle();
  if (deviceError || !deviceRow) {
    return response({ code: "authorization_denied" }, 403);
  }
  const device = deviceRow as DeviceKeyRow;
  const registeredPublicKey = device.public_key
    ? decodeBase64(device.public_key)
    : null;
  if (
    device.user_id !== userId ||
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

  const { data: decisionRows, error: decisionError } = await admin.rpc(
    "authorize_oauth_request",
    {
      p_user_id: userId,
      p_device_id: device.id,
      p_device_public_key: devicePublicKeyValue,
      p_device_fingerprint: deviceFingerprint,
      p_operation: operation,
      p_nonce: nonce,
      p_expires_at: new Date(nowMs + AUTHORIZATION_TTL_MS).toISOString(),
    },
  );
  if (
    decisionError || !Array.isArray(decisionRows) || decisionRows.length !== 1
  ) {
    return response({ code: "authorization_unavailable" }, 500);
  }
  const decision = decisionRows[0] as AuthorizationDecisionRow;
  if (
    !decision.reservation_id ||
    decision.operation !== operation ||
    decision.nonce !== nonce ||
    typeof decision.authorized !== "boolean"
  ) {
    return response({ code: "authorization_unavailable" }, 500);
  }
  if (!decision.authorized) {
    return response(
      { code: decision.denial_code ?? "authorization_denied" },
      decision.denial_code === "authorization_replayed" ? 409 : 403,
    );
  }

  const connectionId = decision.connection_id ?? null;
  const connectionStatus = decision.connection_status ?? "disconnected";
  const writeGrantStatus = decision.write_grant_status ?? "missing";
  const restoreGrantStatus = decision.restore_grant_status ?? "missing";

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
    connectionStatus,
    writeGrant: grantState("backup.write", writeGrantStatus),
    restoreGrant: grantState("backup.restore", restoreGrantStatus),
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
