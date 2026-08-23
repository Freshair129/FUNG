import { useCallback, useEffect, useState } from "react";
import { Cloud, RefreshCw, ShieldCheck } from "lucide-react";
import {
  cancelGoogleDriveConnect,
  connectGoogleDrive,
  createGoogleDriveRestoreIntent,
  describeGoogleDriveError,
  disconnectGoogleDrive,
  getDriveConnectionStatus,
  googleDriveClientId,
  listGoogleDriveArchives,
  restoreGoogleDriveArchive,
  uploadGoogleDriveArchive,
  type DriveArchiveSummary,
  type DriveConnectionStatus,
} from "../lib/googleDriveFlow";
import {
  formatBytes,
  selectRestoreTarget,
  type BackupArchiveRecord,
  type InvokeFn,
} from "../lib/backupFlow";
import "./GoogleDrivePanel.css";

type GoogleDrivePanelProps = {
  invoke: InvokeFn;
  localArchives: BackupArchiveRecord[];
};

type PanelMessage = { type: "success" | "error" | "warning"; text: string } | null;

export function GoogleDrivePanel({ invoke, localArchives }: GoogleDrivePanelProps) {
  const [status, setStatus] = useState<DriveConnectionStatus | null>(null);
  const [remoteArchives, setRemoteArchives] = useState<DriveArchiveSummary[]>([]);
  const [selectedLocalId, setSelectedLocalId] = useState("");
  const [selectedRemoteId, setSelectedRemoteId] = useState("");
  const [restoreTargetId, setRestoreTargetId] = useState<string | null>(null);
  const [restorePhrase, setRestorePhrase] = useState("");
  const [restoreConfirmed, setRestoreConfirmed] = useState(false);
  const [oauthSessionId, setOauthSessionId] = useState<string | null>(null);
  const [busy, setBusy] = useState<"connect" | "load" | "upload" | "restore" | "disconnect" | null>(null);
  const [message, setMessage] = useState<PanelMessage>(null);
  const configured = Boolean(googleDriveClientId());

  const refreshRemote = useCallback(async () => {
    if (!status?.connected) return;
    setBusy("load");
    try {
      setRemoteArchives(await listGoogleDriveArchives(invoke));
    } catch (error) {
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    } finally {
      setBusy(null);
    }
  }, [invoke, status?.connected]);

  const refreshStatus = useCallback(async () => {
    try {
      const next = await getDriveConnectionStatus(invoke);
      setStatus(next);
      if (next.connected) {
        setBusy("load");
        try {
          setRemoteArchives(await listGoogleDriveArchives(invoke));
        } finally {
          setBusy(null);
        }
      } else {
        setRemoteArchives([]);
      }
    } catch (error) {
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    }
  }, [invoke]);

  useEffect(() => {
    if (configured) void refreshStatus();
  }, [configured, refreshStatus]);

  useEffect(() => {
    if (!selectedLocalId && localArchives[0]) setSelectedLocalId(localArchives[0].archiveId);
    if (selectedLocalId && !localArchives.some((archive) => archive.archiveId === selectedLocalId)) {
      setSelectedLocalId(localArchives[0]?.archiveId ?? "");
    }
  }, [localArchives, selectedLocalId]);

  if (!configured) {
    return (
      <section className="drive-panel" aria-label="Google Drive">
        <h3 className="drive-panel-title"><Cloud size={16} /> Google Drive Backup</h3>
        <p className="drive-panel-note">
          ยังไม่เปิดใช้ในเครื่องนี้ — ต้องตั้งค่า Installed-app OAuth Client ID ก่อน
          (ค่า public นี้ไม่ใช่ secret และไม่แทน credential ของผู้ใช้)
        </p>
      </section>
    );
  }

  const selectedLocal = localArchives.find((archive) => archive.archiveId === selectedLocalId);
  const selectedRemote = remoteArchives.find((archive) => archive.fileId === selectedRemoteId);

  const handleConnect = async () => {
    setBusy("connect");
    setMessage(null);
    try {
      const next = await connectGoogleDrive(invoke, setOauthSessionId);
      setOauthSessionId(null);
      setStatus(next);
      setMessage({ type: "success", text: "เชื่อมต่อ Google Drive แล้ว — refresh token อยู่ใน OS keyring" });
      setBusy("load");
      setRemoteArchives(await listGoogleDriveArchives(invoke));
    } catch (error) {
      setOauthSessionId(null);
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    } finally {
      setBusy(null);
    }
  };

  const handleCancel = async () => {
    if (!oauthSessionId) return;
    try {
      await cancelGoogleDriveConnect(invoke, oauthSessionId);
    } catch (error) {
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    }
  };

  const handleUpload = async () => {
    if (!selectedLocal) return;
    setBusy("upload");
    setMessage(null);
    try {
      const uploaded = await uploadGoogleDriveArchive(invoke, selectedLocal);
      setRemoteArchives((current) => [
        uploaded,
        ...current.filter((archive) => archive.fileId !== uploaded.fileId),
      ]);
      setMessage({ type: "success", text: `อัปโหลด ${uploaded.archiveId} ไป appDataFolder แล้ว` });
    } catch (error) {
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    } finally {
      setBusy(null);
    }
  };

  const handleSelectRestoreTarget = async () => {
    const picked = await selectRestoreTarget(invoke);
    setRestoreTargetId(picked.terminalState === "selected" ? (picked.selectedTargetId ?? null) : null);
  };

  const handleRestore = async () => {
    if (!selectedRemote) return;
    setBusy("restore");
    setMessage(null);
    try {
      const restoreIntentId = await createGoogleDriveRestoreIntent(
        invoke,
        selectedRemote.archiveId,
        restoreConfirmed && Boolean(restoreTargetId),
      );
      const result = await restoreGoogleDriveArchive(
        invoke,
        selectedRemote,
        restorePhrase,
        restoreIntentId,
      );
      setMessage({ type: "success", text: `กู้คืน ${result.archiveId} ไปยังโฟลเดอร์ใหม่แล้ว` });
      setRestorePhrase("");
      setRestoreConfirmed(false);
    } catch (error) {
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    } finally {
      setBusy(null);
    }
  };

  const handleDisconnect = async () => {
    setBusy("disconnect");
    setMessage(null);
    try {
      const result = await disconnectGoogleDrive(invoke);
      setStatus(result);
      setRemoteArchives([]);
      setMessage({ type: "success", text: "ตัดการเชื่อมต่อ Google Drive แล้ว" });
    } catch (error) {
      setMessage({ type: "error", text: describeGoogleDriveError(error) });
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="drive-panel" aria-label="Google Drive Backup">
      <h3 className="drive-panel-title"><Cloud size={16} /> Google Drive Backup</h3>
      <p className="drive-panel-note">
        ใช้เฉพาะ <code>drive.appdata</code> — FUNG เก็บ refresh token ใน OS keyring ของ Desktop
        และไม่ส่ง token หรือ audio/transcript เข้า Supabase
      </p>

      {!status?.connected ? (
        <div className="drive-panel-row">
          <span className="drive-panel-state">ยังไม่ได้เชื่อมต่อ Google Drive</span>
          {oauthSessionId ? (
            <button className="backup-panel-btn" type="button" onClick={() => void handleCancel()}>
              ยกเลิกการอนุญาต
            </button>
          ) : (
            <button
              className="backup-panel-btn"
              type="button"
              onClick={() => void handleConnect()}
              disabled={busy !== null}
            >
              {busy === "connect" ? "รอการอนุญาตในเบราว์เซอร์…" : "เชื่อมต่อ Google Drive"}
            </button>
          )}
        </div>
      ) : (
        <>
          <div className="drive-panel-row">
            <span className="drive-panel-state drive-panel-connected">
              <ShieldCheck size={14} /> เชื่อมต่อแล้ว · scope: {status.scope}
            </span>
            <button
              className="backup-panel-btn drive-panel-secondary-btn"
              type="button"
              onClick={() => void refreshRemote()}
              disabled={busy !== null}
              aria-label="รีเฟรชรายการ Google Drive"
            >
              <RefreshCw size={14} /> รีเฟรช
            </button>
            <button
              className="backup-panel-btn drive-panel-danger-btn"
              type="button"
              onClick={() => void handleDisconnect()}
              disabled={busy !== null}
            >
              ตัดการเชื่อมต่อ
            </button>
          </div>

          <div className="drive-panel-block">
            <p className="backup-panel-label">อัปโหลด archive ที่ตรวจสอบแล้วจากเครื่อง</p>
            {localArchives.length > 0 ? (
              <>
                {localArchives.map((archive) => (
                  <label key={archive.archiveId} className="backup-panel-archive">
                    <input
                      type="radio"
                      name="drive-upload-archive"
                      checked={selectedLocalId === archive.archiveId}
                      onChange={() => setSelectedLocalId(archive.archiveId)}
                    />
                    <span>{archive.archiveId} · {formatBytes(archive.byteCount)} · {archive.digest.slice(0, 12)}…</span>
                  </label>
                ))}
                <button
                  className="backup-panel-btn"
                  type="button"
                  onClick={() => void handleUpload()}
                  disabled={busy !== null || !selectedLocal}
                >
                  {busy === "upload" ? "กำลังอัปโหลด…" : "อัปโหลด archive"}
                </button>
              </>
            ) : (
              <p className="drive-panel-note">ยังไม่มี archive ในเครื่องที่ตรวจสอบแล้ว</p>
            )}
          </div>

          <div className="drive-panel-block">
            <p className="backup-panel-label">ไฟล์สำรองใน Google Drive appDataFolder</p>
            {remoteArchives.length > 0 ? (
              remoteArchives.map((archive) => (
                <label key={archive.fileId} className="backup-panel-archive">
                  <input
                    type="radio"
                    name="drive-restore-archive"
                    checked={selectedRemoteId === archive.fileId}
                    onChange={() => setSelectedRemoteId(archive.fileId)}
                  />
                  <span>{archive.archiveId} · {formatBytes(archive.byteCount)} · {archive.modifiedTime ?? "ไม่ทราบเวลา"}</span>
                </label>
              ))
            ) : (
              <p className="drive-panel-note">ยังไม่มี archive บน Google Drive</p>
            )}
            <div className="drive-panel-row">
              <span className="drive-panel-state">
                {restoreTargetId ? `โฟลเดอร์กู้คืน: ${restoreTargetId.slice(0, 16)}…` : "ยังไม่ได้เลือกโฟลเดอร์กู้คืน"}
              </span>
              <button className="backup-panel-btn" type="button" onClick={() => void handleSelectRestoreTarget()} disabled={busy !== null}>
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
              <input type="checkbox" checked={restoreConfirmed} onChange={(event) => setRestoreConfirmed(event.target.checked)} />
              <span>เข้าใจว่าการกู้คืนจะสร้างสำเนาในโฟลเดอร์ใหม่ที่ว่างเท่านั้น</span>
            </label>
            <button
              className="backup-panel-btn"
              type="button"
              onClick={() => void handleRestore()}
              disabled={busy !== null || !selectedRemote || !restoreTargetId || !restoreConfirmed || !restorePhrase.trim()}
            >
              {busy === "restore" ? "กำลังกู้คืน…" : "กู้คืนจาก Google Drive"}
            </button>
          </div>
        </>
      )}

      {message && <p className={`backup-panel-message ${message.type === "warning" ? "warning" : message.type}`}>{message.text}</p>}
    </section>
  );
}
