import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

test("Google Drive contract is an exact-scope native PKCE flow", () => {
  const rust = read("src-tauri/src/drive_oauth.rs");
  const lifecycle = read("src-tauri/src/auth_session.rs");
  assert.match(lifecycle, /SessionLifecycle/);
  assert.match(lifecycle, /RegisteredBrokerEntrypoints/);
  assert.match(`${rust}\n${lifecycle}`, /DriveConnection|DriveCredential/);
  assert.match(`${rust}\n${lifecycle}`, /drive_generation|account_epoch/);
  assert.match(`${rust}\n${lifecycle}`, /commit_marker|marker/);
  assert.match(lifecycle, /NoEntry/);
  assert.match(lifecycle, /verify_absent/);
  assert.match(rust, /drive\.appdata/);
  assert.match(rust, /code_challenge_method.*S256/);
  assert.match(lifecycle, /KeyringPort|keyring::Entry/);
  assert.match(rust, /appDataFolder/);
  assert.match(rust, /drive_archive_digest_mismatch/);
  assert.doesNotMatch(rust, /CloudProviderConfig/);
  assert.doesNotMatch(rust, /if let Ok\(value\) = .*get_password\(\)/s);
});

test("Desktop UI keeps provider backup separate from the filesystem test panel", () => {
  const panel = read("src/components/GoogleDrivePanel.tsx");
  const flow = read("src/lib/googleDriveFlow.ts");
  assert.match(panel, /เชื่อมต่อ Google Drive/);
  assert.match(panel, /อัปโหลด archive/);
  assert.match(panel, /กู้คืนจาก Google Drive/);
  assert.match(flow, /brokerDriveConnectBegin/);
  assert.match(flow, /brokerDriveConnectCancel/);
  assert.match(flow, /brokerDriveDisconnect/);
  assert.doesNotMatch(flow, /InvokeFn|args\s*\?\s*:\s*Record/);
});

test("Edge functions derive CORS from the shared module instead of a hardcoded wildcard", () => {
  const cors = read("supabase/functions/_shared/cors.ts");
  assert.match(cors, /export function buildCorsHeaders/);
  assert.match(cors, /ALLOWED_ORIGIN/);
  assert.match(cors, /Access-Control-Allow-Headers/);
  assert.match(cors, /Access-Control-Allow-Methods/);
  // The default posture (no ALLOWED_ORIGIN configured) must not hand back a
  // hardcoded "*" — see supabase/functions/_shared/cors.ts for rationale.
  assert.doesNotMatch(cors, /"Access-Control-Allow-Origin":\s*"\*"/);

  for (
    const relativePath of [
      "supabase/functions/device-enrollment/index.ts",
      "supabase/functions/google-drive-authorize/index.ts",
      "supabase/functions/google-drive-metadata/index.ts",
    ]
  ) {
    const source = read(relativePath);
    assert.match(
      source,
      /import\s*\{\s*buildCorsHeaders\s*\}\s*from\s*"\.\.\/_shared\/cors\.ts"/,
      `${relativePath} should import the shared CORS helper`,
    );
    assert.doesNotMatch(
      source,
      /"Access-Control-Allow-Origin":\s*"\*"/,
      `${relativePath} should not hardcode a wildcard CORS origin`,
    );
  }
});

test("Supabase metadata writer is authenticated and token-free", () => {
  const edgeFunction = read("supabase/functions/google-drive-metadata/index.ts");
  assert.match(edgeFunction, /withSupabase\(\{ auth: "user" \}/);
  assert.match(edgeFunction, /const PROVIDER = "google_drive"/);
  assert.match(edgeFunction, /const DRIVE_SCOPE/);
  assert.match(edgeFunction, /crypto\.randomUUID\(\)/);
  assert.doesNotMatch(edgeFunction, /refresh_token/);
  assert.doesNotMatch(edgeFunction, /access_token/);
});

test("security lane rejects caller identity and arbitrary URL authority", () => {
  const rust = read("src-tauri/src/drive_oauth.rs");
  const flow = read("src/lib/googleDriveFlow.ts");
  const authFlow = read("src/lib/authFlow.ts");
  const capabilities = read("src-tauri/capabilities/default.json");

  assert.match(read("src-tauri/src/native_auth.rs"), /AuthorizedDriveContext|DriveOperation/);
  assert.doesNotMatch(rust, /\buser_id:\s*String/);
  assert.doesNotMatch(rust, /\bdevice_id:\s*String/);
  assert.doesNotMatch(rust, /\bclient_id:\s*String/);
  assert.doesNotMatch(flow, /userId|deviceId|clientId|sessionProof|supabase/);
  assert.doesNotMatch(flow, /@tauri-apps\/plugin-opener|\bopenUrl\b/);
  assert.doesNotMatch(authFlow, /@tauri-apps\/plugin-opener|\bopenUrl\b/);
  assert.doesNotMatch(capabilities, /opener:allow-open-url/);
});

test("security lane requires native key migration, restore intent, and atomic cancellation", () => {
  const identity = read("src-tauri/src/device_identity.rs");
  const rust = read("src-tauri/src/drive_oauth.rs");
  const backup = read("src-tauri/src/backup.rs");
  const metadata = read("supabase/functions/google-drive-metadata/index.ts");

  assert.match(identity, /device_identity_keyring/);
  assert.match(identity, /readback/);
  assert.match(identity, /remove_file/);
  assert.match(rust, /OAuthTerminalState/);
  assert.match(rust, /broker_drive_restore_intent/);
  assert.match(backup, /RestoreIntent/);
  assert.doesNotMatch(metadata, /upsert\(/);
  assert.doesNotMatch(metadata, /status:\s*"revoked"/);
});

test("Drive provider work remains fenced by the live lifecycle engine", () => {
  const rust = read("src-tauri/src/drive_oauth.rs");
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(rust, /DriveOperationGuard/);
  assert.match(rust, /DriveOperationGuard::from_lease/);
  assert.match(rust, /begin_drive_work/);
  assert.match(rust, /ResumableProviderPort/);
  assert.match(rust, /NativeResumableProvider/);
  assert.match(rust, /operation\.check/);
  assert.match(session, /drive_provider_exchange/);
  assert.match(session, /drive_provider_refresh/);
  assert.match(session, /pending_operations/);
  assert.match(session, /account_begin_operation/);
  assert.match(session, /OperationDrain|wait_empty/);
  assert.match(rust, /check_drive_operation\(self\.ticket\)/);
  assert.match(rust, /blocking_delete_file\(\n\s+operation/);
  assert.match(rust, /download_file\(\n\s+operation/);
});
