// @req FR-107, FR-108, FR-110, FR-114, FR-116, NFR-106, NFR-108
// @tested tests/externalMeetingTools.test.mjs
import { useEffect, useMemo, useReducer, useState } from "react";
import {
  externalConnectorDisconnect,
  externalConnectorRegister,
  externalConnectorsList,
  meetingToolCancel,
  meetingToolExecute,
  meetingToolRevoke,
  meetingToolRunsList,
  meetingToolSuggest,
} from "../tauri";
import {
  buildExternalToolArguments,
  createExternalToolUiState,
  externalMeetingToolsEnabled,
  externalToolErrorMessage,
  reduceExternalToolState,
  type ConnectorCapability,
  type ExternalConnectorSummary,
  type ExternalToolRun,
  type MeetingToolExecutionEnvelope,
  type MeetingToolPreviewEnvelope,
} from "../lib/externalMeetingTools";

type EvidenceSegment = {
  segmentId: string;
  startMs: number;
  speaker: string;
  text: string;
};

const CAPABILITY_LABELS: Record<ConnectorCapability, string> = {
  "documents.search": "ค้นหาเอกสาร",
  "documents.get_metadata": "ดูตำแหน่งและข้อมูลเอกสาร",
  "crm.customer_status.read": "อ่านสถานะลูกค้า",
};

const CRM_FIELD_OPTIONS = [
  ["status", "สถานะ"],
  ["stage", "ขั้นตอน"],
  ["owner", "ผู้ดูแล"],
  ["nextStep", "ขั้นตอนถัดไป"],
  ["updatedAt", "อัปเดตล่าสุด"],
] as const;

const featureEnabled = externalMeetingToolsEnabled({
  VITE_FUNG_EXTERNAL_MEETING_TOOLS: import.meta.env.VITE_FUNG_EXTERNAL_MEETING_TOOLS,
});

