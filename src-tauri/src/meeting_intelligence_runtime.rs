//! Process-local private state for the local meeting assistant.
//!
//! Grants and run metadata live in Genesis. Plaintext drafts and local-only
//! delivery previews remain in memory and are discarded when the owner locks
//! the vault or the application exits.

use std::{collections::HashMap, time::Instant};

use crate::meeting_intelligence_schema::{MeetingAgentDeliveryPreview, MeetingAgentPrivateDraft};

#[derive(Default)]
pub(crate) struct MeetingIntelligenceRuntime {
    pub(crate) sessions: HashMap<(String, String), MeetingAgentSession>,
}

#[derive(Default)]
pub(crate) struct MeetingAgentSession {
    pub(crate) enabled_this_process: bool,
    pub(crate) mode: String,
    pub(crate) state: String,
    pub(crate) revision: u64,
    pub(crate) expires_at: Option<String>,
    pub(crate) allowed_topics: Vec<String>,
    pub(crate) active_run_id: Option<String>,
    pub(crate) active_run_transcript_cursor: Option<i64>,
    pub(crate) last_observed_transcript_cursor: i64,
    pub(crate) drafts: HashMap<String, MeetingAgentPrivateDraft>,
    pub(crate) deliveries: HashMap<String, RuntimeDelivery>,
    pub(crate) run_times: Vec<Instant>,
}

#[derive(Clone)]
pub(crate) struct RuntimeDelivery {
    pub(crate) preview: MeetingAgentDeliveryPreview,
    pub(crate) draft_id: String,
    pub(crate) draft_revision: u64,
    pub(crate) expires_at: Instant,
}

impl MeetingAgentSession {
    pub(crate) fn new_stopped(revision: u64) -> Self {
        Self {
            enabled_this_process: false,
            mode: "off".to_string(),
            state: "stopped".to_string(),
            revision,
            expires_at: None,
            allowed_topics: Vec::new(),
            active_run_id: None,
            active_run_transcript_cursor: None,
            last_observed_transcript_cursor: -1,
            drafts: HashMap::new(),
            deliveries: HashMap::new(),
            run_times: Vec::new(),
        }
    }

    pub(crate) fn clear_sensitive(&mut self) {
        self.enabled_this_process = false;
        self.mode = "off".to_string();
        self.state = "paused".to_string();
        self.expires_at = None;
        self.allowed_topics.clear();
        self.active_run_id = None;
        self.active_run_transcript_cursor = None;
        self.last_observed_transcript_cursor = -1;
        self.drafts.clear();
        self.deliveries.clear();
        self.run_times.clear();
    }

    pub(crate) fn observe_committed_cursor(&mut self, cursor: i64) -> bool {
        if cursor <= self.last_observed_transcript_cursor {
            return false;
        }
        self.last_observed_transcript_cursor = cursor;
        if self
            .active_run_transcript_cursor
            .is_some_and(|run_cursor| cursor > run_cursor)
        {
            self.active_run_id = None;
            self.active_run_transcript_cursor = None;
        }
        let stale_draft_ids = self
            .drafts
            .iter_mut()
            .filter_map(|(draft_id, draft)| {
                if cursor > draft.based_on_transcript_cursor && draft.state == "private" {
                    draft.state = "stale".to_string();
                    Some(draft_id.clone())
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>();
        if !stale_draft_ids.is_empty() && self.state == "drafting" {
            self.state = "observing".to_string();
        }
        self.deliveries
            .retain(|_, delivery| !stale_draft_ids.contains(&delivery.draft_id));
        true
    }
}

impl MeetingIntelligenceRuntime {
    pub(crate) fn session(
        &mut self,
        project_id: &str,
        recording_id: &str,
        persisted_revision: u64,
    ) -> &mut MeetingAgentSession {
        self.sessions
            .entry((project_id.to_string(), recording_id.to_string()))
            .or_insert_with(|| MeetingAgentSession::new_stopped(persisted_revision))
    }

    pub(crate) fn clear_sensitive(&mut self) {
        for session in self.sessions.values_mut() {
            session.clear_sensitive();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_cursor_invalidates_older_drafts_and_active_runs_once() {
        let mut session = MeetingAgentSession::new_stopped(1);
        session.enabled_this_process = true;
        session.mode = "draft".to_string();
        session.state = "drafting".to_string();
        session.active_run_id = Some("run-1".to_string());
        session.active_run_transcript_cursor = Some(4);
        session.last_observed_transcript_cursor = 4;
        session.drafts.insert(
            "draft-1".to_string(),
            MeetingAgentPrivateDraft {
                draft_id: "draft-1".to_string(),
                revision: 1,
                text: "private answer".to_string(),
                citations: Vec::new(),
                based_on_transcript_cursor: 4,
                expires_at: "2026-09-25T00:00:00Z".to_string(),
                state: "private".to_string(),
            },
        );
        session.deliveries.insert(
            "delivery-1".to_string(),
            RuntimeDelivery {
                preview: MeetingAgentDeliveryPreview {
                    intent_id: "delivery-1".to_string(),
                    payload_hash: "hash".to_string(),
                    destination_summary: "local".to_string(),
                    state: "awaiting_approval".to_string(),
                    approval_scope: "local_preview_only".to_string(),
                    external_dispatch_available: false,
                },
                draft_id: "draft-1".to_string(),
                draft_revision: 1,
                expires_at: Instant::now() + std::time::Duration::from_secs(60),
            },
        );

        assert!(session.observe_committed_cursor(5));
        assert_eq!(session.last_observed_transcript_cursor, 5);
        assert_eq!(session.active_run_id, None);
        assert_eq!(session.drafts["draft-1"].state, "stale");
        assert_eq!(session.state, "observing");
        assert!(session.deliveries.is_empty());
        assert!(!session.observe_committed_cursor(5));
    }
}
