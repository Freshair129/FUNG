import { useCallback, useEffect, useState } from "react";
import type { Session } from "@supabase/supabase-js";
import { LogIn, LogOut, MonitorSmartphone, X } from "lucide-react";
import { supabase } from "../lib/supabase";
import {
  beginLoopbackFallbackLogin,
  cancelGoogleLogin,
  listenForAuthCallback,
} from "../lib/authFlow";
import { invoke } from "@tauri-apps/api/core";
import "./AccountLoginPanel.css";

interface NativeEnrollmentProof {
  version: number;
  operation: string;
  userId: string;
  publicKey: string;
  fingerprint: string;
  fingerprintHex: string;
  platform: string;
  deviceLabel: string;
  issuedAtMs: number;
  expiresAtMs: number;
  nonce: string;
  signature: string;
}

type EnrollmentStatus = "idle" | "pending" | "approved" | "revoked" | "pairing_only";

interface AccountLoginPanelProps {
  onClose?: () => void;
}

const DEVICE_ID_KEY = "fung.device.id";

export function AccountLoginPanel({ onClose }: AccountLoginPanelProps) {
  const [session, setSession] = useState<Session | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deviceLabel, setDeviceLabel] = useState("FUNG Desktop");
  const [enrollmentStatus, setEnrollmentStatus] = useState<EnrollmentStatus>("idle");

  useEffect(() => {
    void supabase.auth.getSession().then(({ data }) => setSession(data.session ?? null));
    const { data: sub } = supabase.auth.onAuthStateChange((_e, s) => setSession(s));
    let cleanup: (() => void) | undefined;
    void listenForAuthCallback((err) => {
      setBusy(false);
      setError(err ? `เข้าสู่ระบบไม่สำเร็จ: ${err}` : null);
    }).then((fn) => { cleanup = fn; });
    return () => {
      sub.subscription.unsubscribe();
      cleanup?.();
      void cancelGoogleLogin().catch(() => undefined);
    };
  }, []);

  // Submit only a server-owned pending enrollment. A browser session never
  // inserts, updates, deletes, or audits an authoritative device row.
  useEffect(() => {
    if (!session || enrollmentStatus !== "idle") return;
    let cancelled = false;
    void (async () => {
      try {
        const native = await invoke<NativeEnrollmentProof>("device_enrollment_proof", {
          sessionProof: session.access_token,
          deviceLabel,
        });
        const { fingerprintHex, ...proof } = native;
        const { data: existing, error: selErr } = await supabase
          .from("devices")
          .select("id, authority_state, revoked_at")
          .eq("public_key_fingerprint", fingerprintHex)
          .maybeSingle();
        if (selErr) throw selErr;
        if (existing?.revoked_at || existing?.authority_state === "revoked") {
          if (!cancelled) setEnrollmentStatus("revoked");
          return;
        }
        if (existing?.authority_state === "drive_trusted") {
          if (!cancelled) {
            localStorage.setItem(DEVICE_ID_KEY, existing.id as string);
            setEnrollmentStatus("approved");
          }
          return;
        }
        if (existing?.authority_state === "pairing_only") {
          if (!cancelled) {
            localStorage.setItem(DEVICE_ID_KEY, existing.id as string);
            setEnrollmentStatus("pairing_only");
          }
          return;
        }

        const { data: enrollment, error: enrollmentError } = await supabase.functions.invoke("device-enrollment", {
          body: {
            action: "pending",
            nativeProof: proof,
          },
        });
        if (enrollmentError) throw enrollmentError;
        if (enrollment?.status !== "pending" || !enrollment.requestId) {
          throw new Error("enrollment_unavailable");
        }
        if (!cancelled) setEnrollmentStatus("pending");
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : "ลงทะเบียนอุปกรณ์ไม่สำเร็จ");
        }
      }
    })();
    return () => { cancelled = true; };
  }, [session, enrollmentStatus, deviceLabel]);

  const handleLogin = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await beginLoopbackFallbackLogin();
    } catch (e) {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleLogout = useCallback(async () => {
    await supabase.auth.signOut();
    setEnrollmentStatus("idle");
  }, []);

  return (
    <section className="account-login-panel" aria-label="บัญชี FUNG">
      <header className="account-login-header">
        <MonitorSmartphone size={18} />
        <h3>บัญชี FUNG</h3>
        {onClose && (
          <button type="button" className="account-login-close" onClick={onClose} aria-label="ปิด">
            <X size={16} />
          </button>
        )}
      </header>
      {session ? (
        <div className="account-login-signed-in">
          <p className="account-login-email">{session.user.email}</p>
          <label className="account-login-label">
            ชื่ออุปกรณ์นี้
            <input
              value={deviceLabel}
              onChange={(e) => setDeviceLabel(e.target.value)}
              maxLength={80}
            />
          </label>
          <p className="account-login-status">
            {enrollmentStatus === "pending" && "รอการอนุมัติอุปกรณ์จาก Boss…"}
            {enrollmentStatus === "approved" && "อุปกรณ์ได้รับอนุมัติ ✓"}
            {enrollmentStatus === "revoked" && "อุปกรณ์ถูกเพิกถอน — ต้องลงทะเบียนใหม่"}
            {enrollmentStatus === "pairing_only" && "อุปกรณ์อยู่ในโหมดจับคู่เท่านั้น"}
            {enrollmentStatus === "idle" && "กำลังเตรียมคำขอลงทะเบียน…"}
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
        </div>
      )}
      {error && <p className="account-login-error">{error}</p>}
    </section>
  );
}
