import type { BackupArchiveRecord, InvokeFn, RestoreResult } from "./backupFlow";
import { supabase } from "./supabase";

export const GOOGLE_DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.appdata";

export type DriveConnectionStatus = {
  connected: boolean;
  scope: string | null;
  provider: "google_drive";
};

export type DriveArchiveSummary = {
  fileId: string;
  archiveId: string;
  byteCount: number;
  digest: string | null;
  modifiedTime: string | null;
};

export type DriveConnectState = "idle" | "authorizing" | "connected" | "disconnected";

function publicError(code: string): Error {
  return new Error(code);
}

export function googleDriveClientId(): string | null {
  const value = (import.meta.env.VITE_GOOGLE_DRIVE_CLIENT_ID as string | undefined)?.trim();
  return value || null;
}

async function googleDriveContext(): Promise<{
  sessionProof: string;
}> {
  const [{ data: sessionData }, configuredClientId] = await Promise.all([
    supabase.auth.getSession(),
    Promise.resolve(googleDriveClientId()),
  ]);
  const sessionProof = sessionData.session?.access_token;
  if (!configuredClientId) throw publicError("google_drive_client_id_missing");
  if (!sessionProof) throw publicError("missing_session");
  return { sessionProof };
}

export async function getDriveConnectionStatus(
  invoke: InvokeFn,
): Promise<DriveConnectionStatus> {
  const { sessionProof } = await googleDriveContext();
  return invoke<DriveConnectionStatus>("drive_connection_status", { sessionProof });
}

export async function connectGoogleDrive(
  invoke: InvokeFn,
  onStarted?: (sessionId: string) => void,
): Promise<DriveConnectionStatus> {
  const { sessionProof } = await googleDriveContext();
  const started = await invoke<{
    sessionId: string;
    scope: string;
    redirectUri: string;
  }>("drive_oauth_start", { sessionProof });
  if (started.scope !== GOOGLE_DRIVE_SCOPE) {
    await invoke<void>("drive_oauth_cancel", { sessionId: started.sessionId }).catch(() => undefined);
    throw publicError("drive_oauth_scope_mismatch");
  }
  onStarted?.(started.sessionId);

  try {
    const status = await invoke<DriveConnectionStatus>("drive_oauth_complete", {
      sessionId: started.sessionId,
      sessionProof,
    });
    return status;
  } catch (error) {
    await invoke<void>("drive_oauth_cancel", { sessionId: started.sessionId }).catch(
      () => undefined,
    );
    throw error;
  }
}

export async function cancelGoogleDriveConnect(
  invoke: InvokeFn,
  sessionId: string,
): Promise<void> {
  if (!sessionId) throw publicError("drive_oauth_session_missing");
  await invoke<void>("drive_oauth_cancel", { sessionId });
}

export async function disconnectGoogleDrive(invoke: InvokeFn): Promise<DriveConnectionStatus> {
  const { sessionProof } = await googleDriveContext();
  return invoke<DriveConnectionStatus>("drive_disconnect", { sessionProof });
}

export async function listGoogleDriveArchives(
  invoke: InvokeFn,
): Promise<DriveArchiveSummary[]> {
  const { sessionProof } = await googleDriveContext();
  return invoke<DriveArchiveSummary[]>("drive_archives_list", { sessionProof });
}

export async function uploadGoogleDriveArchive(
  invoke: InvokeFn,
  archive: BackupArchiveRecord,
): Promise<DriveArchiveSummary> {
  const { sessionProof } = await googleDriveContext();
  return invoke<DriveArchiveSummary>("drive_upload_archive", {
    sessionProof,
    archiveId: archive.archiveId,
  });
}

export async function createGoogleDriveRestoreIntent(
  invoke: InvokeFn,
  archiveId: string,
  confirmedCleanTarget: boolean,
): Promise<string> {
  if (!confirmedCleanTarget) throw publicError("restore_not_confirmed");
  const { sessionProof } = await googleDriveContext();
  return invoke<string>("drive_restore_intent_create", { sessionProof, archiveId });
}

export async function restoreGoogleDriveArchive(
  invoke: InvokeFn,
  archive: DriveArchiveSummary,
  recoveryPhrase: string,
  restoreIntentId: string,
): Promise<RestoreResult> {
  const phrase = recoveryPhrase.trim();
  if (!phrase) throw publicError("missing_recovery_phrase");
  if (!restoreIntentId.trim()) throw publicError("restore_intent_invalid");
  const { sessionProof } = await googleDriveContext();
  return invoke<RestoreResult>("drive_restore", {
    sessionProof,
    fileId: archive.fileId,
    archiveId: archive.archiveId,
    recoveryPhrase: phrase,
    restoreIntentId,
  });
}

