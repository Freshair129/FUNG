import { useCallback, useEffect, useState } from "react";
import {
  audioUrl,
  clearConnection,
  fetchRecordings,
  loadConnection,
  LocalApiError,
  parseConnectUrl,
  saveConnection,
  type LocalApiConnection,
  type LocalRecording,
} from "./localApiClient";

/** `unconfigured` (no connect URL pasted yet) is a distinct state from
 * `error` (the desktop is not answering): one needs a paste, the other needs
 * the desktop app opened. Showing the same "nothing here" for both would hide
 * which one the user is looking at. */
export type LocalRecordingsState = "unconfigured" | "loading" | "ready" | "error";

function describe(error: unknown): string {
  if (error instanceof LocalApiError) {
    switch (error.kind) {
      case "unreachable":
        return "ติดต่อ FUNG desktop ไม่ได้ — เปิดแอป desktop บนเครื่องนี้ แล้วกด Start local API ในหน้า Settings › Runtime";
      case "unauthorized":
        return "ลิงก์เชื่อมต่อใช้ไม่ได้แล้ว (desktop ถูกเปิดใหม่) — คัดลอกลิงก์ใหม่จากหน้า Settings › Runtime";
      case "http":
        return `desktop ตอบกลับผิดพลาด: ${error.message}`;
    }
  }
  return error instanceof Error ? error.message : "โหลดรายการไม่สำเร็จ";
}

/** Recordings on the FUNG desktop running on this same machine, read over
 * its loopback API. Nothing here goes through Supabase or leaves the PC. */
export function useLocalRecordings() {
  const [connection, setConnection] = useState<LocalApiConnection | null>(() => loadConnection());
  const [recordings, setRecordings] = useState<LocalRecording[]>([]);
  const [state, setState] = useState<LocalRecordingsState>(connection ? "loading" : "unconfigured");
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    if (!connection) {
      setState("unconfigured");
      return;
    }
    setState("loading");
    setError(null);
    try {
      setRecordings(await fetchRecordings(connection));
      setState("ready");
    } catch (failure) {
      console.error("Failed to load local recordings:", failure);
      setError(describe(failure));
      setState("error");
    }
  }, [connection]);

  useEffect(() => {
    void reload();
  }, [reload]);

  /** Accepts the connect URL from the desktop. Returns false (and changes
   * nothing) when the string is not a loopback connect URL. */
  const connect = useCallback((input: string): boolean => {
    const parsed = parseConnectUrl(input);
    if (!parsed) return false;
    saveConnection(parsed);
    setConnection(parsed);
    return true;
  }, []);

  const disconnect = useCallback(() => {
    clearConnection();
    setConnection(null);
    setRecordings([]);
    setError(null);
    setState("unconfigured");
  }, []);

  const playbackUrl = useCallback(
    (recordingId: string, channel: string): string | null =>
      connection ? audioUrl(connection, recordingId, channel) : null,
    [connection],
  );

  return { connection, recordings, state, error, reload, connect, disconnect, playbackUrl };
}
