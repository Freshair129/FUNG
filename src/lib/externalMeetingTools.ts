// @req FR-106, FR-107, FR-108, FR-112, NFR-102
// @tested tests/externalMeetingTools.test.mjs
export type ConnectorCapability =
  | "documents.search"
  | "documents.get_metadata"
  | "crm.customer_status.read";

export type CrmStatusField = "status" | "stage" | "owner" | "nextStep" | "updatedAt";

export type ExternalConnectorSummary = {
  id: string;
  provider: string;
  accountLabel: string;
  status: string;
  transport: "stdio";
  endpoint: string;
  credentialRef: string | null;
  capabilities: ConnectorCapability[];
};

export type ExternalConnectorRegisterInput = {
  id: string;
  accountLabel: string;
  executable: string;
  capabilities: ConnectorCapability[];
  credential: string | null;
};

export type ExternalConnectorDisconnectReceipt = {
  connectorId: string;
  revokedGrants: number;
  disconnectedAt: string;
};

export type MeetingToolPreview = {
  id: string;
  projectId: string;
  recordingId: string;
  connectorId: string;
  toolName: string;
  capability: ConnectorCapability;
  argumentsHash: string;
  approvedFields: string[];
  evidenceRefs: string[];
  state: string;
  expiresAt: string;
  createdAt: string;
};

export type MeetingToolPreviewEnvelope = {
  preview: MeetingToolPreview;
  arguments: Record<string, unknown>;
  grant: {
    id: string;
    projectId: string;
    recordingId: string;
    connectorId: string;
    capabilities: ConnectorCapability[];
    grantedAt: string;
    expiresAt: string;
    revokedAt: string | null;
  } | null;
};

export type ExternalToolRun = {
  id: string;
  previewId: string;
  projectId: string;
  recordingId: string;
  connectorId: string;
  toolName: string;
  capability: ConnectorCapability;
  requestHash: string;
  outputHash: string | null;
  status: "running" | "completed" | "failed" | "cancelled";
  startedAt: string;
  finishedAt: string | null;
  errorCode: string | null;
  resultRef: string | null;
};

export type ExternalToolResult = {
  id: string;
  runId: string;
  mimeType: string;
  sanitizedPayload: unknown;
  sourceRefs: string[];
  byteSize: number;
  createdAt: string;
};

export type MeetingToolExecutionEnvelope = {
  run: ExternalToolRun;
  result: ExternalToolResult;
};

export type MeetingToolCancelReceipt = {
  runId: string;
  cancellationRequested: boolean;
};

export type MeetingToolRevokeReceipt = {
  grantId: string;
  revokedAt: string;
};

export type ExternalToolForm = {
  query: string;
  customerKey: string;
  documentId?: string;
  crmFields: string[];
};

const CRM_FIELDS = new Set<CrmStatusField>(["status", "stage", "owner", "nextStep", "updatedAt"]);

export function externalMeetingToolsEnabled(env?: Record<string, unknown>): boolean {
  return env?.VITE_FUNG_EXTERNAL_MEETING_TOOLS === "1";
}

function required(value: string, message: string): string {
  const trimmed = value.trim();
  if (!trimmed) throw new Error(message);
  return trimmed;
}

export function buildExternalToolArguments(
  capability: ConnectorCapability,
  form: ExternalToolForm,
): Record<string, unknown> {
  if (capability === "documents.search") {
    return { query: required(form.query, "Query is required") };
  }
  if (capability === "documents.get_metadata") {
    return { documentId: required(form.documentId ?? "", "Document ID is required") };
  }
  const fields = [...new Set(form.crmFields.filter((field): field is CrmStatusField => CRM_FIELDS.has(field as CrmStatusField)))];
  if (fields.length === 0) throw new Error("At least one CRM field is required");
  return {
    customerKey: required(form.customerKey, "Customer key is required"),
    fields,
  };
}

