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
        self.drafts.clear();
        self.deliveries.clear();
        self.run_times.clear();
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
