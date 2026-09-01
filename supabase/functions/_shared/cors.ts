// Shared CORS header builder for the fung Supabase edge functions.
//
// Every real caller of device-enrollment, google-drive-authorize, and
// google-drive-metadata is the desktop app's native HTTP client
// (`reqwest`, see `native_post` in src-tauri/src/auth_session.rs). That
// client never sends an `Origin` header and never evaluates
// `Access-Control-*` response headers at all — CORS is a mechanism browsers
// enforce on themselves, and a non-browser HTTP client ignores it entirely.
// These are also authorization-granting endpoints (device enrollment,
// OAuth grant issuance, audit writes), not public read APIs, so a blanket
// `Access-Control-Allow-Origin: "*"` served no purpose for the real (native)
// caller while giving any web page in any browser the ability to read an
// authenticated user's response if it could otherwise reach the endpoint.
//
// Default posture (deliberate): with no `ALLOWED_ORIGIN` configured, the
// `Access-Control-Allow-Origin` header is omitted entirely. Browsers then
// fail the CORS check and refuse to expose the response to page script;
// native callers are completely unaffected since they never look at this
// header. Set `ALLOWED_ORIGIN` to a comma-separated allowlist (e.g.
// `https://app.example.com,https://admin.example.com`) to opt specific
// browser origins in — each request's `Origin` is reflected back only when
// it is a member of that allowlist (never a bare wildcard alongside
// credentials-bearing responses), matching the standard
// allowlist-and-reflect CORS pattern. `ALLOWED_ORIGIN=*` remains available
// as an explicit opt-out for local/dev use.
const configuredOrigins = (Deno.env.get("ALLOWED_ORIGIN") ?? "")
  .split(",")
  .map((origin) => origin.trim())
  .filter((origin) => origin.length > 0);

/**
 * Builds the CORS response headers for a request. `Access-Control-Allow-Headers`
 * and `Access-Control-Allow-Methods` are always present, preserving the
 * existing behavior all three functions relied on. `Access-Control-Allow-Origin`
 * is included only when an allowlist is configured via `ALLOWED_ORIGIN` and
 * the incoming request's `Origin` (or `*`) matches it.
 */
export function buildCorsHeaders(
  requestOrigin: string | null,
): Record<string, string> {
  const headers: Record<string, string> = {
    "Access-Control-Allow-Headers":
      "authorization, x-client-info, apikey, content-type",
    "Access-Control-Allow-Methods": "POST, OPTIONS",
  };

  if (configuredOrigins.length === 0) {
    return headers;
  }

  if (configuredOrigins.includes("*")) {
    headers["Access-Control-Allow-Origin"] = "*";
    return headers;
  }

  if (requestOrigin && configuredOrigins.includes(requestOrigin)) {
    headers["Access-Control-Allow-Origin"] = requestOrigin;
    headers["Vary"] = "Origin";
  }

  return headers;
}
