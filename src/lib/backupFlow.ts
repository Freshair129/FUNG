/**
 * Phase 4 filesystem test-backup flow.
 *
 * Pure orchestration over the native backup commands so the interaction
 * rules — recovery-secret handling, restore confirmation, truthful
 * unavailable states — are testable without a WebView. The recovery phrase
 * is only ever passed through to `invoke`; it is never stored, logged, or
 * returned from these helpers after a run completes.
 */

export type BackupTerminalState = "unavailable" | "no_verified_archive" | "verified";

export type BackupArchiveRecord = {
  archiveId: string;
  digest: string;
  byteCount: number;
  timestamp: string;
  selectedRootId: string;
  relativeArchiveName: string;
  terminalState: string;
};

export type BackupStatus = {
  terminalState: BackupTerminalState;
  archive: BackupArchiveRecord | null;
};

export type PickerStatus = {
  terminalState: "unavailable" | "selected";
  selectedRootId?: string | null;
  selectedTargetId?: string | null;
};

/** What a restore actually wrote beside the ledger. Reported so a caller can
 * never present an audio-less restore as a whole project. */
export type AudioRestoreSummary = {
  restoredFileCount: number;
  restoredByteCount: number;
  omittedFileCount: number;
};

export type RestoreResult = {
  archiveId: string;
  restoredBundleSha256: string;
  audio: AudioRestoreSummary;
  terminalState: string;
};

/** How much source audio one archive carried. The archive record alone cannot
 * answer that, and a database-only archive must not read as a complete
 * project backup. */
export type AudioBackupSummary = {
  storedFileCount: number;
  storedByteCount: number;
  omittedFileCount: number;
};

export type BackupRunReport = {
  record: BackupArchiveRecord;
  audio: AudioBackupSummary;
};

export type BackupOverview = {
  status: BackupStatus;
  archives: BackupArchiveRecord[];
};

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

const UNAVAILABLE_STATUS: BackupStatus = { terminalState: "unavailable", archive: null };

/** Load current status + archive list. Any native failure is reported as the
 * fail-closed unavailable state, never as a fabricated archive. */
export async function loadBackupOverview(invoke: InvokeFn): Promise<BackupOverview> {
  try {
    const [status, archives] = await Promise.all([
      invoke<BackupStatus>("backup_status"),
      invoke<BackupArchiveRecord[]>("backup_list_archives"),
    ]);
    return { status, archives };
  } catch {
    return { status: UNAVAILABLE_STATUS, archives: [] };
  }
}

export async function selectBackupRoot(invoke: InvokeFn): Promise<PickerStatus> {
  try {
    return await invoke<PickerStatus>("filesystem_backup_select_root");
  } catch {
    return { terminalState: "unavailable", selectedRootId: null };
  }
}

export async function selectRestoreTarget(invoke: InvokeFn): Promise<PickerStatus> {
  try {
    return await invoke<PickerStatus>("backup_restore_select_target");
  } catch {
    return { terminalState: "unavailable", selectedTargetId: null };
  }
}

/** Generate the 24-word recovery phrase for one-time display. The caller must
 * clear it from component state once the user acknowledges writing it down. */
export async function generateRecoveryPhrase(invoke: InvokeFn): Promise<string> {
  return invoke<string>("backup_generate_recovery_phrase");
}

export async function runBackup(
  invoke: InvokeFn,
  recoveryPhrase: string,
): Promise<BackupRunReport> {
  const phrase = recoveryPhrase.trim();
  if (!phrase) throw new Error("missing_recovery_phrase");
  return invoke<BackupRunReport>("backup_run", { recoveryPhrase: phrase });
}

/** Human-readable byte size for the archive/audio counters. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/** One sentence stating exactly what an archive carried, including audio the
 * ledger referenced but the archive could not store. */
export function describeAudioBackup(audio: AudioBackupSummary): string {
  const stored = `เสียงต้นฉบับ ${audio.storedFileCount} ไฟล์ (${formatBytes(audio.storedByteCount)})`;
  if (audio.omittedFileCount === 0) return `${stored} — ครบตามที่บันทึกไว้`;
  return `${stored} — ขาด ${audio.omittedFileCount} ไฟล์ที่อ่านไม่ได้หรือถูกแก้ไขไปแล้ว`;
}

