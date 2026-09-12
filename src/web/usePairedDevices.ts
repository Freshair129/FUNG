import { useCallback, useEffect, useState } from "react";
import { supabase } from "../lib/supabase";

export type PairedDevice = {
  id: string;
  device_label: string;
  platform: string;
  last_seen_at: string | null;
};

/** Load state. `error` is deliberately distinct from an empty `ready` list:
 * a failed query and "you have no devices" look identical to a user
 * otherwise, and only one of them is worth acting on. */
export type PairedDevicesState = "loading" | "ready" | "error";

export function formatRelativeThai(iso: string | null): string {
  if (!iso) return "ไม่เคยใช้งาน";
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "เมื่อสักครู่";
  if (diffMin < 60) return `${diffMin} นาทีที่แล้ว`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr} ชั่วโมงที่แล้ว`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay} วันที่แล้ว`;
}

/** Live paired-device state for the web surfaces. Both the Dashboard tile and
 * the Account settings section read the same rows through this hook so they
 * cannot drift apart — RLS scopes the query to the signed-in user. */
export function usePairedDevices() {
  const [devices, setDevices] = useState<PairedDevice[]>([]);
  const [state, setState] = useState<PairedDevicesState>("loading");
  const [actionError, setActionError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const { data, error } = await supabase
      .from("devices")
      .select("id, device_label, platform, last_seen_at")
      .is("revoked_at", null)
      .order("registered_at", { ascending: false });

    if (error) {
      console.error("Failed to load devices:", error);
      setState("error");
      return;
    }

    setDevices(data ?? []);
    setState("ready");
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const revoke = useCallback(
    async (id: string, source: string) => {
      setActionError(null);
      const {
        data: { user },
      } = await supabase.auth.getUser();

      const { error } = await supabase.from("devices").delete().eq("id", id);
      if (error) {
        console.error("Failed to revoke device:", error);
        setActionError("ยกเลิกอุปกรณ์ไม่สำเร็จ ลองใหม่อีกครั้ง");
        return;
      }

      if (user) {
        await supabase.from("device_audit_events").insert({
          user_id: user.id,
          device_id: id,
          event_type: "device_revoked",
          metadata: { source },
        });
      }

      await reload();
    },
    [reload],
  );

  return { devices, state, actionError, reload, revoke };
}
