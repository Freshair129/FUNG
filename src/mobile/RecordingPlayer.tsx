import { useCallback, useEffect, useRef, useState } from "react";
import { Pause, Play, RotateCcw, RotateCw } from "lucide-react";
import { loadPlaybackSegment, playbackManifest } from "./bridge";
import { fitPeaks, formatPlayerClock, locateSegment, normalizePeaks, peaksFromSamples, segmentOffsets } from "./audioViz";

/**
 * Player for one recording: waveform with a playhead, tap/drag to seek,
 * ±10 s, play/pause, elapsed/total. Recordings are stored as sealed
 * segments, so the player keeps one `<audio>` and swaps segments underneath
 * a single global timeline; peaks are decoded per segment (WebAudio) once
 * and cached for the life of the component. Everything comes from the
 * device's own ledger through the existing playback bridge.
 */

type LoadedSegment = { url: string; peaks: number[] | null };

const PEAKS_PER_SECOND = 20;
const SKIP_MS = 10_000;

async function decodePeaks(bytes: Uint8Array, durationMs: number): Promise<number[] | null> {
  const Ctor = (window as unknown as { AudioContext?: typeof AudioContext; webkitAudioContext?: typeof AudioContext }).AudioContext
    ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return null;
  const context = new Ctor();
  try {
    const copy = new Uint8Array(bytes.length);
    copy.set(bytes);
    const decoded = await context.decodeAudioData(copy.buffer);
    const channel = decoded.getChannelData(0);
    const bars = Math.max(4, Math.round((durationMs / 1000) * PEAKS_PER_SECOND));
    return peaksFromSamples(channel, bars);
  } catch {
    return null;
  } finally {
    void context.close().catch(() => undefined);
  }
}

