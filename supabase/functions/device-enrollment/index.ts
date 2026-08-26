import { withSupabase } from "npm:@supabase/server@1.4.1";

const AUTHORITY_ACTIONS = new Set(["pending", "pairing_only", "revoke"]);
const ENROLLMENT_OPERATION = "device.enrollment.request";
const ENROLLMENT_DOMAIN = "FUNG\0DEVICE_ENROLLMENT\0V1\0";
const MAX_PROOF_TTL_MS = 300_000;
const MAX_CLOCK_SKEW_MS = 30_000;
const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers":
    "authorization, x-client-info, apikey, content-type",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type EnrollmentProof = {
  version: number;
  operation: string;
  userId: string;
  publicKey: string;
  fingerprint: string;
  platform: string;
  deviceLabel: string;
  issuedAtMs: number;
  expiresAtMs: number;
  nonce: string;
  signature: string;
};

type EnrollmentRequest = {
  action?: string;
  deviceId?: string;
  deviceLabel?: string;
  platform?: string;
  publicKey?: string;
  publicKeyFingerprint?: string;
  nativeProof?: unknown;
};

type CanonicalProof = {
  bytes: Uint8Array;
  publicKey: Uint8Array;
  fingerprint: Uint8Array;
  nonce: Uint8Array;
  signature: Uint8Array;
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

function uuidBytes(value: string): Uint8Array | null {
  if (!isUuid(value)) return null;
  const compact = value.replaceAll("-", "");
  const bytes = new Uint8Array(16);
  for (let index = 0; index < bytes.length; index += 1) {
    const parsed = Number.parseInt(compact.slice(index * 2, index * 2 + 2), 16);
    if (!Number.isInteger(parsed)) return null;
    bytes[index] = parsed;
  }
  return bytes;
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

function decodeBase64Url(value: unknown): Uint8Array | null {
  if (typeof value !== "string" || !/^[A-Za-z0-9_-]+$/.test(value)) return null;
  const standard = value.replaceAll("-", "+").replaceAll("_", "/")
    .padEnd(Math.ceil(value.length / 4) * 4, "=");
  const decoded = decodeBase64(standard);
  if (!decoded || encodeBase64Url(decoded) !== value) return null;
  return decoded;
}

function encodeBase64Url(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replaceAll("=", "");
}

function encodeBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes));
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

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
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

function isExactEnrollmentProof(value: unknown): value is EnrollmentProof {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const proof = value as Record<string, unknown>;
  const expected = [
    "deviceLabel",
    "expiresAtMs",
    "fingerprint",
    "issuedAtMs",
    "nonce",
    "operation",
    "platform",
    "publicKey",
    "signature",
    "userId",
    "version",
  ].sort();
  const actual = Object.keys(proof).sort();
  return actual.length === expected.length &&
    actual.every((key, index) => key === expected[index]);
}

function canonicalEnrollmentProof(proof: EnrollmentProof): CanonicalProof | null {
  const userBytes = uuidBytes(proof.userId);
  const publicKey = decodeBase64Url(proof.publicKey);
  const fingerprint = decodeBase64Url(proof.fingerprint);
  const nonce = decodeBase64Url(proof.nonce);
  const signature = decodeBase64Url(proof.signature);
  const platformBytes = new TextEncoder().encode(proof.platform);
  const label = proof.deviceLabel.normalize("NFC").trim();
  const labelBytes = new TextEncoder().encode(label);
  if (
    !userBytes || !publicKey || publicKey.length !== 32 || !fingerprint ||
    fingerprint.length !== 32 || !nonce || nonce.length !== 32 || !signature ||
    signature.length !== 64 || proof.version !== 1 ||
    proof.operation !== ENROLLMENT_OPERATION || proof.platform !== "windows" ||
    label !== proof.deviceLabel || labelBytes.length < 1 || labelBytes.length > 80 ||
    proof.deviceLabel.includes("\u0000") ||
    [...proof.deviceLabel].some((character) => character.charCodeAt(0) < 32 ||
      character.charCodeAt(0) === 127) ||
    !Number.isSafeInteger(proof.issuedAtMs) || !Number.isSafeInteger(proof.expiresAtMs)
  ) {
    return null;
  }

  const now = Date.now();
  if (
    proof.issuedAtMs > now + MAX_CLOCK_SKEW_MS ||
    proof.expiresAtMs <= now ||
    proof.expiresAtMs <= proof.issuedAtMs ||
    proof.expiresAtMs - proof.issuedAtMs > MAX_PROOF_TTL_MS ||
    platformBytes.length > 0xffff || labelBytes.length > 0xffff
  ) {
    return null;
  }

  const bytes = new Uint8Array(
    new TextEncoder().encode(ENROLLMENT_DOMAIN).length + 16 + 32 + 32 + 2 +
      platformBytes.length + 2 + labelBytes.length + 8 + 8 + 32,
  );
  let offset = 0;
  const append = (part: Uint8Array) => {
    bytes.set(part, offset);
    offset += part.length;
  };
  const appendU16 = (value: number) => {
    bytes[offset] = value >>> 8;
    bytes[offset + 1] = value & 0xff;
    offset += 2;
  };
  const appendI64 = (value: number) => {
    new DataView(bytes.buffer).setBigInt64(offset, BigInt(value), false);
    offset += 8;
  };
  append(new TextEncoder().encode(ENROLLMENT_DOMAIN));
  append(userBytes);
  append(publicKey);
  append(fingerprint);
  appendU16(platformBytes.length);
  append(platformBytes);
  appendU16(labelBytes.length);
  append(labelBytes);
  appendI64(proof.issuedAtMs);
  appendI64(proof.expiresAtMs);
  append(nonce);
  return { bytes, publicKey, fingerprint, nonce, signature };
}

