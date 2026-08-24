import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { parseAuthCallbackUrl } from "../src/lib/authParse.ts";
import { hashPairingCode } from "../src/lib/authHash.ts";

test("parses a native-owned loopback callback with its assigned port and state", () => {
  const r = parseAuthCallbackUrl(
    "http://127.0.0.1:49213/auth/callback?code=abc123&state=state-1",
    { expectedPort: 49213, expectedState: "state-1" },
  );
  assert.equal(r.code, "abc123");
  assert.equal(r.error, null);
});

test("parses error", () => {
  const r = parseAuthCallbackUrl(
    "http://127.0.0.1:49213/auth/callback?error=access_denied&error_description=denied&state=state-1",
    { expectedPort: 49213, expectedState: "state-1" },
  );
  assert.equal(r.code, null);
  assert.equal(r.error, "denied");
});

test("rejects a callback from the wrong host, path, port, state, or query shape", () => {
  const options = { expectedPort: 49213, expectedState: "state-1" };
  for (const url of [
    "http://localhost:49213/auth/callback?code=xyz&state=state-1",
    "http://127.0.0.1:49213/other?code=xyz&state=state-1",
    "http://127.0.0.1:49214/auth/callback?code=xyz&state=state-1",
    "http://127.0.0.1:49213/auth/callback?code=xyz&state=wrong",
    "http://127.0.0.1:49213/auth/callback?code=xyz",
    "http://127.0.0.1:49213/auth/callback?code=xyz&state=state-1&extra=1",
    "http://127.0.0.1:49213/auth/callback?code=one&code=two&state=state-1",
  ]) {
    assert.equal(parseAuthCallbackUrl(url, options).error, "invalid_callback");
  }
});

test("rejects garbage", () => {
  assert.equal(parseAuthCallbackUrl("not a url").code, null);
});

test("the frontend cannot choose the native OAuth URL or open a caller-supplied target", () => {
  const flow = readFileSync("src/lib/authFlow.ts", "utf8");
  const panel = readFileSync("src/components/AccountLoginPanel.tsx", "utf8");
  assert.match(flow, /auth_begin_google_login/);
  assert.doesNotMatch(flow, /signInWithOAuth/);
  assert.doesNotMatch(flow, /open_trusted_auth_url/);
  assert.doesNotMatch(panel, /\.from\(["']devices["']\)\s*\.(insert|update|delete)/s);
  assert.doesNotMatch(panel, /\.from\(["']device_audit_events["']\)\s*\.(insert|update|delete)/s);
  assert.match(panel, /functions\.invoke\(["']device-enrollment["']/);
});

test("hashPairingCode matches node:crypto sha256 cross-check", async () => {
  const sessionId = "11111111-1111-1111-1111-111111111111";
  const code = "123456";
  const expected = createHash("sha256").update(`${sessionId}:${code}`).digest("hex");
  const actual = await hashPairingCode(sessionId, code);
  assert.equal(actual, expected);
});
