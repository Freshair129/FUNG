/**
 * Filesystem backup and restore controls.
 *
 * Lives in `src/components/` rather than inside the web Account Settings page
 * because the backup commands are Tauri commands: the web surface can render
 * this panel but can never invoke them, while the desktop shell — the only
 * surface where they actually run — previously had no way to reach them at
 * all. Both shells now mount the same component and pass their own `invoke`.
 */
import { useCallback, useEffect, useState } from "react";
import { HardDrive } from "lucide-react";
import {
  checkAudioIntegrity,
  describeAudioBackup,
  describeAudioIntegrity,
  describeAudioRestore,
  describeBackupError,
  formatBytes,
  generateRecoveryPhrase,
  loadBackupOverview,
  runBackup,
  runRestore,
  selectBackupRoot,
  selectRestoreTarget,
  type BackupArchiveRecord,
  type BackupOverview,
  type InvokeFn,
} from "../lib/backupFlow";
import "./BackupPanel.css";
import { GoogleDrivePanel } from "./GoogleDrivePanel";

type BackupPanelProps = {
  /** Native bridge. Absent when the host surface cannot reach Tauri, which
   * the panel states plainly instead of rendering dead controls. */
  invoke: InvokeFn | null;
  /** Project whose source audio can be verified. Omitted where no project is
   * selected, in which case the check is not offered rather than offered and
   * failing. */
  projectId?: string | null;
};

