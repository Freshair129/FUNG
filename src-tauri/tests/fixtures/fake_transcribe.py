"""Stub worker for FUNGWIRE job-loop tests.

Stands in for `scripts/transcribe.py` so tests don't need the real
faster-whisper model/venv pipeline. Mimics the real script's call contract as
of Final-review fix wave A1 (single multi-path invocation, model loaded
once): it accepts one-or-more positional audio paths (`nargs="+"`, matching
transcribe.py) plus `--profile`/`--model`/`--language`/`--device`/
`--compute-type`, all accepted but unused, and prints `PROGRESS <pct>` lines
on stderr followed by a single combined JSON object (matching the
`WhisperOutput` shape) on stdout.

For each positional path, in order, it emits exactly one deterministic
1000ms segment with text "hi", offset by the cumulative duration of the
paths before it -- path 0 -> [0, 1000), path 1 -> [1000, 2000), path 2 ->
[2000, 3000), etc. -- so tests get a known, per-file-offset result without
needing real audio, a model, or a GPU/CPU device.
"""

import argparse
import json
import sys

parser = argparse.ArgumentParser()
parser.add_argument("audio_paths", nargs="+")
parser.add_argument("--model", default="small")
parser.add_argument("--language", default=None)
parser.add_argument("--profile", default="gpu", choices=["cpu", "gpu"])
parser.add_argument("--device", choices=["cpu", "cuda"])
parser.add_argument("--compute-type", default=None)
args = parser.parse_args()

print("PROGRESS 50", file=sys.stderr)

segments = []
for i, _path in enumerate(args.audio_paths):
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
    "durationMs": len(args.audio_paths) * 1000,
    "segments": segments,
}
print(json.dumps(result))
