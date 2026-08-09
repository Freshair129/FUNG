import { useCallback, useEffect, useRef, useState } from "react";
import type { Session } from "@supabase/supabase-js";
import { LogIn, LogOut, MonitorSmartphone, RefreshCw } from "lucide-react";
import { supabase } from "../lib/supabase";
import {
  beginGoogleLogin,
  beginLoopbackFallbackLogin,
  listenForAuthCallback,
} from "../lib/authFlow";
import { invoke } from "@tauri-apps/api/core";
import "./AccountLoginPanel.css";

interface DeviceIdentity {
  fingerprint: string;
  created: boolean;
}

const DEVICE_ID_KEY = "fung.device.id";
const FALLBACK_DELAY_MS = 120_000;

export function AccountLoginPanel() {
  const [session, setSession] = useState<Session | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showFallback, setShowFallback] = useState(false);
  const [deviceLabel, setDeviceLabel] = useState("FUNG Desktop");
  const [registered, setRegistered] = useState(false);
  const fallbackTimer = useRef<number | null>(null);

  useEffect(() => {
    void supabase.auth.getSession().then(({ data }) => setSession(data.session ?? null));
    const { data: sub } = supabase.auth.onAuthStateChange((_e, s) => setSession(s));
    let cleanup: (() => void) | undefined;
    void listenForAuthCallback((err) => {
      setBusy(false);
      setShowFallback(false);
      if (fallbackTimer.current) window.clearTimeout(fallbackTimer.current);
      setError(err ? `เข้าสู่ระบบไม่สำเร็จ: ${err}` : null);
    }).then((fn) => { cleanup = fn; });
    return () => {
      sub.subscription.unsubscribe();
      cleanup?.();
      if (fallbackTimer.current) window.clearTimeout(fallbackTimer.current);
    };
  }, []);

  // Register this device once per session.
  useEffect(() => {
    if (!session || registered) return;
    let cancelled = false;
    void (async () => {
      try {
        const identity = await invoke<DeviceIdentity>("device_identity_ensure");
        const { data: existing, error: selErr } = await supabase
          .from("devices")
          .select("id, device_label")
          .eq("public_key_fingerprint", identity.fingerprint)
          .maybeSingle();
        if (selErr) throw selErr;
        let deviceId = existing?.id as string | undefined;
        if (!deviceId) {
          const { data: inserted, error: insErr } = await supabase
            .from("devices")
            .insert({
              user_id: session.user.id,
              device_label: deviceLabel,
              platform: "windows",
              public_key_fingerprint: identity.fingerprint,
            })
            .select("id")
            .single();
          if (insErr) throw insErr;
          deviceId = inserted.id as string;
          await supabase.from("device_audit_events").insert({
            user_id: session.user.id,
            device_id: deviceId,
            event_type: "device_registered",
            metadata: { platform: "windows" },
          });
        } else {
          await supabase
            .from("devices")
            .update({ last_seen_at: new Date().toISOString() })
            .eq("id", deviceId);
        }
        if (!cancelled && deviceId) {
          localStorage.setItem(DEVICE_ID_KEY, deviceId);
          setRegistered(true);
        }
      } catch (e) {
        if (!cancelled) {
          console.error("Device registration failed:", e);
          setError("ลงทะเบียนอุปกรณ์ไม่สำเร็จ");
        }
      }
    })();
    return () => { cancelled = true; };
  }, [session, registered, deviceLabel]);

  const handleLogin = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await beginGoogleLogin();
      fallbackTimer.current = window.setTimeout(() => setShowFallback(true), FALLBACK_DELAY_MS);
    } catch (e) {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleFallback = useCallback(async () => {
    setError(null);
    try {
      await beginLoopbackFallbackLogin();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleLogout = useCallback(async () => {
    await supabase.auth.signOut();
    setRegistered(false);
  }, []);

  return (
    <section className="account-login-panel" aria-label="บัญชี FUNG">
      <header className="account-login-header">
        <MonitorSmartphone size={18} />
        <h3>บัญชี FUNG</h3>
      </header>
      {session ? (
        <div className="account-login-signed-in">
          <p className="account-login-email">{session.user.email}</p>
          <label className="account-login-label">
            ชื่ออุปกรณ์นี้
            <input
              value={deviceLabel}
              onChange={(e) => setDeviceLabel(e.target.value)}
              maxLength={120}
            />
          </label>
          <p className="account-login-status">
            {registered ? "อุปกรณ์ลงทะเบียนแล้ว ✓" : "กำลังลงทะเบียนอุปกรณ์…"}
          </p>
          <button type="button" className="account-login-btn" onClick={handleLogout}>
            <LogOut size={15} /> ออกจากระบบ
          </button>
        </div>
      ) : (
        <div className="account-login-signed-out">
          <button type="button" className="account-login-btn" onClick={handleLogin} disabled={busy}>
            <LogIn size={15} /> {busy ? "รอการยืนยันในเบราว์เซอร์…" : "เข้าสู่ระบบด้วย Google"}
          </button>
          {showFallback && (
            <button type="button" className="account-login-btn account-login-btn-secondary" onClick={handleFallback}>
              <RefreshCw size={15} /> ลองวิธีสำรอง (loopback)
            </button>
          )}
        </div>
      )}
      {error && <p className="account-login-error">{error}</p>}
    </section>
  );
}
