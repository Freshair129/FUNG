import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, CheckCircle2, HardDrive, Link2, Monitor, User, X } from "lucide-react";
import { supabase } from "../lib/supabase";
import {
  describeBackupError,
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
import "./AccountSettings.css";

type AccountSettingsProps = {
  onClose: () => void;
};

type OAuthConnection = {
  id: string;
  provider: string;
  status: string;
};

async function tauriInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

const nativeInvoke: InvokeFn = tauriInvoke;

export function AccountSettings({ onClose }: AccountSettingsProps) {
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [savedName, setSavedName] = useState("");
  const [connections, setConnections] = useState<OAuthConnection[]>([]);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  // --- Filesystem test backup (development/test only) ---
  const [backupOverview, setBackupOverview] = useState<BackupOverview>({
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
  const [backupBusy, setBackupBusy] = useState<"backup" | "restore" | null>(null);
  const [backupMessage, setBackupMessage] = useState<
    { type: "success" | "error"; text: string } | null
  >(null);

  const refreshBackup = useCallback(async () => {
    setBackupOverview(await loadBackupOverview(nativeInvoke));
  }, []);

  useEffect(() => {
    void refreshBackup();
  }, [refreshBackup]);

  const handleSelectRoot = async () => {
    setBackupMessage(null);
    const status = await selectBackupRoot(nativeInvoke);
    setRootId(status.terminalState === "selected" ? (status.selectedRootId ?? null) : null);
    await refreshBackup();
  };

  const handleSelectRestoreTarget = async () => {
    setBackupMessage(null);
    const status = await selectRestoreTarget(nativeInvoke);
    setRestoreTargetId(
      status.terminalState === "selected" ? (status.selectedTargetId ?? null) : null,
    );
  };

  const handleGeneratePhrase = async () => {
    setBackupMessage(null);
    try {
      const phrase = await generateRecoveryPhrase(nativeInvoke);
      setGeneratedPhrase(phrase);
      setBackupPhrase(phrase);
    } catch (err) {
      setBackupMessage({ type: "error", text: describeBackupError(err) });
    }
  };

  const handleAcknowledgePhrase = () => {
    // One-time display ends here; the phrase remains only in the transient
    // backup input state until the run completes.
    setGeneratedPhrase(null);
  };

  const handleRunBackup = async () => {
    setBackupBusy("backup");
    setBackupMessage(null);
    try {
      const record = await runBackup(nativeInvoke, backupPhrase);
      setBackupMessage({
        type: "success",
        text: `สำรองสำเร็จ (${record.archiveId}) — ตรวจสอบแล้ว`,
      });
    } catch (err) {
      setBackupMessage({ type: "error", text: describeBackupError(err) });
    } finally {
      setBackupPhrase("");
      setGeneratedPhrase(null);
      setBackupBusy(null);
      await refreshBackup();
    }
  };

  const handleRunRestore = async () => {
    setBackupBusy("restore");
    setBackupMessage(null);
    try {
      const result = await runRestore(
        nativeInvoke,
        restoreArchiveId,
        restorePhrase,
        restoreConfirmed,
      );
      setBackupMessage({
        type: "success",
        text: `กู้คืนสู่โฟลเดอร์ใหม่สำเร็จ (${result.archiveId})`,
      });
    } catch (err) {
      setBackupMessage({ type: "error", text: describeBackupError(err) });
    } finally {
      setRestorePhrase("");
      setRestoreConfirmed(false);
      setBackupBusy(null);
    }
  };

  useEffect(() => {
    const load = async () => {
      const {
        data: { user },
      } = await supabase.auth.getUser();
      if (!user) return;

      setEmail(user.email ?? "");

      const { data: profile, error: profileError } = await supabase
        .from("profiles")
        .select("display_name")
        .eq("id", user.id)
        .single();

      if (profileError) console.error("Failed to load profile:", profileError);

      if (profile) {
        setDisplayName(profile.display_name ?? "");
        setSavedName(profile.display_name ?? "");
      }

      const { data: oauthConnections, error: connError } = await supabase
        .from("oauth_connections")
        .select("id, provider, status")
        .eq("user_id", user.id);

      if (connError) console.error("Failed to load connections:", connError);

      if (oauthConnections) {
        setConnections(oauthConnections);
      }
    };

    void load();
  }, []);

  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleEsc);
    return () => document.removeEventListener("keydown", handleEsc);
  }, [onClose]);

  const handleSave = async () => {
    const trimmed = displayName.trim();
    if (!trimmed || trimmed === savedName) return;

    setSaving(true);
    setMessage(null);

    try {
      const {
        data: { user },
      } = await supabase.auth.getUser();
      if (!user) throw new Error("ไม่พบ session");

      const { error } = await supabase
        .from("profiles")
        .update({ display_name: trimmed })
        .eq("id", user.id);

      if (error) throw error;

      setSavedName(trimmed);
      setMessage({ type: "success", text: "บันทึกแล้ว" });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "บันทึกไม่สำเร็จ",
      });
    } finally {
      setSaving(false);
    }
  };

  const nameChanged = displayName.trim() !== savedName && displayName.trim().length > 0;

  return (
    <div className="account-settings-overlay" role="presentation" onMouseDown={onClose}>
      <section
        className="account-settings-panel"
        aria-label="ตั้งค่าบัญชี"
        aria-modal="true"
        role="dialog"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="account-settings-header">
          <h2>
            <button
              className="account-settings-back"
              type="button"
              onClick={onClose}
              aria-label="ปิด"
            >
              <ArrowLeft size={18} />
            </button>
            ตั้งค่าบัญชี
          </h2>
          <button
            className="account-settings-back"
            type="button"
            onClick={onClose}
            aria-label="ปิด"
          >
            <X size={18} />
          </button>
        </header>

        {/* Profile section */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <User size={16} /> โปรไฟล์
          </h3>
          <div className="account-settings-card">
            <div className="account-settings-field">
              <label className="account-settings-label">ชื่อแสดง</label>
              <input
                className="account-settings-input"
                type="text"
                value={displayName}
                onChange={(e) => {
                  setDisplayName(e.target.value);
                  setMessage(null);
                }}
                maxLength={120}
              />
            </div>
            <div className="account-settings-field">
              <label className="account-settings-label">อีเมล</label>
              <input
                className="account-settings-input"
                type="email"
                value={email}
                readOnly
              />
            </div>
            <button
              className="account-settings-save-btn"
              type="button"
              onClick={() => void handleSave()}
              disabled={saving || !nameChanged}
            >
              {saving ? "กำลังบันทึก..." : "บันทึก"}
            </button>
            {message && (
              <p className={`account-settings-message ${message.type}`}>{message.text}</p>
            )}
          </div>
        </div>

        {/* Connected accounts section */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <Link2 size={16} /> บัญชีที่เชื่อมต่อ
          </h3>
          <div className="account-settings-card">
            {connections.filter((c) => c.status === "active").length > 0 ? (
              connections
                .filter((c) => c.status === "active")
                .map((c) => (
                  <div key={c.id} className="account-settings-connected">
                    <CheckCircle2 size={16} />
                    {c.provider === "google" ? "Google" : c.provider} — เชื่อมต่อแล้ว
                  </div>
                ))
            ) : (
              <p className="account-settings-placeholder">ไม่มีบัญชีที่เชื่อมต่อ</p>
            )}
          </div>
        </div>

        {/* Filesystem test backup (development/test only; Google Drive is TODO) */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <HardDrive size={16} /> สำรองข้อมูล (Development/Test)
          </h3>
          <div className="account-settings-card">
            <p className="account-settings-backup-note">
              ที่เก็บไฟล์ในเครื่องสำหรับพัฒนา/ทดสอบเท่านั้น — ไม่ใช่ cloud backup
              ไฟล์สำรองถูกเข้ารหัสก่อนเขียนเสมอ
            </p>

            <div className="account-settings-backup-row">
              <span className="account-settings-backup-state">
                {backupOverview.status.terminalState === "verified" &&
                  `สำรองล่าสุดตรวจสอบแล้ว: ${backupOverview.status.archive?.archiveId ?? ""}`}
                {backupOverview.status.terminalState === "no_verified_archive" &&
                  "เลือกโฟลเดอร์แล้ว — ยังไม่มีไฟล์สำรองที่ตรวจสอบแล้ว"}
                {backupOverview.status.terminalState === "unavailable" &&
                  "ยังไม่พร้อมใช้งาน — ยังไม่ได้เลือกโฟลเดอร์ปลายทาง"}
              </span>
              <button
                className="account-settings-save-btn"
                type="button"
                onClick={() => void handleSelectRoot()}
                disabled={backupBusy !== null}
              >
                เลือกโฟลเดอร์ปลายทาง
              </button>
            </div>
            {rootId && (
              <p className="account-settings-backup-meta">รหัสโฟลเดอร์: {rootId.slice(0, 16)}…</p>
            )}

            {generatedPhrase ? (
              <div className="account-settings-backup-phrase" role="note">
                <p className="account-settings-backup-note">
                  จดรหัสกู้คืน 24 คำนี้เก็บไว้ — จะแสดงครั้งเดียวเท่านั้น
                  และไม่ถูกบันทึกไว้ที่ใด
                </p>
                <code className="account-settings-backup-phrase-words">{generatedPhrase}</code>
                <button
                  className="account-settings-save-btn"
                  type="button"
                  onClick={handleAcknowledgePhrase}
                >
                  ฉันจดรหัสกู้คืนแล้ว
                </button>
              </div>
            ) : (
              <div className="account-settings-backup-row">
                <input
                  className="account-settings-input"
                  type="password"
                  placeholder="รหัสกู้คืน 24 คำ"
                  value={backupPhrase}
                  onChange={(e) => setBackupPhrase(e.target.value)}
                  autoComplete="off"
                />
                <button
                  className="account-settings-save-btn"
                  type="button"
                  onClick={() => void handleGeneratePhrase()}
                  disabled={backupBusy !== null}
                >
                  สร้างรหัสกู้คืนใหม่
                </button>
              </div>
            )}

            <button
              className="account-settings-save-btn"
              type="button"
              onClick={() => void handleRunBackup()}
              disabled={
                backupBusy !== null ||
                backupOverview.status.terminalState === "unavailable" ||
                backupPhrase.trim().length === 0 ||
                generatedPhrase !== null
              }
            >
              {backupBusy === "backup" ? "กำลังสำรอง..." : "สำรองข้อมูลตอนนี้"}
            </button>

            {backupOverview.archives.length > 0 && (
              <div className="account-settings-backup-archives">
                <p className="account-settings-label">ไฟล์สำรองที่ตรวจสอบแล้ว</p>
                {backupOverview.archives.map((archive: BackupArchiveRecord) => (
                  <label key={archive.archiveId} className="account-settings-backup-archive">
                    <input
                      type="radio"
                      name="restore-archive"
                      checked={restoreArchiveId === archive.archiveId}
                      onChange={() => setRestoreArchiveId(archive.archiveId)}
                    />
                    <span>
                      {archive.archiveId} · {archive.timestamp} ·{" "}
                      {(archive.byteCount / 1024).toFixed(1)} KB
                    </span>
                  </label>
                ))}

                <div className="account-settings-backup-row">
                  <span className="account-settings-backup-state">
                    {restoreTargetId
                      ? `โฟลเดอร์กู้คืน: ${restoreTargetId.slice(0, 16)}…`
                      : "ยังไม่ได้เลือกโฟลเดอร์กู้คืน"}
                  </span>
                  <button
                    className="account-settings-save-btn"
                    type="button"
                    onClick={() => void handleSelectRestoreTarget()}
                    disabled={backupBusy !== null}
                  >
                    เลือกโฟลเดอร์กู้คืน
                  </button>
                </div>
                <input
                  className="account-settings-input"
                  type="password"
                  placeholder="รหัสกู้คืน 24 คำ"
                  value={restorePhrase}
                  onChange={(e) => setRestorePhrase(e.target.value)}
                  autoComplete="off"
                />
                <label className="account-settings-backup-confirm">
                  <input
                    type="checkbox"
                    checked={restoreConfirmed}
                    onChange={(e) => setRestoreConfirmed(e.target.checked)}
                  />
                  <span>
                    เข้าใจว่าการกู้คืนจะสร้างสำเนาในโฟลเดอร์ใหม่ที่ว่างเท่านั้น
                    และไม่เขียนทับข้อมูลปัจจุบัน
                  </span>
                </label>
                <button
                  className="account-settings-save-btn"
                  type="button"
                  onClick={() => void handleRunRestore()}
                  disabled={
                    backupBusy !== null ||
                    !restoreConfirmed ||
                    !restoreArchiveId ||
                    restorePhrase.trim().length === 0 ||
                    !restoreTargetId
                  }
                >
                  {backupBusy === "restore" ? "กำลังกู้คืน..." : "กู้คืนสู่โฟลเดอร์ว่าง"}
                </button>
              </div>
            )}

            {backupMessage && (
              <p className={`account-settings-message ${backupMessage.type}`}>
                {backupMessage.text}
              </p>
            )}
          </div>
        </div>

        {/* Paired devices placeholder */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <Monitor size={16} /> อุปกรณ์ที่จับคู่
          </h3>
          <div className="account-settings-card">
            <p className="account-settings-placeholder">ยังไม่พร้อมใช้งาน</p>
          </div>
        </div>
      </section>
    </div>
  );
}
