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
    * Segment timestamps are relative to the *chunk* or requested rolling
      window; the caller applies the corresponding source-time offset.
  * `condition_on_previous_text` stays False: chunks are independent files, so
    carrying decoder state across them would smear text over chunk borders.

Progress lines are not used here (chunks are short); stderr carries only
diagnostics and is drained by the caller.
"""

import argparse
import json
import os
import sys
import tempfile
import wave


MAX_WINDOW_MS = 4_000
MAX_WINDOW_FRAGMENTS = 3
MAX_WINDOW_FILE_BYTES = 32 * 1024 * 1024


def _valid_integer(value, field):
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError(f"{field} must be an integer")
    return value


def materialize_decode_window(window):
    """Join bounded, finalized WAV slices into one temporary decode window."""
    if not isinstance(window, dict):
        raise ValueError("window must be an object")
    window_start = _valid_integer(window.get("startMs"), "window.startMs")
    window_end = _valid_integer(window.get("endMs"), "window.endMs")
    if window_start < 0 or window_end <= window_start or window_end - window_start > MAX_WINDOW_MS:
        raise ValueError("window range is invalid or exceeds the 4-second limit")
    fragments = window.get("fragments")
    if not isinstance(fragments, list) or not fragments or len(fragments) > MAX_WINDOW_FRAGMENTS:
        raise ValueError("window fragment count is invalid")

    cursor_ms = window_start
    sample_rate = None
    payloads = []
    total_bytes = 0
    parent_dir = None
    for index, fragment in enumerate(fragments):
        if not isinstance(fragment, dict):
            raise ValueError("window fragment must be an object")
        path_value = fragment.get("path")
        if not isinstance(path_value, str) or not os.path.isabs(path_value) or "://" in path_value:
            raise ValueError("window fragment path must be an absolute local path")
        path = os.path.realpath(path_value)
        if not os.path.isfile(path) or os.path.getsize(path) > MAX_WINDOW_FILE_BYTES:
            raise ValueError("window fragment file is missing or exceeds the size limit")
        current_parent = os.path.dirname(path)
        if parent_dir is None:
            parent_dir = current_parent
        elif current_parent != parent_dir:
            raise ValueError("window fragments must share one local custody directory")

        fragment_start = _valid_integer(fragment.get("fragmentStartMs"), "fragment.fragmentStartMs")
        fragment_end = _valid_integer(fragment.get("fragmentEndMs"), "fragment.fragmentEndMs")
        clip_start = _valid_integer(fragment.get("clipStartMs"), "fragment.clipStartMs")
        clip_end = _valid_integer(fragment.get("clipEndMs"), "fragment.clipEndMs")
        if fragment_start < 0 or fragment_end <= fragment_start or fragment_end - fragment_start > 2_000:
            raise ValueError("fragment source range is invalid")
        if clip_start != cursor_ms or clip_end <= clip_start or clip_start < fragment_start or clip_end > fragment_end:
            raise ValueError("window fragments are non-contiguous or outside source custody")
        if clip_end > window_end:
            raise ValueError("window fragment extends beyond the requested window")

        with wave.open(path, "rb") as source:
            if source.getnchannels() != 1 or source.getsampwidth() != 2:
                raise ValueError("window audio must be mono 16-bit PCM")
            if sample_rate is None:
                sample_rate = source.getframerate()
            elif sample_rate != source.getframerate():
                raise ValueError("window fragments use different sample rates")
            frame_count = source.getnframes()
            if frame_count <= 0:
                raise ValueError("window fragment contains no audio frames")
            start_frame = round((clip_start - fragment_start) * sample_rate / 1000)
            end_frame = round((clip_end - fragment_start) * sample_rate / 1000)
            if start_frame < 0 or end_frame <= start_frame or end_frame > frame_count:
                raise ValueError("window clip range is outside the WAV frame bounds")
            source.rewind()
            source.readframes(start_frame)
            payload = source.readframes(end_frame - start_frame)
        expected_bytes = (end_frame - start_frame) * 2
        if len(payload) != expected_bytes:
            raise ValueError("window fragment ended before its declared audio range")
        total_bytes += len(payload)
        if total_bytes > MAX_WINDOW_FILE_BYTES:
            raise ValueError("combined decode window exceeds the size limit")
        payloads.append(payload)
        cursor_ms = clip_end

    if cursor_ms != window_end:
        raise ValueError("window audio does not cover the requested range")
    if not sample_rate:
        raise ValueError("window audio sample rate is invalid")

    temporary = tempfile.NamedTemporaryFile(
        prefix="fung-live-window-",
        suffix=".wav",
        dir=parent_dir,
        delete=False,
    )
    temp_path = temporary.name
    temporary.close()
    try:
        with wave.open(temp_path, "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(sample_rate)
            output.writeframes(b"".join(payloads))
    except Exception:
        try:
            os.unlink(temp_path)
        except OSError:
            pass
        raise
    return temp_path

DEFAULT_MODEL = "large-v3-turbo"


def default_compute_type(model: str, device: str) -> str:
    configured = os.environ.get("FUNG_TRANSCRIPTION_COMPUTE_TYPE")
    if configured:
        return configured
    if device == "cpu":
        return "int8"
    model_name = os.path.basename(os.path.normpath(model)).lower()
    if model_name == "medium":
        return "int8_float16"
    return "float16"


def main() -> int:
    # Windows pipes default to the console codepage (cp1252), which cannot
    # carry Thai text — same fix as transcribe.py.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    sys.stdin.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Persistent chunk transcriber for live meetings.")
    parser.add_argument(
        "--model",
        default=os.environ.get("FUNG_WHISPER_MODEL", DEFAULT_MODEL),
        help="faster-whisper model size, repo id, or bundled local model path",
    )
    parser.add_argument("--language", default=None, help="Force a language code (e.g. th, en); omit to auto-detect")
    parser.add_argument(
        "--profile",
        default=os.environ.get("FUNG_TRANSCRIPTION_PROFILE", "cpu"),
        choices=["cpu", "gpu"],
    )
    args = parser.parse_args()

    from faster_whisper import WhisperModel

    device = "cuda" if args.profile == "gpu" else "cpu"
    compute_type = default_compute_type(args.model, device)

    model = WhisperModel(args.model, device=device, compute_type=compute_type)
    print(
        json.dumps(
            {
                "ready": True,
                "model": args.model,
                "device": device,
                "computeType": compute_type,
                "backend": "faster-whisper",
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

        chunk_id = request.get("id")
        chunk_path = request.get("path")
        response = {
            "id": chunk_id,
            "channel": request.get("channel"),
            "startMs": request.get("startMs", 0),
        }
        temporary_path = None
        try:
            window = request.get("window")
            if window is not None:
                temporary_path = materialize_decode_window(window)
                audio_path = temporary_path
                response["windowId"] = window.get("id")
                response["windowStartMs"] = window.get("startMs")
                response["windowEndMs"] = window.get("endMs")
                response["finalWindow"] = bool(window.get("final", False))
            else:
                audio_path = request.get("path")
            segments_iter, info = model.transcribe(
                audio_path,
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
        finally:
            if temporary_path is not None:
                try:
                    os.unlink(temporary_path)
                except OSError as error:
                    print(f"could not remove temporary decode window: {error}", file=sys.stderr)
        print(json.dumps(response, ensure_ascii=False), flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main())
