"""Local speech-to-text worker for FUNG, backed by faster-whisper.

Invoked by the Rust backend as a subprocess, either with paths given directly:

    python transcribe.py <audio_or_video_path> [<audio_or_video_path> ...] [--model small] [--language th]

or (FUNGWIRE desktop worker, many segments) via a newline-delimited manifest
file, to avoid overflowing the ~32KB Windows command-line length limit that a
several-hundred-segment job's positional-argv paths could otherwise hit:

    python transcribe.py --manifest <path/to/segments.txt> [--model small] [--language th]

Accepts one or more audio/video paths so a single process (and a single
loaded Whisper model) can transcribe every segment of a job, instead of the
caller spawning one process per segment and reloading the model each time
(Final review #3). When multiple paths are given they are transcribed in
order and treated as one continuous recording: each file's segment timestamps
are offset by the cumulative REAL duration (in ms) of the files transcribed
before it, not a fixed window, so the combined transcript's timeline is
accurate regardless of each file's actual length. A single path still works
exactly as before (the offset for the only file is 0).

Add `--concat-only <output.wav>` to skip transcription entirely and instead
decode+concatenate the given path(s)/manifest into one 16kHz mono WAV file
(used by the FUNGWIRE desktop worker before a cloud STT dispatch, since cloud
APIs accept one file, unlike this script's own multi-file --manifest input).
When `faster_whisper` is unavailable, the concat path accepts only 16kHz mono
16-bit WAV inputs and uses the stdlib `wave` decoder; other formats still
require the bundled decoder.

Progress lines are written to stderr as `PROGRESS <0-100>` so the caller can
update job progress without parsing stdout; with multiple files, progress is
scaled across all of them (files-completed fraction plus within-file
fraction). The final transcript is written to stdout as a single JSON object
once processing completes, so partial stdout reads never yield invalid JSON.
"""

import argparse
import json
import os
import sys
import tempfile

