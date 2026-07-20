"""Local speech-to-text worker for FUNG, backed by faster-whisper.

Invoked by the Rust backend as a subprocess:

    python transcribe.py <audio_or_video_path> [--model small] [--language th]

Progress lines are written to stderr as `PROGRESS <0-100>` so the caller can
update job progress without parsing stdout. The final transcript is written
to stdout as a single JSON object once processing completes, so partial
stdout reads never yield invalid JSON.
"""

import argparse
import json
import sys

from faster_whisper import WhisperModel


def main() -> int:
    # Windows pipes stdout through the console codepage (cp1252) by default,
    # which cannot represent Thai/CJK text even when the parent redirects it.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Transcribe an audio/video file locally.")
    parser.add_argument("audio_path", help="Path to the source audio or video file")
    parser.add_argument("--model", default="small", help="faster-whisper model size or repo id")
    parser.add_argument("--language", default=None, help="Force a language code (e.g. th, en); omit to auto-detect")
    parser.add_argument("--profile", default="gpu", choices=["cpu", "gpu"])
    parser.add_argument("--device", choices=["cpu", "cuda"], help="Override the selected profile for diagnostics only")
    parser.add_argument("--compute-type", default=None)
    args = parser.parse_args()

    device = args.device or ("cuda" if args.profile == "gpu" else "cpu")
    compute_type = args.compute_type or ("float16" if device == "cuda" else "int8")

    def report(pct: float) -> None:
        print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

    report(1)
    model = WhisperModel(args.model, device=device, compute_type=compute_type)

    report(5)
    segments_iter, info = model.transcribe(
        args.audio_path,
        language=args.language,
        vad_filter=True,
        word_timestamps=False,
    )

    duration_s = info.duration or 0.0
    segments = []
    for segment in segments_iter:
        text = segment.text.strip()
        if not text:
            continue
        segments.append(
            {
                "startMs": round(segment.start * 1000),
                "endMs": round(segment.end * 1000),
                "text": text,
                "confidence": round(1.0 - segment.no_speech_prob, 4)
                if segment.no_speech_prob is not None
                else None,
            }
        )
        if duration_s > 0:
            report(5 + 93 * min(1.0, segment.end / duration_s))

    report(100)
    result = {
        "language": info.language,
        "languageProbability": round(info.language_probability, 4) if info.language_probability else None,
        "durationMs": round(duration_s * 1000),
        "segments": segments,
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
