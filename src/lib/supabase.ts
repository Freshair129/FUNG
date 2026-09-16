import { createClient } from "@supabase/supabase-js";
import { hasSupabaseConfig } from "./bootstrap";

const supabaseUrl = (import.meta.env.VITE_SUPABASE_URL as string | undefined)?.trim();
const supabaseAnonKey = (import.meta.env.VITE_SUPABASE_ANON_KEY as string | undefined)?.trim();

/**
 * `createClient("", "")` throws (`supabaseUrl is required.`), and the landing
 * page imports this module at the top level, so a build without
 * `VITE_SUPABASE_*` used to white-screen the public site instead of merely
 * lacking sign-in (`.brain/rca/2026-08-14-website-release-bootstrap-env.md`).
 * When unconfigured, build the client against a reserved `.invalid` host:
 * reading the local session still works (no network), the landing page
 * hides its sign-in controls via `supabaseConfigured`, and the gated routes
 * keep showing the explicit "set VITE_SUPABASE_URL" bootstrap message.
 */
const UNCONFIGURED_URL = "https://supabase.unconfigured.invalid";
const UNCONFIGURED_KEY = "unconfigured";

if (!hasSupabaseConfig(import.meta.env)) {
  console.warn("Supabase credentials missing. Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY in .env");
}

export const supabase = createClient(supabaseUrl || UNCONFIGURED_URL, supabaseAnonKey || UNCONFIGURED_KEY, {
  auth: { flowType: "pkce" },
});