def main() -> int:
    # Windows pipes stdout through the console codepage (cp1252) by default,
    # which cannot represent Thai/CJK text even when the parent redirects it.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Transcribe one or more audio/video files locally.")
    parser.add_argument(
        "audio_paths",
        nargs="*",
        help="Path(s) to the source audio or video file(s); transcribed in order as one continuous "
        "timeline. Ignored when --manifest is given.",
    )
    parser.add_argument(
        "--manifest",
        default=None,
        help="Path to a newline-delimited file of segment paths (used instead of positional "
        "audio_paths when the segment count is too large for argv, e.g. FUNGWIRE jobs).",
    )
    parser.add_argument("--model", default="small", help="faster-whisper model size or repo id")
    parser.add_argument("--language", default=None, help="Force a language code (e.g. th, en); omit to auto-detect")
    parser.add_argument("--profile", default="gpu", choices=["cpu", "gpu"])
    parser.add_argument("--device", choices=["cpu", "cuda"], help="Override the selected profile for diagnostics only")
    parser.add_argument("--compute-type", default=None)
    parser.add_argument(
        "--concat-only",
        default=None,
        metavar="OUTPUT_WAV",
        help="Skip transcription entirely; decode and concatenate the given audio path(s)/manifest "
        "into one 16kHz mono 16-bit PCM WAV at this path (FUNGWIRE cloud STT: cloud APIs take one "
        "file, unlike the local --manifest pipeline).",
    )
    args = parser.parse_args()

    if args.manifest:
        with open(args.manifest, "r", encoding="utf-8") as manifest_file:
            audio_paths = [line.strip() for line in manifest_file if line.strip()]
    else:
        audio_paths = args.audio_paths
    if not audio_paths:
        parser.error("either --manifest (non-empty) or at least one positional audio path is required")

    if args.concat_only:
        import wave

        try:
            from faster_whisper.audio import decode_audio
        except ModuleNotFoundError:
            decode_audio = None

        missing_paths = [audio_path for audio_path in audio_paths if not os.path.isfile(audio_path)]
        if missing_paths:
            parser.error(f"audio input not found: {missing_paths[0]}")

        destination = os.path.abspath(args.concat_only)
        destination_directory = os.path.dirname(destination)
        if not os.path.isdir(destination_directory):
            parser.error(f"concat output directory not found: {destination_directory}")
        if os.path.isdir(destination):
            parser.error(f"concat output path is a directory: {destination}")

        def paths_alias(first_path: str, second_path: str) -> bool:
            first_resolved = os.path.normcase(os.path.realpath(os.path.abspath(first_path)))
            second_resolved = os.path.normcase(os.path.realpath(os.path.abspath(second_path)))
            if first_resolved == second_resolved:
                return True
            try:
                return os.path.samefile(first_path, second_path)
            except OSError:
                return False

        protected_paths = list(audio_paths)
        if args.manifest:
            protected_paths.append(args.manifest)
        for protected_path in protected_paths:
            if paths_alias(destination, protected_path):
                parser.error(f"concat output must not alias an input: {protected_path}")

        def report(pct: float) -> None:
            print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

        report(1)
        total = len(audio_paths)
        temp_path = None
        try:
            temp_fd, temp_path = tempfile.mkstemp(
                prefix=f".{os.path.basename(destination)}.",
                suffix=".tmp",
                dir=destination_directory,
            )
            with os.fdopen(temp_fd, "w+b") as temp_file:
                with wave.open(temp_file, "wb") as wav_file:
                    wav_file.setnchannels(1)
                    wav_file.setsampwidth(2)  # 16-bit
                    wav_file.setframerate(16000)
                    for index, audio_path in enumerate(audio_paths):
                        if decode_audio is not None:
                            samples = decode_audio(audio_path, sampling_rate=16000)
                            pcm16 = (samples * 32767.0).clip(-32768, 32767).astype("int16")
                            wav_file.writeframes(pcm16.tobytes())
                        else:
                            with wave.open(audio_path, "rb") as source_wav:
                                if (
                                    source_wav.getnchannels() != 1
                                    or source_wav.getsampwidth() != 2
                                    or source_wav.getframerate() != 16000
                                ):
                                    raise RuntimeError(
                                        "faster_whisper is required to decode non-16kHz mono 16-bit WAV input"
                                    )
                                wav_file.writeframes(source_wav.readframes(source_wav.getnframes()))
                        report(1 + (index + 1) / total * 98)
                temp_file.flush()
                os.fsync(temp_file.fileno())
            os.replace(temp_path, destination)
            temp_path = None
        finally:
            if temp_path:
                try:
                    os.unlink(temp_path)
                except FileNotFoundError:
                    pass
        report(100)
        return 0

    from faster_whisper import WhisperModel

    device = args.device or ("cuda" if args.profile == "gpu" else "cpu")
    compute_type = args.compute_type or ("float16" if device == "cuda" else "int8")

    def report(pct: float) -> None:
        print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

    report(1)
    # Loaded once, before the file loop, regardless of how many paths were
    # given -- this is the whole point of accepting multiple paths (Final
    # review #3): the model load is by far the most expensive part of a
    # per-segment subprocess, so amortizing it across every segment of a job
    # is what makes a 1-hour recording practical instead of ~720 reloads.
    model = WhisperModel(args.model, device=device, compute_type=compute_type)

    report(5)
    total_files = len(audio_paths)
    segments = []
    cumulative_ms = 0
    detected_language = None
    detected_language_probability = None

    for file_index, audio_path in enumerate(audio_paths):
        segments_iter, info = model.transcribe(
            audio_path,
            language=args.language,
            vad_filter=True,
            word_timestamps=False,
        )
        if detected_language is None:
            detected_language = info.language
            detected_language_probability = info.language_probability

        duration_s = info.duration or 0.0
        file_offset_ms = cumulative_ms
        for segment in segments_iter:
            text = segment.text.strip()
            if not text:
                continue
            segments.append(
                {
                    "startMs": file_offset_ms + round(segment.start * 1000),
                    "endMs": file_offset_ms + round(segment.end * 1000),
                    "text": text,
                    "confidence": round(1.0 - segment.no_speech_prob, 4)
                    if segment.no_speech_prob is not None
                    else None,
                }
            )
            if duration_s > 0:
                file_fraction_done = min(1.0, segment.end / duration_s)
                overall_fraction = (file_index + file_fraction_done) / total_files
                report(5 + 93 * overall_fraction)

        cumulative_ms += round(duration_s * 1000)

    report(100)
    result = {
        "language": detected_language,
        "languageProbability": round(detected_language_probability, 4) if detected_language_probability else None,
        "durationMs": cumulative_ms,
        "segments": segments,
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
