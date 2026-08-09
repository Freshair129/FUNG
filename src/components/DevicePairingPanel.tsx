import { useCallback, useEffect, useRef, useState } from "react";
import { Link2, RefreshCw, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { supabase } from "../lib/supabase";
import { hashPairingCode } from "../lib/authFlow";
import "./DevicePairingPanel.css";

interface PairedDeviceRow {
  id: string;
  name: string;
  platform: string;
  fingerprint: string;
  paired_at: string;
  revoked_at: string | null;
  pairing_session_id: string;
}

type PairingState =
  | { kind: "idle" }
  | { kind: "waiting"; sessionId: string; code: string; expiresAt: number }
  | { kind: "confirmed"; peerName: string }
  | { kind: "error"; message: string };

const DEVICE_ID_KEY = "fung.device.id";
const POLL_MS = 2000;

function generateCode(): string {
  const buf = new Uint32Array(1);
  crypto.getRandomValues(buf);
  return String(buf[0] % 1_000_000).padStart(6, "0");
}

export function DevicePairingPanel() {
  const [paired, setPaired] = useState<PairedDeviceRow[]>([]);
  const [pairing, setPairing] = useState<PairingState>({ kind: "idle" });
  const [now, setNow] = useState(Date.now());
  const [creating, setCreating] = useState(false);
  const pollTimer = useRef<number | null>(null);

  const refreshLocal = useCallback(async () => {
    try {
      setPaired(await invoke<PairedDeviceRow[]>("paired_device_list"));
    } catch (e) {
      console.error("Failed to list paired devices:", e);
    }
  }, []);

  useEffect(() => {
    void refreshLocal();
  }, [refreshLocal]);

  // Revocation propagation: verify each local peer still exists in the cloud.
  useEffect(() => {
    void (async () => {
      const local = await invoke<PairedDeviceRow[]>("paired_device_list").catch(() => []);
      const active = local.filter((d) => !d.revoked_at);
      if (active.length === 0) return;
      const { data, error } = await supabase
        .from("devices")
        .select("id")
        .in("id", active.map((d) => d.id));
      if (error) { console.error("Revocation check failed:", error); return; }
      const alive = new Set((data ?? []).map((r) => r.id as string));
      for (const d of active) {
        if (!alive.has(d.id)) await invoke("paired_device_revoke", { id: d.id });
      }
      void refreshLocal();
    })();
  }, [refreshLocal]);

  // Countdown tick while waiting.
  useEffect(() => {
    if (pairing.kind !== "waiting") return;
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [pairing.kind]);

  const stopPolling = useCallback(() => {
    if (pollTimer.current) {
      window.clearInterval(pollTimer.current);
      pollTimer.current = null;
    }
  }, []);

  // Client-side self-expiry: stop polling once the countdown reaches zero.
  useEffect(() => {
    if (pairing.kind !== "waiting") return;
    if (Date.now() >= pairing.expiresAt) {
      stopPolling();
      setPairing({ kind: "error", message: "รหัสหมดอายุ — สร้างรหัสใหม่" });
    }
  }, [now, pairing, stopPolling]);

  const startPairing = useCallback(async () => {
    setCreating(true);
    const myDeviceId = localStorage.getItem(DEVICE_ID_KEY);
    const { data: sessionData } = await supabase.auth.getSession();
    const userId = sessionData.session?.user.id;
    if (!myDeviceId || !userId) {
      setPairing({ kind: "error", message: "ต้องเข้าสู่ระบบและลงทะเบียนอุปกรณ์ก่อน" });
      setCreating(false);
      return;
    }
    const sessionId = crypto.randomUUID();
    const code = generateCode();
    const codeHash = await hashPairingCode(sessionId, code);
    // Opportunistic cleanup of stale sessions (spec §6).
    await supabase
      .from("pairing_sessions")
      .delete()
      .lt("expires_at", new Date(Date.now() - 86_400_000).toISOString());
    const { error } = await supabase.from("pairing_sessions").insert({
      id: sessionId,
      user_id: userId,
      initiator_device_id: myDeviceId,
      code_hash: codeHash,
    });
    if (error) {
      setPairing({ kind: "error", message: `สร้างรหัสไม่สำเร็จ: ${error.message}` });
      setCreating(false);
      return;
    }
    await supabase.from("device_audit_events").insert({
      user_id: userId,
      device_id: myDeviceId,
      event_type: "pairing_session_created",
      metadata: { session_id: sessionId },
    });
    setPairing({ kind: "waiting", sessionId, code, expiresAt: Date.now() + 5 * 60_000 });
    pollTimer.current = window.setInterval(() => void poll(sessionId, userId), POLL_MS);
    setCreating(false);
  }, []);

  const poll = useCallback(
    async (sessionId: string, userId: string) => {
      const { data, error } = await supabase
        .from("pairing_sessions")
        .select("status, responder_device_id")
        .eq("id", sessionId)
        .single();
      if (error) { console.error("Pairing poll failed:", error); return; }
      if (data.status === "confirmed" && data.responder_device_id) {
        stopPolling();
        const { data: peer, error: peerError } = await supabase
          .from("devices")
          .select("id, device_label, platform, public_key_fingerprint")
          .eq("id", data.responder_device_id)
          .single();
        if (peerError || !peer) {
          console.error("Failed to load paired peer device:", peerError);
          setPairing({
            kind: "error",
            message: "จับคู่สำเร็จแต่โหลดข้อมูลอุปกรณ์ไม่ได้ — รีเฟรชรายการอีกครั้ง",
          });
          void refreshLocal();
        } else {
          await invoke("paired_device_upsert", {
            device: {
              id: peer.id,
              name: peer.device_label,
              platform: peer.platform,
              fingerprint: peer.public_key_fingerprint,
              pairing_session_id: sessionId,
            },
          });
          await supabase.from("device_audit_events").insert({
            user_id: userId,
            device_id: peer.id,
            event_type: "pairing_confirmed",
            metadata: { session_id: sessionId },
          });
          setPairing({ kind: "confirmed", peerName: peer.device_label });
          void refreshLocal();
        }
      } else if (data.status === "locked" || data.status === "expired") {
        stopPolling();
        setPairing({
          kind: "error",
          message: data.status === "locked" ? "ใส่รหัสผิดครบ 5 ครั้ง — สร้างรหัสใหม่" : "รหัสหมดอายุ — สร้างรหัสใหม่",
        });
      }
    },
    [refreshLocal, stopPolling],
  );

  useEffect(() => () => stopPolling(), [stopPolling]);

  const revoke = useCallback(
    async (row: PairedDeviceRow) => {
      const { data: sessionData } = await supabase.auth.getSession();
      const userId = sessionData.session?.user.id;
      const { error } = await supabase.from("devices").delete().eq("id", row.id);
      if (error) {
        console.error("Cloud revoke failed:", error);
        setPairing({ kind: "error", message: "ยกเลิกการจับคู่ไม่สำเร็จ ลองใหม่อีกครั้ง" });
        return;
      }
      await invoke("paired_device_revoke", { id: row.id });
      if (userId) {
        await supabase.from("device_audit_events").insert({
          user_id: userId,
          device_id: row.id,
          event_type: "device_revoked",
          metadata: { name: row.name },
        });
      }
      void refreshLocal();
    },
    [refreshLocal],
  );

  const remainingMs = pairing.kind === "waiting" ? Math.max(0, pairing.expiresAt - now) : 0;

  return (
    <section className="device-pairing-panel" aria-label="อุปกรณ์ที่จับคู่">
      <header className="device-pairing-header">
        <Link2 size={18} />
        <h3>อุปกรณ์ที่จับคู่</h3>
      </header>

      <ul className="device-pairing-list">
        {paired.length === 0 && <li className="device-pairing-empty">ยังไม่มีอุปกรณ์ที่จับคู่</li>}
        {paired.map((d) => (
          <li key={d.id} className={d.revoked_at ? "device-pairing-item device-pairing-item-revoked" : "device-pairing-item"}>
            <div>
              <strong>{d.name}</strong>
              <small>{d.platform} · {d.revoked_at ? "ถูกยกเลิกการจับคู่" : "จับคู่แล้ว"}</small>
            </div>
            {!d.revoked_at && (
              <button type="button" className="device-pairing-revoke" onClick={() => void revoke(d)} aria-label={`ยกเลิก ${d.name}`}>
                <Trash2 size={15} />
              </button>
            )}
          </li>
        ))}
      </ul>

      {pairing.kind === "waiting" ? (
        <div className="device-pairing-code-box">
          <p>ใส่รหัสนี้บนมือถือของคุณ</p>
          <strong className="device-pairing-code">{pairing.code}</strong>
          <p className="device-pairing-countdown">
            หมดอายุใน {Math.floor(remainingMs / 60000)}:{String(Math.floor((remainingMs % 60000) / 1000)).padStart(2, "0")} นาที
          </p>
          <p className="device-pairing-hint">รอการยืนยันจากมือถือ…</p>
        </div>
      ) : (
        <button
          type="button"
          className="device-pairing-start"
          onClick={() => void startPairing()}
          disabled={creating}
        >
          <RefreshCw size={15} /> {creating ? "กำลังสร้างรหัส…" : "จับคู่อุปกรณ์ใหม่"}
        </button>
      )}
      {pairing.kind === "confirmed" && (
        <p className="device-pairing-success">จับคู่กับ {pairing.peerName} สำเร็จ ✓</p>
      )}
      {pairing.kind === "error" && <p className="device-pairing-error">{pairing.message}</p>}
    </section>
  );
}
