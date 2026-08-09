"""Stub worker for FUNGWIRE job-loop tests (Task 7).

Stands in for `scripts/transcribe.py` so tests don't need the real
faster-whisper model/venv pipeline. Mimics the same stdout/stderr contract
`run_python_worker` expects: `PROGRESS <pct>` lines on stderr, then a single
JSON object (matching the `WhisperOutput` shape) on stdout. Ignores its
arguments entirely -- the segment path and --profile flag are accepted (so
the real call shape doesn't error) but not used.
"""

import sys

print("PROGRESS 50", file=sys.stderr)
print('{"durationMs":1000,"segments":[{"startMs":0,"endMs":1000,"text":"hi","confidence":0.9}]}')
