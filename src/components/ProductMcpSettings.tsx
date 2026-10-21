import { useCallback, useEffect, useState } from "react";
import { Check, Copy, KeyRound, RefreshCw, Shield, WifiOff } from "lucide-react";
import {
  productMcpGrantCreate,
  productMcpGrantRevoke,
  productMcpGrantsList,
  productMcpStart,
  productMcpStatus,
  productMcpStop,
  type ProductMcpCreatedGrant,
  type ProductMcpGrant,
  type ProductMcpStatus,
} from "../tauri";

type Props = { projectId: string | null };
const SCOPES = [
  { id: "workspace.read", label: "อ่านสถานะและงาน", hint: "ดู job ในโปรเจกต์ที่อนุญาต" },
  { id: "job.control", label: "ควบคุมงาน", hint: "ยกเลิกหรือสั่ง retry job" },
] as const;

export function ProductMcpSettings({ projectId }: Props) {
  const [status, setStatus] = useState<ProductMcpStatus | null>(null);
  const [grants, setGrants] = useState<ProductMcpGrant[]>([]);
  const [clientName, setClientName] = useState("Codex");
  const [port, setPort] = useState(49175);
  const [scopes, setScopes] = useState<string[]>([]);
  const [allProjects, setAllProjects] = useState(false);
  const [created, setCreated] = useState<ProductMcpCreatedGrant | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const [nextStatus, nextGrants] = await Promise.all([productMcpStatus(), productMcpGrantsList()]);
      setStatus(nextStatus);
      setGrants(nextGrants);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const toggleScope = (scope: string) => setScopes((current) => current.includes(scope)
    ? current.filter((item) => item !== scope)
    : [...current, scope]);

  const run = async (operation: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try { await operation(); await refresh(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };

  const createGrant = async () => {
    setBusy(true);
    setError(null);
    setCreated(null);
    try {
      const result = await productMcpGrantCreate({
        clientName,
        scopes,
        projectScope: allProjects || !projectId ? [] : [projectId],
        allProjects,
      });
      setCreated(result);
      await refresh();
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };

  const copyToken = async () => {
    if (!created) return;
    try {
      await navigator.clipboard.writeText(created.token);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      setError("คัดลอกไม่ได้ กรุณาเลือก token แล้วกด Ctrl+C");
    }
  };

  return (
    <div className="settings-runtime product-mcp-settings">
      <section className="settings-local-api" aria-label="เซิร์ฟเวอร์ FUNG Product MCP">
        <h4><Shield size={17} aria-hidden="true" /> FUNG Product MCP</h4>
        <p className="settings-local-api-note">
          เปิด MCP server สำหรับแอปในเครื่องนี้เท่านั้น ใช้ 127.0.0.1 และต้องมี grant กับ scope ที่อนุญาตก่อนเรียกข้อมูลหรือควบคุมงาน
        </p>
        <div className="product-mcp-server-row">
          <label htmlFor="product-mcp-port">Port</label>
          <input id="product-mcp-port" type="number" min={49152} max={65535} value={port}
            onChange={(event) => setPort(Number(event.currentTarget.value))} disabled={status?.running || busy} />
          {status?.running ? (
            <button type="button" className="settings-runtime-start" disabled={busy} onClick={() => void run(productMcpStop)}>
              ปิด MCP
            </button>
          ) : (
            <button type="button" className="settings-runtime-start" disabled={busy || port < 49152 || port > 65535}
              onClick={() => void run(() => productMcpStart(port))}>เปิด MCP</button>
          )}
          <button type="button" className="product-mcp-icon-button" aria-label="รีเฟรชสถานะ MCP" disabled={busy} onClick={() => void refresh()}>
            <RefreshCw size={16} />
          </button>
        </div>
        <div className="product-mcp-endpoint" role="status">
          <span className={`product-mcp-status-dot${status?.running ? " is-running" : ""}`} />
          {status?.running ? "ทำงานอยู่ที่" : "ปิดอยู่ · เปิดเองเมื่อพร้อมใช้"} <code>{status?.endpoint ?? "http://127.0.0.1:49175/mcp"}</code>
        </div>
        <p className="product-mcp-local-note"><WifiOff size={15} /> ไม่เปิดรับการเชื่อมต่อจาก LAN หรืออินเทอร์เน็ต</p>
      </section>

      <section className="settings-local-api" aria-label="ออกสิทธิ์ให้ MCP client">
        <h4><KeyRound size={17} aria-hidden="true" /> ออกสิทธิ์ให้ client</h4>
        <p className="settings-local-api-note">token จะแสดงครั้งเดียวและเก็บใน Windows Credential Manager · ค่าเริ่มต้นไม่มี scope</p>
        <label className="product-mcp-field-label" htmlFor="product-mcp-client-name">ชื่อ client</label>
        <input id="product-mcp-client-name" className="settings-local-api-url" value={clientName}
          onChange={(event) => setClientName(event.currentTarget.value)} maxLength={128} disabled={busy} />
        <fieldset className="product-mcp-scopes">
          <legend>สิทธิ์ที่ให้</legend>
          {SCOPES.map((scope) => (
            <label key={scope.id}>
              <input type="checkbox" checked={scopes.includes(scope.id)} onChange={() => toggleScope(scope.id)} disabled={busy} />
              <span><strong>{scope.label}</strong><small>{scope.hint}</small></span>
            </label>
          ))}
        </fieldset>
        <label className="product-mcp-project-scope">
          <input type="checkbox" checked={allProjects} onChange={(event) => setAllProjects(event.currentTarget.checked)} disabled={busy} />
          <span><strong>อนุญาตทุกโปรเจกต์</strong><small>{allProjects ? "ขอบเขตกว้างที่สุด" : projectId ? `จำกัดเฉพาะโปรเจกต์ที่เลือก (${projectId})` : "ยังไม่เลือกโปรเจกต์ — grant นี้จะอ่าน/ควบคุมงานในโปรเจกต์ไม่ได้"}</small></span>
        </label>
        <button type="button" className="settings-runtime-start" disabled={busy || !clientName.trim()} onClick={() => void createGrant()}>
          ออก grant ใหม่
        </button>
        {created && (
          <div className="product-mcp-created" role="status">
            <strong>คัดลอก token นี้ไปเก็บในตัวจัดการ secret ของ MCP client ตอนนี้</strong>
            <code>{created.token}</code>
            <button type="button" className="settings-runtime-start" onClick={() => void copyToken()}>
              {copied ? <Check size={15} /> : <Copy size={15} />}{copied ? "คัดลอกแล้ว" : "คัดลอก token"}
            </button>
          </div>
        )}
      </section>

      <section className="settings-local-api" aria-label="grant ที่ออกไว้">
        <h4>Grant ที่ออกไว้</h4>
        {grants.length === 0 ? <p className="settings-local-api-note">ยังไม่มี grant</p> : (
          <ul className="product-mcp-grant-list">
            {grants.map((grant) => (
              <li key={grant.id}>
                <div><strong>{grant.clientName}</strong><small>{grant.scopes.length ? grant.scopes.join(" · ") : "ไม่มี scope"}</small>
                  <small>{grant.allProjects ? "ทุกโปรเจกต์" : grant.projectScope.length ? grant.projectScope.join(", ") : "ไม่มีโปรเจกต์"}</small></div>
                <button type="button" className="settings-runtime-start" disabled={busy || !grant.active}
                  onClick={() => void run(() => productMcpGrantRevoke(grant.id))}>{grant.active ? "เพิกถอน" : "เพิกถอนแล้ว"}</button>
              </li>
            ))}
          </ul>
        )}
      </section>
      {error && <p className="settings-local-api-error" role="alert">{error}</p>}
    </div>
  );
}
