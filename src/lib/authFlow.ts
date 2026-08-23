import { supabase } from "./supabase.ts";
import { parseAuthCallbackUrl } from "./authParse.ts";
import { hashPairingCode } from "./authHash.ts";

export { hashPairingCode };

const DEEP_LINK_REDIRECT = "fung://auth/callback";

export async function beginGoogleLogin(redirectTo: string = DEEP_LINK_REDIRECT): Promise<void> {
  const { data, error } = await supabase.auth.signInWithOAuth({
    provider: "google",
    options: { redirectTo, skipBrowserRedirect: true },
  });
  if (error) throw error;
  if (!data?.url) throw new Error("ไม่ได้รับ URL สำหรับเข้าสู่ระบบ");
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke<void>("open_trusted_auth_url", { url: data.url });
}

export async function beginLoopbackFallbackLogin(): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  const port = await invoke<number>("auth_loopback_listen");
  await beginGoogleLogin(`http://127.0.0.1:${port}/auth/callback`);
}

export async function completeFromCallbackUrl(url: string): Promise<void> {
  const { code, error } = parseAuthCallbackUrl(url);
  if (error) throw new Error(error);
  if (!code) throw new Error("missing_code");
  const { error: exchangeError } = await supabase.auth.exchangeCodeForSession(code);
  if (exchangeError) throw exchangeError;
}

/** Wires BOTH callback channels (deep link + loopback event). Returns cleanup. */
export async function listenForAuthCallback(
  onDone: (err: string | null) => void,
): Promise<() => void> {
  const cleanups: Array<() => void> = [];
  try {
    const { onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
    const un = await onOpenUrl((urls) => {
      const target = urls.find((u) => u.includes("/auth/callback") || u.startsWith("fung://auth"));
      if (!target) return;
      completeFromCallbackUrl(target)
        .then(() => onDone(null))
        .catch((e) => onDone(e instanceof Error ? e.message : String(e)));
    });
    cleanups.push(un);
  } catch {
    // plugin unavailable (e.g. web preview) — loopback listener below still applies
  }
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<string>("auth-callback", (event) => {
      completeFromCallbackUrl(event.payload)
        .then(() => onDone(null))
        .catch((e) => onDone(e instanceof Error ? e.message : String(e)));
    });
    cleanups.push(un);
  } catch {
    // not in Tauri
  }
  return () => cleanups.forEach((fn) => fn());
}
