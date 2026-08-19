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

/** What transcribing a recovered recording achieved. `skippedReason` means the
 * pass declined to run — which is not the same as finding nothing to do. */
export type GapFillOutcome = {
  chunksMissingTranscript: number;
  chunksTranscribed: number;
  stillMissing: number;
  skippedReason: string | null;
};

export type RecoveredRecording = {
  adopted: RecoveryOutcome;
  transcript: GapFillOutcome;
};

export async function scanForInterruptedRecordings(invoke: InvokeFn): Promise<RecoveryReport> {
  return invoke<RecoveryReport>("recovery_scan");
}

export async function recoverRecording(
  invoke: InvokeFn,
  recordingId: string,
): Promise<RecoveredRecording> {
  if (!recordingId) throw new Error("missing_recording_id");
  return invoke<RecoveredRecording>("recovery_recover", { recordingId });
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
export function describeRecovery(result: RecoveredRecording): string {
  const outcome = result.adopted;
  const seconds = Math.round(outcome.durationMs / 1000);
  let text: string;
  if (outcome.adoptedChunks === 0 && outcome.unreadableFiles === 0) {
    text = `ปิดการบันทึกเรียบร้อย — ไม่มีไฟล์ค้างให้กู้คืน (ความยาว ${seconds} วินาที)`;
  } else {
    text = `กู้คืนเสียง ${outcome.adoptedChunks} ช่วง (${formatBytes(outcome.adoptedBytes)}) — ความยาวรวม ${seconds} วินาที`;
    if (outcome.unreadableFiles > 0) {
      text += ` — อ่านไม่ได้ ${outcome.unreadableFiles} ไฟล์ ยังอยู่ในโฟลเดอร์เดิม`;
    }
  }
  return `${text} · ${describeGapFill(result.transcript)}`;
}

/** States the transcript result separately, because recovered audio with no
 * words is safe and unreadable at the same time — and a pass that declined to
 * run must not read like a complete transcript. */
export function describeGapFill(gap: GapFillOutcome): string {
  if (gap.skippedReason) {
    return `ยังไม่ได้ตรวจข้อความที่ขาด (${gap.skippedReason})`;
  }
  if (gap.chunksMissingTranscript === 0) {
    return "ข้อความครบอยู่แล้ว";
  }
  if (gap.stillMissing === 0) {
    return `ถอดความเพิ่ม ${gap.chunksTranscribed} ช่วงจนครบ`;
  }
  return `ถอดความเพิ่ม ${gap.chunksTranscribed} ช่วง — ยังขาดอีก ${gap.stillMissing} ช่วง`;
}
