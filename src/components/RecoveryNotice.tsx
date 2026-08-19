/**
 * Surfaces recordings an unclean shutdown left unfinished.
 *
 * The recover action existed nowhere before this: a crashed session was
 * silently marked complete the next time the user started a meeting in the
 * same project, and any audio written after the last committed chunk was
 * simply never mentioned. This is the "was something interrupted?" answer,
 * shown at launch rather than waiting to be asked for.
 */
import { useCallback, useEffect, useState } from "react";
import { AlertTriangle } from "lucide-react";
import {
  describeRecovery,
  recoverRecording,
  scanForInterruptedRecordings,
  type InterruptedRecording,
  type InvokeFn,
} from "../lib/recoveryFlow";
import "./RecoveryNotice.css";

type RecoveryNoticeProps = {
  /** Native bridge, or `null` on a surface that cannot reach Tauri. */
  invoke: InvokeFn | null;
};

export function RecoveryNotice({ invoke }: RecoveryNoticeProps) {
  const [interrupted, setInterrupted] = useState<InterruptedRecording[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!invoke) return;
    try {
      const report = await scanForInterruptedRecordings(invoke);
      setInterrupted(report.interrupted);
    } catch {
      // A scan that cannot run is not evidence that nothing was interrupted,
      // so say nothing rather than implying a clean state.
      setInterrupted([]);
    }
  }, [invoke]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!invoke || interrupted.length === 0) return null;

  const handleRecover = async (recordingId: string) => {
    if (!invoke) return;
    setBusy(recordingId);
    setResult(null);
    try {
      setResult(describeRecovery(await recoverRecording(invoke, recordingId)));
    } catch (error) {
      setResult(
        `กู้คืนไม่สำเร็จ: ${error instanceof Error ? error.message : String(error ?? "")}`,
      );
    } finally {
      setBusy(null);
      await refresh();
    }
  };

  return (
    <section className="recovery-notice" role="status" aria-label="การบันทึกที่ถูกขัดจังหวะ">
      <header className="recovery-notice-header">
        <AlertTriangle size={16} aria-hidden="true" />
        <h3>พบการบันทึกที่ไม่ได้ปิดอย่างถูกต้อง {interrupted.length} รายการ</h3>
      </header>
      <p className="recovery-notice-note">
        เสียงที่บันทึกไว้ยังอยู่ในเครื่อง — กู้คืนเพื่อนำไฟล์ที่ยังไม่ได้บันทึกลง ledger เข้าระบบ
        พร้อมลายเซ็นตรวจสอบ
      </p>
      <ul className="recovery-notice-list">
        {interrupted.map((item) => (
          <li key={item.recordingId} className="recovery-notice-item">
            <span className="recovery-notice-detail">
              <code>{item.recordingId.slice(0, 8)}</code> · สถานะ {item.status} ·{" "}
              {item.knownChunks} ช่วงใน ledger
              {item.orphanFiles.length > 0 && ` · พบไฟล์ค้าง ${item.orphanFiles.length}`}
              {item.missingFiles > 0 && ` · ไฟล์หาย ${item.missingFiles}`}
            </span>
            <button
              className="recovery-notice-btn"
              type="button"
              onClick={() => void handleRecover(item.recordingId)}
              disabled={busy !== null}
            >
              {busy === item.recordingId ? "กำลังกู้คืน..." : "กู้คืน"}
            </button>
          </li>
        ))}
      </ul>
      {result && <p className="recovery-notice-result">{result}</p>}
    </section>
  );
}
