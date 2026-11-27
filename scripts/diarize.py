"""Local speaker-diarization worker for FUNG, backed by pyannote-audio.

Invoked by the Rust backend as a subprocess:

    python diarize.py <audio_path> [--model pyannote/speaker-diarization-3.1]
    python diarize.py --fetch-only [--model ...]

The pyannote pipeline weights are gated on Hugging Face: the user must
accept the model license and expose a token via FUNG_HF_TOKEN (or HF_TOKEN)
for the FIRST download; afterwards the cached model works offline.

When the dependencies come from the in-app add-on, FUNG_DIARIZATION_SITE
names its site-packages. The bundled interpreter is the embeddable CPython,
which ignores PYTHONPATH, so the worker puts that directory on sys.path
itself, ahead of the runtime's own site-packages.

``--fetch-only`` downloads the model into the cache and exits. On failure it
prints ``MODEL_ACCESS <code>`` on stderr, where <code> is one of
``token_invalid``, ``terms_not_accepted``, ``network`` or ``unknown``.
"""

import argparse
import json
import os
import site
import sys


def add_addon_site() -> None:
    addon = os.environ.get("FUNG_DIARIZATION_SITE")
    if not addon:
        return
    before = list(sys.path)
    site.addsitedir(addon)  # also processes any .pth files in the add-on
    added = [entry for entry in sys.path if entry not in before]
    sys.path[:] = added + before


def token_problem(token: str | None, whoami=None) -> str | None:
    """Check the token itself before asking for the gated repo.

    The hub answers a gated repo with the same ``401 GatedRepo`` whether the
    token is wrong or the terms are simply not accepted, so only a separate
    identity call can tell the user which of the two to fix.
    """
    if not token:
        return "token_invalid"
    if whoami is None:
        from huggingface_hub import whoami
    try:
        whoami(token=token)
    except Exception as error:
        status = getattr(getattr(error, "response", None), "status_code", None)
        if status == 401:
            return "token_invalid"
        code = model_access_code(error)
        return code if code == "network" else None
    return None


def model_access_code(error: BaseException) -> str:
    """Classify a model download failure into something a user can act on.

    Checked by class name and HTTP status rather than imported exception
    types, so the classification survives huggingface_hub reorganising its
    error module.
    """
    seen = set()
    current: BaseException | None = error
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        name = type(current).__name__
        response = getattr(current, "response", None)
        status = getattr(response, "status_code", None)
        if name == "GatedRepoError" or status == 403:
            return "terms_not_accepted"
        if status == 401:
            return "token_invalid"
        if name in {"LocalEntryNotFoundError", "ConnectionError", "Timeout", "ConnectTimeout", "ReadTimeout"} or isinstance(current, (ConnectionError, TimeoutError)):
            return "network"
        current = current.__cause__ or current.__context__
    text = str(error).lower()
    if "gated" in text or "accept" in text:
        return "terms_not_accepted"
    if "401" in text or ("invalid" in text and "token" in text):
        return "token_invalid"
    return "unknown"


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
    parser.add_argument("audio_path", nargs="?")
    parser.add_argument("--model", default="pyannote/speaker-diarization-3.1")
    parser.add_argument("--fetch-only", action="store_true")
    args = parser.parse_args()
    if not args.fetch_only and not args.audio_path:
        parser.error("audio_path is required unless --fetch-only is given")

    add_addon_site()

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
    if args.fetch_only:
        problem = token_problem(token)
        if problem:
            print(f"MODEL_ACCESS {problem}", file=sys.stderr, flush=True)
            return 3
    try:
        pipeline = load_pipeline(args.model, token)
    except Exception as error:  # gated model, missing token, offline first run
        if args.fetch_only:
            # Only the code: the exception text can echo request details.
            print(f"MODEL_ACCESS {model_access_code(error)}", file=sys.stderr, flush=True)
        else:
            print(f"MODEL_ACCESS could not load {args.model}: {error}", file=sys.stderr, flush=True)
        return 3

    if args.fetch_only:
        report(100)
        print(json.dumps({"ready": True}))
        return 0

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
