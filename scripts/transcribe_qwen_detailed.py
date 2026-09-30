"""Offline Thai Qwen ASR worker with a separate CTC timing pass."""

from __future__ import annotations

import argparse
import gc
import json
import math
import os
import pathlib
import sys

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"

import numpy as np
import torch
import transformers
from faster_whisper.audio import decode_audio
from pythainlp.tokenize import word_tokenize
from transformers import (
    AutoFeatureExtractor,
    AutoModelForCTC,
    AutoModelForMultimodalLM,
    AutoProcessor,
    AutoTokenizer,
)

script_directory = str(pathlib.Path(__file__).resolve().parent)
if script_directory not in sys.path:
    sys.path.insert(0, script_directory)

from thai_ctc_alignment import aligned_output_spans, ctc_viterbi_align, prepare_alignment_target


ASR_REPOSITORY = "Qwen/Qwen3-ASR-1.7B-hf"
ASR_REVISION = "bcd2b5b7f32b480ab5790554cfa8347f246a14f3"
ALIGNER_REPOSITORY = "wannaphong/wav2vec2-large-xlsr-53-th-cv8-newmm"
ALIGNER_REVISION = "18381b4cfbe8b2e7462827f2c6dea681a31ef5b9"
SAMPLE_RATE = 16_000
AUDIO_PREPROCESSING_PROFILES = ("raw", "afftdn", "speechnorm")


def load_chunk_manifest(path: pathlib.Path) -> list[dict]:
    chunks = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(chunks, list) or not chunks:
        raise ValueError("--chunks-manifest must contain a non-empty JSON array")
    for index, chunk in enumerate(chunks):
        if (
            not isinstance(chunk, dict)
            or not isinstance(chunk.get("path"), str)
            or not pathlib.Path(chunk["path"]).is_file()
            or (
                "startMs" in chunk
                and (
                    not isinstance(chunk["startMs"], int)
                    or isinstance(chunk["startMs"], bool)
                    or chunk["startMs"] < 0
                )
            )
        ):
            raise ValueError(f"invalid or missing audio chunk at index {index}")
    return chunks


def load_audio_chunks(chunks: list[dict]) -> list[dict]:
    loaded = []
    cursor_ms = 0
    for index, chunk in enumerate(chunks):
        audio = decode_audio(chunk["path"], sampling_rate=SAMPLE_RATE)
        audio = np.asarray(audio, dtype=np.float32)
        if audio.ndim != 1 or not audio.size or not np.isfinite(audio).all():
            raise ValueError(f"decoded audio is empty or invalid: {pathlib.Path(chunk['path']).name}")
        duration_ms = round(len(audio) * 1000 / SAMPLE_RATE)
        start_ms = chunk.get("startMs", cursor_ms)
        if index and start_ms < loaded[-1]["start_ms"]:
            raise ValueError("audio chunks are not ordered by their recording offsets")
        loaded.append(
            {
                "path": chunk["path"],
                "audio": audio,
                "duration_ms": duration_ms,
                "start_ms": start_ms,
                **{key: chunk[key] for key in ("id", "mic", "reference") if key in chunk},
            }
        )
        cursor_ms = max(cursor_ms, start_ms + duration_ms)
    return loaded


def preprocess_audio_for_asr(audio: np.ndarray, profile: str) -> np.ndarray:
    if profile not in AUDIO_PREPROCESSING_PROFILES:
        raise ValueError(f"unsupported Qwen audio preprocessing profile: {profile}")
    source = np.asarray(audio, dtype=np.float32)
    if source.ndim != 1 or not source.size or not np.isfinite(source).all():
        raise ValueError("Qwen preprocessing input must be non-empty finite mono audio")
    if profile == "raw":
        return source.copy()

    import av

    graph = av.filter.Graph()
    source_filter = graph.add_abuffer(
        sample_rate=SAMPLE_RATE, format="fltp", layout="mono"
    )
    audio_filter = graph.add(profile)
    sink = graph.add("abuffersink")
    source_filter.link_to(audio_filter)
    audio_filter.link_to(sink)
    graph.configure()

    frame = av.AudioFrame.from_ndarray(
        source.reshape(1, -1).copy(), format="fltp", layout="mono"
    )
    frame.sample_rate = SAMPLE_RATE
    source_filter.push(frame)
    source_filter.push(None)

    filtered_frames = []
    while True:
        try:
            filtered_frame = sink.pull()
        except av.error.EOFError:
            break
        if filtered_frame is None:
            break
        filtered_frames.append(filtered_frame.to_ndarray().reshape(-1))
    if not filtered_frames:
        raise ValueError(f"PyAV {profile} returned no audio samples")
    filtered = np.concatenate(filtered_frames).astype(np.float32, copy=False)
    if filtered.size != source.size:
        raise ValueError(
            f"PyAV {profile} changed the source sample count "
            f"({source.size} to {filtered.size})"
        )
    if not np.isfinite(filtered).all():
        raise ValueError(f"PyAV {profile} returned non-finite audio samples")
    return filtered


