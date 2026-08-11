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
  mobileViewport: boolean;
};

type BodySurfaceInput = Pick<RootRouteInput, "path" | "isTauriRuntime">;

export function resolveRootRoute({
  path,
  surface,
  isTauriRuntime,
  mobileViewport,
}: RootRouteInput): RootRoute {
  if (isTauriRuntime) return "desktop";
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

export function hasSupabaseConfig(env: unknown): boolean {
  const config = env as
    | Partial<Record<"VITE_SUPABASE_URL" | "VITE_SUPABASE_ANON_KEY", string>>
    | undefined;
  return Boolean(
    config?.VITE_SUPABASE_URL?.trim() && config?.VITE_SUPABASE_ANON_KEY?.trim(),
  );
}

export const supabaseConfigured = hasSupabaseConfig(import.meta.env);
