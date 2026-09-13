import assert from "node:assert/strict";
import test from "node:test";
import { fitPeaks, formatPlayerClock, locateSegment, normalizePeaks, peaksFromSamples, pushLevel, segmentOffsets, shapeLevel } from "../src/mobile/audioViz.ts";

test("level history is a bounded rolling window that never mutates its input", () => {
  const start = [10, 20, 30];
  const next = pushLevel(start, 40, 3);
  assert.deepEqual(next, [20, 30, 40]);
  assert.deepEqual(start, [10, 20, 30]);
  assert.deepEqual(pushLevel([], 5, 3), [5]);
  assert.deepEqual(pushLevel([1, 2], 3, 3), [1, 2, 3]);
});

test("shapeLevel lifts quiet input into a visible range and clamps the rest", () => {
  assert.equal(shapeLevel(0), 0);
  assert.equal(shapeLevel(25), 50);
  assert.equal(shapeLevel(100), 100);
  assert.equal(shapeLevel(250), 100);
  assert.equal(shapeLevel(-5), 0);
  assert.equal(shapeLevel(Number.NaN), 0);
});

test("peaks are per-bucket maxima in 0..1 and fit to any bar count", () => {
  // Values chosen to be exactly representable in float32, since decoded
  // audio arrives as Float32Array and the peaks must be its own numbers.
  const samples = new Float32Array([0, 0.5, -1, 0.125, 0.25, 0.375, 0, 0]);
  assert.deepEqual(peaksFromSamples(samples, 4), [0.5, 1, 0.375, 0]);
  assert.deepEqual(peaksFromSamples(samples, 0), []);
  assert.deepEqual(peaksFromSamples([], 3), []);
  assert.deepEqual(fitPeaks([0.5, 1, 0.3, 0], 2), [1, 0.3]);
  assert.deepEqual(fitPeaks([0.2], 3), [0.2, 0.2, 0.2], "upsampling repeats");
  assert.deepEqual(fitPeaks([], 2), [0, 0]);
});

test("a global position maps to the right segment and offset", () => {
  const durations = [5000, 4000, 1000];
  assert.deepEqual(segmentOffsets(durations), [0, 5000, 9000]);
  assert.deepEqual(locateSegment(durations, 0), { index: 0, offsetMs: 0 });
  assert.deepEqual(locateSegment(durations, 4999), { index: 0, offsetMs: 4999 });
  assert.deepEqual(locateSegment(durations, 5000), { index: 1, offsetMs: 0 });
  assert.deepEqual(locateSegment(durations, 9500), { index: 2, offsetMs: 500 });
  assert.deepEqual(locateSegment(durations, 10000), { index: 2, offsetMs: 1000 }, "end clamps into the last segment");
  assert.deepEqual(locateSegment(durations, 99999), { index: 2, offsetMs: 1000 });
  assert.deepEqual(locateSegment(durations, -50), { index: 0, offsetMs: 0 });
  assert.equal(locateSegment([], 0), null);
});

test("player clock is m:ss", () => {
  assert.equal(formatPlayerClock(0), "0:00");
  assert.equal(formatPlayerClock(65_000), "1:05");
  assert.equal(formatPlayerClock(3_599_999), "59:59");
});

test("peaks are normalised to the recording's own maximum and lifted, silence stays flat", () => {
  assert.deepEqual(normalizePeaks([0.02, 0.01, 0.005]), [1, Math.sqrt(0.5), Math.sqrt(0.25)]);
  assert.deepEqual(normalizePeaks([0, 0, 0]), [0, 0, 0]);
  assert.deepEqual(normalizePeaks([]), []);
  assert.deepEqual(normalizePeaks([1]), [1]);
});