def validate_aligned_segments(text: str, segments: list[dict], duration_ms: int) -> None:
    if not text.strip() or not segments:
        raise ValueError("Qwen returned blank text or no aligned segments")
    if "".join(segment["text"] for segment in segments) != text:
        raise ValueError("aligned segment text does not reconstruct the original Qwen text")
    previous_start = -1
    previous_end = -1
    for segment in segments:
        start_ms = segment.get("startMs")
        end_ms = segment.get("endMs")
        segment_text = segment.get("text")
        if (
            not isinstance(start_ms, int)
            or isinstance(start_ms, bool)
            or not isinstance(end_ms, int)
            or isinstance(end_ms, bool)
            or not isinstance(segment_text, str)
            or not segment_text
            or not 0 <= start_ms < end_ms <= duration_ms
        ):
            raise ValueError("aligned candidate contains a blank or out-of-range timestamp span")
        if start_ms < previous_start or end_ms < previous_end:
            raise ValueError("aligned candidate spans are not ordered")
        previous_start = start_ms
        previous_end = end_ms


def validate_resource(profile: str) -> None:
    if profile == "gpu":
        if not torch.cuda.is_available():
            raise RuntimeError("Qwen Detailed GPU execution requested but CUDA is unavailable")
        free_vram, _ = torch.cuda.mem_get_info()
        if free_vram < 6 * 1024**3:
            raise RuntimeError(f"less than 6 GiB free VRAM before Qwen load: {free_vram / 1024**3:.2f} GiB")
    else:
        import psutil

        available_ram = psutil.virtual_memory().available
        if available_ram < 10 * 1024**3:
            raise RuntimeError(f"less than 10 GiB free RAM before Qwen load: {available_ram / 1024**3:.2f} GiB")


def transcribe_qwen(
    chunks: list[dict],
    model_path: pathlib.Path,
    profile: str,
    audio_preprocessing_profile: str,
) -> tuple[list[str], list[dict]]:
    if not model_path.is_dir():
        raise RuntimeError(f"pinned local Qwen model directory is missing: {model_path}")
    validate_resource(profile)
    use_gpu = profile == "gpu"
    dtype = torch.float16 if use_gpu else torch.float32
    device = "cuda:0" if use_gpu else "cpu"
    processor = AutoProcessor.from_pretrained(str(model_path), local_files_only=True)
    model = AutoModelForMultimodalLM.from_pretrained(
        str(model_path),
        local_files_only=True,
        low_cpu_mem_usage=True,
        dtype=dtype,
        device_map=device if use_gpu else None,
    )
    if not use_gpu:
        model.to(device)
    model.eval()

    texts = []
    audio_info = []
    with torch.inference_mode():
        for index, chunk in enumerate(chunks, 1):
            asr_audio = preprocess_audio_for_asr(
                chunk["audio"], audio_preprocessing_profile
            )
            inputs = processor.apply_transcription_request(
                audio=asr_audio, language="Thai"
            )
            inputs = inputs.to(model.device, model.dtype)
            generated = model.generate(**inputs, max_new_tokens=256, do_sample=False)
            generated = generated[:, inputs["input_ids"].shape[1] :]
            text = processor.decode(generated, return_format="transcription_only")[0]
            if not text.strip():
                raise ValueError(f"Qwen returned blank text for chunk {index}")
            texts.append(text)
            audio_info.append({key: chunk[key] for key in ("path", "duration_ms", "start_ms")})
            print(f"ASR {index}/{len(chunks)}", file=sys.stderr, flush=True)

    del model, processor
    del inputs, generated
    gc.collect()
    if use_gpu:
        torch.cuda.empty_cache()
    return texts, audio_info


