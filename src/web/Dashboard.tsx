import { useEffect, useState } from "react";
import { ChevronDown, Cloud, LogOut, Settings } from "lucide-react";
import { FungLogo } from "../components/FungLogo";
import { supabase } from "../lib/supabase";
import { AccountSettings } from "./AccountSettings";
import { formatRelativeThai, usePairedDevices } from "./usePairedDevices";
import "./Dashboard.css";

export function Dashboard() {
  const [displayName, setDisplayName] = useState("");
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { devices, state: devicesState, actionError, reload: reloadDevices, revoke } = usePairedDevices();

  const load = async () => {
    const {
      data: { user },
    } = await supabase.auth.getUser();
    if (!user) return;

    setAvatarUrl(user.user_metadata?.avatar_url ?? null);

    const { data: profile, error: profileError } = await supabase
      .from("profiles")
      .select("display_name")
      .eq("id", user.id)
      .single();

    if (profileError) console.error("Failed to load profile:", profileError);

    if (profile) {
      setDisplayName(profile.display_name ?? "User");
    }
  };

  useEffect(() => {
    void load();
  }, []);

  // Settings can revoke a device too, so re-read both on close.
  useEffect(() => {
    if (!settingsOpen) {
      void load();
      void reloadDevices();
    }
  }, [settingsOpen, reloadDevices]);

  const handleSignOut = async () => {
    await supabase.auth.signOut();
    window.location.href = "/";
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    if (!dropdownOpen) return;
    const close = () => setDropdownOpen(false);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [dropdownOpen]);

  return (
    <div className="dashboard">
      <header className="dashboard-topbar">
        <div className="dashboard-topbar-left">
          <FungLogo size={28} />
          <span className="web-badge">
            <Cloud size={12} /> Web
          </span>
        </div>

        <div className="dashboard-topbar-right">
          <div
            className="dashboard-avatar-btn"
            role="button"
            tabIndex={0}
            onClick={(e) => {
              e.stopPropagation();
              setDropdownOpen((prev) => !prev);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                e.stopPropagation();
                setDropdownOpen((prev) => !prev);
              }
            }}
          >
            {avatarUrl ? (
              <img src={avatarUrl} alt="" referrerPolicy="no-referrer" />
            ) : (
              <span className="avatar-fallback">
                {displayName.charAt(0).toUpperCase() || "U"}
              </span>
            )}
            <ChevronDown size={14} />

            {dropdownOpen && (
              <div className="dashboard-avatar-dropdown">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDropdownOpen(false);
                    setSettingsOpen(true);
                  }}
                >
                  <Settings size={15} /> ตั้งค่าบัญชี
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void handleSignOut();
                  }}
                >
                  <LogOut size={15} /> ออกจากระบบ
                </button>
              </div>
            )}
          </div>
        </div>
      </header>

      <main className="dashboard-main">
        <div className="dashboard-welcome">
          <h1>ยินดีต้อนรับสู่ FUNG Web</h1>
          <p>{displayName}</p>
        </div>

        <div className="dashboard-tiles">
          <div className="dashboard-tile">
            <div className="dashboard-tile-icon">🎙️</div>
            <h3>เริ่มบันทึก</h3>
            <p>เร็วๆ นี้</p>
          </div>
          <div className="dashboard-tile">
            <div className="dashboard-tile-icon">📁</div>
            <h3>ไฟล์ล่าสุด</h3>
            <p>เร็วๆ นี้</p>
          </div>
          <div className="dashboard-tile dashboard-tile-devices">
            <div className="dashboard-tile-icon">📱</div>
            <h3>อุปกรณ์ที่จับคู่</h3>
            {devicesState === "loading" && <p>กำลังโหลดรายการอุปกรณ์…</p>}
            {devicesState === "error" && (
              <p className="dashboard-device-error">
                โหลดรายการอุปกรณ์ไม่สำเร็จ — ไม่ใช่ว่าไม่มีอุปกรณ์
                <button type="button" className="dashboard-device-revoke" onClick={() => void reloadDevices()}>
                  ลองใหม่
                </button>
              </p>
            )}
            {devicesState === "ready" && devices.length === 0 && <p>ยังไม่มีอุปกรณ์ที่จับคู่</p>}
            {devicesState === "ready" && devices.length > 0 && (
              <ul className="dashboard-device-list">
                {devices.map((d) => (
                  <li key={d.id} className="dashboard-device-item">
                    <div className="dashboard-device-info">
                      <strong>{d.device_label}</strong>
                      <small>
                        {d.platform} · {formatRelativeThai(d.last_seen_at)}
                      </small>
                    </div>
                    <button
                      type="button"
                      className="dashboard-device-revoke"
                      onClick={() => void revoke(d.id, "dashboard")}
                    >
                      ยกเลิก
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {actionError && <p className="dashboard-device-error">{actionError}</p>}
          </div>
        </div>
      </main>

      {settingsOpen && <AccountSettings onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}
