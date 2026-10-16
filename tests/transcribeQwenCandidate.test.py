import json
import pathlib
import sys
import tempfile
import types
import unittest
from argparse import Namespace
from unittest.mock import Mock, patch


SCRIPTS = pathlib.Path(__file__).parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))

numpy = types.ModuleType("numpy")
torch = types.ModuleType("torch")
torch.__version__ = "test"
transformers = types.ModuleType("transformers")
transformers.__version__ = "test"
transformers.AutoFeatureExtractor = object()
transformers.AutoModelForCTC = object()
transformers.AutoModelForMultimodalLM = object()
transformers.AutoProcessor = object()
transformers.AutoTokenizer = object()
faster_whisper = types.ModuleType("faster_whisper")
faster_whisper.__path__ = []
faster_whisper_audio = types.ModuleType("faster_whisper.audio")
faster_whisper_audio.decode_audio = object()
pythainlp = types.ModuleType("pythainlp")
pythainlp.__path__ = []
pythainlp_tokenize = types.ModuleType("pythainlp.tokenize")
pythainlp_tokenize.word_tokenize = object()

with patch.dict(
    sys.modules,
    {
        "numpy": numpy,
        "torch": torch,
        "transformers": transformers,
        "faster_whisper": faster_whisper,
        "faster_whisper.audio": faster_whisper_audio,
        "pythainlp": pythainlp,
        "pythainlp.tokenize": pythainlp_tokenize,
    },
):
    import transcribe_qwen_detailed as worker


