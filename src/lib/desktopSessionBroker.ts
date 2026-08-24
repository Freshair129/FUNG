import { invoke } from "@tauri-apps/api/core";
import type { BackupArchiveRecord, RestoreResult } from "./backupFlow";

export const BROKER_OPERATIONS = [
  "broker_session_login_begin", "broker_session_login_cancel", "broker_session_status", "broker_session_logout",
  "broker_enrollment_request", "broker_enrollment_status", "broker_device_list", "broker_pairing_create",
  "broker_pairing_poll", "broker_pairing_reconcile", "broker_device_revoke", "broker_device_audit_list",
  "broker_fungwire_status", "broker_fungwire_set_enabled", "broker_device_endpoint_publish", "account_portal_open",
  "broker_drive_connect_begin", "broker_drive_connect_complete", "broker_drive_connect_cancel", "broker_drive_status",
  "broker_drive_disconnect", "broker_drive_list_archives", "broker_drive_upload_archive", "broker_drive_restore_intent",
  "broker_drive_restore",
] as const;

export type SessionStatus = {
  state: "signed_out" | "login_pending" | "authenticated" | "refreshing" | "refresh_failed" | "logout_pending" | "credential_cleanup_failed" | "shutdown";
  userId: string | null;
  email: string | null;
  accessExpiresAtMs: number | null;
};
export type LoginStarted = { requestId: string; expiresAtMs: number };
export type EnrollmentResult = { requestId: string; status: "pending"; authorityState: "pending" };
export type DeviceRow = { id: string; label: string; platform: string; authorityState: string; pairedAt: string | null; revokedAt: string | null; endpointState: string | null };
export type PairingPeer = { id: string; label: string; platform: string; fingerprint: string };
export type PairingPollResult = { status: string; peer: PairingPeer | null };
export type PairingResult = { pairingId: string; displayCode: string; expiresAtMs: number; status: "waiting" };
export type FungwireStatus = { enabled: boolean; bind: string | null; activeJobs: number; connectedPeers: number };
export type DriveConnectionStatus = { connected: boolean; scope: string | null; provider: "google_drive" };
export type DriveArchiveSummary = { fileId: string; archiveId: string; byteCount: number; digest: string | null; modifiedTime: string | null };
export type DriveConnectStart = { requestId: string; scope: "drive.appdata"; expiresAtMs: number };

export function brokerSessionStatus(): Promise<SessionStatus> { return invoke("broker_session_status"); }
export function brokerSessionLoginBegin(): Promise<LoginStarted> { return invoke("broker_session_login_begin"); }
export function brokerSessionLoginCancel(requestId: string): Promise<{ requestId: string; status: "cancelled" }> {
  return invoke("broker_session_login_cancel", { requestId });
}
export function brokerSessionLogout(): Promise<SessionStatus> { return invoke("broker_session_logout"); }
export function brokerEnrollmentRequest(deviceLabel: string): Promise<EnrollmentResult> {
  return invoke("broker_enrollment_request", { input: { deviceLabel } });
}
export function brokerDeviceList(): Promise<DeviceRow[]> { return invoke("broker_device_list"); }
export function brokerDeviceEndpointPublish(): Promise<{ status: string; updatedAt: string | null }> {
  return invoke("broker_device_endpoint_publish");
}
export function brokerPairingPoll(pairingId: string): Promise<PairingPollResult> {
  return invoke("broker_pairing_poll", { pairingId });
}
export function brokerPairingCreate(label: string): Promise<PairingResult> {
  return invoke("broker_pairing_create", { input: { label } });
}
export function brokerDeviceRevoke(deviceId: string): Promise<{ deviceId: string; status: "revoked" }> {
  return invoke("broker_device_revoke", { deviceId });
}
export function brokerFungwireStatus(): Promise<FungwireStatus> { return invoke("broker_fungwire_status"); }
export function brokerFungwireSetEnabled(enabled: boolean): Promise<FungwireStatus> {
  return invoke("broker_fungwire_set_enabled", { enabled });
}

export function brokerDriveStatus(): Promise<DriveConnectionStatus> { return invoke("broker_drive_status"); }
export function brokerDriveConnectBegin(): Promise<DriveConnectStart> { return invoke("broker_drive_connect_begin"); }
export function brokerDriveConnectComplete(requestId: string): Promise<DriveConnectionStatus> {
  return invoke("broker_drive_connect_complete", { sessionId: requestId });
}
export function brokerDriveConnectCancel(requestId: string): Promise<{ requestId: string; status: "cancelled" }> {
  return invoke("broker_drive_connect_cancel", { sessionId: requestId });
}
export function brokerDriveDisconnect(): Promise<DriveConnectionStatus> { return invoke("broker_drive_disconnect"); }
export function brokerDriveListArchives(): Promise<DriveArchiveSummary[]> { return invoke("broker_drive_list_archives"); }
export function brokerDriveUploadArchive(archive: BackupArchiveRecord): Promise<DriveArchiveSummary> {
  return invoke("broker_drive_upload_archive", { archiveId: archive.archiveId });
}
export function brokerDriveRestoreIntent(archiveId: string): Promise<string> {
  return invoke("broker_drive_restore_intent", { archiveId });
}
export function brokerDriveRestore(
  archive: DriveArchiveSummary,
  recoveryPhrase: string,
  restoreIntentId: string,
): Promise<RestoreResult> {
  return invoke("broker_drive_restore", {
    fileId: archive.fileId,
    archiveId: archive.archiveId,
    recoveryPhrase,
    restoreIntentId,
  });
}
