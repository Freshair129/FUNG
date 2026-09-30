"""Run a local Thai CTC alignment feasibility pass against saved Qwen text."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import pathlib
import time
import wave

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"

import numpy as np
import torch
import transformers
from pythainlp.tokenize import word_tokenize
from transformers import AutoFeatureExtractor, AutoModelForCTC, AutoTokenizer

from thai_ctc_alignment import aligned_output_spans, ctc_viterbi_align, prepare_alignment_target


ALIGNER_REPOSITORY = "wannaphong/wav2vec2-large-xlsr-53-th-cv8-newmm"
ALIGNER_REVISION = "18381b4cfbe8b2e7462827f2c6dea681a31ef5b9"
ALIGNER_CHECKPOINT_BYTES = 1_262_102_632
ALIGNER_CHECKPOINT_SHA256 = "f0135130a25f0f16a59cc376f886f9e54e0c1736f1b85c4c01e93b9f4cc4b090"
QWEN_MODEL = "Qwen/Qwen3-ASR-1.7B-hf"
QWEN_REVISION = "bcd2b5b7f32b480ab5790554cfa8347f246a14f3"


def load_jsonl(path: pathlib.Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_audio(path: pathlib.Path) -> tuple[np.ndarray, int]:
    with wave.open(str(path), "rb") as source:
        sample_rate = source.getframerate()
        channels = source.getnchannels()
        sample_width = source.getsampwidth()
        frames = source.getnframes()
        raw = source.readframes(frames)
    if sample_rate != 16_000:
        raise ValueError(f"audio sample rate is {sample_rate}, expected 16000: {path.name}")
    if sample_width == 2:
        samples = np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0
    elif sample_width == 4:
        samples = np.frombuffer(raw, dtype="<i4").astype(np.float32) / 2147483648.0
    elif sample_width == 1:
        samples = (np.frombuffer(raw, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    else:
        raise ValueError(f"unsupported PCM sample width {sample_width}: {path.name}")
    if channels > 1:
        samples = samples.reshape(-1, channels).mean(axis=1)
    return samples, round(frames * 1000 / sample_rate)


def run(args: argparse.Namespace) -> dict:
    dataset_root = args.manifest.parent
    rows = load_jsonl(args.manifest)
    predictions = load_jsonl(args.predictions)
    if len(rows) != 55 or len({row["id"] for row in rows}) != 55:
        raise ValueError("expected exactly 55 unique LOTUSDIS manifest rows")
    if len(predictions) != 55 or len({row["id"] for row in predictions}) != 55:
        raise ValueError("expected exactly 55 unique saved Qwen predictions")
    row_by_id = {row["id"]: row for row in rows}
    prediction_by_id = {row["id"]: row for row in predictions}
    if set(row_by_id) != set(prediction_by_id):
        raise ValueError("Qwen prediction IDs do not match LOTUSDIS reference IDs")

    model_metrics = json.loads(args.qwen_metrics.read_text(encoding="utf-8"))
    if model_metrics.get("model") != QWEN_MODEL or model_metrics.get("model_revision") != QWEN_REVISION:
        raise ValueError("saved Qwen predictions do not identify the pinned Qwen 1.7B revision")
    checkpoint = args.aligner_model / "model.safetensors"
    if not checkpoint.is_file() or checkpoint.stat().st_size != ALIGNER_CHECKPOINT_BYTES:
        raise ValueError("pinned Thai CTC checkpoint is missing or has the wrong size")
    checkpoint_sha256 = sha256_file(checkpoint)
    if checkpoint_sha256 != ALIGNER_CHECKPOINT_SHA256:
        raise ValueError("pinned Thai CTC checkpoint SHA-256 does not match the repository blob")

    if not torch.cuda.is_available():
        raise ValueError("CUDA is unavailable; feasibility qualification requires the verified GPU")
    free_vram, total_vram = torch.cuda.mem_get_info()
    if free_vram < 2 * 1024**3:
        raise ValueError(f"less than 2 GiB free VRAM before CTC model load: {free_vram / 1024**3:.2f} GiB")

    load_start = time.perf_counter()
    feature_extractor = AutoFeatureExtractor.from_pretrained(
        str(args.aligner_model), local_files_only=True
    )
    tokenizer = AutoTokenizer.from_pretrained(str(args.aligner_model), local_files_only=True)
    model = AutoModelForCTC.from_pretrained(
        str(args.aligner_model),
        local_files_only=True,
        use_safetensors=True,
        dtype=torch.float16,
        device_map="cuda:0",
        low_cpu_mem_usage=True,
    ).eval()
    load_seconds = time.perf_counter() - load_start

    stride_samples = math.prod(model.config.conv_stride)
    sampling_rate = feature_extractor.sampling_rate
    alignment_rows = []
    per_mic = {}
    infer_start = time.perf_counter()
    for index, utterance_id in enumerate(sorted(row_by_id), 1):
        row = row_by_id[utterance_id]
        prediction = prediction_by_id[utterance_id]
        text = prediction.get("text", "")
        target = prepare_alignment_target(
            text,
            tokenizer,
            lambda value: word_tokenize(value, engine="newmm", keep_whitespace=True),
        )
        audio, duration_ms = read_audio(dataset_root / row["audio"])
        inputs = feature_extractor(audio, sampling_rate=sampling_rate, return_tensors="pt")
        input_values = inputs.input_values.to(device=model.device, dtype=model.dtype)
        with torch.inference_mode():
            logits = model(input_values).logits[0]
        log_probabilities = torch.log_softmax(logits.float(), dim=-1).cpu().tolist()
        alignment = ctc_viterbi_align(
            log_probabilities,
            target.token_ids,
            tokenizer.pad_token_id,
        )
        segments = aligned_output_spans(
            target,
            alignment.token_spans,
            stride_samples,
            sampling_rate,
            duration_ms,
        )
        if "".join(segment["text"] for segment in segments) != target.source_text:
            raise ValueError(f"aligned segment text did not reconstruct Qwen text: {utterance_id}")
        if not segments or any(
            not (0 <= segment["startMs"] < segment["endMs"] <= duration_ms)
            for segment in segments
        ):
            raise ValueError(f"invalid source-bounded alignment span: {utterance_id}")
        if any(
            segments[i]["startMs"] < segments[i - 1]["startMs"]
            or segments[i]["endMs"] < segments[i - 1]["endMs"]
            for i in range(1, len(segments))
        ):
            raise ValueError(f"alignment spans are unordered: {utterance_id}")

        alignment_rows.append(
            {
                "id": utterance_id,
                "mic": row["mic"].lower(),
                "durationMs": duration_ms,
                "segments": segments,
                "targetLabelCount": len(target.token_ids),
                "pathLogProbability": alignment.path_log_probability,
            }
        )
        mic = row["mic"].lower()
        group = per_mic.setdefault(mic, {"clips": 0, "segments": 0, "target_labels": 0})
        group["clips"] += 1
        group["segments"] += len(segments)
        group["target_labels"] += len(target.token_ids)
        print(f"ALIGN {index}/55 mic={mic}", flush=True)

    inference_seconds = time.perf_counter() - infer_start
    args.output_dir.mkdir(parents=True, exist_ok=True)
    aligned_path = args.output_dir / "aligned-predictions.jsonl"
    temp_path = aligned_path.with_suffix(".jsonl.tmp")
    temp_path.write_text(
        "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in alignment_rows),
        encoding="utf-8",
    )
    os.replace(temp_path, aligned_path)

    report = {
        "dataset": "LOTUSDIS 55-clip paired pilot",
        "clips": len(alignment_rows),
        "qwen_model": QWEN_MODEL,
        "qwen_revision": QWEN_REVISION,
        "qwen_predictions": str(args.predictions),
        "aligner_model": ALIGNER_REPOSITORY,
        "aligner_revision": ALIGNER_REVISION,
        "aligner_checkpoint_bytes": checkpoint.stat().st_size,
        "aligner_checkpoint_sha256": checkpoint_sha256,
        "backend": f"Transformers {transformers.__version__} / PyTorch {torch.__version__}",
        "device": torch.cuda.get_device_name(0),
        "dtype": str(model.dtype),
        "frame_stride_samples": stride_samples,
        "sampling_rate": sampling_rate,
        "model_load_seconds": load_seconds,
        "inference_seconds": inference_seconds,
        "audio_seconds": sum(row["durationMs"] for row in alignment_rows) / 1000,
        "per_microphone": per_mic,
        "all_ids_aligned": {"expected": 55, "actual": len(alignment_rows)},
        "all_text_preserved_after_whitespace_handling": True,
        "all_spans_source_bounded_ordered_nonzero": True,
        "alignment_quality_boundary": "Feasibility and structural span validation only; LOTUSDIS has no reference timestamps. Five source-audio spot checks remain required before Detailed routing.",
        "runtime_boundary": "Standalone local feasibility run; not the FUNG candidate interpreter or Rust worker.",
        "aligned_predictions_path": str(aligned_path),
    }
    report_path = args.output_dir / "alignment-feasibility.json"
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--predictions", type=pathlib.Path, required=True)
    parser.add_argument("--qwen-metrics", type=pathlib.Path, required=True)
    parser.add_argument("--aligner-model", type=pathlib.Path, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    args = parser.parse_args()
    print(json.dumps(run(args), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
