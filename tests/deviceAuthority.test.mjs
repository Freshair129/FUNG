// @req NFR-101
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

/**
 * W1 (supabase/migrations/20260823000000_w1_device_enrollment_authority.sql)
 * made `public.devices` server-owned: signed-in clients keep SELECT only.
 * That was applied to the live project on 2026-09-13 and immediately broke
 * three client writes that had been fine before — web/mobile revoke via
 * `.delete()`, mobile registration via `.insert()`/`.update()`, and the
 * desktop's LAN-endpoint PATCH. Nothing in the migration reminds a client
 * author of the boundary, so this suite pins it from the client side.
 */

const read = (path) => readFileSync(path, "utf8");

/** Rust source with the test module cut off, as in the egress suite. */
function productionRust(path) {
  const source = read(path);
  const testModule = source.search(/#\[cfg\(test\)\]\s*\nmod /);
  return testModule === -1 ? source : source.slice(0, testModule);
}

function frontendSources() {
  return readdirSync("src", { recursive: true })
    .map((name) => `src/${String(name).replace(/\\/g, "/")}`)
    .filter((path) => /\.tsx?$/.test(path))
    .map((path) => ({ path, source: read(path) }));
}

test("no client writes public.devices directly — W1 leaves signed-in roles SELECT-only", () => {
  const offenders = frontendSources()
    .filter(({ source }) => /from\(["']devices["']\)\s*\.\s*(insert|update|upsert|delete)\s*\(/s.test(source))
    .map(({ path }) => path);
  assert.deepEqual(
    offenders,
    [],
    `${offenders.join(", ")} writes public.devices from the browser/webview; go through src/lib/deviceAuthority.ts`,
  );

  const native = productionRust("src-tauri/src/auth_session.rs");
  assert.doesNotMatch(native, /\.patch\(/, "the native broker must not PATCH /rest/v1/devices");
  assert.match(
    native,
    /rest\/v1\/rpc\/publish_device_endpoint/,
    "the desktop publishes its LAN endpoint through the owner-scoped RPC",
  );
});

test("web and mobile revoke and register only through the device-enrollment function", () => {
  const authority = read("src/lib/deviceAuthority.ts");
  assert.match(authority, /functions\s*\.\s*invoke(?:<[^>]*>)?\(\s*["']device-enrollment["']/);
  assert.match(authority, /action:\s*["']revoke["']/);
  assert.match(authority, /action:\s*["']pairing_only["']/);
  assert.doesNotMatch(
    authority,
    /approve_bootstrap_enrollment|approve_rebind_enrollment|action:\s*["']pending["']/,
    "a browser client never requests trusted enrollment; that is the desktop's native proof path",
  );

  assert.match(read("src/web/usePairedDevices.ts"), /revokeCloudDevice\(/);
  const mobile = read("src/mobile/MobileApp.tsx");
  assert.match(mobile, /revokeCloudDevice\(/);
  assert.match(mobile, /registerPairingDevice\(/);
  // Revocation is soft: a revoked row still exists, so liveness must read
  // `revoked_at` rather than infer it from a missing row.
  assert.match(mobile, /select\(["']id,\s*revoked_at["']\)/);
  assert.match(read("src/web/usePairedDevices.ts"), /\.is\(["']revoked_at["'],\s*null\)/);
});

test("publish_device_endpoint is owner-scoped, revocation-aware, and never touches authority", () => {
  const sql = read("supabase/migrations/20260913000001_publish_device_endpoint.sql");
  assert.match(sql, /security definer/i);
  assert.match(sql, /set search_path = pg_catalog, public, pg_temp/i);
  assert.match(sql, /auth\.uid\(\)/);
  assert.match(sql, /user_id = v_user_id/i);
  assert.match(sql, /revoked_at is null/i);
  assert.match(sql, /authority_state <> 'revoked'/i);
  assert.match(sql, /revoke execute[\s\S]*from public, anon, service_role/i);
  assert.match(sql, /grant execute[\s\S]*to authenticated/i);
  assert.doesNotMatch(sql, /set\s+authority_state|set\s+enrollment_source|revoked_at\s*=/i);
});
