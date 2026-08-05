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
  const [status, setStatus] = useState<ZoomConnectionStatus>({ status: "disconnected", accountLabel: null, revokeFailed: false });
  // Held separately from `status`: the 2000ms poll overwrites `status` with
  // `read_connection`'s hardcoded `revoke_failed: false` on its very next
  // tick, which would erase this warning almost immediately. It is the one
  // message that must persist, since only the user can act on it (removing
  // the app in the Zoom Marketplace), so only `handleDisconnect` writes it.
  const [revokeFailed, setRevokeFailed] = useState(false);
  const [recordings, setRecordings] = useState<ZoomRecordingSummary[]>([]);
  const [recordingsLoaded, setRecordingsLoaded] = useState(false);
  const [busyUuids, setBusyUuids] = useState<Set<string>>(new Set());
  const [queuedUuids, setQueuedUuids] = useState<Set<string>>(new Set());
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
    setRecordingsLoaded(false);
    zoomListRecordings()
      .then(setRecordings)
      .catch((err) => setError(String(err)))
      .finally(() => setRecordingsLoaded(true));
  }, [status.status]);

  const handleConnect = async () => {
    setError(null);
    setRevokeFailed(false);
    try {
      setStatus(await zoomConnect());
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDisconnect = async () => {
    setError(null);
    try {
      const result = await zoomDisconnect();
      setStatus(result);
      setRevokeFailed(result.revokeFailed);
      setRecordings([]);
      setRecordingsLoaded(false);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleImport = async (uuid: string) => {
    setBusyUuids((prev) => new Set(prev).add(uuid));
    setError(null);
    try {
      await zoomImportRecording(uuid);
      setQueuedUuids((prev) => new Set(prev).add(uuid));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusyUuids((prev) => {
        const next = new Set(prev);
        next.delete(uuid);
        return next;
      });
    }
  };

  return (
    <div className="zoom-panel-backdrop" role="dialog" aria-label="Zoom import">
      <div className="zoom-panel">
        <header className="zoom-panel-header">
          <h2>นำเข้าจาก Zoom</h2>
          <button type="button" onClick={onClose} aria-label="Close">×</button>
        </header>
        <div className="zoom-panel-status">
          <span data-status={status.status}>{STATUS_LABEL[status.status]}</span>
          {status.accountLabel && <span className="zoom-panel-account">{status.accountLabel}</span>}
          {status.status === "connected" ? (
            <button type="button" onClick={handleDisconnect}>ยกเลิกการเชื่อมต่อ</button>
          ) : (
            <button type="button" onClick={handleConnect} disabled={status.status === "connecting"}>
              เชื่อมต่อ Zoom
            </button>
          )}
        </div>
        {error && <p className="zoom-panel-error">{error}</p>}
        {revokeFailed && (
          <p className="zoom-panel-error">
            ยกเลิกการเชื่อมต่อในเครื่องแล้ว แต่เพิกถอนสิทธิ์ฝั่ง Zoom ไม่สำเร็จ — ลบแอปออกจาก Zoom Marketplace เพื่อตัดสิทธิ์ให้สมบูรณ์
          </p>
        )}
        {status.status === "connected" && (
          <ul className="zoom-panel-list">
            {recordingsLoaded && recordings.length === 0 && (
              <li className="zoom-panel-empty">ไม่พบ cloud recording ใน 30 วันที่ผ่านมา</li>
            )}
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
                  disabled={busyUuids.has(recording.uuid) || queuedUuids.has(recording.uuid)}
                  onClick={() => void handleImport(recording.uuid)}
                >
                  {queuedUuids.has(recording.uuid)
                    ? "ส่งเข้าคิวแล้ว"
                    : busyUuids.has(recording.uuid)
                      ? "กำลังส่ง…"
                      : "นำเข้า"}
                </button>
              </li>
            ))}
          </ul>
        )}
        <p className="zoom-panel-note">
          ไฟล์เสียงและ transcript ประมวลผลและเก็บในเครื่องนี้เท่านั้น การส่งเข้าคิวยังไม่ใช่การนำเข้าเสร็จสมบูรณ์ —
          ติดตามผลลัพธ์จริงได้จากรายการ Jobs
        </p>
      </div>
    </div>
  );
}
