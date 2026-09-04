// Mobile Google login: TypeScript-owned OAuth per the phase-1 pairing design
// (docs/specs/2026-08-09-phase-1-pairing-desktop-login-design.md §4) and the
// native-session-custody contract. supabase-js runs the PKCE flow, the system
// browser hosts the Google page (Google blocks embedded webviews), and Rust's
// only role is the deep-link scheme registration that routes
// fung://auth/callback back into the app. The Desktop shell does NOT use this
// module — it logs in through the native broker (desktopSessionBroker.ts);
// the legacy native `auth_begin_google_login` command this file once invoked
// was deliberately removed as a secret-bearing alias and must not return
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
 * enforced by exchangeCodeForSession's stored verifier, not by this check. */
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

/** Starts Google login in the system browser. supabase-js builds the PKCE
 * authorize URL and keeps the verifier; the opener plugin leaves the webview
 * untouched. Resolution arrives via the deep-link listener below. */
export async function beginGoogleLogin(): Promise<string> {
  const { data, error } = await supabase.auth.signInWithOAuth({
    provider: "google",
    options: { redirectTo: REDIRECT_URI, skipBrowserRedirect: true },
  });
  if (error || !data?.url) throw new Error(error?.message ?? "auth_start_failed");
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(data.url);
  return data.url;
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
    void supabase.auth.exchangeCodeForSession(code).then(({ error: exchangeError }) => {
      onDone(exchangeError ? exchangeError.message : null);
    });
  });
  return () => {
    terminal = true;
    unlisten();
  };
}
