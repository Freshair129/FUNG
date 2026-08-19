/**
 * Interrupted-recording recovery flow.
 *
 * Pure orchestration over the native recovery commands, so the reporting
 * rules — never implying a clean state, never overstating what was recovered
 * — are testable without a WebView.
 */

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export type InterruptedRecording = {
  recordingId: string;
  projectId: string;
  status: string;
  knownChunks: number;
  /** Audio on disk that no ledger row describes — recoverable. */
  orphanFiles: string[];
  /** Rows whose file is not where they say it is. */
  missingFiles: number;
};

export type RecoveryReport = {
  interrupted: InterruptedRecording[];
  staleJobsFailed: number;
};

export type RecoveryOutcome = {
  recordingId: string;
  adoptedChunks: number;
  adoptedBytes: number;
  unreadableFiles: number;
  durationMs: number;
};

export async function scanForInterruptedRecordings(invoke: InvokeFn): Promise<RecoveryReport> {
  return invoke<RecoveryReport>("recovery_scan");
}

export async function recoverRecording(
  invoke: InvokeFn,
  recordingId: string,
): Promise<RecoveryOutcome> {
  if (!recordingId) throw new Error("missing_recording_id");
  return invoke<RecoveryOutcome>("recovery_recover", { recordingId });
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/**
 * States what a recovery actually did. A run that adopted nothing must not
 * read like a rescue, and unreadable files are named rather than folded into
 * a success count.
 */
export function describeRecovery(outcome: RecoveryOutcome): string {
  const seconds = Math.round(outcome.durationMs / 1000);
  if (outcome.adoptedChunks === 0 && outcome.unreadableFiles === 0) {
    return `ปิดการบันทึกเรียบร้อย — ไม่มีไฟล์ค้างให้กู้คืน (ความยาว ${seconds} วินาที)`;
  }
  let text = `กู้คืนเสียง ${outcome.adoptedChunks} ช่วง (${formatBytes(outcome.adoptedBytes)}) — ความยาวรวม ${seconds} วินาที`;
  if (outcome.unreadableFiles > 0) {
    text += ` — อ่านไม่ได้ ${outcome.unreadableFiles} ไฟล์ ยังอยู่ในโฟลเดอร์เดิม`;
  }
  return text;
}
