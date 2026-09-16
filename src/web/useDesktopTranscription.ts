import { useCallback, useEffect, useRef, useState } from "react";
import {
  fetchJob,
  fetchTranscript,
  importRecording,
  LocalApiError,
  type LocalApiConnection,
  type LocalTranscriptSegment,
} from "./localApiClient";
import {
  updateWebRecording,
  webRecordingFileName,
  type DesktopTranscription,
  type StoredWebRecording,
} from "./webRecordings";

export type DesktopTranscriptionPhase = "uploading" | "running" | "completed" | "failed";

export type DesktopTranscriptionStatus = {
  phase: DesktopTranscriptionPhase;
  /** 0–100 while running. */
  progress: number;
  error: string | null;
  segments: LocalTranscriptSegment[] | null;
};

const POLL_MS = 2000;

function describe(error: unknown): string {
  if (error instanceof LocalApiError) {
    switch (error.kind) {
      case "unreachable":
        return "ติดต่อ FUNG desktop ไม่ได้ — เปิดแอป desktop แล้วกด Start local API";
      case "unauthorized":
        return "ลิงก์เชื่อมต่อใช้ไม่ได้แล้ว — คัดลอกลิงก์ใหม่จาก desktop";
      case "http":
        return `desktop ตอบกลับผิดพลาด: ${error.message}`;
    }
  }
  return error instanceof Error ? error.message : String(error);
}

/**
 * Hands a browser recording to the FUNG desktop on this machine for
 * transcription and follows the job until its transcript is readable.
 *
 * The desktop side is `POST /recordings/import` → `GET /jobs/{id}` →
 * `GET /recordings/{id}/transcript` on the loopback API. Which desktop job
 * belongs to which browser recording is remembered on the recording itself
 * (`desktop` in IndexedDB), so a reload — or a later visit after the desktop
 * finished — picks the transcript back up instead of re-uploading.
 */
export function useDesktopTranscription(
  connection: LocalApiConnection | null,
  recordings: StoredWebRecording[] | null,
  onLinked: (recordingId: string, desktop: DesktopTranscription) => void,
  onCompleted?: () => void,
) {
  const [statuses, setStatuses] = useState<Record<string, DesktopTranscriptionStatus>>({});
  const polling = useRef<number | null>(null);
  const onCompletedRef = useRef(onCompleted);
  onCompletedRef.current = onCompleted;

  const patch = useCallback((id: string, next: Partial<DesktopTranscriptionStatus>) => {
    setStatuses((current) => {
      const base: DesktopTranscriptionStatus = current[id] ?? {
        phase: "running",
        progress: 0,
        error: null,
        segments: null,
      };
      return { ...current, [id]: { ...base, ...next } };
    });
  }, []);

  const loadTranscript = useCallback(
    async (webId: string, desktopRecordingId: string) => {
      if (!connection) return;
      try {
        const segments = await fetchTranscript(connection, desktopRecordingId);
        patch(webId, { phase: "completed", progress: 100, segments, error: null });
        onCompletedRef.current?.();
      } catch (cause) {
        patch(webId, { phase: "failed", error: describe(cause) });
      }
    },
    [connection, patch],
  );

  /** One poll over every recording whose desktop job is not settled yet. */
  const pollOnce = useCallback(async () => {
    if (!connection || !recordings) return;
    const pending = recordings.filter((recording) => {
      if (!recording.desktop) return false;
      const phase = statuses[recording.id]?.phase;
      return phase === undefined || phase === "running";
    });
    await Promise.all(
      pending.map(async (recording) => {
        const desktop = recording.desktop as DesktopTranscription;
        try {
          const job = await fetchJob(connection, desktop.jobId);
          if (job.status === "completed") {
            await loadTranscript(recording.id, desktop.recordingId);
          } else if (job.status === "failed" || job.status === "cancelled") {
            patch(recording.id, {
              phase: "failed",
              error: job.errorMessage ?? `desktop รายงานสถานะ ${job.status}`,
            });
          } else {
            patch(recording.id, { phase: "running", progress: job.progress ?? 0, error: null });
          }
        } catch (cause) {
          // A desktop that went away mid-job is reported, not retried forever.
          patch(recording.id, { phase: "failed", error: describe(cause) });
        }
      }),
    );
  }, [connection, recordings, statuses, loadTranscript, patch]);

  useEffect(() => {
    if (polling.current !== null) {
      window.clearInterval(polling.current);
      polling.current = null;
    }
    if (!connection || !recordings) return;
    const hasPending = recordings.some((recording) => {
      if (!recording.desktop) return false;
      const phase = statuses[recording.id]?.phase;
      return phase === undefined || phase === "running";
    });
    if (!hasPending) return;
    void pollOnce();
    polling.current = window.setInterval(() => void pollOnce(), POLL_MS);
    return () => {
      if (polling.current !== null) window.clearInterval(polling.current);
      polling.current = null;
    };
    // `statuses` is deliberately not a dependency: it changes on every poll
    // and would restart the interval each time; `pollOnce` re-reads it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connection, recordings]);

  const send = useCallback(
    async (recording: StoredWebRecording) => {
      if (!connection) return;
      patch(recording.id, { phase: "uploading", progress: 0, error: null, segments: null });
      try {
        const receipt = await importRecording(connection, recording.blob, webRecordingFileName(recording));
        const desktop: DesktopTranscription = {
          jobId: receipt.jobId,
          projectId: receipt.projectId,
          recordingId: receipt.recordingId,
          sentAt: new Date().toISOString(),
        };
        await updateWebRecording(recording.id, { desktop });
        onLinked(recording.id, desktop);
        patch(recording.id, { phase: "running", progress: 0 });
      } catch (cause) {
        patch(recording.id, { phase: "failed", error: describe(cause) });
      }
    },
    [connection, onLinked, patch],
  );

  return { statuses, send };
}
