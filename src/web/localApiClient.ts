/**
 * Client for the FUNG desktop's loopback API (`src-tauri/src/local_api.rs`).
 *
 * This is the one place in `src/` allowed to call `fetch`, and it may only
 * ever talk to this machine: every entry point refuses a base URL whose host
 * is not loopback, so the audio the desktop serves cannot be pointed at a
 * remote host by a stored value, a pasted string, or a later refactor.
 * `tests/egressRegister.test.mjs` pins both halves of that claim.
 *
 * The desktop shows the user a connect URL — `http://127.0.0.1:PORT/#TOKEN` —
 * generated per launch. The token rides in the fragment so it is never sent
 * in a request line; the browser keeps it in `localStorage` and presents it
 * as a bearer header (JSON) or `?token=` (`<audio src>`, which cannot carry
 * headers).
 */

export type LocalApiConnection = {
  /** Origin only, e.g. `http://127.0.0.1:51234`. */
  baseUrl: string;
  token: string;
};

export type LocalRecording = {
  id: string;
  projectId: string;
  projectName: string | null;
  source: string | null;
  status: string | null;
  durationMs: number;
  createdAt: string | null;
  language: string | null;
  /** `mic` / `system` for desktop live captures, `file` for a whole-file import. */
  channels: string[];
  chunkCount: number;
};

export type LocalApiErrorKind = "unreachable" | "unauthorized" | "http";

export class LocalApiError extends Error {
  kind: LocalApiErrorKind;

  constructor(kind: LocalApiErrorKind, message: string) {
    super(message);
    this.name = "LocalApiError";
    this.kind = kind;
  }
}

const STORAGE_KEY = "fung.localApi";

/** True only for `http(s)://127.0.0.1`, `localhost`, or `[::1]` — any port. */
export function isLoopbackBaseUrl(baseUrl: string): boolean {
  let url: URL;
  try {
    url = new URL(baseUrl);
  } catch {
    return false;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return false;
  return url.hostname === "127.0.0.1" || url.hostname === "localhost" || url.hostname === "[::1]";
}

/**
 * Parses the connect URL the desktop displays. Returns `null` for anything
 * that is not a loopback origin carrying a token in its fragment — including
 * a well-formed URL to some other host, which is the case this guards.
 */
export function parseConnectUrl(input: string): LocalApiConnection | null {
  let url: URL;
  try {
    url = new URL(input.trim());
  } catch {
    return null;
  }
  const token = url.hash.replace(/^#/, "").trim();
  if (!token) return null;
  const baseUrl = url.origin;
  if (!isLoopbackBaseUrl(baseUrl)) return null;
  return { baseUrl, token };
}

export function loadConnection(): LocalApiConnection | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return null;
    const { baseUrl, token } = parsed as Record<string, unknown>;
    if (typeof baseUrl !== "string" || typeof token !== "string" || !token) return null;
    // A stored value is data too: re-check it rather than trusting the writer.
    if (!isLoopbackBaseUrl(baseUrl)) return null;
    return { baseUrl, token };
  } catch {
    return null;
  }
}

export function saveConnection(connection: LocalApiConnection): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(connection));
  } catch {
    // Private mode or blocked storage: the session still works, it just
    // will not survive a reload.
  }
}

export function clearConnection(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Nothing to clear.
  }
}

/** URL for an `<audio src>`; the token has to ride in the query here. */
export function audioUrl(connection: LocalApiConnection, recordingId: string, channel: string): string {
  if (!isLoopbackBaseUrl(connection.baseUrl)) {
    throw new LocalApiError("http", "local API base URL must be loopback");
  }
  const url = new URL(`/recordings/${encodeURIComponent(recordingId)}/audio`, connection.baseUrl);
  url.searchParams.set("channel", channel);
  url.searchParams.set("token", connection.token);
  return url.toString();
}

export async function fetchRecordings(connection: LocalApiConnection): Promise<LocalRecording[]> {
  if (!isLoopbackBaseUrl(connection.baseUrl)) {
    throw new LocalApiError("http", "local API base URL must be loopback");
  }
  let response: Response;
  try {
    response = await fetch(new URL("/recordings", connection.baseUrl).toString(), {
      headers: { Authorization: `Bearer ${connection.token}` },
    });
  } catch (error) {
    throw new LocalApiError("unreachable", error instanceof Error ? error.message : String(error));
  }
  if (response.status === 401) {
    throw new LocalApiError("unauthorized", "the desktop rejected this token");
  }
  if (!response.ok) {
    throw new LocalApiError("http", `HTTP ${response.status}`);
  }
  const body: unknown = await response.json();
  const recordings = (body as { recordings?: unknown })?.recordings;
  return Array.isArray(recordings) ? (recordings as LocalRecording[]) : [];
}
