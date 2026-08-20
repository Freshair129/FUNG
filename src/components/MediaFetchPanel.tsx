import { useCallback, useEffect, useState } from "react";
import {
  fetchAndTranscribe,
  mediaFetchStatus,
  setMediaFetchConsent,
  type Job,
  type MediaFetchReadiness,
} from "../tauri";
import "./MediaFetchPanel.css";

/**
 * The only surface in FUNG that asks the app to reach the internet on the
 * user's behalf, so it says so plainly rather than presenting a URL box and
 * letting the consequence be inferred.
 *
 * Two states are deliberately distinguished for the user, because they call
 * for different actions: not *permitted* (a switch, here) and not *installed*
 * (a staging script, at a terminal). `blockerCode` decides which is shown —
 * never `detail`, which is prose and free to be reworded.
 */
export function MediaFetchPanel({
  projectId,
  onClose,
  onStarted,
}: {
  projectId: string | null;
  onClose: () => void;
  onStarted: (job: Job) => void;
}) {
  const [readiness, setReadiness] = useState<MediaFetchReadiness | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setReadiness(await mediaFetchStatus());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleConsent = async (enabled: boolean) => {
    setError(null);
    setBusy(true);
    try {
      setReadiness(await setMediaFetchConsent(enabled));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleFetch = async () => {
    setError(null);
    setBusy(true);
    try {
      const job = await fetchAndTranscribe(url.trim(), projectId ?? undefined);
      onStarted(job);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const consentWithheld = readiness?.blockerCode === "consentWithheld";
  // Anything that is not consent is an installation problem, which no button
  // in this panel can fix — so it is stated, not offered as an action.
  const notInstalled = Boolean(
    readiness && !readiness.available && readiness.blockerCode !== "consentWithheld",
  );

  return (
    <div className="media-fetch-backdrop" role="dialog" aria-modal="true" aria-label="ดึงสื่อจากลิงก์">
      <div className="media-fetch">
        <header className="media-fetch-header">
          <h2>ดึงสื่อจากลิงก์</h2>
          <button type="button" aria-label="ปิด" onClick={onClose}>
            ×
          </button>
        </header>

        <p className="media-fetch-lede">
          FUNG จะดาวน์โหลดเฉพาะ<strong>เสียง</strong>จากลิงก์ที่วาง แล้วถอดเสียงในเครื่องตามปกติ
          สิ่งที่ออกจากเครื่องคือลิงก์กับหมายเลข IP ของเครื่องนี้เท่านั้น — ไม่มีเสียงหรือ transcript
          ที่บันทึกไว้ถูกส่งออกไป
        </p>

        {consentWithheld && (
          <div className="media-fetch-consent">
            <p>{readiness?.detail}</p>
            <button type="button" disabled={busy} onClick={() => void handleConsent(true)}>
              อนุญาตให้ดึงสื่อจากอินเทอร์เน็ต
            </button>
          </div>
        )}

        {notInstalled && <p className="media-fetch-blocked">{readiness?.detail}</p>}

        {readiness?.available && (
          <>
            {/* Shown before the attempt, not after it fails: the probe
                already knows YouTube will not work without this. */}
            {readiness.jsRuntimeDetail && (
              <p className="media-fetch-advisory">{readiness.jsRuntimeDetail}</p>
            )}

            <label className="media-fetch-field">
              <span>ลิงก์ (http/https)</span>
              <input
                type="url"
                inputMode="url"
                placeholder="https://…"
                value={url}
                disabled={busy}
                onChange={(event) => setUrl(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && url.trim() && !busy) void handleFetch();
                }}
              />
            </label>

            <div className="media-fetch-actions">
              <button
                type="button"
                className="media-fetch-primary"
                disabled={busy || !url.trim()}
                onClick={() => void handleFetch()}
              >
                {busy ? "กำลังเริ่ม…" : "ดึงและถอดเสียง"}
              </button>
              <button type="button" disabled={busy} onClick={() => void handleConsent(false)}>
                ปิดสิทธิ์นี้
              </button>
            </div>

            <p className="media-fetch-note">
              จำกัดความยาวไม่เกิน {Math.round(readiness.maxDurationS / 3600)} ชั่วโมงต่อลิงก์
              ไฟล์ที่ดึงมาจะเข้าสู่ระบบ custody เหมือนไฟล์ที่นำเข้าเอง — มี checksum และสำรองข้อมูลได้
            </p>
          </>
        )}

        {readiness === null && !error && <p className="media-fetch-note">กำลังตรวจสอบ…</p>}
        {error && <p className="media-fetch-error">{error}</p>}
      </div>
    </div>
  );
}
