"""Persistent live-transcription worker for FUNG Meeting Mode.

Unlike `transcribe.py` (one-shot: argv in, JSON out, exit), this worker stays
alive for a whole live session so the Whisper model is loaded into GPU memory
exactly once. The Rust coordinator speaks newline-delimited JSON over
stdin/stdout:

    -> {"id": "<chunk-id>", "path": "C:/.../mic-00001.wav", "channel": "mic", "startMs": 8000}
    <- {"id": "<chunk-id>", "channel": "mic", "startMs": 8000,
        "segments": [{"startMs": 120, "endMs": 3400, "text": "...", "confidence": 0.97}],
        "language": "th"}
    -> {"cmd": "shutdown"}

Contract details the coordinator relies on:
  * Exactly one response line per request line, in request order.
  * A worker-level problem with one chunk is reported as
    {"id": ..., "error": "..."} on stdout — the process keeps running.
  * `{"ready": true, "model": ..., "device": ...}` is printed once after the
    model finishes loading, before any request is answered.
  * Segment timestamps are relative to the *chunk*; the caller adds the
    chunk's session offset (`startMs` is echoed back untouched for that).
  * `condition_on_previous_text` stays False: chunks are independent files, so
    carrying decoder state across them would smear text over chunk borders.

Progress lines are not used here (chunks are short); stderr carries only
diagnostics and is drained by the caller.
"""

import argparse
import json
import os
import sys

from faster_whisper import WhisperModel


def main() -> int:
    # Windows pipes default to the console codepage (cp1252), which cannot
    # carry Thai text — same fix as transcribe.py.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    sys.stdin.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Persistent chunk transcriber for live meetings.")
    parser.add_argument(
        "--model",
        default=os.environ.get("FUNG_WHISPER_MODEL", "small"),
        help="faster-whisper model size, repo id, or bundled local model path",
    )
    parser.add_argument("--language", default=None, help="Force a language code (e.g. th, en); omit to auto-detect")
    parser.add_argument(
        "--profile",
        default=os.environ.get("FUNG_TRANSCRIPTION_PROFILE", "cpu"),
        choices=["cpu", "gpu"],
    )
    args = parser.parse_args()

    device = "cuda" if args.profile == "gpu" else "cpu"
    compute_type = "float16" if device == "cuda" else "int8"

    model = WhisperModel(args.model, device=device, compute_type=compute_type)
    print(json.dumps({"ready": True, "model": args.model, "device": device}, ensure_ascii=False), flush=True)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as error:
            print(json.dumps({"error": f"bad request line: {error}"}, ensure_ascii=False), flush=True)
            continue

        if request.get("cmd") == "shutdown":
            break

        chunk_id = request.get("id")
        chunk_path = request.get("path")
        response = {
            "id": chunk_id,
            "channel": request.get("channel"),
            "startMs": request.get("startMs", 0),
        }
        try:
            segments_iter, info = model.transcribe(
                chunk_path,
                language=args.language,
                vad_filter=True,
                word_timestamps=False,
                condition_on_previous_text=False,
            )
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
            response["segments"] = segments
            response["language"] = info.language
        except Exception as error:  # noqa: BLE001 — one bad chunk must not kill the session
            response["error"] = str(error)
        print(json.dumps(response, ensure_ascii=False), flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main())