export function BackupPanel({ invoke, projectId = null }: BackupPanelProps) {
  const [overview, setOverview] = useState<BackupOverview>({
    status: { terminalState: "unavailable", archive: null },
    archives: [],
  });
  const [rootId, setRootId] = useState<string | null>(null);
  const [restoreTargetId, setRestoreTargetId] = useState<string | null>(null);
  // The generated phrase exists in state only until the user acknowledges it,
  // then it is cleared and never rendered again.
  const [generatedPhrase, setGeneratedPhrase] = useState<string | null>(null);
  const [backupPhrase, setBackupPhrase] = useState("");
  const [restorePhrase, setRestorePhrase] = useState("");
  const [restoreArchiveId, setRestoreArchiveId] = useState("");
  const [restoreConfirmed, setRestoreConfirmed] = useState(false);
  const [busy, setBusy] = useState<"backup" | "restore" | null>(null);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [integrity, setIntegrity] = useState<{ ok: boolean; text: string } | null>(null);
  const [verifying, setVerifying] = useState(false);

  const refresh = useCallback(async () => {
    if (!invoke) return;
    setOverview(await loadBackupOverview(invoke));
  }, [invoke]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!invoke) {
    return (
      <section className="backup-panel" aria-label="สำรองข้อมูล">
        <h3 className="backup-panel-title">
          <HardDrive size={16} /> สำรองข้อมูล
        </h3>
        <p className="backup-panel-note">
          การสำรองข้อมูลทำงานได้เฉพาะในแอปเดสก์ท็อป FUNG เท่านั้น —
          หน้าเว็บนี้เข้าถึงไฟล์ในเครื่องไม่ได้
        </p>
      </section>
    );
  }

  const handleSelectRoot = async () => {
    setMessage(null);
    const status = await selectBackupRoot(invoke);
    setRootId(status.terminalState === "selected" ? (status.selectedRootId ?? null) : null);
    await refresh();
  };

  const handleSelectRestoreTarget = async () => {
    setMessage(null);
    const status = await selectRestoreTarget(invoke);
    setRestoreTargetId(
      status.terminalState === "selected" ? (status.selectedTargetId ?? null) : null,
    );
  };

  const handleGeneratePhrase = async () => {
    setMessage(null);
    try {
      const phrase = await generateRecoveryPhrase(invoke);
      setGeneratedPhrase(phrase);
      setBackupPhrase(phrase);
    } catch (err) {
      setMessage({ type: "error", text: describeBackupError(err) });
    }
  };

  // One-time display ends here; the phrase remains only in the transient
  // backup input state until the run completes.
  const handleAcknowledgePhrase = () => setGeneratedPhrase(null);

  const handleVerifyAudio = async () => {
    if (!invoke || !projectId) return;
    setVerifying(true);
    setIntegrity(null);
    try {
      setIntegrity(describeAudioIntegrity(await checkAudioIntegrity(invoke, projectId)));
    } catch (err) {
      setIntegrity({ ok: false, text: describeBackupError(err) });
    } finally {
      setVerifying(false);
    }
  };

  const handleRunBackup = async () => {
    setBusy("backup");
    setMessage(null);
    try {
      const report = await runBackup(invoke, backupPhrase);
      setMessage({
        type: report.audio.omittedFileCount > 0 ? "error" : "success",
        text: `สำรองสำเร็จ (${report.record.archiveId}) — ${describeAudioBackup(report.audio)}`,
      });
    } catch (err) {
      setMessage({ type: "error", text: describeBackupError(err) });
    } finally {
      setBackupPhrase("");
      setGeneratedPhrase(null);
      setBusy(null);
      await refresh();
    }
  };

  const handleRunRestore = async () => {
    setBusy("restore");
    setMessage(null);
    try {
      const result = await runRestore(invoke, restoreArchiveId, restorePhrase, restoreConfirmed);
      setMessage({
        type: result.audio.omittedFileCount > 0 ? "error" : "success",
        text: `กู้คืนสู่โฟลเดอร์ใหม่สำเร็จ (${result.archiveId}) — ${describeAudioRestore(result.audio)}`,
      });
    } catch (err) {
      setMessage({ type: "error", text: describeBackupError(err) });
    } finally {
      setRestorePhrase("");
      setRestoreConfirmed(false);
      setBusy(null);
    }
  };

  return (
    <section className="backup-panel" aria-label="สำรองข้อมูล">
      <h3 className="backup-panel-title">
        <HardDrive size={16} /> สำรองข้อมูล (Development/Test)
      </h3>
      <p className="backup-panel-note">
        ที่เก็บไฟล์ในเครื่องสำหรับพัฒนา/ทดสอบเท่านั้น — ไม่ใช่ cloud backup
        ไฟล์สำรองถูกเข้ารหัสก่อนเขียนเสมอ และรวมไฟล์เสียงต้นฉบับที่ ledger อ้างถึงไว้ด้วย
      </p>

      <div className="backup-panel-row">
        <span className="backup-panel-state">
          {overview.status.terminalState === "verified" &&
            `สำรองล่าสุดตรวจสอบแล้ว: ${overview.status.archive?.archiveId ?? ""}`}
          {overview.status.terminalState === "no_verified_archive" &&
            "เลือกโฟลเดอร์แล้ว — ยังไม่มีไฟล์สำรองที่ตรวจสอบแล้ว"}
          {overview.status.terminalState === "unavailable" &&
            "ยังไม่พร้อมใช้งาน — ยังไม่ได้เลือกโฟลเดอร์ปลายทาง"}
        </span>
        <button
          className="backup-panel-btn"
          type="button"
          onClick={() => void handleSelectRoot()}
          disabled={busy !== null}
        >
          เลือกโฟลเดอร์ปลายทาง
        </button>
      </div>
      {rootId && <p className="backup-panel-meta">รหัสโฟลเดอร์: {rootId.slice(0, 16)}…</p>}

      {generatedPhrase ? (
        <div className="backup-panel-phrase" role="note">
          <p className="backup-panel-note">
            จดรหัสกู้คืน 24 คำนี้เก็บไว้ — จะแสดงครั้งเดียวเท่านั้น และไม่ถูกบันทึกไว้ที่ใด
          </p>
          <code className="backup-panel-phrase-words">{generatedPhrase}</code>
          <button className="backup-panel-btn" type="button" onClick={handleAcknowledgePhrase}>
            ฉันจดรหัสกู้คืนแล้ว
          </button>
        </div>
      ) : (
        <div className="backup-panel-row">
          <input
            className="backup-panel-input"
            type="password"
            placeholder="รหัสกู้คืน 24 คำ"
            value={backupPhrase}
            onChange={(event) => setBackupPhrase(event.target.value)}
            autoComplete="off"
          />
          <button
            className="backup-panel-btn"
            type="button"
            onClick={() => void handleGeneratePhrase()}
            disabled={busy !== null}
          >
            สร้างรหัสกู้คืนใหม่
          </button>
        </div>
      )}

      <button
        className="backup-panel-btn"
        type="button"
        onClick={() => void handleRunBackup()}
        disabled={
          busy !== null ||
          overview.status.terminalState === "unavailable" ||
          backupPhrase.trim().length === 0 ||
          generatedPhrase !== null
        }
      >
        {busy === "backup" ? "กำลังสำรอง..." : "สำรองข้อมูลตอนนี้"}
      </button>

      {overview.archives.length > 0 && (
        <div className="backup-panel-archives">
          <p className="backup-panel-label">ไฟล์สำรองที่ตรวจสอบแล้ว</p>
          {overview.archives.map((archive: BackupArchiveRecord) => (
            <label key={archive.archiveId} className="backup-panel-archive">
              <input
                type="radio"
                name="restore-archive"
                checked={restoreArchiveId === archive.archiveId}
                onChange={() => setRestoreArchiveId(archive.archiveId)}
              />
              <span>
                {archive.archiveId} · {archive.timestamp} · {formatBytes(archive.byteCount)}
              </span>
            </label>
          ))}

          <div className="backup-panel-row">
            <span className="backup-panel-state">
              {restoreTargetId
                ? `โฟลเดอร์กู้คืน: ${restoreTargetId.slice(0, 16)}…`
                : "ยังไม่ได้เลือกโฟลเดอร์กู้คืน"}
            </span>
            <button
              className="backup-panel-btn"
              type="button"
              onClick={() => void handleSelectRestoreTarget()}
              disabled={busy !== null}
            >
              เลือกโฟลเดอร์กู้คืน
            </button>
          </div>
          <input
            className="backup-panel-input"
            type="password"
            placeholder="รหัสกู้คืน 24 คำ"
            value={restorePhrase}
            onChange={(event) => setRestorePhrase(event.target.value)}
            autoComplete="off"
          />
          <label className="backup-panel-confirm">
            <input
              type="checkbox"
              checked={restoreConfirmed}
              onChange={(event) => setRestoreConfirmed(event.target.checked)}
            />
            <span>
              เข้าใจว่าการกู้คืนจะสร้างสำเนาในโฟลเดอร์ใหม่ที่ว่างเท่านั้น และไม่เขียนทับข้อมูลปัจจุบัน
            </span>
          </label>
          <button
            className="backup-panel-btn"
            type="button"
            onClick={() => void handleRunRestore()}
            disabled={
              busy !== null ||
              !restoreConfirmed ||
              !restoreArchiveId ||
              restorePhrase.trim().length === 0 ||
              !restoreTargetId
            }
          >
            {busy === "restore" ? "กำลังกู้คืน..." : "กู้คืนสู่โฟลเดอร์ว่าง"}
          </button>
        </div>
      )}

      <GoogleDrivePanel invoke={invoke} localArchives={overview.archives} />

      {projectId && (
        <div className="backup-panel-archives">
          <p className="backup-panel-label">ความครบถ้วนของเสียงต้นฉบับ</p>
          <div className="backup-panel-row">
            <span className="backup-panel-state">
              {integrity ? integrity.text : "ยังไม่ได้ตรวจไฟล์เสียงของโปรเจกต์นี้"}
            </span>
            <button
              className="backup-panel-btn"
              type="button"
              onClick={() => void handleVerifyAudio()}
              disabled={verifying || busy !== null}
            >
              {verifying ? "กำลังตรวจ..." : "ตรวจไฟล์เสียง"}
            </button>
          </div>
        </div>
      )}

      {integrity && (
        <p className={`backup-panel-message ${integrity.ok ? "success" : "error"}`}>
          {integrity.text}
        </p>
      )}

      {message && <p className={`backup-panel-message ${message.type}`}>{message.text}</p>}
    </section>
  );
}
