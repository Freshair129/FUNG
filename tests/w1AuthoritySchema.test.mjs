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
  assert.match(sql, /revoke\s+execute[^;]*authorize_oauth_request[^;]*public[^;]*anon[^;]*authenticated/is);
  assert.match(sql, /authorize_oauth_request/i);
  assert.match(sql, /revoke_oauth_operation_grant/i);
  assert.match(sql, /connection[^;]*revok[^;]*oauth_operation_grants/is);
  assert.match(sql, /device[^;]*revok[^;]*oauth_operation_grants/is);
  assert.match(sql, /device_revokes_oauth_operation_grants/i);
});

test("W1 migrations and authorization decision have one explicit transaction boundary", () => {
  for (const sql of [enrollmentMigration(), policyMigration()]) {
    const lines = sql.split(/\r?\n/).map((line) => line.trim());
    const firstCodeLine = lines.find((line) => line && !line.startsWith("--"));
    const lastCodeLine = [...lines].reverse().find((line) =>
      line && !line.startsWith("--")
    );
    assert.equal(firstCodeLine, "BEGIN;");
    assert.equal(lastCodeLine, "COMMIT;");
  }

  const sql = policyMigration();
  assert.match(sql, /authorize_oauth_request\s*\(/i);
  assert.match(sql, /lock order[^\r\n]*device[^\r\n]*connection[^\r\n]*grant[^\r\n]*nonce/i);
  assert.match(sql, /authorize_oauth_request[\s\S]*from\s+public\.devices[\s\S]*for update/is);
  assert.match(sql, /authorize_oauth_request[\s\S]*from\s+public\.oauth_connections[\s\S]*for update/is);
  assert.match(sql, /authorize_oauth_request[\s\S]*from\s+public\.oauth_operation_grants[\s\S]*for update/is);
  assert.match(sql, /authorize_oauth_request[\s\S]*insert\s+into\s+public\.oauth_authorization_reservations[\s\S]*on conflict\s*\(nonce\)\s*do nothing[\s\S]*returning/is);
  assert.match(sql, /authorize_oauth_request[\s\S]*insert\s+into\s+public\.oauth_authorization_decisions/is);
  assert.match(sql, /authorize_oauth_request[\s\S]*is_drive_authorized_desktop/is);
});

test("Drive Edge authority uses the exact device predicate, grants, and RPC lock", () => {
  const authorize = read("supabase/functions/google-drive-authorize/index.ts");
  const metadata = read("supabase/functions/google-drive-metadata/index.ts");
  const lock = read("deno.lock");

  assert.match(authorize, /npm:@supabase\/server@1\.4\.1/);
  assert.match(authorize, /deviceId/);
  assert.match(authorize, /public_key/);
  assert.match(authorize, /backup\.write/);
  assert.match(authorize, /backup\.restore/);
  assert.match(authorize, /connectionStatus/);
  assert.match(authorize, /writeGrant/);
  assert.match(authorize, /restoreGrant/);
  assert.doesNotMatch(authorize, /replayedNonces|purgeReplay|priorAudit|new\s+Map/);
  assert.doesNotMatch(authorize, /from\(["']oauth_audit_events["']\)\s*\n?\s*\.select/s);

  assert.match(authorize, /authorize_oauth_request/);
  assert.doesNotMatch(authorize, /reserve_oauth_authorization/);
  assert.doesNotMatch(authorize, /record_oauth_authorization_decision/);
  assert.doesNotMatch(authorize, /recordDecision/);
  assert.doesNotMatch(authorize, /from\(["']oauth_connections["']\)\s*\n?\s*\.select/s);
  assert.doesNotMatch(authorize, /from\(["']oauth_operation_grants["']\)/s);
  const signatureIndex = authorize.indexOf("const validSignature");
  const decisionRpcIndex = authorize.indexOf("authorize_oauth_request");
  assert.ok(signatureIndex >= 0 && decisionRpcIndex > signatureIndex);

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
  assert.match(sql, /has_function_privilege[\s\S]*is_drive_authorized_desktop/i);
  assert.match(sql, /is_drive_authorized_desktop\(uuid, uuid\)[\s\S]*search_path/i);
  assert.match(sql, /concurrent|50/i);
  assert.doesNotMatch(sql, /--project-ref\s+[A-Za-z0-9_-]+/i);
});

test("W1 revoked connection activation is denied without reactivation", () => {
  const sql = policyMigration();
  assert.match(
    sql,
    /elsif\s+p_operation\s*=\s*'connection\.activate'[\s\S]*?and\s+v_connection_found[\s\S]*?and\s*\([\s\S]*?v_connection\.status\s*=\s*'revoked'[\s\S]*?or\s+v_connection\.revoked_at\s+is\s+not\s+null[\s\S]*?\)[\s\S]*?then[\s\S]*?v_denial_code\s*:=\s*'connection_revoked'/i,
  );
  assert.match(
    sql,
    /if\s+v_authorized\s+and\s+p_operation\s*=\s*'connection\.activate'[\s\S]*?on\s+conflict\s*\(user_id,\s*provider\)/i,
  );
});
