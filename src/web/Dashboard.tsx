import { useCallback, useEffect, useState } from "react";
import { ChevronDown, Cloud, Download, FileText, LogOut, Mic, Settings, Square, Trash2 } from "lucide-react";
import { FungLogo } from "../components/FungLogo";
import { supabase } from "../lib/supabase";
import { AccountSettings } from "./AccountSettings";
import { formatRelativeThai, usePairedDevices } from "./usePairedDevices";
import { useLocalRecordings } from "./useLocalRecordings";
import { useDesktopTranscription } from "./useDesktopTranscription";
import { useWebRecorder } from "./useWebRecorder";
import {
  deleteWebRecording,
  formatBytes,
  formatDurationMs,
  listWebRecordings,
  webRecordingFileName,
  type DesktopTranscription,
  type StoredWebRecording,
} from "./webRecordings";
import "./Dashboard.css";

const CHANNEL_LABELS: Record<string, string> = {
  mic: "ไมค์",
  system: "เสียงระบบ",
  file: "ไฟล์",
};

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

  // Recordings made in this browser (IndexedDB), newest first, each with an
  // object URL for playback/download that is revoked when it leaves the list.
  const [webRecordings, setWebRecordings] = useState<StoredWebRecording[] | null>(null);
  const [webUrls, setWebUrls] = useState<Record<string, string>>({});
  const [webListError, setWebListError] = useState<string | null>(null);
  const onRecordingSaved = useCallback((recording: StoredWebRecording) => {
    setWebRecordings((current) => [recording, ...(current ?? [])]);
  }, []);
  const recorder = useWebRecorder(onRecordingSaved);

  // Hand-off to the desktop on this machine: remembered on the recording so
  // a reload finds the transcript again instead of uploading twice.
  const onDesktopLinked = useCallback((id: string, desktop: DesktopTranscription) => {
    setWebRecordings((current) =>
      (current ?? []).map((recording) => (recording.id === id ? { ...recording, desktop } : recording)),
    );
  }, []);
  const localReload = local.reload;
  const onDesktopCompleted = useCallback(() => {
    void localReload();
  }, [localReload]);
  const desktop = useDesktopTranscription(local.connection, webRecordings, onDesktopLinked, onDesktopCompleted);

  useEffect(() => {
    if (!recorder.supported) {
      setWebRecordings([]);
      return;
    }
    listWebRecordings()
      .then((rows) => setWebRecordings(rows))
      .catch((cause: unknown) => {
        setWebRecordings([]);
        setWebListError(cause instanceof Error ? cause.message : String(cause));
      });
  }, [recorder.supported]);

  useEffect(() => {
    if (!webRecordings) return;
    setWebUrls((current) => {
      const next: Record<string, string> = {};
      for (const recording of webRecordings) {
        next[recording.id] = current[recording.id] ?? URL.createObjectURL(recording.blob);
      }
      for (const [id, url] of Object.entries(current)) {
        if (!(id in next)) URL.revokeObjectURL(url);
      }
      return next;
    });
  }, [webRecordings]);

  useEffect(
    () => () => {
      setWebUrls((current) => {
        Object.values(current).forEach((url) => URL.revokeObjectURL(url));
        return {};
      });
    },
    [],
  );

  const handleDeleteWebRecording = async (id: string) => {
    try {
      await deleteWebRecording(id);
      setWebRecordings((current) => (current ?? []).filter((recording) => recording.id !== id));
    } catch (cause) {
      setWebListError(cause instanceof Error ? cause.message : String(cause));
    }
  };

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
          <div className="dashboard-tile dashboard-tile-recorder">
            <div className="dashboard-tile-icon">🎙️</div>
            <h3>เริ่มบันทึก</h3>
            <p>
              อัดเสียงจากไมค์ในเบราว์เซอร์นี้ ไฟล์เก็บไว้ <strong>ในเบราว์เซอร์นี้เท่านั้น</strong> ไม่ขึ้น cloud —
              ดาวน์โหลดแล้วนำเข้า FUNG desktop เพื่อถอดเสียงได้
            </p>
            {!recorder.supported && (
              <p className="dashboard-device-error">เบราว์เซอร์นี้อัดเสียงไม่ได้ (ต้องมี MediaRecorder และ IndexedDB)</p>
            )}
            {recorder.supported && (
              <div className="dashboard-rec-controls">
                {recorder.state === "recording" ? (
                  <>
                    <div className="dashboard-rec-live" aria-live="polite">
                      <span className="dashboard-rec-dot" aria-hidden="true" />
                      <span className="dashboard-rec-timer">{formatDurationMs(recorder.elapsedMs)}</span>
                      <div
                        className="dashboard-level"
                        role="meter"
                        aria-label="ระดับเสียง"
                        aria-valuemin={0}
                        aria-valuemax={100}
                        aria-valuenow={Math.round(recorder.level * 100)}
                      >
                        <div className="dashboard-level-fill" style={{ width: `${Math.round(recorder.level * 100)}%` }} />
                      </div>
                    </div>
                    <button type="button" className="dashboard-rec-btn is-stop" onClick={() => void recorder.stop()}>
                      <Square size={14} /> หยุดและบันทึก
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    className="dashboard-rec-btn"
                    onClick={() => void recorder.start()}
                    disabled={recorder.state !== "idle"}
                  >
                    <Mic size={14} />
                    {recorder.state === "requesting"
                      ? "กำลังขอใช้ไมค์…"
                      : recorder.state === "saving"
                        ? "กำลังบันทึก…"
                        : "เริ่มอัด"}
                  </button>
                )}
              </div>
            )}
            {recorder.error && <p className="dashboard-device-error">{recorder.error}</p>}
            {webListError && <p className="dashboard-device-error">{webListError}</p>}
            {webRecordings && webRecordings.length > 0 && (
              <ul className="dashboard-device-list dashboard-web-list">
                {webRecordings.map((recording) => (
                  <li key={recording.id} className="dashboard-recording-item">
                    <div className="dashboard-rec-row">
                      <div className="dashboard-device-info">
                        <strong>{formatRecordedAt(recording.createdAt)}</strong>
                        <small>
                          {formatDurationMs(recording.durationMs)} · {formatBytes(recording.bytes)}
                        </small>
                      </div>
                      <div className="dashboard-rec-actions">
                        {!recording.desktop && (
                          <button
                            type="button"
                            className="dashboard-rec-action is-primary"
                            disabled={local.state !== "ready" || desktop.statuses[recording.id]?.phase === "uploading"}
                            title={
                              local.state === "ready"
                                ? "ส่งไฟล์ไป FUNG desktop บนเครื่องนี้เพื่อถอดเสียง (ไม่ผ่าน cloud)"
                                : "เชื่อมต่อ FUNG desktop ในช่อง “ไฟล์ล่าสุด” ด้านล่างก่อน"
                            }
                            onClick={() => void desktop.send(recording)}
                          >
                            <FileText size={13} />
                            {desktop.statuses[recording.id]?.phase === "uploading" ? "กำลังส่ง…" : "ถอดเสียงที่ desktop"}
                          </button>
                        )}
                        <a
                          className="dashboard-rec-action"
                          href={webUrls[recording.id]}
                          download={webRecordingFileName(recording)}
                        >
                          <Download size={13} /> ดาวน์โหลด
                        </a>
                        <button
                          type="button"
                          className="dashboard-rec-action is-danger"
                          onClick={() => void handleDeleteWebRecording(recording.id)}
                        >
                          <Trash2 size={13} /> ลบ
                        </button>
                      </div>
                    </div>
                    {webUrls[recording.id] && (
                      <audio className="dashboard-audio" controls preload="metadata" src={webUrls[recording.id]} />
                    )}
                    {(() => {
                      const status = desktop.statuses[recording.id];
                      if (!recording.desktop && !status) return null;
                      if (status?.phase === "failed") {
                        return (
                          <p className="dashboard-device-error">
                            ถอดเสียงที่ desktop ไม่สำเร็จ — {status.error}
                            {recording.desktop && (
                              <button
                                type="button"
                                className="dashboard-link-btn"
                                onClick={() => void desktop.send(recording)}
                              >
                                ส่งใหม่
                              </button>
                            )}
                          </p>
                        );
                      }
                      if (status?.phase === "completed" && status.segments) {
                        return (
                          <details className="dashboard-transcript" open>
                            <summary>
                              ถอดเสียงแล้ว · {status.segments.length} ช่วง · อยู่ในโปรเจกต์ของ desktop แล้ว
                            </summary>
                            {status.segments.length === 0 ? (
                              <p className="dashboard-connect-hint">desktop ไม่พบคำพูดในไฟล์นี้</p>
                            ) : (
                              <ol className="dashboard-transcript-list">
                                {status.segments.map((segment) => (
                                  <li key={segment.id}>
                                    <span className="dashboard-transcript-time">{formatDurationMs(segment.startMs)}</span>
                                    {segment.speakerName && (
                                      <span className="dashboard-transcript-speaker">{segment.speakerName}</span>
                                    )}
                                    <span>{segment.text}</span>
                                  </li>
                                ))}
                              </ol>
                            )}
                          </details>
                        );
                      }
                      const progress = status?.progress ?? 0;
                      return (
                        <p className="dashboard-connect-hint dashboard-transcript-progress" aria-live="polite">
                          {status?.phase === "uploading"
                            ? "กำลังส่งไฟล์ไป desktop…"
                            : local.state === "ready"
                              ? `desktop กำลังถอดเสียง… ${progress}%`
                              : "ส่งไป desktop แล้ว — เชื่อมต่อ desktop อีกครั้งเพื่อดู transcript"}
                        </p>
                      );
                    })()}
                  </li>
                ))}
              </ul>
            )}
            {webRecordings && webRecordings.length === 0 && recorder.supported && recorder.state === "idle" && (
              <p className="dashboard-connect-hint">ยังไม่มีไฟล์ที่อัดในเบราว์เซอร์นี้</p>
            )}
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
                          {formatRecordedAt(recording.createdAt)} · {formatDurationMs(recording.durationMs)}
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
