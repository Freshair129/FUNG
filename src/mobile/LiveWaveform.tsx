import { useEffect, useRef } from "react";

/**
 * Scrolling live waveform for the capture screen: each level sample (0-100,
 * already perceptually shaped) becomes one bar, newest at the right, so the
 * picture moves with the actual input the way a phone voice-memo recorder
 * does. Nothing is drawn when there is no sample — a still line, not fake
 * motion.
 */
export function LiveWaveform({ history, active }: { history: readonly number[]; active: boolean }) {
  const canvas = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const width = element.clientWidth || 300;
    const height = element.clientHeight || 72;
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
    const barColor = styles.getPropertyValue(active ? "--m-record" : "--m-sage").trim() || "#c9553d";
    const lineColor = styles.getPropertyValue("--m-line").trim() || "rgba(255,255,255,0.15)";
    const barWidth = 3;
    const gap = 2;
    const step = barWidth + gap;
    const middle = height / 2;

    context.strokeStyle = lineColor;
    context.lineWidth = 1;
    context.beginPath();
    context.moveTo(0, middle + 0.5);
    context.lineTo(width, middle + 0.5);
    context.stroke();

    const capacity = Math.floor(width / step);
    const visible = history.slice(Math.max(0, history.length - capacity));
    context.fillStyle = barColor;
    for (let index = 0; index < visible.length; index += 1) {
      const level = Math.min(100, Math.max(0, visible[index]));
      const barHeight = Math.max(2, (level / 100) * (height - 6));
      const x = width - (visible.length - index) * step;
      const y = middle - barHeight / 2;
      context.beginPath();
      context.roundRect(x, y, barWidth, barHeight, barWidth / 2);
      context.fill();
    }
  }, [history, active]);

  return <canvas ref={canvas} className={`m-live-wave ${active ? "is-active" : ""}`} aria-hidden="true" />;
}
