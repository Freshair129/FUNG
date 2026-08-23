import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const enrollmentMigration = () =>
  read("supabase/migrations/20260823000000_w1_device_enrollment_authority.sql");
const policyMigration = () =>
  read("supabase/migrations/20260823000001_w1_drive_authorization_policy.sql");

test("W1 device authority is legacy-first and database-owner gated", () => {
  const sql = enrollmentMigration();
  assert.match(sql, /authority_state\s+text\s+not\s+null\s+default\s+'legacy'/i);
  assert.match(sql, /authority_state[^;]*(?:pairing_only[^;]*drive_trusted[^;]*revoked|drive_trusted[^;]*pairing_only[^;]*revoked)/is);
  assert.match(sql, /update\s+public\.devices\s+set\s+authority_state\s*=\s*'legacy'/i);
  assert.match(sql, /create\s+table\s+(?:if\s+not\s+exists\s+)?public\.device_enrollment_requests/is);
  assert.match(sql, /native_proof/i);
  assert.match(sql, /expires_at/i);
  assert.match(sql, /unique[^;]*public_key_fingerprint[^;]*pending[^;]*approved/is);
  assert.match(sql, /revoke\s+(?:all|insert)[^;]*public\.devices[^;]*public[^;]*anon[^;]*authenticated[^;]*service_role/is);
  assert.match(sql, /drop\s+policy[^;]*devices:[^;]*(?:register|updates|removes)/is);
  assert.match(sql, /approve_bootstrap_enrollment\s*\(/i);
  assert.match(sql, /revoke\s+execute[^;]*approve_bootstrap_enrollment[^;]*public[^;]*anon[^;]*authenticated[^;]*service_role/is);
  assert.match(sql, /approve_rebind_enrollment\s*\(/i);
  assert.match(sql, /authority_state\s*=\s*'revoked'[\s\S]*revocation_reason\s*=\s*'approved_rebind'/i);
  assert.match(sql, /enrollment_source[\s\S]*'approved_rebind'/i);
  assert.match(sql, /pg_catalog\.pg_get_userbyid[\s\S]*pg_catalog\.current_database/i);
  assert.match(sql, /security\s+definer\s+set\s+search_path\s*=\s*pg_catalog,\s*public,\s*pg_temp/is);
  assert.match(sql, /create_pairing_session\s*\(/i);
  assert.match(sql, /authority_state\s*=\s*case[\s\S]*'pairing_only'/i);
  assert.match(sql, /confirm_pairing\s*\(/i);
  assert.match(sql, /p_responder_device_id[\s\S]*revoked_at\s+is\s+null/i);

  const enrollmentEdge = read("supabase/functions/device-enrollment/index.ts");
  assert.match(enrollmentEdge, /withSupabase\(\{\s*auth:\s*["']user["']/s);
  assert.match(enrollmentEdge, /ctx\.userClaims\?\.id/);
  assert.match(enrollmentEdge, /create_device_enrollment_request/);
  assert.match(enrollmentEdge, /register_pairing_device/);
  assert.match(enrollmentEdge, /revoke_device_for_user/);
  assert.doesNotMatch(enrollmentEdge, /approve_bootstrap_enrollment/);
  assert.doesNotMatch(enrollmentEdge, /from\(["']devices["']\)\s*\.insert/s);
});

test("W1 operation grants are independent and replay reservation is durable", () => {
  const sql = policyMigration();
  assert.match(sql, /create\s+table\s+(?:if\s+not\s+exists\s+)?public\.oauth_operation_grants/is);
  assert.match(sql, /backup\.write/);
  assert.match(sql, /backup\.restore/);
  assert.match(sql, /check[^;]*backup\.write[^;]*backup\.restore/is);
  assert.match(sql, /alter\s+table\s+public\.oauth_operation_grants\s+enable\s+row\s+level\s+security/i);
  assert.match(sql, /revoke\s+all[^;]*oauth_operation_grants[^;]*anon[^;]*authenticated[^;]*service_role/is);
  assert.match(sql, /create\s+table\s+(?:if\s+not\s+exists\s+)?public\.oauth_authorization_reservations/is);
  assert.match(sql, /unique[^;]*nonce/is);
  assert.match(sql, /insert\s+into\s+public\.oauth_authorization_reservations[\s\S]*on\s+conflict\s+\([^)]*nonce[^)]*\)\s+do\s+nothing[\s\S]*returning/is);
  assert.match(sql, /oauth_authorization_decisions/i);
  assert.match(sql, /revoke\s+execute[^;]*reserve_oauth_authorization[^;]*public[^;]*anon[^;]*authenticated/is);
  assert.match(sql, /revoke_oauth_operation_grant/i);
  assert.match(sql, /connection[^;]*revok[^;]*oauth_operation_grants/is);
  assert.match(sql, /device[^;]*revok[^;]*oauth_operation_grants/is);
  assert.match(sql, /device_revokes_oauth_operation_grants/i);
});

test("Drive Edge authority uses the exact device predicate, grants, and RPC lock", () => {
  const authorize = read("supabase/functions/google-drive-authorize/index.ts");
  const metadata = read("supabase/functions/google-drive-metadata/index.ts");
  const lock = read("deno.lock");

  assert.match(authorize, /npm:@supabase\/server@1\.4\.1/);
  assert.match(authorize, /deviceId/);
  assert.match(authorize, /platform["']?\s*[,)]?\s*["']windows["']/i);
  assert.match(authorize, /drive_trusted/);
  assert.match(authorize, /boss_bootstrap/);
  assert.match(authorize, /approved_rebind/);
  assert.match(authorize, /revoked_at/);
  assert.match(authorize, /oauth_operation_grants/);
  assert.match(authorize, /backup\.write/);
  assert.match(authorize, /backup\.restore/);
  assert.match(authorize, /reserve_oauth_authorization/);
  assert.match(authorize, /connectionStatus/);
  assert.match(authorize, /writeGrant/);
  assert.match(authorize, /restoreGrant/);
  assert.doesNotMatch(authorize, /replayedNonces|purgeReplay|priorAudit|new\s+Map/);
  assert.doesNotMatch(authorize, /from\(["']oauth_audit_events["']\)\s*\n?\s*\.select/s);

  assert.match(metadata, /npm:@supabase\/server@1\.4\.1/);
  assert.doesNotMatch(metadata, /oauth_operation_grants|reserve_oauth_authorization|approve_bootstrap_enrollment/);
  assert.match(lock, /npm:@supabase\/server@1\.4\.1/);
  assert.doesNotMatch(lock, /npm:@supabase\/server@\*/);
});

test("Committed SQL evidence covers privileges, fixed search paths, and no project authority", () => {
  const sql = read("supabase/tests/w1_authority_schema.sql");
  assert.match(sql, /has_table_privilege/i);
  assert.match(sql, /has_function_privilege/i);
  assert.match(sql, /relrowsecurity/i);
  assert.match(sql, /proconfig/i);
  assert.match(sql, /service_role/i);
  assert.match(sql, /pg_catalog,\s*public,\s*pg_temp/i);
  assert.match(sql, /concurrent|50/i);
  assert.doesNotMatch(sql, /--project-ref\s+[A-Za-z0-9_-]+/i);
});
