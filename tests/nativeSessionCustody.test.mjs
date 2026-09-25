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
  for (const operation of [
    "broker_session_login_begin",
    "broker_session_login_cancel",
    "broker_session_status",
    "broker_session_logout",
    "broker_enrollment_request",
    "broker_enrollment_status",
    "broker_device_list",
    "broker_pairing_create",
    "broker_pairing_poll",
    "broker_pairing_reconcile",
    "broker_device_revoke",
    "broker_device_audit_list",
    "broker_fungwire_status",
    "broker_fungwire_set_enabled",
    "broker_device_endpoint_publish",
    "account_portal_open",
  ]) {
    assert.match(broker, new RegExp(operation));
  }
  assert.doesNotMatch(broker, /url\s*:\s*string|headers\s*:\s*Record|bearer|sessionProof|accessToken|refreshToken/);
  assert.doesNotMatch(broker, /args\s*\?\s*:\s*Record\s*<\s*string\s*,\s*unknown\s*>/);
  assert.doesNotMatch(broker, /asBrokerInvoke|BrokerInvoke/);
});

test("native session custody has generation ownership, zeroization, keyring-only refresh, and cleanup", () => {
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(session, /SessionLifecycle/);
  assert.match(session, /AccountSession|account_epoch/);
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
  assert.deepEqual(
    [...session.matchAll(/\.emit\s*\(\s*"([^"]+)"/g)].map((match) => match[1]),
    ["auth-session-changed"],
  );
  assert.doesNotMatch(session, /localStorage|sessionStorage|Genesis|metadata/);
});

test("automatic refresh lifecycle changes notify panels to clear account-bound state", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const refreshStart = session.indexOf("pub(crate) async fn ensure_access_token(");
  const refreshEnd = session.indexOf("\npub(crate) fn native_user_id", refreshStart);
  const refresh = session.slice(refreshStart, refreshEnd);
  assert.match(refresh, /witness_before_refresh\s*=\s*read_lifecycle_witness\(\)/);
  assert.match(refresh, /finish_refresh\(ticket, result\)/);
  assert.match(refresh, /witness_after_refresh\s*=\s*read_lifecycle_witness\(\)/);
  assert.match(refresh, /witness_before_refresh\s*!=\s*witness_after_refresh/);
  assert.match(refresh, /emit_account_lifecycle_changed\(app\)/);
});

test("native command inventory removes secret-bearing legacy aliases", () => {
  const lib = read("src-tauri/src/lib.rs");
  for (const oldName of [
    "auth_begin_google_login",
    "auth_cancel_google_login",
    "native_device_enrollment_proof",
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
    "broker_enrollment_status",
    "broker_device_list",
    "broker_pairing_create",
    "broker_pairing_poll",
    "broker_pairing_reconcile",
    "broker_device_revoke",
    "broker_device_audit_list",
    "broker_device_endpoint_publish",
  ]) {
    assert.match(lib, new RegExp(`\\b${name}\\b`));
  }
});

test("Desktop consumers never carry session proof or token-shaped public values", () => {
  const files = [
    "src/components/AccountLoginPanel.tsx",
    "src/components/DevicePairingPanel.tsx",
    "src/lib/desktopSessionBroker.ts",
  ];
  for (const file of files) {
    const source = read(file);
    assert.doesNotMatch(source, /sessionProof|access_token|refresh_token|accessToken|refreshToken|bearer/i, file);
    assert.doesNotMatch(source, /supabase|@supabase\/supabase-js|auth-callback/, file);
  }
  assert.doesNotMatch(read("src/components/AccountLoginPanel.tsx"), /BrokerInvoke|Record<string,\s*unknown>|args\s*\?/);
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
  assert.doesNotMatch(nativeAuth, /sessionProof|AuthCallbackEvent|auth-callback|emit_auth_callback/);
  assert.doesNotMatch(
    nativeAuth,
    /pub\s+(?:async\s+)?fn\s+(?:auth_begin_google_login|auth_cancel_google_login|native_device_enrollment_proof)\b/,
  );
});

test("native broker source retains lifecycle and authority paths as supplemental evidence", () => {
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(session, /LoginPending|login_pending/);
  assert.match(session, /callback_from_request|parse_callback/);
  assert.doesNotMatch(session, /refresh_from_keyring/);
  assert.match(session, /begin_refresh|finish_refresh/);
  assert.match(session, /begin_account_operation/);
  assert.match(session, /Condvar|refresh_flight/);
  assert.match(session, /broker_enrollment_request[\s\S]*device-enrollment/);
  assert.match(session, /create_pairing_session/);
  assert.match(session, /broker_device_revoke[\s\S]*action:\s*"revoke"/);
  assert.doesNotMatch(session, /serde_json::Value/);
});

