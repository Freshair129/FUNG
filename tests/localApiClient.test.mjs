// @req NFR-101
import test from "node:test";
import assert from "node:assert/strict";
import { audioUrl, isLoopbackBaseUrl, parseConnectUrl } from "../src/web/localApiClient.ts";

/**
 * The web dashboard's loopback client is the frontend's one permitted
 * `fetch`, and the egress register's claim that it "cannot reach a remote
 * host" rests on these parsers refusing anything that is not this machine.
 * A pasted string, a stored value, and a built playback URL all pass through
 * here, so each one is checked at the boundary it would be abused at.
 */

test("only loopback origins are accepted as a base URL", () => {
  assert.ok(isLoopbackBaseUrl("http://127.0.0.1:51234"));
  assert.ok(isLoopbackBaseUrl("http://localhost:1420"));
  assert.ok(isLoopbackBaseUrl("https://127.0.0.1"));
  assert.ok(isLoopbackBaseUrl("http://[::1]:8080"));

  assert.equal(isLoopbackBaseUrl("http://127.0.0.1.evil.example"), false, "prefix is not identity");
  assert.equal(isLoopbackBaseUrl("http://localhost.evil.example"), false);
  assert.equal(isLoopbackBaseUrl("http://192.168.1.10:51234"), false, "LAN is not loopback");
  assert.equal(isLoopbackBaseUrl("https://fung-seven.vercel.app"), false);
  assert.equal(isLoopbackBaseUrl("ftp://127.0.0.1"), false, "scheme allowlist");
  assert.equal(isLoopbackBaseUrl("not a url"), false);
  assert.equal(isLoopbackBaseUrl(""), false);
});

test("a connect URL is a loopback origin with the token in the fragment", () => {
  assert.deepEqual(parseConnectUrl("http://127.0.0.1:51234/#abc123"), {
    baseUrl: "http://127.0.0.1:51234",
    token: "abc123",
  });
  assert.deepEqual(parseConnectUrl("  http://localhost:9/#t  "), { baseUrl: "http://localhost:9", token: "t" }, "trimmed");
  assert.deepEqual(
    parseConnectUrl("http://127.0.0.1:51234/some/path?x=1#tok"),
    { baseUrl: "http://127.0.0.1:51234", token: "tok" },
    "only the origin is kept — a path or query in the pasted string is dropped",
  );

  assert.equal(parseConnectUrl("http://127.0.0.1:51234/"), null, "no token");
  assert.equal(parseConnectUrl("http://127.0.0.1:51234/#"), null, "empty token");
  assert.equal(parseConnectUrl("https://fung-seven.vercel.app/#abc"), null, "remote host with a token is still refused");
  assert.equal(parseConnectUrl("http://192.168.1.10:51234/#abc"), null);
  assert.equal(parseConnectUrl("127.0.0.1:51234#abc"), null, "scheme required");
  assert.equal(parseConnectUrl(""), null);
});

test("playback URLs carry channel and token and encode the id", () => {
  const connection = { baseUrl: "http://127.0.0.1:51234", token: "s3cret" };
  const url = new URL(audioUrl(connection, "rec/1 a", "mic"));
  assert.equal(url.origin, "http://127.0.0.1:51234");
  assert.equal(url.pathname, "/recordings/rec%2F1%20a/audio", "an id cannot introduce a path segment");
  assert.equal(url.searchParams.get("channel"), "mic");
  assert.equal(url.searchParams.get("token"), "s3cret");
});

test("a non-loopback connection cannot build a playback URL", () => {
  assert.throws(
    () => audioUrl({ baseUrl: "https://fung-seven.vercel.app", token: "t" }, "id", "mic"),
    /loopback/,
    "the guard must hold even for a connection object built by hand",
  );
});
