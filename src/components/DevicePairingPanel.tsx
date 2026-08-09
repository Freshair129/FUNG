import { useCallback, useEffect, useRef, useState } from "react";
import { Link2, RefreshCw, Trash2, X } from "lucide-react";
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
// Republish the LAN endpoint at this cadence while FUNGWIRE is enabled, so a
// DHCP-reassigned IP (or a server rebind after this desktop restarts) never
// leaves mobile with a stale `devices.lan_endpoint` for longer than this.
const FUNGWIRE_PUBLISH_MS = 60_000;

interface FungwireStatus {
  enabled: boolean;
  bind: string | null;
  activeJobs: number;
  connectedPeers: number;
}

const FUNGWIRE_STATUS_IDLE: FungwireStatus = { enabled: false, bind: null, activeJobs: 0, connectedPeers: 0 };

interface DevicePairingPanelProps {
  onClose?: () => void;
}

function generateCode(): string {
  const buf = new Uint32Array(1);
  crypto.getRandomValues(buf);
  return String(buf[0] % 1_000_000).padStart(6, "0");
}

export function DevicePairingPanel({ onClose }: DevicePairingPanelProps) {
  const [paired, setPaired] = useState<PairedDeviceRow[]>([]);
  const [pairing, setPairing] = useState<PairingState>({ kind: "idle" });
  const [now, setNow] = useState(Date.now());
  const [creating, setCreating] = useState(false);
  const pollTimer = useRef<number | null>(null);

  const [fungwireStatus, setFungwireStatus] = useState<FungwireStatus>(FUNGWIRE_STATUS_IDLE);
  const [fungwireBusy, setFungwireBusy] = useState(false);
  const [fungwireError, setFungwireError] = useState<string | null>(null);
  const fungwirePublishTimer = useRef<number | null>(null);

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

  // Reads this desktop's currently-bound LAN endpoint (Task 9's
  // `fungwire_local_endpoint`) and writes it into this device's own
  // Supabase `devices` row so a paired mobile can resolve it later — this
  // session only ever updates its own row (found via DEVICE_ID_KEY), never
  // another device's. A missing endpoint (server not actually bound, or no
  // routable LAN IP) is a silent no-op rather than an error.
  const publishFungwireEndpoint = useCallback(async () => {
    const myDeviceId = localStorage.getItem(DEVICE_ID_KEY);
    if (!myDeviceId) return;
    try {
      const endpoint = await invoke<string | null>("fungwire_local_endpoint");
      if (!endpoint) return;
      const { error } = await supabase
        .from("devices")
        .update({ lan_endpoint: endpoint, lan_endpoint_updated_at: new Date().toISOString() })
        .eq("id", myDeviceId);
      if (error) console.error("Failed to publish FUNGWIRE endpoint:", error);
    } catch (e) {
      console.error("Failed to read FUNGWIRE local endpoint:", e);
    }
  }, []);

  const stopFungwirePublishing = useCallback(() => {
    if (fungwirePublishTimer.current) {
      window.clearInterval(fungwirePublishTimer.current);
      fungwirePublishTimer.current = null;
    }
  }, []);

  const startFungwirePublishing = useCallback(() => {
    stopFungwirePublishing();
    void publishFungwireEndpoint();
    fungwirePublishTimer.current = window.setInterval(() => void publishFungwireEndpoint(), FUNGWIRE_PUBLISH_MS);
  }, [publishFungwireEndpoint, stopFungwirePublishing]);

  // Pick up an already-running server from an earlier mount of this panel
  // (the Tauri-side server lives for the app's process lifetime, not this
  // component's) and resume endpoint publishing if so. Cleans up the
  // publish interval on unmount either way.
  useEffect(() => {
    void invoke<FungwireStatus>("fungwire_status")
      .then((status) => {
        setFungwireStatus(status);
        if (status.enabled) startFungwirePublishing();
      })
      .catch((e) => console.error("Failed to read FUNGWIRE status:", e));
    return () => stopFungwirePublishing();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only probe of server state that already exists independently of this component
  }, []);

  const toggleFungwire = useCallback(async () => {
    setFungwireBusy(true);
    setFungwireError(null);
    try {
      const status = await invoke<FungwireStatus>("fungwire_server_set_enabled", { enabled: !fungwireStatus.enabled });
      setFungwireStatus(status);
      if (status.enabled) startFungwirePublishing();
      else stopFungwirePublishing();
    } catch (e) {
      console.error("Failed to toggle FUNGWIRE server:", e);
      setFungwireError("เปิด/ปิดการเชื่อมต่อไม่สำเร็จ");
    } finally {
      setFungwireBusy(false);
    }
  }, [fungwireStatus.enabled, startFungwirePublishing, stopFungwirePublishing]);

  // Revocation propagation: verify each local peer still exists in the cloud.
  // Guarded on an active session so a logged-out or different-account state
  // can never wipe the local paired-device list (I4).
  useEffect(() => {
    void (async () => {
      const { data: sessionData } = await supabase.auth.getSession();
      if (!sessionData.session) return;
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

  // I5: reconcile against sessions this device initiated and confirmed on the
  // server but whose responder never made it into the local paired-device
  // list (e.g. the panel was closed mid-handshake, before the poll picked up
  // the confirmation). Runs once on mount; only proceeds once this device's
  // id is known.
  useEffect(() => {
    const myDeviceId = localStorage.getItem(DEVICE_ID_KEY);
    if (!myDeviceId) return;
    let cancelled = false;
    void (async () => {
      const { data, error } = await supabase
        .from("pairing_sessions")
        .select("id, responder_device_id")
        .eq("initiator_device_id", myDeviceId)
        .eq("status", "confirmed");
      if (cancelled) return;
      if (error) { console.error("Pairing reconciliation query failed:", error); return; }
      const local = await invoke<PairedDeviceRow[]>("paired_device_list").catch(() => []);
      if (cancelled) return;
      const known = new Set(local.map((d) => d.id));
      for (const row of data ?? []) {
        if (!row.responder_device_id || known.has(row.responder_device_id)) continue;
        const { data: peer, error: peerError } = await supabase
          .from("devices")
          .select("id, device_label, platform, public_key_fingerprint")
          .eq("id", row.responder_device_id)
          .single();
        if (cancelled) return;
        if (peerError || !peer) {
          console.error("Reconciliation: failed to load paired peer device:", peerError);
          continue;
        }
        await invoke("paired_device_upsert", {
          device: {
            id: peer.id,
            name: peer.device_label,
            platform: peer.platform,
            fingerprint: peer.public_key_fingerprint,
            pairing_session_id: row.id,
          },
        });
      }
      if (!cancelled) void refreshLocal();
    })();
    return () => { cancelled = true; };
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
    // Session creation (and the opportunistic stale-session sweep, spec §6)
    // now goes through the security-definer create_pairing_session RPC —
    // clients no longer have direct insert/delete on pairing_sessions.
    // user_id is set server-side from auth.uid(), not sent here.
    const { error } = await supabase.rpc("create_pairing_session", {
      p_session_id: sessionId,
      p_code_hash: codeHash,
      p_initiator_device_id: myDeviceId,
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
        {onClose && (
          <button type="button" className="device-pairing-close" onClick={onClose} aria-label="ปิด">
            <X size={16} />
          </button>
        )}
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

      <section className="device-pairing-fungwire" aria-label="การเชื่อมต่อ FUNGWIRE">
        <div className="device-pairing-fungwire-header">
          <div>
            <strong>การเชื่อมต่อ FUNGWIRE</strong>
            <small>เปิดให้มือถือส่งงานมาประมวลผล</small>
          </div>
          <button
            type="button"
            className={`device-pairing-fungwire-switch ${fungwireStatus.enabled ? "is-on" : ""}`}
            role="switch"
            aria-checked={fungwireStatus.enabled}
            aria-label="เปิด/ปิด FUNGWIRE"
            onClick={() => void toggleFungwire()}
            disabled={fungwireBusy}
          >
            <i />
          </button>
        </div>
        {fungwireStatus.enabled && (
          <dl className="device-pairing-fungwire-status">
            <div><dt>ที่อยู่ในเครือข่าย</dt><dd>{fungwireStatus.bind ?? "ไม่ทราบ"}</dd></div>
            <div><dt>งานที่กำลังทำ</dt><dd>{fungwireStatus.activeJobs}</dd></div>
            <div><dt>อุปกรณ์ที่เชื่อมต่อ</dt><dd>{fungwireStatus.connectedPeers}</dd></div>
          </dl>
        )}
        {fungwireError && <p className="device-pairing-error">{fungwireError}</p>}
      </section>

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
