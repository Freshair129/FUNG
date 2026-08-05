import { useCallback, useEffect, useRef, useState } from "react";
import {
  zoomConnect,
  zoomConnectionStatus,
  zoomDisconnect,
  zoomImportRecording,
  zoomListRecordings,
  type ZoomConnectionStatus,
  type ZoomRecordingSummary,
} from "../tauri";
import "./ZoomPanel.css";

const STATUS_LABEL: Record<ZoomConnectionStatus["status"], string> = {
  disconnected: "ยังไม่ได้เชื่อมต่อ",
  connecting: "กำลังเชื่อมต่อ… ยืนยันใน browser",
  connected: "เชื่อมต่อแล้ว",
  error: "ต้องเชื่อมต่อใหม่",
};

export function ZoomPanel({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<ZoomConnectionStatus>({ status: "disconnected", accountLabel: null });
  const [recordings, setRecordings] = useState<ZoomRecordingSummary[]>([]);
  const [busyUuid, setBusyUuid] = useState<string | null>(null);
  const [importedUuids, setImportedUuids] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await zoomConnectionStatus());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
    pollRef.current = window.setInterval(() => void refreshStatus(), 2000);
    return () => {
      if (pollRef.current !== null) window.clearInterval(pollRef.current);
    };
  }, [refreshStatus]);

  useEffect(() => {
    if (status.status !== "connected") return;
    zoomListRecordings().then(setRecordings).catch((err) => setError(String(err)));
  }, [status.status]);

  const handleConnect = async () => {
    setError(null);
    try {
      setStatus(await zoomConnect());
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDisconnect = async () => {
    setError(null);
    try {
      setStatus(await zoomDisconnect());
      setRecordings([]);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleImport = async (uuid: string) => {
    setBusyUuid(uuid);
    setError(null);
    try {
      await zoomImportRecording(uuid);
      setImportedUuids((prev) => new Set(prev).add(uuid));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusyUuid(null);
    }
  };

  return (
    <div className="zoom-panel-backdrop" role="dialog" aria-label="Zoom import">
      <div className="zoom-panel">
        <header className="zoom-panel-header">
          <h2>Import from Zoom</h2>
          <button type="button" onClick={onClose} aria-label="Close">×</button>
        </header>
        <div className="zoom-panel-status">
          <span data-status={status.status}>{STATUS_LABEL[status.status]}</span>
          {status.accountLabel && <span className="zoom-panel-account">{status.accountLabel}</span>}
          {status.status === "connected" ? (
            <button type="button" onClick={handleDisconnect}>Disconnect</button>
          ) : (
            <button type="button" onClick={handleConnect} disabled={status.status === "connecting"}>
              Connect Zoom
            </button>
          )}
        </div>
        {error && <p className="zoom-panel-error">{error}</p>}
        {status.status === "connected" && (
          <ul className="zoom-panel-list">
            {recordings.length === 0 && <li className="zoom-panel-empty">ไม่พบ cloud recording ใน 30 วันที่ผ่านมา</li>}
            {recordings.map((recording) => (
              <li key={recording.uuid}>
                <div>
                  <strong>{recording.topic}</strong>
                  <span>
                    {new Date(recording.startTime).toLocaleString()} · {recording.durationMinutes} นาที ·{" "}
                    {recording.hasParticipantAudio ? "เสียงแยกรายคน ✓" : "เสียงรวม (แยกผู้พูดด้วย AI)"}
                  </span>
                </div>
                <button
                  type="button"
                  disabled={busyUuid === recording.uuid || importedUuids.has(recording.uuid)}
                  onClick={() => void handleImport(recording.uuid)}
                >
                  {importedUuids.has(recording.uuid) ? "Imported ✓" : busyUuid === recording.uuid ? "Importing…" : "Import"}
                </button>
              </li>
            ))}
          </ul>
        )}
        <p className="zoom-panel-note">
          ไฟล์เสียงและ transcript ประมวลผลและเก็บในเครื่องนี้เท่านั้น ติดตามความคืบหน้าได้จากรายการ Jobs
        </p>
      </div>
    </div>
  );
}
