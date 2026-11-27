import { useCallback, useEffect, useState } from "react";
import {
  diarizationAddonCancel,
  diarizationAddonInstall,
  diarizationAddonRemove,
  diarizationClearToken,
  diarizationFetchModel,
  diarizationOpenPage,
  diarizationSetToken,
  diarizationStatus,
  onDiarizationAddonFinished,
  onDiarizationAddonProgress,
  type DiarizationAddonProgress,
  type DiarizationReadiness,
} from "../tauri";
import "./DiarizationSetupPanel.css";

function megabytes(bytes: number): string {
  return `${Math.round(bytes / 1_048_576).toLocaleString("th-TH")} MB`;
}

/**
 * Turns on speaker separation from inside the app: downloads the add-on
 * (pinned wheels from PyPI), then walks the user through the gated model's
 * terms and token. Each step is driven by the readiness probe's blocker, so
 * the panel only ever offers the *next* thing to do.
 *
 * See `docs/specs/2026-11-26-diarization-in-app-enable-flow.md`.
 */
export function DiarizationSetupPanel({ onChanged }: { onChanged?: (readiness: DiarizationReadiness | null) => void }) {
  const [readiness, setReadiness] = useState<DiarizationReadiness | null>(null);
  const [progress, setProgress] = useState<DiarizationAddonProgress | null>(null);
  const [installing, setInstalling] = useState(false);
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await diarizationStatus();
      setReadiness(next);
      onChanged?.(next);
    } catch (err) {
      setError(String(err));
    }
  }, [onChanged]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    const disposers: Array<() => void> = [];
    void (async () => {
      const offProgress = await onDiarizationAddonProgress((next) => {
        setInstalling(next.phase !== "done");
        setProgress(next);
      });
      const offFinished = await onDiarizationAddonFinished((finished) => {
        setInstalling(false);
        setProgress(null);
        if (!finished.ok) setError(finished.detail ?? finished.code ?? "ติดตั้งไม่สำเร็จ");
        void refresh();
      });
      if (disposed) {
        offProgress();
        offFinished();
      } else {
        disposers.push(offProgress, offFinished);
      }
    })();
    return () => {
      disposed = true;
      for (const dispose of disposers) dispose();
    };
  }, [refresh]);

  const run = async (action: () => Promise<void>) => {
    setError(null);
    setNotice(null);
    setBusy(true);
    try {
      await action();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const startInstall = () =>
    run(async () => {
      setInstalling(true);
      try {
        await diarizationAddonInstall();
      } catch (err) {
        setInstalling(false);
        throw err;
      }
    });

  const saveTokenAndFetch = () =>
    run(async () => {
      await diarizationSetToken(token);
      setToken("");
      const outcome = await diarizationFetchModel();
      if (outcome.ok) {
        setNotice("ดาวน์โหลดโมเดลแล้ว พร้อมแยกผู้พูด");
      } else {
        setError(outcome.detail ?? "ดาวน์โหลดโมเดลไม่สำเร็จ");
      }
      await refresh();
    });

  const remove = (includeModel: boolean) =>
    run(async () => {
      await diarizationAddonRemove(includeModel);
      setConfirmRemove(false);
      setNotice(includeModel ? "ลบส่วนเสริมและโมเดลแล้ว" : "ลบส่วนเสริมแล้ว");
      await refresh();
    });

  if (readiness === null) {
    return (
      <section className="diarization-setup" aria-label="การแยกเสียงผู้พูด">
        <p className="diarization-setup__note">{error ?? "กำลังตรวจสอบ…"}</p>
      </section>
    );
  }

  const needsAddon = readiness.blocker === "dependenciesMissing";
  const needsModel = readiness.blocker === "modelNotFetched" || (readiness.available && !readiness.modelPresent);
  const unrecoverable = readiness.blocker === "runtimeMissing" || readiness.blocker === "workerMissing";
  const percent = progress && progress.totalBytes > 0
    ? Math.min(100, Math.round((progress.doneBytes / progress.totalBytes) * 100))
    : 0;

  return (
    <section className="diarization-setup" aria-label="การแยกเสียงผู้พูด">
      <header className="diarization-setup__header">
        <h3>การแยกเสียงผู้พูด</h3>
        <span className={`diarization-setup__badge${readiness.available ? " is-ready" : ""}`}>
          {readiness.available ? "พร้อมใช้" : "ยังไม่ได้เปิดใช้"}
        </span>
      </header>
      <p className="diarization-setup__lede">
        แยกเสียงในไฟล์ออกเป็น Speaker 1, Speaker 2 … ด้วยโมเดล {readiness.model} ที่รันในเครื่องนี้
        เสียงไม่ถูกส่งออกจากเครื่อง
      </p>

      {unrecoverable && <p className="diarization-setup__blocked">{readiness.detail}</p>}

      {needsAddon && !installing && (
        <div className="diarization-setup__step">
          <p>
            ขั้นที่ 1: ดาวน์โหลดส่วนเสริมประมาณ <strong>{megabytes(readiness.addonDownloadBytes)}</strong> จาก PyPI
            (pyannote.audio และ PyTorch) ใช้พื้นที่ดิสก์หลังติดตั้งประมาณ {megabytes(readiness.addonDownloadBytes * 3.5)}
          </p>
          <button type="button" className="diarization-setup__primary" disabled={busy} onClick={() => void startInstall()}>
            เปิดใช้การแยกเสียงผู้พูด
          </button>
        </div>
      )}

      {installing && (
        <div className="diarization-setup__step" role="status">
          <p>
            {progress?.phase === "unpacking"
              ? `กำลังติดตั้ง ${progress.doneFiles}/${progress.totalFiles} แพ็กเกจ`
              : progress
                ? `กำลังดาวน์โหลด ${megabytes(progress.doneBytes)} จาก ${megabytes(progress.totalBytes)}`
                : "กำลังเริ่ม…"}
          </p>
          <progress max={100} value={progress?.phase === "unpacking" ? (progress.doneFiles / Math.max(1, progress.totalFiles)) * 100 : percent} />
          <button type="button" onClick={() => void diarizationAddonCancel()}>
            ยกเลิก
          </button>
        </div>
      )}

      {!needsAddon && !unrecoverable && needsModel && !installing && (
        <div className="diarization-setup__step">
          <p>ขั้นที่ 2: โมเดลนี้ต้องขอสิทธิ์จากเจ้าของก่อนใช้ ทำครั้งเดียวด้วยบัญชี Hugging Face ของคุณ</p>
          <ol>
            <li>
              เข้าสู่ระบบ Hugging Face แล้วกดยอมรับเงื่อนไขทั้งสองหน้า{" "}
              <button type="button" className="diarization-setup__link" onClick={() => void diarizationOpenPage("pipeline")}>
                speaker-diarization-3.1
              </button>{" "}
              และ{" "}
              <button type="button" className="diarization-setup__link" onClick={() => void diarizationOpenPage("segmentation")}>
                segmentation-3.0
              </button>
            </li>
            <li>
              สร้าง token แบบ Read ที่{" "}
              <button type="button" className="diarization-setup__link" onClick={() => void diarizationOpenPage("tokens")}>
                หน้าการตั้งค่า token
              </button>{" "}
              แล้ววางด้านล่าง
            </li>
          </ol>
          <label className="diarization-setup__field">
            <span>Hugging Face token</span>
            <input
              type="password"
              autoComplete="off"
              spellCheck={false}
              placeholder="hf_…"
              value={token}
              disabled={busy}
              onChange={(event) => setToken(event.target.value)}
            />
          </label>
          <p className="diarization-setup__note">
            token เก็บใน Windows Credential Manager เท่านั้น ใช้ครั้งเดียวตอนดาวน์โหลดโมเดล
          </p>
          <button
            type="button"
            className="diarization-setup__primary"
            disabled={busy || !token.trim()}
            onClick={() => void saveTokenAndFetch()}
          >
            {busy ? "กำลังตรวจสอบและดาวน์โหลด…" : "ตรวจสอบและดาวน์โหลดโมเดล"}
          </button>
        </div>
      )}

      {readiness.available && readiness.modelPresent && (
        <div className="diarization-setup__step">
          <p className="diarization-setup__note">โมเดลอยู่ที่ {readiness.cacheRoot} ใช้งานได้โดยไม่ต้องต่ออินเทอร์เน็ต</p>
          {readiness.tokenConfigured && (
            <button type="button" disabled={busy} onClick={() => void run(async () => {
              await diarizationClearToken();
              setNotice("ลบ token แล้ว");
              await refresh();
            })}>
              ลบ token (ไม่ต้องใช้แล้ว)
            </button>
          )}
        </div>
      )}

      {readiness.addonInstalled && !installing && (
        <div className="diarization-setup__remove">
          {confirmRemove ? (
            <>
              <p>ลบส่วนเสริมออกจากเครื่อง? ถ้าลบโมเดลด้วย จะต้องดาวน์โหลดและใส่ token ใหม่เมื่อเปิดใช้อีกครั้ง</p>
              <div className="diarization-setup__actions">
                <button type="button" disabled={busy} onClick={() => void remove(false)}>ลบเฉพาะส่วนเสริม</button>
                <button type="button" disabled={busy} onClick={() => void remove(true)}>ลบทั้งส่วนเสริมและโมเดล</button>
                <button type="button" disabled={busy} onClick={() => setConfirmRemove(false)}>ไม่ลบ</button>
              </div>
            </>
          ) : (
            <button type="button" disabled={busy} onClick={() => setConfirmRemove(true)}>
              ลบการแยกเสียงผู้พูด…
            </button>
          )}
        </div>
      )}

      {notice && <p className="diarization-setup__notice" role="status">{notice}</p>}
      {error && <p className="diarization-setup__error" role="alert">{error}</p>}
    </section>
  );
}
