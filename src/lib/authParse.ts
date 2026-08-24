export interface AuthCallbackResult {
  code: string | null;
  error: string | null;
}

export interface AuthCallbackOptions {
  expectedPort?: number;
  expectedState?: string;
}

const CALLBACK_PATH = "/auth/callback";
const MAX_VALUE_LENGTH = 8192;

function invalid(): AuthCallbackResult {
  return { code: null, error: "invalid_callback" };
}

function safeValue(value: string | null): value is string {
  return Boolean(
    value &&
      value.length <= MAX_VALUE_LENGTH &&
      ![...value].some((character) => character < " " || character === "\u007f"),
  );
}

/**
 * Parses only the callback that native FUNG opened and is currently awaiting.
 * Native performs the authoritative listener/state check; this is the
 * frontend's defensive parser for the typed callback boundary.
 */
export function parseAuthCallbackUrl(
  url: string,
  options: AuthCallbackOptions = {},
): AuthCallbackResult {
  try {
    if (
      !Number.isInteger(options.expectedPort) ||
      options.expectedPort! < 1 ||
      options.expectedPort! > 65535
    ) {
      return invalid();
    }
    if (!safeValue(options.expectedState ?? "")) return invalid();

    const parsed = new URL(url);
    if (
      parsed.protocol !== "http:" ||
      parsed.hostname !== "127.0.0.1" ||
      Number(parsed.port) !== options.expectedPort ||
      parsed.pathname !== CALLBACK_PATH ||
      parsed.username ||
      parsed.password ||
      parsed.hash
    ) {
      return invalid();
    }

    const pairs = [...parsed.searchParams.entries()];
    const names = pairs.map(([name]) => name);
    if (pairs.length === 0 || new Set(names).size !== names.length) return invalid();
    if (names.some((name) => !["code", "error", "error_description", "state"].includes(name))) {
      return invalid();
    }

    const code = parsed.searchParams.get("code");
    const errorCode = parsed.searchParams.get("error");
    const errorDescription = parsed.searchParams.get("error_description");
    const state = parsed.searchParams.get("state");
    if (state !== options.expectedState || !safeValue(state)) return invalid();

    const hasCode = code !== null;
    const hasError = errorCode !== null;
    if (hasCode === hasError || (errorDescription !== null && !hasError)) return invalid();
    if (hasCode) {
      if (pairs.length !== 2 || !safeValue(code)) return invalid();
      return { code, error: null };
    }

    if (
      (errorDescription !== null && !safeValue(errorDescription)) ||
      (errorDescription === null && pairs.length !== 2) ||
      (errorDescription !== null && pairs.length !== 3) ||
      !safeValue(errorCode)
    ) {
      return invalid();
    }
    return { code: null, error: errorDescription ?? errorCode };
  } catch {
    return invalid();
  }
}