export function RecordingPlayer({ recordingId, autoPlay = false }: { recordingId: string; autoPlay?: boolean }) {
  const audio = useRef<HTMLAudioElement | null>(null);
  const segments = useRef<Map<number, LoadedSegment>>(new Map());
  const durations = useRef<number[]>([]);
  const current = useRef<number>(0); // segment index currently in the <audio>
  const wantPlaying = useRef(false);
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const dragging = useRef(false);

  const [totalMs, setTotalMs] = useState(0);
  const [positionMs, setPositionMs] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [peaks, setPeaks] = useState<number[] | null>(null);
  const [peakState, setPeakState] = useState<"loading" | "ready" | "unavailable">("loading");
  const [loadedCount, setLoadedCount] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const ensureSegment = useCallback(async (index: number): Promise<LoadedSegment> => {
    const sequence = index + 1;
    const cached = segments.current.get(sequence);
    if (cached) return cached;
    const segment = await loadPlaybackSegment(recordingId, sequence);
    if (!segment) throw new Error("การเล่นเสียงใช้ได้ในแอปที่ติดตั้งบนเครื่อง");
    const bytes = Uint8Array.from(segment.bytes);
    const url = URL.createObjectURL(new Blob([bytes], { type: segment.mimeType }));
    const loaded: LoadedSegment = { url, peaks: await decodePeaks(bytes, segment.durationMs) };
    segments.current.set(sequence, loaded);
    setLoadedCount(segments.current.size);
    return loaded;
  }, [recordingId]);

  // Manifest first (cheap), then decode peaks segment by segment in order so
  // the waveform fills in from the left while the user can already play.
  useEffect(() => {
    let cancelled = false;
    setError(null);
    setPeaks(null);
    setPeakState("loading");
    setPositionMs(0);
    setPlaying(false);
    void (async () => {
      try {
        const manifest = await playbackManifest(recordingId);
        if (cancelled) return;
        if (!manifest || manifest.segments.length === 0) {
          durations.current = [];
          setTotalMs(0);
          setPeakState("unavailable");
          setError("ยังไม่มีส่วนเสียงที่เล่นได้");
          return;
        }
        durations.current = manifest.segments.map((segment) => segment.durationMs);
        setTotalMs(manifest.durationMs);
        const merged: number[][] = [];
        let allDecoded = true;
        for (let index = 0; index < manifest.segments.length; index += 1) {
          const loaded = await ensureSegment(index);
          if (cancelled) return;
          const target = Math.max(4, Math.round((manifest.segments[index].durationMs / 1000) * PEAKS_PER_SECOND));
          if (loaded.peaks) merged.push(fitPeaks(loaded.peaks, target));
          else allDecoded = false;
        }
        if (allDecoded) {
          setPeaks(merged.flat());
          setPeakState("ready");
        } else {
          setPeakState("unavailable");
        }
      } catch (failure) {
        if (!cancelled) {
          setPeakState("unavailable");
          setError(failure instanceof Error ? failure.message : String(failure));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [recordingId, ensureSegment]);

  // One <audio> for the whole component; revoke every blob URL on unmount.
  useEffect(() => {
    const element = new Audio();
    element.preload = "auto";
    audio.current = element;
    const onTime = () => {
      const offsets = segmentOffsets(durations.current);
      setPositionMs((offsets[current.current] ?? 0) + element.currentTime * 1000);
    };
    const onEnded = () => {
      const next = current.current + 1;
      if (next < durations.current.length && wantPlaying.current) {
        void playSegment(next, 0);
      } else {
        wantPlaying.current = false;
        setPlaying(false);
        setPositionMs(durations.current.reduce((sum, value) => sum + value, 0));
      }
    };
    element.addEventListener("timeupdate", onTime);
    element.addEventListener("ended", onEnded);
    element.addEventListener("pause", () => setPlaying(!element.paused && wantPlaying.current));
    element.addEventListener("play", () => setPlaying(true));
    const cache = segments.current;
    return () => {
      element.pause();
      element.removeEventListener("timeupdate", onTime);
      element.removeEventListener("ended", onEnded);
      for (const loaded of cache.values()) URL.revokeObjectURL(loaded.url);
      cache.clear();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- element lifetime = component lifetime
  }, []);

  const playSegment = async (index: number, offsetMs: number) => {
    const element = audio.current;
    if (!element) return;
    try {
      const loaded = await ensureSegment(index);
      if (current.current !== index || element.src !== loaded.url) {
        current.current = index;
        element.src = loaded.url;
      }
      element.currentTime = Math.max(0, offsetMs) / 1000;
      if (wantPlaying.current) await element.play();
      const offsets = segmentOffsets(durations.current);
      setPositionMs((offsets[index] ?? 0) + Math.max(0, offsetMs));
    } catch (failure) {
      wantPlaying.current = false;
      setPlaying(false);
      setError(failure instanceof Error ? failure.message : String(failure));
    }
  };

  const seek = (ms: number) => {
    const target = Math.min(totalMs, Math.max(0, ms));
    const located = locateSegment(durations.current, target);
    if (!located) return;
    void playSegment(located.index, located.offsetMs);
  };

  const toggle = () => {
    const element = audio.current;
    if (!element || durations.current.length === 0) return;
    if (wantPlaying.current) {
      wantPlaying.current = false;
      element.pause();
      setPlaying(false);
      return;
    }
    wantPlaying.current = true;
    if (element.src && element.currentTime > 0 && !element.ended && positionMs < totalMs) {
      void element.play().catch(() => { wantPlaying.current = false; setPlaying(false); });
      setPlaying(true);
      return;
    }
    const located = locateSegment(durations.current, positionMs >= totalMs ? 0 : positionMs);
    void playSegment(located?.index ?? 0, located?.offsetMs ?? 0);
  };

  useEffect(() => {
    if (autoPlay && totalMs > 0 && !wantPlaying.current) {
      wantPlaying.current = true;
      void playSegment(0, 0);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- autoplay once the manifest is known
  }, [autoPlay, totalMs]);

  // Waveform + playhead.
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const width = element.clientWidth || 300;
    const height = element.clientHeight || 84;
    const ratio = window.devicePixelRatio || 1;
    if (element.width !== Math.round(width * ratio) || element.height !== Math.round(height * ratio)) {
      element.width = Math.round(width * ratio);
      element.height = Math.round(height * ratio);
    }
    const context = element.getContext("2d");
    if (!context) return;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);
    const styles = getComputedStyle(element);
    const played = styles.getPropertyValue("--m-indigo").trim() || "#6573a0";
    const rest = styles.getPropertyValue("--m-line").trim() || "rgba(255,255,255,0.2)";
    const head = styles.getPropertyValue("--m-record").trim() || "#c9553d";
    const barWidth = 3;
    const gap = 2;
    const bars = Math.max(1, Math.floor(width / (barWidth + gap)));
    // Normalised to this recording's loudest moment: a quiet room recording
    // still shows where the speech is instead of a flat line.
    const fitted = normalizePeaks(fitPeaks(peaks ?? [], bars));
    const progress = totalMs > 0 ? Math.min(1, positionMs / totalMs) : 0;
    const middle = height / 2;
    if (peaks) {
      for (let index = 0; index < bars; index += 1) {
        const value = fitted[index];
        const barHeight = Math.max(3, value * (height - 8));
        const x = index * (barWidth + gap);
        context.fillStyle = index / bars <= progress ? played : rest;
        context.beginPath();
        context.roundRect(x, middle - barHeight / 2, barWidth, barHeight, barWidth / 2);
        context.fill();
      }
    } else {
      context.strokeStyle = rest;
      context.lineWidth = 1;
      context.beginPath();
      context.moveTo(0, middle + 0.5);
      context.lineTo(width, middle + 0.5);
      context.stroke();
    }
    const headX = Math.round(progress * width);
    context.fillStyle = head;
    context.fillRect(headX - 1, 2, 2, height - 4);
  }, [peaks, positionMs, totalMs]);

  const seekFromPointer = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    seek(fraction * totalMs);
  };

  return (
    <div className="m-player" aria-label="ตัวเล่นเสียง">
      <canvas
        ref={canvas}
        className="m-player-wave"
        role="slider"
        aria-label="ตำแหน่งเล่น"
        aria-valuemin={0}
        aria-valuemax={Math.round(totalMs / 1000)}
        aria-valuenow={Math.round(positionMs / 1000)}
        onPointerDown={(event) => { dragging.current = true; event.currentTarget.setPointerCapture(event.pointerId); seekFromPointer(event); }}
        onPointerMove={(event) => { if (dragging.current) seekFromPointer(event); }}
        onPointerUp={(event) => { dragging.current = false; event.currentTarget.releasePointerCapture(event.pointerId); }}
        onPointerCancel={() => { dragging.current = false; }}
      />
      <div className="m-player-times"><span>{formatPlayerClock(positionMs)}</span><span>{peakState === "loading" && totalMs > 0 ? `กำลังอ่านข้อมูลเสียง… ${loadedCount}/${durations.current.length}` : peakState === "unavailable" && totalMs > 0 ? "ยังอ่านรูปคลื่นเสียงไม่ได้" : ""}</span><span>{formatPlayerClock(totalMs)}</span></div>
      <div className="m-player-controls">
        <button type="button" className="m-player-skip" onClick={() => seek(positionMs - SKIP_MS)} aria-label="ย้อน 10 วินาที"><RotateCcw size={22} /><small>10</small></button>
        <button type="button" className="m-player-toggle" onClick={toggle} disabled={totalMs === 0} aria-label={playing ? "หยุดชั่วคราว" : "เล่น"}>{playing ? <Pause size={28} /> : <Play size={28} />}</button>
        <button type="button" className="m-player-skip" onClick={() => seek(positionMs + SKIP_MS)} aria-label="ไปข้างหน้า 10 วินาที"><RotateCw size={22} /><small>10</small></button>
      </div>
      {error && <p className="m-player-error" role="alert">{error}</p>}
    </div>
  );
}
