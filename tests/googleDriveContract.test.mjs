import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

test("Google Drive contract is an exact-scope native PKCE flow", () => {
  const rust = read("src-tauri/src/drive_oauth.rs");
  assert.match(rust, /drive\.appdata/);
  assert.match(rust, /code_challenge_method.*S256/);
  assert.match(rust, /keyring::Entry/);
  assert.match(rust, /appDataFolder/);
  assert.match(rust, /drive_archive_digest_mismatch/);
  assert.doesNotMatch(rust, /CloudProviderConfig/);
});

test("Desktop UI keeps provider backup separate from the filesystem test panel", () => {
  const panel = read("src/components/GoogleDrivePanel.tsx");
  const flow = read("src/lib/googleDriveFlow.ts");
  assert.match(panel, /เชื่อมต่อ Google Drive/);
  assert.match(panel, /อัปโหลด archive/);
  assert.match(panel, /กู้คืนจาก Google Drive/);
  assert.match(flow, /drive_oauth_start/);
  assert.match(flow, /drive_oauth_cancel/);
  assert.match(flow, /drive_disconnect/);
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

  assert.match(read("src-tauri/src/native_auth.rs"), /AuthorizedDriveContext/);
  assert.doesNotMatch(rust, /\buser_id:\s*String/);
  assert.doesNotMatch(rust, /\bdevice_id:\s*String/);
  assert.doesNotMatch(rust, /\bclient_id:\s*String/);
  assert.doesNotMatch(flow, /userId|deviceId|clientId/);
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
  assert.match(rust, /drive_restore_intent_create/);
  assert.match(backup, /RestoreIntent/);
  assert.doesNotMatch(metadata, /upsert\(/);
  assert.doesNotMatch(metadata, /status:\s*"revoked"/);
});
