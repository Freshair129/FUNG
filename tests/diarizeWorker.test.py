import importlib.util
import pathlib
import sys
import types
import unittest
from unittest.mock import Mock, patch


SCRIPT = pathlib.Path(__file__).parents[1] / "scripts" / "diarize.py"
SPEC = importlib.util.spec_from_file_location("diarize_worker", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DiarizationWorkerTests(unittest.TestCase):
    def test_pipeline_receives_local_decoded_mono_waveform(self):
        audio = [0.1, -0.1]
        decoder = Mock(return_value=audio)
        audio_module = types.ModuleType("faster_whisper.audio")
        audio_module.decode_audio = decoder
        faster_whisper = types.ModuleType("faster_whisper")
        faster_whisper.audio = audio_module
        tensor = Mock()
        tensor.unsqueeze.return_value = "waveform"
        torch = types.SimpleNamespace(from_numpy=Mock(return_value=tensor))
        pipeline = Mock(return_value="result")

        with patch.dict(sys.modules, {
            "faster_whisper": faster_whisper,
            "faster_whisper.audio": audio_module,
        }):
            result = MODULE.run_pipeline(pipeline, "fixture.wav", torch)

        decoder.assert_called_once_with("fixture.wav", sampling_rate=16000)
        torch.from_numpy.assert_called_once_with(audio)
        tensor.unsqueeze.assert_called_once_with(0)
        pipeline.assert_called_once_with({"waveform": "waveform", "sample_rate": 16000})
        self.assertEqual(result, "result")

    def test_pyannote_four_output_is_normalized_to_annotation(self):
        annotation = object()
        output = types.SimpleNamespace(speaker_diarization=annotation)
        self.assertIs(MODULE.annotation_from_output(output), annotation)
        self.assertIs(MODULE.annotation_from_output(annotation), annotation)

    def test_turn_json_keeps_anonymous_speakers_and_existing_fields(self):
        class Segment:
            start = 1.25
            end = 2.5

        class Annotation:
            def itertracks(self, yield_label):
                if not yield_label:
                    raise AssertionError("FUNG requires speaker labels from the annotation")
                yield Segment(), None, "speaker-a"
                yield Segment(), None, "speaker-b"

        turns = MODULE.turns_from_annotation(Annotation())
        self.assertEqual(turns, [
            {
                "speakerKey": "s:0",
                "displayName": "Speaker 1",
                "startMs": 1250,
                "endMs": 2500,
                "confidence": None,
            },
            {
                "speakerKey": "s:1",
                "displayName": "Speaker 2",
                "startMs": 1250,
                "endMs": 2500,
                "confidence": None,
            },
        ])


if __name__ == "__main__":
    unittest.main()
