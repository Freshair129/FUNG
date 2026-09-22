import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");
const escapeRegex = (value) =>
  value.replace(/[.*+?^$()|[\]\\]/g, "\\$&");

const contract = read("contracts/meeting-intelligence-v1.yaml");
const schema = read("src-tauri/src/meeting_intelligence_schema.rs");
const adapter = read("src-tauri/src/genesis_adapter.rs");

test("meeting-intelligence contract records approved native authority and custody", () => {
  for (const marker of [
    "owner_principal_ref",
    "account-free local vault remains usable",
    "request actor, ACL, provider label",
    "people_metadata",
    "XChaCha20-Poly1305",
    "model_run_id, or none",
    "ciphertext_max_bytes: 4096",
    "opaque profile_id is an approved necessary index",
    "current expected review/evidence revision",
    "capture_frontier",
    "MEETING_COMMIT_UNCERTAIN",
    "explicit_native_owner_unlock",
    "account_generation",
    "operation_fence",
    "durable original transaction_id",
    "account_switch_commit_serialization",
    "R3 CANDIDATE IMPLEMENTATION",
    "approved_r3_scope",
    "deterministic cross-thread regressions",
    "independent Luna G1 is NOT_RUN",
    "identity-protected unlock",
    "account-free local-owner vault",
  ]) {
    assert.match(contract, new RegExp(escapeRegex(marker)));
  }
});

test("supplemental source-shape checks show relationship plaintext is not in typed attribution", () => {
  assert.match(schema, /pub\(crate\) struct ParticipantAttribution/);
  assert.doesNotMatch(
    schema,
    /struct ParticipantAttribution[\s\S]{0,900}provider_label\s*:/,
  );
  assert.doesNotMatch(
    schema,
    /struct ParticipantAttribution[\s\S]{0,900}person_id\s*:/,
  );
  assert.doesNotMatch(
    schema,
    /struct ParticipantAttribution[\s\S]{0,900}acl_subject\s*:/,
  );
  assert.match(adapter, /identity_link_row_with_model|model_context/);
  assert.match(adapter, /participant_profiles/);
  assert.match(adapter, /unlock_native_local_owner/);
  assert.match(adapter, /revalidate_native_local_owner_session_under_fence/);
  assert.match(adapter, /reopens_and_replays_durable_transaction_identity/);
  assert.match(adapter, /with_account_commit_fence/);
  assert.match(adapter, /native account operation is unavailable/);
  assert.match(
    adapter,
    /r3_contender_first_switch_before_broker_fence_rejects_without_partial_effects/,
  );
  assert.match(adapter, /r3_commit_fence_first_blocks_switch_until_successful_commit/);
  assert.match(adapter, /r3_commit_fence_error_releases_before_guard_teardown/);
  assert.doesNotMatch(
    adapter,
    /r2_registered_broker_account_switch_after_final_check_is_unclosed/,
  );
});

test("contract keeps deferred and unrun evidence explicit", () => {
  assert.match(contract, /clone-only 64->128/);
  assert.match(contract, /preserve v10 and the approved retained v11 aggregate set/);
  for (const marker of [
    'account_switch_commit_serialization: "IMPLEMENTED_CANDIDATE',
    "account_switch_diagnostic: \"KNOWN_BUG_BASELINE_ONLY",
    "independent Luna G1 required",
    'native_keyring_integration: "NOT_RUN"',
    'native_ui_or_uat: "NOT_RUN"',
    'backup_key_recovery: "NOT_RUN"',
    'provider_or_cloud: "NOT_RUN"',
    'ci_portable_release: "NOT_RUN"',
  ]) {
    assert.match(contract, new RegExp(escapeRegex(marker)));
  }
});
