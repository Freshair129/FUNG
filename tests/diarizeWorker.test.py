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


class _Response:
    def __init__(self, status_code):
        self.status_code = status_code


def _http_error(name, status):
    error_type = type(name, (Exception,), {})
    error = error_type("request failed")
    error.response = _Response(status)
    return error


class ModelAccessCodeTests(unittest.TestCase):
    def test_gated_repository_means_terms_not_accepted(self):
        self.assertEqual(MODULE.model_access_code(_http_error("GatedRepoError", 403)), "terms_not_accepted")
        self.assertEqual(MODULE.model_access_code(_http_error("HfHubHTTPError", 403)), "terms_not_accepted")

    def test_unauthorized_means_token_invalid(self):
        self.assertEqual(MODULE.model_access_code(_http_error("HfHubHTTPError", 401)), "token_invalid")

    def test_no_connection_means_network(self):
        self.assertEqual(MODULE.model_access_code(ConnectionError("offline")), "network")
        local = type("LocalEntryNotFoundError", (Exception,), {})("not cached and offline")
        self.assertEqual(MODULE.model_access_code(local), "network")

    def test_cause_chain_is_followed(self):
        outer = RuntimeError("pipeline failed")
        outer.__cause__ = _http_error("GatedRepoError", 403)
        self.assertEqual(MODULE.model_access_code(outer), "terms_not_accepted")

    def test_anything_else_is_unknown(self):
        self.assertEqual(MODULE.model_access_code(ValueError("boom")), "unknown")


class TokenProblemTests(unittest.TestCase):
    # The hub returns `401 GatedRepo` for both a wrong token and unaccepted
    # terms, so the worker asks `whoami` first to tell them apart.
    def test_missing_token_is_reported_before_any_request(self):
        whoami = Mock()
        self.assertEqual(MODULE.token_problem(None, whoami), "token_invalid")
        whoami.assert_not_called()

    def test_rejected_identity_means_token_invalid(self):
        whoami = Mock(side_effect=_http_error("HfHubHTTPError", 401))
        self.assertEqual(MODULE.token_problem("hf_x", whoami), "token_invalid")

    def test_valid_identity_defers_to_the_repo_request(self):
        self.assertIsNone(MODULE.token_problem("hf_x", Mock(return_value={"name": "u"})))

    def test_offline_identity_check_means_network(self):
        whoami = Mock(side_effect=ConnectionError("offline"))
        self.assertEqual(MODULE.token_problem("hf_x", whoami), "network")


class AddonSiteTests(unittest.TestCase):
    def test_addon_site_goes_first_on_sys_path(self):
        original = list(sys.path)
        try:
            with patch.dict("os.environ", {"FUNG_DIARIZATION_SITE": str(pathlib.Path(__file__).parent)}):
                MODULE.add_addon_site()
            self.assertEqual(pathlib.Path(sys.path[0]), pathlib.Path(__file__).parent)
        finally:
            sys.path[:] = original

    def test_no_addon_leaves_sys_path_alone(self):
        original = list(sys.path)
        with patch.dict("os.environ", {}, clear=False):
            import os
            os.environ.pop("FUNG_DIARIZATION_SITE", None)
            MODULE.add_addon_site()
        self.assertEqual(sys.path, original)


if __name__ == "__main__":
    unittest.main()