export function describeGoogleDriveError(raw: unknown): string {
  const message = raw instanceof Error ? raw.message : String(raw ?? "");
  const messages: Record<string, string> = {
    google_drive_client_id_missing: "ยังไม่ได้ตั้ง Google Drive OAuth Client ID สำหรับ Desktop",
    google_drive_client_id_missing_or_invalid: "Google Drive OAuth Client ID ไม่ถูกต้อง",
    missing_session: "ต้องเข้าสู่ระบบ FUNG ก่อนเชื่อมต่อ Google Drive",
    missing_device_identity: "ยังไม่ได้ลงทะเบียนอุปกรณ์นี้กับบัญชี FUNG",
    supabase_authorization_config_missing: "ยังไม่ได้ตั้งค่า native authorization ของ FUNG",
    supabase_authorization_origin_invalid: "ที่อยู่ authorization ของ FUNG ไม่อยู่ใน trusted origin",
    drive_authorization_denied: "อุปกรณ์หรือสิทธิ์ของบัญชีนี้ไม่ได้รับอนุญาต",
    drive_authorization_unavailable: "ตรวจสอบสิทธิ์ native authorization ไม่สำเร็จ",
    drive_authorization_expired: "สิทธิ์ native authorization หมดอายุ — ลองใหม่อีกครั้ง",
    auth_url_untrusted: "ปฏิเสธ URL เข้าสู่ระบบที่ไม่อยู่ใน trusted registry",
    auth_url_open_failed: "เปิดหน้าต่างเข้าสู่ระบบไม่ได้",
    drive_oauth_open_failed: "เปิดหน้าต่าง Google OAuth ไม่ได้",
    drive_connection_activation_failed: "บันทึกการเชื่อมต่อ Google Drive ไม่สำเร็จ",
    drive_oauth_scope_mismatch: "การอนุญาตไม่ตรง scope ที่กำหนด — ระบบปฏิเสธการเชื่อมต่อ",
    drive_oauth_cancelled: "ยกเลิกการเชื่อมต่อ Google Drive แล้ว",
    drive_oauth_expired: "คำขอเชื่อมต่อหมดอายุ — ลองใหม่อีกครั้ง",
    drive_oauth_session_missing: "ไม่พบเซสชัน OAuth — ลองเริ่มใหม่",
    drive_oauth_token_exchange_failed: "แลกเปลี่ยนสิทธิ์ Google Drive ไม่สำเร็จ",
    drive_oauth_offline_access_missing: "Google ไม่ส่งสิทธิ์สำหรับการเชื่อมต่อถาวร",
    drive_not_connected: "ยังไม่ได้เชื่อมต่อ Google Drive",
    drive_keyring_unavailable: "ระบบเก็บ credential ของเครื่องใช้งานไม่ได้",
    drive_token_refresh_failed: "ต่ออายุสิทธิ์ Google Drive ไม่สำเร็จ — อาจต้องเชื่อมต่อใหม่",
    drive_list_failed: "อ่านรายการไฟล์สำรองจาก Google Drive ไม่สำเร็จ",
    drive_upload_failed: "อัปโหลดไฟล์สำรองไป Google Drive ไม่สำเร็จ",
    drive_download_failed: "ดาวน์โหลดไฟล์สำรองจาก Google Drive ไม่สำเร็จ",
    drive_manifest_not_found: "ไม่พบ manifest ของไฟล์สำรองบน Google Drive",
    drive_manifest_invalid: "manifest บน Google Drive ไม่ถูกต้อง",
    drive_archive_digest_mismatch: "ไฟล์สำรองบน Google Drive ไม่ตรงกับ digest — ยกเลิกการกู้คืน",
    drive_archive_already_exists: "มีไฟล์สำรอง archive ID นี้อยู่แล้ว แต่ digest ไม่ตรง",
    restore_not_confirmed: "ต้องยืนยันการกู้คืนสู่โฟลเดอร์ว่างก่อน",
    missing_recovery_phrase: "กรอกรหัสกู้คืน 24 คำก่อนกู้คืน",
    restore_intent_invalid: "คำขอกู้คืนหมดอายุหรือไม่ตรงกับ archive/โฟลเดอร์ที่เลือก",
  };
  return messages[message] ?? "การทำงาน Google Drive ล้มเหลว — ยังไม่มีการเปลี่ยนข้อมูลโดยอัตโนมัติ";
}
