export type RootRoute =
  | "desktop"
  | "mobile"
  | "auth-callback"
  | "dashboard"
  | "landing";

type RootRouteInput = {
  path: string;
  surface: string | null;
  isTauriRuntime: boolean;
  /**
   * True when the Tauri runtime is the Android (or iOS) shell rather than a
   * desktop window. Tauri is Tauri on every platform, so `isTauriRuntime`
   * alone cannot tell a phone from a laptop — without this the Android APK
   * loads the fixed 1280x800 desktop shell and every `src/mobile/` screen
   * becomes unreachable on the device it was written for.
   */
  isMobilePlatform: boolean;
  mobileViewport: boolean;
};

type BodySurfaceInput = Pick<RootRouteInput, "path" | "isTauriRuntime">;

export function resolveRootRoute({
  path,
  surface,
  isTauriRuntime,
  isMobilePlatform,
  mobileViewport,
}: RootRouteInput): RootRoute {
  if (isTauriRuntime) {
    // An explicit `?surface=` wins so either shell can be opened on the other
    // for debugging; otherwise the platform decides.
    if (surface === "desktop") return "desktop";
    if (surface === "mobile") return "mobile";
    return isMobilePlatform ? "mobile" : "desktop";
  }
  if (path === "/auth/callback") return "auth-callback";
  if (path === "/app" && surface === "mobile") return "mobile";
  if (path === "/app" && surface === "desktop") return "desktop";
  if (path === "/app" && mobileViewport) return "dashboard";
  if (path === "/app") return "dashboard";
  return "landing";
}

export function resolveBodySurface({ path, isTauriRuntime }: BodySurfaceInput): "app" | "landing" {
  if (isTauriRuntime) return "app";
  return path === "/" || (path !== "/app" && path !== "/auth/callback")
    ? "landing"
    : "app";
}

/**
 * Detects the mobile Tauri shell from the webview user agent.
 *
 * Tauri exposes no synchronous platform constant to the webview, and the
 * `os` plugin is async — routing happens before first paint, so it cannot be
 * awaited here without flashing the wrong shell. Both mobile targets identify
 * themselves in the UA (Android's system WebView, iOS's WKWebView), which is
 * decided by the host OS and not by page content, so it is sound for this
 * one decision.
 */
export function isMobileTauriPlatform(userAgent: string): boolean {
  return /android|iphone|ipad|ipod/i.test(userAgent);
}

export function hasSupabaseConfig(env: unknown): boolean {
  const config = env as
    | Partial<Record<"VITE_SUPABASE_URL" | "VITE_SUPABASE_ANON_KEY", string>>
    | undefined;
  return Boolean(
    config?.VITE_SUPABASE_URL?.trim() && config?.VITE_SUPABASE_ANON_KEY?.trim(),
  );
}

/**
 * Whether a route must refuse to render without Supabase credentials.
 *
 * The browser-only surfaces are built around a cloud session and have nothing
 * to show without one. The two Tauri shells do: capture, transcription and
 * local review are the product, and they never touch Supabase. Blocking them
 * on a missing cloud credential would turn local-first operation into a
 * configuration error screen.
 */
export function requiresSupabaseConfig(route: RootRoute, isTauriRuntime: boolean): boolean {
  if (isTauriRuntime) return false;
  return route === "mobile" || route === "auth-callback" || route === "dashboard";
}

export const supabaseConfigured = hasSupabaseConfig(import.meta.env);
