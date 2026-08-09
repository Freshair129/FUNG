"""Stub worker for FUNGWIRE job-loop tests.

Stands in for `scripts/transcribe.py` so tests don't need the real
faster-whisper model/venv pipeline. Mimics the real script's call contract as
of Final-review fix wave C (threaded progress + manifest file, not argv): it
accepts EITHER one-or-more positional audio paths (`nargs="*"`, matching
transcribe.py) OR a `--manifest <path>` option pointing at a newline-delimited
file of paths (used instead of positionals when given -- this is what the
FUNGWIRE desktop worker now always uses, to avoid overflowing the Windows
argv length limit for many-segment jobs), plus `--profile`/`--model`/
`--language`/`--device`/`--compute-type`, all accepted but unused.

It prints two `PROGRESS <pct>` lines on stderr, separated by a short sleep,
before printing the final combined JSON object (matching the `WhisperOutput`
shape) on stdout -- proving (in
`fungwire_server::tests::transcribing_progress_is_streamed_before_result`)
that a real mid-transcription percent gets forwarded to the client as a
`Control::Progress{stage:"transcribing"}` frame, not just a single 100% sent
after the fact. The sleep is a few hundred milliseconds, never anywhere close
to `TRANSCRIBE_KEEPALIVE_INTERVAL` (20s), so tests stay fast.

For each path, in order, it emits exactly one deterministic 1000ms segment
with text "hi", offset by the cumulative duration of the paths before it --
path 0 -> [0, 1000), path 1 -> [1000, 2000), path 2 -> [2000, 3000), etc. --
so tests get a known, per-file-offset result without needing real audio, a
model, or a GPU/CPU device.
"""

import argparse
import json
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument("audio_paths", nargs="*")
parser.add_argument("--manifest", default=None)
parser.add_argument("--model", default="small")
parser.add_argument("--language", default=None)
parser.add_argument("--profile", default="gpu", choices=["cpu", "gpu"])
parser.add_argument("--device", choices=["cpu", "cuda"])
parser.add_argument("--compute-type", default=None)
args = parser.parse_args()

if args.manifest:
    with open(args.manifest, "r", encoding="utf-8") as manifest_file:
        audio_paths = [line.strip() for line in manifest_file if line.strip()]
else:
    audio_paths = args.audio_paths

print("PROGRESS 30", file=sys.stderr, flush=True)
time.sleep(0.2)
print("PROGRESS 70", file=sys.stderr, flush=True)

segments = []
for i, _path in enumerate(audio_paths):
    offset_ms = i * 1000
    segments.append(
        {
            "startMs": offset_ms,
            "endMs": offset_ms + 1000,
            "text": "hi",
            "confidence": 0.9,
        }
    )

result = {
    "durationMs": len(audio_paths) * 1000,
    "segments": segments,
}
print(json.dumps(result))