async function verifyEnrollmentProof(
  proof: EnrollmentProof,
  userId: string,
): Promise<{ canonical: CanonicalProof; fingerprintHex: string } | null> {
  if (proof.userId.toLowerCase() !== userId.toLowerCase()) return null;
  const canonical = canonicalEnrollmentProof(proof);
  if (!canonical) return null;
  const fingerprintDigest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", asArrayBuffer(canonical.publicKey)),
  );
  if (!sameBytes(fingerprintDigest, canonical.fingerprint)) return null;
  const verifiedKey = await crypto.subtle.importKey(
    "raw",
    asArrayBuffer(canonical.publicKey),
    { name: "Ed25519" },
    false,
    ["verify"],
  );
  const validSignature = await crypto.subtle.verify(
    { name: "Ed25519" },
    verifiedKey,
    asArrayBuffer(canonical.signature),
    asArrayBuffer(canonical.bytes),
  );
  if (!validSignature) return null;
  return { canonical, fingerprintHex: hex(fingerprintDigest) };
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

  if (action === "pairing_only") {
    if (!validDeviceFields(body)) {
      return response({ code: "invalid_request" }, 400);
    }
    const publicKey = body.publicKey as string;
    const publicKeyFingerprint = body.publicKeyFingerprint as string;
    if (!(await isMatchingKeyMaterial(publicKey, publicKeyFingerprint))) {
      return response({ code: "invalid_device_key" }, 400);
    }
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

  if (!isExactEnrollmentProof(body.nativeProof)) {
    return response({ code: "invalid_native_proof" }, 400);
  }
  const proof = body.nativeProof;
  const verified = await verifyEnrollmentProof(proof, userId);
  if (!verified) {
    return response({ code: "invalid_native_proof" }, 400);
  }
  const publicKey = encodeBase64(verified.canonical.publicKey);
  const proofSignatureHex = hex(verified.canonical.signature);
  const nonceHash = new Uint8Array(
    await crypto.subtle.digest("SHA-256", asArrayBuffer(verified.canonical.nonce)),
  );
  const envelopeHash = new Uint8Array(
    await crypto.subtle.digest("SHA-256", asArrayBuffer(verified.canonical.bytes)),
  );
  const { data, error } = await admin.rpc(
    "create_device_enrollment_request",
    {
      p_user_id: userId,
      p_device_label: proof.deviceLabel,
      p_platform: proof.platform,
      p_public_key: publicKey,
      p_public_key_fingerprint: verified.fingerprintHex,
      p_proof_version: proof.version,
      p_proof_operation: proof.operation,
      p_nonce_hash_hex: hex(nonceHash),
      p_issued_at_ms: proof.issuedAtMs,
      p_expires_at_ms: proof.expiresAtMs,
      p_envelope_hash_hex: hex(envelopeHash),
      p_proof_signature_hex: proofSignatureHex,
    },
  );
  if (error) {
    const message = typeof error.message === "string" ? error.message : "";
    if (message.includes("proof_replayed")) {
      return response({ code: "proof_replayed" }, 409);
    }
    if (message.includes("device_identity_already_registered")) {
      return response({ code: "device_identity_already_registered" }, 409);
    }
    return response({ code: "enrollment_unavailable" }, 500);
  }
  if (!Array.isArray(data) || data.length !== 1) {
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
