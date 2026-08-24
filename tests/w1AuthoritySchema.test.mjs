import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const enrollmentMigration = () =>
  read("supabase/migrations/20260823000000_w1_device_enrollment_authority.sql");
const policyMigration = () =>
  read("supabase/migrations/20260823000001_w1_drive_authorization_policy.sql");
const proofMigration = () =>
  read("supabase/migrations/20260824000000_w1_enrollment_proof_nonce.sql");

const POSTGRES_IMAGE = "postgres:17-alpine";
const dockerProbe = spawnSync(
  "docker",
  ["version", "--format", "{{.Server.Version}}"],
  { encoding: "utf8", windowsHide: true },
);
const dockerUnavailable = dockerProbe.status !== 0
  ? `Docker unavailable; PostgreSQL 17 executable evidence is skipped: ${
      dockerProbe.error?.message ?? dockerProbe.stderr?.trim() ?? "unknown error"
    }`
  : false;

const docker = (args, options = {}) =>
  spawnSync("docker", args, {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  });

const resultText = (result) =>
  `exit=${result.status}\nstdout:\n${result.stdout ?? ""}\nstderr:\n${result.stderr ?? ""}`;

const waitForPostgres = (container) => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const ready = docker([
      "exec",
      container,
      "pg_isready",
      "-U",
      "postgres",
      "-d",
      "postgres",
    ]);
    if (ready.status === 0 && /accepting connections/i.test(ready.stdout ?? "")) {
      return;
    }
    const waitBuffer = new Int32Array(new SharedArrayBuffer(4));
    Atomics.wait(waitBuffer, 0, 0, 250);
  }
  throw new Error(`PostgreSQL 17 readiness timeout for ${container}`);
};

const ensurePostgresImage = () => {
  const inspect = docker(["image", "inspect", POSTGRES_IMAGE]);
  if (inspect.status === 0) return;
  const pull = docker(["pull", POSTGRES_IMAGE]);
  assert.equal(
    pull.status,
    0,
    `Unable to pull ${POSTGRES_IMAGE}.\n${resultText(pull)}`,
  );
};

const runPsql = (container, sql) =>
  docker([
    "exec",
    "-i",
    container,
    "psql",
    "-v",
    "ON_ERROR_STOP=1",
    "-X",
    "-q",
    "-A",
    "-t",
    "-U",
    "postgres",
    "-d",
    "postgres",
  ], { input: sql });

const minimalPostgres17Prerequisites = String.raw`
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS auth;
CREATE OR REPLACE FUNCTION auth.uid() RETURNS uuid LANGUAGE sql STABLE AS $$
  SELECT NULL::uuid
$$;
CREATE ROLE anon NOLOGIN;
CREATE ROLE authenticated NOLOGIN;
CREATE ROLE service_role NOLOGIN;
CREATE TABLE public.profiles (
  id uuid PRIMARY KEY,
  display_name text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE public.devices (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  device_label text NOT NULL,
  platform text NOT NULL,
  public_key_fingerprint text NOT NULL,
  public_key text,
  registered_at timestamptz NOT NULL DEFAULT now(),
  last_seen_at timestamptz,
  revoked_at timestamptz
);
CREATE TABLE public.oauth_connections (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  provider text NOT NULL,
  provider_subject_reference text,
  approved_scopes text[] NOT NULL DEFAULT '{}',
  status text NOT NULL DEFAULT 'active',
  connected_at timestamptz NOT NULL DEFAULT now(),
  revoked_at timestamptz,
  last_authorized_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT oauth_connections_unique_provider_per_user UNIQUE (user_id, provider)
);
CREATE TABLE public.pairing_sessions (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  initiator_device_id uuid NOT NULL REFERENCES public.devices(id) ON DELETE CASCADE,
  responder_device_id uuid REFERENCES public.devices(id) ON DELETE SET NULL,
  code_hash text NOT NULL,
  status text NOT NULL DEFAULT 'pending',
  attempt_count integer NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL DEFAULT now() + interval '5 minutes',
  confirmed_at timestamptz
);
`;

