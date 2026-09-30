import pathlib
import sys
import unittest

import numpy as np


SCRIPTS = pathlib.Path(__file__).parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))

from transcribe_qwen_detailed import preprocess_audio_for_asr


class QwenAudioPreprocessingIntegrationTests(unittest.TestCase):
    def test_profiles_are_repeatable_source_preserving_and_duration_preserving(self):
        sample_rate = 16_000
        time = np.arange(sample_rate * 2, dtype=np.float32) / sample_rate
        rng = np.random.default_rng(20260930)
        source = (
            0.2 * np.sin(2 * np.pi * 440 * time)
            + 0.01 * rng.standard_normal(time.size)
        ).astype(np.float32)
        original = source.copy()

        for profile in ("raw", "afftdn", "speechnorm"):
            with self.subTest(profile=profile):
                first = preprocess_audio_for_asr(source, profile)
                second = preprocess_audio_for_asr(source, profile)
                self.assertEqual(first.shape, source.shape)
                self.assertTrue(np.isfinite(first).all())
                np.testing.assert_array_equal(first, second)
                np.testing.assert_array_equal(source, original)
                self.assertIsNot(first, source)


if __name__ == "__main__":
    unittest.main()
