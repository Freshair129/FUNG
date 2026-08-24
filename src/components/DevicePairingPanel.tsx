import { useCallback, useEffect, useRef, useState } from "react";
import { Link2, RefreshCw, Trash2, X } from "lucide-react";
import {
  brokerDeviceEndpointPublish,
  brokerDeviceList,
  brokerDeviceRevoke,
  brokerFungwireSetEnabled,
  brokerFungwireStatus,
  brokerPairingCreate,
  brokerPairingPoll,
} from "../lib/desktopSessionBroker";
import "./DevicePairingPanel.css";

interface PairedDeviceRow {
  id: string;
  label: string;
  platform: string;
  authorityState: string;
  pairedAt: string | null;
  revokedAt: string | null;
  endpointState: string | null;
}

type PairingState =
  | { kind: "idle" }
  | { kind: "waiting"; pairingId: string; displayCode: string; expiresAtMs: number }
  | { kind: "confirmed"; peerName: string }
  | { kind: "error"; message: string };

interface FungwireStatus {
  enabled: boolean;
  bind: string | null;
  activeJobs: number;
  connectedPeers: number;
}

interface DevicePairingPanelProps { onClose?: () => void }

const POLL_MS = 2000;
const PUBLISH_MS = 60_000;
const EMPTY_FUNGWIRE: FungwireStatus = { enabled: false, bind: null, activeJobs: 0, connectedPeers: 0 };