const activeReplayEvidence = String.raw`
BEGIN;
DO $$
DECLARE
  v_user_id uuid := '00000000-0000-0000-0000-000000000001';
  v_device_id uuid;
  v_connection_id uuid;
  v_public_key text := encode(convert_to('w1-active-key', 'UTF8'), 'base64');
  v_fingerprint text;
  v_nonce uuid := '00000000-0000-0000-0000-000000000099';
  v_first record;
  v_replay record;
  v_reservation_count bigint;
  v_decision_count bigint;
BEGIN
  v_fingerprint := encode(
    pg_catalog.sha256(pg_catalog.decode(v_public_key, 'base64')),
    'hex'
  );
  INSERT INTO public.devices (
    user_id, device_label, platform, public_key_fingerprint, public_key,
    authority_state, enrollment_source, enrolled_at, approved_at
  ) VALUES (
    v_user_id, 'W1 active device', 'windows', v_fingerprint, v_public_key,
    'drive_trusted', 'boss_bootstrap', pg_catalog.now(), pg_catalog.now()
  ) RETURNING id INTO v_device_id;
  INSERT INTO public.oauth_connections (
    user_id, provider, approved_scopes, status, connected_at, last_authorized_at
  ) VALUES (
    v_user_id, 'google_drive',
    ARRAY['https://www.googleapis.com/auth/drive.appdata']::text[],
    'active', pg_catalog.now(), pg_catalog.now()
  ) RETURNING id INTO v_connection_id;
  INSERT INTO public.oauth_operation_grants (
    user_id, connection_id, operation, granted_by, granted_role
  ) VALUES (
    v_user_id, v_connection_id, 'backup.write',
    'w1_executable_evidence', 'database_owner'
  );

  SELECT * INTO v_first
  FROM public.authorize_oauth_request(
    v_user_id, v_device_id, v_public_key, v_fingerprint, 'backup.write',
    v_nonce, pg_catalog.now() + pg_catalog.make_interval(mins => 1)
  );
  IF v_first.authorized IS DISTINCT FROM true
    OR v_first.denial_code IS NOT NULL THEN
    RAISE EXCEPTION 'active backup.write was not allowed';
  END IF;
  SELECT count(*) INTO v_reservation_count
  FROM public.oauth_authorization_reservations
  WHERE nonce = v_nonce;
  SELECT count(*) INTO v_decision_count
  FROM public.oauth_authorization_decisions
  WHERE reservation_id = v_first.reservation_id;
  IF v_reservation_count <> 1 OR v_decision_count <> 1 THEN
    RAISE EXCEPTION 'active authorization did not durably reserve and decide';
  END IF;

  SELECT * INTO v_replay
  FROM public.authorize_oauth_request(
    v_user_id, v_device_id, v_public_key, v_fingerprint, 'backup.write',
    v_nonce, pg_catalog.now() + pg_catalog.make_interval(mins => 1)
  );
  IF v_replay.authorized IS DISTINCT FROM false
    OR v_replay.denial_code IS DISTINCT FROM 'authorization_replayed'
    OR v_replay.reservation_id IS DISTINCT FROM v_first.reservation_id THEN
    RAISE EXCEPTION 'repeated nonce was not durably rejected';
  END IF;
END;
$$;
ROLLBACK;
`;

