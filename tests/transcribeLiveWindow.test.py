import importlib.util
import tempfile
import unittest
import wave
from pathlib import Path


MODULE_PATH = Path(__file__).parents[1] / "scripts" / "transcribe_live.py"
SPEC = importlib.util.spec_from_file_location("transcribe_live", MODULE_PATH)
transcribe_live = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(transcribe_live)


class LiveDecodeWindowTests(unittest.TestCase):
    def _write_wav(self, path, seconds=1):
        with wave.open(str(path), "wb") as audio:
            audio.setnchannels(1)
            audio.setsampwidth(2)
            audio.setframerate(16_000)
            audio.writeframes(b"\x01\x00" * (16_000 * seconds))
        return str(path)

    def test_materializes_contiguous_slices_from_durable_fragments(self):
        with tempfile.TemporaryDirectory() as directory:
            first = self._write_wav(Path(directory) / "mic-00001.wav")
            second = self._write_wav(Path(directory) / "mic-00002.wav")
            window_path = transcribe_live.materialize_decode_window(
                {
                    "startMs": 500,
                    "endMs": 1_500,
                    "fragments": [
                        {
                            "path": first,
                            "fragmentStartMs": 0,
                            "fragmentEndMs": 1_000,
                            "clipStartMs": 500,
                            "clipEndMs": 1_000,
                        },
                        {
                            "path": second,
                            "fragmentStartMs": 1_000,
                            "fragmentEndMs": 2_000,
                            "clipStartMs": 1_000,
                            "clipEndMs": 1_500,
                        },
                    ],
                }
            )
            try:
                with wave.open(window_path, "rb") as audio:
                    self.assertEqual(audio.getnchannels(), 1)
                    self.assertEqual(audio.getsampwidth(), 2)
                    self.assertEqual(audio.getframerate(), 16_000)
                    self.assertEqual(audio.getnframes(), 16_000)
            finally:
                Path(window_path).unlink(missing_ok=True)

    def test_rejects_gaps_and_remote_audio_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self._write_wav(Path(directory) / "mic-00001.wav")
            base = {
                "startMs": 0,
                "endMs": 1_000,
                "fragments": [
                    {
                        "path": path,
                        "fragmentStartMs": 0,
                        "fragmentEndMs": 1_000,
                        "clipStartMs": 1,
                        "clipEndMs": 1_000,
                    }
                ],
            }
            with self.assertRaisesRegex(ValueError, "non-contiguous"):
                transcribe_live.materialize_decode_window(base)
            base["fragments"][0]["path"] = "https://example.invalid/audio.wav"
            with self.assertRaisesRegex(ValueError, "absolute local path"):
                transcribe_live.materialize_decode_window(base)

    def test_rejects_windows_larger_than_the_scheduler_bound(self):
        with self.assertRaisesRegex(ValueError, "4-second limit"):
            transcribe_live.materialize_decode_window(
                {"startMs": 0, "endMs": 4_001, "fragments": []}
            )


if __name__ == "__main__":
    unittest.main()
