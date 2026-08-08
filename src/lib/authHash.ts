/**
 * Pure WebCrypto helper — no supabase import, so this loads standalone under
 * plain `node --test` (supabase.ts reads `import.meta.env`, a Vite-only
 * global that throws under plain Node).
 */

/** sha256(`${sessionId}:${code}`) lowercase hex — MUST match the SQL expression in confirm_pairing. */
export async function hashPairingCode(sessionId: string, code: string): Promise<string> {
  const data = new TextEncoder().encode(`${sessionId}:${code}`);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
