import test from "node:test";
import assert from "node:assert/strict";
import {
  reconcileMobileDevice,
  deviceCacheActionForSession,
} from "../src/lib/deviceReconcile.ts";

function fakePort({ ownedRow = null } = {}) {
  const calls = { find: [], insert: [], refresh: [], audit: [] };
  const port = {
    findOwnedDevice: async (userId, fingerprint) => {
      calls.find.push({ userId, fingerprint });
      return ownedRow;
    },
    insertDevice: async (userId, fingerprint) => {
      calls.insert.push({ userId, fingerprint });
      return { id: "new-row-id" };
    },
    refreshDevice: async (deviceId, publicKey) => {
      calls.refresh.push({ deviceId, publicKey });
    },
    auditRegistered: async (userId, deviceId) => {
      calls.audit.push({ userId, deviceId });
    },
  };
  return { port, calls };
}

test("valid session with an owned row reuses it and avoids a duplicate insert", async () => {
  const { port, calls } = fakePort({ ownedRow: { id: "row-1" } });
  const outcome = await reconcileMobileDevice(port, "user-1", "fp-1", "pk", "row-1");
  assert.equal(outcome.deviceId, "row-1");
  assert.equal(outcome.registered, false);
  assert.equal(outcome.cacheStale, false);
  assert.equal(calls.insert.length, 0);
  assert.equal(calls.refresh.length, 1);
  // Ownership is part of the lookup, not an afterthought.
  assert.deepEqual(calls.find[0], { userId: "user-1", fingerprint: "fp-1" });
});

test("stale cached id is reported so the caller replaces the cache", async () => {
  const { port } = fakePort({ ownedRow: { id: "row-current" } });
  const outcome = await reconcileMobileDevice(port, "user-1", "fp-1", "pk", "row-stale");
  assert.equal(outcome.deviceId, "row-current");
  assert.equal(outcome.cacheStale, true);
});

test("missing row registers exactly one android row and audits it", async () => {
  const { port, calls } = fakePort({ ownedRow: null });
  const outcome = await reconcileMobileDevice(port, "user-1", "fp-1", "pk", null);
  assert.equal(outcome.deviceId, "new-row-id");
  assert.equal(outcome.registered, true);
  assert.equal(outcome.cacheStale, true);
  assert.equal(calls.insert.length, 1);
  assert.equal(calls.audit.length, 1);
  // public key is set through the refresh path (UPDATE grant), not INSERT.
  assert.deepEqual(calls.refresh[0], { deviceId: "new-row-id", publicKey: "pk" });
});

test("revoked-elsewhere device (cache points at a deleted row) re-registers", async () => {
  const { port, calls } = fakePort({ ownedRow: null });
  const outcome = await reconcileMobileDevice(port, "user-1", "fp-1", "pk", "row-revoked");
  assert.equal(outcome.deviceId, "new-row-id");
  assert.equal(outcome.cacheStale, true);
  assert.equal(calls.insert.length, 1);
});

test("missing session or identity never touches the devices table", async () => {
  const { port, calls } = fakePort();
  await assert.rejects(() => reconcileMobileDevice(port, "", "fp-1", "pk", null), /missing_session/);
  await assert.rejects(
    () => reconcileMobileDevice(port, "user-1", "", "pk", null),
    /missing_device_identity/,
  );
  assert.equal(calls.find.length, 0);
  assert.equal(calls.insert.length, 0);
});

test("cache is cleared on signed-out or revoked sessions and kept on valid ones", () => {
  assert.equal(deviceCacheActionForSession(false), "clear");
  assert.equal(deviceCacheActionForSession(true), "keep");
});
