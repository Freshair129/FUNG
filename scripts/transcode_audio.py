"""Decode a project-owned audio file and write a local WAV or MP3 export.

This helper is launched by FUNG's bundled Python runtime through
``run_python_worker``. It never fetches codecs or media, and it only writes the
explicit output path supplied by the native caller.
"""

from __future__ import annotations

import argparse
import io
import sys
from pathlib import Path

MAX_OUTPUT_BYTES = 512 * 1024 * 1024


class BoundedOutput(io.FileIO):
    """Enforce the export budget on every mux write, including header rewrites."""
    def __init__(self, path: Path, max_bytes: int):
        if not 0 < max_bytes <= MAX_OUTPUT_BYTES:
            raise ValueError("invalid audio export byte ceiling")
        self.max_bytes = max_bytes
        super().__init__(path, "w+b")

    def write(self, data):
        if self.tell() + len(data) > self.max_bytes:
            raise RuntimeError("audio export exceeds byte ceiling")
        return super().write(data)

    def truncate(self, size=None):
        size = self.tell() if size is None else size
        if size > self.max_bytes:
            raise RuntimeError("audio export exceeds byte ceiling")
        return super().truncate(size)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Transcode one local audio file for FUNG export.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--format", required=True, choices=("wav", "mp3"))
    parser.add_argument("--max-output-bytes", type=int, default=MAX_OUTPUT_BYTES)
    return parser


def transcode(input_path: Path, output_path: Path, output_format: str, *, max_output_bytes: int = MAX_OUTPUT_BYTES) -> None:
    import av

    audio_format = "wav" if output_format == "wav" else "mp3"
    codec = "pcm_s16le" if output_format == "wav" else "libmp3lame"
    sample_rate = 16_000 if output_format == "wav" else 44_100

    output_path.parent.mkdir(parents=True, exist_ok=True)
    temporary_path = output_path.with_name(f".{output_path.name}.tmp")
    temporary_path.unlink(missing_ok=True)
    try:
        with av.open(str(input_path), mode="r") as source:
            stream = next((candidate for candidate in source.streams if candidate.type == "audio"), None)
            if stream is None:
                raise RuntimeError("source has no audio stream")

            resampler = av.audio.resampler.AudioResampler(
                format="s16",
                layout="mono",
                rate=sample_rate,
            )
            with BoundedOutput(temporary_path, max_output_bytes) as bounded, av.open(bounded, mode="w", format=audio_format) as destination:
                output_stream = destination.add_stream(codec, rate=sample_rate)
                output_stream.layout = "mono"

                for frame in source.decode(stream):
                    for resampled in resampler.resample(frame):
                        for packet in output_stream.encode(resampled):
                            destination.mux(packet)

                for resampled in resampler.resample(None):
                    for packet in output_stream.encode(resampled):
                        destination.mux(packet)
                for packet in output_stream.encode():
                    destination.mux(packet)

        if not temporary_path.is_file() or temporary_path.stat().st_size == 0:
            raise RuntimeError("encoder produced an empty output")
        temporary_path.replace(output_path)
    except Exception:
        temporary_path.unlink(missing_ok=True)
        raise


def main() -> int:
    args = _parser().parse_args()
    try:
        transcode(args.input, args.output, args.format, max_output_bytes=args.max_output_bytes)
    except Exception as error:  # pragma: no cover - exercised by the runtime smoke
        print(f"audio transcode failed: {error}", file=sys.stderr)
        return 1
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
