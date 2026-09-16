import { invoke } from "@tauri-apps/api/core";

export const BROKER_OPERATIONS = [
  "broker_session_login_begin", "broker_session_login_cancel", "broker_session_status", "broker_session_logout",
  "broker_enrollment_request", "broker_enrollment_status", "broker_device_list", "broker_pairing_create",
  "broker_pairing_poll", "broker_pairing_reconcile", "broker_device_revoke", "broker_device_audit_list",
  "broker_fungwire_status", "broker_fungwire_set_enabled", "broker_device_endpoint_publish", "account_portal_open",
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

export function brokerSessionStatus(): Promise<SessionStatus> { return invoke("broker_session_status"); }
export function brokerSessionLoginBegin(): Promise<LoginStarted> { return invoke("broker_session_login_begin"); }
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
