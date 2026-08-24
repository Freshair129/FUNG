import { supabase } from "./supabase.ts";
import { parseAuthCallbackUrl, type AuthCallbackOptions } from "./authParse.ts";
import { hashPairingCode } from "./authHash.ts";

export { hashPairingCode };

export interface NativeAuthStarted {
  requestId: string;
  redirectUri: string;
  expiresAtMs: number;
}

export interface NativeAuthCallback {
  requestId: string;
  code: string | null;
  error: string | null;
}

let activeRequestId: string | null = null;

async function tauriInvoke() {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke;
}

/** Native creates and opens the exact OAuth URL; no caller URL is accepted. */
export async function beginGoogleLogin(): Promise<string> {
  const invoke = await tauriInvoke();
  const started = await invoke<NativeAuthStarted>("auth_begin_google_login");
  if (!started?.requestId) throw new Error("auth_start_failed");
  activeRequestId = started.requestId;
  return started.requestId;
}

export async function beginLoopbackFallbackLogin(): Promise<string> {
  return beginGoogleLogin();
}

export async function cancelGoogleLogin(requestId: string = activeRequestId ?? ""): Promise<void> {
  if (!requestId) return;
  const invoke = await tauriInvoke();
  await invoke<void>("auth_cancel_google_login", { requestId });
  if (activeRequestId === requestId) activeRequestId = null;
}

export async function completeFromCallbackUrl(
  url: string,
  options: AuthCallbackOptions,
): Promise<void> {
  const { code, error } = parseAuthCallbackUrl(url, options);
  if (error) throw new Error(error);
  if (!code) throw new Error("missing_code");
  const { error: exchangeError } = await supabase.auth.exchangeCodeForSession(code);
  if (exchangeError) throw exchangeError;
}

/** Wires the one native callback channel. Returns cleanup. */
export async function listenForAuthCallback(
  onDone: (err: string | null) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  let terminal = false;
  const unlisten = await listen<NativeAuthCallback>("auth-callback", (event) => {
    if (terminal) return;
    terminal = true;
    const callback = event.payload;
    if (!callback?.requestId || callback.requestId !== activeRequestId) {
      activeRequestId = null;
      onDone("invalid_callback");
      return;
    }
    activeRequestId = null;
    if (callback.error) {
      onDone(callback.error);
      return;
    }
    if (!callback.code) {
      onDone("missing_code");
      return;
    }
    void supabase.auth.exchangeCodeForSession(callback.code).then(({ error }) => {
      onDone(error ? error.message : null);
    });
  });
  return () => {
    terminal = true;
    unlisten();
  };
}
