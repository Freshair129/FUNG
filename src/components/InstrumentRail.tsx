import { useId } from "react";
import { Circle, Download, Play, Settings, Upload, Wifi } from "lucide-react";
import "./InstrumentRail.css";

interface InstrumentRailProps {
  recording: boolean;
  onRecord: () => void;
  onImport: () => void;
  importDisabled: boolean;
  onExport: () => void;
  exportTitle: string;
  onPairDevice: () => void;
  onOpenSettings: () => void;
}

// Right edge is a constant x=74 for the full height. The left edge steps
// from x=0 (upper zone, wider) to x=10 (lower zone, narrower/stepped in) via
// a single sweep=0 arc at local y:320-340 — the same relative direction and
// sweep as the app's own panel notch (PANEL_PATH in App.tsx), whose seam
// sits at canonical panel-space y:330-350. InstrumentRail renders as a
// sibling of .panel-glass (stage space), while PANEL_PATH is drawn inside
// .panel-rim at `inset: 12px` (panel space) — so panel-space y maps to
// stage-space y+12, and this rail's own `top: 22px` subtracts again to get
// local y. Seam: (330+12-22)=320 to (350+12-22)=340.
const NOTCH_PATH =
  "M 16,0 H 58 A 16 16 0 0 1 74,16 V 622 A 16 16 0 0 1 58,638 H 26 A 16 16 0 0 1 10,622 V 340 A 16 16 0 0 0 0,320 V 16 A 16 16 0 0 1 16,0 Z";

// There is no real input-level source wired up in the desktop frontend, so
// this renders a visibly inactive meter (all segments dark, reduced
// opacity) instead of one that looks live but is permanently stuck at 0.
function VuBar({ id }: { id: string }) {
  const segments = 6;
  const segEls = [];
  for (let i = 0; i < segments; i += 1) {
    segEls.push(
      <div
        key={`${id}-${i}`}
        className="instrument-rail__bar-seg"
        style={{ height: `${100 / segments}%`, background: "transparent" }}
      />,
    );
  }
  return <div className="instrument-rail__bar instrument-rail__bar--inactive">{segEls}</div>;
}

export function InstrumentRail({
  recording,
  onRecord,
  onImport,
  importDisabled,
  onExport,
  exportTitle,
  onPairDevice,
  onOpenSettings,
}: InstrumentRailProps) {
  const gradientId = useId();

  return (
    <div className="instrument-rail no-drag" aria-label="Instrument rail">
      <svg className="instrument-rail__shape" viewBox="0 0 74 638" aria-hidden="true">
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="var(--rail-fill-start, #fffdf7)" />
            <stop offset="100%" stopColor="var(--rail-fill-end, #e5dac7)" />
          </linearGradient>
        </defs>
        <path d={NOTCH_PATH} fill={`url(#${gradientId})`} />
        <path d={NOTCH_PATH} className="instrument-rail__stroke-outer" />
        <path d={NOTCH_PATH} className="instrument-rail__stroke-inner" />
      </svg>
      <div className="instrument-rail__content">
        <div className="instrument-rail__vu" aria-label="Input level meter unavailable" title="No live input level source in this build">
          <VuBar id="vu-l" />
          <VuBar id="vu-r" />
        </div>
        <div className="instrument-rail__vu-label">L&nbsp;&nbsp;R</div>

        <div className="instrument-rail__buttons">
          <button
            type="button"
            className={`instrument-rail__button ${recording ? "is-active" : ""}`}
            aria-label={recording ? "Pause recording" : "Start recording"}
            onClick={onRecord}
          >
            <Circle size={18} fill={recording ? "currentColor" : "none"} />
          </button>
          <button
            type="button"
            className="instrument-rail__button"
            aria-label="Import audio"
            title="Import audio or video and transcribe locally"
            disabled={importDisabled}
            onClick={onImport}
          >
            <Upload size={18} />
          </button>
          <button
            type="button"
            className="instrument-rail__button"
            aria-label="Playback unavailable"
            title="No local playback in this desktop build"
            disabled
          >
            <Play size={18} />
          </button>
          <button
            type="button"
            className="instrument-rail__button"
            aria-label="Export subtitles"
            title={exportTitle}
            onClick={onExport}
          >
            <Download size={18} />
          </button>
          <button type="button" className="instrument-rail__button" aria-label="Pair device" onClick={onPairDevice}>
            <Wifi size={18} />
          </button>
          <button type="button" className="instrument-rail__button" aria-label="Settings" onClick={onOpenSettings}>
            <Settings size={18} />
          </button>
        </div>
      </div>
    </div>
  );
}
