"""Persistent JSONL worker for the opt-in Transformers Thai Whisper candidate."""

import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from transcribe_transformers import decode_audio, load_pipeline, transcribe_audio


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    sys.stdin.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Persistent Transformers Thai Whisper candidate worker.")
    parser.add_argument(
        "--model",
        default=os.environ.get("FUNG_WHISPER_MODEL", "whisper-th-large-combined"),
        help="Local Transformers model directory",
    )
    parser.add_argument("--language", default=None, help="Force a language code such as th")
    parser.add_argument("--profile", default=os.environ.get("FUNG_TRANSCRIPTION_PROFILE", "cpu"), choices=["cpu", "gpu"])
    args = parser.parse_args()

    asr_pipeline = load_pipeline(args.model, args.profile)
    print(
        json.dumps(
            {
                "ready": True,
                "model": args.model,
                "backend": "transformers",
                "device": "cuda" if args.profile == "gpu" else "cpu",
            },
            ensure_ascii=False,
        ),
        flush=True,
    )

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

        response = {
            "id": request.get("id"),
            "channel": request.get("channel"),
            "startMs": request.get("startMs", 0),
        }
        try:
            chunk_path = request.get("path")
            audio = decode_audio(chunk_path)
            duration_s = len(audio) / 16000.0
            response["segments"] = transcribe_audio(asr_pipeline, audio, args.language, duration_s)
            response["language"] = args.language
        except Exception as error:  # noqa: BLE001 — one bad chunk must not end a live session
            response["error"] = str(error)
        print(json.dumps(response, ensure_ascii=False), flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main())