def align_qwen_texts(
    chunks: list[dict],
    texts: list[str],
    model_path: pathlib.Path,
    profile: str,
) -> tuple[list[dict], list[dict], dict]:
    if not model_path.is_dir():
        raise RuntimeError(f"pinned local Thai CTC model directory is missing: {model_path}")
    use_gpu = profile == "gpu"
    if use_gpu:
        free_vram, _ = torch.cuda.mem_get_info()
        if free_vram < 2 * 1024**3:
            raise RuntimeError(f"less than 2 GiB free VRAM before Thai CTC load: {free_vram / 1024**3:.2f} GiB")
    dtype = torch.float16 if use_gpu else torch.float32
    device = "cuda:0" if use_gpu else "cpu"
    feature_extractor = AutoFeatureExtractor.from_pretrained(str(model_path), local_files_only=True)
    tokenizer = AutoTokenizer.from_pretrained(str(model_path), local_files_only=True)
    model = AutoModelForCTC.from_pretrained(
        str(model_path),
        local_files_only=True,
        use_safetensors=True,
        low_cpu_mem_usage=True,
        dtype=dtype,
        device_map=device if use_gpu else None,
    )
    if not use_gpu:
        model.to(device)
    model.eval()
    stride_samples = math.prod(model.config.conv_stride)
    sampling_rate = feature_extractor.sampling_rate
    aligned = []
    chunk_results = []
    with torch.inference_mode():
        for index, (chunk, text) in enumerate(zip(chunks, texts, strict=True), 1):
            target = prepare_alignment_target(
                text,
                tokenizer,
                lambda value: word_tokenize(value, engine="newmm", keep_whitespace=True),
            )
            inputs = feature_extractor(
                chunk["audio"], sampling_rate=sampling_rate, return_tensors="pt"
            )
            input_values = inputs.input_values.to(device=model.device, dtype=model.dtype)
            logits = model(input_values).logits[0]
            log_probabilities = torch.log_softmax(logits.float(), dim=-1).cpu().tolist()
            alignment = ctc_viterbi_align(log_probabilities, target.token_ids, tokenizer.pad_token_id)
            spans = aligned_output_spans(
                target,
                alignment.token_spans,
                stride_samples,
                sampling_rate,
                chunk["duration_ms"],
            )
            validate_aligned_segments(text, spans, chunk["duration_ms"])
            for span in spans:
                span["startMs"] += chunk["start_ms"]
                span["endMs"] += chunk["start_ms"]
            aligned.extend(spans)
            chunk_results.append(
                {
                    **{key: chunk[key] for key in ("id", "mic", "reference") if key in chunk},
                    "text": text,
                    "durationMs": chunk["duration_ms"],
                    "startMs": chunk["start_ms"],
                    "segments": spans,
                }
            )
            print(f"ALIGN {index}/{len(chunks)}", file=sys.stderr, flush=True)

    if any(
        aligned[index]["startMs"] < aligned[index - 1]["startMs"]
        or aligned[index]["endMs"] < aligned[index - 1]["endMs"]
        for index in range(1, len(aligned))
    ):
        raise ValueError("aligned candidate spans are not ordered across audio chunks")
    return aligned, chunk_results, {
        "frameStrideSamples": stride_samples,
        "samplingRate": sampling_rate,
        "device": torch.cuda.get_device_name(0) if use_gpu else "cpu",
        "dtype": str(model.dtype),
    }


def run(args: argparse.Namespace) -> dict:
    if args.language and args.language.lower() not in {"th", "thai"}:
        raise ValueError("the qualified Qwen + Thai CTC candidate currently accepts Thai only")
    chunks = load_audio_chunks(load_chunk_manifest(args.chunks_manifest))
    texts, audio_info = transcribe_qwen(
        chunks,
        args.asr_model,
        args.profile,
        args.audio_preprocessing_profile,
    )
    aligned, chunk_results, ctc_info = align_qwen_texts(
        chunks, texts, args.aligner_model, args.profile
    )
    cursor_ms = max(item["start_ms"] + item["duration_ms"] for item in audio_info)
    return {
        "durationMs": cursor_ms,
        "segments": aligned,
        "chunkResults": chunk_results,
        "candidateProvenance": {
            "backend": f"Transformers {transformers.__version__} / PyTorch {torch.__version__}",
            "asrModel": ASR_REPOSITORY,
            "asrRevision": ASR_REVISION,
            "alignerModel": ALIGNER_REPOSITORY,
            "alignerRevision": ALIGNER_REVISION,
            "audioPreprocessingProfile": args.audio_preprocessing_profile,
            "device": ctc_info["device"],
            "dtype": ctc_info["dtype"],
            "frameStrideSamples": ctc_info["frameStrideSamples"],
            "samplingRate": ctc_info["samplingRate"],
        },
    }


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chunks-manifest", type=pathlib.Path, required=True)
    parser.add_argument("--asr-model", type=pathlib.Path, required=True)
    parser.add_argument("--aligner-model", type=pathlib.Path, required=True)
    parser.add_argument("--language", default="th")
    parser.add_argument("--profile", choices=["cpu", "gpu"], default="cpu")
    parser.add_argument(
        "--audio-preprocessing-profile",
        choices=AUDIO_PREPROCESSING_PROFILES,
        default="raw",
    )
    args = parser.parse_args()
    print(json.dumps(run(args), ensure_ascii=False, allow_nan=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
