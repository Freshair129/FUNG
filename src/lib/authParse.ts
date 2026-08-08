export interface AuthCallbackResult {
  code: string | null;
  error: string | null;
}

export function parseAuthCallbackUrl(url: string): AuthCallbackResult {
  try {
    const normalized = url.startsWith("fung://")
      ? url.replace("fung://", "https://fung.local/")
      : url;
    const parsed = new URL(normalized);
    const error =
      parsed.searchParams.get("error_description") ?? parsed.searchParams.get("error");
    return { code: parsed.searchParams.get("code"), error };
  } catch {
    return { code: null, error: "invalid_url" };
  }
}
