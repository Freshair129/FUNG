import importlib.util
import pathlib
import unittest


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


if __name__ == "__main__":
    unittest.main()
