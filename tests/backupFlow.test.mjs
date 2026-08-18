import test from "node:test";
import assert from "node:assert/strict";
import {
  loadBackupOverview,
  selectBackupRoot,
  selectRestoreTarget,
  runBackup,
  runRestore,
  describeBackupError,
} from "../src/lib/backupFlow.ts";

const record = {
  archiveId: "fung-20260819T000000Z-abc",
  digest: "aa".repeat(32),
  byteCount: 1024,
  timestamp: "2026-08-19T00:00:00Z",
  selectedRootId: "root-id",
  relativeArchiveName: "archives/fung-20260819T000000Z-abc.fungbk",
  terminalState: "verified",
};

function fakeInvoke(handlers) {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push({ command, args });
    if (!(command in handlers)) throw new Error(`unexpected command ${command}`);
    const handler = handlers[command];
    if (handler instanceof Error) throw handler;
    return typeof handler === "function" ? handler(args) : handler;
  };
  return { invoke, calls };
}

test("overview reports verified status and archives from native truth", async () => {
  const { invoke } = fakeInvoke({
    backup_status: { terminalState: "verified", archive: record },
    backup_list_archives: [record],
  });
  const overview = await loadBackupOverview(invoke);
  assert.equal(overview.status.terminalState, "verified");
  assert.equal(overview.archives.length, 1);
});

test("overview is fail-closed unavailable when native commands fail", async () => {
  const { invoke } = fakeInvoke({
    backup_status: new Error("not in tauri"),
    backup_list_archives: [],
  });
  const overview = await loadBackupOverview(invoke);
  assert.equal(overview.status.terminalState, "unavailable");
  assert.deepEqual(overview.archives, []);
});

test("root selection returns only opaque identifiers", async () => {
  const { invoke } = fakeInvoke({
    filesystem_backup_select_root: { terminalState: "selected", selectedRootId: "opaque-1" },
  });
  const status = await selectBackupRoot(invoke);
  assert.equal(status.terminalState, "selected");
  assert.equal(status.selectedRootId, "opaque-1");
  assert.equal("path" in status, false);
});

test("cancelled root selection is truthful unavailable, not an error", async () => {
  const { invoke } = fakeInvoke({
    filesystem_backup_select_root: new Error("dialog closed"),
  });
  const status = await selectBackupRoot(invoke);
  assert.equal(status.terminalState, "unavailable");
  assert.equal(status.selectedRootId, null);
});

test("restore target selection mirrors the opaque picker contract", async () => {
  const { invoke } = fakeInvoke({
    backup_restore_select_target: { terminalState: "selected", selectedTargetId: "opaque-2" },
  });
  const status = await selectRestoreTarget(invoke);
  assert.equal(status.selectedTargetId, "opaque-2");
});

test("backup requires a recovery phrase and passes it through once", async () => {
  const { invoke, calls } = fakeInvoke({ backup_run: record });
  await assert.rejects(() => runBackup(invoke, "   "), /missing_recovery_phrase/);
  const result = await runBackup(invoke, " word ".repeat(1) + "phrase");
  assert.equal(result.terminalState, "verified");
  const runCalls = calls.filter((c) => c.command === "backup_run");
  assert.equal(runCalls.length, 1);
  assert.equal(typeof runCalls[0].args.recoveryPhrase, "string");
});

test("backup failure surfaces the native non-secret reason", async () => {
  const { invoke } = fakeInvoke({ backup_run: new Error("recovery phrase is invalid") });
  await assert.rejects(() => runBackup(invoke, "abandon ".repeat(24).trim()));
});

test("restore never invokes the command without explicit confirmation", async () => {
  const { invoke, calls } = fakeInvoke({ backup_restore: {} });
  await assert.rejects(
    () => runRestore(invoke, record.archiveId, "phrase", false),
    /restore_not_confirmed/,
  );
  assert.equal(calls.length, 0);
});

test("confirmed restore passes archive id and phrase to the native command", async () => {
  const restoreResult = {
    archiveId: record.archiveId,
    restoredBundleSha256: "bb".repeat(32),
    terminalState: "restored",
  };
  const { invoke, calls } = fakeInvoke({ backup_restore: restoreResult });
  const result = await runRestore(invoke, record.archiveId, "phrase words", true);
  assert.equal(result.terminalState, "restored");
  assert.deepEqual(calls[0].args, {
    archiveId: record.archiveId,
    recoveryPhrase: "phrase words",
  });
});

test("error descriptions are truthful and never echo secret material", () => {
  assert.match(describeBackupError(new Error("recovery phrase is invalid")), /24 คำ/);
  assert.match(describeBackupError(new Error("archive authentication failed")), /รหัสกู้คืนผิดหรือไฟล์ถูกแก้ไข/);
  assert.match(describeBackupError(new Error("backup root is unavailable")), /โฟลเดอร์ปลายทาง/);
  assert.match(describeBackupError(new Error("restore target already exists")), /ไม่เขียนทับ/);
  assert.match(describeBackupError(new Error("post-restore verification failed")), /ไม่รายงานว่ากู้คืนสำเร็จ/);
  const fallback = describeBackupError(new Error("boom"));
  assert.match(fallback, /ล้มเหลว/);
});
