"""Local speaker-diarization worker for FUNG, backed by pyannote-audio.

Invoked by the Rust backend as a subprocess:

    python diarize.py <audio_path> [--model pyannote/speaker-diarization-3.1]

The pyannote pipeline weights are gated on Hugging Face: the user must
accept the model license and expose a token via FUNG_HF_TOKEN (or HF_TOKEN)
for the FIRST download; afterwards the cached model works offline.
"""

import argparse
import json
import os
import sys


def load_pipeline(model: str, token: str | None):
    from pyannote.audio import Pipeline

    return Pipeline.from_pretrained(model, token=token)


def run_pipeline(pipeline, audio_path: str, torch):
    """Decode locally and use pyannote's in-memory waveform input contract."""
    from faster_whisper.audio import decode_audio

    audio = decode_audio(audio_path, sampling_rate=16000)
    waveform = torch.from_numpy(audio).unsqueeze(0)
    return pipeline({"waveform": waveform, "sample_rate": 16000})


def annotation_from_output(output):
    """Normalize the pyannote 4 output wrapper to its regular annotation."""
    return getattr(output, "speaker_diarization", output)


def turns_from_annotation(diarization) -> list[dict]:
    turns = []
    labels = []
    for segment, _track, label in diarization.itertracks(yield_label=True):
        if label not in labels:
            labels.append(label)
        index = labels.index(label)
        turns.append({
            "speakerKey": f"s:{index}",
            "displayName": f"Speaker {index + 1}",
            "startMs": round(segment.start * 1000),
            "endMs": round(segment.end * 1000),
            "confidence": None,
        })
    return turns


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Diarize an audio file locally.")
    parser.add_argument("audio_path")
    parser.add_argument("--model", default="pyannote/speaker-diarization-3.1")
    args = parser.parse_args()

    def report(pct: float) -> None:
        print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

    report(1)
    try:
        import torch
        from pyannote.audio import Pipeline
    except ImportError as error:
        print(f"MODEL_ACCESS pyannote-audio is not installed: {error}", file=sys.stderr, flush=True)
        return 3

    token = os.environ.get("FUNG_HF_TOKEN") or os.environ.get("HF_TOKEN")
    try:
        pipeline = load_pipeline(args.model, token)
    except Exception as error:  # gated model, missing token, offline first run
        print(f"MODEL_ACCESS could not load {args.model}: {error}", file=sys.stderr, flush=True)
        return 3

    if torch.cuda.is_available():
        pipeline.to(torch.device("cuda"))

    report(10)
    try:
        diarization = annotation_from_output(run_pipeline(pipeline, args.audio_path, torch))
    except Exception as error:  # bad audio, OOM, or any other inference failure
        print(f"DIARIZE_FAILED {error}", file=sys.stderr, flush=True)
        return 4
    report(90)

    turns = turns_from_annotation(diarization)
    duration_ms = max((turn["endMs"] for turn in turns), default=0)

    report(100)
    print(json.dumps({"durationMs": duration_ms, "turns": turns}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
