import { withSupabase } from "npm:@supabase/server";

const PROVIDER = "google_drive";
const DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.appdata";
const CLIENT_TYPES = new Set(["desktop", "mobile"]);
const EVENTS = new Set([
  "authorization_started",
  "authorization_completed",
  "authorization_denied",
  "authorization_expired",
  "connection_revoked",
  "token_refresh_failed",
]);
const NATIVE_ONLY_EVENTS = new Set([
  "authorization_completed",
  "connection_revoked",
]);

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers":
    "authorization, x-client-info, apikey, content-type",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type MetadataRequest = {
  eventType?: string;
  clientType?: string;
  scopes?: unknown;
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

function normalizeScopes(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.length !== 1 || value[0] !== DRIVE_SCOPE) {
    return null;
  }
  return [DRIVE_SCOPE];
}

const handler = withSupabase({ auth: "user" }, async (request, ctx) => {
  if (request.method !== "POST") {
    return response({ code: "method_not_allowed" }, 405);
  }

  const userId = ctx.userClaims?.id;
  if (!userId) return response({ code: "unauthenticated" }, 401);

  let body: MetadataRequest;
  try {
    body = await request.json() as MetadataRequest;
  } catch {
    return response({ code: "invalid_request" }, 400);
  }

  const eventType = body.eventType ?? "";
  const clientType = body.clientType ?? "";
  const scopes = normalizeScopes(body.scopes);
  if (!EVENTS.has(eventType) || !CLIENT_TYPES.has(clientType) || !scopes) {
    return response({ code: "invalid_request" }, 400);
  }
  if (NATIVE_ONLY_EVENTS.has(eventType)) {
    return response({ code: "native_authorization_required" }, 403);
  }

  const correlationId = crypto.randomUUID();
  const now = new Date().toISOString();
  // This function is intentionally not coupled to generated database types:
  // the repository migration is the schema authority and the service-role
  // client is used only for these two controlled metadata tables.
  const admin = ctx.supabaseAdmin as any;
  const outcome = eventType === "authorization_completed" ||
      eventType === "authorization_started"
    ? "success"
    : eventType === "authorization_denied"
    ? "denied"
    : eventType === "authorization_expired"
    ? "expired"
    : eventType === "connection_revoked"
    ? "success"
    : "failed";

  const { data, error } = await admin
    .from("oauth_connections")
    .select("id")
    .eq("user_id", userId)
    .eq("provider", PROVIDER)
    .maybeSingle();
  if (error) return response({ code: "metadata_read_failed" }, 500);
  const connectionId = (data?.id as string | undefined) ?? null;

  const { error: auditError } = await admin
    .from("oauth_audit_events")
    .insert({
      user_id: userId,
      connection_id: connectionId,
      provider: PROVIDER,
      client_type: clientType,
      event_type: eventType,
      outcome,
      scopes,
      correlation_id: correlationId,
      occurred_at: now,
    });
  if (auditError) return response({ code: "metadata_write_failed" }, 500);

  return response({ ok: true, provider: PROVIDER, eventType, correlationId });
});

export default {
  fetch: async (request: Request) => {
    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }
    return withCors(await handler(request));
  },
};
