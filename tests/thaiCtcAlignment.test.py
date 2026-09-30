import pathlib
import sys
import unittest


SCRIPT = pathlib.Path(__file__).parents[1] / "scripts" / "thai_ctc_alignment.py"
sys.path.insert(0, str(SCRIPT.parent))
import thai_ctc_alignment as MODULE


class FakeTokenizer:
    word_delimiter_token = "|"
    pad_token_id = 0
    unk_token_id = 99
    vocab = {
        "|": 1,
        "\u0e15": 2,
        "\u0e33": 3,
        "\u0e07": 4,
        "\u0e32": 5,
        "\u0e19": 6,
        "\u00e1": 7,
    }

    def convert_tokens_to_ids(self, token):
        return self.vocab.get(token, self.unk_token_id)


class TargetPreparationTests(unittest.TestCase):
    def test_alignment_normalizes_one_thai_sequence_and_preserves_original_text(self):
        source = "\u0e15\u0e4d\u0e32 \u0e07\u0e32\u0e19"
        pieces = ["\u0e15\u0e4d\u0e32", " ", "\u0e07\u0e32\u0e19"]
        target = MODULE.prepare_alignment_target(source, FakeTokenizer(), lambda _: pieces)

        self.assertEqual(target.source_text, source)
        self.assertEqual(target.token_ids, (2, 3, 1, 4, 5, 6))
        self.assertEqual([word.text for word in target.words], ["\u0e15\u0e4d\u0e32", "\u0e07\u0e32\u0e19"])
        self.assertEqual([group.text for group in target.groups], [source])

    def test_rejects_out_of_vocabulary_text(self):
        with self.assertRaisesRegex(ValueError, r"U\+0041"):
            MODULE.prepare_alignment_target("A", FakeTokenizer(), lambda text: [text])

    def test_rejects_tokenizer_that_drops_source_text(self):
        with self.assertRaisesRegex(ValueError, "did not preserve"):
            MODULE.prepare_alignment_target("\u0e15", FakeTokenizer(), lambda _: [])

    def test_alignment_normalizes_labels_but_preserves_original_unicode_spelling(self):
        source = "a\u0301"
        target = MODULE.prepare_alignment_target(source, FakeTokenizer(), lambda _: [source])

        self.assertEqual(target.source_text, source)
        self.assertEqual(target.token_ids, (7,))
        self.assertEqual(target.groups[0].text, source)


class CtcViterbiTests(unittest.TestCase):
    @staticmethod
    def emissions(labels):
        rows = []
        for label in labels:
            row = [-10.0, -10.0, -10.0, -10.0]
            row[label] = 0.0
            rows.append(row)
        return rows

    def test_returns_monotonic_token_spans_for_repeated_frames(self):
        result = MODULE.ctc_viterbi_align(self.emissions([0, 1, 1, 0, 2, 0]), [1, 2], 0)

        self.assertEqual([(span.start, span.end) for span in result.token_spans], [(1, 3), (4, 5)])
        self.assertGreater(result.path_log_probability, -1.0)

    def test_repeated_target_labels_require_an_intervening_blank_frame(self):
        result = MODULE.ctc_viterbi_align(self.emissions([0, 1, 0, 1, 0]), [1, 1], 0)

        self.assertEqual([(span.start, span.end) for span in result.token_spans], [(1, 2), (3, 4)])
        with self.assertRaisesRegex(ValueError, "too short"):
            MODULE.ctc_viterbi_align(self.emissions([1, 1]), [1, 1], 0)

    def test_rejects_target_without_a_valid_path(self):
        rows = [[0.0, float("-inf"), -1.0], [0.0, float("-inf"), -1.0]]
        with self.assertRaisesRegex(ValueError, "no valid path"):
            MODULE.ctc_viterbi_align(rows, [1], 0)


class TimestampTests(unittest.TestCase):
    def test_frame_span_rounds_outward_and_clamps_to_audio(self):
        self.assertEqual(MODULE.frame_span_to_milliseconds(1, 2, 320, 16000, 1000), (20, 40))
        self.assertEqual(MODULE.frame_span_to_milliseconds(49, 51, 320, 16000, 1000), (980, 1000))

    def test_rejects_zero_duration_frame_mapping(self):
        with self.assertRaisesRegex(ValueError, "outside"):
            MODULE.frame_span_to_milliseconds(50, 51, 320, 16000, 1000)


if __name__ == "__main__":
    unittest.main()
