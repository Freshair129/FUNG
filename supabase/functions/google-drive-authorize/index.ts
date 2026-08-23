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

type DeviceRow = {
  id: string;
  user_id: string;
  platform: string;
  authority_state: string;
  enrollment_source: string;
  public_key: string | null;
  public_key_fingerprint: string;
  revoked_at: string | null;
};

type ConnectionRow = {
  id: string;
  status: string;
  approved_scopes: unknown;
  revoked_at: string | null;
};

type GrantRow = {
  operation: string;
  status: string;
  revoked_at: string | null;
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

function exactDriveScope(value: unknown): boolean {
  return Array.isArray(value) && value.length === 1 && value[0] === DRIVE_SCOPE;
}

function requiredGrant(operation: string): string | null {
  if (operation === "backup.write") return "backup.write";
  if (operation === "backup.read" || operation === "backup.restore") {
    return "backup.restore";
  }
  return null;
}

function grantState(
  grants: GrantRow[],
  operation: string,
): Record<string, unknown> {
  const grant = grants.find((candidate) => candidate.operation === operation);
  return {
    operation,
    active: Boolean(grant && grant.status === "active" && !grant.revoked_at),
    status: grant?.status ?? "missing",
  };
}

async function recordDecision(
  admin: any,
  reservationId: string,
  userId: string,
): Promise<boolean> {
  const { data, error } = await admin.rpc(
    "record_oauth_authorization_decision",
    {
      p_reservation_id: reservationId,
      p_user_id: userId,
      p_decision: "allowed",
      p_denial_code: null,
    },
  );
  return !error && Boolean(data);
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
  const { data: devices, error: deviceError } = await admin
    .from("devices")
    .select(
      "id,user_id,platform,authority_state,enrollment_source,public_key,public_key_fingerprint,revoked_at",
    )
    .eq("id", deviceId)
    .eq("user_id", userId)
    .eq("platform", "windows")
    .eq("authority_state", "drive_trusted")
    .in("enrollment_source", ["boss_bootstrap", "approved_rebind"])
    .is("revoked_at", null)
    .limit(2);
  if (deviceError || !Array.isArray(devices) || devices.length !== 1) {
    return response({ code: "authorization_denied" }, 403);
  }
  const device = devices[0] as DeviceRow;
  const registeredPublicKey = device.public_key
    ? decodeBase64(device.public_key)
    : null;
  if (
    device.user_id !== userId ||
    device.platform !== "windows" ||
    device.authority_state !== "drive_trusted" ||
    !["boss_bootstrap", "approved_rebind"].includes(device.enrollment_source) ||
    device.revoked_at !== null ||
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

  const { data: existingConnection, error: connectionReadError } = await admin
    .from("oauth_connections")
    .select("id,status,approved_scopes,revoked_at")
    .eq("user_id", userId)
    .eq("provider", PROVIDER)
    .maybeSingle();
  if (connectionReadError) {
    return response({ code: "authorization_unavailable" }, 500);
  }
  const connection = existingConnection as ConnectionRow | null;
  const connectionIsActive = Boolean(
    connection && connection.status === "active" &&
      connection.revoked_at === null &&
      exactDriveScope(connection.approved_scopes),
  );

  let grants: GrantRow[] = [];
  if (connection?.id) {
    const { data, error } = await admin
      .from("oauth_operation_grants")
      .select("operation,status,revoked_at")
      .eq("connection_id", connection.id)
      .in("operation", ["backup.write", "backup.restore"]);
    if (error || !Array.isArray(data)) {
      return response({ code: "authorization_unavailable" }, 500);
    }
    grants = data as GrantRow[];
  }

  const required = requiredGrant(operation);
  if (
    required && (!connectionIsActive ||
      !grants.some((grant) =>
        grant.operation === required &&
        grant.status === "active" && grant.revoked_at === null
      ))
  ) {
    return response({ code: "authorization_denied" }, 403);
  }

  const { data: reservationRows, error: reservationError } = await admin.rpc(
    "reserve_oauth_authorization",
    {
      p_user_id: userId,
      p_device_id: device.id,
      p_connection_id: connection?.id ?? null,
      p_operation: operation,
      p_nonce: nonce,
      p_expires_at: new Date(nowMs + AUTHORIZATION_TTL_MS).toISOString(),
    },
  );
  if (
    reservationError || !Array.isArray(reservationRows) ||
    reservationRows.length !== 1
  ) {
    return response({ code: "authorization_unavailable" }, 500);
  }
  const reservation = reservationRows[0] as {
    reservation_id?: string;
    won?: boolean;
  };
  if (!reservation.reservation_id) {
    return response({ code: "authorization_unavailable" }, 500);
  }
  if (reservation.won !== true) {
    return response({ code: "authorization_replayed" }, 409);
  }

  let connectionId = connection?.id ?? null;
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
  } else if (operation === "connection.revoke" && connection?.id) {
    const { error } = await admin
      .from("oauth_connections")
      .update({ status: "revoked", revoked_at: new Date(nowMs).toISOString() })
      .eq("id", connection.id)
      .eq("user_id", userId)
      .eq("provider", PROVIDER);
    if (error) return response({ code: "authorization_unavailable" }, 500);
    grants = grants.map((grant) => ({
      ...grant,
      status: "revoked",
      revoked_at: new Date(nowMs).toISOString(),
    }));
  }

  if (!await recordDecision(admin, reservation.reservation_id, userId)) {
    return response({ code: "authorization_unavailable" }, 500);
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

  const connectionStatus = operation === "connection.activate"
    ? "active"
    : operation === "connection.revoke"
    ? "revoked"
    : connection
    ? (connectionIsActive ? "active" : connection.status)
    : "disconnected";
  return response({
    authorized: true,
    provider: PROVIDER,
    userId,
    deviceId: device.id,
    deviceFingerprint,
    connectionId,
    connectionStatus,
    writeGrant: grantState(grants, "backup.write"),
    restoreGrant: grantState(grants, "backup.restore"),
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
