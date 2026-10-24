"""Bounded worker output plumbing; fake codec is not codec/runtime acceptance."""
import importlib.util
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("fung_transcode", Path(__file__).resolve().parents[1] / "scripts/transcode_audio.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class TranscodeBudgetTests(unittest.TestCase):
    def test_output_limit_is_enforced_before_writing(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "output.tmp"
            with worker.BoundedOutput(path, 8) as output:
                self.assertEqual(output.write(b"12345678"), 8)
                with self.assertRaisesRegex(RuntimeError, "byte ceiling"):
                    output.write(b"9")
            self.assertEqual(path.read_bytes(), b"12345678")

    def test_invalid_limit_is_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            for limit in (0, worker.MAX_OUTPUT_BYTES + 1):
                with self.assertRaises(ValueError):
                    worker.BoundedOutput(Path(root) / "output.tmp", limit)

    def test_mux_overflow_preserves_previous_export_and_removes_stage(self):
        class Context:
            def __enter__(self): return self
            def __exit__(self, *args): return False
        class Source(Context):
            streams = [types.SimpleNamespace(type="audio")]
            def decode(self, stream): yield object()
        class Encoder:
            def encode(self, frame=None): return [b"123456789"] if frame is not None else []
        class Destination(Context):
            def __init__(self, output): self.output = output
            def add_stream(self, *args, **kwargs): return Encoder()
            def mux(self, packet): self.output.write(packet)
        class Resampler:
            def __init__(self, **kwargs): pass
            def resample(self, frame): return [frame] if frame is not None else []
        av = types.SimpleNamespace(
            open=lambda path, mode, **kwargs: Source() if mode == "r" else Destination(path),
            audio=types.SimpleNamespace(resampler=types.SimpleNamespace(AudioResampler=Resampler)),
        )
        with tempfile.TemporaryDirectory() as root, patch.dict("sys.modules", {"av": av}):
            target = Path(root) / "audio.mp3"
            target.write_bytes(b"previous valid output")
            with self.assertRaisesRegex(RuntimeError, "byte ceiling"):
                worker.transcode(Path(root) / "source.m4a", target, "mp3", max_output_bytes=8)
            self.assertEqual(target.read_bytes(), b"previous valid output")
            self.assertFalse(target.with_name(".audio.mp3.tmp").exists())


if __name__ == "__main__":
    unittest.main()
