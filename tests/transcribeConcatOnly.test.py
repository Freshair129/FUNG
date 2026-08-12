"""Focused integration tests for scripts/transcribe.py --concat-only."""

import json
import os
import subprocess
import sys
import tempfile
import unittest
import wave
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "transcribe.py"


def write_silent_wav(path: Path, frames: int) -> None:
    with wave.open(str(path), "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(16000)
        wav_file.writeframes(b"\0\0" * frames)


def concat_temps(directory: Path, destination: Path) -> list[Path]:
    return list(directory.glob(f".{destination.name}.*.tmp"))


class ConcatOnlyTests(unittest.TestCase):
    def run_worker(
        self, *args: str, env: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            env=env,
        )

    def test_manifest_atomically_replaces_destination_without_loading_a_model(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            first = temp_path / "first.wav"
            second = temp_path / "second.wav"
            output = temp_path / "combined.wav"
            manifest = temp_path / "segments.txt"
            write_silent_wav(first, 16000)
            write_silent_wav(second, 8000)
            manifest.write_text(f"{first}\n{second}\n", encoding="utf-8")
            previous_output = b"preserve-this-until-success"
            output.write_bytes(previous_output)

            result = self.run_worker(
                "--manifest",
                str(manifest),
                "--concat-only",
                str(output),
                "--model",
                "not-loaded-in-concat-mode",
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")
            self.assertIn("PROGRESS 100", result.stderr)
            self.assertNotEqual(output.read_bytes(), previous_output)
            self.assertEqual(concat_temps(temp_path, output), [])
            with wave.open(str(output), "rb") as wav_file:
                self.assertEqual(wav_file.getnchannels(), 1)
                self.assertEqual(wav_file.getsampwidth(), 2)
                self.assertEqual(wav_file.getframerate(), 16000)
                self.assertEqual(wav_file.getnframes(), 24000)

    def test_missing_input_fails_before_creating_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            missing = temp_path / "missing.wav"
            output = temp_path / "combined.wav"

            result = self.run_worker(str(missing), "--concat-only", str(output))

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("audio input not found", result.stderr)
            self.assertFalse(output.exists())
            self.assertEqual(concat_temps(temp_path, output), [])

    def test_corrupt_later_input_preserves_destination_and_cleans_temp(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            first = temp_path / "first.wav"
            corrupt = temp_path / "corrupt.wav"
            output = temp_path / "combined.wav"
            write_silent_wav(first, 16000)
            corrupt.write_bytes(b"not-a-decodable-audio-file")
            previous_output = b"existing-destination-must-survive"
            output.write_bytes(previous_output)

            result = self.run_worker(str(first), str(corrupt), "--concat-only", str(output))

            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(output.read_bytes(), previous_output)
            self.assertEqual(concat_temps(temp_path, output), [])

    def test_output_alias_of_input_is_rejected_without_modifying_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            source = temp_path / "source.wav"
            write_silent_wav(source, 16000)
            source_bytes = source.read_bytes()
            (temp_path / "path-alias").mkdir()
            resolved_alias = temp_path / "path-alias" / ".." / source.name

            result = self.run_worker(str(source), "--concat-only", str(resolved_alias))

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("must not alias an input", result.stderr)
            self.assertEqual(source.read_bytes(), source_bytes)
            self.assertEqual(concat_temps(temp_path, source), [])

    def test_existing_transcription_mode_still_emits_combined_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            fake_package = temp_path / "faster_whisper"
            fake_package.mkdir()
            (fake_package / "__init__.py").write_text(
                """
class _Segment:
    start = 0.1
    end = 0.2
    text = " test "
    no_speech_prob = 0.25

class _Info:
    duration = 1.0
    language = "en"
    language_probability = 0.9

class WhisperModel:
    def __init__(self, *args, **kwargs):
        pass
    def transcribe(self, *args, **kwargs):
        return iter([_Segment()]), _Info()
""",
                encoding="utf-8",
            )
            env = os.environ.copy()
            env["PYTHONPATH"] = str(temp_path) + os.pathsep + env.get("PYTHONPATH", "")

            result = self.run_worker("first.wav", "second.wav", "--profile", "cpu", env=env)

            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(result.stdout)
            self.assertEqual(payload["durationMs"], 2000)
            self.assertEqual(
                payload["segments"],
                [
                    {"startMs": 100, "endMs": 200, "text": "test", "confidence": 0.75},
                    {"startMs": 1100, "endMs": 1200, "text": "test", "confidence": 0.75},
                ],
            )


if __name__ == "__main__":
    unittest.main()
