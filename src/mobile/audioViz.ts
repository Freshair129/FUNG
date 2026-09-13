/**
 * Pure helpers behind the two audio visualisations on mobile: the scrolling
 * live waveform while recording (a rolling history of input levels) and the
 * playback waveform with a playhead (peaks reduced from decoded audio).
 * No DOM, no Tauri — `tests/audioViz.test.mjs` covers them directly.
 */

/** Perceptual scaling for a 0-100 amplitude: quiet speech reads as ~40-60
 * instead of a barely visible sliver, loud input still tops out at 100. */
export function shapeLevel(percent: number): number {
  const clamped = Math.min(100, Math.max(0, Number.isFinite(percent) ? percent : 0));
  return Math.round(Math.sqrt(clamped / 100) * 100);
}

/** Appends one level sample to a rolling history, dropping the oldest past
 * `max`. Returns a new array so React state updates see a change. */
export function pushLevel(history: readonly number[], level: number, max: number): number[] {
  const next = history.length >= max ? history.slice(history.length - max + 1) : history.slice();
  next.push(level);
  return next;
}

/** Reduces PCM samples to `bars` peaks in 0..1 (max |sample| per bucket).
 * Peak rather than RMS: the playback waveform is for spotting where speech
 * is, and peaks keep short words visible. */
export function peaksFromSamples(samples: ArrayLike<number>, bars: number): number[] {
  if (bars <= 0 || samples.length === 0) return [];
  const bucket = samples.length / bars;
  const peaks: number[] = new Array(bars);
  for (let index = 0; index < bars; index += 1) {
    const start = Math.floor(index * bucket);
    const end = Math.min(samples.length, Math.max(start + 1, Math.floor((index + 1) * bucket)));
    let peak = 0;
    for (let cursor = start; cursor < end; cursor += 1) {
      const magnitude = Math.abs(samples[cursor]);
      if (magnitude > peak) peak = magnitude;
    }
    peaks[index] = Math.min(1, peak);
  }
  return peaks;
}

/** Resamples a peak array to exactly `bars` entries (max over each span), so
 * per-segment peak arrays can be concatenated and then fitted to a canvas. */
export function fitPeaks(peaks: readonly number[], bars: number): number[] {
  if (bars <= 0) return [];
  if (peaks.length === 0) return new Array(bars).fill(0);
  const out: number[] = new Array(bars);
  const span = peaks.length / bars;
  for (let index = 0; index < bars; index += 1) {
    const start = Math.floor(index * span);
    const end = Math.min(peaks.length, Math.max(start + 1, Math.floor((index + 1) * span)));
    let peak = 0;
    for (let cursor = start; cursor < end; cursor += 1) peak = Math.max(peak, peaks[cursor]);
    out[index] = peak;
  }
  return out;
}

/** Which segment a global position falls in, and the offset inside it. The
 * last segment absorbs positions at or past the end so seeking to the very
 * end never yields an out-of-range index. */
export function locateSegment(durationsMs: readonly number[], positionMs: number): { index: number; offsetMs: number } | null {
  if (durationsMs.length === 0) return null;
  const target = Math.max(0, positionMs);
  let elapsed = 0;
  for (let index = 0; index < durationsMs.length; index += 1) {
    const duration = Math.max(0, durationsMs[index]);
    if (target < elapsed + duration) return { index, offsetMs: target - elapsed };
    elapsed += duration;
  }
  const last = durationsMs.length - 1;
  return { index: last, offsetMs: Math.max(0, durationsMs[last]) };
}

/** Global start offset of each segment, in ms. */
export function segmentOffsets(durationsMs: readonly number[]): number[] {
  const offsets: number[] = [];
  let elapsed = 0;
  for (const duration of durationsMs) {
    offsets.push(elapsed);
    elapsed += Math.max(0, duration);
  }
  return offsets;
}

export function formatPlayerClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

/** Scales peaks to the recording's own loudest moment and lifts them
 * perceptually (sqrt), so a quiet room recording still shows its shape
 * instead of a flat line; silence stays flat. */
export function normalizePeaks(peaks: readonly number[]): number[] {
  let max = 0;
  for (const value of peaks) if (value > max) max = value;
  if (max <= 0.0005) return peaks.map(() => 0);
  return peaks.map((value) => Math.sqrt(Math.min(1, Math.max(0, value) / max)));
}
