//! Revision-aware local transcript scheduling primitives.
//!
//! Audio fragments are durable before this module sees them. Hypotheses stay
//! provisional in memory; callers must persist a `CommitCandidate` through
//! the Genesis batch boundary before emitting it as committed or adapting it
//! to the legacy `live-segment` event.
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};

use serde::Serialize;

pub(crate) const LEGACY_FRAGMENT_MS: i64 = 8_000;
pub(crate) const REVISIONED_FRAGMENT_MS: i64 = 2_000;
pub(crate) const DECODE_WINDOW_MS: i64 = 4_000;
pub(crate) const DECODE_OVERLAP_MS: i64 = 1_000;
pub(crate) const MAX_WINDOW_FRAGMENTS: usize = 3;
const MAX_UTTERANCE_TEXT_BYTES: usize = 16 * 1024;
const MATCH_TOLERANCE_MS: i64 = 1_000;
const REQUIRED_STABLE_WINDOWS: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiveTranscriptProfile {
    Chunked,
    Revisioned,
}

impl LiveTranscriptProfile {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("chunked") {
            "chunked" => Ok(Self::Chunked),
            "revisioned" => Ok(Self::Revisioned),
            _ => Err("transcript profile must be `chunked` or `revisioned`".to_string()),
        }
    }

    pub(crate) fn fragment_ms(self) -> i64 {
        match self {
            Self::Chunked => LEGACY_FRAGMENT_MS,
            Self::Revisioned => REVISIONED_FRAGMENT_MS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudioFragmentRef {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) sequence_no: i64,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WindowFragment {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) fragment_start_ms: i64,
    pub(crate) fragment_end_ms: i64,
    pub(crate) clip_start_ms: i64,
    pub(crate) clip_end_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodeWindow {
    pub(crate) id: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) is_final: bool,
    pub(crate) fragments: Vec<WindowFragment>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveTranscriptEvent {
    pub(crate) schema_version: u8,
    pub(crate) event_id: String,
    pub(crate) event_type: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) meeting_session_id: String,
    pub(crate) source_session_id: String,
    pub(crate) track_id: String,
    pub(crate) source_generation: i64,
    pub(crate) utterance_id: Option<String>,
    pub(crate) revision: Option<u64>,
    pub(crate) supersedes_revision: Option<u64>,
    pub(crate) persisted_revision: Option<u64>,
    pub(crate) state: String,
    pub(crate) origin: String,
    pub(crate) start_ms: Option<i64>,
    pub(crate) end_ms: Option<i64>,
    pub(crate) text: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) attribution: TranscriptAttribution,
    pub(crate) audio_refs: Vec<String>,
    pub(crate) model_run_id: Option<String>,
    pub(crate) committed_cursor: Option<i64>,
    pub(crate) emitted_at: String,
    pub(crate) received_at: Option<String>,
    pub(crate) source_clock_uncertainty_ms: Option<u64>,
    pub(crate) review_state: String,
    pub(crate) quality_flags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptAttribution {
    pub(crate) kind: String,
    pub(crate) participant_session_id: Option<String>,
    pub(crate) speaker_cluster_id: String,
    pub(crate) label_snapshot: String,
    pub(crate) evidence_revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AudioGap {
    pub(crate) expected_sequence_no: i64,
    pub(crate) observed_sequence_no: i64,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WindowBatch {
    pub(crate) gap: Option<AudioGap>,
    pub(crate) windows: Vec<DecodeWindow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScheduleError {
    InvalidRange,
    FragmentTooLong,
    SequenceWentBackwards,
    OverlappingAudio,
    WindowNotFullyCovered,
    TooManyFragments,
}

/// Produces 4-second, per-source windows with a 1-second overlap. Each source
/// remains independent, so mic/system timelines are never interleaved into a
/// false participant track. The scheduler retains only paths and range data.
pub(crate) struct RollingWindowScheduler {
    next_sequence_no: Option<i64>,
    next_window_start_ms: Option<i64>,
    last_end_ms: Option<i64>,
    fragments: VecDeque<AudioFragmentRef>,
    window_ordinal: u64,
}

impl RollingWindowScheduler {
    pub(crate) fn new() -> Self {
        Self {
            next_sequence_no: None,
            next_window_start_ms: None,
            last_end_ms: None,
            fragments: VecDeque::new(),
            window_ordinal: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.next_sequence_no = None;
        self.next_window_start_ms = None;
        self.last_end_ms = None;
        self.fragments.clear();
    }

    pub(crate) fn push(
        &mut self,
        source_id: &str,
        fragment: AudioFragmentRef,
    ) -> Result<WindowBatch, ScheduleError> {
        if fragment.sequence_no < 0 || fragment.start_ms < 0 || fragment.end_ms <= fragment.start_ms
        {
            return Err(ScheduleError::InvalidRange);
        }
        if fragment.end_ms - fragment.start_ms > REVISIONED_FRAGMENT_MS {
            return Err(ScheduleError::FragmentTooLong);
        }

        let expected_sequence = self.next_sequence_no.unwrap_or(fragment.sequence_no);
        let expected_start = self.last_end_ms.unwrap_or(fragment.start_ms);
        if fragment.sequence_no < expected_sequence {
            return Err(ScheduleError::SequenceWentBackwards);
        }
        if fragment.start_ms < expected_start {
            return Err(ScheduleError::OverlappingAudio);
        }

        let gap = (fragment.sequence_no != expected_sequence
            || fragment.start_ms != expected_start)
            .then_some(AudioGap {
                expected_sequence_no: expected_sequence,
                observed_sequence_no: fragment.sequence_no,
                start_ms: expected_start,
                end_ms: fragment.start_ms,
            });
        if gap.is_some() {
            // Never decode across an unaccounted source interval. The new
            // generation of windows starts at the first fragment after it.
            self.fragments.clear();
            self.next_window_start_ms = Some(fragment.start_ms);
        }

        if self.next_window_start_ms.is_none() {
            self.next_window_start_ms = Some(fragment.start_ms);
        }
        self.next_sequence_no = Some(fragment.sequence_no + 1);
        self.last_end_ms = Some(fragment.end_ms);
        self.fragments.push_back(fragment);

        let mut windows = Vec::new();
        loop {
            let start_ms = self.next_window_start_ms.expect("window start initialized");
            let end_ms = start_ms + DECODE_WINDOW_MS;
            if self.last_end_ms.unwrap_or_default() < end_ms {
                break;
            }
            windows.push(self.build_window(source_id, start_ms, end_ms, false)?);
            self.window_ordinal = self.window_ordinal.saturating_add(1);
            self.next_window_start_ms = Some(start_ms + DECODE_WINDOW_MS - DECODE_OVERLAP_MS);
            self.discard_expired_fragments();
        }
        Ok(WindowBatch { gap, windows })
    }

    /// Seals a shorter last window after capture stops. This is a decode
    /// request only; durable revisions still require the guarded batch API.
    pub(crate) fn flush(&mut self, source_id: &str) -> Result<Option<DecodeWindow>, ScheduleError> {
        let Some(end_ms) = self.last_end_ms else {
            return Ok(None);
        };
        let Some(next_start) = self.next_window_start_ms else {
            return Ok(None);
        };
        if end_ms <= next_start {
            return Ok(None);
        }
        let start_ms = next_start.max(end_ms - DECODE_WINDOW_MS);
        if end_ms - start_ms < 400 {
            return Ok(None);
        }
        let window = self.build_window(source_id, start_ms, end_ms, true)?;
        self.window_ordinal = self.window_ordinal.saturating_add(1);
        self.next_window_start_ms = Some(end_ms);
        self.discard_expired_fragments();
        Ok(Some(window))
    }

    fn build_window(
        &self,
        source_id: &str,
        start_ms: i64,
        end_ms: i64,
        is_final: bool,
    ) -> Result<DecodeWindow, ScheduleError> {
        if start_ms < 0 || end_ms <= start_ms || end_ms - start_ms > DECODE_WINDOW_MS {
            return Err(ScheduleError::InvalidRange);
        }
        let mut cursor_ms = start_ms;
        let mut fragments = Vec::new();
        for fragment in &self.fragments {
            let clip_start_ms = start_ms.max(fragment.start_ms);
            let clip_end_ms = end_ms.min(fragment.end_ms);
            if clip_start_ms >= clip_end_ms {
                continue;
            }
            if clip_start_ms != cursor_ms {
                return Err(ScheduleError::WindowNotFullyCovered);
            }
            fragments.push(WindowFragment {
                id: fragment.id.clone(),
                path: fragment.path.clone(),
                fragment_start_ms: fragment.start_ms,
                fragment_end_ms: fragment.end_ms,
                clip_start_ms,
                clip_end_ms,
            });
            cursor_ms = clip_end_ms;
        }
        if cursor_ms != end_ms {
            return Err(ScheduleError::WindowNotFullyCovered);
        }
        if fragments.len() > MAX_WINDOW_FRAGMENTS {
            return Err(ScheduleError::TooManyFragments);
        }
        Ok(DecodeWindow {
            id: format!("{source_id}:{}:{}", self.window_ordinal, end_ms),
            start_ms,
            end_ms,
            is_final,
            fragments,
        })
    }

    fn discard_expired_fragments(&mut self) {
        let retain_from = self.next_window_start_ms.unwrap_or_default();
        while self
            .fragments
            .front()
            .is_some_and(|fragment| fragment.end_ms <= retain_from)
        {
            self.fragments.pop_front();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TranscriptHypothesis {
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
    pub(crate) language: Option<String>,
    pub(crate) confidence: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TranscriptUpdate {
    Provisional {
        utterance_id: String,
        revision: u64,
        hypothesis: TranscriptHypothesis,
    },
    Discard {
        utterance_id: String,
        revision: u64,
    },
    CommitCandidate {
        utterance_id: String,
        revision: u64,
        expected_persisted_revision: i64,
        hypothesis: TranscriptHypothesis,
    },
}

#[derive(Clone, Debug)]
struct TrackedUtterance {
    utterance_id: String,
    display_revision: u64,
    latest: TranscriptHypothesis,
    stable_windows: u8,
    last_seen_window: u64,
    persisted_revision: i64,
    committed_text: Option<String>,
    committed_hypothesis: Option<TranscriptHypothesis>,
    candidate_offered_revision: Option<u64>,
}

/// Tracks provisional hypotheses by source-time overlap, never by text alone.
/// A stable result becomes a commit candidate; the caller must atomically
/// persist it before calling `mark_committed` or emitting committed events.
pub(crate) struct RevisionTracker {
    utterances: HashMap<String, TrackedUtterance>,
    window_ordinal: u64,
}

impl RevisionTracker {
    pub(crate) fn new() -> Self {
        Self {
            utterances: HashMap::new(),
            window_ordinal: 0,
        }
    }

    pub(crate) fn observe(
        &mut self,
        window: &DecodeWindow,
        hypotheses: Vec<TranscriptHypothesis>,
    ) -> Vec<TranscriptUpdate> {
        self.window_ordinal = self.window_ordinal.saturating_add(1);
        let current_window = self.window_ordinal;
        let mut updates = Vec::new();
        let mut valid = hypotheses
            .into_iter()
            .filter(|item| {
                item.start_ms >= window.start_ms
                    && item.end_ms > item.start_ms
                    && item.end_ms <= window.end_ms
                    && item.text.len() <= MAX_UTTERANCE_TEXT_BYTES
                    && !item.text.trim().is_empty()
            })
            .collect::<Vec<_>>();
        valid.sort_by_key(|item| (item.start_ms, item.end_ms));

        for (candidate_ordinal, hypothesis) in valid.into_iter().enumerate() {
            let best_id = self
                .utterances
                .values()
                .filter(|tracked| temporal_match(&tracked.latest, &hypothesis))
                .max_by_key(|tracked| temporal_overlap(&tracked.latest, &hypothesis))
                .map(|tracked| tracked.utterance_id.clone());
            let utterance_id = best_id.unwrap_or_else(|| {
                format!(
                    "utt:{}:{}:{}",
                    window.id, candidate_ordinal, hypothesis.start_ms
                )
            });
            let tracked = self
                .utterances
                .entry(utterance_id.clone())
                .or_insert_with(|| TrackedUtterance {
                    utterance_id: utterance_id.clone(),
                    display_revision: 1,
                    latest: hypothesis.clone(),
                    stable_windows: 0,
                    last_seen_window: 0,
                    persisted_revision: 0,
                    committed_text: None,
                    committed_hypothesis: None,
                    candidate_offered_revision: None,
                });
            let first_observation = tracked.last_seen_window == 0;
            let unchanged = hypothesis_equal(&tracked.latest, &hypothesis);
            if unchanged && tracked.last_seen_window + 1 == current_window {
                tracked.stable_windows = tracked.stable_windows.saturating_add(1);
            } else {
                tracked.stable_windows = 1;
            }
            if !unchanged {
                tracked.display_revision = tracked.display_revision.saturating_add(1);
                tracked.latest = hypothesis.clone();
                tracked.candidate_offered_revision = None;
            } else {
                tracked.latest.confidence = hypothesis.confidence;
            }
            tracked.last_seen_window = current_window;
            if !unchanged || first_observation {
                updates.push(TranscriptUpdate::Provisional {
                    utterance_id: utterance_id.clone(),
                    revision: tracked.display_revision,
                    hypothesis: hypothesis.clone(),
                });
            }

            let changed_since_commit = tracked
                .committed_hypothesis
                .as_ref()
                .is_none_or(|committed| !hypothesis_equal(committed, &hypothesis));
            let stable_enough = window.is_final
                || hypothesis.end_ms <= window.end_ms - DECODE_OVERLAP_MS
                || tracked.stable_windows >= REQUIRED_STABLE_WINDOWS;
            if changed_since_commit
                && stable_enough
                && tracked.candidate_offered_revision != Some(tracked.display_revision)
            {
                tracked.candidate_offered_revision = Some(tracked.display_revision);
                updates.push(TranscriptUpdate::CommitCandidate {
                    utterance_id,
                    revision: tracked.display_revision,
                    expected_persisted_revision: tracked.persisted_revision,
                    hypothesis,
                });
            }
        }

        let stale = self
            .utterances
            .values()
            .filter(|tracked| {
                tracked.last_seen_window + 1 < current_window
                    && tracked.committed_text.is_none()
                    && tracked.latest.end_ms <= window.end_ms
            })
            .map(|tracked| tracked.utterance_id.clone())
            .collect::<Vec<_>>();
        for utterance_id in stale {
            if let Some(tracked) = self.utterances.remove(&utterance_id) {
                updates.push(TranscriptUpdate::Discard {
                    utterance_id,
                    revision: tracked.display_revision,
                });
            }
        }
        let retain_after_ms = window.start_ms - MATCH_TOLERANCE_MS;
        self.utterances.retain(|_, tracked| {
            tracked.committed_text.is_none() || tracked.latest.end_ms > retain_after_ms
        });
        updates
    }

    /// Advance durable state only after the Genesis batch returns successfully.
    pub(crate) fn mark_committed(
        &mut self,
        utterance_id: &str,
        persisted_revision: i64,
        effective_text: &str,
    ) -> Result<(), String> {
        let Some(tracked) = self.utterances.get_mut(utterance_id) else {
            return Err("unknown provisional utterance".to_string());
        };
        if persisted_revision != tracked.persisted_revision + 1 {
            return Err("persisted transcript revision is not monotonic".to_string());
        }
        tracked.persisted_revision = persisted_revision;
        tracked.committed_text = Some(effective_text.to_string());
        let mut committed_hypothesis = tracked.latest.clone();
        committed_hypothesis.text = effective_text.to_string();
        tracked.committed_hypothesis = Some(committed_hypothesis);
        tracked.candidate_offered_revision = None;
        Ok(())
    }

    pub(crate) fn mark_commit_failed(&mut self, utterance_id: &str) {
        if let Some(tracked) = self.utterances.get_mut(utterance_id) {
            tracked.candidate_offered_revision = None;
        }
    }

    /// Discard uncommitted hypotheses after a source discontinuity or worker
    /// restart. Already committed hypotheses remain available for later
    /// revision matching, but no provisional text crosses an audio gap.
    pub(crate) fn discard_uncommitted(&mut self) -> Vec<TranscriptUpdate> {
        let ids = self
            .utterances
            .values()
            .filter(|tracked| tracked.committed_text.is_none())
            .map(|tracked| tracked.utterance_id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|utterance_id| {
                self.utterances
                    .remove(&utterance_id)
                    .map(|tracked| TranscriptUpdate::Discard {
                        utterance_id,
                        revision: tracked.display_revision,
                    })
            })
            .collect()
    }

    /// A source batch cannot mark audio complete while a current hypothesis
    /// for that audio is still provisional or waiting for a commit attempt.
    pub(crate) fn has_pending_revision(&self) -> bool {
        self.utterances.values().any(|tracked| {
            tracked
                .committed_hypothesis
                .as_ref()
                .is_none_or(|committed| !hypothesis_equal(committed, &tracked.latest))
        })
    }
}

fn temporal_overlap(left: &TranscriptHypothesis, right: &TranscriptHypothesis) -> i64 {
    left.end_ms.min(right.end_ms) - left.start_ms.max(right.start_ms)
}

fn temporal_match(left: &TranscriptHypothesis, right: &TranscriptHypothesis) -> bool {
    temporal_overlap(left, right) > 0
        && (left.start_ms - right.start_ms).abs() <= MATCH_TOLERANCE_MS
        && left.language == right.language
}

fn hypothesis_equal(left: &TranscriptHypothesis, right: &TranscriptHypothesis) -> bool {
    left.start_ms == right.start_ms
        && left.end_ms == right.end_ms
        && left.text == right.text
        && left.language == right.language
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragment(sequence_no: i64, start_ms: i64, end_ms: i64) -> AudioFragmentRef {
        AudioFragmentRef {
            id: format!("chunk-{sequence_no}"),
            path: format!("/fixture/mic-{sequence_no:05}.wav"),
            sequence_no,
            start_ms,
            end_ms,
        }
    }

    fn hypothesis(start_ms: i64, end_ms: i64, text: &str) -> TranscriptHypothesis {
        TranscriptHypothesis {
            start_ms,
            end_ms,
            text: text.to_string(),
            language: Some("th".to_string()),
            confidence: None,
        }
    }

    fn window(id: &str, start_ms: i64, end_ms: i64) -> DecodeWindow {
        DecodeWindow {
            id: id.to_string(),
            start_ms,
            end_ms,
            is_final: false,
            fragments: Vec::new(),
        }
    }

    #[test]
    fn profile_keeps_chunked_default_and_revisioned_fragments_bounded() {
        assert_eq!(
            LiveTranscriptProfile::parse(None),
            Ok(LiveTranscriptProfile::Chunked)
        );
        assert_eq!(
            LiveTranscriptProfile::parse(Some("revisioned")),
            Ok(LiveTranscriptProfile::Revisioned)
        );
        assert!(LiveTranscriptProfile::parse(Some("unknown")).is_err());
        assert_eq!(LiveTranscriptProfile::Chunked.fragment_ms(), 8_000);
        assert_eq!(LiveTranscriptProfile::Revisioned.fragment_ms(), 2_000);
    }

    #[test]
    fn scheduler_builds_bounded_windows_with_exact_one_second_overlap() {
        let mut scheduler = RollingWindowScheduler::new();
        let first = scheduler.push("mic", fragment(0, 0, 2_000)).unwrap();
        assert!(first.windows.is_empty());
        let second = scheduler.push("mic", fragment(1, 2_000, 4_000)).unwrap();
        assert_eq!(
            (second.windows[0].start_ms, second.windows[0].end_ms),
            (0, 4_000)
        );
        assert_eq!(second.windows[0].fragments.len(), 2);
        let third = scheduler.push("mic", fragment(2, 4_000, 6_000)).unwrap();
        assert!(third.windows.is_empty());
        let fourth = scheduler.push("mic", fragment(3, 6_000, 8_000)).unwrap();
        assert_eq!(
            (fourth.windows[0].start_ms, fourth.windows[0].end_ms),
            (3_000, 7_000)
        );
        assert_eq!(fourth.windows[0].fragments.len(), 3);
        assert_eq!(fourth.windows[0].fragments[0].clip_start_ms, 3_000);
        assert_eq!(fourth.windows[0].fragments[2].clip_end_ms, 7_000);
        assert!(fourth.windows[0].fragments.len() <= MAX_WINDOW_FRAGMENTS);
    }

    #[test]
    fn scheduler_resets_windows_and_reports_source_sequence_or_time_gaps() {
        let mut scheduler = RollingWindowScheduler::new();
        scheduler.push("system", fragment(0, 0, 2_000)).unwrap();
        let batch = scheduler.push("system", fragment(2, 4_000, 6_000)).unwrap();
        assert_eq!(
            batch.gap.as_ref().map(|gap| gap.expected_sequence_no),
            Some(1)
        );
        assert_eq!(
            batch.gap.as_ref().map(|gap| gap.observed_sequence_no),
            Some(2)
        );
        assert!(
            batch.windows.is_empty(),
            "a decode window cannot cross an unaccounted gap"
        );
        assert_eq!(
            scheduler
                .push("system", fragment(1, 5_000, 7_000))
                .unwrap_err(),
            ScheduleError::SequenceWentBackwards
        );
    }

    #[test]
    fn stop_flushes_a_bounded_tail_only_once() {
        let mut scheduler = RollingWindowScheduler::new();
        scheduler.push("mic", fragment(0, 0, 2_000)).unwrap();
        let tail = scheduler.flush("mic").unwrap().unwrap();
        assert!(tail.is_final);
        assert_eq!((tail.start_ms, tail.end_ms), (0, 2_000));
        assert!(scheduler.flush("mic").unwrap().is_none());
    }

    #[test]
    fn tracker_matches_by_time_and_emits_commit_candidate_only_after_stability() {
        let mut tracker = RevisionTracker::new();
        let first = tracker.observe(
            &window("w1", 0, 4_000),
            vec![hypothesis(300, 3_800, "สวัสดี")],
        );
        let id = match &first[0] {
            TranscriptUpdate::Provisional {
                utterance_id,
                revision,
                ..
            } => {
                assert_eq!(*revision, 1);
                utterance_id.clone()
            }
            other => panic!("unexpected first update: {other:?}"),
        };
        assert!(!first
            .iter()
            .any(|update| matches!(update, TranscriptUpdate::CommitCandidate { .. })));
        let stable = tracker.observe(
            &window("w2", 0, 4_000),
            vec![hypothesis(300, 3_800, "สวัสดี")],
        );
        assert!(stable.iter().any(|update| matches!(update, TranscriptUpdate::CommitCandidate { utterance_id, revision: 1, expected_persisted_revision: 0, .. } if utterance_id == &id)));
        tracker.mark_committed(&id, 1, "สวัสดี").unwrap();
        let repeated = tracker.observe(
            &window("w3", 0, 4_000),
            vec![hypothesis(300, 3_800, "สวัสดี")],
        );
        assert!(!repeated
            .iter()
            .any(|update| matches!(update, TranscriptUpdate::Provisional { .. })));
        assert!(tracker.mark_committed(&id, 3, "ผิดลำดับ").is_err());
    }

    #[test]
    fn same_text_at_a_distinct_time_is_a_distinct_utterance() {
        let mut tracker = RevisionTracker::new();
        let first = tracker.observe(&window("w1", 0, 4_000), vec![hypothesis(200, 700, "ใช่")]);
        let first_id = match &first[0] {
            TranscriptUpdate::Provisional { utterance_id, .. } => utterance_id,
            _ => unreachable!(),
        };
        let repeated = tracker.observe(
            &window("w2", 3_000, 7_000),
            vec![hypothesis(4_200, 4_700, "ใช่")],
        );
        let second_id = match &repeated[0] {
            TranscriptUpdate::Provisional { utterance_id, .. } => utterance_id,
            _ => unreachable!(),
        };
        assert_ne!(first_id, second_id);
    }

    #[test]
    fn tracker_discards_omitted_uncommitted_provisionals_explicitly() {
        let mut tracker = RevisionTracker::new();
        let first = tracker.observe(
            &window("w1", 0, 4_000),
            vec![hypothesis(200, 900, "คำชั่วคราว")],
        );
        let id = match &first[0] {
            TranscriptUpdate::Provisional { utterance_id, .. } => utterance_id.clone(),
            _ => unreachable!(),
        };
        tracker.observe(&window("w2", 3_000, 7_000), Vec::new());
        let updates = tracker.observe(&window("w3", 6_000, 10_000), Vec::new());
        assert!(updates.iter().any(|update| matches!(update, TranscriptUpdate::Discard { utterance_id, .. } if utterance_id == &id)));
    }

    #[test]
    fn tracker_prunes_committed_hypotheses_outside_the_matching_window() {
        let mut tracker = RevisionTracker::new();
        let first = tracker.observe(
            &window("w1", 0, 4_000),
            vec![hypothesis(500, 1_000, "สวัสดี")],
        );
        let id = match &first[0] {
            TranscriptUpdate::Provisional { utterance_id, .. } => utterance_id.clone(),
            _ => unreachable!(),
        };
        tracker.mark_committed(&id, 1, "สวัสดี").unwrap();

        tracker.observe(&window("w2", 7_000, 8_000), Vec::new());

        assert!(!tracker.utterances.contains_key(&id));
    }
}