class QwenCandidateContractTests(unittest.TestCase):
    def test_approved_single_safetensors_checkpoint_remains_supported(self):
        with tempfile.TemporaryDirectory() as directory:
            model_path = pathlib.Path(directory)
            (model_path / "model.safetensors").write_bytes(b"fixture")

            worker.validate_qwen_single_checkpoint(model_path, "ASR")

    def test_sharded_index_is_rejected_before_either_qwen_loader_runs(self):
        attack_indexes = (
            (
                "model.safetensors.index.json",
                '{"weight_map":{"weight":"../outside/payload.safetensors"}}',
            ),
            (
                "model.safetensors.index.json",
                '{"weight_map":{"weight":"C:\\\\outside\\\\payload.safetensors"}}',
            ),
            (
                "MODEL.SAFETENSORS.INDEX.JSON",
                '{"weight_map":{"weight":"..\\\\outside\\\\payload.safetensors"}}',
            ),
        )
        for index_name, index_contents in attack_indexes:
            with (
                self.subTest(index_name=index_name, index_contents=index_contents),
                tempfile.TemporaryDirectory() as directory,
            ):
                model_path = pathlib.Path(directory)
                (model_path / "model.safetensors").write_bytes(b"fixture")
                (model_path / index_name).write_text(
                    index_contents, encoding="utf-8"
                )

                processor_load = Mock()
                asr_model_load = Mock()
                with (
                    patch.object(
                        worker,
                        "AutoProcessor",
                        types.SimpleNamespace(from_pretrained=processor_load),
                    ),
                    patch.object(
                        worker,
                        "AutoModelForMultimodalLM",
                        types.SimpleNamespace(from_pretrained=asr_model_load),
                    ),
                ):
                    with self.assertRaisesRegex(
                        RuntimeError, "sharded checkpoints are not allowed"
                    ):
                        worker.transcribe_qwen([], model_path, "cpu", "raw")
                processor_load.assert_not_called()
                asr_model_load.assert_not_called()

                feature_load = Mock()
                tokenizer_load = Mock()
                aligner_model_load = Mock()
                with (
                    patch.object(
                        worker,
                        "AutoFeatureExtractor",
                        types.SimpleNamespace(from_pretrained=feature_load),
                    ),
                    patch.object(
                        worker,
                        "AutoTokenizer",
                        types.SimpleNamespace(from_pretrained=tokenizer_load),
                    ),
                    patch.object(
                        worker,
                        "AutoModelForCTC",
                        types.SimpleNamespace(from_pretrained=aligner_model_load),
                    ),
                ):
                    with self.assertRaisesRegex(
                        RuntimeError, "sharded checkpoints are not allowed"
                    ):
                        worker.align_qwen_texts([], [], model_path, "cpu")
                feature_load.assert_not_called()
                tokenizer_load.assert_not_called()
                aligner_model_load.assert_not_called()

    def test_requires_the_approved_single_safetensors_checkpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "pinned model.safetensors is missing"):
                worker.validate_qwen_single_checkpoint(pathlib.Path(directory), "ASR")

    def test_accepts_ordered_source_bounded_segments_that_preserve_candidate_text(self):
        worker.validate_aligned_segments(
            "สวัสดีครับ",
            [
                {"startMs": 120, "endMs": 310, "text": "สวัสดี", "confidence": None},
                {"startMs": 330, "endMs": 500, "text": "ครับ", "confidence": None},
            ],
            500,
        )

    def test_preserves_original_leading_and_trailing_whitespace(self):
        original = " สวัสดีครับ "
        worker.validate_aligned_segments(
            original,
            [{"startMs": 0, "endMs": 500, "text": original, "confidence": None}],
            500,
        )

    def test_rejects_empty_candidate_text_and_missing_segments(self):
        with self.assertRaisesRegex(ValueError, "blank text"):
            worker.validate_aligned_segments(" ", [], 500)
        with self.assertRaisesRegex(ValueError, "blank text"):
            worker.validate_aligned_segments("text", [], 500)

    def test_rejects_segment_text_that_does_not_reconstruct_qwen_output(self):
        with self.assertRaisesRegex(ValueError, "reconstruct"):
            worker.validate_aligned_segments(
                "สวัสดีครับ",
                [{"startMs": 0, "endMs": 100, "text": "สวัสดี", "confidence": None}],
                500,
            )

    def test_rejects_zero_duration_out_of_range_and_non_integer_spans(self):
        invalid = [
            {"startMs": 10, "endMs": 10, "text": "a"},
            {"startMs": -1, "endMs": 10, "text": "a"},
            {"startMs": 0, "endMs": 501, "text": "a"},
            {"startMs": 0.0, "endMs": 10, "text": "a"},
            {"startMs": True, "endMs": 10, "text": "a"},
        ]
        for segment in invalid:
            with self.subTest(segment=segment), self.assertRaisesRegex(ValueError, "timestamp span"):
                worker.validate_aligned_segments("a", [segment], 500)

    def test_rejects_decreasing_segment_order(self):
        with self.assertRaisesRegex(ValueError, "not ordered"):
            worker.validate_aligned_segments(
                "ab",
                [
                    {"startMs": 100, "endMs": 200, "text": "a"},
                    {"startMs": 50, "endMs": 180, "text": "b"},
                ],
                500,
            )

    def test_chunk_manifest_rejects_boolean_offsets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            audio = root / "sample.wav"
            audio.write_bytes(b"fixture")
            manifest = root / "chunks.json"
            manifest.write_text(
                json.dumps([{"path": str(audio), "startMs": True}]),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "invalid or missing"):
                worker.load_chunk_manifest(manifest)

    def test_preprocessing_rejects_unknown_profiles(self):
        with self.assertRaisesRegex(ValueError, "unsupported"):
            worker.preprocess_audio_for_asr([], "unexpected")

    def test_alignment_receives_the_original_decoded_audio(self):
        source_audio = object()
        chunk = {
            "path": "sample.wav",
            "audio": source_audio,
            "duration_ms": 100,
            "start_ms": 0,
        }
        args = Namespace(
            language="th",
            chunks_manifest=pathlib.Path("chunks.json"),
            asr_model=pathlib.Path("asr"),
            aligner_model=pathlib.Path("aligner"),
            profile="cpu",
            audio_preprocessing_profile="afftdn",
        )
        with (
            patch.object(worker, "load_chunk_manifest", return_value=[]),
            patch.object(worker, "load_audio_chunks", return_value=[chunk]),
            patch.object(worker, "transcribe_qwen", return_value=(
                ["text"], [{"start_ms": 0, "duration_ms": 100}]
            )) as transcribe,
            patch.object(
                worker,
                "align_qwen_texts",
                return_value=(
                    [{"startMs": 0, "endMs": 100, "text": "text"}],
                    [],
                    {
                        "device": "cpu",
                        "dtype": "torch.float32",
                        "frameStrideSamples": 320,
                        "samplingRate": 16_000,
                    },
                ),
            ) as align,
        ):
            worker.run(args)

        transcribe.assert_called_once_with([chunk], pathlib.Path("asr"), "cpu", "afftdn")
        self.assertIs(align.call_args.args[0][0]["audio"], source_audio)


if __name__ == "__main__":
    unittest.main()
