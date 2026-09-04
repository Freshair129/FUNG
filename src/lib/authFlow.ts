// Mobile Google login: TypeScript-owned OAuth per the phase-1 pairing design
// (docs/specs/2026-08-09-phase-1-pairing-desktop-login-design.md §4) and the
// native-session-custody contract. This module generates the PKCE pair, opens
// the authorize URL in the system browser (Google blocks embedded webviews),
// and exchanges the deep-link code for tokens itself; Rust's only role is the
// deep-link scheme registration that routes fung://auth/callback back into
// the app. The Desktop shell does NOT use this module — it logs in through
// the native broker (desktopSessionBroker.ts); the legacy native
// `auth_begin_google_login` command this file once invoked was deliberately
// removed as a secret-bearing alias and must not return
// (tests/nativeSessionCustody.test.mjs pins its absence).
import { supabase } from "./supabase.ts";
import { hashPairingCode } from "./authHash.ts";

export { hashPairingCode };

const REDIRECT_URI = "fung://auth/callback";
const MAX_VALUE_LENGTH = 8192;

function safeValue(value: string | null): value is string {
  return Boolean(
    value &&
      value.length <= MAX_VALUE_LENGTH &&
      ![...value].some((character) => character < " " || character === ""),
  );
}

/** Defensive parser for the deep-link callback; PKCE binding itself is
 * enforced by the token exchange's code_verifier, not by this check. */
export function parseDeepLinkCallback(url: string): { code: string | null; error: string | null } {
  const invalid = { code: null, error: "invalid_callback" };
  try {
    const parsed = new URL(url);
    // "fung://auth/callback" parses with host "auth" and pathname "/callback".
    if (parsed.protocol !== "fung:" || `${parsed.host}${parsed.pathname}` !== "auth/callback") {
      return invalid;
    }
    const code = parsed.searchParams.get("code");
    const errorCode = parsed.searchParams.get("error");
    const errorDescription = parsed.searchParams.get("error_description");
    if ((code !== null) === (errorCode !== null)) return invalid;
    if (code !== null) return safeValue(code) ? { code, error: null } : invalid;
    if (!safeValue(errorCode)) return invalid;
    return { code: null, error: errorDescription && safeValue(errorDescription) ? errorDescription : errorCode };
  } catch {
    return invalid;
  }
}

// PKCE is owned HERE, not delegated to the supabase-js OAuth helper —
// tests/authFlow.test.mjs pins that boundary so URL construction and
// verifier handling stay explicit and auditable. The flow builds
// /auth/v1/authorize itself and exchanges the code at
// /auth/v1/token?grant_type=pkce, handing only the resulting tokens to
// supabase-js via setSession.
const VERIFIER_STORAGE_KEY = "fung.auth.pkce_verifier";
let pendingVerifier: string | null = null;

function base64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

async function createPkcePair(): Promise<{ verifier: string; challenge: string }> {
  const verifier = base64Url(crypto.getRandomValues(new Uint8Array(32)));
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return { verifier, challenge: base64Url(new Uint8Array(digest)) };
}

function rememberVerifier(verifier: string): void {
  pendingVerifier = verifier;
  try {
    sessionStorage.setItem(VERIFIER_STORAGE_KEY, verifier);
  } catch {
    // In-memory copy still covers the common same-process round trip.
  }
}

function takeVerifier(): string | null {
  const verifier = pendingVerifier ?? (() => {
    try {
      return sessionStorage.getItem(VERIFIER_STORAGE_KEY);
    } catch {
      return null;
    }
  })();
  pendingVerifier = null;
  try {
    sessionStorage.removeItem(VERIFIER_STORAGE_KEY);
  } catch {
    // Ignore.
  }
  return verifier;
}

/** Starts Google login in the system browser with a locally generated PKCE
 * pair. The webview never holds URL-opening authority: it hands only the
 * code challenge to `auth_open_google_authorize`, and native code fixes the
 * origin, path, provider, and redirect target itself. Resolution arrives via
 * the deep-link listener below. */
export async function beginGoogleLogin(): Promise<string> {
  const { verifier, challenge } = await createPkcePair();
  rememberVerifier(verifier);
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("auth_open_google_authorize", { codeChallenge: challenge });
  return challenge;
}

async function exchangeCode(code: string): Promise<string | null> {
  const supabaseUrl = import.meta.env.VITE_SUPABASE_URL as string | undefined;
  const anonKey = import.meta.env.VITE_SUPABASE_ANON_KEY as string | undefined;
  if (!supabaseUrl || !anonKey) return "auth_config_missing";
  const verifier = takeVerifier();
  if (!verifier) return "missing_verifier";
  const response = await fetch(`${supabaseUrl}/auth/v1/token?grant_type=pkce`, {
    method: "POST",
    headers: { "content-type": "application/json", apikey: anonKey },
    body: JSON.stringify({ auth_code: code, code_verifier: verifier }),
  });
  const payload: { access_token?: string; refresh_token?: string; error_description?: string; msg?: string } =
    await response.json().catch(() => ({}));
  if (!response.ok || !payload.access_token || !payload.refresh_token) {
    return payload.error_description ?? payload.msg ?? "exchange_failed";
  }
  const { error } = await supabase.auth.setSession({
    access_token: payload.access_token,
    refresh_token: payload.refresh_token,
  });
  return error ? error.message : null;
}

/** Wires the deep-link callback channel. Returns cleanup. */
export async function listenForAuthCallback(
  onDone: (err: string | null) => void,
): Promise<() => void> {
  const { onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
  let terminal = false;
  const unlisten = await onOpenUrl((urls) => {
    if (terminal) return;
    const url = urls.find((candidate) => candidate.startsWith("fung://auth/callback"));
    if (!url) return;
    terminal = true;
    const { code, error } = parseDeepLinkCallback(url);
    if (error || !code) {
      onDone(error ?? "invalid_callback");
      return;
    }
    void exchangeCode(code).then(onDone);
  });
  return () => {
    terminal = true;
    unlisten();
  };
}