test("S2-F2 uses a forward migration for exact proof metadata and one-use nonce reservation", () => {
  const sql = proofMigration();
  assert.match(sql, /^BEGIN;/m);
  assert.match(sql, /ALTER TABLE\s+public\.device_enrollment_requests[\s\S]*proof_version/is);
  assert.match(sql, /proof_operation/);
  assert.match(sql, /proof_nonce_hash/);
  assert.match(sql, /proof_issued_at_ms/);
  assert.match(sql, /proof_expires_at_ms/);
  assert.match(sql, /proof_envelope_hash/);
  assert.match(sql, /proof_signature/);
  assert.match(sql, /CREATE TABLE\s+public\.device_enrollment_proof_reservations/is);
  assert.match(sql, /nonce_hash\s+bytea[^;]*UNIQUE/is);
  assert.match(sql, /indefinite|retained|append-only/i);
  assert.match(sql, /ON CONFLICT[^\n]*nonce_hash[^\n]*DO NOTHING[\s\S]*RETURNING/is);
  assert.match(sql, /proof_replayed/);
  assert.match(sql, /ROLLBACK|COMMIT/);
  assert.match(sql, /create_device_enrollment_request\s*\(/i);
  assert.match(sql, /proof_version[^,]*integer|p_proof_version/i);
  assert.match(sql, /proof_operation[^,]*text|p_proof_operation/i);
  assert.match(sql, /SET\s+search_path\s*=\s*pg_catalog,\s*public,\s*pg_temp/i);
  assert.match(sql, /REVOKE\s+ALL\s+ON\s+TABLE\s+public\.device_enrollment_proof_reservations[\s\S]*service_role/is);
});

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
  assert.match(sql, /insert\s+into\s+public\.oauth_authorization_reservations[\s\S]*on\s+conflict(?:\s+on\s+constraint\s+[a-z_]+|\s*\([^)]*nonce[^)]*\))\s+do\s+nothing[\s\S]*returning/is);
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
  assert.match(sql, /authorize_oauth_request[\s\S]*insert\s+into\s+public\.oauth_authorization_reservations[\s\S]*on conflict(?:\s+on constraint\s+[a-z_]+|\s*\(nonce\))\s*do nothing[\s\S]*returning/is);
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

test(
  "W1 executable PostgreSQL 17 authority evidence runs migrations and rolls back",
  { skip: dockerUnavailable },
  () => {
    ensurePostgresImage();
    const container = `fung-w1-pg17-${process.pid}-${Date.now()}`;
    const started = docker([
      "run",
      "--rm",
      "--detach",
      "--name",
      container,
      "--env",
      "POSTGRES_PASSWORD=postgres",
      POSTGRES_IMAGE,
    ]);
    assert.equal(started.status, 0, `PostgreSQL 17 container failed to start.\n${resultText(started)}`);

    try {
      waitForPostgres(container);

      const migrations = runPsql(
        container,
        minimalPostgres17Prerequisites
          + read("supabase/migrations/20260823000000_w1_device_enrollment_authority.sql")
          + read("supabase/migrations/20260823000001_w1_drive_authorization_policy.sql")
          + proofMigration()
          + "INSERT INTO public.profiles (id, display_name) VALUES ('00000000-0000-0000-0000-000000000001', 'W1 executable evidence');\n",
      );
      assert.equal(migrations.status, 0, `PostgreSQL 17 migration apply failed.\n${resultText(migrations)}`);

      const activeReplay = runPsql(container, activeReplayEvidence);
      assert.equal(
        activeReplay.status,
        0,
        `PostgreSQL 17 active/replay evidence failed.\n${resultText(activeReplay)}`,
      );

      const committedEvidence = runPsql(
        container,
        read("supabase/tests/w1_authority_schema.sql"),
      );
      assert.equal(
        committedEvidence.status,
        0,
        `Committed PostgreSQL 17 evidence failed.\n${resultText(committedEvidence)}`,
      );

      const rollback = runPsql(
        container,
        String.raw`SELECT
          (SELECT count(*) FROM public.devices)::text || '|' ||
          (SELECT count(*) FROM public.oauth_connections)::text || '|' ||
          (SELECT count(*) FROM public.oauth_operation_grants)::text || '|' ||
          (SELECT count(*) FROM public.oauth_authorization_reservations)::text || '|' ||
          (SELECT count(*) FROM public.oauth_authorization_decisions)::text;`,
      );
      assert.equal(rollback.status, 0, `PostgreSQL 17 rollback probe failed.\n${resultText(rollback)}`);
      assert.equal(rollback.stdout.trim(), "0|0|0|0|0", resultText(rollback));
    } finally {
      docker(["rm", "--force", container]);
    }
  },
);