function shortTime(ms: number): string {
  const seconds = Math.floor(ms / 1_000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function resultEnvelope(value: unknown): value is MeetingToolExecutionEnvelope {
  return Boolean(value && typeof value === "object" && "run" in value && "result" in value);
}

function previewEnvelope(value: unknown): value is MeetingToolPreviewEnvelope {
  return Boolean(value && typeof value === "object" && "preview" in value && "arguments" in value);
}

export function ExternalMeetingToolsPanel({
  projectId,
  recordingId,
  segments,
}: {
  projectId: string | null;
  recordingId: string | null;
  segments: EvidenceSegment[];
}) {
  const [connectors, setConnectors] = useState<ExternalConnectorSummary[]>([]);
  const [connectorId, setConnectorId] = useState("");
  const [capability, setCapability] = useState<ConnectorCapability>("documents.search");
  const [query, setQuery] = useState("");
  const [documentId, setDocumentId] = useState("");
  const [customerKey, setCustomerKey] = useState("");
  const [crmFields, setCrmFields] = useState<string[]>(["status", "stage"]);
  const [evidenceRefs, setEvidenceRefs] = useState<string[]>([]);
  const [history, setHistory] = useState<ExternalToolRun[]>([]);
  const [toolState, dispatch] = useReducer(reduceExternalToolState, undefined, createExternalToolUiState);
  const [busy, setBusy] = useState(false);
  const [cancelRequested, setCancelRequested] = useState(false);
  const [revoking, setRevoking] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [setupError, setSetupError] = useState<string | null>(null);
  const [registerId, setRegisterId] = useState("");
  const [registerLabel, setRegisterLabel] = useState("");
  const [registerExecutable, setRegisterExecutable] = useState("");
  const [registerCredential, setRegisterCredential] = useState("");
  const [registerCapabilities, setRegisterCapabilities] = useState<ConnectorCapability[]>([
    "documents.search",
  ]);

  const activeConnectors = useMemo(
    () => connectors.filter((connector) => connector.status === "connected"),
    [connectors],
  );
  const selectedConnector = connectors.find((connector) => connector.id === connectorId) ?? null;
  const availableCapabilities = selectedConnector?.capabilities ?? [];
  const recentSegments = segments.slice(-5);
  const preview = previewEnvelope(toolState.preview) ? toolState.preview : null;
  const execution = resultEnvelope(toolState.execution) ? toolState.execution : null;

  const refreshConnectors = async () => {
    const rows = await externalConnectorsList();
    setConnectors(rows);
    setConnectorId((current) => {
      if (rows.some((connector) => connector.id === current && connector.status === "connected")) return current;
      return rows.find((connector) => connector.status === "connected")?.id ?? "";
    });
  };

  const refreshHistory = async () => {
    if (!projectId || !recordingId) {
      setHistory([]);
      return;
    }
    setHistory(await meetingToolRunsList(projectId, recordingId));
  };

  useEffect(() => {
    if (!featureEnabled) return;
    void refreshConnectors().catch((error) => setSetupError(externalToolErrorMessage(error)));
  }, []);

  useEffect(() => {
    if (!featureEnabled) return;
    void refreshHistory().catch(() => setHistory([]));
  }, [projectId, recordingId]);

  useEffect(() => {
    if (availableCapabilities.length > 0 && !availableCapabilities.includes(capability)) {
      setCapability(availableCapabilities[0]);
    }
  }, [availableCapabilities, capability]);

  if (!featureEnabled) return null;

  const toggleEvidence = (segmentId: string) => {
    setEvidenceRefs((current) =>
      current.includes(segmentId)
        ? current.filter((candidate) => candidate !== segmentId)
        : [...current, segmentId].slice(-5),
    );
  };

  const toggleRegisterCapability = (next: ConnectorCapability) => {
    setRegisterCapabilities((current) =>
      current.includes(next) ? current.filter((candidate) => candidate !== next) : [...current, next],
    );
  };

  const handleRegister = async () => {
    setBusy(true);
    setSetupError(null);
    try {
      const connector = await externalConnectorRegister({
        id: registerId.trim(),
        accountLabel: registerLabel.trim(),
        executable: registerExecutable.trim(),
        capabilities: registerCapabilities,
        credential: registerCredential || null,
      });
      setRegisterCredential("");
      setSetupOpen(false);
      await refreshConnectors();
      setConnectorId(connector.id);
    } catch (error) {
      setRegisterCredential("");
      setSetupError(externalToolErrorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const handleDisconnect = async (id: string) => {
    setBusy(true);
    setSetupError(null);
    try {
      await externalConnectorDisconnect(id);
      dispatch({ type: "reset" });
      await refreshConnectors();
    } catch (error) {
      setSetupError(externalToolErrorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const handlePreview = async () => {
    if (!projectId || !recordingId || !connectorId || evidenceRefs.length === 0) return;
    setBusy(true);
    try {
      const argumentsValue = buildExternalToolArguments(capability, {
        query,
        documentId,
        customerKey,
        crmFields,
      });
      const nextPreview = await meetingToolSuggest({
        projectId,
        recordingId,
        connectorId,
        capability,
        arguments: argumentsValue,
        evidenceRefs,
      });
      dispatch({ type: "previewReady", preview: nextPreview });
    } catch (error) {
      dispatch({ type: "executionFailed", error: externalToolErrorMessage(error) });
    } finally {
      setBusy(false);
    }
  };

  const handleExecute = async () => {
    if (!preview) return;
    const runId = crypto.randomUUID();
    setCancelRequested(false);
    dispatch({ type: "executionStarted", runId });
    try {
      const nextExecution = await meetingToolExecute({
        runId,
        previewId: preview.preview.id,
        approvedPreviewHash: preview.preview.argumentsHash,
        arguments: preview.arguments,
      });
      dispatch({ type: "executionCompleted", execution: nextExecution });
      await refreshHistory();
    } catch (error) {
      const message = externalToolErrorMessage(error);
      dispatch(
        String(error).includes("TOOL_CANCELLED")
          ? { type: "executionCancelled" }
          : { type: "executionFailed", error: message },
      );
      await refreshHistory();
    }
  };

  const handleCancel = async () => {
    if (!toolState.runId || cancelRequested) return;
    setCancelRequested(true);
    try {
      await meetingToolCancel(toolState.runId);
    } catch (error) {
      dispatch({ type: "executionFailed", error: externalToolErrorMessage(error) });
    }
  };

  const handleRevoke = async () => {
    if (!preview?.grant || !projectId || !recordingId || revoking) return;
    setRevoking(true);
    try {
      await meetingToolRevoke({
        grantId: preview.grant.id,
        projectId,
        recordingId,
      });
      dispatch({ type: "reset" });
    } catch (error) {
      dispatch({ type: "executionFailed", error: externalToolErrorMessage(error) });
    } finally {
      setRevoking(false);
    }
  };

  return (
    <section className="external-tools" aria-labelledby="external-tools-title">
      <div className="external-tools__heading">
        <div>
          <h3 id="external-tools-title">ค้นด้วยเครื่องมือภายนอก</h3>
          <p>ส่งเฉพาะช่องข้อมูลและหลักฐานที่คุณเลือก ไม่มีการเรียกเครื่องมือก่อนอนุมัติ</p>
        </div>
        <button className="live-btn" type="button" onClick={() => setSetupOpen((open) => !open)}>
          {setupOpen ? "ปิดการตั้งค่า" : "ตั้งค่าเครื่องมือ"}
        </button>
      </div>

      {setupOpen && (
        <div className="external-tools__setup">
          <div className="external-tools__connector-list">
            {connectors.length === 0 ? (
              <p className="live-empty">ยังไม่มีเครื่องมือ local stdio</p>
            ) : (
              connectors.map((connector) => (
                <div className="external-tools__connector" key={connector.id}>
                  <div>
                    <strong>{connector.accountLabel}</strong>
                    <span>{connector.status} · {connector.endpoint}</span>
                  </div>
                  {connector.status === "connected" && (
                    <button
                      className="live-btn live-btn-danger-subtle"
                      type="button"
                      disabled={busy}
                      onClick={() => void handleDisconnect(connector.id)}
                    >
                      ยกเลิกการเชื่อมต่อ
                    </button>
                  )}
                </div>
              ))
            )}
          </div>

          <div className="external-tools__register">
            <label>
              รหัสเครื่องมือ
              <input value={registerId} onChange={(event) => setRegisterId(event.target.value)} placeholder="knowledge-base" />
            </label>
            <label>
              ชื่อที่แสดง
              <input value={registerLabel} onChange={(event) => setRegisterLabel(event.target.value)} placeholder="Knowledge Base ภายใน" />
            </label>
            <label className="external-tools__wide">
              พาธไฟล์ MCP executable
              <input value={registerExecutable} onChange={(event) => setRegisterExecutable(event.target.value)} placeholder="D:\\tools\\kb-mcp.exe" />
            </label>
            <label className="external-tools__wide">
              Credential (ไม่บังคับ เก็บใน OS keyring)
              <input
                type="password"
                autoComplete="off"
                value={registerCredential}
                onChange={(event) => setRegisterCredential(event.target.value)}
              />
            </label>
            <fieldset className="external-tools__wide">
              <legend>สิทธิ์อ่านที่อนุญาต</legend>
              {(Object.keys(CAPABILITY_LABELS) as ConnectorCapability[]).map((item) => (
                <label key={item}>
                  <input
                    type="checkbox"
                    checked={registerCapabilities.includes(item)}
                    onChange={() => toggleRegisterCapability(item)}
                  />
                  {CAPABILITY_LABELS[item]}
                </label>
              ))}
            </fieldset>
            <button
              className="live-btn live-btn-primary external-tools__wide"
              type="button"
              disabled={busy || !registerId.trim() || !registerLabel.trim() || !registerExecutable.trim() || registerCapabilities.length === 0}
              onClick={() => void handleRegister()}
            >
              บันทึกเครื่องมือ
            </button>
          </div>
        </div>
      )}

      {setupError && <p className="live-note live-note-error" role="alert">{setupError}</p>}

      <div className="external-tools__controls">
        <label>
          เครื่องมือ
          <select value={connectorId} onChange={(event) => setConnectorId(event.target.value)}>
            <option value="">เลือกเครื่องมือ</option>
            {activeConnectors.map((connector) => (
              <option key={connector.id} value={connector.id}>{connector.accountLabel}</option>
            ))}
          </select>
        </label>
        <label>
          งานที่อนุญาต
          <select value={capability} onChange={(event) => setCapability(event.target.value as ConnectorCapability)} disabled={!selectedConnector}>
            {availableCapabilities.map((item) => (
              <option key={item} value={item}>{CAPABILITY_LABELS[item]}</option>
            ))}
          </select>
        </label>
      </div>

      {capability === "documents.search" && (
        <label className="external-tools__query">
          คำค้นเอกสาร
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="เช่น สัญญาลูกค้าเดือนสิงหาคม" />
        </label>
      )}
      {capability === "documents.get_metadata" && (
        <label className="external-tools__query">
          รหัสเอกสาร
          <input value={documentId} onChange={(event) => setDocumentId(event.target.value)} placeholder="document-42" />
        </label>
      )}
      {capability === "crm.customer_status.read" && (
        <div className="external-tools__crm">
          <label>
            รหัสลูกค้า
            <input value={customerKey} onChange={(event) => setCustomerKey(event.target.value)} placeholder="customer-42" />
          </label>
          <fieldset>
            <legend>ช่องสถานะที่จะอ่าน</legend>
            {CRM_FIELD_OPTIONS.map(([field, label]) => (
              <label key={field}>
                <input
                  type="checkbox"
                  checked={crmFields.includes(field)}
                  onChange={() => setCrmFields((current) => current.includes(field) ? current.filter((item) => item !== field) : [...current, field])}
                />
                {label}
              </label>
            ))}
          </fieldset>
        </div>
      )}

      <fieldset className="external-tools__evidence">
        <legend>หลักฐานจากบทสนทนาที่จะผูกกับคำขอ</legend>
        {recentSegments.length === 0 ? (
          <p className="live-empty">เริ่มประชุมและรอ transcript ก่อนเลือกหลักฐาน</p>
        ) : (
          recentSegments.map((segment) => (
            <label key={segment.segmentId}>
              <input
                type="checkbox"
                checked={evidenceRefs.includes(segment.segmentId)}
                onChange={() => toggleEvidence(segment.segmentId)}
              />
              <span>{shortTime(segment.startMs)} · {segment.speaker}</span>
              <span>{segment.text.slice(0, 110)}</span>
            </label>
          ))
        )}
      </fieldset>

      {toolState.phase === "idle" || toolState.phase === "failed" || toolState.phase === "cancelled" ? (
        <button
          className="live-btn live-btn-primary external-tools__primary"
          type="button"
          disabled={busy || !projectId || !recordingId || !connectorId || evidenceRefs.length === 0}
          onClick={() => void handlePreview()}
        >
          {busy ? "กำลังเตรียม..." : "เตรียมคำขอ"}
        </button>
      ) : null}

      {preview && toolState.phase === "preview" && (
        <div className="external-tools__preview">
          <div className="external-tools__state-line">
            <strong>รอการอนุมัติครั้งเดียว</strong>
            <span>หมดอายุ {new Date(preview.preview.expiresAt).toLocaleTimeString("th-TH")}</span>
          </div>
          <dl>
            <div><dt>เครื่องมือ</dt><dd>{selectedConnector?.accountLabel ?? preview.preview.connectorId}</dd></div>
            <div><dt>งาน</dt><dd>{CAPABILITY_LABELS[preview.preview.capability]}</dd></div>
            <div><dt>ช่องข้อมูลออกจากเครื่อง</dt><dd>{preview.preview.approvedFields.join(", ")}</dd></div>
            <div><dt>หลักฐาน</dt><dd>{preview.preview.evidenceRefs.length} ช่วง</dd></div>
            {preview.grant && (
              <div><dt>สิทธิ์ meeting scope</dt><dd>อ่านอย่างเดียว ถึง {new Date(preview.grant.expiresAt).toLocaleTimeString("th-TH")}</dd></div>
            )}
          </dl>
          <pre>{JSON.stringify(preview.arguments, null, 2)}</pre>
          <p>การกดอนุมัติจะเรียก local stdio หนึ่งครั้ง สิทธิ์ meeting scope มีอายุไม่เกิน 15 นาที</p>
          <div className="external-tools__actions">
            <button className="live-btn" type="button" onClick={() => dispatch({ type: "reset" })}>ยกเลิกคำขอ</button>
            {preview.grant && (
              <button className="live-btn live-btn-danger-subtle" type="button" disabled={revoking} onClick={() => void handleRevoke()}>
                {revoking ? "กำลังเพิกถอน..." : "เพิกถอนสิทธิ์การประชุม"}
              </button>
            )}
            <button className="live-btn live-btn-primary" type="button" onClick={() => void handleExecute()}>อนุมัติและค้นข้อมูล</button>
          </div>
        </div>
      )}

      {toolState.phase === "running" && (
        <div className="external-tools__running" aria-live="polite">
          <div>
            <strong>{cancelRequested ? "กำลังยกเลิกการค้น..." : "กำลังค้นข้อมูล..."}</strong>
            <span>การบันทึกเสียงยังทำงานต่อ</span>
          </div>
          <button className="live-btn" type="button" disabled={cancelRequested} onClick={() => void handleCancel()}>
            ยกเลิกการค้น
          </button>
        </div>
      )}

      {toolState.error && <p className="live-note live-note-error" role="alert">{toolState.error}</p>}
      {toolState.phase === "cancelled" && <p className="live-note" role="status">ยกเลิกการค้นแล้ว การบันทึกเสียงยังทำงานต่อ</p>}

      {execution && toolState.phase === "completed" && (
        <div className="external-tools__result" aria-live="polite">
          <div className="external-tools__state-line">
            <strong>ผลจาก {selectedConnector?.accountLabel ?? execution.run.connectorId}</strong>
            <span>{new Date(execution.result.createdAt).toLocaleTimeString("th-TH")}</span>
          </div>
          <pre>{JSON.stringify(execution.result.sanitizedPayload, null, 2)}</pre>
          <p>
            Policy: อ่านอย่างเดียว · หลักฐาน {preview?.preview.evidenceRefs.length ?? 0} ช่วง · Run {execution.run.id}
          </p>
          <div className="external-tools__sources">
            <strong>แหล่งข้อมูล</strong>
            {execution.result.sourceRefs.length === 0
              ? <span>เครื่องมือไม่ได้ส่ง source reference</span>
              : execution.result.sourceRefs.map((source) => <code key={source}>{source}</code>)}
          </div>
          <p>ข้อมูลนี้มาจากเครื่องมือภายนอกและผ่านการกรองแล้ว โปรดตรวจเทียบกับแหล่งข้อมูลก่อนใช้ตัดสินใจ</p>
          <div className="external-tools__actions">
            <button className="live-btn" type="button" onClick={() => dispatch({ type: "reset" })}>เตรียมคำขอใหม่</button>
            {preview?.grant && (
              <button className="live-btn live-btn-danger-subtle" type="button" disabled={revoking} onClick={() => void handleRevoke()}>
                {revoking ? "กำลังเพิกถอน..." : "เพิกถอนสิทธิ์การประชุม"}
              </button>
            )}
          </div>
        </div>
      )}

      {history.length > 0 && (
        <details className="external-tools__history">
          <summary>ประวัติการเรียกเครื่องมือ {history.length} รายการ</summary>
          <ul>
            {history.slice(0, 8).map((run) => (
              <li key={run.id}>
                <span>{CAPABILITY_LABELS[run.capability]}</span>
                <span>{run.status} · {new Date(run.startedAt).toLocaleTimeString("th-TH")}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
