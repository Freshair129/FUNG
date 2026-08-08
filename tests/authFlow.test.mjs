import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { parseAuthCallbackUrl } from "../src/lib/authParse.ts";
import { hashPairingCode } from "../src/lib/authHash.ts";

test("parses code from deep link url", () => {
  const r = parseAuthCallbackUrl("fung://auth/callback?code=abc123");
  assert.equal(r.code, "abc123");
  assert.equal(r.error, null);
});

test("parses error", () => {
  const r = parseAuthCallbackUrl("fung://auth/callback?error=access_denied&error_description=denied");
  assert.equal(r.code, null);
  assert.equal(r.error, "denied");
});

test("parses loopback url", () => {
  const r = parseAuthCallbackUrl("http://127.0.0.1:49213/auth/callback?code=xyz");
  assert.equal(r.code, "xyz");
});

test("rejects garbage", () => {
  assert.equal(parseAuthCallbackUrl("not a url").code, null);
});

test("hashPairingCode matches node:crypto sha256 cross-check", async () => {
  const sessionId = "11111111-1111-1111-1111-111111111111";
  const code = "123456";
  const expected = createHash("sha256").update(`${sessionId}:${code}`).digest("hex");
  const actual = await hashPairingCode(sessionId, code);
  assert.equal(actual, expected);
});
