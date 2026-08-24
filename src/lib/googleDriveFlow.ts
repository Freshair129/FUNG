import type { BackupArchiveRecord, InvokeFn, RestoreResult } from "./backupFlow";

export const GOOGLE_DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.appdata";
export type DriveConnectionStatus = { connected: boolean; scope: string | null; provider: "google_drive" };
export type DriveArchiveSummary = { fileId: string; archiveId: string; byteCount: number; digest: string | null; modifiedTime: string | null };
export type DriveConnectState = "idle" | "authorizing" | "connected" | "disconnected";

function publicError(code: string): Error { return new Error(code); }

export function googleDriveClientId(): string | null {
  const value = (import.meta.env.VITE_GOOGLE_DRIVE_CLIENT_ID as string | undefined)?.trim();
  return value || null;
}

export async function getDriveConnectionStatus(invoke: InvokeFn): Promise<DriveConnectionStatus> {
  return invoke<DriveConnectionStatus>("broker_drive_status");
}

export async function connectGoogleDrive(invoke: InvokeFn, onStarted?: (requestId: string) => void): Promise<DriveConnectionStatus> {
  const started = await invoke<{ requestId: string; scope: string; expiresAtMs: number }>("broker_drive_connect_begin");
  if (started.scope !== "drive.appdata") throw publicError("drive_oauth_scope_mismatch");
  onStarted?.(started.requestId);
  try {
    return await invoke<DriveConnectionStatus>("broker_drive_connect_complete", { requestId: started.requestId });
  } catch (error) {
    await invoke("broker_drive_connect_cancel", { requestId: started.requestId }).catch(() => undefined);
    throw error;
  }
}

export async function cancelGoogleDriveConnect(invoke: InvokeFn, requestId: string): Promise<void> {
  if (!requestId.trim()) throw publicError("drive_oauth_session_missing");
  await invoke("broker_drive_connect_cancel", { requestId });
}

export function disconnectGoogleDrive(invoke: InvokeFn): Promise<DriveConnectionStatus> { return invoke("broker_drive_disconnect"); }
export function listGoogleDriveArchives(invoke: InvokeFn): Promise<DriveArchiveSummary[]> { return invoke("broker_drive_list_archives"); }
export function uploadGoogleDriveArchive(invoke: InvokeFn, archive: BackupArchiveRecord): Promise<DriveArchiveSummary> {
  return invoke("broker_drive_upload_archive", { archiveId: archive.archiveId });
}
export async function createGoogleDriveRestoreIntent(invoke: InvokeFn, archiveId: string, confirmedCleanTarget: boolean): Promise<string> {
  if (!confirmedCleanTarget) throw publicError("restore_not_confirmed");
  return invoke("broker_drive_restore_intent", { archiveId });
}
export async function restoreGoogleDriveArchive(invoke: InvokeFn, archive: DriveArchiveSummary, recoveryPhrase: string, restoreIntentId: string): Promise<RestoreResult> {
  const phrase = recoveryPhrase.trim();
  if (!phrase) throw publicError("missing_recovery_phrase");
  if (!restoreIntentId.trim()) throw publicError("restore_intent_invalid");
  return invoke("broker_drive_restore", { fileId: archive.fileId, archiveId: archive.archiveId, recoveryPhrase: phrase, restoreIntentId });
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
