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

test("browser/Mobile auth adapter remains separate from the Desktop broker", () => {
  const flow = readFileSync("src/lib/authFlow.ts", "utf8");
  const panel = readFileSync("src/components/AccountLoginPanel.tsx", "utf8");
  const broker = readFileSync("src/lib/desktopSessionBroker.ts", "utf8");
  assert.doesNotMatch(flow, /signInWithOAuth/);
  assert.doesNotMatch(flow, /open_trusted_auth_url/);
  assert.match(panel, /desktopSessionBroker/);
  assert.doesNotMatch(panel, /supabase|sessionProof|access_token|auth-callback|setSession/);
  assert.match(broker, /broker_session_login_begin/);
  assert.match(broker, /broker_session_status/);
});

test("native owns PKCE exchange and never emits a token-bearing WebView session", () => {
  const native = readFileSync("src-tauri/src/native_auth.rs", "utf8");
  const session = readFileSync("src-tauri/src/auth_session.rs", "utf8");

  assert.match(session, /code_verifier/);
  assert.match(session, /code_challenge/);
  assert.match(session, /code_challenge_method["']?\s*[,=:]\s*["']S256["']/i);
  assert.match(session, /auth\/v1\/token/);
  assert.match(session, /auth\/v1\/user/);
  assert.match(session, /Zeroizing/);
  assert.doesNotMatch(native, /AuthCallbackEvent|auth-callback|emit_auth_callback|sessionProof/);
  assert.match(session, /keyring::Entry/);
  assert.match(session, /Zeroizing/);
  assert.match(session, /generation/);
});

test("enrollment proof crosses the typed canonical envelope and is verified at Edge", () => {
  const native = readFileSync("src-tauri/src/native_auth.rs", "utf8");
  const panel = readFileSync("src/components/AccountLoginPanel.tsx", "utf8");
  const edge = readFileSync("supabase/functions/device-enrollment/index.ts", "utf8");

  assert.match(native, /FUNG\\0DEVICE_ENROLLMENT\\0V1\\0/);
  for (const field of [
    "version",
    "operation",
    "user_id",
    "platform",
    "device_label",
    "issued_at_ms",
    "expires_at_ms",
    "nonce",
    "signature",
  ]) {
    assert.match(native, new RegExp(field));
  }
  assert.doesNotMatch(panel, /nativeProof|sessionProof|access_token/);
  assert.match(edge, /canonicalEnrollmentProof/);
  assert.match(edge, /crypto\.subtle\.verify/);
  assert.match(edge, /nonce_hash/i);
  assert.match(edge, /proof_replayed/);
});

test("hashPairingCode matches node:crypto sha256 cross-check", async () => {
  const sessionId = "11111111-1111-1111-1111-111111111111";
  const code = "123456";
  const expected = createHash("sha256").update(`${sessionId}:${code}`).digest("hex");
  const actual = await hashPairingCode(sessionId, code);
  assert.equal(actual, expected);
});
