import assert from "node:assert/strict";
import test from "node:test";

import { describePairingFailure } from "../src/lib/pairingErrors.ts";

test("missing Supabase config explains why desktop pairing cannot load and disables setup", () => {
  const failure = describePairingFailure("auth_required", false);
  assert.equal(failure.setupRequired, true);
  assert.match(failure.message, /ยังไม่ได้ตั้งค่า Supabase/);
  assert.match(failure.message, /VITE_SUPABASE_URL/);
});

test("native auth configuration errors keep the setup guidance actionable", () => {
  const failure = describePairingFailure("auth_config_invalid");
  assert.equal(failure.setupRequired, true);
  assert.match(failure.message, /VITE_SUPABASE_ANON_KEY/);
});

test("signed-out desktop asks the user to sign in before pairing", () => {
  const failure = describePairingFailure("auth_required");
  assert.equal(failure.setupRequired, true);
  assert.match(failure.message, /ลงชื่อเข้าใช้/);
});

test("temporary authorization outages remain retryable", () => {
  const failure = describePairingFailure("authorization_unavailable");
  assert.equal(failure.setupRequired, false);
  assert.match(failure.message, /เชื่อมต่อบริการบัญชีไม่ได้/);
});
