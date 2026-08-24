import type { InvokeFn } from "./backupFlow";

export const BROKER_OPERATIONS = [
  "broker_session_login_begin",
  "broker_session_login_cancel",
  "broker_session_status",
  "broker_session_logout",
  "broker_enrollment_request",
  "broker_enrollment_status",
  "broker_device_list",
  "broker_pairing_create",
  "broker_pairing_poll",
  "broker_pairing_reconcile",
  "broker_device_revoke",
  "broker_device_audit_list",
  "broker_fungwire_status",
  "broker_fungwire_set_enabled",
  "broker_device_endpoint_publish",
  "account_portal_open",
  "broker_drive_connect_begin",
  "broker_drive_connect_complete",
  "broker_drive_connect_cancel",
  "broker_drive_status",
  "broker_drive_disconnect",
  "broker_drive_list_archives",
  "broker_drive_upload_archive",
  "broker_drive_restore_intent",
  "broker_drive_restore",
] as const;

export type BrokerOperation = (typeof BROKER_OPERATIONS)[number];
export type SessionStatus = {
  state: "signed_out" | "login_pending" | "authenticated" | "refreshing" | "refresh_failed" | "logout_pending" | "credential_cleanup_failed" | "shutdown";
  userId: string | null;
  email: string | null;
  accessExpiresAtMs: number | null;
};

export type BrokerInvoke = <T>(operation: BrokerOperation, args?: Record<string, unknown>) => Promise<T>;

export async function brokerSessionStatus(invoke: BrokerInvoke): Promise<SessionStatus> {
  return invoke<SessionStatus>("broker_session_status");
}

export async function brokerSessionLoginBegin(invoke: BrokerInvoke) {
  return invoke<{ requestId: string; expiresAtMs: number }>("broker_session_login_begin");
}

export async function brokerSessionLoginCancel(invoke: BrokerInvoke, requestId: string) {
  return invoke<{ requestId: string; status: "cancelled" }>("broker_session_login_cancel", { requestId });
}

export async function brokerSessionLogout(invoke: BrokerInvoke) {
  return invoke<SessionStatus>("broker_session_logout");
}

export async function brokerEnrollmentRequest(invoke: BrokerInvoke, deviceLabel: string) {
  return invoke<{ requestId: string; status: "pending"; authorityState: "pending" }>(
    "broker_enrollment_request",
    { input: { deviceLabel } },
  );
}

export const asBrokerInvoke = (invoke: InvokeFn): BrokerInvoke =>
  (operation, args) => invoke(operation, args);
