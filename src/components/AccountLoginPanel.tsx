import { useCallback, useEffect, useState } from "react";
import { LogIn, LogOut, MonitorSmartphone, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import {
  brokerEnrollmentRequest,
  brokerSessionLoginBegin,
  brokerSessionLogout,
  brokerSessionStatus,
  type BrokerInvoke,
  type SessionStatus,
} from "../lib/desktopSessionBroker";
import "./AccountLoginPanel.css";

interface AccountLoginPanelProps { onClose?: () => void; }
type EnrollmentStatus = "idle" | "pending";
const brokerInvoke: BrokerInvoke = <T,>(operation: Parameters<BrokerInvoke>[0], args?: Record<string, unknown>) => invoke<T>(operation, args);

export function AccountLoginPanel({ onClose }: AccountLoginPanelProps) {
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deviceLabel, setDeviceLabel] = useState("FUNG Desktop");
  const [enrollmentStatus, setEnrollmentStatus] = useState<EnrollmentStatus>("idle");

  const refresh = useCallback(async () => {
    try { setStatus(await brokerSessionStatus(brokerInvoke)); }
    catch (e) { setError(e instanceof Error ? e.message : "auth_unavailable"); }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    if (status?.state !== "login_pending" && status?.state !== "refreshing") return;
    const timer = window.setInterval(() => { void refresh(); }, 500);
    return () => window.clearInterval(timer);
  }, [refresh, status?.state]);

  const handleLogin = useCallback(async () => {
    setBusy(true); setError(null);
    try { await brokerSessionLoginBegin(brokerInvoke); await refresh(); }
    catch (e) { setError(e instanceof Error ? e.message : "auth_start_failed"); }
    finally { setBusy(false); }
  }, [refresh]);

  const handleLogout = useCallback(async () => {
    setBusy(true); setError(null);
    try { await brokerSessionLogout(brokerInvoke); setEnrollmentStatus("idle"); await refresh(); }
    catch (e) { setError(e instanceof Error ? e.message : "auth_logout_incomplete"); }
    finally { setBusy(false); }
  }, [refresh]);

  useEffect(() => {
    if (status?.state !== "authenticated" || enrollmentStatus !== "idle") return;
    let cancelled = false;
    void brokerEnrollmentRequest(brokerInvoke, deviceLabel)
      .then(() => { if (!cancelled) setEnrollmentStatus("pending"); })
      .catch((e) => { if (!cancelled && e instanceof Error && e.message !== "auth_required") setError(e.message); });
    return () => { cancelled = true; };
  }, [deviceLabel, enrollmentStatus, status?.state]);

  return (
    <section className="account-login-panel" aria-label="บัญชี FUNG">
      <header className="account-login-header"><MonitorSmartphone size={18} /><h3>บัญชี FUNG</h3>{onClose && <button type="button" className="account-login-close" onClick={onClose} aria-label="ปิด"><X size={16} /></button>}</header>
      {status?.state === "authenticated" ? (
        <div className="account-login-signed-in"><p className="account-login-email">{status.email ?? "ลงชื่อเข้าใช้แล้ว"}</p><label className="account-login-label">ชื่ออุปกรณ์นี้<input value={deviceLabel} onChange={(e) => setDeviceLabel(e.target.value)} maxLength={80} /></label><p className="account-login-status">{enrollmentStatus === "pending" ? "รอการอนุมัติอุปกรณ์จาก Boss…" : "กำลังเตรียมคำขอลงทะเบียน…"}</p><button type="button" className="account-login-btn" onClick={() => void handleLogout()} disabled={busy}><LogOut size={15} /> ออกจากระบบ</button></div>
      ) : (
        <div className="account-login-signed-out"><button type="button" className="account-login-btn" onClick={() => void handleLogin()} disabled={busy}><LogIn size={15} /> {busy ? "กำลังเปิดการเข้าสู่ระบบ…" : "เข้าสู่ระบบด้วย Google"}</button></div>
      )}
      {error && <p className="account-login-error">{error}</p>}
    </section>
  );
}