/** Same for a completed restore. */
export function describeAudioRestore(audio: AudioRestoreSummary): string {
  const restored = `คืนไฟล์เสียง ${audio.restoredFileCount} ไฟล์ (${formatBytes(audio.restoredByteCount)})`;
  if (audio.omittedFileCount === 0) return `${restored} — ครบตามที่ไฟล์สำรองบันทึกไว้`;
  return `${restored} — ไฟล์สำรองระบุว่าขาดไปแล้ว ${audio.omittedFileCount} ไฟล์ตั้งแต่ตอนสำรอง`;
}

/** Restore requires an explicit confirmation from the user because it creates
 * a new clean target; without it the command is never invoked. */
export async function runRestore(
  invoke: InvokeFn,
  archiveId: string,
  recoveryPhrase: string,
  confirmedCleanTarget: boolean,
): Promise<RestoreResult> {
  if (!confirmedCleanTarget) throw new Error("restore_not_confirmed");
  const phrase = recoveryPhrase.trim();
  if (!phrase) throw new Error("missing_recovery_phrase");
  if (!archiveId) throw new Error("missing_archive_id");
  return invoke<RestoreResult>("backup_restore", { archiveId, recoveryPhrase: phrase });
}

/** Map native error strings to truthful, non-secret user-facing text. */
export function describeBackupError(raw: unknown): string {
  const message = raw instanceof Error ? raw.message : String(raw ?? "");
  if (message.includes("recovery phrase is invalid") || message === "missing_recovery_phrase") {
    return "รหัสกู้คืนไม่ถูกต้อง — ต้องเป็นวลี 24 คำที่สร้างไว้";
  }
  if (message.includes("authentication failed")) {
    return "ยืนยันตัวตนของไฟล์สำรองไม่ผ่าน — รหัสกู้คืนผิดหรือไฟล์ถูกแก้ไข";
  }
  if (message.includes("root is unavailable")) {
    return "ยังไม่ได้เลือกโฟลเดอร์ปลายทาง หรือโฟลเดอร์ใช้งานไม่ได้";
  }
  if (message.includes("restore target parent is unavailable")) {
    return "ยังไม่ได้เลือกโฟลเดอร์เป้าหมายสำหรับกู้คืน";
  }
  if (message.includes("restore target already exists")) {
    return "โฟลเดอร์เป้าหมายมีอยู่แล้ว — ระบบไม่เขียนทับข้อมูลเดิม";
  }
  if (message.includes("already running")) {
    return "มีงานสำรอง/กู้คืนกำลังทำงานอยู่";
  }
  if (message.includes("archive is unavailable")) {
    return "ไม่พบไฟล์สำรองหรือไฟล์ไม่ผ่านการตรวจสอบ";
  }
  if (message === "restore_not_confirmed") {
    return "ต้องยืนยันการกู้คืนสู่โฟลเดอร์ว่างก่อน";
  }
  if (message.includes("verification failed")) {
    return "การตรวจสอบหลังกู้คืนไม่ผ่าน — ไม่รายงานว่ากู้คืนสำเร็จ";
  }
  if (message.includes("in-memory limit")) {
    return "โปรเจกต์นี้มีเสียงรวมใหญ่เกินขนาดที่สำรองได้ในครั้งเดียว — ยังไม่รองรับการสำรองแบบสตรีม";
  }
  if (message.includes("audio files, above what one archive can enumerate")) {
    return "โปรเจกต์นี้อ้างถึงไฟล์เสียงมากเกินกว่าที่ไฟล์สำรองเดียวจะรวบรวมได้";
  }
  if (message.includes("audio inventory could not be read")) {
    return "อ่านรายการไฟล์เสียงจาก Genesis ไม่สำเร็จ — ไม่สำรองเพื่อไม่ให้ได้ไฟล์ที่ขาดเสียง";
  }
  if (message.includes("backup payload digest mismatch")) {
    return "ข้อมูลในไฟล์สำรองไม่ตรงกับลายเซ็นที่บันทึกไว้ — ไม่กู้คืนข้อมูลที่อาจผิด";
  }
  if (message.includes("backup payload")) {
    return "โครงสร้างข้อมูลในไฟล์สำรองไม่ถูกต้อง — ไม่กู้คืน";
  }
  if (message.includes("audio could not be restored")) {
    return "เขียนไฟล์เสียงลงโฟลเดอร์กู้คืนไม่สำเร็จ — ไม่รายงานว่ากู้คืนสำเร็จ";
  }
  return "การทำงานล้มเหลว: " + (message || "ไม่ทราบสาเหตุ");
}
