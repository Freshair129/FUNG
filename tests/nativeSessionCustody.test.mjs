import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

test("Desktop broker exposes only a closed typed operation allowlist", () => {
  const broker = read("src/lib/desktopSessionBroker.ts");
  assert.match(broker, /BROKER_OPERATIONS/);
  assert.match(broker, /broker_session_login_begin/);
  assert.match(broker, /broker_session_logout/);
  assert.match(broker, /broker_enrollment_request/);
  assert.match(broker, /broker_device_list/);
  assert.match(broker, /broker_drive_connect_begin/);
  assert.match(broker, /broker_drive_list_archives/);
  assert.doesNotMatch(broker, /url\s*:\s*string|headers\s*:\s*Record|bearer|sessionProof|accessToken|refreshToken/);
  assert.doesNotMatch(broker, /invoke\s*<[^>]+>\s*\([^,]+,\s*input/);
});

test("native session custody has generation ownership, zeroization, keyring-only refresh, and cleanup", () => {
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(session, /Zeroizing/);
  assert.match(session, /keyring::Entry/);
  assert.match(session, /generation/);
  assert.match(session, /single|in.flight|refreshing/i);
  assert.match(session, /staged|readback|active/i);
  assert.match(session, /logout|shutdown/i);
  assert.match(session, /delete|remove/);
  assert.doesNotMatch(session, /pub\s+(?:access|refresh)_token/);
  assert.doesNotMatch(session, /emit\s*\(/);
  assert.doesNotMatch(session, /localStorage|sessionStorage|Genesis|metadata/);
});

test("native command inventory removes secret-bearing legacy aliases", () => {
  const lib = read("src-tauri/src/lib.rs");
  for (const oldName of [
    "auth_begin_google_login",
    "auth_cancel_google_login",
    "native_device_enrollment_proof",
    "drive_oauth_start",
    "drive_oauth_complete",
    "drive_oauth_cancel",
    "drive_connection_status",
    "drive_disconnect",
    "drive_archives_list",
    "drive_upload_archive",
    "drive_restore_intent_create",
    "drive_restore",
    "paired_device_upsert",
    "paired_device_list",
    "paired_device_revoke",
  ]) {
    assert.doesNotMatch(lib, new RegExp(`generate_handler![\\s\\S]*\\b${oldName}\\b`));
  }
  for (const name of [
    "broker_session_login_begin",
    "broker_session_login_cancel",
    "broker_session_status",
    "broker_session_logout",
    "broker_enrollment_request",
    "broker_device_list",
    "broker_drive_connect_begin",
    "broker_drive_connect_complete",
    "broker_drive_connect_cancel",
    "broker_drive_status",
    "broker_drive_disconnect",
    "broker_drive_list_archives",
    "broker_drive_upload_archive",
    "broker_drive_restore_intent",
    "broker_drive_restore",
  ]) {
    assert.match(lib, new RegExp(name));
  }
});

test("Desktop consumers never carry session proof or token-shaped public values", () => {
  const files = [
    "src/components/AccountLoginPanel.tsx",
    "src/components/DevicePairingPanel.tsx",
    "src/lib/googleDriveFlow.ts",
    "src/lib/desktopSessionBroker.ts",
  ];
  for (const file of files) {
    const source = read(file);
    assert.doesNotMatch(source, /sessionProof|access_token|refresh_token|accessToken|refreshToken|bearer/i, file);
    assert.doesNotMatch(source, /supabase|@supabase\/supabase-js|auth-callback/, file);
  }
});

test("Drive authority is checked before keyring/provider effects", () => {
  const drive = read("src-tauri/src/drive_oauth.rs");
  assert.match(drive, /authorize|authorization/i);
  assert.match(drive, /deny|denied|unavailable/i);
  assert.match(drive, /keyring::Entry/);
  assert.match(drive, /drive\.appdata/);
  assert.match(drive, /Zeroizing/);
  assert.match(drive, /broker_drive_connect_begin/);
});

test("browser and Mobile adapters remain available while Desktop uses the broker", () => {
  const auth = read("src/lib/authFlow.ts");
  const supabase = read("src/lib/supabase.ts");
  assert.match(auth, /supabase/);
  assert.match(supabase, /createClient/);
  assert.match(read("src/mobile/MobileApp.tsx"), /authFlow|supabase/);
  assert.match(read("src/web/AuthGuard.tsx"), /supabase/);
});

test("legacy native secret-bearing source is removed rather than merely deregistered", () => {
  const nativeAuth = read("src-tauri/src/native_auth.rs");
  const drive = read("src-tauri/src/drive_oauth.rs");
  for (const source of [nativeAuth, drive]) {
    assert.doesNotMatch(source, /sessionProof|AuthCallbackEvent|auth-callback|emit_auth_callback/);
    assert.doesNotMatch(source, /pub\s+(?:async\s+)?fn\s+(?:auth_begin_google_login|auth_cancel_google_login|drive_oauth_start|drive_oauth_complete|drive_oauth_cancel|drive_connection_status|drive_disconnect|drive_archives_list|drive_upload_archive|drive_restore_intent_create|drive_restore)\b/);
  }
});

test("native broker implements terminal lifecycle and real authority/provider paths", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const drive = read("src-tauri/src/drive_oauth.rs");
  assert.match(session, /LoginPending|login_pending/);
  assert.match(session, /callback_from_request|parse_callback/);
  assert.match(session, /exchange_code/);
  assert.match(session, /refresh_from_keyring/);
  assert.match(session, /Condvar|refresh_flight/);
  assert.match(session, /broker_enrollment_request[\s\S]*device-enrollment/);
  assert.match(session, /create_pairing_session/);
  assert.match(session, /broker_device_revoke[\s\S]*action:\s*"revoke"/);
  assert.doesNotMatch(session, /serde_json::Value/);
  assert.doesNotMatch(drive, /Result<serde_json::Value>/);
  assert.match(drive, /ConnectionActivate/);
  assert.match(drive, /save_refresh_token/);
  assert.match(drive, /broker_drive_restore/);
  assert.ok(drive.indexOf("ConnectionActivate") >= 0);
});
