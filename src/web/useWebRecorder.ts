import { useCallback, useEffect, useRef, useState } from "react";
import {
  pickRecorderMimeType,
  saveWebRecording,
  webRecordingSupported,
  type StoredWebRecording,
} from "./webRecordings";

export type WebRecorderState = "idle" | "requesting" | "recording" | "saving";

type WebRecorder = {
  state: WebRecorderState;
  supported: boolean;
  error: string | null;
  elapsedMs: number;
  /** Live input level 0–1 (RMS of the time-domain signal), 0 when idle. */
  level: number;
  start: () => Promise<void>;
  stop: () => Promise<void>;
};

const TIMESLICE_MS = 1000;

function describeStartError(error: unknown): string {
  const name = error instanceof DOMException ? error.name : "";
  if (name === "NotAllowedError" || name === "SecurityError") {
    return "เบราว์เซอร์ไม่อนุญาตให้ใช้ไมค์ — กดอนุญาตที่แถบที่อยู่แล้วลองใหม่";
  }
  if (name === "NotFoundError" || name === "OverconstrainedError") {
    return "ไม่พบไมโครโฟนในเครื่องนี้";
  }
  if (name === "NotReadableError") {
    return "ไมค์ถูกโปรแกรมอื่นใช้อยู่ ปิดโปรแกรมนั้นแล้วลองใหม่";
  }
  return error instanceof Error ? error.message : String(error);
}

/**
 * Microphone → MediaRecorder → IndexedDB, for the web dashboard.
 *
 * Mirrors the mobile shell's web backend (`MobileApp.tsx` `begin`/`stop`)
 * but keeps the whole recording in memory until stop and then persists one
 * blob, because here there is no Rust ledger to stream segments into. A
 * saved recording is handed to `onSaved` so the caller can refresh its list
 * without re-reading IndexedDB.
 */
export function useWebRecorder(onSaved: (recording: StoredWebRecording) => void): WebRecorder {
  const [state, setState] = useState<WebRecorderState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [level, setLevel] = useState(0);
  const supported = webRecordingSupported();

  const stream = useRef<MediaStream | null>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const chunks = useRef<Blob[]>([]);
  const startedAt = useRef<number>(0);
  const audioContext = useRef<AudioContext | null>(null);
  const meterFrame = useRef<number | null>(null);
  const clock = useRef<number | null>(null);

  const releaseHardware = useCallback(() => {
    if (meterFrame.current !== null) {
      cancelAnimationFrame(meterFrame.current);
      meterFrame.current = null;
    }
    if (clock.current !== null) {
      window.clearInterval(clock.current);
      clock.current = null;
    }
    stream.current?.getTracks().forEach((track) => track.stop());
    stream.current = null;
    void audioContext.current?.close().catch(() => undefined);
    audioContext.current = null;
    setLevel(0);
  }, []);

  useEffect(() => () => releaseHardware(), [releaseHardware]);

  const startMeter = (media: MediaStream) => {
    try {
      const context = new AudioContext();
      const analyser = context.createAnalyser();
      analyser.fftSize = 1024;
      context.createMediaStreamSource(media).connect(analyser);
      audioContext.current = context;
      const samples = new Float32Array(analyser.fftSize);
      const tick = () => {
        analyser.getFloatTimeDomainData(samples);
        let sum = 0;
        for (let i = 0; i < samples.length; i += 1) sum += samples[i] * samples[i];
        // RMS of speech sits around 0.05–0.3; scale so normal talking fills
        // most of the bar without clipping shouting to exactly 1.
        setLevel(Math.min(1, Math.sqrt(sum / samples.length) * 4));
        meterFrame.current = requestAnimationFrame(tick);
      };
      meterFrame.current = requestAnimationFrame(tick);
    } catch {
      // A meter is a nicety; recording continues without it.
    }
  };

  const start = useCallback(async () => {
    if (state !== "idle") return;
    setError(null);
    if (!supported) {
      setError("เบราว์เซอร์นี้อัดเสียงไม่ได้ (ต้องมี MediaRecorder และ IndexedDB)");
      return;
    }
    setState("requesting");
    try {
      const media = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true },
      });
      const mimeType = pickRecorderMimeType((candidate) => MediaRecorder.isTypeSupported(candidate));
      if (!mimeType) {
        media.getTracks().forEach((track) => track.stop());
        throw new Error("เบราว์เซอร์นี้ไม่รองรับการอัดเสียงเป็นไฟล์ (webm/mp4/ogg)");
      }
      const instance = new MediaRecorder(media, { mimeType, audioBitsPerSecond: 128000 });
      chunks.current = [];
      instance.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.current.push(event.data);
      };
      stream.current = media;
      recorder.current = instance;
      startedAt.current = Date.now();
      setElapsedMs(0);
      instance.start(TIMESLICE_MS);
      clock.current = window.setInterval(() => setElapsedMs(Date.now() - startedAt.current), 250);
      startMeter(media);
      setState("recording");
    } catch (cause) {
      releaseHardware();
      setError(describeStartError(cause));
      setState("idle");
    }
  }, [state, supported, releaseHardware]);

  const stop = useCallback(async () => {
    const instance = recorder.current;
    if (!instance || state !== "recording") return;
    setState("saving");
    try {
      const stopped = new Promise<void>((resolve) =>
        instance.addEventListener("stop", () => resolve(), { once: true }),
      );
      if (instance.state !== "inactive") {
        instance.requestData();
        instance.stop();
        await stopped;
      }
      const durationMs = Math.max(1, Date.now() - startedAt.current);
      const mimeType = instance.mimeType || chunks.current[0]?.type || "audio/webm";
      const blob = new Blob(chunks.current, { type: mimeType });
      if (blob.size === 0) throw new Error("ไม่ได้รับข้อมูลเสียงจากไมค์เลย");
      const recording: StoredWebRecording = {
        id: crypto.randomUUID(),
        createdAt: new Date(startedAt.current).toISOString(),
        durationMs,
        mimeType,
        bytes: blob.size,
        blob,
      };
      await saveWebRecording(recording);
      onSaved(recording);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      recorder.current = null;
      chunks.current = [];
      releaseHardware();
      setElapsedMs(0);
      setState("idle");
    }
  }, [state, onSaved, releaseHardware]);

  return { state, supported, error, elapsedMs, level, start, stop };
}
