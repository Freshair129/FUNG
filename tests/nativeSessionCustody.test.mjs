import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
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
  assert.doesNotMatch(broker, /args\s*\?\s*:\s*Record\s*<\s*string\s*,\s*unknown\s*>/);
  assert.doesNotMatch(broker, /asBrokerInvoke|BrokerInvoke/);
});

test("native session custody has generation ownership, zeroization, keyring-only refresh, and cleanup", () => {
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(session, /SessionLifecycle/);
  assert.match(session, /AccountSession|account_epoch/);
  assert.match(session, /DriveConnection|drive_generation/);
  assert.match(session, /operation_id|operationId/i);
  assert.match(session, /commit_fence|CommitFence/i);
  assert.match(session, /quiescing/);
  assert.match(session, /Zeroizing/);
  assert.match(session, /KeyringPort/);
  assert.match(session, /marker|commit_marker/i);
  assert.match(session, /NoEntry/);
  assert.match(session, /readback|verify_absent/i);
  assert.match(session, /single.?flight|refreshing/i);
  assert.match(session, /logout|shutdown/i);
  assert.match(session, /delete|remove/);
  assert.doesNotMatch(session, /LifecycleCore|SessionMemory|KeyringSeam|ClockSeam|ListenerSeam|RequestTargetSeam|ProviderSeam/);
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
  assert.doesNotMatch(read("src/components/AccountLoginPanel.tsx"), /BrokerInvoke|Record<string,\s*unknown>|args\s*\?/);
  assert.doesNotMatch(read("src/lib/googleDriveFlow.ts"), /InvokeFn|invoke\s*<[^>]+>/);
});

test("Drive authority is checked before keyring/provider effects", () => {
  const drive = read("src-tauri/src/drive_oauth.rs");
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(drive, /authorize|authorization/i);
  assert.match(drive, /deny|denied|unavailable/i);
  assert.match(`${drive}\n${session}`, /KeyringPort|keyring::Entry/);
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

test("native broker source retains lifecycle and authority paths as supplemental evidence", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const drive = read("src-tauri/src/drive_oauth.rs");
  assert.match(session, /LoginPending|login_pending/);
  assert.match(session, /callback_from_request|parse_callback/);
  assert.match(session, /drive_provider_exchange/);
  assert.doesNotMatch(session, /refresh_from_keyring/);
  assert.match(session, /begin_refresh|finish_refresh/);
  assert.match(session, /drive_provider_exchange|drive_provider_refresh/);
  assert.match(session, /account_begin_operation|begin_account_operation/);
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

test("native broker behavioral matrix executes through Rust seams", () => {
  const result = spawnSync(
    "cargo",
    [
      "test",
      "--manifest-path",
      path.join(root, "src-tauri", "Cargo.toml"),
      "native_behavioral_",
      "--",
      "--nocapture",
    ],
    { cwd: root, encoding: "utf8", timeout: 300000, windowsHide: true },
  );
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  assert.equal(result.status, 0, output);
  assert.match(output, /passed/);
  const behavioralOutput = output.slice(output.indexOf("running "), output.indexOf("\nrunning 0 tests"));
  assert.doesNotMatch(behavioralOutput, /secret|verifier|access-token|refresh-token/i);
  assert.doesNotMatch(read("src-tauri/src/auth_session.rs"), /BehavioralBroker|TestState|ProviderMode|LifecycleCore|SessionMemory/);
  assert.match(read("src-tauri/src/auth_session.rs"), /production_shutdown|shutdown_with|SessionLifecycleState/);
});

test("production custody keeps the durable registry and typed recovery ingress", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const drive = read("src-tauri/src/drive_oauth.rs");
  assert.match(session, /SlotIndex|slot-index|DRIVE_DOMAINS_INDEX/);
  assert.match(session, /CredentialMarker|content_sha256|format_version/);
  assert.match(session, /DomainRegistry|registry_digest/);
  assert.doesNotMatch(session, /1\.\.=RECOVERY_SLOT_LIMIT|RECOVERY_SLOT_LIMIT/);
  assert.match(session, /recover_startup[\s\S]*DRIVE_DOMAINS_INDEX/);
  assert.match(session, /pending_operations|account_epoch/);
  assert.match(drive, /DriveOperationGuard/);
  assert.match(drive, /operation\.check/);
  assert.match(drive, /struct RecoveryPhrase/);
  assert.doesNotMatch(drive, /broker_drive_restore[\s\S]*recovery_phrase:\s*String/);
});

test("registered adapters and behavioral tests share production lifecycle entrypoints", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const lib = read("src-tauri/src/lib.rs");
  assert.match(session, /registered_login_begin/);
  assert.match(session, /registered_login_take_for_exchange/);
  assert.match(session, /registered_login_complete/);
  assert.match(session, /registered_listener_callback/);
  assert.match(session, /recover_startup/);
  assert.doesNotMatch(session, /spawn_listener[\s\S]*NativeListener/);
  assert.match(session, /begin_login[\s\S]*take_login_for_exchange[\s\S]*complete_login/);
  assert.match(session, /begin_refresh[\s\S]*finish_refresh/);
  assert.match(session, /begin_account_operation[\s\S]*ensure_account_ticket/);
  assert.match(lib, /auth_session::startup_recover/);
  for (const helper of ["begin", "complete", "startup", "refresh_single_flight", "protected"]) {
    assert.doesNotMatch(session, new RegExp(`#\\[cfg\\(test\\)\\][\\s\\S]{0,120}fn ${helper}\\b`));
  }
  assert.doesNotMatch(session, /LifecycleCore|SessionMemory|dead.*port/i);
});

test("Drive admission drains without holding the lifecycle mutex and fences each provider boundary", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const drive = read("src-tauri/src/drive_oauth.rs");
  assert.match(session, /OperationDrain|wait_empty|drive_drain/);
  assert.match(session, /begin_drive_disconnect[\s\S]*finish_drive_disconnect/);
  assert.match(session, /begin_terminal_transition[\s\S]*finish_terminal_transition/);
  assert.match(session, /drive\.quiescing[\s\S]*wait_empty[\s\S]*drive_generation/);
  assert.match(drive, /LifecycleTicket/);
  assert.match(drive, /drive_check\(ticket\)/);
  assert.match(drive, /invocation\.ensure_valid\(\)\?[\s\S]*drive_check\(ticket\)[\s\S]*\.put\([\s\S]*drive_check\(ticket\)/);
  assert.match(drive, /blocking_list_files\(ticket/);
  assert.match(drive, /upload_small_file\(\n\s+ticket/);
  assert.match(drive, /download_file\(operation\.ticket/);
});