export type ExternalToolUiState = {
  phase: "idle" | "preview" | "running" | "completed" | "failed" | "cancelled";
  preview: MeetingToolPreviewEnvelope | { id: string } | null;
  runId: string | null;
  execution: MeetingToolExecutionEnvelope | { run: { id: string }; result: { sourceRefs: string[] } } | null;
  error: string | null;
};

export type ExternalToolUiAction =
  | { type: "previewReady"; preview: ExternalToolUiState["preview"] & object }
  | { type: "executionStarted"; runId: string }
  | { type: "executionCompleted"; execution: ExternalToolUiState["execution"] & object }
  | { type: "executionFailed"; error: string }
  | { type: "executionCancelled" }
  | { type: "reset" };

export function createExternalToolUiState(): ExternalToolUiState {
  return { phase: "idle", preview: null, runId: null, execution: null, error: null };
}

export function reduceExternalToolState(
  state: ExternalToolUiState,
  action: ExternalToolUiAction,
): ExternalToolUiState {
  switch (action.type) {
    case "previewReady":
      return { phase: "preview", preview: action.preview, runId: null, execution: null, error: null };
    case "executionStarted":
      return { ...state, phase: "running", runId: action.runId, execution: null, error: null };
    case "executionCompleted":
      return { ...state, phase: "completed", execution: action.execution, error: null };
    case "executionFailed":
      return { ...state, phase: "failed", execution: null, error: action.error };
    case "executionCancelled":
      return { ...state, phase: "cancelled", error: null };
    case "reset":
      return createExternalToolUiState();
  }
}

const ERROR_MESSAGES: Record<string, string> = {
  CONNECTOR_NOT_FOUND: "ไม่พบเครื่องมือที่เลือก กรุณาเลือกการเชื่อมต่อใหม่ การบันทึกเสียงยังทำงานต่อ",
  CONNECTOR_UNHEALTHY: "เครื่องมือภายนอกไม่พร้อมใช้งาน การบันทึกเสียงยังทำงานต่อ",
  CAPABILITY_DENIED: "ความสามารถนี้ยังไม่ได้รับอนุญาตสำหรับการประชุมนี้",
  APPROVAL_REQUIRED: "คำขอนี้ต้องได้รับการอนุมัติใหม่ก่อนเรียกใช้เครื่องมือ",
  PREVIEW_CHANGED: "รายละเอียดคำขอเปลี่ยนไป กรุณาเตรียมคำขอใหม่ก่อนอนุมัติ",
  GRANT_EXPIRED: "สิทธิ์ของการประชุมหมดอายุ กรุณาเตรียมคำขอใหม่",
  GRANT_REVOKED: "สิทธิ์ของเครื่องมือนี้ถูกเพิกถอนแล้ว",
  WRITE_TOOL_DENIED: "FUNG อนุญาตเฉพาะการอ่านข้อมูล เครื่องมือที่แก้ไขข้อมูลถูกปฏิเสธ",
  EGRESS_FIELD_DENIED: "คำขอมีข้อมูลเกินขอบเขตที่อนุญาต กรุณาตรวจช่องข้อมูลอีกครั้ง",
  TOOL_TIMEOUT: "เครื่องมือภายนอกหมดเวลา การบันทึกเสียงยังทำงานต่อ",
  TOOL_CANCELLED: "ยกเลิกการค้นข้อมูลแล้ว การบันทึกเสียงยังทำงานต่อ",
  RESULT_TOO_LARGE: "ผลลัพธ์มีขนาดเกินขอบเขตที่ปลอดภัย",
  RESULT_UNSAFE: "ผลลัพธ์ถูกปฏิเสธเพราะไม่ผ่านการตรวจความปลอดภัย",
  KEYRING_UNAVAILABLE: "ไม่สามารถเข้าถึงคลังรหัสลับของระบบได้",
};

export function externalToolErrorMessage(error: unknown): string {
  const raw = typeof error === "string" ? error : String(error);
  const code = Object.keys(ERROR_MESSAGES).find((candidate) => raw.includes(candidate));
  return code ? ERROR_MESSAGES[code] : "เรียกใช้เครื่องมือไม่สำเร็จ การบันทึกเสียงยังทำงานต่อ";
}
