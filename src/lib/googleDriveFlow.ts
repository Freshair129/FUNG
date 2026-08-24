import type { BackupArchiveRecord, RestoreResult } from "./backupFlow";
import {
  brokerDriveConnectBegin,
  brokerDriveConnectCancel,
  brokerDriveConnectComplete,
  brokerDriveDisconnect,
  brokerDriveListArchives,
  brokerDriveRestore,
  brokerDriveRestoreIntent,
  brokerDriveStatus,
  brokerDriveUploadArchive,
  type DriveArchiveSummary,
  type DriveConnectionStatus,
} from "./desktopSessionBroker";

export type { DriveArchiveSummary, DriveConnectionStatus } from "./desktopSessionBroker";

export const GOOGLE_DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.appdata";
export type DriveConnectState = "idle" | "authorizing" | "connected" | "disconnected";

function publicError(code: string): Error { return new Error(code); }

export function googleDriveClientId(): string | null {
  const value = (import.meta.env.VITE_GOOGLE_DRIVE_CLIENT_ID as string | undefined)?.trim();
  return value || null;
}

export async function getDriveConnectionStatus(_legacyPanelArgument?: unknown): Promise<DriveConnectionStatus> {
  return brokerDriveStatus();
}

export async function connectGoogleDrive(_legacyPanelArgument?: unknown, onStarted?: (requestId: string) => void): Promise<DriveConnectionStatus> {
  const started = await brokerDriveConnectBegin();
  if (started.scope !== "drive.appdata") throw publicError("drive_oauth_scope_mismatch");
  onStarted?.(started.requestId);
  try {
    return await brokerDriveConnectComplete(started.requestId);
  } catch (error) {
    await brokerDriveConnectCancel(started.requestId).catch(() => undefined);
    throw error;
  }
}

export async function cancelGoogleDriveConnect(_legacyPanelArgument: unknown, requestId: string): Promise<void> {
  if (!requestId.trim()) throw publicError("drive_oauth_session_missing");
  await brokerDriveConnectCancel(requestId);
}

export function disconnectGoogleDrive(_legacyPanelArgument?: unknown): Promise<DriveConnectionStatus> { return brokerDriveDisconnect(); }
export function listGoogleDriveArchives(_legacyPanelArgument?: unknown): Promise<DriveArchiveSummary[]> { return brokerDriveListArchives(); }
export function uploadGoogleDriveArchive(_legacyPanelArgument: unknown, archive: BackupArchiveRecord): Promise<DriveArchiveSummary> {
  return brokerDriveUploadArchive(archive);
}
export async function createGoogleDriveRestoreIntent(_legacyPanelArgument: unknown, archiveId: string, confirmedCleanTarget: boolean): Promise<string> {
  if (!confirmedCleanTarget) throw publicError("restore_not_confirmed");
  return brokerDriveRestoreIntent(archiveId);
}
export async function restoreGoogleDriveArchive(_legacyPanelArgument: unknown, archive: DriveArchiveSummary, recoveryPhrase: string, restoreIntentId: string): Promise<RestoreResult> {
  const phrase = recoveryPhrase.trim();
  if (!phrase) throw publicError("missing_recovery_phrase");
  if (!restoreIntentId.trim()) throw publicError("restore_intent_invalid");
  return brokerDriveRestore(archive, phrase, restoreIntentId);
}

export function describeGoogleDriveError(raw: unknown): string {
  const message = raw instanceof Error ? raw.message : String(raw ?? "");
  const messages: Record<string, string> = {
    auth_required: "ต้องเข้าสู่ระบบ FUNG ก่อนเชื่อมต่อ Google Drive",
    authorization_denied: "อุปกรณ์หรือสิทธิ์ของบัญชีนี้ไม่ได้รับอนุญาต",
    drive_oauth_scope_mismatch: "การอนุญาตไม่ตรง scope ที่กำหนด — ระบบปฏิเสธการเชื่อมต่อ",
    drive_oauth_cancelled: "ยกเลิกการเชื่อมต่อ Google Drive แล้ว",
    drive_not_connected: "ยังไม่ได้เชื่อมต่อ Google Drive",
    drive_archive_digest_mismatch: "ไฟล์สำรองบน Google Drive ไม่ตรงกับ digest — ยกเลิกการกู้คืน",
    restore_not_confirmed: "ต้องยืนยันการกู้คืนสู่โฟลเดอร์ว่างก่อน",
    missing_recovery_phrase: "กรอกรหัสกู้คืน 24 คำก่อนกู้คืน",
    restore_intent_invalid: "คำขอกู้คืนหมดอายุหรือไม่ตรงกับ archive/โฟลเดอร์ที่เลือก",
  };
  return messages[message] ?? "การทำงาน Google Drive ล้มเหลว — ยังไม่มีการเปลี่ยนข้อมูลโดยอัตโนมัติ";
}
