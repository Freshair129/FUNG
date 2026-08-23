import { withSupabase } from "npm:@supabase/server@1.4.1";

const AUTHORITY_ACTIONS = new Set(["pending", "pairing_only", "revoke"]);
const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers":
    "authorization, x-client-info, apikey, content-type",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type EnrollmentRequest = {
  action?: string;
  deviceId?: string;
  deviceLabel?: string;
  platform?: string;
  publicKey?: string;
  publicKeyFingerprint?: string;
  nativeProof?: string;
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

function isUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
    .test(value);
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

function asArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

async function isMatchingKeyMaterial(
  publicKey: string,
  fingerprint: string,
): Promise<boolean> {
  const key = decodeBase64(publicKey);
  if (!key || key.length !== 32 || !/^[0-9a-f]{64}$/i.test(fingerprint)) {
    return false;
  }
  const digest = await crypto.subtle.digest("SHA-256", asArrayBuffer(key));
  return hex(new Uint8Array(digest)) === fingerprint.toLowerCase();
}

function validDeviceFields(body: EnrollmentRequest): boolean {
  return Boolean(
    body.deviceLabel && body.deviceLabel.length <= 120 && body.platform &&
      body.platform.length <= 40 && body.publicKey && body.publicKeyFingerprint,
  );
}

const handler = withSupabase({ auth: "user" }, async (request, ctx) => {
  if (request.method !== "POST") {
    return response({ code: "method_not_allowed" }, 405);
  }

  const userId = ctx.userClaims?.id;
  if (!userId || !isUuid(userId)) {
    return response({ code: "unauthenticated" }, 401);
  }

  let body: EnrollmentRequest;
  try {
    body = await request.json() as EnrollmentRequest;
  } catch {
    return response({ code: "invalid_request" }, 400);
  }

  const action = body.action ?? "pending";
  if (!AUTHORITY_ACTIONS.has(action)) {
    return response({ code: "invalid_request" }, 400);
  }

  const admin = ctx.supabaseAdmin as any;
  if (action === "revoke") {
    const deviceId = body.deviceId ?? "";
    if (!isUuid(deviceId)) {
      return response({ code: "invalid_request" }, 400);
    }
    const { data, error } = await admin.rpc("revoke_device_for_user", {
      p_user_id: userId,
      p_device_id: deviceId,
    });
    if (error) return response({ code: "enrollment_unavailable" }, 500);
    if (data !== true) return response({ code: "device_not_found" }, 404);
    return response({ ok: true, deviceId, authorityState: "revoked" });
  }

  if (!validDeviceFields(body)) {
    return response({ code: "invalid_request" }, 400);
  }
  const publicKey = body.publicKey as string;
  const publicKeyFingerprint = body.publicKeyFingerprint as string;
  if (
    !(await isMatchingKeyMaterial(publicKey, publicKeyFingerprint))
  ) {
    return response({ code: "invalid_device_key" }, 400);
  }

  if (action === "pairing_only") {
    const { data, error } = await admin.rpc("register_pairing_device", {
      p_user_id: userId,
      p_device_label: body.deviceLabel,
      p_platform: body.platform,
      p_public_key: publicKey,
      p_public_key_fingerprint: publicKeyFingerprint.toLowerCase(),
    });
    if (error || !isUuid(data as string)) {
      return response({ code: "enrollment_unavailable" }, 500);
    }
    return response({
      ok: true,
      deviceId: data,
      authorityState: "pairing_only",
    });
  }

  const nativeProof = body.nativeProof ?? "";
  if (nativeProof.length < 1 || nativeProof.length > 8192) {
    return response({ code: "invalid_native_proof" }, 400);
  }
  const { data, error } = await admin.rpc(
    "create_device_enrollment_request",
    {
      p_user_id: userId,
      p_device_label: body.deviceLabel,
      p_platform: body.platform,
      p_public_key: publicKey,
      p_public_key_fingerprint: publicKeyFingerprint.toLowerCase(),
      p_native_proof: nativeProof,
    },
  );
  if (error || !Array.isArray(data) || data.length !== 1) {
    return response({ code: "enrollment_unavailable" }, 500);
  }
  const requestRow = data[0] as {
    request_id?: string;
    request_status?: string;
    request_expires_at?: string;
  };
  if (!requestRow.request_id || requestRow.request_status !== "pending") {
    return response({ code: "enrollment_unavailable" }, 500);
  }
  return response({
    ok: true,
    requestId: requestRow.request_id,
    status: "pending",
    expiresAt: requestRow.request_expires_at,
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
