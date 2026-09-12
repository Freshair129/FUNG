import { useEffect, useState } from "react";
import { ChevronDown, Cloud, LogOut, Settings } from "lucide-react";
import { FungLogo } from "../components/FungLogo";
import { supabase } from "../lib/supabase";
import { AccountSettings } from "./AccountSettings";
import { formatRelativeThai, usePairedDevices } from "./usePairedDevices";
import { useLocalRecordings } from "./useLocalRecordings";
import "./Dashboard.css";

const CHANNEL_LABELS: Record<string, string> = {
  mic: "ไมค์",
  system: "เสียงระบบ",
  file: "ไฟล์",
};

function formatDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mmss = `${hours > 0 ? String(minutes).padStart(2, "0") : minutes}:${String(seconds).padStart(2, "0")}`;
  return hours > 0 ? `${hours}:${mmss}` : mmss;
}

function formatRecordedAt(iso: string | null): string {
  if (!iso) return "ไม่ทราบเวลา";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString("th-TH", { dateStyle: "medium", timeStyle: "short" });
}

export function Dashboard() {
  const [displayName, setDisplayName] = useState("");
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { devices, state: devicesState, actionError, reload: reloadDevices, revoke } = usePairedDevices();
  const local = useLocalRecordings();
  const [connectInput, setConnectInput] = useState("");
  const [connectError, setConnectError] = useState<string | null>(null);
  // Which channel each recording is playing; defaults to its first one.
  const [channelById, setChannelById] = useState<Record<string, string>>({});

  const handleConnect = () => {
    if (local.connect(connectInput)) {
      setConnectInput("");
      setConnectError(null);
    } else {
      setConnectError("ลิงก์ไม่ถูกต้อง — ต้องเป็น http://127.0.0.1:PORT/#TOKEN ที่คัดลอกจาก desktop เครื่องนี้");
    }
  };

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
          <div className="dashboard-tile dashboard-tile-recordings">
            <div className="dashboard-tile-icon">📁</div>
            <h3>ไฟล์ล่าสุด</h3>
            {local.state === "unconfigured" && (
              <div className="dashboard-connect">
                <p>
                  เล่นไฟล์ที่อัดไว้บน FUNG desktop <strong>เครื่องนี้</strong> — อ่านตรงจากเครื่อง ไม่มีอะไรขึ้น cloud
                </p>
                <p className="dashboard-connect-hint">
                  เปิดแอป desktop → Settings › Runtime → Start local API → คัดลอก “ลิงก์เชื่อมต่อเว็บ” มาวางที่นี่
                </p>
                <div className="dashboard-connect-row">
                  <input
                    className="dashboard-connect-input"
                    type="text"
                    placeholder="http://127.0.0.1:PORT/#TOKEN"
                    value={connectInput}
                    onChange={(e) => setConnectInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleConnect();
                    }}
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <button type="button" className="dashboard-connect-btn" onClick={handleConnect}>
                    เชื่อมต่อ
                  </button>
                </div>
                {connectError && <p className="dashboard-device-error">{connectError}</p>}
              </div>
            )}
            {local.state === "loading" && <p>กำลังโหลดรายการจาก desktop…</p>}
            {local.state === "error" && (
              <div>
                <p className="dashboard-device-error">{local.error}</p>
                <div className="dashboard-connect-actions">
                  <button type="button" className="dashboard-device-revoke" onClick={() => void local.reload()}>
                    ลองใหม่
                  </button>
                  <button type="button" className="dashboard-link-btn" onClick={local.disconnect}>
                    ตัดการเชื่อมต่อ
                  </button>
                </div>
              </div>
            )}
            {local.state === "ready" && local.recordings.length === 0 && <p>desktop ยังไม่มีไฟล์ที่อัดไว้</p>}
            {local.state === "ready" && local.recordings.length > 0 && (
              <ul className="dashboard-device-list">
                {local.recordings.map((recording) => {
                  const channel = channelById[recording.id] ?? recording.channels[0] ?? null;
                  const src = channel ? local.playbackUrl(recording.id, channel) : null;
                  return (
                    <li key={recording.id} className="dashboard-recording-item">
                      <div className="dashboard-device-info">
                        <strong>{recording.projectName ?? "(ไม่มีชื่อโปรเจกต์)"}</strong>
                        <small>
                          {formatRecordedAt(recording.createdAt)} · {formatDuration(recording.durationMs)}
                          {recording.status && recording.status !== "completed" ? ` · ${recording.status}` : ""}
                        </small>
                      </div>
                      {recording.channels.length > 1 && (
                        <div className="dashboard-channel-row" role="group" aria-label="ช่องเสียง">
                          {recording.channels.map((option) => (
                            <button
                              key={option}
                              type="button"
                              className={`dashboard-channel-btn${option === channel ? " is-active" : ""}`}
                              onClick={() => setChannelById((prev) => ({ ...prev, [recording.id]: option }))}
                            >
                              {CHANNEL_LABELS[option] ?? option}
                            </button>
                          ))}
                        </div>
                      )}
                      {src ? (
                        // Keyed on the channel so switching it reloads the element
                        // instead of continuing the old stream under a new src.
                        <audio key={`${recording.id}-${channel}`} className="dashboard-audio" controls preload="none" src={src} />
                      ) : (
                        <p className="dashboard-device-error">ไม่มีไฟล์เสียงในเครื่องสำหรับรายการนี้</p>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
            {local.connection && local.state !== "unconfigured" && (
              <p className="dashboard-connect-footer">
                เชื่อมต่อกับ {local.connection.baseUrl}
                <button type="button" className="dashboard-link-btn" onClick={local.disconnect}>
                  ตัดการเชื่อมต่อ
                </button>
              </p>
            )}
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
