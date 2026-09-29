import importlib.util
import pathlib
import sys
import types
import unittest
from unittest.mock import Mock, patch


SCRIPT = pathlib.Path(__file__).parents[1] / "scripts" / "transcribe_transformers.py"
SPEC = importlib.util.spec_from_file_location("transcribe_transformers", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class RecordingOffsetTests(unittest.TestCase):
    def test_chunk_segments_keep_recording_timeline_offsets(self):
        source = [
            {"startMs": 20, "endMs": 640, "text": "สวัสดี", "confidence": None}
        ]

        result = MODULE.apply_recording_offset(source, 12_000)

        self.assertEqual(result[0]["startMs"], 12_020)
        self.assertEqual(result[0]["endMs"], 12_640)
        self.assertEqual(result[0]["text"], "สวัสดี")
        self.assertEqual(source[0]["startMs"], 20, "the input segment must remain unchanged")


class CandidateLoaderTests(unittest.TestCase):
    def test_model_load_is_local_and_uses_accelerate_low_memory_path(self):
        torch = types.ModuleType("torch")
        torch.cuda = types.SimpleNamespace(is_available=lambda: False)
        torch.float16 = object()
        torch.float32 = object()

        model = Mock()
        processor = types.SimpleNamespace(tokenizer=object(), feature_extractor=object())
        auto_model = types.SimpleNamespace(from_pretrained=Mock(return_value=model))
        auto_processor = types.SimpleNamespace(from_pretrained=Mock(return_value=processor))
        pipeline = Mock(return_value=object())
        transformers = types.ModuleType("transformers")
        transformers.AutoModelForSpeechSeq2Seq = auto_model
        transformers.AutoProcessor = auto_processor
        transformers.pipeline = pipeline

        with patch.dict(sys.modules, {"torch": torch, "transformers": transformers}):
            with patch("os.path.isdir", return_value=True):
                MODULE.load_pipeline("local-candidate-model", "cpu")

        auto_processor.from_pretrained.assert_called_once_with(
            "local-candidate-model",
            local_files_only=True,
        )
        auto_model.from_pretrained.assert_called_once_with(
            "local-candidate-model",
            local_files_only=True,
            low_cpu_mem_usage=True,
            torch_dtype=torch.float32,
        )
        model.to.assert_called_once_with("cpu")
        pipeline.assert_called_once()


if __name__ == "__main__":
    unittest.main()