export function DevicePairingPanel({ onClose }: DevicePairingPanelProps) {
  const [devices, setDevices] = useState<PairedDeviceRow[]>([]);
  const [pairing, setPairing] = useState<PairingState>({ kind: "idle" });
  const [now, setNow] = useState(() => Date.now());
  const [busy, setBusy] = useState(false);
  const [fungwire, setFungwire] = useState<FungwireStatus>(EMPTY_FUNGWIRE);
  const [fungwireError, setFungwireError] = useState<string | null>(null);
  const pollTimer = useRef<number | null>(null);
  const publishTimer = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try { setDevices(await brokerDeviceList()); }
    catch { setDevices([]); }
  }, []);

  const refreshFungwire = useCallback(async () => {
    try { setFungwire(await brokerFungwireStatus()); }
    catch { setFungwire(EMPTY_FUNGWIRE); }
  }, []);

  useEffect(() => { void refresh(); void refreshFungwire(); }, [refresh, refreshFungwire]);

  const stopPolling = useCallback(() => {
    if (pollTimer.current !== null) window.clearInterval(pollTimer.current);
    pollTimer.current = null;
  }, []);

  const stopPublishing = useCallback(() => {
    if (publishTimer.current !== null) window.clearInterval(publishTimer.current);
    publishTimer.current = null;
  }, []);

  const publishEndpoint = useCallback(async () => {
    try { await brokerDeviceEndpointPublish(); }
    catch { /* Endpoint publication is retried while the native server is enabled. */ }
  }, []);

  const startPublishing = useCallback(() => {
    stopPublishing();
    void publishEndpoint();
    publishTimer.current = window.setInterval(() => void publishEndpoint(), PUBLISH_MS);
  }, [publishEndpoint, stopPublishing]);

  useEffect(() => () => { stopPolling(); stopPublishing(); }, [stopPolling, stopPublishing]);

  const pollPairing = useCallback((pairingId: string) => {
    stopPolling();
    pollTimer.current = window.setInterval(() => {
      void brokerPairingPoll(pairingId)
        .then((result) => {
          if (result.status === "confirmed") {
            stopPolling();
            setPairing({ kind: "confirmed", peerName: result.peer?.label || "อุปกรณ์ใหม่" });
            void refresh();
          } else if (result.status === "expired" || result.status === "cancelled" || result.status === "locked") {
            stopPolling();
            setPairing({ kind: "error", message: "รหัสจับคู่หมดอายุหรือถูกยกเลิก" });
          }
        })
        .catch(() => { stopPolling(); setPairing({ kind: "error", message: "ตรวจสอบการจับคู่ไม่สำเร็จ" }); });
    }, POLL_MS);
  }, [refresh, stopPolling]);

  useEffect(() => {
    if (pairing.kind !== "waiting") return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    if (now >= pairing.expiresAtMs) {
      stopPolling();
      setPairing({ kind: "error", message: "รหัสหมดอายุ — สร้างรหัสใหม่" });
    }
    return () => window.clearInterval(timer);
  }, [now, pairing, stopPolling]);

  const startPairing = useCallback(async () => {
    setBusy(true);
    try {
      const result = await brokerPairingCreate("FUNG Desktop");
      setPairing({ kind: "waiting", pairingId: result.pairingId, displayCode: result.displayCode, expiresAtMs: result.expiresAtMs });
      pollPairing(result.pairingId);
    } catch { setPairing({ kind: "error", message: "สร้างรหัสจับคู่ไม่สำเร็จ" }); }
    finally { setBusy(false); }
  }, [pollPairing]);

  const revoke = useCallback(async (deviceId: string) => {
    try { await brokerDeviceRevoke(deviceId); await refresh(); }
    catch { /* The native broker returns a redacted error; keep the list unchanged. */ }
  }, [refresh]);

  const toggleFungwire = useCallback(async () => {
    setBusy(true); setFungwireError(null);
    try {
      const next = await brokerFungwireSetEnabled(!fungwire.enabled);
      setFungwire(next);
      if (next.enabled) startPublishing(); else stopPublishing();
    } catch { setFungwireError("เปิด/ปิดการเชื่อมต่อไม่สำเร็จ"); }
    finally { setBusy(false); }
  }, [fungwire.enabled, startPublishing, stopPublishing]);

  return (
    <section className="device-pairing-panel" aria-label="Device pairing">
      <header className="device-pairing-header">
        <Link2 size={20} aria-hidden="true" />
        <h3>เชื่อมต่ออุปกรณ์</h3>
        {onClose && <button className="device-pairing-close" onClick={onClose} aria-label="ปิด"><X size={18} /></button>}
      </header>

      <div className="device-pairing-list">
        {devices.length === 0 && <p className="device-pairing-empty">ยังไม่มีอุปกรณ์ที่เชื่อมต่อ</p>}
        {devices.map((device) => (
          <div className={`device-pairing-item${device.revokedAt ? " device-pairing-item-revoked" : ""}`} key={device.id}>
            <div><strong>{device.label || device.platform}</strong><small>{device.platform} · {device.authorityState}</small></div>
            {!device.revokedAt && <button className="device-pairing-revoke" onClick={() => void revoke(device.id)} aria-label={`ยกเลิก ${device.label}`}><Trash2 size={16} /></button>}
          </div>
        ))}
      </div>

      {pairing.kind === "waiting" && <div className="device-pairing-code-box"><p>กรอกรหัสนี้บนอุปกรณ์ที่ต้องการเชื่อมต่อ</p><div className="device-pairing-code">{pairing.displayCode}</div><div className="device-pairing-countdown">เหลือ {Math.max(0, Math.ceil((pairing.expiresAtMs - now) / 1000))} วินาที</div></div>}
      {pairing.kind === "confirmed" && <p className="device-pairing-success">เชื่อมต่อ {pairing.peerName} สำเร็จ</p>}
      {pairing.kind === "error" && <p className="device-pairing-error">{pairing.message}</p>}
      <button className="device-pairing-start" onClick={() => void startPairing()} disabled={busy || pairing.kind === "waiting"}><Link2 size={16} /> สร้างรหัสจับคู่</button>
      <button className="device-pairing-start" onClick={() => void refresh()} disabled={busy}><RefreshCw size={16} /> รีเฟรช</button>

      <div className="device-pairing-fungwire">
        <div className="device-pairing-fungwire-header"><strong>FUNGWIRE</strong><small>เชื่อมต่อภายในเครือข่าย</small><button className={`device-pairing-fungwire-switch${fungwire.enabled ? " is-on" : ""}`} onClick={() => void toggleFungwire()} disabled={busy} aria-label="สลับ FUNGWIRE"><i /></button></div>
        <dl className="device-pairing-fungwire-status"><div><dt>สถานะ</dt><dd>{fungwire.enabled ? "เปิดใช้งาน" : "ปิดใช้งาน"}</dd></div><div><dt>อุปกรณ์เชื่อมต่อ</dt><dd>{fungwire.connectedPeers}</dd></div><div><dt>งานที่ทำงาน</dt><dd>{fungwire.activeJobs}</dd></div></dl>
        {fungwireError && <p className="device-pairing-error">{fungwireError}</p>}
      </div>
    </section>
  );
}