test("native broker behavioral matrix executes through registered Rust entrypoints", () => {
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
  const firstMatrixStart = output.indexOf("running ");
  assert.ok(firstMatrixStart >= 0, output);
  const firstZeroTests = output.indexOf("\nrunning 0 tests", firstMatrixStart);
  const behavioralOutput = output.slice(
    firstMatrixStart,
    firstZeroTests >= 0 ? firstZeroTests : undefined,
  );
  const count = Number(behavioralOutput.match(/running\s+(\d+)\s+tests/)?.[1] ?? 0);
  assert.ok(count > 0, behavioralOutput);
  assert.match(behavioralOutput, /test result: ok\.\s+\d+ passed/);
  assert.doesNotMatch(behavioralOutput, /secret|verifier|access-token|refresh-token/i);
  assert.match(behavioralOutput, /native_behavioral_rotation_uses_registered_login_completion/);
  assert.match(behavioralOutput, /native_behavioral_registered_startup_recovery_both_domains_fault_matrix/);
  const session = read("src-tauri/src/auth_session.rs");
  assert.doesNotMatch(session, /BehavioralBroker|TestState|ProviderMode|LifecycleCore|SessionMemory/);
  assert.match(session, /RegisteredBrokerEntrypoints|SessionLifecycleState/);
});

test("production custody keeps the durable registry and typed recovery ingress", () => {
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(session, /SlotIndex|slot-index/);
  assert.match(session, /CredentialMarker|content_sha256|format_version/);
  assert.match(session, /recover_startup/);
  assert.match(session, /ACCOUNT_DOMAIN|ACCOUNT_MARKER|index_slot/);
  assert.doesNotMatch(session, /1\.\.=RECOVERY_SLOT_LIMIT|RECOVERY_SLOT_LIMIT/);
  assert.match(session, /pending_operations|account_epoch/);
});

test("registered adapters and behavioral tests share production lifecycle entrypoints", () => {
  const session = read("src-tauri/src/auth_session.rs");
  const lib = read("src-tauri/src/lib.rs");
  assert.match(session, /trait RegisteredBrokerPort/);
  assert.match(session, /struct RegisteredBrokerEntrypoints/);
  assert.match(session, /type NativeRegisteredBroker/);
  assert.match(session, /RegisteredBrokerEntrypoints::new\([\s\S]*NativeKeyring[\s\S]*NativeClock[\s\S]*NativeListener[\s\S]*NativeProvider/);
  assert.match(session, /registered_login_begin/);
  assert.match(session, /registered_login_take_for_exchange/);
  assert.match(session, /registered_login_complete/);
  assert.match(session, /registered_listener_callback/);
  assert.match(session, /recover_startup/);
  assert.doesNotMatch(session, /spawn_listener[\s\S]*NativeListener/);
  assert.match(session, /begin_login[\s\S]*take_login_for_exchange[\s\S]*complete_login/);
  assert.match(session, /begin_refresh[\s\S]*finish_refresh/);
  assert.match(session, /ensure_account_ticket/);
  assert.match(session, /begin_account_operation/);
  assert.match(lib, /auth_session::startup_recover/);
  const genericLifecycle = session.slice(
    session.indexOf("impl<K, C, L, P> SessionLifecycle"),
    session.indexOf("pub(crate) struct RegisteredBrokerEntrypoints"),
  );
  assert.doesNotMatch(genericLifecycle, /pub\(crate\) fn (?:logout|shutdown)\b/);
  for (const forbidden of [
    "registered_accept_material",
    "seed_active",
    "fail_keyring_at",
    "fail_cleanup",
    "fail_provider_with",
    "invalidate_generation",
    "set_quiescing",
    "write_keyring_slot",
  ]) {
    assert.doesNotMatch(session, new RegExp(`\\b${forbidden}\\b`));
  }
  for (const helper of ["begin", "complete", "startup", "refresh_single_flight", "protected"]) {
    assert.doesNotMatch(session, new RegExp(`#\\[cfg\\(test\\)\\][\\s\\S]{0,120}fn ${helper}\\b`));
  }
  assert.doesNotMatch(session, /LifecycleCore|SessionMemory|dead.*port/i);
  for (const recoveryEvidence of [
    "struct RecoveryRow",
    "recovery_trace",
    "marker_id",
    "index_id",
    "slot_id",
    "orphan_id",
    "first_lifecycle_state",
    "restart_lifecycle_state",
    "terminal_cleanup_failed_expected",
    "first_publication_outcome",
    "restart_publication_outcome",
    "public_publication_outcome",
    "make_fixture_with_keyring",
  ]) {
    assert.match(session, new RegExp(recoveryEvidence));
  }
  assert.match(session, /first_result[\s\S]*restart_result[\s\S]*first_readback[\s\S]*restart_readback/);
  assert.match(session, /RecoveryCase::CorruptTarget[\s\S]*RecoveryCase::ValidPersisted/);
});

test("terminal transitions drain admitted native work before cleanup", () => {
  const session = read("src-tauri/src/auth_session.rs");
  assert.match(session, /OperationDrain|wait_empty/);
  assert.match(session, /begin_terminal_transition[\s\S]*finish_terminal_transition/);
  assert.match(session, /pending_operations|account_epoch/);
});
