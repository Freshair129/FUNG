import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, CheckCircle2, Link2, Monitor, User, X } from "lucide-react";
import { supabase } from "../lib/supabase";
import { BackupPanel } from "../components/BackupPanel";
import type { InvokeFn } from "../lib/backupFlow";
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

/** The backup commands are Tauri commands. This page also renders in a plain
 * browser (the web Dashboard), where `invoke` does not exist — so hand the
 * panel `null` there and let it say so rather than offering dead controls. */
const nativeInvoke: InvokeFn | null =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window ? tauriInvoke : null;

export function AccountSettings({ onClose }: AccountSettingsProps) {
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [savedName, setSavedName] = useState("");
  const [connections, setConnections] = useState<OAuthConnection[]>([]);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

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
                    {c.provider === "google_drive"
                      ? "Google Drive"
                      : c.provider === "google"
                        ? "Google"
                        : c.provider} — เชื่อมต่อแล้ว
                  </div>
                ))
            ) : (
              <p className="account-settings-placeholder">ไม่มีบัญชีที่เชื่อมต่อ</p>
            )}
          </div>
        </div>

        <div className="account-settings-section">
          <BackupPanel invoke={nativeInvoke} />
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
