"""Thai CTC transcript alignment helpers for the opt-in Qwen worker."""

from __future__ import annotations

import math
import unicodedata
from dataclasses import dataclass
from typing import Callable, Sequence


THAI_NIKHAHIT_SARA_AA = "\u0e4d\u0e32"
THAI_SARA_AM = "\u0e33"


@dataclass(frozen=True)
class AlignmentWord:
    text: str
    target_start: int
    target_end: int
    output_group: int


@dataclass(frozen=True)
class AlignmentGroup:
    text: str
    first_word: int
    end_word: int


@dataclass(frozen=True)
class AlignmentTarget:
    source_text: str
    token_ids: tuple[int, ...]
    words: tuple[AlignmentWord, ...]
    groups: tuple[AlignmentGroup, ...]


@dataclass(frozen=True)
class FrameSpan:
    start: int
    end: int
    mean_log_probability: float


@dataclass(frozen=True)
class ViterbiAlignment:
    token_spans: tuple[FrameSpan, ...]
    path_log_probability: float


def normalize_ctc_token(text: str) -> str:
    """Normalize only the Thai spelling absent from the pinned CTC vocabulary."""
    return unicodedata.normalize("NFC", text).replace(THAI_NIKHAHIT_SARA_AA, THAI_SARA_AM)


def prepare_alignment_target(
    text: str,
    tokenizer,
    word_tokenizer: Callable[[str], Sequence[str]],
) -> AlignmentTarget:
    """Map Thai words to CTC labels without changing the emitted source text."""
    if not isinstance(text, str) or not text.strip():
        raise ValueError("Qwen transcript is blank")

    source_text = text
    pieces = list(word_tokenizer(source_text))
    if not pieces or "".join(pieces) != source_text:
        raise ValueError("Thai tokenizer did not preserve the original Qwen transcript")

    delimiter_token = getattr(tokenizer, "word_delimiter_token", None)
    delimiter_id = tokenizer.convert_tokens_to_ids(delimiter_token) if delimiter_token else None
    blank_id = getattr(tokenizer, "pad_token_id", None)
    unknown_id = getattr(tokenizer, "unk_token_id", None)
    if not isinstance(delimiter_id, int) or delimiter_id in (blank_id, unknown_id):
        raise ValueError("Thai CTC tokenizer has no valid word-delimiter label")
    if not isinstance(blank_id, int):
        raise ValueError("Thai CTC tokenizer has no CTC blank label")

    target_ids: list[int] = []
    words: list[AlignmentWord] = []
    groups: list[dict[str, int | str]] = []
    pending_whitespace = ""

    for piece in pieces:
        if piece.isspace():
            pending_whitespace += piece
            continue
        if not piece:
            raise ValueError("Thai tokenizer returned an empty token")

        normalized_piece = normalize_ctc_token(piece)
        labels: list[int] = []
        for character in normalized_piece:
            label_id = tokenizer.convert_tokens_to_ids(character)
            if not isinstance(label_id, int) or label_id in (blank_id, unknown_id):
                raise ValueError(
                    f"Thai CTC vocabulary cannot represent U+{ord(character):04X}"
                )
            labels.append(label_id)
        if not labels:
            raise ValueError("Thai CTC tokenizer produced an empty label sequence")

        if words:
            target_ids.append(delimiter_id)
        target_start = len(target_ids)
        target_ids.extend(labels)
        target_end = len(target_ids)

        if pending_whitespace and groups:
            output_group = len(groups) - 1
            groups[output_group]["text"] = str(groups[output_group]["text"]) + pending_whitespace + piece
            groups[output_group]["end_word"] = len(words) + 1
        else:
            output_group = len(groups)
            groups.append({"text": pending_whitespace + piece, "first_word": len(words), "end_word": len(words) + 1})
        words.append(AlignmentWord(piece, target_start, target_end, output_group))
        pending_whitespace = ""

    if pending_whitespace and groups:
        groups[-1]["text"] = str(groups[-1]["text"]) + pending_whitespace
    if not words or not target_ids:
        raise ValueError("Qwen transcript has no CTC-alignable Thai tokens")
    if "".join(str(group["text"]) for group in groups) != source_text:
        raise ValueError("CTC output grouping did not preserve the original Qwen transcript")

    return AlignmentTarget(
        source_text=source_text,
        token_ids=tuple(target_ids),
        words=tuple(words),
        groups=tuple(
            AlignmentGroup(
                text=str(group["text"]),
                first_word=int(group["first_word"]),
                end_word=int(group["end_word"]),
            )
            for group in groups
        ),
    )


