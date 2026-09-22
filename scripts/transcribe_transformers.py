"""Opt-in Transformers/PyTorch worker for the Thai Whisper candidate.

This worker is deliberately separate from ``transcribe.py``. The Thai
checkpoint is a Transformers/PyTorch ``pytorch_model.bin`` and is not a
faster-whisper/CTranslate2 model directory. Rust selects this worker only for
the explicit ``thai-large-candidate`` profile.

The model argument must resolve to a local directory. Hugging Face is kept
offline so a missing or incomplete candidate stage fails instead of silently
downloading during transcription.
"""

import argparse
import json
import os
import sys


DEFAULT_MODEL = "whisper-th-large-combined"


def load_pipeline(model_path: str, profile: str):
    """Load the Transformers ASR pipeline once for a batch or live session."""
    if not os.path.isdir(model_path):
        raise RuntimeError(
            f"Transformers candidate model directory is missing: {model_path}. "
            "Run scripts/stage_whisper_transformers_candidate.ps1 first."
        )

    os.environ.setdefault("HF_HUB_OFFLINE", "1")
    import torch
    from transformers import AutoModelForSpeechSeq2Seq, AutoProcessor, pipeline

    use_cuda = profile == "gpu"
    if use_cuda and not torch.cuda.is_available():
        raise RuntimeError("Transformers candidate requested GPU execution but CUDA is unavailable")

    device = "cuda" if use_cuda else "cpu"
    dtype = torch.float16 if use_cuda else torch.float32
    processor = AutoProcessor.from_pretrained(model_path, local_files_only=True)
    model = AutoModelForSpeechSeq2Seq.from_pretrained(
        model_path,
        local_files_only=True,
        torch_dtype=dtype,
    )
    model.to(device)

    return pipeline(
        "automatic-speech-recognition",
        model=model,
        tokenizer=processor.tokenizer,
        feature_extractor=processor.feature_extractor,
        chunk_length_s=30,
        stride_length_s=5,
        device=0 if use_cuda else -1,
        torch_dtype=dtype,
    )


def decode_audio(path: str):
    """Use the already-staged decoder; only the model backend changes here."""
    from faster_whisper.audio import decode_audio as decode

    return decode(path, sampling_rate=16000)


def transcribe_audio(asr_pipeline, audio, language: str | None, duration_s: float):
    generate_kwargs = {"task": "transcribe"}
    if language:
        generate_kwargs["language"] = language
    result = asr_pipeline(
        {"raw": audio, "sampling_rate": 16000},
        return_timestamps=True,
        generate_kwargs=generate_kwargs,
    )

    segments = []
    for chunk in result.get("chunks", []):
        timestamp = chunk.get("timestamp") or (None, None)
        start_s, end_s = timestamp
        if start_s is None:
            start_s = 0.0
        if end_s is None:
            end_s = duration_s
        text = (chunk.get("text") or "").strip()
        if not text:
            continue
        segments.append(
            {
                "startMs": round(start_s * 1000),
                "endMs": round(end_s * 1000),
                "text": text,
                "confidence": None,
            }
        )

    if not segments and (result.get("text") or "").strip():
        segments.append(
            {
                "startMs": 0,
                "endMs": round(duration_s * 1000),
                "text": result["text"].strip(),
                "confidence": None,
            }
        )
    return segments


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Transcribe with the opt-in Transformers Thai Whisper candidate.")
    parser.add_argument("audio_paths", nargs="*", help="Audio/video paths in one continuous timeline")
    parser.add_argument("--manifest", default=None, help="Newline-delimited audio path manifest")
    parser.add_argument(
        "--model",
        default=os.environ.get("FUNG_WHISPER_MODEL", DEFAULT_MODEL),
        help="Local Transformers model directory; repository IDs are rejected by offline loading",
    )
    parser.add_argument("--language", default=None, help="Force a language code such as th")
    parser.add_argument("--profile", default=os.environ.get("FUNG_TRANSCRIPTION_PROFILE", "cpu"), choices=["cpu", "gpu"])
    args = parser.parse_args()

    if args.manifest:
        with open(args.manifest, "r", encoding="utf-8") as manifest_file:
            audio_paths = [line.strip() for line in manifest_file if line.strip()]
    else:
        audio_paths = args.audio_paths
    if not audio_paths:
        parser.error("either --manifest (non-empty) or at least one positional audio path is required")

    missing_paths = [path for path in audio_paths if not os.path.isfile(path)]
    if missing_paths:
        parser.error(f"audio input not found: {missing_paths[0]}")

    def report(pct: float) -> None:
        print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)

    report(1)
    asr_pipeline = load_pipeline(args.model, args.profile)
    report(5)

    segments = []
    cumulative_ms = 0
    total_files = len(audio_paths)
    for index, audio_path in enumerate(audio_paths):
        audio = decode_audio(audio_path)
        duration_s = len(audio) / 16000.0
        file_offset_ms = cumulative_ms
        for segment in transcribe_audio(asr_pipeline, audio, args.language, duration_s):
            segment["startMs"] += file_offset_ms
            segment["endMs"] += file_offset_ms
            segments.append(segment)
        cumulative_ms += round(duration_s * 1000)
        report(5 + 93 * ((index + 1) / total_files))

    report(100)
    print(
        json.dumps(
            {
                "language": args.language,
                "languageProbability": None,
                "durationMs": cumulative_ms,
                "segments": segments,
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
