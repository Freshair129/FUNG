// @req NFR-101
import test from "node:test";
import assert from "node:assert/strict";
import {
  audioUrl,
  fetchJob,
  fetchTranscript,
  importRecording,
  isLoopbackBaseUrl,
  LocalApiError,
  parseConnectUrl,
} from "../src/web/localApiClient.ts";

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

/**
 * The upload → job → transcript trio is the browser's only *write* to the
 * desktop and its only way to read a transcript. Driven against a recording
 * `fetch` so the exact wire shape the Rust router expects
 * (`local_api.rs` `import_recording`, `/jobs/{id}`, `/recordings/{id}/transcript`)
 * is pinned from the client side too.
 */
test("the upload posts the bytes with their type and name, and reads the receipt", async () => {
  const calls = [];
  const original = globalThis.fetch;
  globalThis.fetch = async (url, init) => {
    calls.push({ url: String(url), init });
    return new Response(JSON.stringify({ jobId: "j1", projectId: "p1", recordingId: "r1" }), {
      status: 202,
      headers: { "Content-Type": "application/json" },
    });
  };
  try {
    const blob = new Blob(["not really opus"], { type: "audio/webm;codecs=opus" });
    const receipt = await importRecording(
      { baseUrl: "http://127.0.0.1:51234", token: "tok" },
      blob,
      "fung-web-2026-09-16-1203.webm",
    );
    assert.deepEqual(receipt, { jobId: "j1", projectId: "p1", recordingId: "r1" });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, "http://127.0.0.1:51234/recordings/import");
    assert.equal(calls[0].init.method, "POST");
    assert.equal(calls[0].init.headers.Authorization, "Bearer tok");
    assert.equal(calls[0].init.headers["Content-Type"], "audio/webm;codecs=opus");
    assert.equal(calls[0].init.headers["X-Fung-Filename"], "fung-web-2026-09-16-1203.webm");
    assert.equal(calls[0].init.body, blob, "the blob itself is the body — no re-encoding, no multipart");
  } finally {
    globalThis.fetch = original;
  }
});

test("job and transcript reads hit their routes and unwrap the desktop's envelopes", async () => {
  const calls = [];
  const original = globalThis.fetch;
  globalThis.fetch = async (url) => {
    calls.push(String(url));
    if (String(url).includes("/jobs/")) {
      return Response.json({ job: { id: "j1", projectId: "p1", type: "transcript.transcribe", status: "running", progress: 40, errorCode: null, errorMessage: null } });
    }
    return Response.json({
      projectId: "p1",
      recordingId: "r1",
      transcript: { segments: [{ id: "s1", startMs: 0, endMs: 900, text: "สวัสดีครับ", speakerName: null, confidence: 0.9 }], capped: false, cap: 1000, cappedRecordingIds: [] },
    });
  };
  try {
    const connection = { baseUrl: "http://localhost:9", token: "t" };
    const job = await fetchJob(connection, "j1");
    assert.equal(job.status, "running");
    assert.equal(job.progress, 40);
    const segments = await fetchTranscript(connection, "r 1");
    assert.equal(segments.length, 1);
    assert.equal(segments[0].text, "สวัสดีครับ");
    assert.deepEqual(calls, ["http://localhost:9/jobs/j1", "http://localhost:9/recordings/r%201/transcript"]);
  } finally {
    globalThis.fetch = original;
  }
});

test("a non-loopback connection cannot upload or read, and never reaches fetch", async () => {
  const original = globalThis.fetch;
  globalThis.fetch = async () => {
    throw new Error("fetch must not be called for a non-loopback base URL");
  };
  try {
    const remote = { baseUrl: "https://fung-seven.vercel.app", token: "t" };
    for (const attempt of [
      () => importRecording(remote, new Blob(["x"]), "x.webm"),
      () => fetchJob(remote, "j1"),
      () => fetchTranscript(remote, "r1"),
    ]) {
      await assert.rejects(attempt, (error) => error instanceof LocalApiError && error.kind === "http");
    }
  } finally {
    globalThis.fetch = original;
  }
});

test("a desktop error body is surfaced by its code, not swallowed", async () => {
  const original = globalThis.fetch;
  globalThis.fetch = async () =>
    new Response(JSON.stringify({ error: "IMPORT_UNAVAILABLE", detail: "no runtime" }), { status: 503 });
  try {
    await assert.rejects(
      () => importRecording({ baseUrl: "http://127.0.0.1:1", token: "t" }, new Blob(["x"]), "x.webm"),
      (error) => error instanceof LocalApiError && error.kind === "http" && /IMPORT_UNAVAILABLE: no runtime/.test(error.message),
    );
  } finally {
    globalThis.fetch = original;
  }
});