def ctc_viterbi_align(
    log_probabilities: Sequence[Sequence[float]],
    target_ids: Sequence[int],
    blank_id: int,
) -> ViterbiAlignment:
    """Find the best monotonic CTC path and return one span per target label."""
    if not log_probabilities or not target_ids:
        raise ValueError("CTC emissions and target labels must be non-empty")
    if not isinstance(blank_id, int) or any(label == blank_id for label in target_ids):
        raise ValueError("CTC target labels must not contain the blank label")

    frame_count = len(log_probabilities)
    label_count = len(target_ids)
    minimum_frames = label_count + sum(
        target_ids[index] == target_ids[index - 1]
        for index in range(1, label_count)
    )
    if frame_count < minimum_frames:
        raise ValueError("CTC emissions are too short to cover the target labels")

    state_labels: list[int] = [blank_id]
    for label_id in target_ids:
        state_labels.extend((int(label_id), blank_id))
    state_count = len(state_labels)
    if any(max(state_labels) >= len(frame) for frame in log_probabilities):
        raise ValueError("CTC emissions do not contain every target label")

    previous = [-math.inf] * state_count
    previous[0] = float(log_probabilities[0][blank_id])
    previous[1] = float(log_probabilities[0][state_labels[1]])
    backpointers = [bytearray(state_count) for _ in range(frame_count)]

    for frame_index in range(1, frame_count):
        row = log_probabilities[frame_index]
        current = [-math.inf] * state_count
        for state_index, label_id in enumerate(state_labels):
            best_score = previous[state_index]
            step = 0
            if state_index > 0 and previous[state_index - 1] > best_score:
                best_score = previous[state_index - 1]
                step = 1
            if (
                state_index > 1
                and label_id != blank_id
                and label_id != state_labels[state_index - 2]
                and previous[state_index - 2] > best_score
            ):
                best_score = previous[state_index - 2]
                step = 2
            emission = float(row[label_id])
            if math.isnan(emission):
                raise ValueError("CTC emissions contain NaN")
            if best_score != -math.inf and emission != -math.inf:
                current[state_index] = best_score + emission
                backpointers[frame_index][state_index] = step
            else:
                backpointers[frame_index][state_index] = 255
        previous = current

    final_candidates = [state_count - 1, state_count - 2]
    final_state = max(final_candidates, key=lambda state: previous[state])
    path_log_probability = previous[final_state]
    if not math.isfinite(path_log_probability):
        raise ValueError("CTC emissions have no valid path for the requested transcript")

    state_path = [final_state] * frame_count
    state = final_state
    for frame_index in range(frame_count - 1, 0, -1):
        step = backpointers[frame_index][state]
        if step == 255 or state < step:
            raise ValueError("CTC alignment backtrace is incomplete")
        state -= step
        state_path[frame_index - 1] = state
    if state not in (0, 1):
        raise ValueError("CTC alignment did not begin at the start of the target")

    token_spans: list[FrameSpan] = []
    for token_index, label_id in enumerate(target_ids):
        target_state = token_index * 2 + 1
        frames = [index for index, state_id in enumerate(state_path) if state_id == target_state]
        if not frames:
            raise ValueError("CTC alignment omitted a target label")
        mean_log_probability = sum(float(log_probabilities[index][label_id]) for index in frames) / len(frames)
        token_spans.append(FrameSpan(frames[0], frames[-1] + 1, mean_log_probability))

    return ViterbiAlignment(tuple(token_spans), path_log_probability)


def frame_span_to_milliseconds(
    start_frame: int,
    end_frame: int,
    stride_samples: int,
    sampling_rate: int,
    duration_ms: int,
) -> tuple[int, int]:
    if start_frame < 0 or end_frame <= start_frame:
        raise ValueError("CTC frame span is empty or reversed")
    if stride_samples <= 0 or sampling_rate <= 0 or duration_ms <= 0:
        raise ValueError("CTC timing metadata is invalid")
    start_ms = (start_frame * stride_samples * 1000) // sampling_rate
    end_numerator = end_frame * stride_samples * 1000
    end_ms = (end_numerator + sampling_rate - 1) // sampling_rate
    end_ms = min(end_ms, duration_ms)
    if start_ms < 0 or start_ms >= duration_ms or end_ms <= start_ms:
        raise ValueError("CTC frame span falls outside the source audio")
    return start_ms, end_ms


def aligned_output_spans(
    target: AlignmentTarget,
    token_spans: Sequence[FrameSpan],
    stride_samples: int,
    sampling_rate: int,
    duration_ms: int,
) -> list[dict[str, int | str | None]]:
    if len(token_spans) != len(target.token_ids):
        raise ValueError("CTC alignment did not return one span per target label")

    output: list[dict[str, int | str | None]] = []
    previous_start = -1
    previous_end = -1
    for group in target.groups:
        words = target.words[group.first_word : group.end_word]
        if not words:
            raise ValueError("CTC output group has no aligned words")
        first_span = token_spans[words[0].target_start]
        last_span = token_spans[words[-1].target_end - 1]
        start_ms, _ = frame_span_to_milliseconds(
            first_span.start, first_span.end, stride_samples, sampling_rate, duration_ms
        )
        _, end_ms = frame_span_to_milliseconds(
            last_span.start, last_span.end, stride_samples, sampling_rate, duration_ms
        )
        if start_ms < previous_start or end_ms < previous_end:
            raise ValueError("CTC output spans are not ordered")
        output.append({"startMs": start_ms, "endMs": end_ms, "text": group.text, "confidence": None})
        previous_start = start_ms
        previous_end = end_ms
    return output
