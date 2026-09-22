import { useEffect, useState } from "react";
import { CheckCircle2, FolderOpen, HardDrive, Loader2, RotateCcw, TriangleAlert } from "lucide-react";
import {
  nativeInvoke,
  pickRecordingOutputFolder,
  recordingOutputGet,
  recordingOutputReset,
  recordingOutputSet,
  type RecordingOutputStatus,
} from "../../tauri.ts";
import "./RecordingOutputPanel.css";

type RecordingOutputPanelProps = {
  captureActive: boolean;
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function RecordingOutputPanel({ captureActive }: RecordingOutputPanelProps) {
  const [status, setStatus] = useState<RecordingOutputStatus | null>(null);
  const [issue, setIssue] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    if (!nativeInvoke) {
      setIssue("การเลือกปลายทางไฟล์บันทึกต้องใช้ FUNG Desktop");
      return;
    }
    try {
      setStatus(await recordingOutputGet());
      setIssue(null);
    } catch (error) {
      setIssue(errorMessage(error));
    }
  };

  useEffect(() => {
    void refresh();
  }, [captureActive]);

  const locked = captureActive || Boolean(status?.captureActive);

  const chooseFolder = async () => {
    if (locked || busy) return;
    setBusy(true);
    setIssue(null);
    try {
      const path = await pickRecordingOutputFolder();
      if (path) setStatus(await recordingOutputSet(path));
    } catch (error) {
      setIssue(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const resetFolder = async () => {
    if (locked || busy) return;
    setBusy(true);
    setIssue(null);
    try {
      setStatus(await recordingOutputReset());
    } catch (error) {
      setIssue(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const unavailable = status ? !status.writable : Boolean(issue);
  const statusText = locked
    ? "กำลังบันทึก — เปลี่ยนปลายทางไม่ได้"
    : unavailable
      ? "ปลายทางไม่พร้อมใช้งาน"
      : "ปลายทางพร้อมเขียน";

  return (
    <section className="recording-output-page" aria-labelledby="recording-output-heading">
      <div className="recording-output-page__heading-row">
        <div>
          <h2 id="recording-output-heading">ปลายทางไฟล์บันทึก</h2>
          <p>กำหนดโฟลเดอร์สำหรับไฟล์เสียงใหม่ โปรเจกต์ใหม่ และไฟล์ส่งออกของ FUNG</p>
        </div>
        <HardDrive aria-hidden="true" size={28} strokeWidth={1.6} />
      </div>

      <div className={`recording-output-card${unavailable ? " is-unavailable" : ""}`}>
        <div className="recording-output-card__label">ปลายทางปัจจุบัน</div>
        <code className="recording-output-card__path">
          {status?.currentPath ?? "ยังอ่านปลายทางจาก Desktop ไม่ได้"}
        </code>
        <div className="recording-output-card__status" role="status" aria-live="polite">
          {unavailable ? <TriangleAlert size={16} aria-hidden="true" /> : <CheckCircle2 size={16} aria-hidden="true" />}
          <span>{statusText}</span>
        </div>
        {status?.issue ? <p className="recording-output-card__issue">{status.issue}</p> : null}
        {issue && !status?.issue ? <p className="recording-output-card__issue">{issue}</p> : null}
        <div className="recording-output-card__actions">
          <button type="button" className="desktop-shell__action-button" onClick={() => void chooseFolder()} disabled={locked || busy || !nativeInvoke}>
            {busy ? <Loader2 size={16} className="recording-output-spin" aria-hidden="true" /> : <FolderOpen size={16} aria-hidden="true" />}
            เลือกโฟลเดอร์
          </button>
          <button type="button" className="desktop-shell__action-button is-secondary" onClick={() => void resetFolder()} disabled={locked || busy || !nativeInvoke}>
            <RotateCcw size={16} aria-hidden="true" />
            คืนค่าเริ่มต้น
          </button>
        </div>
      </div>

      <div className="recording-output-note">
        <strong>การเปลี่ยนแปลงมีผลกับไฟล์ใหม่เท่านั้น</strong>
        <span>ไฟล์บันทึกเดิมจะไม่ถูกย้าย และยังเปิดอ่านจากตำแหน่งเดิมได้</span>
        <span>ค่าเริ่มต้น: {status?.defaultPath ?? "Documents\\fung"}</span>
      </div>
    </section>
  );
}
