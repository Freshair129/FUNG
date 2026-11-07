//! Bounded native PCM playback for recording review.
//!
//! The renderer supplies only an opaque recording key and an explicit
//! channel.  Ledger rows resolve to a project custody root, and every source
//! is opened and validated before the same handle is handed to the playback
//! worker.  The realtime callback only drains a bounded queue.

use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{State, WebviewWindow};

use crate::genesis_adapter;
use crate::recording_review::{
    self, AdmissionError, NativeCaptureGuard, RecordingStorageRow, ReviewError,
};
use crate::AppState;

const MAX_CHUNK_DESCRIPTORS: usize = 20_000;
const MAX_PLAYER_METADATA_BYTES: usize = 8 * 1024 * 1024;
const MAX_QUEUE_BYTES: usize = 1024 * 1024;
const MAX_READ_BYTES: usize = 64 * 1024;
const MAX_OPEN_TIMEOUT: Duration = Duration::from_secs(10);
const PLAYBACK_ADOPTION_ACK_TIMEOUT: Duration = Duration::from_secs(10);
const PLAYBACK_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
const PLAYBACK_HIDDEN_GRACE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_LEASE_CLOSE_DISPATCH_FAILURES: u8 = 3;
const OUTPUT_CHANNELS: usize = 2;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 96_000;
const COMMON_OUTPUT_SAMPLE_RATES: [u32; 10] = [
    8_000, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000,
];

const STATE_PAUSED: u8 = 0;
const STATE_PLAYING: u8 = 1;
const STATE_ENDED: u8 = 2;
const STATE_ERROR: u8 = 3;

const PLAYBACK_LEASE_PHASE_SHIFT: u32 = 62;
const PLAYBACK_LEASE_TICK_MASK: u64 = (1_u64 << PLAYBACK_LEASE_PHASE_SHIFT) - 1;
const PLAYBACK_LEASE_AWAITING_ACK: u64 = 0;
const PLAYBACK_LEASE_ADOPTED: u64 = 1;
const PLAYBACK_LEASE_REVOKED: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackOutputLeaseState {
    Permitted,
    AwaitingAdoption,
    ReconciliationPending,
    Expired,
}

type AudioReader = hound::WavReader<BufReader<File>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MissingRange {
    pub(crate) start_ms: u64,
    pub(crate) end_ms: u64,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackState {
    pub(crate) handle: String,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<PlaybackSourceIdentity>,
    pub(crate) state: String,
    pub(crate) position_ms: u64,
    pub(crate) duration_ms: u64,
    pub(crate) stream_epoch: u64,
    pub(crate) degraded: bool,
    pub(crate) missing_ranges: Vec<MissingRange>,
    pub(crate) output_latency_ms: Option<u64>,
    pub(crate) error: Option<ReviewError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) adoption: Option<PlaybackAdoptionOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackAdoptionOffer {
    pub(crate) generation: u64,
    pub(crate) token: String,
}

#[derive(Debug, Clone, Default)]
struct PlaybackVisibility {
    transition_lock: Arc<Mutex<()>>,
    transition_at_ms: Arc<AtomicU64>,
    window_hidden: Arc<AtomicBool>,
    hidden_since_ms: Arc<AtomicU64>,
    hidden_grace_active: Arc<AtomicBool>,
    visibility_changed_at_ms: Arc<AtomicU64>,
    last_heartbeat_ms: Arc<AtomicU64>,
}

impl PlaybackVisibility {
    fn lock_transition(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock_transition_with_wait_hook(|| {})
    }

    fn lock_transition_with_wait_hook(
        &self,
        on_wait: impl FnOnce(),
    ) -> std::sync::MutexGuard<'_, ()> {
        match self.transition_lock.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                on_wait();
                self.transition_lock
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            }
        }
    }

    fn try_lock_transition(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        match self.transition_lock.try_lock() {
            Ok(guard) => Some(guard),
            Err(std::sync::TryLockError::Poisoned(error)) => Some(error.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }

    #[cfg(test)]
    fn set_hidden_at(&self, hidden: bool, now_ms: u64) -> bool {
        self.set_hidden_at_with_hook(hidden, now_ms, || {})
    }

    fn set_hidden_current(&self, hidden: bool) -> bool {
        self.set_hidden_with_clock_and_wait_hook(hidden, playback_lease_tick_ms, || {}, || {})
    }

    #[cfg(test)]
    fn set_hidden_at_with_hook(
        &self,
        hidden: bool,
        now_ms: u64,
        after_fields: impl FnOnce(),
    ) -> bool {
        self.set_hidden_with_clock_and_wait_hook(hidden, || now_ms, || {}, after_fields)
    }

    fn set_hidden_with_clock_and_wait_hook(
        &self,
        hidden: bool,
        mut clock: impl FnMut() -> u64,
        on_wait: impl FnOnce(),
        after_fields: impl FnOnce(),
    ) -> bool {
        let _guard = self.lock_transition_with_wait_hook(on_wait);
        let now_ms = clock();
        if now_ms < self.transition_at_ms.load(Ordering::Acquire) {
            return false;
        }
        let currently_hidden = self.window_hidden.load(Ordering::Acquire);
        if currently_hidden == hidden {
            return false;
        }
        if hidden {
            let visible_since = self.visibility_changed_at_ms.load(Ordering::Acquire);
            let last_heartbeat = self.last_heartbeat_ms.load(Ordering::Acquire);
            let renderer_live_since_show = visible_since == 0 || last_heartbeat > visible_since;
            if renderer_live_since_show || !self.hidden_grace_active.load(Ordering::Acquire) {
                self.hidden_since_ms.store(now_ms, Ordering::Release);
                self.hidden_grace_active.store(true, Ordering::Release);
            }
            self.window_hidden.store(true, Ordering::Release);
        } else {
            self.visibility_changed_at_ms
                .store(now_ms, Ordering::Release);
            self.window_hidden.store(false, Ordering::Release);
        }
        self.transition_at_ms.store(now_ms, Ordering::Release);
        after_fields();
        true
    }

    fn reset_for_new_open(&self) {
        let _guard = self.lock_transition();
        if !self.window_hidden.load(Ordering::Acquire) {
            self.hidden_since_ms.store(0, Ordering::Release);
            self.hidden_grace_active.store(false, Ordering::Release);
            self.last_heartbeat_ms.store(0, Ordering::Release);
        }
    }

    fn record_heartbeat_unlocked(&self, now_ms: u64) {
        self.last_heartbeat_ms.fetch_max(now_ms, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct PlaybackAdoptionLease {
    generation: u64,
    handle: String,
    token: String,
    state: AtomicU64,
    close_requested: Arc<AtomicBool>,
    visibility: PlaybackVisibility,
}

fn playback_lease_tick_ms() -> u64 {
    static PROCESS_START: OnceLock<Instant> = OnceLock::new();
    PROCESS_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(PLAYBACK_LEASE_TICK_MASK as u128) as u64
}

fn playback_lease_word(phase: u64, deadline_ms: u64) -> u64 {
    (phase << PLAYBACK_LEASE_PHASE_SHIFT) | deadline_ms.min(PLAYBACK_LEASE_TICK_MASK)
}

fn playback_lease_phase(state: u64) -> u64 {
    state >> PLAYBACK_LEASE_PHASE_SHIFT
}

fn playback_lease_deadline(state: u64) -> u64 {
    state & PLAYBACK_LEASE_TICK_MASK
}

impl PlaybackAdoptionLease {
    fn new(
        generation: u64,
        handle: String,
        close_requested: Arc<AtomicBool>,
        visibility: PlaybackVisibility,
    ) -> Self {
        Self::new_at(
            generation,
            handle,
            close_requested,
            visibility,
            playback_lease_tick_ms(),
        )
    }

    fn new_at(
        generation: u64,
        handle: String,
        close_requested: Arc<AtomicBool>,
        visibility: PlaybackVisibility,
        now_ms: u64,
    ) -> Self {
        let deadline = now_ms.saturating_add(PLAYBACK_ADOPTION_ACK_TIMEOUT.as_millis() as u64);
        Self {
            generation,
            handle,
            token: recording_review::random_opaque_id(),
            state: AtomicU64::new(playback_lease_word(PLAYBACK_LEASE_AWAITING_ACK, deadline)),
            close_requested,
            visibility,
        }
    }

    fn offer(&self) -> PlaybackAdoptionOffer {
        PlaybackAdoptionOffer {
            generation: self.generation,
            token: self.token.clone(),
        }
    }

    fn matches(&self, handle: &str, generation: u64, token: &str) -> bool {
        self.handle == handle && self.generation == generation && self.token == token
    }

    #[cfg(test)]
    fn acknowledge(&self, handle: &str, generation: u64, token: &str, now_ms: u64) -> bool {
        self.acknowledge_with_clock_and_wait_hook(handle, generation, token, || now_ms, || {})
    }

    fn acknowledge_current(&self, handle: &str, generation: u64, token: &str) -> bool {
        self.acknowledge_with_clock_and_wait_hook(
            handle,
            generation,
            token,
            playback_lease_tick_ms,
            || {},
        )
    }

    fn acknowledge_with_clock_and_wait_hook(
        &self,
        handle: &str,
        generation: u64,
        token: &str,
        mut clock: impl FnMut() -> u64,
        on_wait: impl FnOnce(),
    ) -> bool {
        if !self.matches(handle, generation, token) {
            return false;
        }
        self.revoke_if_expired_at(clock());
        let _visibility_guard = self.visibility.lock_transition_with_wait_hook(on_wait);
        let now_ms = clock();
        if now_ms < self.visibility.transition_at_ms.load(Ordering::Acquire) {
            return false;
        }
        loop {
            let current = self.state.load(Ordering::Acquire);
            match playback_lease_phase(current) {
                PLAYBACK_LEASE_ADOPTED => {
                    if now_ms < playback_lease_deadline(current) {
                        return true;
                    }
                    self.revoke_state(current);
                    return false;
                }
                PLAYBACK_LEASE_AWAITING_ACK => {
                    if now_ms >= playback_lease_deadline(current) {
                        self.revoke_state(current);
                        return false;
                    }
                    let deadline = if self.visibility.window_hidden.load(Ordering::Acquire) {
                        if !self.visibility.hidden_grace_active.load(Ordering::Acquire) {
                            self.revoke_state(current);
                            return false;
                        }
                        self.visibility
                            .hidden_since_ms
                            .load(Ordering::Acquire)
                            .saturating_add(PLAYBACK_HIDDEN_GRACE_TIMEOUT.as_millis() as u64)
                    } else {
                        now_ms.saturating_add(PLAYBACK_HEARTBEAT_TIMEOUT.as_millis() as u64)
                    };
                    if now_ms >= deadline {
                        self.revoke_state(current);
                        return false;
                    }
                    if self
                        .state
                        .compare_exchange(
                            current,
                            playback_lease_word(PLAYBACK_LEASE_ADOPTED, deadline),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        self.visibility.record_heartbeat_unlocked(now_ms);
                        return true;
                    }
                }
                _ => return false,
            }
        }
    }

    #[cfg(test)]
    fn heartbeat(&self, handle: &str, generation: u64, token: &str, now_ms: u64) -> bool {
        self.heartbeat_with_clock_and_wait_hook(handle, generation, token, || now_ms, || {})
    }

    fn heartbeat_current(&self, handle: &str, generation: u64, token: &str) -> bool {
        self.heartbeat_with_clock_and_wait_hook(
            handle,
            generation,
            token,
            playback_lease_tick_ms,
            || {},
        )
    }

    fn heartbeat_with_clock_and_wait_hook(
        &self,
        handle: &str,
        generation: u64,
        token: &str,
        mut clock: impl FnMut() -> u64,
        on_wait: impl FnOnce(),
    ) -> bool {
        if !self.matches(handle, generation, token) {
            return false;
        }
        self.revoke_if_expired_at(clock());
        let _visibility_guard = self.visibility.lock_transition_with_wait_hook(on_wait);
        let now_ms = clock();
        if now_ms < self.visibility.transition_at_ms.load(Ordering::Acquire)
            || now_ms < self.visibility.last_heartbeat_ms.load(Ordering::Acquire)
        {
            return false;
        }
        loop {
            let current = self.state.load(Ordering::Acquire);
            if playback_lease_phase(current) != PLAYBACK_LEASE_ADOPTED {
                return false;
            }
            let current_deadline = playback_lease_deadline(current);
            if now_ms >= current_deadline {
                self.revoke_state(current);
                return false;
            }
            let hidden = self.visibility.window_hidden.load(Ordering::Acquire);
            let hidden_since = self.visibility.hidden_since_ms.load(Ordering::Acquire);
            let next_deadline = if hidden {
                if !self.visibility.hidden_grace_active.load(Ordering::Acquire) {
                    self.revoke_state(current);
                    return false;
                }
                let deadline =
                    hidden_since.saturating_add(PLAYBACK_HIDDEN_GRACE_TIMEOUT.as_millis() as u64);
                if now_ms >= deadline {
                    self.revoke_state(current);
                    return false;
                }
                deadline
            } else {
                now_ms.saturating_add(PLAYBACK_HEARTBEAT_TIMEOUT.as_millis() as u64)
            };
            if self
                .state
                .compare_exchange(
                    current,
                    playback_lease_word(PLAYBACK_LEASE_ADOPTED, next_deadline),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.visibility.record_heartbeat_unlocked(now_ms);
                return true;
            }
        }
    }

    fn revoke_state(&self, expected: u64) -> bool {
        if self
            .state
            .compare_exchange(
                expected,
                playback_lease_word(PLAYBACK_LEASE_REVOKED, 0),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.close_requested.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }

    fn revoke_if_expired(&self) -> bool {
        self.revoke_if_expired_at(playback_lease_tick_ms())
    }

    fn revoke_if_expired_at(&self, now_ms: u64) -> bool {
        if playback_lease_phase(self.state.load(Ordering::Acquire)) == PLAYBACK_LEASE_REVOKED {
            return false;
        }
        let Some(_visibility_guard) = self.visibility.try_lock_transition() else {
            return false;
        };
        self.reconcile_if_expired_at_locked(now_ms)
    }

    fn reconcile_if_expired_at_locked(&self, now_ms: u64) -> bool {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let phase = playback_lease_phase(current);
            if phase == PLAYBACK_LEASE_REVOKED {
                return false;
            }
            let current_deadline = playback_lease_deadline(current);
            if phase == PLAYBACK_LEASE_ADOPTED
                && !self.visibility.window_hidden.load(Ordering::Acquire)
            {
                let visible_since = self
                    .visibility
                    .visibility_changed_at_ms
                    .load(Ordering::Acquire);
                let last_heartbeat = self.visibility.last_heartbeat_ms.load(Ordering::Acquire);
                let hidden_since = self.visibility.hidden_since_ms.load(Ordering::Acquire);
                let hidden_deadline =
                    hidden_since.saturating_add(PLAYBACK_HIDDEN_GRACE_TIMEOUT.as_millis() as u64);
                let visible_deadline =
                    visible_since.saturating_add(PLAYBACK_HEARTBEAT_TIMEOUT.as_millis() as u64);
                if self.visibility.hidden_grace_active.load(Ordering::Acquire)
                    && visible_since >= last_heartbeat
                    && hidden_since < current_deadline
                    && visible_since < hidden_deadline
                    && now_ms < hidden_deadline
                    && current_deadline != visible_deadline
                {
                    if self
                        .state
                        .compare_exchange(
                            current,
                            playback_lease_word(PLAYBACK_LEASE_ADOPTED, visible_deadline),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        continue;
                    }
                    continue;
                }
            }
            if now_ms < current_deadline {
                return false;
            }
            if phase == PLAYBACK_LEASE_ADOPTED
                && self.visibility.window_hidden.load(Ordering::Acquire)
                && self.visibility.hidden_grace_active.load(Ordering::Acquire)
            {
                let hidden_since = self.visibility.hidden_since_ms.load(Ordering::Acquire);
                let hidden_deadline =
                    hidden_since.saturating_add(PLAYBACK_HIDDEN_GRACE_TIMEOUT.as_millis() as u64);
                if hidden_since < current_deadline
                    && hidden_deadline > current_deadline
                    && now_ms < hidden_deadline
                {
                    if self
                        .state
                        .compare_exchange(
                            current,
                            playback_lease_word(PLAYBACK_LEASE_ADOPTED, hidden_deadline),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return false;
                    }
                    continue;
                }
            }
            if self
                .state
                .compare_exchange(
                    current,
                    playback_lease_word(PLAYBACK_LEASE_REVOKED, 0),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.close_requested.store(true, Ordering::Release);
                return true;
            }
        }
    }

    fn is_live(&self, now_ms: u64) -> bool {
        self.revoke_if_expired_at(now_ms);
        let current = self.state.load(Ordering::Acquire);
        playback_lease_phase(current) == PLAYBACK_LEASE_ADOPTED
            && now_ms < playback_lease_deadline(current)
    }

    fn output_state_at(&self, now_ms: u64) -> PlaybackOutputLeaseState {
        let Some(_visibility_guard) = self.visibility.try_lock_transition() else {
            let current = self.state.load(Ordering::Acquire);
            return match playback_lease_phase(current) {
                PLAYBACK_LEASE_AWAITING_ACK if now_ms < playback_lease_deadline(current) => {
                    PlaybackOutputLeaseState::AwaitingAdoption
                }
                PLAYBACK_LEASE_AWAITING_ACK => {
                    if self.revoke_state(current)
                        || playback_lease_phase(self.state.load(Ordering::Acquire))
                            == PLAYBACK_LEASE_REVOKED
                    {
                        PlaybackOutputLeaseState::Expired
                    } else {
                        PlaybackOutputLeaseState::ReconciliationPending
                    }
                }
                PLAYBACK_LEASE_ADOPTED => PlaybackOutputLeaseState::ReconciliationPending,
                _ => PlaybackOutputLeaseState::Expired,
            };
        };
        self.reconcile_if_expired_at_locked(now_ms);
        let current = self.state.load(Ordering::Acquire);
        match playback_lease_phase(current) {
            PLAYBACK_LEASE_ADOPTED if now_ms < playback_lease_deadline(current) => {
                PlaybackOutputLeaseState::Permitted
            }
            PLAYBACK_LEASE_AWAITING_ACK if now_ms < playback_lease_deadline(current) => {
                PlaybackOutputLeaseState::AwaitingAdoption
            }
            _ => PlaybackOutputLeaseState::Expired,
        }
    }

    fn is_revoked(&self) -> bool {
        playback_lease_phase(self.state.load(Ordering::Acquire)) == PLAYBACK_LEASE_REVOKED
    }

    #[cfg(test)]
    fn visibility_changed(&self, hidden: bool, now_ms: u64) -> bool {
        self.visibility_changed_with_clock_and_wait_hook(hidden, || now_ms, || {})
    }

    fn visibility_changed_current(&self, hidden: bool) -> bool {
        self.visibility_changed_with_clock_and_wait_hook(hidden, playback_lease_tick_ms, || {})
    }

    fn visibility_changed_with_clock_and_wait_hook(
        &self,
        hidden: bool,
        mut clock: impl FnMut() -> u64,
        on_wait: impl FnOnce(),
    ) -> bool {
        let _visibility_guard = self.visibility.lock_transition_with_wait_hook(on_wait);
        let now_ms = clock();
        let visibility_changed_at = self
            .visibility
            .visibility_changed_at_ms
            .load(Ordering::Acquire);
        if now_ms < self.visibility.transition_at_ms.load(Ordering::Acquire)
            || self.visibility.window_hidden.load(Ordering::Acquire) != hidden
        {
            return false;
        }

        let current = self.state.load(Ordering::Acquire);
        if playback_lease_phase(current) != PLAYBACK_LEASE_ADOPTED {
            return false;
        }

        let current_deadline = playback_lease_deadline(current);
        let hidden_since = self.visibility.hidden_since_ms.load(Ordering::Acquire);
        let hidden_deadline =
            hidden_since.saturating_add(PLAYBACK_HIDDEN_GRACE_TIMEOUT.as_millis() as u64);
        let last_heartbeat = self.visibility.last_heartbeat_ms.load(Ordering::Acquire);
        let deadline = if hidden {
            if !self.visibility.hidden_grace_active.load(Ordering::Acquire)
                || (now_ms >= current_deadline
                    && !(hidden_since < current_deadline
                        && hidden_deadline > current_deadline
                        && now_ms < hidden_deadline))
            {
                self.revoke_state(current);
                return false;
            }
            hidden_deadline
        } else {
            let visible_deadline =
                visibility_changed_at.saturating_add(PLAYBACK_HEARTBEAT_TIMEOUT.as_millis() as u64);
            if now_ms >= current_deadline
                && !(self.visibility.hidden_grace_active.load(Ordering::Acquire)
                    && visibility_changed_at >= last_heartbeat
                    && hidden_since < current_deadline
                    && visibility_changed_at < hidden_deadline
                    && now_ms < hidden_deadline
                    && current_deadline != visible_deadline)
            {
                self.revoke_state(current);
                return false;
            }
            visible_deadline
        };

        if now_ms >= deadline {
            self.revoke_state(current);
            return false;
        }

        self.state
            .compare_exchange(
                current,
                playback_lease_word(PLAYBACK_LEASE_ADOPTED, deadline),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

#[cfg(test)]
mod playback_adoption_lease_tests {
    use super::*;

    fn new_lease(generation: u64, handle: &str, now_ms: u64) -> PlaybackAdoptionLease {
        new_lease_with_visibility(generation, handle, now_ms, PlaybackVisibility::default())
    }

    fn new_lease_with_visibility(
        generation: u64,
        handle: &str,
        now_ms: u64,
        visibility: PlaybackVisibility,
    ) -> PlaybackAdoptionLease {
        PlaybackAdoptionLease::new_at(
            generation,
            handle.to_string(),
            Arc::new(AtomicBool::new(false)),
            visibility,
            now_ms,
        )
    }

    #[test]
    fn adoption_requires_exact_handle_generation_and_nonce_and_is_idempotent() {
        let lease = new_lease(7, "handle-a", 100);
        let offer = lease.offer();

        assert!(!lease.acknowledge("handle-b", 7, &offer.token, 101));
        assert!(!lease.acknowledge("handle-a", 8, &offer.token, 101));
        assert!(!lease.acknowledge("handle-a", 7, "wrong-token", 101));
        assert_eq!(
            playback_lease_phase(lease.state.load(Ordering::Acquire)),
            PLAYBACK_LEASE_AWAITING_ACK
        );

        assert!(lease.acknowledge("handle-a", 7, &offer.token, 101));
        assert!(lease.acknowledge("handle-a", 7, &offer.token, 102));
        assert!(lease.is_live(103));
    }

    #[test]
    fn acknowledgement_at_deadline_revokes_and_cannot_be_resurrected() {
        let lease = new_lease(3, "late", 50);
        let offer = lease.offer();
        let deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));

        assert!(!lease.acknowledge("late", offer.generation, &offer.token, deadline));
        assert!(lease.is_revoked());
        assert!(lease.close_requested.load(Ordering::Acquire));
        assert!(!lease.heartbeat("late", offer.generation, &offer.token, deadline + 1));
    }

    #[test]
    fn acknowledgement_waiting_for_visibility_lock_rechecks_deadline() {
        let lease = Arc::new(new_lease(32, "delayed-ack", 1_000));
        let offer = lease.offer();
        let clock = Arc::new(AtomicU64::new(10_999));
        let held_transition = lease.visibility.lock_transition();
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let worker_lease = Arc::clone(&lease);
        let worker_clock = Arc::clone(&clock);
        let handle = "delayed-ack".to_string();
        let token = offer.token.clone();
        let worker = thread::spawn(move || {
            worker_lease.acknowledge_with_clock_and_wait_hook(
                &handle,
                32,
                &token,
                || worker_clock.load(Ordering::Acquire),
                || waiting_tx.send(()).unwrap(),
            )
        });

        waiting_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        clock.store(11_000, Ordering::Release);
        drop(held_transition);

        assert!(!worker.join().unwrap());
        assert!(lease.is_revoked());
        assert!(lease.close_requested.load(Ordering::Acquire));
    }

    #[test]
    fn heartbeat_extends_only_a_live_adopted_generation_and_expiry_is_terminal() {
        let lease = new_lease(12, "heartbeat", 1_000);
        let offer = lease.offer();
        assert!(lease.acknowledge("heartbeat", 12, &offer.token, 1_001));

        assert!(!lease.heartbeat("heartbeat", 11, &offer.token, 2_000));
        assert!(!lease.heartbeat("heartbeat", 12, "stale-token", 2_000));
        assert!(lease.heartbeat("heartbeat", 12, &offer.token, 30_000));
        let renewed_deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));
        assert_eq!(renewed_deadline, 60_000);
        assert!(!lease.revoke_if_expired_at(59_999));
        assert!(lease.revoke_if_expired_at(60_000));
        assert!(!lease.heartbeat("heartbeat", 12, &offer.token, 60_001));
        assert!(lease.is_revoked());
        assert!(lease.close_requested.load(Ordering::Acquire));
    }

    #[test]
    fn heartbeat_waiting_for_visibility_lock_cannot_renew_past_deadline() {
        let lease = Arc::new(new_lease(33, "delayed-heartbeat", 1_000));
        let offer = lease.offer();
        assert!(lease.acknowledge("delayed-heartbeat", 33, &offer.token, 1_001));
        let clock = Arc::new(AtomicU64::new(31_000));
        let held_transition = lease.visibility.lock_transition();
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let worker_lease = Arc::clone(&lease);
        let worker_clock = Arc::clone(&clock);
        let handle = "delayed-heartbeat".to_string();
        let token = offer.token.clone();
        let worker = thread::spawn(move || {
            worker_lease.heartbeat_with_clock_and_wait_hook(
                &handle,
                33,
                &token,
                || worker_clock.load(Ordering::Acquire),
                || waiting_tx.send(()).unwrap(),
            )
        });

        waiting_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        clock.store(31_001, Ordering::Release);
        drop(held_transition);

        assert!(!worker.join().unwrap());
        assert!(lease.is_revoked());
        assert!(lease.close_requested.load(Ordering::Acquire));
    }

    #[test]
    fn stale_heartbeat_cannot_shorten_lease_or_cross_visibility_transition() {
        let visibility = PlaybackVisibility::default();
        let lease = PlaybackAdoptionLease::new_at(
            30,
            "monotonic-heartbeat".to_string(),
            Arc::new(AtomicBool::new(false)),
            visibility.clone(),
            1_000,
        );
        let offer = lease.offer();
        assert!(lease.acknowledge("monotonic-heartbeat", 30, &offer.token, 1_001));
        assert!(lease.heartbeat("monotonic-heartbeat", 30, &offer.token, 10_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            40_000
        );

        assert!(!lease.heartbeat("monotonic-heartbeat", 30, &offer.token, 9_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            40_000,
            "an out-of-order heartbeat cannot move the deadline backward"
        );

        assert!(visibility.set_hidden_at(true, 20_000));
        assert!(lease.visibility_changed(true, 20_000));
        let hidden_deadline = 320_000;
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            hidden_deadline
        );
        assert!(!lease.heartbeat("monotonic-heartbeat", 30, &offer.token, 15_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            hidden_deadline,
            "a heartbeat that predates the visibility change cannot renew it"
        );
    }

    #[test]
    fn hidden_and_restore_grace_are_bounded_and_cannot_revive_an_expired_lease() {
        let lease = new_lease(22, "hidden", 1_000);
        let offer = lease.offer();
        assert!(lease.acknowledge("hidden", 22, &offer.token, 1_001));
        lease
            .visibility
            .hidden_since_ms
            .store(2_000, Ordering::Release);
        lease
            .visibility
            .hidden_grace_active
            .store(true, Ordering::Release);
        lease
            .visibility
            .visibility_changed_at_ms
            .store(2_000, Ordering::Release);
        lease
            .visibility
            .window_hidden
            .store(true, Ordering::Release);
        assert!(lease.visibility_changed(true, 2_000));
        let hidden_deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));
        assert_eq!(hidden_deadline, 302_000);
        assert!(!lease.revoke_if_expired_at(hidden_deadline - 1));
        lease
            .visibility
            .window_hidden
            .store(false, Ordering::Release);
        lease
            .visibility
            .visibility_changed_at_ms
            .store(200_000, Ordering::Release);
        assert!(lease.visibility_changed(false, 200_000));
        let restore_deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));
        assert_eq!(restore_deadline, 230_000);
        assert!(!lease.revoke_if_expired_at(restore_deadline - 1));
        assert!(lease.revoke_if_expired_at(restore_deadline));

        let expired = new_lease(23, "already-expired", 1_000);
        let expired_offer = expired.offer();
        assert!(expired.acknowledge("already-expired", 23, &expired_offer.token, 1_001));
        let deadline = playback_lease_deadline(expired.state.load(Ordering::Acquire));
        assert!(expired.revoke_if_expired_at(deadline));
        assert!(!expired.visibility_changed(true, deadline + 1));
        assert!(!expired.visibility_changed(false, deadline + 2));
        assert!(!expired.heartbeat("already-expired", 23, &expired_offer.token, deadline + 3));
        assert!(expired.is_revoked());
    }

    #[test]
    fn hidden_heartbeats_do_not_slide_the_five_minute_deadline() {
        let lease = new_lease(24, "fixed-hidden-grace", 1_000);
        let offer = lease.offer();
        assert!(lease.acknowledge("fixed-hidden-grace", 24, &offer.token, 1_001));
        lease
            .visibility
            .hidden_since_ms
            .store(2_000, Ordering::Release);
        lease
            .visibility
            .hidden_grace_active
            .store(true, Ordering::Release);
        lease
            .visibility
            .visibility_changed_at_ms
            .store(2_000, Ordering::Release);
        lease
            .visibility
            .window_hidden
            .store(true, Ordering::Release);
        assert!(lease.visibility_changed(true, 2_000));

        let fixed_deadline = 302_000;
        assert!(lease.heartbeat("fixed-hidden-grace", 24, &offer.token, 10_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            fixed_deadline
        );
        assert!(lease.heartbeat("fixed-hidden-grace", 24, &offer.token, 200_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            fixed_deadline
        );
        assert!(!lease.heartbeat("fixed-hidden-grace", 24, &offer.token, fixed_deadline));
        assert!(lease.is_revoked());
    }

    #[test]
    fn stale_visibility_callback_cannot_overwrite_newer_transition() {
        let visibility = PlaybackVisibility::default();
        let lease = PlaybackAdoptionLease::new_at(
            28,
            "stale-visibility-callback".to_string(),
            Arc::new(AtomicBool::new(false)),
            visibility.clone(),
            1_000,
        );
        let offer = lease.offer();
        assert!(lease.acknowledge("stale-visibility-callback", 28, &offer.token, 1_001));

        assert!(visibility.set_hidden_at(true, 2_000));
        assert!(lease.visibility_changed(true, 2_000));
        assert!(visibility.set_hidden_at(false, 3_000));
        assert!(lease.visibility_changed(false, 3_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            33_000
        );

        assert!(
            !lease.visibility_changed(true, 2_500),
            "an older hide callback cannot replace the later visible state"
        );
        assert!(!visibility.window_hidden.load(Ordering::Acquire));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            33_000
        );
        assert!(lease.is_live(3_001));
    }

    #[test]
    fn duplicate_restore_does_not_extend_heartbeat_recovery_deadline() {
        let visibility = PlaybackVisibility::default();
        let lease = new_lease_with_visibility(31, "duplicate-restore", 1_000, visibility.clone());
        let offer = lease.offer();
        assert!(lease.acknowledge("duplicate-restore", 31, &offer.token, 1_001));

        assert!(visibility.set_hidden_at(true, 2_000));
        assert!(lease.visibility_changed(true, 2_000));
        assert!(visibility.set_hidden_at(false, 3_000));
        assert!(lease.visibility_changed(false, 3_000));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            33_000
        );

        if visibility.set_hidden_at(false, 10_000) {
            lease.visibility_changed(false, 10_000);
        }
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            33_000,
            "a duplicate visible event cannot move the recovery deadline"
        );
        assert!(lease.revoke_if_expired_at(33_000));
        assert!(lease.is_revoked());
    }

    #[test]
    fn expiry_callback_stays_silent_during_visibility_transition_then_reconciles() {
        let visibility = PlaybackVisibility::default();
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            29,
            "visibility-transition-expiry".to_string(),
            Arc::new(AtomicBool::new(false)),
            visibility.clone(),
            1_000,
        ));
        let offer = lease.offer();
        assert!(lease.acknowledge("visibility-transition-expiry", 29, &offer.token, 1_001));

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let transition_visibility = visibility.clone();
        let transition = thread::spawn(move || {
            transition_visibility.set_hidden_at_with_hook(true, 2_000, || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(
            !lease.revoke_if_expired_at(31_002),
            "the callback must not block or revoke while visibility fields are changing"
        );
        assert!(!lease.is_revoked());
        assert!(!lease.close_requested.load(Ordering::Acquire));
        assert!(
            !lease.is_live(31_002),
            "callback output remains silent until expiry reconciliation completes"
        );

        release_tx.send(()).unwrap();
        assert!(transition.join().unwrap());
        assert!(!lease.revoke_if_expired_at(31_002));
        assert!(lease.is_live(31_002));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            302_000
        );
    }

    #[test]
    fn tray_hide_waiting_for_transition_lock_cannot_revive_expired_lease() {
        let visibility = PlaybackVisibility::default();
        let lease = Arc::new(new_lease_with_visibility(
            34,
            "delayed-tray-hide",
            1_000,
            visibility.clone(),
        ));
        let offer = lease.offer();
        assert!(lease.acknowledge("delayed-tray-hide", 34, &offer.token, 1_001));

        let clock = Arc::new(AtomicU64::new(31_000));
        let held_transition = visibility.lock_transition();
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let worker_visibility = visibility.clone();
        let worker_lease = Arc::clone(&lease);
        let worker_clock = Arc::clone(&clock);
        let worker = thread::spawn(move || {
            let changed = worker_visibility.set_hidden_with_clock_and_wait_hook(
                true,
                || worker_clock.load(Ordering::Acquire),
                || waiting_tx.send(()).unwrap(),
                || {},
            );
            let lease_updated = worker_lease.visibility_changed_with_clock_and_wait_hook(
                true,
                || worker_clock.load(Ordering::Acquire),
                || {},
            );
            (changed, lease_updated)
        });

        waiting_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        clock.store(31_002, Ordering::Release);
        drop(held_transition);

        assert_eq!(worker.join().unwrap(), (true, false));
        assert_eq!(visibility.hidden_since_ms.load(Ordering::Acquire), 31_002);
        assert!(lease.is_revoked());
        assert!(lease.close_requested.load(Ordering::Acquire));
    }

    #[test]
    fn tray_visibility_updates_survive_manager_lock_and_expiry_is_not_revived() {
        let manager = PlaybackManager::new();
        let generation = 25;
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            generation,
            "manager-locked-visibility".to_string(),
            Arc::new(AtomicBool::new(false)),
            manager.lifecycle.visibility.clone(),
            playback_lease_tick_ms(),
        ));
        let offer = lease.offer();
        let acknowledged_at = playback_lease_tick_ms().saturating_add(1);
        assert!(lease.acknowledge(
            "manager-locked-visibility",
            generation,
            &offer.token,
            acknowledged_at
        ));

        let held_manager = manager.inner.lock().unwrap();
        manager.set_hidden_to_tray(true);
        assert!(manager
            .lifecycle
            .visibility
            .window_hidden
            .load(Ordering::Acquire));
        let hidden_since = manager
            .lifecycle
            .visibility
            .hidden_since_ms
            .load(Ordering::Acquire);
        manager.set_hidden_to_tray(true);
        assert_eq!(
            manager
                .lifecycle
                .visibility
                .hidden_since_ms
                .load(Ordering::Acquire),
            hidden_since,
            "duplicate hide must not restart the fixed tray grace"
        );
        drop(held_manager);
        assert!(!lease.revoke_if_expired_at(hidden_since.saturating_add(31_000)));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            hidden_since.saturating_add(PLAYBACK_HIDDEN_GRACE_TIMEOUT.as_millis() as u64)
        );

        lease
            .visibility
            .last_heartbeat_ms
            .store(0, Ordering::Release);
        let held_manager = manager.inner.lock().unwrap();
        manager.set_hidden_to_tray(false);
        let visible_since = manager
            .lifecycle
            .visibility
            .visibility_changed_at_ms
            .load(Ordering::Acquire);
        drop(held_manager);
        let restored_deadline =
            visible_since.saturating_add(PLAYBACK_HEARTBEAT_TIMEOUT.as_millis() as u64);
        assert!(!lease.revoke_if_expired_at(restored_deadline - 1));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            restored_deadline
        );
        assert!(lease.revoke_if_expired_at(restored_deadline));
        assert!(!lease.heartbeat(
            "manager-locked-visibility",
            generation,
            &offer.token,
            restored_deadline + 1
        ));
        assert!(lease.is_revoked());
    }

    #[test]
    fn restore_before_watchdog_applies_only_unexpired_hidden_grace() {
        let manager = PlaybackManager::new();
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            26,
            "delayed-visibility-reconcile".to_string(),
            Arc::new(AtomicBool::new(false)),
            manager.lifecycle.visibility.clone(),
            1_000,
        ));
        let offer = lease.offer();
        assert!(lease.acknowledge("delayed-visibility-reconcile", 26, &offer.token, 1_001));
        let original_deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));
        assert_eq!(original_deadline, 31_001);

        let held_manager = manager.inner.lock().unwrap();
        manager.set_hidden_to_tray_at(true, 2_000);
        drop(held_manager);
        assert_eq!(
            manager
                .lifecycle
                .visibility
                .hidden_since_ms
                .load(Ordering::Acquire),
            2_000
        );

        // No watchdog tick runs during this hidden interval. The restore is
        // still inside grace, but occurs after the original 30-second deadline.
        let held_manager = manager.inner.lock().unwrap();
        manager.set_hidden_to_tray_at(false, 30_000);
        manager.set_hidden_to_tray_at(true, 31_000);
        assert_eq!(
            manager
                .lifecycle
                .visibility
                .hidden_since_ms
                .load(Ordering::Acquire),
            2_000
        );
        manager.set_hidden_to_tray_at(false, 32_000);
        drop(held_manager);
        assert_eq!(
            manager
                .lifecycle
                .visibility
                .visibility_changed_at_ms
                .load(Ordering::Acquire),
            32_000
        );
        assert!(!lease.revoke_if_expired_at(32_001));
        let restored_deadline = 62_000;
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            restored_deadline
        );
        assert!(lease.is_live(32_001));
        assert!(lease.revoke_if_expired_at(restored_deadline));

        let expired_manager = PlaybackManager::new();
        let expired = Arc::new(PlaybackAdoptionLease::new_at(
            27,
            "expired-hidden-restore".to_string(),
            Arc::new(AtomicBool::new(false)),
            expired_manager.lifecycle.visibility.clone(),
            1_000,
        ));
        let expired_offer = expired.offer();
        assert!(expired.acknowledge("expired-hidden-restore", 27, &expired_offer.token, 1_001));
        let expired_deadline = playback_lease_deadline(expired.state.load(Ordering::Acquire));
        expired_manager.set_hidden_to_tray_at(true, 2_000);
        expired_manager.set_hidden_to_tray_at(false, 302_001);
        assert!(expired.revoke_if_expired_at(302_001));
        assert!(expired.is_revoked());
        assert!(!expired.heartbeat(
            "expired-hidden-restore",
            27,
            &expired_offer.token,
            expired_deadline + 1
        ));
    }

    #[test]
    fn old_generation_offer_cannot_adopt_a_replacement_session() {
        let old = new_lease(9, "old-handle", 1_000);
        let replacement = new_lease(10, "new-handle", 1_001);
        let old_offer = old.offer();

        assert!(!replacement.acknowledge(
            "old-handle",
            old_offer.generation,
            &old_offer.token,
            1_002
        ));
        assert!(!replacement.is_live(1_003));
        assert_eq!(
            playback_lease_phase(replacement.state.load(Ordering::Acquire)),
            PLAYBACK_LEASE_AWAITING_ACK
        );
    }

    #[test]
    fn output_is_suppressed_until_ack_and_after_lease_expiry() {
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            2,
            "output".to_string(),
            Arc::clone(&runtime.close_requested),
            PlaybackVisibility::default(),
            10,
        ));
        let offer = lease.offer();
        runtime.adoption_lease.set(Arc::clone(&lease)).unwrap();

        assert!(!runtime.output_permitted_at(11));
        assert!(!runtime.close_requested.load(Ordering::Acquire));
        assert!(lease.acknowledge("output", 2, &offer.token, 11));
        assert!(runtime.output_permitted_at(12));

        let deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));
        assert!(!runtime.output_permitted_at(deadline));
        assert!(runtime.close_requested.load(Ordering::Acquire));
    }

    #[test]
    fn output_callback_latches_close_for_expired_unadopted_offer() {
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            4,
            "unadopted-expiry".to_string(),
            Arc::clone(&runtime.close_requested),
            PlaybackVisibility::default(),
            10,
        ));
        let offer = lease.offer();
        runtime.adoption_lease.set(Arc::clone(&lease)).unwrap();
        let deadline = playback_lease_deadline(lease.state.load(Ordering::Acquire));
        let held_transition = lease.visibility.lock_transition();

        assert!(!runtime.output_permitted_at(deadline));
        assert!(runtime.close_requested.load(Ordering::Acquire));
        assert!(lease.is_revoked());
        drop(held_transition);
        assert!(!lease.revoke_if_expired_at(deadline));
        assert!(!lease.acknowledge("unadopted-expiry", 4, &offer.token, deadline));
        assert!(lease.is_revoked());
    }

    #[test]
    fn expired_output_callback_revokes_ack_still_inside_its_critical_section() {
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            6,
            "ack-expiry-race".to_string(),
            Arc::clone(&runtime.close_requested),
            PlaybackVisibility::default(),
            10,
        ));
        let offer = lease.offer();
        runtime.adoption_lease.set(Arc::clone(&lease)).unwrap();
        let clock = Arc::new(AtomicU64::new(10_009));
        let clock_calls = Arc::new(AtomicU64::new(0));
        let (sampled_tx, sampled_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker_lease = Arc::clone(&lease);
        let worker_clock = Arc::clone(&clock);
        let worker_clock_calls = Arc::clone(&clock_calls);
        let handle = "ack-expiry-race".to_string();
        let token = offer.token.clone();
        let worker = thread::spawn(move || {
            worker_lease.acknowledge_with_clock_and_wait_hook(
                &handle,
                6,
                &token,
                || {
                    let now_ms = worker_clock.load(Ordering::Acquire);
                    if worker_clock_calls.fetch_add(1, Ordering::AcqRel) == 1 {
                        sampled_tx.send(now_ms).unwrap();
                        resume_rx.recv().unwrap();
                    }
                    now_ms
                },
                || {},
            )
        });

        assert_eq!(
            sampled_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            10_009
        );
        clock.store(10_010, Ordering::Release);
        assert!(!runtime.output_permitted_at(10_010));
        assert!(runtime.close_requested.load(Ordering::Acquire));

        resume_tx.send(()).unwrap();
        assert!(!worker.join().unwrap());
        assert!(lease.is_revoked());
    }

    #[test]
    fn output_callback_defers_close_while_valid_tray_grace_is_transitioning() {
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let visibility = PlaybackVisibility::default();
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            5,
            "transitioning-tray-grace".to_string(),
            Arc::clone(&runtime.close_requested),
            visibility.clone(),
            1_000,
        ));
        let offer = lease.offer();
        assert!(lease.acknowledge("transitioning-tray-grace", 5, &offer.token, 1_001));
        runtime.adoption_lease.set(Arc::clone(&lease)).unwrap();

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let transition_visibility = visibility.clone();
        let transition = thread::spawn(move || {
            transition_visibility.set_hidden_at_with_hook(true, 2_000, || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(!runtime.output_permitted_at(31_002));
        assert!(
            !runtime.close_requested.load(Ordering::Acquire),
            "a busy transition may still reconcile the old deadline into valid tray grace"
        );

        release_tx.send(()).unwrap();
        assert!(transition.join().unwrap());
        assert!(runtime.output_permitted_at(31_002));
        assert!(!runtime.close_requested.load(Ordering::Acquire));
        assert_eq!(
            playback_lease_deadline(lease.state.load(Ordering::Acquire)),
            302_000
        );
    }

    #[test]
    fn output_callback_stays_silent_while_restore_grace_is_unreconciled() {
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let visibility = PlaybackVisibility::default();
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            7,
            "transitioning-restore".to_string(),
            Arc::clone(&runtime.close_requested),
            visibility.clone(),
            1_000,
        ));
        let offer = lease.offer();
        assert!(lease.acknowledge("transitioning-restore", 7, &offer.token, 1_001));
        runtime.adoption_lease.set(Arc::clone(&lease)).unwrap();
        assert!(visibility.set_hidden_at(true, 2_000));
        assert!(lease.visibility_changed(true, 2_000));

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let transition_visibility = visibility.clone();
        let transition = thread::spawn(move || {
            transition_visibility.set_hidden_at_with_hook(false, 5_000, || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(!runtime.output_permitted_at(35_000));
        assert!(
            !runtime.close_requested.load(Ordering::Acquire),
            "a busy restore transition cannot use its stale five-minute hidden deadline"
        );

        release_tx.send(()).unwrap();
        assert!(transition.join().unwrap());
        assert!(!runtime.output_permitted_at(35_000));
        assert!(runtime.close_requested.load(Ordering::Acquire));
        assert!(lease.is_revoked());
    }

    #[test]
    fn lease_watchdog_latches_output_and_returns_while_manager_close_is_parked() {
        let manager = PlaybackManager::new();
        let opening = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let guard_token = guard.try_open_playback_token().unwrap();
        let registry = Arc::new(ProcessOwnerRegistry::new());
        registry
            .reserve(manager.clone(), Arc::clone(&guard), opening.generation)
            .unwrap();
        let now = playback_lease_tick_ms();
        let runtime_close = Arc::new(AtomicBool::new(false));
        let adoption = Arc::new(PlaybackAdoptionLease::new_at(
            opening.generation,
            "parked-close".to_string(),
            Arc::clone(&runtime_close),
            PlaybackVisibility::default(),
            now.saturating_sub(1),
        ));
        adoption.state.store(
            playback_lease_word(PLAYBACK_LEASE_ADOPTED, now.saturating_sub(1)),
            Ordering::Release,
        );
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::Active;
        registry.slot.lock().unwrap().as_mut().unwrap().owner = 17;
        registry
            .slot
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .adoption_lease = Some(adoption);
        let root_registry = Arc::new(OnceLock::new());
        root_registry.set(Arc::downgrade(&registry)).unwrap();

        let held_manager = manager.inner.lock().unwrap();
        let watchdog_registry = Arc::clone(&root_registry);
        let watchdog =
            thread::spawn(move || process_pending_root_close_request(&watchdog_registry));
        watchdog.join().unwrap();
        for _ in 0..100 {
            process_pending_root_close_request(&root_registry);
        }
        assert!(runtime_close.load(Ordering::Acquire));
        assert!(
            guard.playback_open(),
            "watchdog expiry is not cleanup proof"
        );
        {
            let root = registry.slot.lock().unwrap();
            let slot = root.as_ref().unwrap();
            assert!(slot.close_dispatching.load(Ordering::Acquire));
            assert_eq!(slot.close_dispatch_attempts.load(Ordering::Acquire), 1);
        }

        drop(held_manager);
        let finish_by = Instant::now() + Duration::from_secs(2);
        loop {
            let dispatching = registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|slot| slot.close_dispatching.load(Ordering::Acquire));
            if !dispatching {
                break;
            }
            assert!(
                Instant::now() < finish_by,
                "parked close requester did not finish"
            );
            thread::sleep(Duration::from_millis(1));
        }
        registry.slot.lock().unwrap().take();
        assert_eq!(guard.try_release_playback_token(guard_token), Ok(true));
        manager.abort_open(opening.generation);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackSourceIdentity {
    source_id: String,
    capture_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloseReceipt {
    pub(crate) closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackAdoptionReceipt {
    pub(crate) adopted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackHeartbeatReceipt {
    pub(crate) alive: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackLeaseBinding {
    generation: u64,
    token: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PlaybackManager {
    inner: Arc<Mutex<PlaybackSlot>>,
    lifecycle: Arc<PlaybackLifecycle>,
}

#[allow(dead_code)]
struct ProcessOwnerRegistry {
    slot: Mutex<Option<ProcessOwnerSlot>>,
    supervisor: Mutex<Option<std::sync::Weak<ProcessCleanupSupervisor>>>,
}

#[allow(dead_code)]
struct PrepareHandleCapsule {
    handle: Mutex<PrepareHandleInner>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrepareStatus {
    Reserved,
    Starting,
    Published,
    Claimed,
    Joining,
    Joined,
    NoWorker,
    Terminal,
}

#[derive(Debug)]
struct PrepareHandleInner {
    status: PrepareStatus,
    handle: Option<JoinHandle<()>>,
    fallback: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
struct PrepareClaim {
    capsule: Arc<PrepareHandleCapsule>,
}

#[allow(dead_code)]
struct PrepareSpawnPermit {
    capsule: Arc<PrepareHandleCapsule>,
    pending: Option<JoinHandle<()>>,
}

impl PrepareSpawnPermit {
    fn attach(&mut self, handle: JoinHandle<()>) -> Result<(), JoinHandle<()>> {
        if self.pending.is_some() {
            return Err(handle);
        }
        self.pending = Some(handle);
        Ok(())
    }

    fn spawn_with(
        &mut self,
        work: Box<dyn FnOnce() + Send>,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> std::io::Result<()> {
        // No user code, lock acquisition or other fallible operation occurs
        // between successful spawn and moving the handle into this permit.
        let handle = spawn(work)?;
        if let Err(handle) = self.attach(handle) {
            self.capsule.retain_terminal(handle);
            return Err(std::io::Error::other("duplicate prepare handle"));
        }
        Ok(())
    }

    fn mark_no_worker(&mut self) -> Result<(), ReviewError> {
        let mut inner = self
            .capsule
            .handle
            .lock()
            .map_err(|_| safe_storage_error())?;
        if self.pending.is_some()
            || inner.status != PrepareStatus::Starting
            || inner.handle.is_some()
        {
            return Err(safe_storage_error());
        }
        inner.status = PrepareStatus::NoWorker;
        Ok(())
    }

    fn publish(mut self) -> Result<(), ReviewError> {
        let handle = self.pending.take().ok_or_else(safe_storage_error)?;
        match self.capsule.publish(handle) {
            Ok(()) => Ok(()),
            Err(handle) => {
                self.pending = Some(handle);
                Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Prepare publication failed.",
                    false,
                ))
            }
        }
    }
}

impl Drop for PrepareSpawnPermit {
    fn drop(&mut self) {
        if let Some(handle) = self.pending.take() {
            self.capsule.retain_terminal(handle);
        } else if let Ok(mut inner) = self.capsule.handle.lock() {
            if inner.status == PrepareStatus::Starting {
                inner.status = PrepareStatus::Terminal;
            }
        }
    }
}

impl PrepareClaim {
    fn join(self) -> Result<(), ReviewError> {
        let handle = {
            let mut inner = self
                .capsule
                .handle
                .lock()
                .map_err(|_| safe_storage_error())?;
            if inner.status != PrepareStatus::Claimed {
                return Err(safe_storage_error());
            }
            let handle = inner.handle.take().ok_or_else(safe_storage_error)?;
            inner.status = PrepareStatus::Joining;
            handle
        };
        let result = handle.join();
        let mut inner = self
            .capsule
            .handle
            .lock()
            .map_err(|_| safe_storage_error())?;
        inner.status = if result.is_ok() {
            PrepareStatus::Joined
        } else {
            PrepareStatus::Terminal
        };
        result.map_err(|_| playback_error("NATIVE_UNAVAILABLE", "Prepare owner failed.", false))
    }
}

impl Drop for PrepareClaim {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.capsule.handle.lock() {
            if inner.status == PrepareStatus::Claimed {
                inner.status = PrepareStatus::Published;
            } else if inner.status == PrepareStatus::Joining {
                inner.status = PrepareStatus::Terminal;
            }
        }
    }
}

#[allow(dead_code)]
impl PrepareHandleCapsule {
    fn reserve_permit(self: &Arc<Self>) -> Result<PrepareSpawnPermit, ReviewError> {
        self.reserve_spawn()?;
        Ok(PrepareSpawnPermit {
            capsule: Arc::clone(self),
            pending: None,
        })
    }
    fn reserve_spawn(&self) -> Result<(), ReviewError> {
        let mut inner = self.handle.lock().map_err(|_| safe_storage_error())?;
        if inner.status != PrepareStatus::Reserved {
            return Err(safe_storage_error());
        }
        inner.status = PrepareStatus::Starting;
        Ok(())
    }

    fn retain_terminal(&self, handle: JoinHandle<()>) {
        let mut inner = self
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.status = PrepareStatus::Terminal;
        if inner.handle.is_none() {
            inner.handle = Some(handle);
        } else {
            // One non-clonable spawn permit exists per capsule, so at most
            // one rejected publication can require this fallback.
            inner.fallback.push(handle);
        }
    }
    fn status(&self) -> PrepareStatus {
        self.handle
            .lock()
            .map(|inner| inner.status)
            .unwrap_or(PrepareStatus::Terminal)
    }

    fn claim(self: &Arc<Self>) -> Result<PrepareClaim, ReviewError> {
        let mut inner = self.handle.lock().map_err(|_| safe_storage_error())?;
        if inner.status != PrepareStatus::Published || inner.handle.is_none() {
            return Err(safe_storage_error());
        }
        inner.status = PrepareStatus::Claimed;
        Ok(PrepareClaim {
            capsule: Arc::clone(self),
        })
    }
    fn new() -> Self {
        Self {
            handle: Mutex::new(PrepareHandleInner {
                status: PrepareStatus::Reserved,
                handle: None,
                fallback: Vec::new(),
            }),
        }
    }
    fn publish(&self, handle: JoinHandle<()>) -> Result<(), JoinHandle<()>> {
        match self.handle.lock() {
            Ok(mut slot)
                if matches!(
                    slot.status,
                    PrepareStatus::Reserved | PrepareStatus::Starting
                ) && slot.handle.is_none() =>
            {
                slot.handle = Some(handle);
                slot.status = PrepareStatus::Published;
                Ok(())
            }
            _ => Err(handle),
        }
    }
}

#[allow(dead_code)]
struct ProcessOwnerSlot {
    manager: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    generation: u64,
    owner: u128,
    close_requested: Arc<AtomicBool>,
    close_dispatching: Arc<AtomicBool>,
    close_dispatch_failures: Arc<AtomicU8>,
    close_dispatch_attempts: Arc<AtomicU64>,
    adoption_lease: Option<Arc<PlaybackAdoptionLease>>,
    phase: ProcessOwnerPhase,
    pending_open: Option<Arc<Mutex<Option<OpenPlaybackTask>>>>,
    prepare_owner: Arc<PrepareHandleCapsule>,
    worker_owner: Arc<PrepareHandleCapsule>,
    reaping: Option<Arc<Mutex<Option<PlaybackSession>>>>,
    late_fallback: Arc<Mutex<Option<PlaybackSession>>>,
    late_collision: Arc<Mutex<Option<PlaybackSession>>>,
    late_reserved: bool,
    reap_ready_token: Option<u64>,
    reaper_scheduled: bool,
    reaper_owner: Arc<PrepareHandleCapsule>,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
enum ProcessOwnerPhase {
    PreHandoff,
    Owned,
    Claimed,
    Executing,
    ExecutorCompleted,
    Active,
    Reaping,
    Terminal,
}

// A single executing claim gets one non-clonable late-session permit. The
// reservation remains held until the process slot is cleared after cleanup.
struct LateReservation {
    fallback: Arc<Mutex<Option<PlaybackSession>>>,
    collision: Arc<Mutex<Option<PlaybackSession>>>,
}

#[allow(dead_code)]
struct ProcessOpenClaim {
    registry: Arc<ProcessOwnerRegistry>,
    manager: PlaybackManager,
    generation: u64,
    close_requested: Arc<AtomicBool>,
    deadline: Instant,
}

impl ProcessOpenClaim {
    fn complete(&mut self) -> Result<(), ReviewError> {
        let reaper = {
            let root = self
                .registry
                .slot
                .lock()
                .map_err(|_| safe_storage_error())?;
            let slot = root
                .as_ref()
                .filter(|slot| {
                    slot.generation == self.generation
                        && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
                })
                .ok_or_else(safe_storage_error)?;
            if slot.phase == ProcessOwnerPhase::Executing && slot.reaper_scheduled {
                Some(Arc::clone(&slot.reaper_owner))
            } else if matches!(
                slot.phase,
                ProcessOwnerPhase::Executing
                    | ProcessOwnerPhase::Active
                    | ProcessOwnerPhase::Reaping
            ) {
                None
            } else {
                return Err(safe_storage_error());
            }
        };
        if let Some(reaper) = reaper {
            reaper.claim()?.join()?;
        }
        let mut finished = false;
        {
            let mut registry = self
                .registry
                .slot
                .lock()
                .map_err(|_| safe_storage_error())?;
            if let Some(slot) = registry.as_mut().filter(|slot| {
                slot.generation == self.generation
                    && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
            }) {
                if slot.phase == ProcessOwnerPhase::Executing {
                    slot.phase = if slot.reap_ready_token.is_some() || slot.reaper_scheduled {
                        ProcessOwnerPhase::ExecutorCompleted
                    } else {
                        ProcessOwnerPhase::Terminal
                    };
                    if let Some(token) = slot.reap_ready_token {
                        if slot.guard.try_release_playback_token(token) != Ok(true) {
                            slot.phase = ProcessOwnerPhase::Terminal;
                            return Err(safe_storage_error());
                        }
                        registry.take();
                        finished = true;
                    }
                }
            }
        }
        if finished {
            if let Ok(mut manager) = self.manager.inner.lock() {
                if manager
                    .opening
                    .as_ref()
                    .is_some_and(|opening| opening.generation == self.generation)
                {
                    manager.opening = None;
                }
            }
        }
        Ok(())
    }

    fn cancel_unstarted(&mut self) -> Result<(), ReviewError> {
        let mut manager = self
            .manager
            .inner
            .lock()
            .map_err(|_| safe_storage_error())?;
        let mut root = self
            .registry
            .slot
            .lock()
            .map_err(|_| safe_storage_error())?;
        let slot = root
            .as_ref()
            .filter(|slot| {
                slot.generation == self.generation
                    && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
                    && slot.phase == ProcessOwnerPhase::Claimed
                    && slot.prepare_owner.status() == PrepareStatus::Reserved
                    && slot.worker_owner.status() == PrepareStatus::Reserved
                    && !slot.late_reserved
                    && !slot.reaper_scheduled
            })
            .ok_or_else(safe_storage_error)?;
        let task_capsule = slot.pending_open.as_ref().ok_or_else(safe_storage_error)?;
        let task_guard = task_capsule.lock().map_err(|_| safe_storage_error())?;
        let task = task_guard.as_ref().ok_or_else(safe_storage_error)?;
        if task.lease.generation != self.generation
            || !Arc::ptr_eq(&task.guard, &slot.guard)
            || slot
                .late_fallback
                .lock()
                .map_err(|_| safe_storage_error())?
                .is_some()
            || slot
                .late_collision
                .lock()
                .map_err(|_| safe_storage_error())?
                .is_some()
        {
            return Err(safe_storage_error());
        }
        task.lease.cancel.store(true, Ordering::Release);
        if task.guard.try_release_playback_token(task.guard_token) != Ok(true) {
            return Err(safe_storage_error());
        }
        drop(task_guard);
        let removed = root.take();
        if manager
            .opening
            .as_ref()
            .is_some_and(|opening| opening.generation == self.generation)
        {
            manager.opening = None;
        }
        drop(root);
        drop(manager);
        drop(removed);
        Ok(())
    }

    fn mark_terminal(&mut self) {
        if let Ok(mut root) = self.registry.slot.lock() {
            if let Some(slot) = root.as_mut().filter(|slot| {
                slot.generation == self.generation
                    && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
            }) {
                slot.phase = ProcessOwnerPhase::Terminal;
            }
        }
    }

    fn execute(
        mut self,
        supervisor: &ProcessCleanupSupervisor,
    ) -> Result<PlaybackState, ReviewError> {
        supervisor.attach_registry(&self.registry)?;
        let task = self.take_task()?;
        let owner = task.owner;
        let manager = self.manager.clone();
        let result = open_playback_owned(task);
        // The supervisor joins reapers separately so timeout reaches the caller
        // without waiting for blocked native I/O.
        if let Err(error) = supervisor.submit(self) {
            if let Ok(state) = &result {
                // The handle cannot be delivered. Suppress and retain the exact
                // active session through the same owned-close path.
                drop(dispatch_owned_close(manager, owner, &state.handle));
            }
            return Err(error);
        }
        result
    }

    fn take_task(&mut self) -> Result<OpenPlaybackTask, ReviewError> {
        let (task, rejected_as_timeout) = {
            let mut registry = self
                .registry
                .slot
                .lock()
                .map_err(|_| safe_storage_error())?;
            let slot = registry
                .as_mut()
                .filter(|slot| {
                    slot.generation == self.generation
                        && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
                        && slot.phase == ProcessOwnerPhase::Claimed
                })
                .ok_or_else(safe_storage_error)?;
            let capsule = slot.pending_open.as_ref().ok_or_else(safe_storage_error)?;
            let mut pending = capsule.lock().map_err(|_| safe_storage_error())?;
            let current = pending.as_ref().ok_or_else(safe_storage_error)?;
            if current.lease.generation != self.generation || current.deadline != self.deadline {
                return Err(safe_storage_error());
            }

            let mut rejected_as_timeout = Instant::now() >= self.deadline;
            let cancelled = current.lease.cancel.load(Ordering::Acquire);
            if rejected_as_timeout || cancelled {
                (None, rejected_as_timeout)
            } else {
                let task = pending.take().ok_or_else(safe_storage_error)?;
                rejected_as_timeout = Instant::now() >= self.deadline;
                if rejected_as_timeout || task.lease.cancel.load(Ordering::Acquire) {
                    *pending = Some(task);
                    (None, rejected_as_timeout)
                } else {
                    slot.phase = ProcessOwnerPhase::Executing;
                    (Some(task), false)
                }
            }
        };

        if let Some(task) = task {
            return Ok(task);
        }
        if let Err(error) = self.cancel_unstarted() {
            self.mark_terminal();
            return Err(error);
        }
        Err(if rejected_as_timeout {
            open_timeout_error()
        } else {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native desktop window is unavailable.",
                true,
            )
        })
    }
}

// One process-lifetime worker; a single process registry admits at most one
// playback owner. A blocked reaper intentionally blocks subsequent admission.
struct CloseRetryIdentity {
    registry: std::sync::Weak<ProcessOwnerRegistry>,
    generation: u64,
    capsule: Arc<Mutex<Option<PlaybackSession>>>,
}

enum ProcessCleanupJob {
    Open(ProcessOpenClaim),
    WakeClose,
    Close {
        manager: PlaybackManager,
        registry: std::sync::Weak<ProcessOwnerRegistry>,
        generation: u64,
        owner: u128,
        handle: String,
        capsule: Arc<Mutex<Option<PlaybackSession>>>,
    },
    #[cfg(test)]
    Barrier {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    },
}

impl ProcessCleanupJob {
    fn still_owned(&self) -> bool {
        let (registry, manager, generation) = match self {
            #[cfg(test)]
            Self::Barrier { .. } => return false,
            Self::WakeClose => return false,
            Self::Open(claim) => (
                Some(Arc::clone(&claim.registry)),
                &claim.manager,
                claim.generation,
            ),
            Self::Close {
                manager,
                registry,
                generation,
                ..
            } => (registry.upgrade(), manager, *generation),
        };
        registry
            .and_then(|registry| {
                registry.slot.lock().ok().map(|root| {
                    root.as_ref().is_some_and(|slot| {
                        slot.generation == generation
                            && Arc::ptr_eq(&slot.manager.inner, &manager.inner)
                    })
                })
            })
            .unwrap_or(true)
    }
}

struct ProcessCleanupSupervisor {
    sender: mpsc::SyncSender<ProcessCleanupJob>,
    failed: Arc<Mutex<Option<ProcessCleanupJob>>>,
    alive: Arc<AtomicBool>,
    root_registry: Arc<OnceLock<std::sync::Weak<ProcessOwnerRegistry>>>,
}

struct SupervisorHealth(Arc<AtomicBool>);
impl Drop for SupervisorHealth {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct ProcessPlaybackRoot {
    registry: Arc<ProcessOwnerRegistry>,
    supervisor: Arc<ProcessCleanupSupervisor>,
}

impl ProcessPlaybackRoot {
    fn start() -> Result<Arc<Self>, ReviewError> {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start()?;
        supervisor.attach_registry(&registry)?;
        *registry
            .supervisor
            .lock()
            .map_err(|_| safe_storage_error())? = Some(Arc::downgrade(&supervisor));
        spawn_playback_lease_watchdog(Arc::clone(&supervisor.root_registry))?;
        Ok(Arc::new(Self {
            registry,
            supervisor,
        }))
    }
}

fn process_playback_root() -> Result<&'static Arc<ProcessPlaybackRoot>, ReviewError> {
    static ROOT: std::sync::OnceLock<Option<Arc<ProcessPlaybackRoot>>> = std::sync::OnceLock::new();
    let root = ROOT
        .get_or_init(|| ProcessPlaybackRoot::start().ok())
        .as_ref()
        .ok_or_else(safe_storage_error)?;
    if !root.supervisor.alive.load(Ordering::Acquire) {
        return Err(safe_storage_error());
    }
    Ok(root)
}

fn process_pending_root_close_request(
    root_registry: &OnceLock<std::sync::Weak<ProcessOwnerRegistry>>,
) {
    let Some(registry) = root_registry.get().and_then(std::sync::Weak::upgrade) else {
        return;
    };
    let pending = match registry.slot.try_lock() {
        Ok(mut root) => root.as_mut().and_then(|slot| {
            if slot.phase == ProcessOwnerPhase::Active {
                if let Some(lease) = slot.adoption_lease.as_ref() {
                    lease.revoke_if_expired();
                    if lease.is_revoked() {
                        slot.close_requested.store(true, Ordering::Release);
                    }
                }
            }
            (slot.phase == ProcessOwnerPhase::Active
                && slot.close_requested.load(Ordering::Acquire))
            .then(|| {
                (
                    slot.manager.clone(),
                    slot.generation,
                    slot.owner,
                    Arc::clone(&slot.close_requested),
                    Arc::clone(&slot.close_dispatching),
                    Arc::clone(&slot.close_dispatch_failures),
                    Arc::clone(&slot.close_dispatch_attempts),
                )
            })
        }),
        Err(std::sync::TryLockError::Poisoned(_) | std::sync::TryLockError::WouldBlock) => return,
    };
    let Some((manager, generation, owner, close_requested, dispatching, failures, attempts)) =
        pending
    else {
        return;
    };
    if failures.load(Ordering::Acquire) >= MAX_LEASE_CLOSE_DISPATCH_FAILURES {
        mark_lease_close_terminal_if_active(&registry, &manager, generation, &close_requested);
        return;
    }
    if dispatching
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    if close_requested
        .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        dispatching.store(false, Ordering::Release);
        return;
    }
    let close_requested_after_spawn_failure = Arc::clone(&close_requested);
    let dispatching_after_spawn_failure = Arc::clone(&dispatching);
    let failures_after_spawn_failure = Arc::clone(&failures);
    let registry_after_spawn_failure = Arc::clone(&registry);
    let close_manager = manager.clone();
    let finish_manager = manager.clone();
    let close_registry = Arc::downgrade(&registry);
    attempts.fetch_add(1, Ordering::AcqRel);
    let spawned = thread::Builder::new()
        .name("fung-playback-lease-close-request".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                close_active_process_request(close_manager, close_registry, generation, owner, None)
            }));
            finish_lease_close_attempt(
                &registry,
                &finish_manager,
                generation,
                &close_requested,
                &dispatching,
                &failures,
                result,
            );
        });
    if spawned.is_err() {
        // The root still owns the exact close request. The independent
        // watchdog retries a bounded number of thread-creation failures.
        let failed = failures_after_spawn_failure.fetch_add(1, Ordering::AcqRel) + 1;
        dispatching_after_spawn_failure.store(false, Ordering::Release);
        if failed >= MAX_LEASE_CLOSE_DISPATCH_FAILURES {
            mark_lease_close_terminal_if_active(
                &registry_after_spawn_failure,
                &manager,
                generation,
                &close_requested_after_spawn_failure,
            );
        } else {
            close_requested_after_spawn_failure.store(true, Ordering::Release);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn mark_lease_close_terminal_if_active(
    registry: &Arc<ProcessOwnerRegistry>,
    manager: &PlaybackManager,
    generation: u64,
    close_requested: &AtomicBool,
) {
    match registry.slot.try_lock() {
        Ok(mut root) => {
            if let Some(slot) = root.as_mut().filter(|slot| {
                slot.generation == generation && Arc::ptr_eq(&slot.manager.inner, &manager.inner)
            }) {
                if slot.phase == ProcessOwnerPhase::Active {
                    slot.phase = ProcessOwnerPhase::Terminal;
                }
            }
            close_requested.store(false, Ordering::Release);
        }
        Err(std::sync::TryLockError::Poisoned(_) | std::sync::TryLockError::WouldBlock) => {
            close_requested.store(true, Ordering::Release);
        }
    }
}

fn finish_lease_close_attempt(
    registry: &ProcessOwnerRegistry,
    manager: &PlaybackManager,
    generation: u64,
    close_requested: &AtomicBool,
    dispatching: &AtomicBool,
    failures: &AtomicU8,
    result: std::thread::Result<Result<(), ReviewError>>,
) {
    let mut root = match registry.slot.lock() {
        Ok(root) => root,
        Err(_) => {
            eprintln!("process playback close remains owned by poisoned root");
            return;
        }
    };
    let Some(slot) = root.as_mut().filter(|slot| {
        slot.generation == generation && Arc::ptr_eq(&slot.manager.inner, &manager.inner)
    }) else {
        dispatching.store(false, Ordering::Release);
        return;
    };
    if slot.phase == ProcessOwnerPhase::Active {
        match result {
            Ok(Ok(())) => {
                // A completed close request should have moved the exact root
                // to Reaping and then released it. Active here is an invariant
                // failure, so retain ownership terminally.
                slot.phase = ProcessOwnerPhase::Terminal;
                slot.close_requested.store(false, Ordering::Release);
            }
            _ => {
                let failed = failures.fetch_add(1, Ordering::AcqRel) + 1;
                if failed >= MAX_LEASE_CLOSE_DISPATCH_FAILURES {
                    slot.phase = ProcessOwnerPhase::Terminal;
                    slot.close_requested.store(false, Ordering::Release);
                } else {
                    close_requested.store(true, Ordering::Release);
                }
            }
        }
    }
    dispatching.store(false, Ordering::Release);
}

fn spawn_playback_lease_watchdog(
    root_registry: Arc<OnceLock<std::sync::Weak<ProcessOwnerRegistry>>>,
) -> Result<(), ReviewError> {
    thread::Builder::new()
        .name("fung-playback-lease-watchdog".into())
        .spawn(move || loop {
            if root_registry
                .get()
                .and_then(std::sync::Weak::upgrade)
                .is_none()
            {
                break;
            }
            process_pending_root_close_request(&root_registry);
            thread::sleep(Duration::from_millis(10));
        })
        .map(|_| ())
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "Playback lease watchdog is unavailable.",
                true,
            )
        })
}

impl ProcessCleanupSupervisor {
    fn start() -> Result<Arc<Self>, ReviewError> {
        let (sender, receiver) = mpsc::sync_channel::<ProcessCleanupJob>(1);
        let alive = Arc::new(AtomicBool::new(true));
        let worker_alive = Arc::clone(&alive);
        let failed = Arc::new(Mutex::new(None));
        let worker_failed = Arc::clone(&failed);
        let root_registry = Arc::new(OnceLock::new());
        let worker_root_registry = Arc::clone(&root_registry);
        thread::Builder::new()
            .name("fung-process-playback-supervisor".into())
            .spawn(move || {
                let _health = SupervisorHealth(worker_alive);
                loop {
                    let mut job = match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(job) => job,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    loop {
                        let result =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match job {
                                ProcessCleanupJob::Open(mut claim) => claim.complete(),
                                ProcessCleanupJob::WakeClose => Ok(()),
                                #[cfg(test)]
                                ProcessCleanupJob::Barrier { entered, release } => {
                                    let _ = entered.send(());
                                    let _ = release.recv();
                                    Ok(())
                                }
                                ProcessCleanupJob::Close {
                                    manager,
                                    registry,
                                    generation,
                                    owner,
                                    handle,
                                    capsule,
                                } => {
                                    let valid = registry
                                        .upgrade()
                                        .and_then(|registry| {
                                            registry.slot.lock().ok().map(|root| {
                                                root.as_ref().is_some_and(|slot| {
                                                    slot.generation == generation
                                                        && Arc::ptr_eq(
                                                            &slot.manager.inner,
                                                            &manager.inner,
                                                        )
                                                        && slot.reaping.as_ref().is_some_and(
                                                            |held| Arc::ptr_eq(held, &capsule),
                                                        )
                                                })
                                            })
                                        })
                                        .unwrap_or(false);
                                    if !valid {
                                        return Err(safe_storage_error());
                                    }
                                    tauri::async_runtime::block_on(dispatch_owned_close_once(
                                        manager,
                                        owner,
                                        &handle,
                                        CloseRetryIdentity {
                                            registry,
                                            generation,
                                            capsule,
                                        },
                                    ))
                                }
                            }));
                        if !matches!(result, Ok(Ok(()))) {
                            eprintln!("playback supervisor retained failed cleanup");
                        }
                        // A close wake can be dropped when the bounded queue is
                        // full. Recheck the authoritative root after every job
                        // so its atomic request is still serviced once the
                        // current owner operation completes.
                        process_pending_root_close_request(&worker_root_registry);
                        let queued = receiver.try_recv().ok();
                        let next = queued.or_else(|| {
                            worker_failed
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .take()
                        });
                        let Some(next) = next else {
                            break;
                        };
                        job = next;
                    }
                }
            })
            .map_err(|_| {
                playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback supervisor is unavailable.",
                    true,
                )
            })?;
        Ok(Arc::new(Self {
            sender,
            failed,
            alive,
            root_registry,
        }))
    }

    fn attach_registry(&self, registry: &Arc<ProcessOwnerRegistry>) -> Result<(), ReviewError> {
        if self.root_registry.set(Arc::downgrade(registry)).is_ok()
            || self
                .root_registry
                .get()
                .and_then(std::sync::Weak::upgrade)
                .is_some_and(|registered| Arc::ptr_eq(&registered, registry))
        {
            Ok(())
        } else {
            Err(safe_storage_error())
        }
    }

    fn wake_close_nonblocking(&self) {
        if self.alive.load(Ordering::Acquire) {
            let _ = self.sender.try_send(ProcessCleanupJob::WakeClose);
        }
    }

    fn submit(&self, claim: ProcessOpenClaim) -> Result<(), ReviewError> {
        self.attach_registry(&claim.registry)?;
        self.submit_job(ProcessCleanupJob::Open(claim))
    }

    fn submit_job(&self, job: ProcessCleanupJob) -> Result<(), ReviewError> {
        let result = if self.alive.load(Ordering::Acquire) {
            self.sender.try_send(job)
        } else {
            Err(mpsc::TrySendError::Disconnected(job))
        };
        match result {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job)) => {
                // Single-admission invariant bounds this parking slot to one.
                let mut failed = self
                    .failed
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let stale = failed.as_ref().is_some_and(|prior| !prior.still_owned());
                if stale {
                    failed.take();
                }
                if failed.is_none() {
                    *failed = Some(job);
                }
                // A violated single-admission invariant remains terminal. The
                // registry continues to retain all spawned owner capsules.
                Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback cleanup is pending.",
                    false,
                ))
            }
        }
    }
}

impl Drop for ProcessOpenClaim {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.registry.slot.lock() {
            if let Some(slot) = registry.as_mut().filter(|slot| {
                slot.generation == self.generation
                    && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
            }) {
                match slot.phase {
                    ProcessOwnerPhase::Claimed => {
                        let untouched = slot.pending_open.as_ref().is_some_and(|task| {
                            task.lock().map(|task| task.is_some()).unwrap_or(false)
                        });
                        slot.phase = if untouched {
                            ProcessOwnerPhase::Owned
                        } else {
                            ProcessOwnerPhase::Terminal
                        };
                    }
                    ProcessOwnerPhase::Executing => slot.phase = ProcessOwnerPhase::Terminal,
                    _ => {}
                }
            }
        }
    }
}

#[allow(dead_code)]
struct ProcessOpenAdmission {
    manager: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    registry: Arc<ProcessOwnerRegistry>,
    lease: OpenLease,
    guard_token: u64,
    rollback_armed: bool,
}

#[allow(dead_code)]
impl ProcessOpenAdmission {
    fn handoff(
        mut self,
        io: OpenPlaybackIo,
        timeout: Duration,
        owner: u128,
    ) -> Result<(), ReviewError> {
        let task = OpenPlaybackTask {
            playback: self.manager.clone(),
            guard: Arc::clone(&self.guard),
            guard_token: self.guard_token,
            lease: self.lease.clone(),
            io,
            deadline: Instant::now() + timeout,
            owner,
            prepare_owner: None,
            worker_owner: None,
            process_registry: Some(Arc::downgrade(&self.registry)),
            process_task: None,
            late_owner: None,
            late_collision: None,
        };
        let capsule = Arc::new(Mutex::new(Some(task)));
        {
            // Manager before registry: shutdown cannot invalidate the opening
            // between this check and publication of the owned task.
            let manager = self
                .manager
                .inner
                .lock()
                .map_err(|_| safe_storage_error())?;
            if self.manager.lifecycle.disposed.load(Ordering::Acquire)
                || self.lease.cancel.load(Ordering::Acquire)
                || manager
                    .opening
                    .as_ref()
                    .is_none_or(|opening| opening.generation != self.lease.generation)
            {
                return Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback admission was cancelled.",
                    true,
                ));
            }
            let mut registry = self
                .registry
                .slot
                .lock()
                .map_err(|_| safe_storage_error())?;
            let slot = registry
                .as_mut()
                .filter(|slot| {
                    slot.generation == self.lease.generation
                        && Arc::ptr_eq(&slot.manager.inner, &self.manager.inner)
                        && slot.phase == ProcessOwnerPhase::PreHandoff
                })
                .ok_or_else(safe_storage_error)?;
            {
                let mut task = capsule.lock().map_err(|_| safe_storage_error())?;
                task.as_mut().ok_or_else(safe_storage_error)?.process_task =
                    Some(Arc::downgrade(&capsule));
                task.as_mut().ok_or_else(safe_storage_error)?.prepare_owner =
                    Some(Arc::clone(&slot.prepare_owner));
                task.as_mut().ok_or_else(safe_storage_error)?.worker_owner =
                    Some(Arc::clone(&slot.worker_owner));
                task.as_mut().ok_or_else(safe_storage_error)?.late_owner =
                    Some(Arc::clone(&slot.late_fallback));
                task.as_mut().ok_or_else(safe_storage_error)?.late_collision =
                    Some(Arc::clone(&slot.late_collision));
            }
            slot.owner = owner;
            slot.pending_open = Some(Arc::clone(&capsule));
            slot.phase = ProcessOwnerPhase::Owned;
            self.rollback_armed = false;
        }
        Ok(())
    }
    #[cfg(test)]
    fn reserve_with_hook(
        manager: PlaybackManager,
        guard: Arc<NativeCaptureGuard>,
        registry: Arc<ProcessOwnerRegistry>,
        hook: impl FnOnce(),
    ) -> Result<Self, ReviewError> {
        Self::reserve_impl(manager, guard, registry, hook)
    }
    fn reserve(
        manager: PlaybackManager,
        guard: Arc<NativeCaptureGuard>,
        registry: Arc<ProcessOwnerRegistry>,
    ) -> Result<Self, ReviewError> {
        Self::reserve_impl(manager, guard, registry, || {})
    }

    fn reserve_impl(
        manager: PlaybackManager,
        guard: Arc<NativeCaptureGuard>,
        registry: Arc<ProcessOwnerRegistry>,
        hook: impl FnOnce(),
    ) -> Result<Self, ReviewError> {
        let lease = manager.begin_open()?;
        if let Err(error) = registry.reserve(manager.clone(), Arc::clone(&guard), lease.generation)
        {
            manager.abort_open(lease.generation);
            return Err(error);
        }
        let guard_token = match guard.try_open_playback_token() {
            Ok(token) => token,
            Err(error) => {
                return match error {
                    recording_review::PlaybackGuardFailure::Busy(error) => {
                        let _ = registry.release(&manager, lease.generation);
                        manager.abort_open(lease.generation);
                        Err(map_admission_error(error))
                    }
                    recording_review::PlaybackGuardFailure::Poisoned
                    | recording_review::PlaybackGuardFailure::Exhausted => {
                        // The guard state is uncertain; retain both process owner
                        // and opening reservation rather than claiming rollback.
                        Err(playback_error(
                            "NATIVE_UNAVAILABLE",
                            "Playback admission is unavailable.",
                            false,
                        ))
                    }
                };
            }
        };
        let admission = Self {
            manager,
            guard,
            registry,
            lease,
            guard_token,
            rollback_armed: true,
        };
        hook();
        {
            let slot = admission
                .manager
                .inner
                .lock()
                .map_err(|_| safe_storage_error())?;
            let valid = !admission.manager.lifecycle.disposed.load(Ordering::Acquire)
                && !admission.lease.cancel.load(Ordering::Acquire)
                && slot
                    .opening
                    .as_ref()
                    .is_some_and(|opening| opening.generation == admission.lease.generation);
            if !valid {
                return Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback admission was cancelled.",
                    true,
                ));
            }
        }
        Ok(admission)
    }
}

impl Drop for ProcessOpenAdmission {
    fn drop(&mut self) {
        // This type is pre-handoff only; no worker has been created. Poison or
        // stale owner uncertainty must nevertheless keep admission fail-closed.
        if !self.rollback_armed {
            return;
        }
        if self
            .registry
            .rollback_before_handoff(
                &self.manager,
                &self.guard,
                self.lease.generation,
                self.guard_token,
            )
            .is_err()
        {
            return;
        }
        if let Ok(mut slot) = self.manager.inner.lock() {
            if slot
                .opening
                .as_ref()
                .is_some_and(|opening| opening.generation == self.lease.generation)
            {
                slot.opening = None;
            }
        }
    }
}

#[allow(dead_code)]
impl ProcessOwnerRegistry {
    fn claim_open(
        self: &Arc<Self>,
        manager: &PlaybackManager,
        generation: u64,
    ) -> Result<ProcessOpenClaim, ReviewError> {
        // Match the handoff's manager -> registry lock order so shutdown
        // cannot invalidate the generation while a normal claim is admitted.
        let manager_slot = manager.inner.lock().map_err(|_| safe_storage_error())?;
        if manager.lifecycle.disposed.load(Ordering::Acquire)
            || !manager_slot.opening.as_ref().is_some_and(|opening| {
                opening.generation == generation && !opening.cancel.load(Ordering::Acquire)
            })
        {
            return Err(playback_error(
                "NATIVE_UNAVAILABLE",
                "Playback admission was cancelled.",
                true,
            ));
        }
        let mut registry = self.slot.lock().map_err(|_| safe_storage_error())?;
        let slot = registry
            .as_mut()
            .filter(|slot| {
                slot.generation == generation
                    && Arc::ptr_eq(&slot.manager.inner, &manager.inner)
                    && slot.phase == ProcessOwnerPhase::Owned
                    && slot.pending_open.is_some()
            })
            .ok_or_else(safe_storage_error)?;
        let deadline = slot
            .pending_open
            .as_ref()
            .ok_or_else(safe_storage_error)?
            .lock()
            .map_err(|_| safe_storage_error())?
            .as_ref()
            .map(|task| task.deadline)
            .ok_or_else(safe_storage_error)?;
        let close_requested = Arc::clone(&slot.close_requested);
        slot.phase = ProcessOwnerPhase::Claimed;
        Ok(ProcessOpenClaim {
            registry: Arc::clone(self),
            manager: manager.clone(),
            generation,
            close_requested,
            deadline,
        })
    }
    fn cancel_unclaimed(
        self: &Arc<Self>,
        manager: &PlaybackManager,
        generation: u64,
    ) -> Result<(), ReviewError> {
        let (deadline, close_requested) = {
            let _manager = manager.inner.lock().map_err(|_| safe_storage_error())?;
            let mut root = self.slot.lock().map_err(|_| safe_storage_error())?;
            let slot = root
                .as_mut()
                .filter(|slot| {
                    slot.generation == generation
                        && Arc::ptr_eq(&slot.manager.inner, &manager.inner)
                        && slot.phase == ProcessOwnerPhase::Owned
                })
                .ok_or_else(safe_storage_error)?;
            let deadline = slot
                .pending_open
                .as_ref()
                .ok_or_else(safe_storage_error)?
                .lock()
                .map_err(|_| safe_storage_error())?
                .as_ref()
                .map(|task| task.deadline)
                .ok_or_else(safe_storage_error)?;
            let close_requested = Arc::clone(&slot.close_requested);
            slot.phase = ProcessOwnerPhase::Claimed;
            (deadline, close_requested)
        };
        let mut claim = ProcessOpenClaim {
            registry: Arc::clone(self),
            manager: manager.clone(),
            generation,
            close_requested,
            deadline,
        };
        match claim.cancel_unstarted() {
            Ok(()) => Ok(()),
            Err(error) => {
                claim.mark_terminal();
                Err(error)
            }
        }
    }

    fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            supervisor: Mutex::new(None),
        }
    }
    fn reserve(
        &self,
        manager: PlaybackManager,
        guard: Arc<NativeCaptureGuard>,
        generation: u64,
    ) -> Result<(), ReviewError> {
        let mut slot = self.slot.lock().map_err(|_| safe_storage_error())?;
        if slot.is_some() || generation == 0 {
            return Err(playback_error(
                "PLAYBACK_BUSY",
                "Playback cleanup owner is unavailable.",
                true,
            ));
        }
        *slot = Some(ProcessOwnerSlot {
            manager,
            guard,
            generation,
            owner: 0,
            close_requested: Arc::new(AtomicBool::new(false)),
            close_dispatching: Arc::new(AtomicBool::new(false)),
            close_dispatch_failures: Arc::new(AtomicU8::new(0)),
            close_dispatch_attempts: Arc::new(AtomicU64::new(0)),
            adoption_lease: None,
            phase: ProcessOwnerPhase::PreHandoff,
            pending_open: None,
            prepare_owner: Arc::new(PrepareHandleCapsule::new()),
            worker_owner: Arc::new(PrepareHandleCapsule::new()),
            reaping: None,
            late_fallback: Arc::new(Mutex::new(None)),
            late_collision: Arc::new(Mutex::new(None)),
            late_reserved: false,
            reap_ready_token: None,
            reaper_scheduled: false,
            reaper_owner: Arc::new(PrepareHandleCapsule::new()),
        });
        Ok(())
    }
    fn release(&self, manager: &PlaybackManager, generation: u64) -> Result<(), ReviewError> {
        let mut slot = self.slot.lock().map_err(|_| safe_storage_error())?;
        if !slot.as_ref().is_some_and(|owner| {
            owner.generation == generation
                && Arc::ptr_eq(&owner.manager.inner, &manager.inner)
                && owner.phase == ProcessOwnerPhase::PreHandoff
        }) {
            return Err(playback_error(
                "PLAYBACK_HANDLE_INVALID",
                "Playback cleanup owner is stale.",
                false,
            ));
        }
        slot.take();
        Ok(())
    }

    fn rollback_before_handoff(
        &self,
        manager: &PlaybackManager,
        guard: &Arc<NativeCaptureGuard>,
        generation: u64,
        guard_token: u64,
    ) -> Result<(), ReviewError> {
        // Registry -> capture guard, with no manager/SDK lock held. If either
        // lock is poisoned, retain the slot and guard instead of assuming safe
        // rollback. This operation only applies before any worker is started.
        let mut slot = self.slot.lock().map_err(|_| safe_storage_error())?;
        if !slot.as_ref().is_some_and(|owner| {
            owner.generation == generation
                && Arc::ptr_eq(&owner.manager.inner, &manager.inner)
                && Arc::ptr_eq(&owner.guard, guard)
                && owner.phase == ProcessOwnerPhase::PreHandoff
        }) {
            return Err(safe_storage_error());
        }
        if !guard
            .try_release_playback_token(guard_token)
            .map_err(|_| safe_storage_error())?
        {
            return Err(safe_storage_error());
        }
        slot.take();
        Ok(())
    }
    #[cfg(test)]
    fn owner(&self) -> Option<PlaybackManager> {
        self.slot
            .lock()
            .ok()?
            .as_ref()
            .map(|slot| slot.manager.clone())
    }
}

#[derive(Debug)]
struct PlaybackLifecycle {
    disposed: AtomicBool,
    generation: AtomicU64,
    routed_controls: Mutex<()>,
    visibility: PlaybackVisibility,
}

#[derive(Debug, Default)]
struct OpenAttemptCleanup {
    runtime: OnceLock<Arc<PlaybackRuntime>>,
    worker_close: OnceLock<mpsc::Sender<WorkerCommand>>,
}

impl OpenAttemptCleanup {
    fn signal_close(&self) {
        if let Some(runtime) = self.runtime.get() {
            runtime.close_requested.store(true, Ordering::Release);
            if let Some(cancel) = runtime.routed_cancel.get() {
                cancel.store(true, Ordering::Release);
            }
        }
        if let Some(worker_close) = self.worker_close.get() {
            let _ = worker_close.send(WorkerCommand::Close);
        }
    }
}

#[derive(Debug, Clone)]
struct OpenLease {
    generation: u64,
    cancel: Arc<AtomicBool>,
    cleanup: Arc<OpenAttemptCleanup>,
}

#[derive(Debug)]
struct OpeningPlayback {
    generation: u64,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
struct PlaybackSlot {
    opening: Option<OpeningPlayback>,
    active: Option<PlaybackSession>,
    closing: Option<ClosingPlayback>,
    closed: std::collections::HashMap<String, ClosedPlayback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClosingPhase {
    Starting,
    Running,
    Retryable,
    Terminal,
}

#[derive(Debug)]
struct ClosingPlayback {
    owner: u128,
    handle: String,
    adoption: Option<PlaybackAdoptionOffer>,
    capsule: Arc<Mutex<Option<PlaybackSession>>>,
    phase: ClosingPhase,
    retry_enqueued: bool,
}

#[derive(Debug, Clone)]
struct ClosedPlayback {
    owner: u128,
    adoption: Option<PlaybackAdoptionOffer>,
}

#[derive(Debug, Clone)]
struct ProcessPlaybackIdentity {
    registry: std::sync::Weak<ProcessOwnerRegistry>,
    task: std::sync::Weak<Mutex<Option<OpenPlaybackTask>>>,
    generation: u64,
}

#[derive(Debug)]
struct PlaybackSession {
    process_identity: Option<ProcessPlaybackIdentity>,
    guard_token: u64,
    owner: u128,
    handle: String,
    project_id: String,
    recording_id: String,
    channel: String,
    duration_ms: u64,
    duration_frames: u64,
    sample_rate: u32,
    runtime: Arc<PlaybackRuntime>,
    command_tx: mpsc::Sender<WorkerCommand>,
    join: Option<WorkerJoin>,
    guard: Arc<NativeCaptureGuard>,
}

#[cfg(test)]
fn test_held_guard() -> Arc<NativeCaptureGuard> {
    let guard = Arc::new(NativeCaptureGuard::default());
    assert_eq!(guard.try_open_playback_token().unwrap(), 1);
    guard
}

#[derive(Debug)]
struct PlaybackInstallFailure {
    error: ReviewError,
    session: PlaybackSession,
}

impl PlaybackManager {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PlaybackSlot::default())),
            lifecycle: Arc::new(PlaybackLifecycle {
                disposed: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                routed_controls: Mutex::new(()),
                visibility: PlaybackVisibility::default(),
            }),
        }
    }

    pub(crate) fn set_hidden_to_tray(&self, hidden: bool) {
        if !self.lifecycle.visibility.set_hidden_current(hidden) {
            return;
        }
        if let Ok(slot) = self.inner.try_lock() {
            if let Some(lease) = slot
                .active
                .as_ref()
                .and_then(|session| session.runtime.adoption_lease.get())
            {
                lease.visibility_changed_current(hidden);
            }
        }
    }

    #[cfg(test)]
    fn set_hidden_to_tray_at(&self, hidden: bool, now_ms: u64) {
        if !self.lifecycle.visibility.set_hidden_at(hidden, now_ms) {
            return;
        }
        if let Ok(slot) = self.inner.try_lock() {
            if let Some(lease) = slot
                .active
                .as_ref()
                .and_then(|session| session.runtime.adoption_lease.get())
            {
                lease.visibility_changed(hidden, now_ms);
            }
        }
    }

    fn begin_open(&self) -> Result<OpenLease, ReviewError> {
        let mut slot = self.inner.lock().expect("playback mutex poisoned");
        if self.lifecycle.disposed.load(Ordering::Acquire) {
            return Err(playback_error(
                "NATIVE_UNAVAILABLE",
                "The native desktop window is unavailable.",
                true,
            ));
        }
        if slot.opening.is_some() || slot.active.is_some() || slot.closing.is_some() {
            return Err(playback_error(
                "PLAYBACK_BUSY",
                "Another playback session is open.",
                false,
            ));
        }
        self.lifecycle.visibility.reset_for_new_open();
        let generation = self.lifecycle.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        slot.opening = Some(OpeningPlayback {
            generation,
            cancel: Arc::clone(&cancel),
        });
        Ok(OpenLease {
            generation,
            cancel,
            cleanup: Arc::new(OpenAttemptCleanup::default()),
        })
    }

    fn abort_open(&self, generation: u64) {
        let mut slot = self.inner.lock().expect("playback mutex poisoned");
        if slot
            .opening
            .as_ref()
            .is_some_and(|opening| opening.generation == generation)
        {
            slot.opening = None;
        }
    }

    fn install(
        &self,
        generation: u64,
        session: PlaybackSession,
    ) -> Result<(), Box<PlaybackInstallFailure>> {
        let mut slot = self.inner.lock().expect("playback mutex poisoned");
        let Some(opening) = slot.opening.as_ref() else {
            return Err(Box::new(PlaybackInstallFailure {
                error: playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native desktop window is unavailable.",
                    true,
                ),
                session,
            }));
        };
        if self.lifecycle.disposed.load(Ordering::Acquire)
            || opening.generation != generation
            || opening.cancel.load(Ordering::Acquire)
        {
            return Err(Box::new(PlaybackInstallFailure {
                error: playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native desktop window is unavailable.",
                    true,
                ),
                session,
            }));
        }
        if slot.active.is_some() {
            return Err(Box::new(PlaybackInstallFailure {
                error: playback_error("PLAYBACK_BUSY", "Another playback session is open.", false),
                session,
            }));
        }
        if let Some(identity) = &session.process_identity {
            let transition = (|| -> Result<(), ReviewError> {
                let registry = identity.registry.upgrade().ok_or_else(safe_storage_error)?;
                let task = identity.task.upgrade().ok_or_else(safe_storage_error)?;
                let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
                let owner = root
                    .as_mut()
                    .filter(|owner| {
                        owner.generation == generation
                            && identity.generation == generation
                            && Arc::ptr_eq(&owner.manager.inner, &self.inner)
                            && Arc::ptr_eq(&owner.guard, &session.guard)
                            && owner.phase == ProcessOwnerPhase::Executing
                            && owner
                                .pending_open
                                .as_ref()
                                .is_some_and(|pending| Arc::ptr_eq(pending, &task))
                    })
                    .ok_or_else(safe_storage_error)?;
                if task.lock().map_err(|_| safe_storage_error())?.is_some() {
                    return Err(safe_storage_error());
                }
                owner.adoption_lease = session.runtime.adoption_lease.get().cloned();
                owner.phase = ProcessOwnerPhase::Active;
                Ok(())
            })();
            if let Err(error) = transition {
                return Err(Box::new(PlaybackInstallFailure { error, session }));
            }
        }
        slot.opening = None;
        slot.active = Some(session);
        Ok(())
    }

    fn install_from_fallback(
        &self,
        generation: u64,
        fallback: &Arc<Mutex<Option<PlaybackSession>>>,
    ) -> Result<(), ReviewError> {
        let mut manager = self.inner.lock().map_err(|_| safe_storage_error())?;
        let opening = manager.opening.as_ref().ok_or_else(safe_storage_error)?;
        if self.lifecycle.disposed.load(Ordering::Acquire)
            || opening.generation != generation
            || opening.cancel.load(Ordering::Acquire)
            || manager.active.is_some()
        {
            return Err(safe_storage_error());
        }
        // The session remains rooted in fallback until every check passes.
        let identity = fallback
            .lock()
            .map_err(|_| safe_storage_error())?
            .as_ref()
            .and_then(|session| session.process_identity.clone())
            .ok_or_else(safe_storage_error)?;
        let registry = identity.registry.upgrade().ok_or_else(safe_storage_error)?;
        let task = identity.task.upgrade().ok_or_else(safe_storage_error)?;
        let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
        let owner = root
            .as_mut()
            .filter(|owner| {
                owner.generation == generation
                    && identity.generation == generation
                    && Arc::ptr_eq(&owner.manager.inner, &self.inner)
                    && owner.phase == ProcessOwnerPhase::Executing
                    && owner.late_reserved
                    && Arc::ptr_eq(&owner.late_fallback, fallback)
                    && owner
                        .pending_open
                        .as_ref()
                        .is_some_and(|pending| Arc::ptr_eq(pending, &task))
            })
            .ok_or_else(safe_storage_error)?;
        let mut stored = fallback.lock().map_err(|_| safe_storage_error())?;
        let session = stored.as_ref().ok_or_else(safe_storage_error)?;
        if !Arc::ptr_eq(&owner.guard, &session.guard) {
            return Err(safe_storage_error());
        }
        owner.adoption_lease = session.runtime.adoption_lease.get().cloned();
        owner.phase = ProcessOwnerPhase::Active;
        manager.opening = None;
        manager.active = stored.take();
        Ok(())
    }

    fn suppress_late(fallback: &Arc<Mutex<Option<PlaybackSession>>>) {
        let stored = fallback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(session) = stored.as_ref() {
            session
                .runtime
                .close_requested
                .store(true, Ordering::Release);
            signal_routed_cancel(session);
            let _ = session.command_tx.send(WorkerCommand::Close);
        }
    }

    fn reserve_late_cleanup(
        &self,
        identity: &ProcessPlaybackIdentity,
        fallback: &Arc<Mutex<Option<PlaybackSession>>>,
    ) -> Result<(u128, String), ReviewError> {
        let mut manager = self.inner.lock().map_err(|_| safe_storage_error())?;
        if manager.active.is_some() || manager.closing.is_some() {
            return Err(safe_storage_error());
        }
        let registry = identity.registry.upgrade().ok_or_else(safe_storage_error)?;
        let task = identity.task.upgrade().ok_or_else(safe_storage_error)?;
        let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
        let owner = root
            .as_mut()
            .filter(|owner| {
                owner.generation == identity.generation
                    && Arc::ptr_eq(&owner.manager.inner, &self.inner)
                    && owner.phase == ProcessOwnerPhase::Executing
                    && owner.late_reserved
                    && Arc::ptr_eq(&owner.late_fallback, fallback)
                    && owner
                        .pending_open
                        .as_ref()
                        .is_some_and(|pending| Arc::ptr_eq(pending, &task))
                    && owner.reaping.is_none()
            })
            .ok_or_else(safe_storage_error)?;
        let stored = fallback.lock().map_err(|_| safe_storage_error())?;
        let session = stored.as_ref().ok_or_else(safe_storage_error)?;
        if !Arc::ptr_eq(&owner.guard, &session.guard) {
            return Err(safe_storage_error());
        }
        let adoption = session
            .runtime
            .adoption_lease
            .get()
            .map(|lease| lease.offer());
        session
            .runtime
            .close_requested
            .store(true, Ordering::Release);
        signal_routed_cancel(session);
        let _ = session.command_tx.send(WorkerCommand::Close);
        let identity_pair = (session.owner, session.handle.clone());
        owner.reaping = Some(Arc::clone(fallback));
        owner.phase = ProcessOwnerPhase::Reaping;
        manager.opening = None;
        manager.closing = Some(ClosingPlayback {
            owner: session.owner,
            handle: session.handle.clone(),
            adoption,
            capsule: Arc::clone(fallback),
            phase: ClosingPhase::Retryable,
            retry_enqueued: false,
        });
        Ok(identity_pair)
    }

    #[cfg(test)]
    fn reserve_owned_close(
        &self,
        owner: u128,
        handle: &str,
    ) -> Result<Option<Arc<Mutex<Option<PlaybackSession>>>>, ReviewError> {
        self.reserve_owned_close_checked(owner, handle, None)
    }

    fn reserve_owned_close_checked(
        &self,
        owner: u128,
        handle: &str,
        expected: Option<&CloseRetryIdentity>,
    ) -> Result<Option<Arc<Mutex<Option<PlaybackSession>>>>, ReviewError> {
        let mut slot = self.inner.lock().map_err(|_| safe_storage_error())?;
        if let Some(expected) = expected {
            let closing = slot
                .closing
                .as_ref()
                .filter(|closing| {
                    closing.owner == owner
                        && closing.handle == handle
                        && closing.phase == ClosingPhase::Retryable
                        && Arc::ptr_eq(&closing.capsule, &expected.capsule)
                })
                .ok_or_else(safe_storage_error)?;
            let _ = closing;
            let registry = expected.registry.upgrade().ok_or_else(safe_storage_error)?;
            let root = registry.slot.lock().map_err(|_| safe_storage_error())?;
            if !root.as_ref().is_some_and(|root_owner| {
                root_owner.generation == expected.generation
                    && Arc::ptr_eq(&root_owner.manager.inner, &self.inner)
                    && root_owner
                        .reaping
                        .as_ref()
                        .is_some_and(|capsule| Arc::ptr_eq(capsule, &expected.capsule))
            }) {
                return Err(safe_storage_error());
            }
        }
        if let Some(closing) = slot.closing.as_mut() {
            if closing.owner != owner || closing.handle != handle {
                return Err(playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                ));
            }
            if closing.phase == ClosingPhase::Retryable {
                closing.phase = ClosingPhase::Starting;
                return Ok(Some(Arc::clone(&closing.capsule)));
            }
            if closing.phase == ClosingPhase::Terminal {
                return Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback cleanup failed.",
                    false,
                ));
            }
            return Err(playback_error(
                "PLAYBACK_BUSY",
                "Playback cleanup is pending or failed.",
                true,
            ));
        }
        if slot
            .closed
            .get(handle)
            .is_some_and(|issued| issued.owner == owner)
        {
            return Ok(None);
        }
        let session = slot.active.take().ok_or_else(|| {
            playback_error(
                "PLAYBACK_HANDLE_INVALID",
                "The playback handle is invalid.",
                false,
            )
        })?;
        if session.owner != owner || session.handle != handle {
            slot.active = Some(session);
            return Err(playback_error(
                "PLAYBACK_HANDLE_INVALID",
                "The playback handle is invalid.",
                false,
            ));
        }
        let identity = session.process_identity.clone();
        let adoption = session
            .runtime
            .adoption_lease
            .get()
            .map(|lease| lease.offer());
        let capsule = Arc::new(Mutex::new(Some(session)));
        if let Some(identity) = &identity {
            let transition = (|| -> Result<(), ReviewError> {
                let registry = identity.registry.upgrade().ok_or_else(safe_storage_error)?;
                let task = identity.task.upgrade().ok_or_else(safe_storage_error)?;
                let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
                let root_owner = root
                    .as_mut()
                    .filter(|candidate| {
                        candidate.generation == identity.generation
                            && Arc::ptr_eq(&candidate.manager.inner, &self.inner)
                            && candidate.phase == ProcessOwnerPhase::Active
                            && candidate
                                .pending_open
                                .as_ref()
                                .is_some_and(|pending| Arc::ptr_eq(pending, &task))
                            && candidate.reaping.is_none()
                    })
                    .ok_or_else(safe_storage_error)?;
                root_owner.reaping = Some(Arc::clone(&capsule));
                root_owner.phase = ProcessOwnerPhase::Reaping;
                Ok(())
            })();
            if let Err(error) = transition {
                slot.active = capsule.lock().map_err(|_| safe_storage_error())?.take();
                return Err(error);
            }
        }
        // Root ownership is established before suppressing the worker. If
        // transition fails the restored active session remains usable.
        {
            let session = capsule.lock().map_err(|_| safe_storage_error())?;
            let session = session.as_ref().ok_or_else(safe_storage_error)?;
            session
                .runtime
                .close_requested
                .store(true, Ordering::Release);
            signal_routed_cancel(session);
            let _ = session.command_tx.send(WorkerCommand::Close);
        }
        slot.closing = Some(ClosingPlayback {
            owner,
            handle: handle.to_owned(),
            adoption,
            capsule: Arc::clone(&capsule),
            phase: ClosingPhase::Starting,
            retry_enqueued: false,
        });
        Ok(Some(capsule))
    }

    #[cfg(test)]
    fn take_for_close(
        &self,
        owner: u128,
        handle: &str,
    ) -> Result<Option<PlaybackSession>, ReviewError> {
        let mut slot = self.inner.lock().expect("playback mutex poisoned");
        if let Some(session) = slot.active.as_ref() {
            if session.handle != handle {
                return Err(playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                ));
            }
            if session.owner != owner {
                return Err(playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                ));
            }
            return Ok(slot.active.take());
        }
        if slot
            .closed
            .get(handle)
            .is_some_and(|issued_owner| issued_owner.owner == owner)
        {
            return Ok(None);
        }
        Err(playback_error(
            "PLAYBACK_HANDLE_INVALID",
            "The playback handle is invalid.",
            false,
        ))
    }

    fn mark_closed(&self, handle: String, owner: u128, adoption: Option<PlaybackAdoptionOffer>) {
        let mut slot = self.inner.lock().expect("playback mutex poisoned");
        if slot.closed.len() >= 16 {
            if let Some(oldest) = slot.closed.keys().next().cloned() {
                slot.closed.remove(&oldest);
            }
        }
        slot.closed
            .insert(handle, ClosedPlayback { owner, adoption });
    }

    fn active_snapshot(&self, owner: u128, handle: &str) -> Result<PlaybackSnapshot, ReviewError> {
        let slot = self.inner.lock().expect("playback mutex poisoned");
        let session = slot
            .active
            .as_ref()
            .filter(|session| session.handle == handle && session.owner == owner)
            .ok_or_else(|| {
                playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                )
            })?;
        Ok(PlaybackSnapshot {
            handle: session.handle.clone(),
            project_id: session.project_id.clone(),
            recording_id: session.recording_id.clone(),
            channel: session.channel.clone(),
            duration_ms: session.duration_ms,
            sample_rate: session.sample_rate,
            runtime: Arc::clone(&session.runtime),
        })
    }

    fn adoption_lease_for(
        &self,
        owner: u128,
        handle: &str,
    ) -> Result<Arc<PlaybackAdoptionLease>, ReviewError> {
        let slot = self.inner.lock().map_err(|_| safe_storage_error())?;
        let session = slot
            .active
            .as_ref()
            .filter(|session| session.handle == handle && session.owner == owner)
            .ok_or_else(|| {
                playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                )
            })?;
        session
            .runtime
            .adoption_lease
            .get()
            .cloned()
            .ok_or_else(safe_storage_error)
    }

    fn adopt(
        &self,
        owner: u128,
        handle: &str,
        generation: u64,
        token: &str,
    ) -> Result<(), ReviewError> {
        let lease = self.adoption_lease_for(owner, handle)?;
        if lease.acknowledge_current(handle, generation, token) {
            Ok(())
        } else {
            Err(playback_error(
                "PLAYBACK_ADOPTION_EXPIRED",
                "The playback adoption offer is stale or expired.",
                false,
            ))
        }
    }

    fn heartbeat(
        &self,
        owner: u128,
        handle: &str,
        generation: u64,
        token: &str,
    ) -> Result<(), ReviewError> {
        let lease = self.adoption_lease_for(owner, handle)?;
        if lease.heartbeat_current(handle, generation, token) {
            Ok(())
        } else {
            Err(playback_error(
                "PLAYBACK_LEASE_EXPIRED",
                "The playback session lease has expired.",
                false,
            ))
        }
    }

    fn validate_adopted_binding(
        &self,
        owner: u128,
        handle: &str,
        generation: u64,
        token: &str,
    ) -> Result<(), ReviewError> {
        let lease = self.adoption_lease_for(owner, handle)?;
        if !lease.matches(handle, generation, token) {
            return Err(playback_error(
                "PLAYBACK_HANDLE_INVALID",
                "The playback handle is invalid.",
                false,
            ));
        }
        if lease.is_live(playback_lease_tick_ms()) {
            Ok(())
        } else {
            Err(playback_error(
                "PLAYBACK_ADOPTION_REQUIRED",
                "Playback must be adopted before it can be used.",
                false,
            ))
        }
    }

    fn validate_close_binding(
        &self,
        owner: u128,
        handle: &str,
        generation: u64,
        token: &str,
    ) -> Result<(), ReviewError> {
        let slot = self.inner.lock().map_err(|_| safe_storage_error())?;
        if let Some(session) = slot
            .active
            .as_ref()
            .filter(|session| session.handle == handle && session.owner == owner)
        {
            let lease = session
                .runtime
                .adoption_lease
                .get()
                .ok_or_else(safe_storage_error)?;
            if lease.matches(handle, generation, token) {
                return Ok(());
            }
        }
        let binding_matches = |adoption: &PlaybackAdoptionOffer| {
            adoption.generation == generation && adoption.token == token
        };
        if let Some(closing) = slot
            .closing
            .as_ref()
            .filter(|closing| closing.handle == handle && closing.owner == owner)
        {
            if closing.adoption.as_ref().is_some_and(binding_matches) {
                return Ok(());
            }
        }
        if let Some(closed) = slot
            .closed
            .get(handle)
            .filter(|closed| closed.owner == owner)
        {
            if closed.adoption.as_ref().is_some_and(binding_matches) {
                return Ok(());
            }
        }
        Err(playback_error(
            "PLAYBACK_HANDLE_INVALID",
            "The playback handle is invalid.",
            false,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn control(
        &self,
        owner: u128,
        handle: &str,
        expected_epoch: u64,
        action: &str,
        position_ms: Option<u64>,
    ) -> Result<PlaybackSnapshot, ReviewError> {
        let routed = {
            let slot = self.inner.lock().expect("playback mutex poisoned");
            slot.active
                .as_ref()
                .filter(|s| s.handle == handle && s.owner == owner)
                .is_some_and(|s| s.runtime.routed_lease.is_some())
        };
        if routed {
            return self.control_routed(owner, handle, expected_epoch, action, position_ms);
        }
        let slot = self.inner.lock().expect("playback mutex poisoned");
        let session = slot
            .active
            .as_ref()
            .filter(|session| session.handle == handle && session.owner == owner)
            .ok_or_else(|| {
                playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                )
            })?;
        if session.runtime.stream_epoch.load(Ordering::Acquire) != expected_epoch {
            return Err(playback_error(
                "PLAYBACK_STALE_EPOCH",
                "Playback state is stale; refresh and retry.",
                true,
            ));
        }

        match action {
            "play" => {
                session.command_tx.send(WorkerCommand::Play).map_err(|_| {
                    playback_error(
                        "PLAYBACK_HANDLE_INVALID",
                        "The playback handle is invalid.",
                        false,
                    )
                })?;
                session
                    .runtime
                    .state
                    .store(STATE_PLAYING, Ordering::Release);
            }
            "pause" => {
                session.command_tx.send(WorkerCommand::Pause).map_err(|_| {
                    playback_error(
                        "PLAYBACK_HANDLE_INVALID",
                        "The playback handle is invalid.",
                        false,
                    )
                })?;
                session.runtime.state.store(STATE_PAUSED, Ordering::Release);
            }
            "seek" => {
                let position_ms = position_ms
                    .ok_or_else(|| ReviewError::invalid("positionMs is required for seek."))?;
                if position_ms > session.duration_ms {
                    return Err(ReviewError::invalid("positionMs is outside the recording."));
                }
                let frame = frames_for_ms(position_ms, session.sample_rate);
                let epoch = session.runtime.stream_epoch.load(Ordering::Acquire) + 1;
                session.runtime.clear_queue();
                session.runtime.stream_epoch.store(epoch, Ordering::Release);
                session
                    .runtime
                    .position_frames
                    .store(frame, Ordering::Release);
                session
                    .runtime
                    .source_exhausted
                    .store(frame >= session.duration_frames, Ordering::Release);
                session
                    .command_tx
                    .send(WorkerCommand::Seek { frame, epoch })
                    .map_err(|_| {
                        playback_error(
                            "PLAYBACK_HANDLE_INVALID",
                            "The playback handle is invalid.",
                            false,
                        )
                    })?;
                if frame >= session.duration_frames {
                    session.runtime.state.store(STATE_ENDED, Ordering::Release);
                }
            }
            _ => return Err(ReviewError::invalid("action is invalid.")),
        }

        Ok(PlaybackSnapshot {
            handle: session.handle.clone(),
            project_id: session.project_id.clone(),
            recording_id: session.recording_id.clone(),
            channel: session.channel.clone(),
            duration_ms: session.duration_ms,
            sample_rate: session.sample_rate,
            runtime: Arc::clone(&session.runtime),
        })
    }

    fn control_routed(
        &self,
        owner: u128,
        handle: &str,
        expected_epoch: u64,
        action: &str,
        position_ms: Option<u64>,
    ) -> Result<PlaybackSnapshot, ReviewError> {
        // Serialize control requests without retaining the manager mutex while
        // waiting for a real-time callback to finish its output critical section.
        let _serial = self
            .lifecycle
            .routed_controls
            .lock()
            .map_err(|_| safe_storage_error())?;
        let (runtime, lease) = {
            let slot = self.inner.lock().map_err(|_| safe_storage_error())?;
            let session = slot
                .active
                .as_ref()
                .filter(|s| s.owner == owner && s.handle == handle)
                .ok_or_else(|| {
                    playback_error(
                        "PLAYBACK_HANDLE_INVALID",
                        "The playback handle is invalid.",
                        false,
                    )
                })?;
            let lease = session
                .runtime
                .routed_lease
                .as_ref()
                .cloned()
                .ok_or_else(|| {
                    playback_error(
                        "PLAYBACK_SOURCE_INVALID",
                        "Source playback is unavailable.",
                        false,
                    )
                })?;
            (Arc::clone(&session.runtime), lease)
        };
        let _output = lease.output_fence().ok_or_else(|| {
            playback_error(
                "PLAYBACK_SOURCE_REVOKED",
                "Source playback is unavailable.",
                false,
            )
        })?;
        let slot = self.inner.lock().map_err(|_| safe_storage_error())?;
        let session = slot
            .active
            .as_ref()
            .filter(|s| s.owner == owner && s.handle == handle && Arc::ptr_eq(&s.runtime, &runtime))
            .ok_or_else(|| {
                playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                )
            })?;
        if runtime.stream_epoch.load(Ordering::Acquire) != expected_epoch {
            return Err(playback_error(
                "PLAYBACK_STALE_EPOCH",
                "Playback state is stale; refresh and retry.",
                true,
            ));
        }
        // A worker error is terminal. The receiver may still exist briefly
        // while it exits, so a successful send must not revive its state.
        if runtime.state.load(Ordering::Acquire) == STATE_ERROR {
            return Err(runtime
                .error
                .lock()
                .ok()
                .and_then(|error| error.clone())
                .unwrap_or_else(|| {
                    playback_error(
                        "PLAYBACK_SOURCE_INVALID",
                        "Source playback is unavailable.",
                        false,
                    )
                }));
        }
        let ring = runtime.routed_control.get().ok_or_else(|| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "Source playback is unavailable.",
                true,
            )
        })?;
        let send = |command| {
            session.command_tx.send(command).map_err(|_| {
                let error = playback_error(
                    "PLAYBACK_HANDLE_INVALID",
                    "The playback handle is invalid.",
                    false,
                );
                // The worker is no longer accepting commands. Earlier epoch or
                // position updates are not rolled back; keep the session terminal.
                runtime.set_error(error.clone());
                error
            })
        };
        match action {
            "play" => {
                let mut epoch = expected_epoch;
                if runtime.position_frames.load(Ordering::Acquire) >= session.duration_frames {
                    epoch = expected_epoch
                        .checked_add(1)
                        .ok_or_else(safe_storage_error)?;
                    ring.advance_epoch().map_err(|_| {
                        playback_error(
                            "PLAYBACK_SOURCE_REVOKED",
                            "Source playback is unavailable.",
                            false,
                        )
                    })?;
                    runtime.stream_epoch.store(epoch, Ordering::Release);
                    runtime.position_frames.store(0, Ordering::Release);
                    runtime.source_exhausted.store(false, Ordering::Release);
                    send(WorkerCommand::RoutedSeek { epoch, frame: 0 })?;
                }
                send(WorkerCommand::RoutedPlay { epoch })?;
                runtime.underrun_started_ms.store(0, Ordering::Release);
                transition_routed_state(&runtime, STATE_PLAYING)?;
            }
            "pause" => {
                let epoch = expected_epoch
                    .checked_add(1)
                    .ok_or_else(safe_storage_error)?;
                ring.advance_epoch().map_err(|_| {
                    playback_error(
                        "PLAYBACK_SOURCE_REVOKED",
                        "Source playback is unavailable.",
                        false,
                    )
                })?;
                let frame = runtime
                    .position_frames
                    .load(Ordering::Acquire)
                    .min(session.duration_frames);
                transition_routed_state(&runtime, STATE_PAUSED)?;
                runtime.underrun_started_ms.store(0, Ordering::Release);
                runtime.stream_epoch.store(epoch, Ordering::Release);
                send(WorkerCommand::RoutedPause { epoch, frame })?;
            }
            "seek" => {
                let position = position_ms
                    .ok_or_else(|| ReviewError::invalid("positionMs is required for seek."))?;
                if position > session.duration_ms {
                    return Err(ReviewError::invalid("positionMs is outside the recording."));
                }
                let frame = if position == session.duration_ms {
                    session.duration_frames
                } else {
                    frames_for_ms(position, session.sample_rate).min(session.duration_frames)
                };
                let epoch = expected_epoch
                    .checked_add(1)
                    .ok_or_else(safe_storage_error)?;
                ring.advance_epoch().map_err(|_| {
                    playback_error(
                        "PLAYBACK_SOURCE_REVOKED",
                        "Source playback is unavailable.",
                        false,
                    )
                })?;
                runtime.stream_epoch.store(epoch, Ordering::Release);
                runtime.position_frames.store(frame, Ordering::Release);
                runtime.underrun_started_ms.store(0, Ordering::Release);
                runtime
                    .source_exhausted
                    .store(frame >= session.duration_frames, Ordering::Release);
                if frame >= session.duration_frames {
                    transition_routed_state(&runtime, STATE_ENDED)?;
                } else if runtime.state.load(Ordering::Acquire) == STATE_ENDED {
                    transition_routed_state(&runtime, STATE_PAUSED)?;
                }
                send(WorkerCommand::RoutedSeek { epoch, frame })?;
            }
            _ => return Err(ReviewError::invalid("action is invalid.")),
        }
        finalize_routed_control(&runtime, &lease)?;
        Ok(PlaybackSnapshot {
            handle: session.handle.clone(),
            project_id: session.project_id.clone(),
            recording_id: session.recording_id.clone(),
            channel: session.channel.clone(),
            duration_ms: session.duration_ms,
            sample_rate: session.sample_rate,
            runtime,
        })
    }

    pub(crate) fn shutdown(&self) {
        let close = {
            let mut slot = self.inner.lock().expect("playback mutex poisoned");
            self.lifecycle.disposed.store(true, Ordering::SeqCst);
            self.lifecycle.generation.fetch_add(1, Ordering::SeqCst);
            if let Some(opening) = slot.opening.take() {
                opening.cancel.store(true, Ordering::SeqCst);
            }
            slot.active
                .as_ref()
                .map(|s| (s.owner, s.handle.clone()))
                .or_else(|| {
                    slot.closing
                        .as_ref()
                        .filter(|c| c.phase == ClosingPhase::Retryable)
                        .map(|c| (c.owner, c.handle.clone()))
                })
        };
        if let Some((owner, handle)) = close {
            // Starting ownership is synchronous; dropping this future does not
            // drop the spawned cleanup worker or the manager's closing slot.
            drop(dispatch_owned_close(self.clone(), owner, &handle));
        }
    }

    #[cfg(test)]
    fn install_test(&self, session: PlaybackSession) {
        let lease = self.begin_open().unwrap();
        self.install(lease.generation, session).unwrap();
    }
}

#[derive(Debug)]
struct PlaybackSnapshot {
    handle: String,
    project_id: String,
    recording_id: String,
    channel: String,
    duration_ms: u64,
    sample_rate: u32,
    runtime: Arc<PlaybackRuntime>,
}

struct PlaybackRuntime {
    state: AtomicU8,
    position_frames: AtomicU64,
    stream_epoch: AtomicU64,
    queue: Mutex<VecDeque<i16>>,
    source_exhausted: AtomicBool,
    error: Mutex<Option<ReviewError>>,
    degraded: bool,
    missing_ranges: Vec<MissingRange>,
    started_at: Instant,
    underrun_started_ms: AtomicU64,
    device_lost: AtomicBool,
    routed_control: OnceLock<crate::routed_playback_ring::Control>,
    routed_lease: Option<Arc<crate::source_playback_lease::SourcePlaybackLease>>,
    routed_cancel: OnceLock<Arc<AtomicBool>>,
    source_identity: OnceLock<PlaybackSourceIdentity>,
    adoption_lease: OnceLock<Arc<PlaybackAdoptionLease>>,
    close_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for PlaybackRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaybackRuntime").finish_non_exhaustive()
    }
}

impl PlaybackRuntime {
    fn new(degraded: bool, missing_ranges: Vec<MissingRange>) -> Self {
        Self {
            state: AtomicU8::new(STATE_PAUSED),
            position_frames: AtomicU64::new(0),
            stream_epoch: AtomicU64::new(0),
            queue: Mutex::new(VecDeque::with_capacity(MAX_QUEUE_BYTES / 2)),
            source_exhausted: AtomicBool::new(false),
            error: Mutex::new(None),
            degraded,
            missing_ranges,
            started_at: Instant::now(),
            underrun_started_ms: AtomicU64::new(0),
            device_lost: AtomicBool::new(false),
            routed_control: OnceLock::new(),
            routed_lease: None,
            routed_cancel: OnceLock::new(),
            source_identity: OnceLock::new(),
            adoption_lease: OnceLock::new(),
            close_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    fn new_routed(
        degraded: bool,
        missing_ranges: Vec<MissingRange>,
        lease: Arc<crate::source_playback_lease::SourcePlaybackLease>,
    ) -> Self {
        let mut runtime = Self::new(degraded, missing_ranges);
        // Routed playback uses only its bounded SPSC queue; do not keep the
        // legacy 1 MiB allocation alive alongside it.
        runtime.queue = Mutex::new(VecDeque::new());
        runtime.routed_lease = Some(lease);
        runtime
    }

    fn clear_queue(&self) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.clear();
        }
    }

    fn set_error(&self, error: ReviewError) {
        if let Ok(mut target) = self.error.lock() {
            *target = Some(error);
        }
        self.state.store(STATE_ERROR, Ordering::Release);
    }

    fn output_permitted(&self) -> bool {
        self.output_permitted_at(playback_lease_tick_ms())
    }

    fn output_permitted_at(&self, now_ms: u64) -> bool {
        let permitted = match self.adoption_lease.get() {
            None => true,
            Some(lease) => match lease.output_state_at(now_ms) {
                PlaybackOutputLeaseState::Permitted => true,
                PlaybackOutputLeaseState::AwaitingAdoption => false,
                PlaybackOutputLeaseState::ReconciliationPending => false,
                PlaybackOutputLeaseState::Expired => {
                    self.close_requested.store(true, Ordering::Release);
                    false
                }
            },
        };
        permitted && !self.close_requested.load(Ordering::Acquire)
    }

    fn position_ms(&self, sample_rate: u32, duration_ms: u64) -> u64 {
        if self.routed_lease.is_some() && self.state.load(Ordering::Acquire) == STATE_ENDED {
            return duration_ms;
        }
        let frames = self.position_frames.load(Ordering::Acquire);
        let position = frames.saturating_mul(1_000) / u64::from(sample_rate.max(1));
        position.min(duration_ms)
    }

    fn to_public_state(
        &self,
        handle: &str,
        project_id: &str,
        recording_id: &str,
        channel: &str,
        sample_rate: u32,
        duration_ms: u64,
    ) -> PlaybackState {
        let state = match self.state.load(Ordering::Acquire) {
            STATE_PLAYING => "playing",
            STATE_ENDED => "ended",
            STATE_ERROR => "error",
            _ => "paused",
        };
        PlaybackState {
            handle: handle.to_string(),
            project_id: project_id.to_string(),
            recording_id: recording_id.to_string(),
            channel: channel.to_string(),
            source: self.source_identity.get().cloned(),
            state: state.to_string(),
            position_ms: self.position_ms(sample_rate, duration_ms),
            duration_ms,
            stream_epoch: self.stream_epoch.load(Ordering::Acquire),
            degraded: self.degraded,
            missing_ranges: self.missing_ranges.clone(),
            output_latency_ms: None,
            error: self.error.lock().ok().and_then(|error| error.clone()),
            adoption: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ChunkDescriptor {
    id: String,
    path: String,
    sequence_no: i64,
    start_ms: u64,
    end_ms: u64,
    byte_size: u64,
    frame_start: u64,
    frame_end: u64,
    available: bool,
}

#[derive(Debug, Clone)]
struct PreparedPlayback {
    project_root: PathBuf,
    project_id: String,
    recording_id: String,
    channel: String,
    duration_ms: u64,
    sample_rate: u32,
    source_channels: u16,
    duration_frames: u64,
    descriptors: Vec<ChunkDescriptor>,
    degraded: bool,
    missing_ranges: Vec<MissingRange>,
    routed: Option<Arc<RoutedPlaybackSource>>,
}

struct RoutedPlaybackSource {
    storage: Arc<genesis_block_native::Storage>,
    data_root: PathBuf,
    manifest: Arc<crate::completed_source_media::Manifest>,
    source_id: String,
    epoch: i64,
    cancel: Arc<AtomicBool>,
    lease: Arc<crate::source_playback_lease::SourcePlaybackLease>,
}

impl std::fmt::Debug for RoutedPlaybackSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoutedPlaybackSource")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct ValidatedWave {
    spec: hound::WavSpec,
    frames: u64,
}

#[derive(Debug)]
enum WorkerCommand {
    Play,
    Pause,
    Seek { frame: u64, epoch: u64 },
    RoutedPlay { epoch: u64 },
    RoutedPause { epoch: u64, frame: u64 },
    RoutedSeek { epoch: u64, frame: u64 },
    Close,
}

struct OpenSource {
    reader: AudioReader,
    channels: u16,
    sample_rate: u32,
    frame_count: u64,
    next_frame: u64,
}

fn playback_error(code: &str, message: &str, retryable: bool) -> ReviewError {
    ReviewError::new(code, message, retryable)
}

fn routed_terminal_error(runtime: &PlaybackRuntime) -> ReviewError {
    runtime
        .error
        .lock()
        .ok()
        .and_then(|error| error.clone())
        .unwrap_or_else(|| {
            playback_error(
                "PLAYBACK_SOURCE_INVALID",
                "Source playback is unavailable.",
                false,
            )
        })
}

fn transition_routed_state_impl(
    runtime: &PlaybackRuntime,
    next: u8,
    hook: impl FnOnce(),
) -> Result<(), ReviewError> {
    let mut current = runtime.state.load(Ordering::Acquire);
    let mut hook = Some(hook);
    loop {
        if current == STATE_ERROR {
            return Err(routed_terminal_error(runtime));
        }
        if let Some(hook) = hook.take() {
            hook();
        }
        match runtime.state.compare_exchange_weak(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn transition_routed_state(runtime: &PlaybackRuntime, next: u8) -> Result<(), ReviewError> {
    transition_routed_state_impl(runtime, next, || {})
}

#[cfg(test)]
fn transition_routed_state_with_hook(
    runtime: &PlaybackRuntime,
    next: u8,
    hook: impl FnOnce(),
) -> Result<(), ReviewError> {
    transition_routed_state_impl(runtime, next, hook)
}

/// Caller holds the output fence. A later revocation can occur after this
/// linearization point; it does not make an earlier success a lasting lease.
fn finalize_routed_control(
    runtime: &PlaybackRuntime,
    lease: &crate::source_playback_lease::SourcePlaybackLease,
) -> Result<(), ReviewError> {
    if !lease.is_available() {
        return Err(playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        ));
    }
    if runtime.state.load(Ordering::Acquire) == STATE_ERROR {
        return Err(routed_terminal_error(runtime));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenWaitError {
    Timeout,
    Cancelled,
    Disconnected,
}

fn open_timeout_error() -> ReviewError {
    playback_error(
        "PLAYBACK_OPEN_TIMEOUT",
        "Playback preparation timed out.",
        true,
    )
}

fn open_start_error(cancel: &AtomicBool, deadline: Instant) -> Option<ReviewError> {
    if Instant::now() >= deadline {
        Some(open_timeout_error())
    } else if cancel.load(Ordering::Acquire) {
        Some(playback_error(
            "NATIVE_UNAVAILABLE",
            "The native desktop window is unavailable.",
            true,
        ))
    } else {
        None
    }
}

fn check_open_cancelled(cancel: &AtomicBool) -> Result<(), ReviewError> {
    if cancel.load(Ordering::Acquire) {
        Err(open_timeout_error())
    } else {
        Ok(())
    }
}

fn recv_until_deadline_or_cancel<T>(
    receiver: &mpsc::Receiver<T>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<T, OpenWaitError> {
    recv_until_deadline_or_cancel_with_clock(receiver, deadline, cancel, &Instant::now)
}

fn recv_until_deadline_or_cancel_with_clock<T>(
    receiver: &mpsc::Receiver<T>,
    deadline: Instant,
    cancel: &AtomicBool,
    clock: &impl Fn() -> Instant,
) -> Result<T, OpenWaitError> {
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(OpenWaitError::Cancelled);
        }
        let remaining = deadline
            .checked_duration_since(clock())
            .ok_or(OpenWaitError::Timeout)?;
        let wait = remaining.min(Duration::from_millis(20));
        match receiver.recv_timeout(wait) {
            Ok(value) if !cancel.load(Ordering::Acquire) && clock() < deadline => return Ok(value),
            Ok(_) => {
                if cancel.load(Ordering::Acquire) {
                    return Err(OpenWaitError::Cancelled);
                }
                return Err(OpenWaitError::Timeout);
            }
            Err(mpsc::RecvTimeoutError::Timeout) if clock() >= deadline => {
                return Err(OpenWaitError::Timeout)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(OpenWaitError::Disconnected),
        }
    }
}

fn map_admission_error(error: AdmissionError) -> ReviewError {
    match error {
        AdmissionError::CaptureActive => playback_error(
            "PLAYBACK_CAPTURE_ACTIVE",
            "Playback is unavailable while capture is starting, active, or stopping.",
            true,
        ),
        AdmissionError::PlaybackBusy => {
            playback_error("PLAYBACK_BUSY", "Another playback session is open.", false)
        }
    }
}

fn safe_storage_error() -> ReviewError {
    playback_error(
        "PLAYBACK_IO_FAILED",
        "Playback source data could not be read.",
        true,
    )
}

fn parse_u64(row: &Value, key: &str) -> Result<u64, ReviewError> {
    row.get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .map(|value| value as u64)
        .ok_or_else(|| {
            playback_error(
                "INVALID_RECORDING_METADATA",
                "Recording metadata is invalid.",
                false,
            )
        })
}

fn parse_string(row: &Value, key: &str) -> Result<String, ReviewError> {
    row.get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            playback_error(
                "INVALID_RECORDING_METADATA",
                "Recording metadata is invalid.",
                false,
            )
        })
}

fn frames_for_ms(ms: u64, sample_rate: u32) -> u64 {
    (u128::from(ms) * u128::from(sample_rate) / 1_000) as u64
}

fn routed_duration_ms(frames: u64, sample_rate: u32) -> u64 {
    let ms = (u128::from(frames) * 1_000).div_ceil(u128::from(sample_rate.max(1)));
    u64::try_from(ms).unwrap_or(u64::MAX)
}

#[cfg(test)]
fn ms_for_frames(frames: u64, sample_rate: u32) -> u64 {
    (u128::from(frames) * 1_000 / u128::from(sample_rate.max(1))) as u64
}

fn within_millisecond_precision(
    actual_frames: u64,
    timestamped_duration_ms: u64,
    sample_rate: u32,
) -> bool {
    let rate = u128::from(sample_rate);
    let actual_duration_scaled = u128::from(actual_frames) * 1_000;
    let timestamped_duration_scaled = u128::from(timestamped_duration_ms) * rate;
    actual_duration_scaled.abs_diff(timestamped_duration_scaled) * 2 <= rate
}

fn reject_untrusted_path(raw: &str) -> Result<(), ReviewError> {
    if raw.is_empty() || raw.contains('\0') {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        ));
    }
    let normalized = raw.replace('\\', "/");
    if normalized.starts_with("//")
        || normalized.starts_with("\\\\")
        || normalized.starts_with("/dev/")
        || normalized.starts_with("\\\\?\\")
        || normalized.starts_with("\\\\.\\")
        || normalized.starts_with("/??/")
        || normalized.split('/').any(|part| part == "..")
    {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        ));
    }
    if normalized
        .split_once(':')
        .is_some_and(|(prefix, suffix)| prefix.len() > 1 && !suffix.is_empty())
    {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        ));
    }
    // A colon after a drive prefix is an alternate data stream on Windows.
    if normalized.as_bytes().get(1) == Some(&b':') {
        let has_absolute_drive_prefix =
            normalized.len() > 2 && matches!(normalized.as_bytes().get(2), Some(b'/'));
        if !has_absolute_drive_prefix || normalized[2..].contains(':') {
            return Err(playback_error(
                "PLAYBACK_PATH_DENIED",
                "The playback source path is not permitted.",
                false,
            ));
        }
    }
    Ok(())
}

pub(crate) fn normalized_path_inside(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    fn comparable(path: &Path) -> PathBuf {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

        if !path.is_absolute() {
            return path.to_path_buf();
        }

        let mut input = path.as_os_str().encode_wide().collect::<Vec<_>>();
        input.push(0);
        let mut buffer = vec![0u16; input.len()];
        let expanded = loop {
            let Ok(capacity) = u32::try_from(buffer.len()) else {
                break path.to_path_buf();
            };
            let length = unsafe { GetLongPathNameW(input.as_ptr(), buffer.as_mut_ptr(), capacity) };
            if length == 0 {
                break path.to_path_buf();
            }
            if (length as usize) < buffer.len() {
                buffer.truncate(length as usize);
                break PathBuf::from(OsString::from_wide(&buffer));
            }
            let Some(required) = (length as usize).checked_add(1) else {
                break path.to_path_buf();
            };
            if required <= buffer.len() {
                break path.to_path_buf();
            }
            buffer.resize(required, 0);
        };
        let value = expanded.to_string_lossy().into_owned();
        if let Some(value) = value.strip_prefix(r"\\?\") {
            PathBuf::from(value)
        } else {
            expanded
        }
    }

    #[cfg(not(windows))]
    fn comparable(path: &Path) -> PathBuf {
        path.to_path_buf()
    }

    comparable(path).starts_with(comparable(root))
}

#[cfg(windows)]
pub(crate) fn final_path_from_handle(
    file: &File,
    _candidate: &Path,
) -> Result<PathBuf, ReviewError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED,
    };

    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            FILE_NAME_NORMALIZED,
        )
    };
    if length == 0 || length as usize >= buffer.len() {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        ));
    }
    let value = String::from_utf16(&buffer[..length as usize]).map_err(|_| {
        playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        )
    })?;
    if value.starts_with(r"\\?\UNC\") || value.starts_with(r"\\.\") {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        ));
    }
    Ok(PathBuf::from(value))
}

#[cfg(not(windows))]
pub(crate) fn final_path_from_handle(
    file: &File,
    candidate: &Path,
) -> Result<PathBuf, ReviewError> {
    let _ = file;
    std::fs::canonicalize(candidate).map_err(|_| {
        playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        )
    })
}

pub(crate) fn resolve_ledger_path(project_root: &Path, raw: &str) -> Result<PathBuf, ReviewError> {
    reject_untrusted_path(raw)?;
    let raw_path = Path::new(raw);
    let candidate = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        project_root.join(raw_path)
    };
    Ok(candidate)
}

fn open_custodied_file(
    project_root: &Path,
    raw_path: &str,
    expected_size: u64,
) -> Result<Option<File>, ReviewError> {
    let candidate = resolve_ledger_path(project_root, raw_path)?;
    let file = match File::open(&candidate) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(safe_storage_error()),
    };
    let metadata = file.metadata().map_err(|_| safe_storage_error())?;
    if !metadata.is_file() || metadata.len() != expected_size {
        return Err(safe_storage_error());
    }
    let final_path = final_path_from_handle(&file, &candidate)?;
    if !normalized_path_inside(&final_path, project_root) {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        ));
    }
    Ok(Some(file))
}

fn validate_wave(
    project_root: &Path,
    descriptor: &ChunkDescriptor,
) -> Result<Option<ValidatedWave>, ReviewError> {
    let Some(file) = open_custodied_file(project_root, &descriptor.path, descriptor.byte_size)?
    else {
        return Ok(None);
    };
    let reader =
        hound::WavReader::new(BufReader::with_capacity(MAX_READ_BYTES, file)).map_err(|_| {
            playback_error(
                "PLAYBACK_FORMAT_UNSUPPORTED",
                "The playback source format is unsupported.",
                false,
            )
        })?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
        || !(spec.channels == 1 || spec.channels == 2)
        || !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&spec.sample_rate)
    {
        return Err(playback_error(
            "PLAYBACK_FORMAT_UNSUPPORTED",
            "The playback source format is unsupported.",
            false,
        ));
    }
    let frames = u64::from(reader.duration());
    if frames == 0 {
        return Err(playback_error(
            "PLAYBACK_FORMAT_UNSUPPORTED",
            "The playback source format is unsupported.",
            false,
        ));
    }
    Ok(Some(ValidatedWave { spec, frames }))
}

fn open_source(
    project_root: &Path,
    descriptor: &ChunkDescriptor,
) -> Result<Option<OpenSource>, ReviewError> {
    let Some(file) = open_custodied_file(project_root, &descriptor.path, descriptor.byte_size)?
    else {
        return Ok(None);
    };
    let reader =
        hound::WavReader::new(BufReader::with_capacity(MAX_READ_BYTES, file)).map_err(|_| {
            playback_error(
                "PLAYBACK_FORMAT_UNSUPPORTED",
                "The playback source format is unsupported.",
                false,
            )
        })?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
        || !(spec.channels == 1 || spec.channels == 2)
        || !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&spec.sample_rate)
    {
        return Err(playback_error(
            "PLAYBACK_FORMAT_UNSUPPORTED",
            "The playback source format is unsupported.",
            false,
        ));
    }
    let frames = u64::from(reader.duration());
    if frames == 0
        || !within_millisecond_precision(
            frames,
            descriptor.end_ms - descriptor.start_ms,
            spec.sample_rate,
        )
    {
        return Err(playback_error(
            "PLAYBACK_TIMELINE_INVALID",
            "The playback timeline is invalid.",
            false,
        ));
    }
    Ok(Some(OpenSource {
        reader,
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        frame_count: frames,
        next_frame: 0,
    }))
}

pub(crate) fn project_custody_root(
    data_root: &Path,
    storage_path: &str,
) -> Result<PathBuf, ReviewError> {
    reject_untrusted_path(storage_path)?;
    let allowed_projects_roots = crate::recording_output::known_roots_from_config(data_root)
        .into_iter()
        .filter_map(|root| std::fs::canonicalize(root.join("projects")).ok())
        .collect::<Vec<_>>();
    if allowed_projects_roots.is_empty() {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback project root is not permitted.",
            false,
        ));
    }
    let raw = Path::new(storage_path);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        data_root.join(raw)
    };
    let root = std::fs::canonicalize(candidate).map_err(|_| {
        playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback project root is not permitted.",
            false,
        )
    })?;
    if !root.is_dir()
        || !allowed_projects_roots
            .iter()
            .any(|projects_root| normalized_path_inside(&root, projects_root))
    {
        return Err(playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback project root is not permitted.",
            false,
        ));
    }
    Ok(root)
}

fn project_root_for(
    storage: &genesis_block_native::Storage,
    data_root: &Path,
    project_id: &str,
) -> Result<PathBuf, ReviewError> {
    let rows = genesis_adapter::query(
        storage,
        "projects",
        &["storage_path"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .map_err(|_| safe_storage_error())?;
    let row = rows
        .first()
        .ok_or_else(|| playback_error("PROJECT_NOT_FOUND", "The project was not found.", false))?;
    let path = parse_string(row, "projects.storage_path")?;
    project_custody_root(data_root, &path)
}

fn prepare_routed_playback(
    storage: Arc<genesis_block_native::Storage>,
    data_root: &Path,
    row: &RecordingStorageRow,
    source_id: &str,
    epoch: i64,
    cancel: Arc<AtomicBool>,
) -> Result<PreparedPlayback, ReviewError> {
    check_open_cancelled(&cancel)?;
    let lease = crate::source_playback_lease::SourcePlaybackLease::register(
        Arc::clone(&storage),
        &row.identity.recording_id,
    )
    .map_err(|_| {
        playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        )
    })?;
    let manifest = Arc::new(
        crate::completed_source_media::completed_manifest(
            &storage,
            &row.identity.project_id,
            &row.identity.recording_id,
        )
        .map_err(|code| playback_error(&code, "Source playback is unavailable.", false))?,
    );
    if manifest.project_id != row.identity.project_id
        || manifest.recording_id != row.identity.recording_id
    {
        return Err(playback_error(
            "PLAYBACK_SOURCE_INVALID",
            "Source identity is inconsistent.",
            false,
        ));
    }
    let source = manifest
        .sources
        .iter()
        .find(|s| s.id == source_id && s.epoch == epoch)
        .ok_or_else(|| {
            playback_error("PLAYBACK_SOURCE_INVALID", "Source is unavailable.", false)
        })?;
    let bytes = source
        .chunks
        .len()
        .checked_mul(std::mem::size_of::<ChunkDescriptor>())
        .and_then(|n| n.checked_add(manifest.metadata_bytes))
        .ok_or_else(|| {
            playback_error(
                "RESOURCE_LIMIT",
                "Source metadata exceeds the limit.",
                false,
            )
        })?;
    if bytes > MAX_PLAYER_METADATA_BYTES || source.chunks.len() > MAX_CHUNK_DESCRIPTORS {
        return Err(playback_error(
            "RESOURCE_LIMIT",
            "Source metadata exceeds the limit.",
            false,
        ));
    }
    let sample_rate = source.sample_rate;
    let mut descriptors = Vec::with_capacity(source.chunks.len());
    for c in &source.chunks {
        check_open_cancelled(&cancel)?;
        let start = u64::try_from(c.chunk.start_frame).map_err(|_| safe_storage_error())?;
        let end = u64::try_from(c.chunk.end_frame).map_err(|_| safe_storage_error())?;
        descriptors.push(ChunkDescriptor {
            id: String::new(),
            path: String::new(),
            sequence_no: c.chunk.sequence,
            start_ms: start.saturating_mul(1000) / u64::from(sample_rate),
            end_ms: end.saturating_mul(1000) / u64::from(sample_rate),
            byte_size: c.byte_size,
            frame_start: start,
            frame_end: end,
            available: true,
        });
    }
    // The open path clones PreparedPlayback into the worker before installation.
    // Charge both descriptor vectors and reserve room for session/snapshot IDs,
    // control blocks and bounded bookkeeping; SDK allocator peak is separate.
    let retained = manifest
        .metadata_bytes
        .checked_add(
            descriptors
                .capacity()
                .saturating_mul(std::mem::size_of::<ChunkDescriptor>())
                .saturating_mul(2),
        )
        .and_then(|n| n.checked_add(std::mem::size_of::<PreparedPlayback>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<PreparedPlayback>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<RoutedPlaybackSource>()))
        .and_then(|n| n.checked_add(row.identity.project_id.len().saturating_mul(2)))
        .and_then(|n| n.checked_add(row.identity.recording_id.len().saturating_mul(2)))
        .and_then(|n| n.checked_add(source_id.len()))
        .and_then(|n| n.checked_add(data_root.as_os_str().len()))
        .and_then(|n| n.checked_add("source".len() * 2))
        .and_then(|n| n.checked_add(64 * 1024))
        .ok_or_else(|| {
            playback_error(
                "RESOURCE_LIMIT",
                "Source metadata exceeds the limit.",
                false,
            )
        })?;
    if retained > MAX_PLAYER_METADATA_BYTES {
        return Err(playback_error(
            "RESOURCE_LIMIT",
            "Source metadata exceeds the limit.",
            false,
        ));
    }
    let duration_frames = descriptors.last().map(|d| d.frame_end).ok_or_else(|| {
        playback_error("PLAYBACK_SOURCE_MISSING", "Source is unavailable.", false)
    })?;
    let _gate = lease.try_output().ok_or_else(|| {
        playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        )
    })?;
    lease.check_frontier(manifest.frontier).map_err(|_| {
        playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        )
    })?;
    Ok(PreparedPlayback {
        project_root: PathBuf::new(),
        project_id: row.identity.project_id.clone(),
        recording_id: row.identity.recording_id.clone(),
        channel: "source".into(),
        duration_ms: routed_duration_ms(duration_frames, sample_rate),
        sample_rate,
        source_channels: 1,
        duration_frames,
        descriptors,
        degraded: false,
        missing_ranges: Vec::new(),
        routed: Some(Arc::new(RoutedPlaybackSource {
            storage,
            data_root: data_root.to_path_buf(),
            manifest: Arc::clone(&manifest),
            source_id: source_id.into(),
            epoch,
            cancel,
            lease: Arc::clone(&lease),
        })),
    })
}

#[cfg(all(test, windows))]
mod routed_source_tests {
    use super::*;
    use crate::{capture_source_custody as custody, completed_source_media::tests::Fixture};

    #[test]
    fn source_open_factory_preserves_explicit_identity_and_exact_frame_tail() {
        let f = Fixture::new(true);
        let row = recording_review::validate_recording_pair(&f.storage, &f.project, &f.recording)
            .unwrap();
        let io = OpenPlaybackIo::production_source(
            Arc::clone(&f.storage),
            f.temp.path().to_path_buf(),
            row,
            f.regs[1].id.clone(),
            f.regs[1].epoch,
        )
        .unwrap();
        let prepared = (io.prepare)(Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(prepared.channel, "source");
        assert_eq!(prepared.duration_frames, 1);
        let identity = source_identity_for_prepared(&prepared).unwrap().unwrap();
        assert_eq!(identity.source_id, f.regs[1].id);
        assert_eq!(identity.capture_epoch, f.regs[1].epoch as u64);
        let json = serde_json::to_value(identity).unwrap();
        assert_eq!(json["sourceId"], f.regs[1].id);
        assert_eq!(json["captureEpoch"], f.regs[1].epoch);
    }

    #[test]
    fn source_open_factory_denies_bad_epoch_and_unknown_source() {
        let f = Fixture::new(true);
        let row = recording_review::validate_recording_pair(&f.storage, &f.project, &f.recording)
            .unwrap();
        assert!(OpenPlaybackIo::production_source(
            Arc::clone(&f.storage),
            f.temp.path().to_path_buf(),
            row.clone(),
            f.regs[0].id.clone(),
            0
        )
        .is_err());
        let io = OpenPlaybackIo::production_source(
            Arc::clone(&f.storage),
            f.temp.path().to_path_buf(),
            row,
            uuid::Uuid::new_v4().to_string(),
            1,
        )
        .unwrap();
        assert!((io.prepare)(Arc::new(AtomicBool::new(false))).is_err());
    }

    #[test]
    fn dropped_unpolled_routed_close_retains_owner_until_join() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let (tx, rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            assert!(matches!(rx.recv(), Ok(WorkerCommand::Close)));
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx: tx,
            join: Some(WorkerJoin::Direct(worker)),
            guard: Arc::clone(&guard),
        };
        let manager = PlaybackManager::new();
        manager.install_test(session);
        drop(dispatch_owned_close(manager.clone(), 3, "routed"));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(routed.cancel.load(Ordering::Acquire));
        assert!(guard.playback_open());
        assert!(manager.begin_open().is_err());
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while guard.playback_open() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!guard.playback_open());
    }

    #[test]
    fn close_cancels_routed_verification_before_join() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (observed_tx, observed_rx) = mpsc::channel();
        let cancel = Arc::clone(&routed.cancel);
        let worker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                thread::yield_now();
            }
            observed_tx.send(cancel.load(Ordering::Acquire)).unwrap();
        });
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: Some(WorkerJoin::Direct(worker)),
            guard: test_held_guard(),
        };
        started_rx.recv().unwrap();
        cleanup_session(session).unwrap();
        assert!(
            observed_rx.recv().unwrap(),
            "close must signal cancellation before join"
        );
        assert!(
            crate::source_playback_lease::SourcePlaybackLease::register(
                Arc::clone(&f.storage),
                &f.recording
            )
            .is_ok(),
            "close must not revoke the recording"
        );
    }

    #[test]
    fn cancelled_routed_callback_suppresses_queued_frames() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        let (control, mut writer, mut reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control.clone()).unwrap();
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        assert_eq!(
            writer.push(control.current_epoch(), [123, 456]),
            crate::routed_playback_ring::Push::Published
        );
        routed.cancel.store(true, Ordering::Release);
        let mut output = [1_i16; 2];
        render_routed(&mut output, &runtime, routed, &mut reader, 1);
        assert_eq!(output, [0, 0]);
        assert_eq!(runtime.position_frames.load(Ordering::Acquire), 0);
    }

    #[test]
    fn routed_close_fences_inflight_producer_without_manager_lock() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: None,
            guard: test_held_guard(),
        };
        let producer = routed.lease.producer_output().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let cleanup = thread::spawn(move || {
            cleanup_session(session).unwrap();
            done_tx.send(()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !routed.cancel.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(routed.cancel.load(Ordering::Acquire));
        assert!(
            done_rx.try_recv().is_err(),
            "close must fence in-flight publication"
        );
        drop(producer);
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        cleanup.join().unwrap();
    }

    #[test]
    fn routed_close_waits_for_inflight_output_callback() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: None,
            guard: test_held_guard(),
        };
        let callback = routed.lease.try_output().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let cleanup = thread::spawn(move || {
            cleanup_session(session).unwrap();
            done_tx.send(()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !routed.cancel.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(routed.cancel.load(Ordering::Acquire));
        assert!(
            done_rx.try_recv().is_err(),
            "close must fence the in-flight callback"
        );
        drop(callback);
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        cleanup.join().unwrap();
    }

    #[test]
    fn routed_close_does_not_acknowledge_a_panicked_worker() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: Some(WorkerJoin::Direct(thread::spawn(|| {
                panic!("simulated worker panic")
            }))),
            guard: Arc::clone(&guard),
        };
        let manager = PlaybackManager::new();
        spawn_session_cleanup(manager.clone(), session)
            .unwrap()
            .join()
            .unwrap();
        assert!(
            manager.take_for_close(3, "routed").is_err(),
            "failed cleanup must not be marked closed"
        );
        assert!(
            guard.try_open_playback().is_err(),
            "failed join must keep capture admission closed"
        );
    }

    #[test]
    fn routed_close_fences_output_during_concurrent_revocation() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: None,
            guard: test_held_guard(),
        };
        let callback = routed.lease.try_output().unwrap();
        let storage = Arc::clone(&f.storage);
        let recording = f.recording.clone();
        let revoker = thread::spawn(move || {
            crate::source_playback_lease::revoke_recording(&storage, &recording)
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while routed.lease.is_available() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!routed.lease.is_available());
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let cleanup = thread::spawn(move || {
            started_tx.send(()).unwrap();
            cleanup_session(session).unwrap();
            done_tx.send(()).unwrap();
        });
        started_rx.recv().unwrap();
        let premature = done_rx.recv_timeout(Duration::from_millis(250)).is_ok();
        drop(callback);
        revoker.join().unwrap();
        cleanup.join().unwrap();
        assert!(
            !premature,
            "close acknowledged while a revoked output callback was in flight"
        );
    }

    #[test]
    fn routed_close_fences_producer_during_concurrent_revocation() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: None,
            guard: test_held_guard(),
        };
        let producer = routed.lease.producer_output().unwrap();
        let storage = Arc::clone(&f.storage);
        let recording = f.recording.clone();
        let revoker = thread::spawn(move || {
            crate::source_playback_lease::revoke_recording(&storage, &recording)
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while routed.lease.is_available() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!routed.lease.is_available());
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let cleanup = thread::spawn(move || {
            started_tx.send(()).unwrap();
            cleanup_session(session).unwrap();
            done_tx.send(()).unwrap();
        });
        started_rx.recv().unwrap();
        let premature = done_rx.recv_timeout(Duration::from_millis(250)).is_ok();
        drop(producer);
        revoker.join().unwrap();
        cleanup.join().unwrap();
        assert!(
            !premature,
            "close acknowledged while revoked publication was in flight"
        );
    }

    #[test]
    fn routed_close_signals_cancel_before_future_is_polled() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let session = PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime,
            command_tx,
            join: None,
            guard: test_held_guard(),
        };
        drop(dispatch_close_playback(PlaybackManager::new(), session));
        assert!(
            routed.cancel.load(Ordering::Acquire),
            "dropping an unpolled future must suppress further output"
        );
    }

    #[test]
    fn routed_callback_reads_current_frames_without_legacy_queue_lock() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        let (control, mut writer, mut reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control.clone()).unwrap();
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        assert_eq!(
            writer.push(control.current_epoch(), [123, 456]),
            crate::routed_playback_ring::Push::Published
        );
        let _legacy_queue = runtime.queue.lock().unwrap();
        let mut output = [0_i16; 2];
        render_routed(&mut output, &runtime, routed, &mut reader, 1);
        assert_eq!(output, [123, 456]);
        assert_eq!(runtime.position_frames.load(Ordering::Acquire), 1);
    }

    #[test]
    fn routed_callback_drains_stale_frames_while_paused() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        let (control, mut writer, mut reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control.clone()).unwrap();
        let old = control.current_epoch();
        assert_eq!(
            writer.push(old, [123, 456]),
            crate::routed_playback_ring::Push::Published
        );
        let new_epoch = control.advance_epoch().unwrap();
        assert_eq!(
            writer.push(new_epoch, [7, 8]),
            crate::routed_playback_ring::Push::Published
        );
        let mut output = [1_i16; 2];
        render_routed(&mut output, &runtime, routed, &mut reader, 2);
        assert_eq!(output, [0, 0]);
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        render_routed(&mut output, &runtime, routed, &mut reader, 2);
        assert_eq!(output, [7, 8]);
    }

    #[test]
    fn routed_health_reports_device_loss_while_paused_and_persistent_underrun() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let mut runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        runtime.device_lost.store(true, Ordering::Release);
        assert_eq!(
            routed_worker_health(&runtime).unwrap().code,
            "PLAYBACK_DEVICE_LOST"
        );
        runtime.device_lost.store(false, Ordering::Release);
        runtime.started_at = Instant::now() - Duration::from_secs(4);
        runtime.underrun_started_ms.store(1, Ordering::Release);
        assert!(
            routed_worker_health(&runtime).is_none(),
            "a paused player must not report an underrun"
        );
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        assert_eq!(
            routed_worker_health(&runtime).unwrap().code,
            "PLAYBACK_BUFFER_UNDERRUN"
        );
    }

    #[test]
    fn old_generation_error_does_not_override_new_seek() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        let fence = routed.lease.output_fence().unwrap();
        let lease = Arc::clone(&routed.lease);
        let worker_runtime = Arc::clone(&runtime);
        let (started_tx, started_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            publish_routed_generation_error(
                &worker_runtime,
                &lease,
                0,
                playback_error("PLAYBACK_IO_FAILED", "old generation failed", true),
            )
        });
        started_rx.recv().unwrap();
        runtime.stream_epoch.store(1, Ordering::Release);
        runtime.state.store(STATE_PAUSED, Ordering::Release);
        drop(fence);
        assert!(!worker.join().unwrap());
        assert_eq!(runtime.state.load(Ordering::Acquire), STATE_PAUSED);
        assert!(runtime.error.lock().unwrap().is_none());
    }

    #[test]
    fn stale_underrun_is_rechecked_after_pause_fence() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let mut new_runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        new_runtime.started_at = Instant::now() - Duration::from_secs(4);
        let runtime = Arc::new(new_runtime);
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        runtime.underrun_started_ms.store(1, Ordering::Release);
        assert!(routed_worker_health(&runtime).is_some());
        let fence = routed.lease.output_fence().unwrap();
        let lease = Arc::clone(&routed.lease);
        let worker_runtime = Arc::clone(&runtime);
        let (started_tx, started_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            publish_routed_health(&worker_runtime, &lease)
        });
        started_rx.recv().unwrap();
        runtime.state.store(STATE_PAUSED, Ordering::Release);
        runtime.underrun_started_ms.store(0, Ordering::Release);
        drop(fence);
        assert!(!worker.join().unwrap());
        assert_eq!(runtime.state.load(Ordering::Acquire), STATE_PAUSED);
        assert!(runtime.error.lock().unwrap().is_none());
    }

    #[test]
    fn routed_pause_fences_callback_without_holding_manager_lock() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        let (control, _writer, _reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control.clone()).unwrap();
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        let (command_tx, command_rx) = mpsc::channel();
        let manager = PlaybackManager::new();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: 1,
            duration_frames: 8,
            sample_rate: 8000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        let callback = routed.lease.try_output().unwrap();
        let other = manager.clone();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            tx.send(other.control(3, "routed", 0, "pause", None))
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while manager.lifecycle.routed_controls.try_lock().is_ok() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(
            manager.lifecycle.routed_controls.try_lock().is_err(),
            "control did not reach the callback fence"
        );
        assert!(rx.try_recv().is_err());
        assert!(
            manager.inner.try_lock().is_ok(),
            "must not hold manager while waiting for callback"
        );
        drop(callback);
        let snapshot = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        worker.join().unwrap();
        assert_eq!(snapshot.runtime.stream_epoch.load(Ordering::Acquire), 1);
        assert_eq!(control.current_epoch(), 2);
        assert!(matches!(
            command_rx.recv().unwrap(),
            WorkerCommand::RoutedPause { epoch: 1, frame: 0 }
        ));
        assert!(
            matches!(manager.control(3, "routed", 0, "play", None), Err(error) if error.code == "PLAYBACK_STALE_EPOCH")
        );
    }

    #[test]
    fn routed_seek_then_pause_keeps_only_latest_generation_actionable() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        let (control, mut writer, mut reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control.clone()).unwrap();
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        let (command_tx, command_rx) = mpsc::channel();
        let manager = PlaybackManager::new();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: 1,
            duration_frames: 8,
            sample_rate: 8000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        let old = control.current_epoch();
        assert_eq!(
            writer.push(old, [100, 100]),
            crate::routed_playback_ring::Push::Published
        );
        manager.control(3, "routed", 0, "seek", Some(0)).unwrap();
        let seek_ring_epoch = control.current_epoch();
        assert_eq!(
            writer.push(seek_ring_epoch, [200, 200]),
            crate::routed_playback_ring::Push::Published
        );
        manager.control(3, "routed", 1, "pause", None).unwrap();
        assert!(matches!(
            command_rx.recv().unwrap(),
            WorkerCommand::RoutedSeek { epoch: 1, .. }
        ));
        assert!(matches!(
            command_rx.recv().unwrap(),
            WorkerCommand::RoutedPause { epoch: 2, .. }
        ));
        assert_eq!(runtime.stream_epoch.load(Ordering::Acquire), 2);
        let mut output = [1_i16; 2];
        render_routed(&mut output, &runtime, routed, &mut reader, 8);
        assert_eq!(output, [0, 0]);
        assert_eq!(
            reader.poll(false, 4),
            crate::routed_playback_ring::Poll::Empty
        );
    }

    #[test]
    fn routed_controls_cannot_revive_terminal_error_before_worker_drop() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        let (control, _writer, _reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control).unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let manager = PlaybackManager::new();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: 1,
            duration_frames: 8,
            sample_rate: 8000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        runtime.set_error(playback_error("PLAYBACK_DEVICE_LOST", "device lost", true));
        for (action, position) in [("play", None), ("pause", None), ("seek", Some(0))] {
            let result = manager.control(3, "routed", 0, action, position);
            assert!(matches!(result, Err(error) if error.code == "PLAYBACK_DEVICE_LOST"));
            assert_eq!(runtime.state.load(Ordering::Acquire), STATE_ERROR);
        }
        assert!(
            command_rx.try_recv().is_err(),
            "terminal session must not receive new commands"
        );
    }

    #[test]
    fn routed_transition_cannot_overwrite_concurrent_terminal_error() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        let outcome = transition_routed_state_with_hook(&runtime, STATE_PLAYING, || {
            runtime.set_error(playback_error("PLAYBACK_SOURCE_REVOKED", "revoked", false));
        });
        assert!(matches!(outcome, Err(error) if error.code == "PLAYBACK_SOURCE_REVOKED"));
        assert_eq!(runtime.state.load(Ordering::Acquire), STATE_ERROR);
    }

    #[test]
    fn routed_final_check_rejects_revocation_after_successful_transition() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = PlaybackRuntime::new_routed(false, vec![], Arc::clone(&routed.lease));
        let fence = routed.lease.output_fence().unwrap();
        transition_routed_state(&runtime, STATE_PLAYING).unwrap();
        let storage = Arc::clone(&f.storage);
        let recording = f.recording.clone();
        let worker = thread::spawn(move || {
            crate::source_playback_lease::revoke_recording(&storage, &recording)
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while routed.lease.is_available() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!routed.lease.is_available());
        assert!(
            matches!(finalize_routed_control(&runtime, &routed.lease), Err(error) if error.code == "PLAYBACK_SOURCE_REVOKED")
        );
        drop(fence);
        worker.join().unwrap();
    }

    #[test]
    fn routed_failed_seek_send_is_terminal_without_claiming_rollback() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        let (control, _writer, _reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control.clone()).unwrap();
        runtime.state.store(STATE_PLAYING, Ordering::Release);
        let (command_tx, command_rx) = mpsc::channel();
        drop(command_rx);
        let manager = PlaybackManager::new();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: 1,
            duration_frames: 8,
            sample_rate: 8000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        assert!(
            matches!(manager.control(3, "routed", 0, "seek", Some(0)), Err(error) if error.code == "PLAYBACK_HANDLE_INVALID")
        );
        assert_eq!(runtime.state.load(Ordering::Acquire), STATE_ERROR);
        assert_eq!(runtime.stream_epoch.load(Ordering::Acquire), 1);
        assert_eq!(control.current_epoch(), 2);
    }

    fn prepare(f: &Fixture) -> PreparedPlayback {
        let row = RecordingStorageRow {
            identity: recording_review::RecordingIdentity {
                project_id: f.project.clone(),
                recording_id: f.recording.clone(),
            },
            source: "routed_endpoint".into(),
            status: "completed".into(),
            duration_ms: 0,
            created_at: "2026-10-29T00:00:00Z".into(),
            updated_at: "2026-10-29T00:00:00Z".into(),
            language: None,
        };
        prepare_routed_playback(
            Arc::clone(&f.storage),
            f.temp.path(),
            &row,
            &f.regs[0].id,
            f.regs[0].epoch,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    }

    #[test]
    fn routed_fractional_duration_seeks_start_and_exact_eof() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        assert_eq!(prepared.duration_frames, 1);
        assert_eq!(prepared.duration_ms, 1, "one frame remains seekable");
        let routed = prepared.routed.as_ref().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new_routed(
            false,
            vec![],
            Arc::clone(&routed.lease),
        ));
        let (control, _writer, _reader) = crate::routed_playback_ring::channel(4).unwrap();
        runtime.routed_control.set(control).unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let manager = PlaybackManager::new();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 3,
            handle: "routed".into(),
            project_id: f.project.clone(),
            recording_id: f.recording.clone(),
            channel: "source".into(),
            duration_ms: prepared.duration_ms,
            duration_frames: prepared.duration_frames,
            sample_rate: prepared.sample_rate,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        manager.control(3, "routed", 0, "seek", Some(0)).unwrap();
        assert_eq!(runtime.position_frames.load(Ordering::Acquire), 0);
        assert_eq!(runtime.state.load(Ordering::Acquire), STATE_PAUSED);
        assert!(matches!(
            command_rx.recv().unwrap(),
            WorkerCommand::RoutedSeek { epoch: 1, frame: 0 }
        ));
        let end = manager
            .control(3, "routed", 1, "seek", Some(prepared.duration_ms))
            .unwrap();
        assert_eq!(runtime.position_frames.load(Ordering::Acquire), 1);
        assert_eq!(runtime.state.load(Ordering::Acquire), STATE_ENDED);
        assert_eq!(public_state(&end).position_ms, prepared.duration_ms);
        assert!(matches!(
            command_rx.recv().unwrap(),
            WorkerCommand::RoutedSeek { epoch: 2, frame: 1 }
        ));
    }

    #[test]
    fn routed_duration_rounds_up_only_fractional_milliseconds() {
        assert_eq!(routed_duration_ms(0, 44_100), 0);
        assert_eq!(routed_duration_ms(1, 44_100), 1);
        assert_eq!(routed_duration_ms(44_100, 44_100), 1_000);
        assert_eq!(routed_duration_ms(44_101, 44_100), 1_001);
    }

    #[test]
    fn routed_player_queues_exact_selected_source_and_sub_ms_tail() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        assert_eq!(prepared.duration_frames, 1);
        assert_eq!(prepared.duration_ms, 1);
        assert_eq!(prepared.channel, "source");
        let runtime = PlaybackRuntime::new(false, vec![]);
        let (mut cursor, mut index, mut reader) = (0, None, None);
        fill_queue(&prepared, &runtime, &mut cursor, &mut index, &mut reader).unwrap();
        assert_eq!(cursor, 1);
        assert_eq!(
            runtime
                .queue
                .lock()
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![111, 111]
        );
        assert!(reader.is_none(), "EOF closes the verified source handle");
    }

    #[test]
    fn routed_player_denies_queued_refill_after_aggregate_quarantine() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let a = custody::begin_source_commit(
            &f.storage,
            &uuid::Uuid::new_v4().to_string(),
            "2026-10-29T00:00:00Z",
        )
        .unwrap();
        custody::quarantine_capture_source(&f.storage, &a, &f.regs[1].id, "late-contamination")
            .unwrap();
        let runtime = PlaybackRuntime::new(false, vec![]);
        let (mut cursor, mut index, mut reader) = (0, None, None);
        let error =
            fill_queue(&prepared, &runtime, &mut cursor, &mut index, &mut reader).unwrap_err();
        assert_eq!(error.code, "PLAYBACK_SOURCE_REVOKED");
        assert!(runtime.queue.lock().unwrap().is_empty());
        assert_eq!(cursor, 0);
    }

    #[test]
    fn ordinary_callback_gate_contention_does_not_revoke_source() {
        let f = Fixture::new(true);
        let prepared = prepare(&f);
        let lease = Arc::clone(&prepared.routed.as_ref().unwrap().lease);
        let gate = lease.try_output().unwrap();
        let runtime = Arc::new(PlaybackRuntime::new(false, vec![]));
        let worker_runtime = Arc::clone(&runtime);
        let worker = thread::spawn(move || {
            let (mut cursor, mut index, mut reader) = (0, None, None);
            fill_queue(
                &prepared,
                &worker_runtime,
                &mut cursor,
                &mut index,
                &mut reader,
            )
            .map(|_| cursor)
        });
        thread::sleep(Duration::from_millis(30));
        drop(gate);
        assert_eq!(worker.join().unwrap().unwrap(), 1);
        assert_eq!(
            runtime
                .queue
                .lock()
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![111, 111]
        );
        assert!(lease.is_available());
    }
}

fn prepare_playback(
    storage: &genesis_block_native::Storage,
    data_root: &Path,
    row: &RecordingStorageRow,
    channel: String,
    cancel: Arc<AtomicBool>,
) -> Result<PreparedPlayback, ReviewError> {
    check_open_cancelled(&cancel)?;
    if !matches!(channel.as_str(), "mic" | "system" | "file") {
        return Err(ReviewError::invalid("channel is invalid."));
    }
    crate::routed_media_access::require_legacy_media_source(storage, &row.identity.recording_id)
        .map_err(|_| {
            playback_error(
                "PLAYBACK_SOURCE_UNSUPPORTED",
                "This recording requires source-aware playback.",
                false,
            )
        })?;
    let project_root = project_root_for(storage, data_root, &row.identity.project_id)?;
    check_open_cancelled(&cancel)?;
    let rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &[
            "id",
            "recording_id",
            "sequence_no",
            "file_path",
            "start_ms",
            "end_ms",
            "byte_size",
        ],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(&row.identity.recording_id),
        )],
    )
    .map_err(|_| safe_storage_error())?;
    check_open_cancelled(&cancel)?;
    if rows.len() > MAX_CHUNK_DESCRIPTORS {
        return Err(ReviewError::resource_limit());
    }
    let metadata_bytes = rows
        .iter()
        .map(|item| {
            serde_json::to_vec(item)
                .map(|bytes| bytes.len())
                .unwrap_or(MAX_PLAYER_METADATA_BYTES)
        })
        .try_fold(0usize, |total, next| total.checked_add(next))
        .ok_or_else(ReviewError::resource_limit)?;
    if metadata_bytes > MAX_PLAYER_METADATA_BYTES {
        return Err(ReviewError::resource_limit());
    }

    let mut descriptors = Vec::new();
    for item in rows {
        let path = parse_string(&item, "audio_chunks.file_path")?;
        if recording_review::channel_for_path(&path, &row.source) != Some(channel.as_str()) {
            continue;
        }
        let start_ms = parse_u64(&item, "audio_chunks.start_ms")?;
        let end_ms = parse_u64(&item, "audio_chunks.end_ms")?;
        let byte_size = parse_u64(&item, "audio_chunks.byte_size")?;
        if end_ms <= start_ms || end_ms > row.duration_ms.max(0) as u64 {
            return Err(playback_error(
                "PLAYBACK_TIMELINE_INVALID",
                "The playback timeline is invalid.",
                false,
            ));
        }
        descriptors.push(ChunkDescriptor {
            id: parse_string(&item, "audio_chunks.id")?,
            path,
            sequence_no: parse_u64(&item, "audio_chunks.sequence_no")? as i64,
            start_ms,
            end_ms,
            byte_size,
            frame_start: 0,
            frame_end: 0,
            available: false,
        });
    }
    if descriptors.is_empty() {
        return Err(playback_error(
            "PLAYBACK_SOURCE_MISSING",
            "No source audio is available for this channel.",
            false,
        ));
    }
    descriptors.sort_by(|left, right| {
        left.start_ms
            .cmp(&right.start_ms)
            .then_with(|| left.end_ms.cmp(&right.end_ms))
            .then_with(|| left.sequence_no.cmp(&right.sequence_no))
            .then_with(|| left.id.cmp(&right.id))
    });
    for pair in descriptors.windows(2) {
        if pair[1].start_ms < pair[0].end_ms {
            return Err(playback_error(
                "PLAYBACK_TIMELINE_INVALID",
                "The playback timeline is invalid.",
                false,
            ));
        }
    }

    let mut sample_rate = None;
    let mut source_channels = None;
    let mut missing_ranges = Vec::new();
    let mut available_count = 0usize;
    for descriptor in &mut descriptors {
        check_open_cancelled(&cancel)?;
        let wave = validate_wave(&project_root, descriptor)?;
        if let Some(wave) = wave {
            let rate = wave.spec.sample_rate;
            let channels = wave.spec.channels;
            if !within_millisecond_precision(
                wave.frames,
                descriptor.end_ms - descriptor.start_ms,
                rate,
            ) {
                return Err(playback_error(
                    "PLAYBACK_TIMELINE_INVALID",
                    "The playback timeline is invalid.",
                    false,
                ));
            }
            if sample_rate.is_some_and(|value| value != rate)
                || source_channels.is_some_and(|value| value != channels)
            {
                return Err(playback_error(
                    "PLAYBACK_FORMAT_UNSUPPORTED",
                    "The playback source formats do not match.",
                    false,
                ));
            }
            sample_rate = Some(rate);
            source_channels = Some(channels);
            descriptor.frame_start = frames_for_ms(descriptor.start_ms, rate);
            descriptor.frame_end = frames_for_ms(descriptor.end_ms, rate);
            descriptor.available = true;
            available_count += 1;
        } else {
            missing_ranges.push(MissingRange {
                start_ms: descriptor.start_ms,
                end_ms: descriptor.end_ms,
                reason: "missing_chunk".to_string(),
            });
        }
    }
    check_open_cancelled(&cancel)?;
    let Some(sample_rate) = sample_rate else {
        return Err(playback_error(
            "PLAYBACK_SOURCE_MISSING",
            "No source audio is available for this channel.",
            false,
        ));
    };
    let source_channels = source_channels.unwrap_or(0);
    for descriptor in &mut descriptors {
        if !descriptor.available {
            descriptor.frame_start = frames_for_ms(descriptor.start_ms, sample_rate);
            descriptor.frame_end = frames_for_ms(descriptor.end_ms, sample_rate);
        }
    }
    let duration_ms = row.duration_ms as u64;
    let duration_frames = frames_for_ms(duration_ms, sample_rate);
    if descriptors
        .last()
        .is_some_and(|descriptor| descriptor.frame_end > duration_frames + 1)
    {
        return Err(playback_error(
            "PLAYBACK_TIMELINE_INVALID",
            "The playback timeline is invalid.",
            false,
        ));
    }
    for pair in descriptors.windows(2) {
        if pair[1].start_ms > pair[0].end_ms {
            missing_ranges.push(MissingRange {
                start_ms: pair[0].end_ms,
                end_ms: pair[1].start_ms,
                reason: "timeline_gap".to_string(),
            });
        }
    }
    if let Some(first) = descriptors.first() {
        if first.start_ms > 0 {
            missing_ranges.push(MissingRange {
                start_ms: 0,
                end_ms: first.start_ms,
                reason: "timeline_gap".to_string(),
            });
        }
    }
    if let Some(last) = descriptors.last() {
        if last.end_ms < duration_ms {
            missing_ranges.push(MissingRange {
                start_ms: last.end_ms,
                end_ms: duration_ms,
                reason: "timeline_gap".to_string(),
            });
        }
    }
    missing_ranges.sort_by_key(|range| (range.start_ms, range.end_ms));
    check_open_cancelled(&cancel)?;
    Ok(PreparedPlayback {
        project_root,
        project_id: row.identity.project_id.clone(),
        recording_id: row.identity.recording_id.clone(),
        channel,
        duration_ms,
        sample_rate,
        source_channels,
        duration_frames,
        descriptors,
        degraded: !missing_ranges.is_empty() || available_count < 1,
        missing_ranges,
        routed: None,
    })
}

fn output_rate_for_range(source_rate: u32, minimum: u32, maximum: u32) -> Option<u32> {
    if minimum == 0 || minimum > maximum {
        return None;
    }
    if (minimum..=maximum).contains(&source_rate) {
        return Some(source_rate);
    }
    COMMON_OUTPUT_SAMPLE_RATES
        .into_iter()
        .chain([minimum, maximum])
        .filter(|rate| (minimum..=maximum).contains(rate))
        .min_by_key(|rate| rate.abs_diff(source_rate))
}

fn output_sample_format_priority(format: SampleFormat) -> Option<u8> {
    match format {
        SampleFormat::F32 => Some(0),
        SampleFormat::I16 => Some(1),
        SampleFormat::U16 => Some(2),
        _ => None,
    }
}

fn supported_output_config(
    device: &cpal::Device,
    source_rate: u32,
) -> Result<(StreamConfig, SampleFormat, u32), ReviewError> {
    let configs = device.supported_output_configs().map_err(|_| {
        playback_error(
            "PLAYBACK_OUTPUT_UNSUPPORTED",
            "The output device has no compatible stereo format.",
            false,
        )
    })?;
    let mut selected: Option<(u32, u8, StreamConfig, SampleFormat, u32)> = None;
    for config in configs {
        if config.channels() != OUTPUT_CHANNELS as u16 {
            continue;
        }
        let format = config.sample_format();
        let Some(format_priority) = output_sample_format_priority(format) else {
            continue;
        };
        let Some(output_rate) = output_rate_for_range(
            source_rate,
            config.min_sample_rate().0,
            config.max_sample_rate().0,
        ) else {
            continue;
        };
        let rate_distance = output_rate.abs_diff(source_rate);
        let is_better = selected
            .as_ref()
            .is_none_or(|(best_distance, best_priority, _, _, _)| {
                (rate_distance, format_priority) < (*best_distance, *best_priority)
            });
        if is_better {
            selected = Some((
                rate_distance,
                format_priority,
                config
                    .with_sample_rate(cpal::SampleRate(output_rate))
                    .config(),
                format,
                output_rate,
            ));
        }
    }
    selected
        .map(|(_, _, config, format, output_rate)| (config, format, output_rate))
        .ok_or_else(|| {
            playback_error(
                "PLAYBACK_OUTPUT_UNSUPPORTED",
                "The output device has no compatible stereo format.",
                false,
            )
        })
}

fn interpolate_sample(left: i16, right: i16, phase: u64, output_rate: u32) -> i16 {
    let output_rate = i64::from(output_rate.max(1));
    let phase = phase.min(output_rate as u64) as i64;
    let numerator = i64::from(left) * (output_rate - phase) + i64::from(right) * phase;
    (numerator / output_rate).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

fn next_resampled_frame(
    queue: &mut VecDeque<i16>,
    phase: &mut u64,
    source_rate: u32,
    output_rate: u32,
    source_exhausted: bool,
) -> Option<([i16; OUTPUT_CHANNELS], u64)> {
    if source_rate == 0 || output_rate == 0 {
        return None;
    }
    let available_frames = queue.len() / OUTPUT_CHANNELS;
    if available_frames == 0 || (available_frames < 2 && !source_exhausted) {
        return None;
    }

    let left = [queue[0], queue[1]];
    let right = if available_frames >= 2 {
        [queue[2], queue[3]]
    } else {
        left
    };
    let frame = [
        interpolate_sample(left[0], right[0], *phase, output_rate),
        interpolate_sample(left[1], right[1], *phase, output_rate),
    ];
    let accumulated_phase = phase.saturating_add(u64::from(source_rate));
    let advance_frames = accumulated_phase / u64::from(output_rate);
    if !source_exhausted && advance_frames >= available_frames as u64 {
        return None;
    }

    let consumed_frames = advance_frames.min(available_frames as u64);
    for _ in 0..consumed_frames {
        queue.pop_front();
        queue.pop_front();
    }
    *phase = accumulated_phase % u64::from(output_rate);
    Some((frame, consumed_frames))
}

fn set_silence<T: SizedSample + FromSample<i16>>(data: &mut [T]) {
    for sample in data {
        *sample = T::from_sample(0_i16);
    }
}

fn render_routed<T: SizedSample + FromSample<i16>>(
    data: &mut [T],
    runtime: &PlaybackRuntime,
    routed: &RoutedPlaybackSource,
    reader: &mut crate::routed_playback_ring::Reader,
    duration_frames: u64,
) {
    use crate::routed_playback_ring::Poll;
    if routed.cancel.load(Ordering::Acquire) || !runtime.output_permitted() {
        set_silence(data);
        return;
    }
    let Some(_gate) = routed.lease.try_output() else {
        set_silence(data);
        return;
    };
    if routed.cancel.load(Ordering::Acquire) || !runtime.output_permitted() {
        set_silence(data);
        return;
    }
    let paused = runtime.state.load(Ordering::Acquire) != STATE_PLAYING;
    if paused {
        // Drain stale epochs even when paused so old data cannot fill the ring.
        let _ = reader.poll(true, crate::routed_playback_ring::MAX_DISCARD_PER_POLL);
        set_silence(data);
        return;
    }
    let epoch = runtime.stream_epoch.load(Ordering::Acquire);
    let mut discard_budget = crate::routed_playback_ring::MAX_DISCARD_PER_POLL;
    let mut count = 0_u64;
    let mut missing = false;
    for output in data.chunks_mut(OUTPUT_CHANNELS) {
        let samples = if missing {
            [0, 0]
        } else {
            match reader.poll(false, discard_budget) {
                Poll::Frame(frame) => frame.samples,
                Poll::Empty | Poll::DrainPending | Poll::Paused | Poll::Revoked => {
                    missing = true;
                    [0, 0]
                }
            }
        };
        // A poll may use the entire stale budget; do not repeat it for every
        // output frame. The bounded race-related extra pop is documented by rtrb wrapper.
        discard_budget = 0;
        if !missing {
            count += 1;
        }
        output[0] = T::from_sample(samples[0]);
        if output.len() > 1 {
            output[1] = T::from_sample(samples[1]);
        }
    }
    if missing {
        let elapsed = runtime.started_at.elapsed().as_millis() as u64;
        let _ = runtime.underrun_started_ms.compare_exchange(
            0,
            elapsed,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    } else {
        runtime.underrun_started_ms.store(0, Ordering::Release);
    }
    if count > 0 && runtime.stream_epoch.load(Ordering::Acquire) == epoch {
        let position = runtime.position_frames.load(Ordering::Acquire);
        runtime.position_frames.store(
            position.saturating_add(count).min(duration_frames),
            Ordering::Release,
        );
    }
}

fn build_routed_output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    runtime: Arc<PlaybackRuntime>,
    routed: Arc<RoutedPlaybackSource>,
    mut reader: crate::routed_playback_ring::Reader,
    duration_frames: u64,
) -> Result<Stream, ReviewError>
where
    T: SizedSample + FromSample<i16> + Send + 'static,
{
    let callback_runtime = Arc::clone(&runtime);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                render_routed(
                    data,
                    &callback_runtime,
                    &routed,
                    &mut reader,
                    duration_frames,
                )
            },
            move |_error| {
                runtime.device_lost.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| playback_error("PLAYBACK_DEVICE_LOST", "The output device was lost.", true))
}

fn build_output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    runtime: Arc<PlaybackRuntime>,
    source_rate: u32,
    output_rate: u32,
    duration_frames: u64,
    routed: Option<Arc<RoutedPlaybackSource>>,
) -> Result<Stream, ReviewError>
where
    T: SizedSample + FromSample<i16> + Send + 'static,
{
    let stream_runtime = Arc::clone(&runtime);
    let mut resampler_phase = 0_u64;
    let mut resampler_epoch = runtime.stream_epoch.load(Ordering::Acquire);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                if !stream_runtime.output_permitted()
                    || stream_runtime.state.load(Ordering::Acquire) != STATE_PLAYING
                {
                    set_silence(data);
                    return;
                }
                let _gate = match &routed {
                    Some(source) => match source.lease.try_output() {
                        Some(gate) => Some(gate),
                        None => {
                            set_silence(data);
                            return;
                        }
                    },
                    None => None,
                };
                let mut queue = match stream_runtime.queue.try_lock() {
                    Ok(queue) => queue,
                    Err(_) => {
                        set_silence(data);
                        return;
                    }
                };
                let epoch = stream_runtime.stream_epoch.load(Ordering::Acquire);
                if epoch != resampler_epoch {
                    resampler_phase = 0;
                    resampler_epoch = epoch;
                }
                let source_exhausted = stream_runtime.source_exhausted.load(Ordering::Acquire);
                let mut missing = false;
                let mut consumed_source_frames = 0_u64;
                for frame in data.chunks_mut(OUTPUT_CHANNELS) {
                    let (sample, consumed) = match next_resampled_frame(
                        &mut queue,
                        &mut resampler_phase,
                        source_rate,
                        output_rate,
                        source_exhausted,
                    ) {
                        Some(result) => result,
                        None => {
                            missing = true;
                            ([0, 0], 0)
                        }
                    };
                    frame[0] = T::from_sample(sample[0]);
                    if frame.len() > 1 {
                        frame[1] = T::from_sample(sample[1]);
                    }
                    consumed_source_frames = consumed_source_frames.saturating_add(consumed);
                }
                if missing {
                    let elapsed = stream_runtime.started_at.elapsed().as_millis() as u64;
                    let _ = stream_runtime.underrun_started_ms.compare_exchange(
                        0,
                        elapsed,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                } else {
                    stream_runtime
                        .underrun_started_ms
                        .store(0, Ordering::Release);
                }
                if consumed_source_frames > 0 {
                    let mut position = stream_runtime.position_frames.load(Ordering::Acquire);
                    loop {
                        if stream_runtime.stream_epoch.load(Ordering::Acquire) != epoch {
                            break;
                        }
                        let next_position = position
                            .saturating_add(consumed_source_frames)
                            .min(duration_frames);
                        match stream_runtime.position_frames.compare_exchange_weak(
                            position,
                            next_position,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => break,
                            Err(actual_position) => position = actual_position,
                        }
                    }
                }
            },
            move |_error| {
                runtime.device_lost.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| playback_error("PLAYBACK_DEVICE_LOST", "The output device was lost.", true))
}

fn read_frame(
    reader: &mut AudioReader,
    channels: u16,
) -> Result<[i16; OUTPUT_CHANNELS], ReviewError> {
    let mut samples = reader.samples::<i16>();
    let left = samples
        .next()
        .ok_or_else(safe_storage_error)?
        .map_err(|_| safe_storage_error())?;
    if channels == 1 {
        Ok([left, left])
    } else {
        let right = samples
            .next()
            .ok_or_else(safe_storage_error)?
            .map_err(|_| safe_storage_error())?;
        Ok([left, right])
    }
}

fn read_source_frame(source: &mut OpenSource) -> Result<[i16; OUTPUT_CHANNELS], ReviewError> {
    if source.next_frame >= source.frame_count {
        // Millisecond timestamps can round a source duration up by at most
        // half a millisecond. Keep playback on its ledger timeline with silence.
        return Ok([0, 0]);
    }
    let frame = read_frame(&mut source.reader, source.channels)?;
    source.next_frame += 1;
    Ok(frame)
}

fn queue_len(runtime: &PlaybackRuntime) -> usize {
    runtime
        .queue
        .lock()
        .map(|queue| queue.len())
        .unwrap_or(MAX_QUEUE_BYTES / 2)
}

fn queue_frame(runtime: &PlaybackRuntime, frame: [i16; OUTPUT_CHANNELS]) -> bool {
    let Ok(mut queue) = runtime.queue.lock() else {
        return false;
    };
    if (queue.len() + OUTPUT_CHANNELS) * std::mem::size_of::<i16>() > MAX_QUEUE_BYTES {
        return false;
    }
    queue.push_back(frame[0]);
    queue.push_back(frame[1]);
    true
}

fn open_descriptor(
    prepared: &PreparedPlayback,
    descriptor: &ChunkDescriptor,
) -> Result<Option<OpenSource>, ReviewError> {
    let source = if let Some(routed) = &prepared.routed {
        let reader = crate::completed_source_media::open_verified_chunk(
            &routed.storage,
            &routed.data_root,
            &routed.manifest,
            &routed.source_id,
            routed.epoch,
            descriptor.sequence_no,
            &routed.cancel,
        )
        .map_err(|code| playback_error(&code, "Source playback is unavailable.", false))?;
        Some(OpenSource {
            reader,
            channels: 1,
            sample_rate: prepared.sample_rate,
            frame_count: descriptor.frame_end - descriptor.frame_start,
            next_frame: 0,
        })
    } else {
        open_source(&prepared.project_root, descriptor)?
    };
    if let Some(source) = source.as_ref() {
        if source.channels != prepared.source_channels || source.sample_rate != prepared.sample_rate
        {
            return Err(playback_error(
                "PLAYBACK_FORMAT_UNSUPPORTED",
                "The playback source formats do not match.",
                false,
            ));
        }
    }
    Ok(source)
}

fn fill_queue(
    prepared: &PreparedPlayback,
    runtime: &PlaybackRuntime,
    cursor: &mut u64,
    source_index: &mut Option<usize>,
    source: &mut Option<OpenSource>,
) -> Result<(), ReviewError> {
    if let Some(routed) = &prepared.routed {
        if !routed.lease.is_available() {
            return Err(playback_error(
                "PLAYBACK_SOURCE_REVOKED",
                "Source playback is unavailable.",
                false,
            ));
        }
        let available = (MAX_QUEUE_BYTES / std::mem::size_of::<i16>())
            .saturating_sub(queue_len(runtime))
            / OUTPUT_CHANNELS;
        let maximum =
            available.min(MAX_READ_BYTES / (OUTPUT_CHANNELS * std::mem::size_of::<i16>()));
        let mut pending = Vec::with_capacity(maximum * OUTPUT_CHANNELS);
        let mut next_cursor = *cursor;
        while next_cursor < prepared.duration_frames && pending.len() / OUTPUT_CHANNELS < maximum {
            let index = prepared
                .descriptors
                .partition_point(|d| d.frame_end <= next_cursor);
            if !prepared
                .descriptors
                .get(index)
                .is_some_and(|d| d.frame_start <= next_cursor && next_cursor < d.frame_end)
            {
                return Err(playback_error(
                    "PLAYBACK_SOURCE_MISSING",
                    "Source playback is unavailable.",
                    false,
                ));
            }
            if Some(index) != *source_index {
                *source = open_descriptor(prepared, &prepared.descriptors[index])?;
                *source_index = Some(index);
                let reader = source.as_mut().ok_or_else(|| {
                    playback_error(
                        "PLAYBACK_SOURCE_MISSING",
                        "Source playback is unavailable.",
                        false,
                    )
                })?;
                let local_frame = next_cursor - prepared.descriptors[index].frame_start;
                reader.next_frame = local_frame;
                if local_frame < reader.frame_count {
                    reader
                        .reader
                        .seek(local_frame as u32)
                        .map_err(|_| safe_storage_error())?;
                }
            }
            pending.extend_from_slice(&read_source_frame(
                source.as_mut().ok_or_else(safe_storage_error)?,
            )?);
            next_cursor += 1;
        }
        if !pending.is_empty() {
            let _gate = routed.lease.producer_output().ok_or_else(|| {
                playback_error(
                    "PLAYBACK_SOURCE_REVOKED",
                    "Source playback is unavailable.",
                    false,
                )
            })?;
            let mut queue = runtime.queue.lock().map_err(|_| safe_storage_error())?;
            if (queue.len() + pending.len()) * std::mem::size_of::<i16>() > MAX_QUEUE_BYTES {
                return Err(playback_error(
                    "PLAYBACK_BUFFER_UNDERRUN",
                    "Source playback is unavailable.",
                    true,
                ));
            }
            queue.extend(pending);
            *cursor = next_cursor;
        }
        if *cursor >= prepared.duration_frames {
            *source = None;
            *source_index = None;
        }
        return Ok(());
    }
    let mut frames_written = 0usize;
    let max_frames = MAX_READ_BYTES / (OUTPUT_CHANNELS * std::mem::size_of::<i16>());
    if prepared
        .routed
        .as_ref()
        .is_some_and(|r| !r.lease.is_available())
    {
        return Err(playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        ));
    }
    while *cursor < prepared.duration_frames
        && queue_len(runtime) * std::mem::size_of::<i16>() < MAX_QUEUE_BYTES
        && frames_written < max_frames
    {
        let index = prepared.descriptors.iter().position(|descriptor| {
            descriptor.frame_start <= *cursor && *cursor < descriptor.frame_end
        });
        if index != *source_index {
            *source = None;
            *source_index = index;
            if let Some(index) = index {
                if prepared.descriptors[index].available {
                    let Some(opened) = open_descriptor(prepared, &prepared.descriptors[index])?
                    else {
                        return Err(playback_error(
                            "PLAYBACK_SOURCE_MISSING",
                            "Playback source data disappeared after preparation.",
                            false,
                        ));
                    };
                    *source = Some(opened);
                    if let Some(reader) = source.as_mut() {
                        let local_frame =
                            cursor.saturating_sub(prepared.descriptors[index].frame_start);
                        reader.next_frame = local_frame;
                        if local_frame < reader.frame_count {
                            reader
                                .reader
                                .seek(local_frame as u32)
                                .map_err(|_| safe_storage_error())?;
                        }
                    }
                }
            }
        }
        let frame = if let Some(reader) = source.as_mut() {
            read_source_frame(reader)?
        } else {
            [0, 0]
        };
        if !queue_frame(runtime, frame) {
            break;
        }
        *cursor += 1;
        frames_written += 1;
    }
    if *cursor >= prepared.duration_frames {
        *source = None;
        *source_index = None;
    }
    Ok(())
}

fn publish_ready(
    ready_tx: &mpsc::Sender<Result<(), ReviewError>>,
    result: Result<(), ReviewError>,
    cancel: &AtomicBool,
) {
    if !cancel.load(Ordering::Acquire) {
        let _ = ready_tx.send(result);
    }
}

fn routed_worker_health(runtime: &PlaybackRuntime) -> Option<ReviewError> {
    if runtime.device_lost.load(Ordering::Acquire) {
        return Some(playback_error(
            "PLAYBACK_DEVICE_LOST",
            "The output device was lost.",
            true,
        ));
    }
    let underrun = runtime.underrun_started_ms.load(Ordering::Acquire);
    if runtime.state.load(Ordering::Acquire) == STATE_PLAYING
        && underrun > 0
        && runtime.started_at.elapsed().as_millis() as u64 > underrun + 1_000
    {
        return Some(playback_error(
            "PLAYBACK_BUFFER_UNDERRUN",
            "Playback buffer underrun paused playback.",
            true,
        ));
    }
    None
}

/// Publish a source-read failure only if its generation is still current.
/// A control may have advanced the epoch while file verification was in flight.
fn publish_routed_generation_error(
    runtime: &PlaybackRuntime,
    lease: &crate::source_playback_lease::SourcePlaybackLease,
    generation: u64,
    error: ReviewError,
) -> bool {
    let Some(_output) = lease.output_fence() else {
        runtime.set_error(playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        ));
        return true;
    };
    if runtime.stream_epoch.load(Ordering::Acquire) != generation {
        return false;
    }
    runtime.set_error(error);
    true
}

/// An observed timeout is only a hint until rechecked under the same fence as
/// pause/seek; a later control may have reset the underrun timer already.
fn publish_routed_health(
    runtime: &PlaybackRuntime,
    lease: &crate::source_playback_lease::SourcePlaybackLease,
) -> bool {
    let Some(_output) = lease.output_fence() else {
        runtime.set_error(playback_error(
            "PLAYBACK_SOURCE_REVOKED",
            "Source playback is unavailable.",
            false,
        ));
        return true;
    };
    if let Some(error) = routed_worker_health(runtime) {
        runtime.set_error(error);
        return true;
    }
    false
}

fn worker_loop(
    prepared: PreparedPlayback,
    runtime: Arc<PlaybackRuntime>,
    cancel: Arc<AtomicBool>,
    command_rx: mpsc::Receiver<WorkerCommand>,
    ready_tx: mpsc::Sender<Result<(), ReviewError>>,
) {
    if prepared.routed.is_some() {
        worker_loop_routed(prepared, runtime, cancel, command_rx, ready_tx);
        return;
    }
    if cancel.load(Ordering::Acquire) {
        return;
    }
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_OUTPUT_UNSUPPORTED",
                "No compatible output device is available.",
                false,
            )),
            &cancel,
        );
        return;
    };
    let (config, sample_format, output_rate) =
        match supported_output_config(&device, prepared.sample_rate) {
            Ok(config) => config,
            Err(error) => {
                publish_ready(&ready_tx, Err(error), &cancel);
                return;
            }
        };
    if prepared.routed.is_some() && output_rate != prepared.sample_rate {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_FORMAT_UNSUPPORTED",
                "The output device does not support the source sample rate.",
                false,
            )),
            &cancel,
        );
        return;
    }
    if cancel.load(Ordering::Acquire) {
        return;
    }
    let stream = match sample_format {
        SampleFormat::F32 => build_output_stream::<f32>(
            &device,
            &config,
            Arc::clone(&runtime),
            prepared.sample_rate,
            output_rate,
            prepared.duration_frames,
            prepared.routed.clone(),
        ),
        SampleFormat::I16 => build_output_stream::<i16>(
            &device,
            &config,
            Arc::clone(&runtime),
            prepared.sample_rate,
            output_rate,
            prepared.duration_frames,
            prepared.routed.clone(),
        ),
        SampleFormat::U16 => build_output_stream::<u16>(
            &device,
            &config,
            Arc::clone(&runtime),
            prepared.sample_rate,
            output_rate,
            prepared.duration_frames,
            prepared.routed.clone(),
        ),
        _ => Err(playback_error(
            "PLAYBACK_OUTPUT_UNSUPPORTED",
            "The output device has no compatible stereo format.",
            false,
        )),
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => {
            publish_ready(&ready_tx, Err(error), &cancel);
            return;
        }
    };
    if cancel.load(Ordering::Acquire) {
        drop(stream);
        return;
    }
    if stream.play().is_err() {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_DEVICE_LOST",
                "The output device was lost.",
                true,
            )),
            &cancel,
        );
        return;
    }
    if cancel.load(Ordering::Acquire) {
        drop(stream);
        return;
    }
    publish_ready(&ready_tx, Ok(()), &cancel);
    if cancel.load(Ordering::Acquire) {
        drop(stream);
        return;
    }

    let mut cursor = 0_u64;
    let mut source_index = None;
    let mut source = None;
    loop {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        if prepared
            .routed
            .as_ref()
            .is_some_and(|source| !source.lease.is_available())
        {
            runtime.clear_queue();
            runtime.set_error(playback_error(
                "PLAYBACK_SOURCE_REVOKED",
                "Source playback is unavailable.",
                false,
            ));
            break;
        }
        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(WorkerCommand::Close) => break,
            Ok(WorkerCommand::Play) => {
                if runtime.state.load(Ordering::Acquire) == STATE_ENDED
                    || runtime.position_frames.load(Ordering::Acquire) >= prepared.duration_frames
                {
                    cursor = 0;
                    runtime.stream_epoch.fetch_add(1, Ordering::AcqRel);
                    runtime.position_frames.store(0, Ordering::Release);
                    runtime.clear_queue();
                    runtime.source_exhausted.store(false, Ordering::Release);
                    source = None;
                    source_index = None;
                }
                runtime.state.store(STATE_PLAYING, Ordering::Release);
            }
            Ok(WorkerCommand::Pause) => {
                runtime.state.store(STATE_PAUSED, Ordering::Release);
                runtime.stream_epoch.fetch_add(1, Ordering::AcqRel);
                cursor = runtime
                    .position_frames
                    .load(Ordering::Acquire)
                    .min(prepared.duration_frames);
                runtime.clear_queue();
                runtime
                    .source_exhausted
                    .store(cursor >= prepared.duration_frames, Ordering::Release);
                source = None;
                source_index = None;
            }
            Ok(WorkerCommand::Seek { frame, epoch }) => {
                cursor = frame.min(prepared.duration_frames);
                runtime.stream_epoch.store(epoch, Ordering::Release);
                runtime.position_frames.store(cursor, Ordering::Release);
                runtime.clear_queue();
                runtime
                    .source_exhausted
                    .store(cursor >= prepared.duration_frames, Ordering::Release);
                source = None;
                source_index = None;
                if cursor >= prepared.duration_frames {
                    runtime.state.store(STATE_ENDED, Ordering::Release);
                }
            }
            Ok(
                WorkerCommand::RoutedPlay { .. }
                | WorkerCommand::RoutedPause { .. }
                | WorkerCommand::RoutedSeek { .. },
            ) => {
                runtime.set_error(playback_error(
                    "PLAYBACK_SOURCE_INVALID",
                    "Playback command is invalid.",
                    false,
                ));
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if cancel.load(Ordering::Acquire) {
            break;
        }
        if runtime.device_lost.load(Ordering::Acquire) {
            runtime.set_error(playback_error(
                "PLAYBACK_DEVICE_LOST",
                "The output device was lost.",
                true,
            ));
            break;
        }
        if runtime.state.load(Ordering::Acquire) != STATE_PLAYING {
            continue;
        }
        if cancel.load(Ordering::Acquire) {
            break;
        }
        if let Err(error) = fill_queue(
            &prepared,
            &runtime,
            &mut cursor,
            &mut source_index,
            &mut source,
        ) {
            runtime.set_error(error);
            break;
        }
        runtime
            .source_exhausted
            .store(cursor >= prepared.duration_frames, Ordering::Release);
        let underrun = runtime.underrun_started_ms.load(Ordering::Acquire);
        if underrun > 0 && runtime.started_at.elapsed().as_millis() as u64 > underrun + 1_000 {
            runtime.set_error(playback_error(
                "PLAYBACK_BUFFER_UNDERRUN",
                "Playback buffer underrun paused playback.",
                true,
            ));
            break;
        }
        if cursor >= prepared.duration_frames && queue_len(&runtime) == 0 {
            runtime
                .position_frames
                .store(prepared.duration_frames, Ordering::Release);
            runtime.state.store(STATE_ENDED, Ordering::Release);
        }
    }
    if runtime.state.load(Ordering::Acquire) != STATE_ERROR {
        runtime.state.store(STATE_PAUSED, Ordering::Release);
    }
    drop(stream);
}

fn worker_loop_routed(
    prepared: PreparedPlayback,
    runtime: Arc<PlaybackRuntime>,
    cancel: Arc<AtomicBool>,
    command_rx: mpsc::Receiver<WorkerCommand>,
    ready_tx: mpsc::Sender<Result<(), ReviewError>>,
) {
    use crate::routed_playback_ring::{self as ring, Push};
    let Some(routed) = prepared.routed.as_ref().cloned() else {
        return;
    };
    if !Arc::ptr_eq(&cancel, &routed.cancel)
        || runtime
            .routed_cancel
            .set(Arc::clone(&routed.cancel))
            .is_err()
    {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_SOURCE_INVALID",
                "Source playback is unavailable.",
                false,
            )),
            &cancel,
        );
        return;
    }
    if cancel.load(Ordering::Acquire) {
        return;
    }
    let (control, mut writer, reader) = match ring::channel(ring::MAX_FRAMES) {
        Ok(parts) => parts,
        Err(_) => {
            publish_ready(
                &ready_tx,
                Err(playback_error(
                    "RESOURCE_LIMIT",
                    "Source playback is unavailable.",
                    false,
                )),
                &cancel,
            );
            return;
        }
    };
    if runtime.routed_control.set(control.clone()).is_err() {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_SOURCE_INVALID",
                "Source playback is unavailable.",
                false,
            )),
            &cancel,
        );
        return;
    }
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_OUTPUT_UNSUPPORTED",
                "No compatible output device is available.",
                false,
            )),
            &cancel,
        );
        return;
    };
    let (config, format, output_rate) = match supported_output_config(&device, prepared.sample_rate)
    {
        Ok(value) => value,
        Err(error) => {
            publish_ready(&ready_tx, Err(error), &cancel);
            return;
        }
    };
    if output_rate != prepared.sample_rate {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_FORMAT_UNSUPPORTED",
                "The output device does not support the source sample rate.",
                false,
            )),
            &cancel,
        );
        return;
    }
    let stream = match format {
        SampleFormat::F32 => build_routed_output_stream::<f32>(
            &device,
            &config,
            Arc::clone(&runtime),
            Arc::clone(&routed),
            reader,
            prepared.duration_frames,
        ),
        SampleFormat::I16 => build_routed_output_stream::<i16>(
            &device,
            &config,
            Arc::clone(&runtime),
            Arc::clone(&routed),
            reader,
            prepared.duration_frames,
        ),
        SampleFormat::U16 => build_routed_output_stream::<u16>(
            &device,
            &config,
            Arc::clone(&runtime),
            Arc::clone(&routed),
            reader,
            prepared.duration_frames,
        ),
        _ => Err(playback_error(
            "PLAYBACK_OUTPUT_UNSUPPORTED",
            "The output device has no compatible stereo format.",
            false,
        )),
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => {
            publish_ready(&ready_tx, Err(error), &cancel);
            return;
        }
    };
    if cancel.load(Ordering::Acquire) {
        drop(stream);
        return;
    }
    if stream.play().is_err() {
        publish_ready(
            &ready_tx,
            Err(playback_error(
                "PLAYBACK_DEVICE_LOST",
                "The output device was lost.",
                true,
            )),
            &cancel,
        );
        return;
    }
    publish_ready(&ready_tx, Ok(()), &cancel);
    let mut cursor = 0_u64;
    let mut command_epoch = 0_u64;
    let (mut source_index, mut source) = (None, None);
    loop {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        if !routed.lease.is_available() {
            control.revoke();
            runtime.set_error(playback_error(
                "PLAYBACK_SOURCE_REVOKED",
                "Source playback is unavailable.",
                false,
            ));
            break;
        }
        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(WorkerCommand::Close) => break,
            Ok(WorkerCommand::RoutedPlay { epoch }) => {
                if epoch != runtime.stream_epoch.load(Ordering::Acquire) {
                    continue;
                }
                command_epoch = epoch;
            }
            Ok(
                WorkerCommand::RoutedPause { epoch, frame }
                | WorkerCommand::RoutedSeek { epoch, frame },
            ) => {
                if epoch != runtime.stream_epoch.load(Ordering::Acquire) {
                    continue;
                }
                command_epoch = epoch;
                cursor = frame.min(prepared.duration_frames);
                source = None;
                source_index = None;
            }
            Ok(_) => {
                runtime.set_error(playback_error(
                    "PLAYBACK_SOURCE_INVALID",
                    "Playback command is invalid.",
                    false,
                ));
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if routed_worker_health(&runtime).is_some()
            && publish_routed_health(&runtime, &routed.lease)
        {
            break;
        }
        if runtime.state.load(Ordering::Acquire) != STATE_PLAYING
            || command_epoch != runtime.stream_epoch.load(Ordering::Acquire)
        {
            continue;
        }
        let ring_epoch = control.current_epoch();
        let count = writer
            .free_slots()
            .min(2048)
            .min(prepared.duration_frames.saturating_sub(cursor) as usize);
        let mut pending = Vec::with_capacity(count);
        let mut candidate = cursor;
        let read_result = (|| -> Result<(), ReviewError> {
            while pending.len() < count {
                let index = prepared
                    .descriptors
                    .partition_point(|d| d.frame_end <= candidate);
                let descriptor = prepared
                    .descriptors
                    .get(index)
                    .filter(|d| d.frame_start <= candidate && candidate < d.frame_end)
                    .ok_or_else(|| {
                        playback_error(
                            "PLAYBACK_SOURCE_MISSING",
                            "Source playback is unavailable.",
                            false,
                        )
                    })?;
                if source_index != Some(index) {
                    source = open_descriptor(&prepared, descriptor)?;
                    source_index = Some(index);
                    let reader = source.as_mut().ok_or_else(safe_storage_error)?;
                    let offset = candidate - descriptor.frame_start;
                    reader.next_frame = offset;
                    if offset < reader.frame_count {
                        reader
                            .reader
                            .seek(offset as u32)
                            .map_err(|_| safe_storage_error())?;
                    }
                }
                pending.push(read_source_frame(
                    source.as_mut().ok_or_else(safe_storage_error)?,
                )?);
                candidate += 1;
            }
            Ok(())
        })();
        if cancel.load(Ordering::Acquire) {
            break;
        }
        if let Err(error) = read_result {
            if publish_routed_generation_error(&runtime, &routed.lease, command_epoch, error) {
                break;
            }
            source = None;
            source_index = None;
            continue;
        }
        if !pending.is_empty() {
            let Some(_gate) = routed.lease.producer_output() else {
                control.revoke();
                runtime.set_error(playback_error(
                    "PLAYBACK_SOURCE_REVOKED",
                    "Source playback is unavailable.",
                    false,
                ));
                break;
            };
            if command_epoch != runtime.stream_epoch.load(Ordering::Acquire)
                || ring_epoch != control.current_epoch()
                || cancel.load(Ordering::Acquire)
            {
                source = None;
                source_index = None;
                continue;
            }
            for frame in pending {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                match writer.push(ring_epoch, frame) {
                    Push::Published => cursor += 1,
                    Push::Full | Push::Stale | Push::Invalidated => {
                        source = None;
                        source_index = None;
                        break;
                    }
                    Push::Revoked => {
                        control.revoke();
                        runtime.set_error(playback_error(
                            "PLAYBACK_SOURCE_REVOKED",
                            "Source playback is unavailable.",
                            false,
                        ));
                        break;
                    }
                }
            }
        }
        if runtime.state.load(Ordering::Acquire) == STATE_ERROR {
            break;
        }
        if cursor >= prepared.duration_frames
            && runtime.position_frames.load(Ordering::Acquire) >= prepared.duration_frames
        {
            // Serialize the EOF transition with seek/pause control; otherwise
            // an old epoch can mark a freshly sought session ended.
            let Some(_output) = routed.lease.output_fence() else {
                control.revoke();
                runtime.set_error(playback_error(
                    "PLAYBACK_SOURCE_REVOKED",
                    "Source playback is unavailable.",
                    false,
                ));
                break;
            };
            if command_epoch == runtime.stream_epoch.load(Ordering::Acquire)
                && runtime.position_frames.load(Ordering::Acquire) >= prepared.duration_frames
            {
                let _ = runtime.state.compare_exchange(
                    STATE_PLAYING,
                    STATE_ENDED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            }
        }
    }
    control.revoke();
    if runtime.state.load(Ordering::Acquire) != STATE_ERROR {
        runtime.state.store(STATE_PAUSED, Ordering::Release);
    }
    drop(stream);
}

struct WorkerLaunch {
    command_tx: mpsc::Sender<WorkerCommand>,
    ready_rx: mpsc::Receiver<Result<(), ReviewError>>,
    join: WorkerJoin,
}

enum PrepareJoin {
    Direct(JoinHandle<()>),
    Owned(Arc<PrepareHandleCapsule>),
}

impl std::fmt::Debug for PrepareJoin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Direct(_) => "DirectJoin",
            Self::Owned(_) => "OwnedJoin",
        })
    }
}

type WorkerJoin = PrepareJoin;

impl PrepareJoin {
    fn join(self) -> Result<(), ReviewError> {
        match self {
            Self::Direct(handle) => handle
                .join()
                .map_err(|_| playback_error("NATIVE_UNAVAILABLE", "Prepare owner failed.", false)),
            Self::Owned(capsule) => capsule.claim()?.join(),
        }
    }
}

type PrepareFn =
    Box<dyn FnOnce(Arc<AtomicBool>) -> Result<PreparedPlayback, ReviewError> + Send + 'static>;
enum WorkerStartFailure {
    NotStarted(ReviewError),
    UnknownOwnership(ReviewError),
}

type WorkerFn = Arc<
    dyn Fn(
            PreparedPlayback,
            Arc<PlaybackRuntime>,
            Arc<AtomicBool>,
            Option<Arc<PrepareHandleCapsule>>,
        ) -> Result<WorkerLaunch, WorkerStartFailure>
        + Send
        + Sync,
>;

struct OpenPlaybackIo {
    prepare: PrepareFn,
    worker: WorkerFn,
}

impl OpenPlaybackIo {
    fn production(
        storage: Arc<genesis_block_native::Storage>,
        data_root: PathBuf,
        row: RecordingStorageRow,
        channel: String,
    ) -> Self {
        let prepare =
            Box::new(move |cancel| prepare_playback(&storage, &data_root, &row, channel, cancel));
        Self {
            prepare,
            worker: Arc::new(spawn_worker),
        }
    }

    fn production_source(
        storage: Arc<genesis_block_native::Storage>,
        data_root: PathBuf,
        row: RecordingStorageRow,
        source_id: String,
        epoch: i64,
    ) -> Result<Self, ReviewError> {
        if epoch <= 0
            || epoch > 9_007_199_254_740_991
            || uuid::Uuid::parse_str(&source_id)
                .map(|id| id.to_string() != source_id)
                .unwrap_or(true)
        {
            return Err(playback_error(
                "PLAYBACK_SOURCE_INVALID",
                "Source is unavailable.",
                false,
            ));
        }
        let prepare = Box::new(move |cancel| {
            prepare_routed_playback(storage, &data_root, &row, &source_id, epoch, cancel)
        });
        Ok(Self {
            prepare,
            worker: Arc::new(spawn_worker),
        })
    }
}

fn source_identity_for_prepared(
    prepared: &PreparedPlayback,
) -> Result<Option<PlaybackSourceIdentity>, ReviewError> {
    match &prepared.routed {
        Some(source)
            if prepared.channel == "source"
                && source.epoch > 0
                && source.epoch <= 9_007_199_254_740_991 =>
        {
            Ok(Some(PlaybackSourceIdentity {
                source_id: source.source_id.clone(),
                capture_epoch: source.epoch as u64,
            }))
        }
        None if prepared.channel != "source" => Ok(None),
        _ => Err(playback_error(
            "PLAYBACK_SOURCE_INVALID",
            "Source identity is unavailable.",
            false,
        )),
    }
}

struct OpenPlaybackTask {
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    lease: OpenLease,
    io: OpenPlaybackIo,
    deadline: Instant,
    owner: u128,
    prepare_owner: Option<Arc<PrepareHandleCapsule>>,
    worker_owner: Option<Arc<PrepareHandleCapsule>>,
    process_registry: Option<std::sync::Weak<ProcessOwnerRegistry>>,
    process_task: Option<std::sync::Weak<Mutex<Option<OpenPlaybackTask>>>>,
    late_owner: Option<Arc<Mutex<Option<PlaybackSession>>>>,
    late_collision: Option<Arc<Mutex<Option<PlaybackSession>>>>,
}

fn signal_routed_cancel(session: &PlaybackSession) {
    if let Some(cancel) = session.runtime.routed_cancel.get() {
        cancel.store(true, Ordering::Release);
    }
}

fn cleanup_session(mut session: PlaybackSession) -> Result<(), ReviewError> {
    signal_routed_cancel(&session);
    if session.runtime.routed_cancel.get().is_some() {
        // Fence in-flight output/publication even if quarantine revoked access.
        // The gates are acquired separately and outside manager/storage locks.
        if let Some(lease) = &session.runtime.routed_lease {
            lease.quiesce();
        }
    }
    let _ = session.command_tx.send(WorkerCommand::Close);
    let joined = session.join.take().map(|join| join.join()).transpose();
    joined.map_err(|_| playback_error("NATIVE_UNAVAILABLE", "Playback cleanup failed.", false))?;
    if session
        .guard
        .try_release_playback_token(session.guard_token)
        != Ok(true)
    {
        return Err(playback_error(
            "NATIVE_UNAVAILABLE",
            "Playback cleanup ownership is uncertain.",
            false,
        ));
    }
    Ok(())
}

fn spawn_session_cleanup(
    playback: PlaybackManager,
    session: PlaybackSession,
) -> Result<JoinHandle<()>, ReviewError> {
    // Suppress routed output before handing ownership to a potentially failing
    // thread spawn. Failure remains an error and keeps admission fail-closed.
    signal_routed_cancel(&session);
    let handle = session.handle.clone();
    let owner = session.owner;
    thread::Builder::new()
        .name("fung-playback-cleanup".to_string())
        .spawn(move || {
            let adoption = session
                .runtime
                .adoption_lease
                .get()
                .map(|lease| lease.offer());
            match cleanup_session(session) {
                Ok(()) => playback.mark_closed(handle, owner, adoption),
                Err(error) => eprintln!("playback cleanup failed: {}", error.code),
            }
        })
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback cleanup owner is unavailable.",
                true,
            )
        })
}

fn launch_worker_thread(
    command_tx: mpsc::Sender<WorkerCommand>,
    ready_rx: mpsc::Receiver<Result<(), ReviewError>>,
    owner: Option<Arc<PrepareHandleCapsule>>,
    job: Box<dyn FnOnce() + Send>,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
) -> Result<WorkerLaunch, WorkerStartFailure> {
    let unavailable = || {
        playback_error(
            "NATIVE_UNAVAILABLE",
            "The native playback owner is unavailable.",
            true,
        )
    };
    let join = if let Some(capsule) = owner {
        let mut permit = capsule
            .reserve_permit()
            .map_err(WorkerStartFailure::UnknownOwnership)?;
        // Trusted spawn returns Err only when no thread was created. On success
        // attach immediately, before any callback or lock acquisition.
        let handle = match spawn(job) {
            Ok(handle) => handle,
            Err(_) => {
                permit
                    .mark_no_worker()
                    .map_err(WorkerStartFailure::UnknownOwnership)?;
                return Err(WorkerStartFailure::NotStarted(unavailable()));
            }
        };
        if let Err(handle) = permit.attach(handle) {
            capsule.retain_terminal(handle);
            return Err(WorkerStartFailure::UnknownOwnership(safe_storage_error()));
        }
        permit
            .publish()
            .map_err(WorkerStartFailure::UnknownOwnership)?;
        WorkerJoin::Owned(capsule)
    } else {
        WorkerJoin::Direct(spawn(job).map_err(|_| WorkerStartFailure::NotStarted(unavailable()))?)
    };
    Ok(WorkerLaunch {
        command_tx,
        ready_rx,
        join,
    })
}

fn spawn_worker(
    prepared: PreparedPlayback,
    runtime: Arc<PlaybackRuntime>,
    cancel: Arc<AtomicBool>,
    owner: Option<Arc<PrepareHandleCapsule>>,
) -> Result<WorkerLaunch, WorkerStartFailure> {
    let (command_tx, command_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let job = Box::new(move || worker_loop(prepared, runtime, cancel, command_rx, ready_tx));
    launch_worker_thread(command_tx, ready_rx, owner, job, |work| {
        thread::Builder::new()
            .name("fung-playback-owner".to_string())
            .spawn(work)
    })
}

#[cfg(test)]
fn spawn_open_reaper(
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    cancel: Arc<AtomicBool>,
    generation: u64,
    prepare_join: Option<PrepareJoin>,
    worker: Option<WorkerLaunch>,
) -> Result<JoinHandle<()>, ReviewError> {
    spawn_open_reaper_with_spawn(
        playback,
        guard,
        guard_token,
        cancel,
        generation,
        prepare_join,
        worker,
        |job| {
            thread::Builder::new()
                .name("fung-playback-open-reaper".to_string())
                .spawn(job)
        },
    )
}

struct ProcessReaperInput {
    worker: Option<WorkerLaunch>,
    identity: Option<ProcessPlaybackIdentity>,
}

fn spawn_process_open_reaper(
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    cancel: Arc<AtomicBool>,
    generation: u64,
    prepare_join: Option<PrepareJoin>,
    input: ProcessReaperInput,
) -> Result<(), ReviewError> {
    spawn_process_open_reaper_with_spawn(
        playback,
        guard,
        guard_token,
        cancel,
        generation,
        prepare_join,
        input.worker,
        input.identity,
        |work| {
            thread::Builder::new()
                .name("fung-playback-open-reaper".into())
                .spawn(work)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_process_open_reaper_with_spawn(
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    cancel: Arc<AtomicBool>,
    generation: u64,
    prepare_join: Option<PrepareJoin>,
    worker: Option<WorkerLaunch>,
    identity: Option<ProcessPlaybackIdentity>,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
) -> Result<(), ReviewError> {
    let Some(identity) = identity else {
        return spawn_open_reaper_with_spawn(
            playback,
            guard,
            guard_token,
            cancel,
            generation,
            prepare_join,
            worker,
            spawn,
        )
        .map(drop);
    };
    let registry = identity.registry.upgrade().ok_or_else(safe_storage_error)?;
    let task = identity.task.upgrade().ok_or_else(safe_storage_error)?;
    let owner = {
        let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
        let owner = root
            .as_mut()
            .filter(|owner| {
                owner.generation == identity.generation
                    && Arc::ptr_eq(&owner.manager.inner, &playback.inner)
                    && owner.phase == ProcessOwnerPhase::Executing
                    && !owner.reaper_scheduled
                    && owner
                        .pending_open
                        .as_ref()
                        .is_some_and(|pending| Arc::ptr_eq(pending, &task))
            })
            .ok_or_else(safe_storage_error)?;
        owner.reaper_scheduled = true;
        Arc::clone(&owner.reaper_owner)
    };
    let mut permit = owner.reserve_permit()?;
    cancel.store(true, Ordering::Release);
    let has_worker = worker.is_some();
    let job = Box::new(move || {
        let mut cleanup_ok = true;
        if let Some(worker) = worker {
            let WorkerLaunch {
                command_tx, join, ..
            } = worker;
            let _ = command_tx.send(WorkerCommand::Close);
            cleanup_ok &= join.join().is_ok();
        }
        if let Some(join) = prepare_join {
            cleanup_ok &= join.join().is_ok();
        }
        if !cleanup_ok {
            eprintln!("playback open cleanup failed: owner thread panicked");
            return;
        }
        if let Err(error) =
            finish_process_open_reap(&playback, &guard, guard_token, &identity, has_worker)
        {
            eprintln!("playback open cleanup owner retained: {}", error.code);
        }
    });
    permit.spawn_with(job, spawn).map_err(|_| {
        playback_error(
            "NATIVE_UNAVAILABLE",
            "The native playback cleanup owner is unavailable.",
            true,
        )
    })?;
    permit.publish()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spawn_open_reaper_with_spawn(
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    cancel: Arc<AtomicBool>,
    generation: u64,
    prepare_join: Option<PrepareJoin>,
    worker: Option<WorkerLaunch>,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
) -> Result<JoinHandle<()>, ReviewError> {
    spawn_open_reaper_impl(
        playback,
        guard,
        guard_token,
        cancel,
        generation,
        prepare_join,
        worker,
        None,
        spawn,
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_open_reaper_impl(
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    cancel: Arc<AtomicBool>,
    generation: u64,
    prepare_join: Option<PrepareJoin>,
    worker: Option<WorkerLaunch>,
    identity: Option<ProcessPlaybackIdentity>,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
) -> Result<JoinHandle<()>, ReviewError> {
    let has_worker = worker.is_some();
    cancel.store(true, Ordering::Release);
    spawn(Box::new(move || {
        let mut cleanup_ok = true;
        if let Some(worker) = worker {
            let WorkerLaunch {
                command_tx, join, ..
            } = worker;
            let _ = command_tx.send(WorkerCommand::Close);
            cleanup_ok &= join.join().is_ok();
        }
        if let Some(join) = prepare_join {
            cleanup_ok &= join.join().is_ok();
        }
        if !cleanup_ok {
            // A panicked owner cannot prove that all native resources were
            // destroyed. Preserve the admission/opening reservation.
            eprintln!("playback open cleanup failed: owner thread panicked");
            return;
        }
        if let Some(identity) = identity {
            match finish_process_open_reap(&playback, &guard, guard_token, &identity, has_worker) {
                Ok(true) => {}
                Ok(false) => return,
                Err(error) => {
                    eprintln!("playback open cleanup owner retained: {}", error.code);
                    return;
                }
            }
        } else if guard.try_release_playback_token(guard_token) != Ok(true) {
            eprintln!("playback open cleanup failed: stale or poisoned guard");
            return;
        }
        playback.abort_open(generation);
    }))
    .map_err(|_| {
        playback_error(
            "NATIVE_UNAVAILABLE",
            "The native playback cleanup owner is unavailable.",
            true,
        )
    })
}

fn finish_process_open_reap(
    playback: &PlaybackManager,
    guard: &Arc<NativeCaptureGuard>,
    token: u64,
    identity: &ProcessPlaybackIdentity,
    has_worker: bool,
) -> Result<bool, ReviewError> {
    let registry = identity.registry.upgrade().ok_or_else(safe_storage_error)?;
    let task = identity.task.upgrade().ok_or_else(safe_storage_error)?;
    let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
    let owner = root
        .as_ref()
        .filter(|owner| {
            owner.generation == identity.generation
                && Arc::ptr_eq(&owner.manager.inner, &playback.inner)
                && Arc::ptr_eq(&owner.guard, guard)
                && matches!(
                    owner.phase,
                    ProcessOwnerPhase::Executing | ProcessOwnerPhase::ExecutorCompleted
                )
                && owner
                    .pending_open
                    .as_ref()
                    .is_some_and(|pending| Arc::ptr_eq(pending, &task))
                && owner.reaping.is_none()
        })
        .ok_or_else(safe_storage_error)?;
    let prepare_status = owner.prepare_owner.status();
    if (has_worker && prepare_status != PrepareStatus::Joined)
        || (!has_worker
            && !matches!(
                prepare_status,
                PrepareStatus::Joined | PrepareStatus::NoWorker
            ))
    {
        return Err(safe_storage_error());
    }
    let worker_status = owner.worker_owner.status();
    if (has_worker && worker_status != PrepareStatus::Joined)
        || (!has_worker
            && !matches!(
                worker_status,
                PrepareStatus::Reserved | PrepareStatus::NoWorker
            ))
        || owner
            .late_fallback
            .lock()
            .map_err(|_| safe_storage_error())?
            .is_some()
        || owner
            .late_collision
            .lock()
            .map_err(|_| safe_storage_error())?
            .is_some()
    {
        return Err(safe_storage_error());
    }
    if owner.phase == ProcessOwnerPhase::Executing {
        root.as_mut()
            .ok_or_else(safe_storage_error)?
            .reap_ready_token = Some(token);
        return Ok(false);
    }
    if guard.try_release_playback_token(token) != Ok(true) {
        return Err(safe_storage_error());
    }
    root.take();
    Ok(true)
}

fn public_state(snapshot: &PlaybackSnapshot) -> PlaybackState {
    snapshot.runtime.to_public_state(
        &snapshot.handle,
        &snapshot.project_id,
        &snapshot.recording_id,
        &snapshot.channel,
        snapshot.sample_rate,
        snapshot.duration_ms,
    )
}

fn release_owned_open(
    playback: &PlaybackManager,
    guard: &NativeCaptureGuard,
    guard_token: u64,
    generation: u64,
) -> Result<(), ReviewError> {
    if guard.try_release_playback_token(guard_token) != Ok(true) {
        return Err(playback_error(
            "NATIVE_UNAVAILABLE",
            "Playback cleanup ownership is uncertain.",
            false,
        ));
    }
    playback.abort_open(generation);
    Ok(())
}

fn release_finished_prepare(
    playback: &PlaybackManager,
    guard: &Arc<NativeCaptureGuard>,
    token: u64,
    generation: u64,
    identity: Option<&ProcessPlaybackIdentity>,
) -> Result<(), ReviewError> {
    if let Some(identity) = identity {
        if finish_process_open_reap(playback, guard, token, identity, false)? {
            playback.abort_open(generation);
        }
        Ok(())
    } else {
        release_owned_open(playback, guard, token, generation)
    }
}

fn validate_process_metadata(count: usize, production: bool) -> Result<(), ReviewError> {
    if (production || count != 0) && count != 6 {
        return Err(safe_storage_error());
    }
    Ok(())
}

fn open_playback_owned(task: OpenPlaybackTask) -> Result<PlaybackState, ReviewError> {
    let OpenPlaybackTask {
        playback,
        guard,
        guard_token,
        lease,
        io,
        deadline,
        owner,
        prepare_owner,
        worker_owner,
        process_registry,
        process_task,
        late_owner,
        late_collision,
    } = task;
    // Partial ownership metadata is never an admission or cleanup authority.
    let ownership_count = usize::from(prepare_owner.is_some())
        + usize::from(worker_owner.is_some())
        + usize::from(process_registry.is_some())
        + usize::from(process_task.is_some())
        + usize::from(late_owner.is_some())
        + usize::from(late_collision.is_some());
    validate_process_metadata(ownership_count, !cfg!(test))?;
    let reaper_identity =
        process_registry
            .clone()
            .zip(process_task.clone())
            .map(|(registry, task)| ProcessPlaybackIdentity {
                registry,
                task,
                generation: lease.generation,
            });
    let OpenPlaybackIo { prepare, worker } = io;
    let cancel = Arc::clone(&lease.cancel);
    let (prepared_tx, prepared_rx) = mpsc::channel();
    let prepare_cancel = Arc::clone(&cancel);
    let prepare_job: Box<dyn FnOnce() + Send> = Box::new(move || {
        let result = match open_start_error(&prepare_cancel, deadline) {
            Some(error) => Err(error),
            None => prepare(Arc::clone(&prepare_cancel)),
        };
        let _ = prepared_tx.send(result);
    });
    // A supervised task reserves its process-owned capsule before spawning.
    // Direct legacy callers retain the previous path until supervisor wiring.
    let prepare_join = match prepare_owner {
        Some(capsule) => {
            let mut permit = capsule.reserve_permit()?;
            if let Some(error) = open_start_error(&cancel, deadline) {
                permit.mark_no_worker()?;
                release_finished_prepare(
                    &playback,
                    &guard,
                    guard_token,
                    lease.generation,
                    reaper_identity.as_ref(),
                )?;
                return Err(error);
            }
            if permit
                .spawn_with(prepare_job, |work| {
                    thread::Builder::new()
                        .name("fung-playback-prepare".to_string())
                        .spawn(work)
                })
                .is_err()
            {
                // A failed thread spawn proves no preparation owner started.
                permit.mark_no_worker()?;
                release_finished_prepare(
                    &playback,
                    &guard,
                    guard_token,
                    lease.generation,
                    reaper_identity.as_ref(),
                )?;
                return Err(open_start_error(&cancel, deadline).unwrap_or_else(|| {
                    playback_error(
                        "NATIVE_UNAVAILABLE",
                        "Prepare owner could not be started.",
                        false,
                    )
                }));
            }
            permit.publish()?;
            PrepareJoin::Owned(capsule)
        }
        None => {
            if let Some(error) = open_start_error(&cancel, deadline) {
                release_owned_open(&playback, &guard, guard_token, lease.generation)?;
                return Err(error);
            }
            let handle = thread::Builder::new()
                .name("fung-playback-prepare".to_string())
                .spawn(prepare_job)
                .map_err(|_| {
                    if let Err(error) =
                        release_owned_open(&playback, &guard, guard_token, lease.generation)
                    {
                        return error;
                    }
                    playback_error(
                        "NATIVE_UNAVAILABLE",
                        "The native playback owner is unavailable.",
                        true,
                    )
                })?;
            PrepareJoin::Direct(handle)
        }
    };

    let prepared = match recv_until_deadline_or_cancel(&prepared_rx, deadline, &cancel) {
        Ok(result) => {
            if prepare_join.join().is_err() {
                // A panicked owner cannot prove that preparation quiesced.
                return Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native playback preparation owner is unavailable.",
                    true,
                ));
            }
            match result {
                Ok(prepared) => prepared,
                Err(error) => {
                    release_finished_prepare(
                        &playback,
                        &guard,
                        guard_token,
                        lease.generation,
                        reaper_identity.as_ref(),
                    )?;
                    return Err(error);
                }
            }
        }
        Err(wait_error) => {
            let error = match wait_error {
                OpenWaitError::Cancelled => playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native desktop window is unavailable.",
                    true,
                ),
                OpenWaitError::Timeout => open_timeout_error(),
                OpenWaitError::Disconnected => safe_storage_error(),
            };
            if let Err(cleanup_error) = spawn_process_open_reaper(
                playback.clone(),
                Arc::clone(&guard),
                guard_token,
                Arc::clone(&cancel),
                lease.generation,
                Some(prepare_join),
                ProcessReaperInput {
                    worker: None,
                    identity: reaper_identity.clone(),
                },
            ) {
                eprintln!(
                    "playback open cleanup could not be scheduled: {}",
                    cleanup_error.code
                );
            }
            return Err(error);
        }
    };

    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
        release_finished_prepare(
            &playback,
            &guard,
            guard_token,
            lease.generation,
            reaper_identity.as_ref(),
        )?;
        return Err(if cancel.load(Ordering::Acquire) {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native desktop window is unavailable.",
                true,
            )
        } else {
            open_timeout_error()
        });
    }

    let source_identity = match source_identity_for_prepared(&prepared) {
        Ok(identity) => identity,
        Err(error) => {
            release_finished_prepare(
                &playback,
                &guard,
                guard_token,
                lease.generation,
                reaper_identity.as_ref(),
            )?;
            return Err(error);
        }
    };
    let runtime = Arc::new(match &prepared.routed {
        Some(routed) => PlaybackRuntime::new_routed(
            prepared.degraded,
            prepared.missing_ranges.clone(),
            Arc::clone(&routed.lease),
        ),
        None => PlaybackRuntime::new(prepared.degraded, prepared.missing_ranges.clone()),
    });
    if lease.cleanup.runtime.set(Arc::clone(&runtime)).is_err() {
        release_finished_prepare(
            &playback,
            &guard,
            guard_token,
            lease.generation,
            reaper_identity.as_ref(),
        )?;
        return Err(safe_storage_error());
    }
    if let Some(identity) = source_identity {
        let _ = runtime.source_identity.set(identity);
    }
    // Reserve both bounded late slots before launching the worker. The
    // permit cannot be cloned and the registry rejects a second reservation.
    let late_reservation = if let (Some(registry), Some(task), Some(fallback), Some(collision)) = (
        &process_registry,
        &process_task,
        &late_owner,
        &late_collision,
    ) {
        let registry = registry.upgrade().ok_or_else(safe_storage_error)?;
        let task = task.upgrade().ok_or_else(safe_storage_error)?;
        let mut root = registry.slot.lock().map_err(|_| safe_storage_error())?;
        let root_owner = root
            .as_mut()
            .filter(|root_owner| {
                root_owner.generation == lease.generation
                    && Arc::ptr_eq(&root_owner.manager.inner, &playback.inner)
                    && root_owner.phase == ProcessOwnerPhase::Executing
                    && !root_owner.late_reserved
                    && root_owner
                        .pending_open
                        .as_ref()
                        .is_some_and(|pending| Arc::ptr_eq(pending, &task))
                    && Arc::ptr_eq(&root_owner.late_fallback, fallback)
                    && Arc::ptr_eq(&root_owner.late_collision, collision)
            })
            .ok_or_else(safe_storage_error)?;
        let first = fallback.lock().map_err(|_| safe_storage_error())?;
        let second = collision.lock().map_err(|_| safe_storage_error())?;
        if first.is_some() || second.is_some() {
            return Err(safe_storage_error());
        }
        root_owner.late_reserved = true;
        Some(LateReservation {
            fallback: Arc::clone(fallback),
            collision: Arc::clone(collision),
        })
    } else {
        None
    };
    let worker = match worker(
        prepared.clone(),
        Arc::clone(&runtime),
        Arc::clone(&cancel),
        worker_owner,
    ) {
        Ok(worker) => worker,
        Err(WorkerStartFailure::NotStarted(error)) => {
            release_finished_prepare(
                &playback,
                &guard,
                guard_token,
                lease.generation,
                reaper_identity.as_ref(),
            )?;
            return Err(error);
        }
        Err(WorkerStartFailure::UnknownOwnership(error)) => {
            // A worker may exist: retain guard and process slot without acknowledging cleanup.
            return Err(error);
        }
    };
    if lease
        .cleanup
        .worker_close
        .set(worker.command_tx.clone())
        .is_err()
    {
        let error = safe_storage_error();
        if let Err(cleanup_error) = spawn_process_open_reaper(
            playback.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&cancel),
            lease.generation,
            None,
            ProcessReaperInput {
                worker: Some(worker),
                identity: reaper_identity.clone(),
            },
        ) {
            eprintln!(
                "playback open cleanup could not be scheduled: {}",
                cleanup_error.code
            );
        }
        return Err(error);
    }
    if cancel.load(Ordering::Acquire) {
        lease.cleanup.signal_close();
    }

    match recv_until_deadline_or_cancel(&worker.ready_rx, deadline, &cancel) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            if let Err(cleanup_error) = spawn_process_open_reaper(
                playback.clone(),
                Arc::clone(&guard),
                guard_token,
                Arc::clone(&cancel),
                lease.generation,
                None,
                ProcessReaperInput {
                    worker: Some(worker),
                    identity: reaper_identity.clone(),
                },
            ) {
                eprintln!(
                    "playback open cleanup could not be scheduled: {}",
                    cleanup_error.code
                );
            }
            return Err(error);
        }
        Err(wait_error) => {
            let error = match wait_error {
                OpenWaitError::Cancelled => playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native desktop window is unavailable.",
                    true,
                ),
                OpenWaitError::Timeout => open_timeout_error(),
                OpenWaitError::Disconnected => playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native playback owner is unavailable.",
                    true,
                ),
            };
            if let Err(cleanup_error) = spawn_process_open_reaper(
                playback.clone(),
                Arc::clone(&guard),
                guard_token,
                Arc::clone(&cancel),
                lease.generation,
                None,
                ProcessReaperInput {
                    worker: Some(worker),
                    identity: reaper_identity.clone(),
                },
            ) {
                eprintln!(
                    "playback open cleanup could not be scheduled: {}",
                    cleanup_error.code
                );
            }
            return Err(error);
        }
    }

    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
        let error = if cancel.load(Ordering::Acquire) {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native desktop window is unavailable.",
                true,
            )
        } else {
            open_timeout_error()
        };
        if let Err(cleanup_error) = spawn_process_open_reaper(
            playback.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&cancel),
            lease.generation,
            None,
            ProcessReaperInput {
                worker: Some(worker),
                identity: reaper_identity.clone(),
            },
        ) {
            eprintln!(
                "playback open cleanup could not be scheduled: {}",
                cleanup_error.code
            );
        }
        return Err(error);
    }

    let handle = recording_review::random_opaque_id();
    let adoption_lease = Arc::new(PlaybackAdoptionLease::new(
        lease.generation,
        handle.clone(),
        Arc::clone(&runtime.close_requested),
        playback.lifecycle.visibility.clone(),
    ));
    if runtime
        .adoption_lease
        .set(Arc::clone(&adoption_lease))
        .is_err()
    {
        runtime.close_requested.store(true, Ordering::Release);
        if let Err(cleanup_error) = spawn_process_open_reaper(
            playback.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&cancel),
            lease.generation,
            None,
            ProcessReaperInput {
                worker: Some(worker),
                identity: reaper_identity.clone(),
            },
        ) {
            eprintln!(
                "playback adoption setup cleanup could not be scheduled: {}",
                cleanup_error.code
            );
        }
        return Err(safe_storage_error());
    }
    let WorkerLaunch {
        command_tx, join, ..
    } = worker;
    let session = PlaybackSession {
        process_identity: process_registry.zip(process_task).map(|(registry, task)| {
            ProcessPlaybackIdentity {
                registry,
                task,
                generation: lease.generation,
            }
        }),
        guard_token,
        owner,
        handle: handle.clone(),
        project_id: prepared.project_id.clone(),
        recording_id: prepared.recording_id.clone(),
        channel: prepared.channel.clone(),
        duration_ms: prepared.duration_ms,
        duration_frames: prepared.duration_frames,
        sample_rate: prepared.sample_rate,
        runtime: Arc::clone(&runtime),
        command_tx,
        join: Some(join),
        guard: Arc::clone(&guard),
    };
    // Root the exact session before any fallible frontier/install work. A
    // poisoned fallback is recovered solely to retain ownership, never to
    // claim a trusted state transition.
    let identity = session.process_identity.clone();
    let mut legacy_session = None;
    if let Some(reservation) = &late_reservation {
        let fallback = &reservation.fallback;
        let (mut stored, poisoned) = match fallback.lock() {
            Ok(stored) => (stored, false),
            Err(error) => (error.into_inner(), true),
        };
        if stored.is_none() {
            if poisoned {
                session
                    .runtime
                    .close_requested
                    .store(true, Ordering::Release);
                signal_routed_cancel(&session);
                let _ = session.command_tx.send(WorkerCommand::Close);
            }
            *stored = Some(session);
            drop(stored);
            if poisoned {
                return Err(safe_storage_error());
            }
        } else {
            // The unique executing claim can create at most one new session.
            // A pre-existing fallback is an invariant violation: suppress both
            // and retain the new session in the pre-reserved collision slot.
            if let Some(previous) = stored.as_ref() {
                previous
                    .runtime
                    .close_requested
                    .store(true, Ordering::Release);
                signal_routed_cancel(previous);
                let _ = previous.command_tx.send(WorkerCommand::Close);
            }
            drop(stored);
            session
                .runtime
                .close_requested
                .store(true, Ordering::Release);
            signal_routed_cancel(&session);
            let _ = session.command_tx.send(WorkerCommand::Close);
            let mut retained = reservation
                .collision
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if retained.is_none() {
                *retained = Some(session);
            } else {
                // No safe owner remains for a third session. This state is not
                // reachable through the unique claim; do not claim cleanup.
                return Err(safe_storage_error());
            }
            return Err(safe_storage_error());
        }
    } else {
        legacy_session = Some(session);
    }
    // Fence the final frontier check and installation against quarantine. No
    // wait for cleanup is performed while the output gate is held.
    let source_gate = match &prepared.routed {
        Some(source) => match source.lease.try_output() {
            Some(gate)
                if source
                    .lease
                    .check_frontier(source.manifest.frontier)
                    .is_ok() =>
            {
                Some(gate)
            }
            _ => {
                if let (Some(identity), Some(reservation)) = (&identity, &late_reservation) {
                    let fallback = &reservation.fallback;
                    PlaybackManager::suppress_late(fallback);
                    if let Ok((late_owner_id, late_handle)) =
                        playback.reserve_late_cleanup(identity, fallback)
                    {
                        drop(dispatch_owned_close(
                            playback.clone(),
                            late_owner_id,
                            &late_handle,
                        ));
                    }
                } else if let Some(session) = legacy_session.take() {
                    if let Err(cleanup_error) = spawn_session_cleanup(playback.clone(), session) {
                        eprintln!(
                            "source playback cleanup could not be scheduled: {}",
                            cleanup_error.code
                        );
                    }
                }
                return Err(playback_error(
                    "PLAYBACK_SOURCE_REVOKED",
                    "Source playback is unavailable.",
                    false,
                ));
            }
        },
        None => None,
    };
    if let Some(reservation) = &late_reservation {
        let fallback = &reservation.fallback;
        if let Err(error) = playback.install_from_fallback(lease.generation, fallback) {
            drop(source_gate);
            if let Some(identity) = &identity {
                PlaybackManager::suppress_late(fallback);
                if let Ok((late_owner_id, late_handle)) =
                    playback.reserve_late_cleanup(identity, fallback)
                {
                    drop(dispatch_owned_close(
                        playback.clone(),
                        late_owner_id,
                        &late_handle,
                    ));
                }
            }
            return Err(error);
        }
    } else if let Err(failure) = playback.install(
        lease.generation,
        legacy_session.take().ok_or_else(safe_storage_error)?,
    ) {
        drop(source_gate);
        let PlaybackInstallFailure { error, session } = *failure;
        if let Err(cleanup_error) = spawn_session_cleanup(playback.clone(), session) {
            eprintln!(
                "late playback install cleanup could not be scheduled: {}",
                cleanup_error.code
            );
        }
        return Err(error);
    }
    drop(source_gate);
    let snapshot = playback.active_snapshot(owner, &handle)?;
    let mut state = public_state(&snapshot);
    state.adoption = Some(adoption_lease.offer());
    Ok(state)
}

#[derive(Clone)]
struct ProcessOpenCleanupOwner {
    supervisor: Arc<ProcessCleanupSupervisor>,
    close_requested: Arc<AtomicBool>,
}

struct OpenDispatchGuard {
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    lease: OpenLease,
    owner: u128,
    process_cleanup: Option<ProcessOpenCleanupOwner>,
    started: bool,
    delivered: bool,
    installed_handle: Option<String>,
}

impl OpenDispatchGuard {
    #[cfg(test)]
    fn mark_started(&mut self) {
        self.started = true;
    }

    fn mark_delivered(&mut self) {
        self.delivered = true;
    }
}

impl Drop for OpenDispatchGuard {
    fn drop(&mut self) {
        if self.delivered {
            return;
        }
        self.lease.cancel.store(true, Ordering::Release);
        self.lease.cleanup.signal_close();
        if !self.started {
            // No owner thread was spawned; rollback only the exact opening.
            if self.guard.try_release_playback_token(self.guard_token) == Ok(true) {
                if let Ok(mut slot) = self.playback.inner.lock() {
                    if slot
                        .opening
                        .as_ref()
                        .is_some_and(|opening| opening.generation == self.lease.generation)
                    {
                        slot.opening = None;
                    }
                }
            }
            return;
        }
        match self.process_cleanup.take() {
            Some(process_cleanup) => {
                process_cleanup
                    .close_requested
                    .store(true, Ordering::Release);
                process_cleanup.supervisor.wake_close_nonblocking();
            }
            None => {
                #[cfg(test)]
                schedule_open_dispatch_cleanup(
                    self.playback.clone(),
                    self.owner,
                    self.lease.generation,
                    self.installed_handle.take(),
                );
                #[cfg(not(test))]
                eprintln!("playback cleanup has no process owner; admission remains retained");
            }
        }
    }
}

#[cfg(test)]
fn schedule_open_dispatch_cleanup(
    playback: PlaybackManager,
    owner: u128,
    generation: u64,
    installed_handle: Option<String>,
) {
    let cleanup = move || {
        if playback.lifecycle.generation.load(Ordering::Acquire) != generation {
            return;
        }
        let handle = match installed_handle {
            Some(handle) => Some(handle),
            None => playback.inner.lock().ok().and_then(|slot| {
                slot.active
                    .as_ref()
                    .filter(|session| session.owner == owner)
                    .map(|session| session.handle.clone())
            }),
        };
        if let Some(handle) = handle {
            if let Err(error) =
                tauri::async_runtime::block_on(dispatch_owned_close(playback, owner, &handle))
            {
                eprintln!(
                    "late playback delivery cleanup could not be completed: {}",
                    error.code
                );
            }
        }
    };
    if let Err(error) = thread::Builder::new()
        .name("fung-late-playback-delivery-cleanup".into())
        .spawn(cleanup)
    {
        eprintln!("late playback delivery cleanup could not be scheduled: {error}");
    }
}

fn try_lock_playback_slot(
    playback: &PlaybackManager,
) -> Result<Option<std::sync::MutexGuard<'_, PlaybackSlot>>, ReviewError> {
    match playback.inner.try_lock() {
        Ok(slot) => Ok(Some(slot)),
        Err(std::sync::TryLockError::Poisoned(_)) => Err(safe_storage_error()),
        Err(std::sync::TryLockError::WouldBlock) => Ok(None),
    }
}

#[cfg(test)]
fn dispatch_open_playback(
    task: OpenPlaybackTask,
) -> impl std::future::Future<Output = Result<PlaybackState, ReviewError>> {
    // Construct outside the future so dropping it before the first poll rolls
    // back the still-unstarted admission.
    let mut delivery = OpenDispatchGuard {
        playback: task.playback.clone(),
        guard: Arc::clone(&task.guard),
        guard_token: task.guard_token,
        lease: task.lease.clone(),
        owner: task.owner,
        process_cleanup: None,
        started: false,
        delivered: false,
        installed_handle: None,
    };
    async move {
        delivery.mark_started();
        let result = tauri::async_runtime::spawn_blocking(move || open_playback_owned(task))
            .await
            .map_err(|_| {
                playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native playback owner is unavailable.",
                    true,
                )
            })?;
        if result.is_ok() {
            delivery.mark_delivered();
        }
        result
    }
}

struct ProcessExecutionTask {
    claim: Option<ProcessOpenClaim>,
    supervisor: Arc<ProcessCleanupSupervisor>,
}

impl ProcessExecutionTask {
    fn run(mut self) -> Result<PlaybackState, ReviewError> {
        let claim = self.claim.take().ok_or_else(safe_storage_error)?;
        claim.execute(&self.supervisor)
    }
}

impl Drop for ProcessExecutionTask {
    fn drop(&mut self) {
        if let Some(mut claim) = self.claim.take() {
            if claim.cancel_unstarted().is_err() {
                claim.mark_terminal();
            }
        }
    }
}

fn schedule_process_execution<T>(
    execution: ProcessExecutionTask,
    schedule: impl FnOnce(Box<dyn FnOnce() -> Result<PlaybackState, ReviewError> + Send>) -> T,
) -> T {
    schedule(Box::new(move || execution.run()))
}

struct ProcessOpenDispatch {
    claim: ProcessOpenClaim,
    supervisor: Arc<ProcessCleanupSupervisor>,
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    lease: OpenLease,
    owner: u128,
}

fn dispatch_process_open(
    claim: ProcessOpenClaim,
    supervisor: Arc<ProcessCleanupSupervisor>,
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    guard_token: u64,
    lease: OpenLease,
    owner: u128,
) -> impl std::future::Future<Output = Result<PlaybackState, ReviewError>> {
    dispatch_process_open_with_scheduler(
        ProcessOpenDispatch {
            claim,
            supervisor,
            playback,
            guard,
            guard_token,
            lease,
            owner,
        },
        |job| {
            let running = tauri::async_runtime::spawn_blocking(job);
            async move {
                running.await.map_err(|_| {
                    playback_error(
                        "NATIVE_UNAVAILABLE",
                        "The native playback owner is unavailable.",
                        true,
                    )
                })
            }
        },
    )
}

fn dispatch_process_open_with_scheduler<Running>(
    dispatch: ProcessOpenDispatch,
    schedule: impl FnOnce(Box<dyn FnOnce() -> Result<PlaybackState, ReviewError> + Send>) -> Running,
) -> impl std::future::Future<Output = Result<PlaybackState, ReviewError>>
where
    Running: std::future::Future<Output = Result<Result<PlaybackState, ReviewError>, ReviewError>>
        + Send
        + 'static,
{
    dispatch_process_open_with_scheduler_hook(dispatch, schedule, |_| {})
}

fn dispatch_process_open_with_scheduler_hook<Running, Hook>(
    dispatch: ProcessOpenDispatch,
    schedule: impl FnOnce(Box<dyn FnOnce() -> Result<PlaybackState, ReviewError> + Send>) -> Running,
    after_deadline_check: Hook,
) -> impl std::future::Future<Output = Result<PlaybackState, ReviewError>>
where
    Running: std::future::Future<Output = Result<Result<PlaybackState, ReviewError>, ReviewError>>
        + Send
        + 'static,
    Hook: FnOnce(Instant) + Send + 'static,
{
    let ProcessOpenDispatch {
        claim,
        supervisor,
        playback,
        guard,
        guard_token,
        lease,
        owner,
    } = dispatch;
    let deadline = claim.deadline;
    let registry_attached = supervisor.attach_registry(&claim.registry).is_ok();
    let process_cleanup = ProcessOpenCleanupOwner {
        supervisor: Arc::clone(&supervisor),
        close_requested: Arc::clone(&claim.close_requested),
    };
    // Start outside the returned future: dropping an unpolled caller does not
    // drop the process-owned claim, and the guard cancels the generation.
    let delivery = OpenDispatchGuard {
        playback,
        guard,
        guard_token,
        lease,
        owner,
        process_cleanup: Some(process_cleanup),
        started: true,
        delivered: false,
        installed_handle: None,
    };
    let execution = if registry_attached {
        schedule_process_execution(
            ProcessExecutionTask {
                claim: Some(claim),
                supervisor: Arc::clone(&supervisor),
            },
            schedule,
        )
    } else {
        let mut claim = claim;
        if claim.cancel_unstarted().is_err() {
            claim.mark_terminal();
        }
        schedule(Box::new(|| Err(safe_storage_error())))
    };
    let running = execution;
    async move {
        let mut delivery = delivery;
        let result = match tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            running,
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                // The executor still owns its claim. Timeout only fences its
                // generation; exact owner cleanup releases admission later.
                delivery.lease.cancel.store(true, Ordering::Release);
                return Err(open_timeout_error());
            }
        };
        if let Ok(state) = &result {
            delivery.installed_handle = Some(state.handle.clone());
        }
        // Tokio polls the inner future before its timer. An executor that is
        // already ready when this caller is next scheduled can therefore win
        // after the absolute deadline; fence delivery and let Drop close any
        // exact-generation session that was installed meanwhile.
        let pre_delivery_check = Instant::now();
        if pre_delivery_check >= deadline {
            delivery.lease.cancel.store(true, Ordering::Release);
            return Err(open_timeout_error());
        }
        after_deadline_check(pre_delivery_check);
        if let Ok(state) = &result {
            let playback = delivery.playback.clone();
            let manager = loop {
                if playback.lifecycle.disposed.load(Ordering::Acquire)
                    || delivery.lease.cancel.load(Ordering::Acquire)
                {
                    return Err(playback_error(
                        "NATIVE_UNAVAILABLE",
                        "Playback was closed before delivery.",
                        true,
                    ));
                }
                if Instant::now() >= deadline {
                    delivery.lease.cancel.store(true, Ordering::Release);
                    return Err(open_timeout_error());
                }
                if let Some(manager) = try_lock_playback_slot(&playback)? {
                    break manager;
                }
                let retry_at = (Instant::now() + Duration::from_millis(1)).min(deadline);
                tokio::time::sleep_until(tokio::time::Instant::from_std(retry_at)).await;
            };
            let valid = !playback.lifecycle.disposed.load(Ordering::Acquire)
                && !delivery.lease.cancel.load(Ordering::Acquire)
                && manager.active.as_ref().is_some_and(|session| {
                    session.owner == delivery.owner && session.handle == state.handle
                });
            if !valid {
                return Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback was closed before delivery.",
                    true,
                ));
            }
            if Instant::now() >= deadline {
                delivery.lease.cancel.store(true, Ordering::Release);
                return Err(open_timeout_error());
            }
            delivery.mark_delivered();
        } else if Instant::now() >= deadline {
            delivery.lease.cancel.store(true, Ordering::Release);
            return Err(open_timeout_error());
        }
        result
    }
}

fn prepare_process_open(
    root: &Arc<ProcessPlaybackRoot>,
    playback: &PlaybackManager,
    guard: &Arc<NativeCaptureGuard>,
    io: OpenPlaybackIo,
    timeout: Duration,
    owner: u128,
    after_handoff: impl FnOnce(),
) -> Result<(ProcessOpenClaim, OpenLease, u64), ReviewError> {
    if !root.supervisor.alive.load(Ordering::Acquire) {
        return Err(safe_storage_error());
    }
    let admission = ProcessOpenAdmission::reserve(
        playback.clone(),
        Arc::clone(guard),
        Arc::clone(&root.registry),
    )?;
    let lease = admission.lease.clone();
    let token = admission.guard_token;
    admission.handoff(io, timeout, owner)?;
    after_handoff();
    let claim = match root.registry.claim_open(playback, lease.generation) {
        Ok(claim) => claim,
        Err(error) => {
            if let Err(cleanup_error) = root.registry.cancel_unclaimed(playback, lease.generation) {
                eprintln!("playback admission owner retained: {}", cleanup_error.code);
            }
            return Err(error);
        }
    };
    Ok((claim, lease, token))
}

#[tauri::command]
pub(crate) async fn desktop_playback_open_source(
    window: WebviewWindow,
    state: State<'_, AppState>,
    project_id: String,
    recording_id: String,
    source_id: String,
    capture_epoch: i64,
) -> Result<PlaybackState, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    let row =
        recording_review::validate_recording_pair(&state.genesis, &project_id, &recording_id)?;
    let root = process_playback_root()?;
    let playback = state.playback.clone();
    let guard = Arc::clone(&state.native_capture);
    let io = OpenPlaybackIo::production_source(
        Arc::clone(&state.genesis),
        state.data_root.clone(),
        row,
        source_id,
        capture_epoch,
    )?;
    let (claim, lease, guard_token) =
        prepare_process_open(root, &playback, &guard, io, MAX_OPEN_TIMEOUT, owner, || {})?;
    let result = dispatch_process_open(
        claim,
        Arc::clone(&root.supervisor),
        playback.clone(),
        guard,
        guard_token,
        lease,
        owner,
    )
    .await?;
    if !matches!(recording_review::trusted_main_owner(&window, &state), Ok(current) if current == owner)
    {
        drop(dispatch_owned_close(playback, owner, &result.handle));
        return Err(playback_error(
            "NATIVE_UNAVAILABLE",
            "The native desktop window is unavailable.",
            false,
        ));
    }
    Ok(result)
}

#[tauri::command]
pub(crate) async fn desktop_playback_open(
    window: WebviewWindow,
    state: State<'_, AppState>,
    project_id: String,
    recording_id: String,
    channel: String,
) -> Result<PlaybackState, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    let row =
        recording_review::validate_recording_pair(&state.genesis, &project_id, &recording_id)?;
    let root = process_playback_root()?;
    let playback = state.playback.clone();
    let guard = Arc::clone(&state.native_capture);
    let io = OpenPlaybackIo::production(
        Arc::clone(&state.genesis),
        state.data_root.clone(),
        row,
        channel,
    );
    let (claim, lease, guard_token) =
        prepare_process_open(root, &playback, &guard, io, MAX_OPEN_TIMEOUT, owner, || {})?;
    dispatch_process_open(
        claim,
        Arc::clone(&root.supervisor),
        playback,
        guard,
        guard_token,
        lease,
        owner,
    )
    .await
}

#[tauri::command]
pub(crate) fn desktop_playback_adopt(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
    lease: PlaybackLeaseBinding,
) -> Result<PlaybackAdoptionReceipt, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    recording_review::validate_opaque_id(&lease.token, "adoptionToken")?;
    state
        .playback
        .adopt(owner, &handle, lease.generation, &lease.token)?;
    Ok(PlaybackAdoptionReceipt { adopted: true })
}

#[tauri::command]
pub(crate) fn desktop_playback_heartbeat(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
    lease: PlaybackLeaseBinding,
) -> Result<PlaybackHeartbeatReceipt, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    recording_review::validate_opaque_id(&lease.token, "adoptionToken")?;
    state
        .playback
        .heartbeat(owner, &handle, lease.generation, &lease.token)?;
    Ok(PlaybackHeartbeatReceipt { alive: true })
}

#[tauri::command]
pub(crate) fn desktop_playback_control(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
    lease: PlaybackLeaseBinding,
    expected_epoch: u64,
    action: String,
    position_ms: Option<u64>,
) -> Result<PlaybackState, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    recording_review::validate_opaque_id(&lease.token, "adoptionToken")?;
    state
        .playback
        .validate_adopted_binding(owner, &handle, lease.generation, &lease.token)?;
    let snapshot = state
        .playback
        .control(owner, &handle, expected_epoch, &action, position_ms)?;
    Ok(public_state(&snapshot))
}

#[tauri::command]
pub(crate) fn desktop_playback_status(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
    lease: PlaybackLeaseBinding,
) -> Result<PlaybackState, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    recording_review::validate_opaque_id(&lease.token, "adoptionToken")?;
    state
        .playback
        .validate_adopted_binding(owner, &handle, lease.generation, &lease.token)?;
    let snapshot = state.playback.active_snapshot(owner, &handle)?;
    Ok(public_state(&snapshot))
}

#[tauri::command]
pub(crate) async fn desktop_playback_close(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
    lease: PlaybackLeaseBinding,
) -> Result<CloseReceipt, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    recording_review::validate_opaque_id(&lease.token, "adoptionToken")?;
    state
        .playback
        .validate_close_binding(owner, &handle, lease.generation, &lease.token)?;
    dispatch_owned_close(state.playback.clone(), owner, &handle).await?;
    Ok(CloseReceipt { closed: true })
}

fn dispatch_owned_close(
    playback: PlaybackManager,
    owner: u128,
    handle: &str,
) -> impl std::future::Future<Output = Result<(), ReviewError>> {
    dispatch_owned_close_with_spawn(playback, owner, handle, |work| {
        thread::Builder::new()
            .name("fung-owned-close".into())
            .spawn(work)
    })
}

type CloseWork = Box<dyn FnOnce() -> Result<(), ReviewError> + Send>;

fn close_active_process_request(
    playback: PlaybackManager,
    registry: std::sync::Weak<ProcessOwnerRegistry>,
    generation: u64,
    owner: u128,
    requested_handle: Option<String>,
) -> Result<(), ReviewError> {
    close_active_process_request_with_spawn(
        playback,
        registry,
        generation,
        owner,
        requested_handle,
        |work| {
            thread::Builder::new()
                .name("fung-process-cancelled-open-close".into())
                .spawn(work)
        },
    )
}

fn close_active_process_request_with_spawn(
    playback: PlaybackManager,
    registry: std::sync::Weak<ProcessOwnerRegistry>,
    generation: u64,
    owner: u128,
    requested_handle: Option<String>,
    mut spawn: impl FnMut(CloseWork) -> std::io::Result<JoinHandle<Result<(), ReviewError>>>,
) -> Result<(), ReviewError> {
    let Some(registry) = registry.upgrade() else {
        return Ok(());
    };
    let root_matches = registry
        .slot
        .lock()
        .map_err(|_| safe_storage_error())?
        .as_ref()
        .is_some_and(|slot| {
            slot.generation == generation && Arc::ptr_eq(&slot.manager.inner, &playback.inner)
        });
    if !root_matches {
        return Ok(());
    }

    let handle = match playback.inner.lock() {
        Ok(slot) => {
            if playback.lifecycle.generation.load(Ordering::Acquire) != generation {
                return Ok(());
            }
            if let Some(session) = slot.active.as_ref().filter(|session| {
                session.owner == owner
                    && requested_handle
                        .as_ref()
                        .is_none_or(|handle| handle == &session.handle)
            }) {
                Some(session.handle.clone())
            } else if let Some(closing) = slot.closing.as_ref().filter(|closing| {
                closing.owner == owner
                    && requested_handle
                        .as_ref()
                        .is_none_or(|handle| handle == &closing.handle)
            }) {
                (closing.phase == ClosingPhase::Retryable).then(|| closing.handle.clone())
            } else {
                None
            }
        }
        Err(_) => {
            mark_process_cleanup_terminal(&registry, &playback, generation);
            return Err(safe_storage_error());
        }
    };
    let Some(handle) = handle else {
        return Ok(());
    };

    let mut attempt = 0;
    loop {
        attempt += 1;
        let spawn_attempt = &mut spawn;
        let result = tauri::async_runtime::block_on(dispatch_owned_close_with_spawn(
            playback.clone(),
            owner,
            &handle,
            spawn_attempt,
        ));
        match result {
            Ok(()) => return Ok(()),
            Err(error) if error.retryable && attempt < 3 => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => {
                mark_process_cleanup_terminal(&registry, &playback, generation);
                return Err(error);
            }
        }
    }
}

fn mark_process_cleanup_terminal(
    registry: &ProcessOwnerRegistry,
    playback: &PlaybackManager,
    generation: u64,
) {
    if let Ok(mut root) = registry.slot.lock() {
        if let Some(slot) = root.as_mut().filter(|slot| {
            slot.generation == generation && Arc::ptr_eq(&slot.manager.inner, &playback.inner)
        }) {
            slot.phase = ProcessOwnerPhase::Terminal;
        }
    }
}

fn dispatch_owned_close_once(
    playback: PlaybackManager,
    owner: u128,
    handle: &str,
    expected: CloseRetryIdentity,
) -> impl std::future::Future<Output = Result<(), ReviewError>> {
    dispatch_owned_close_impl(playback, owner, handle, false, Some(expected), |work| {
        thread::Builder::new()
            .name("fung-owned-close-retry".into())
            .spawn(work)
    })
}

fn dispatch_owned_close_with_spawn(
    playback: PlaybackManager,
    owner: u128,
    handle: &str,
    spawn: impl FnOnce(CloseWork) -> std::io::Result<JoinHandle<Result<(), ReviewError>>>,
) -> impl std::future::Future<Output = Result<(), ReviewError>> {
    dispatch_owned_close_impl(playback, owner, handle, true, None, spawn)
}

fn dispatch_owned_close_impl(
    playback: PlaybackManager,
    owner: u128,
    handle: &str,
    auto_retry: bool,
    expected: Option<CloseRetryIdentity>,
    spawn: impl FnOnce(CloseWork) -> std::io::Result<JoinHandle<Result<(), ReviewError>>>,
) -> impl std::future::Future<Output = Result<(), ReviewError>> {
    let launch = (|| {
        let Some(capsule) =
            playback.reserve_owned_close_checked(owner, handle, expected.as_ref())?
        else {
            return Ok(None);
        };
        let identity = capsule
            .lock()
            .map_err(|_| safe_storage_error())?
            .as_ref()
            .ok_or_else(safe_storage_error)?
            .process_identity
            .clone();
        let retry_identity = identity.clone();
        let work_capsule = Arc::clone(&capsule);
        let work_manager = playback.clone();
        let spawned = spawn(Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                {
                    let mut slot = work_manager
                        .inner
                        .lock()
                        .map_err(|_| safe_storage_error())?;
                    let closing = slot
                        .closing
                        .as_mut()
                        .filter(|c| Arc::ptr_eq(&c.capsule, &work_capsule))
                        .ok_or_else(safe_storage_error)?;
                    closing.phase = ClosingPhase::Running;
                }
                let session = work_capsule
                    .lock()
                    .map_err(|_| safe_storage_error())?
                    .take()
                    .ok_or_else(safe_storage_error)?;
                cleanup_session(session)
            }))
            .unwrap_or_else(|_| {
                Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback cleanup failed.",
                    false,
                ))
            });
            let mut slot = work_manager
                .inner
                .lock()
                .map_err(|_| safe_storage_error())?;
            let closing = slot
                .closing
                .as_mut()
                .filter(|c| Arc::ptr_eq(&c.capsule, &work_capsule))
                .ok_or_else(safe_storage_error)?;
            if result.is_ok() {
                let handle = closing.handle.clone();
                let owner = closing.owner;
                let adoption = closing.adoption.clone();
                // Hold the process root through the closed record commit.
                macro_rules! terminal_on_error {
                    ($expression:expr) => {
                        match $expression {
                            Ok(value) => value,
                            Err(error) => {
                                closing.phase = ClosingPhase::Terminal;
                                return Err(error);
                            }
                        }
                    };
                }
                let registry = match &identity {
                    Some(identity) => Some(terminal_on_error!(identity
                        .registry
                        .upgrade()
                        .ok_or_else(safe_storage_error))),
                    None => None,
                };
                let mut root = match &registry {
                    Some(registry) => Some(terminal_on_error!(registry
                        .slot
                        .lock()
                        .map_err(|_| safe_storage_error()))),
                    None => None,
                };
                if let (Some(identity), Some(root)) = (&identity, root.as_ref()) {
                    let task =
                        terminal_on_error!(identity.task.upgrade().ok_or_else(safe_storage_error));
                    let valid = root.as_ref().is_some_and(|root_owner| {
                        root_owner.generation == identity.generation
                            && Arc::ptr_eq(&root_owner.manager.inner, &work_manager.inner)
                            && root_owner.phase == ProcessOwnerPhase::Reaping
                            && root_owner
                                .pending_open
                                .as_ref()
                                .is_some_and(|pending| Arc::ptr_eq(pending, &task))
                            && root_owner
                                .reaping
                                .as_ref()
                                .is_some_and(|pending| Arc::ptr_eq(pending, &work_capsule))
                            && root_owner.prepare_owner.status() == PrepareStatus::Joined
                            && root_owner.worker_owner.status() == PrepareStatus::Joined
                    });
                    if !valid {
                        closing.phase = ClosingPhase::Terminal;
                        return Err(safe_storage_error());
                    }
                }
                slot.closing = None;
                if slot.closed.len() >= 16 {
                    if let Some(oldest) = slot.closed.keys().next().cloned() {
                        slot.closed.remove(&oldest);
                    }
                }
                slot.closed
                    .insert(handle, ClosedPlayback { owner, adoption });
                if let Some(root) = root.as_mut() {
                    root.take();
                }
            } else {
                closing.phase = ClosingPhase::Terminal;
            }
            result
        }));
        match spawned {
            Ok(join) => Ok(Some(join)),
            Err(_) => {
                let schedule_retry = {
                    let mut slot = playback.inner.lock().map_err(|_| safe_storage_error())?;
                    if let Some(closing) = slot
                        .closing
                        .as_mut()
                        .filter(|c| Arc::ptr_eq(&c.capsule, &capsule))
                    {
                        closing.phase = ClosingPhase::Retryable;
                        if auto_retry
                            && playback.lifecycle.disposed.load(Ordering::Acquire)
                            && !closing.retry_enqueued
                            && retry_identity.is_some()
                        {
                            closing.retry_enqueued = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                if schedule_retry {
                    if let Some(identity) = retry_identity {
                        let queued = identity.registry.upgrade().and_then(|registry| {
                            let supervisor = {
                                let weak = registry.supervisor.lock().ok()?;
                                weak.as_ref()?.upgrade()?
                            };
                            Some(supervisor.submit_job(ProcessCleanupJob::Close {
                                manager: playback.clone(),
                                registry: Arc::downgrade(&registry),
                                generation: identity.generation,
                                owner,
                                handle: handle.to_owned(),
                                capsule: Arc::clone(&capsule),
                            }))
                        });
                        if !matches!(queued, Some(Ok(()))) {
                            eprintln!("playback shutdown cleanup remains pending");
                        }
                    }
                }
                Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "Playback cleanup could not be scheduled.",
                    true,
                ))
            }
        }
    })();
    async move {
        if let Some(join) = launch? {
            tauri::async_runtime::spawn_blocking(move || join.join())
                .await
                .map_err(|_| {
                    playback_error("NATIVE_UNAVAILABLE", "Playback cleanup wait failed.", true)
                })?
                .map_err(|_| {
                    playback_error("NATIVE_UNAVAILABLE", "Playback cleanup failed.", false)
                })??;
        }
        Ok(())
    }
}

#[cfg(test)]
fn dispatch_close_playback(
    playback: PlaybackManager,
    session: PlaybackSession,
) -> impl std::future::Future<Output = Result<(), ReviewError>> {
    // A future can be dropped without ever being polled. Signal before creating
    // it so cancellation still suppresses audio in that case.
    signal_routed_cancel(&session);
    async move {
        let session_handle = session.handle.clone();
        let session_owner = session.owner;
        let adoption = session
            .runtime
            .adoption_lease
            .get()
            .map(|lease| lease.offer());
        tauri::async_runtime::spawn_blocking(move || {
            cleanup_session(session)?;
            playback.mark_closed(session_handle, session_owner, adoption);
            Ok::<(), ReviewError>(())
        })
        .await
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback cleanup owner is unavailable.",
                true,
            )
        })??;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // Shared by source-custody regression fixtures; exercises preparation without
    // creating a CPAL stream or accessing any output device.
    pub(crate) fn prepare_source_without_output(
        storage: &genesis_block_native::Storage,
        data_root: &Path,
        row: &RecordingStorageRow,
    ) -> Result<(), ReviewError> {
        prepare_playback(
            storage,
            data_root,
            row,
            "file".into(),
            Arc::new(AtomicBool::new(false)),
        )
        .map(|_| ())
    }

    #[test]
    fn output_rate_falls_back_to_a_device_supported_rate() {
        assert_eq!(output_rate_for_range(16_000, 44_100, 44_100), Some(44_100));
        assert_eq!(output_rate_for_range(48_000, 44_100, 48_000), Some(48_000));
        assert_eq!(output_rate_for_range(16_000, 48_000, 96_000), Some(48_000));
        assert_eq!(output_rate_for_range(16_000, 0, 0), None);
    }

    #[test]
    fn resampler_interpolates_and_keeps_source_timeline_rate() {
        let mut queue = VecDeque::new();
        for sample in 0..200_i16 {
            queue.extend([sample * 100, sample * 100]);
        }
        let mut phase = 0_u64;
        let mut consumed = 0_u64;
        for _ in 0..441 {
            let (_, source_frames) =
                next_resampled_frame(&mut queue, &mut phase, 16_000, 44_100, false).unwrap();
            consumed += source_frames;
        }
        assert_eq!(consumed, 160);
        assert_eq!(phase, 0);
    }

    #[test]
    fn resampler_emits_linear_midpoint_and_drains_final_source_frame() {
        let mut queue = VecDeque::from([0_i16, 0, 1_000, 1_000]);
        let mut phase = 0_u64;
        let (first, consumed) =
            next_resampled_frame(&mut queue, &mut phase, 22_050, 44_100, false).unwrap();
        assert_eq!(first, [0, 0]);
        assert_eq!(consumed, 0);

        let (midpoint, consumed) =
            next_resampled_frame(&mut queue, &mut phase, 22_050, 44_100, true).unwrap();
        assert_eq!(midpoint, [500, 500]);
        assert_eq!(consumed, 1);
        assert_eq!(queue, VecDeque::from([1_000_i16, 1_000]));

        let (tail, consumed) =
            next_resampled_frame(&mut queue, &mut phase, 22_050, 44_100, true).unwrap();
        assert_eq!(tail, [1_000, 1_000]);
        assert_eq!(consumed, 0);

        let (last, consumed) =
            next_resampled_frame(&mut queue, &mut phase, 22_050, 44_100, true).unwrap();
        assert_eq!(last, [1_000, 1_000]);
        assert_eq!(consumed, 1);
        assert!(queue.is_empty());
    }

    #[test]
    #[ignore = "requires the Windows default audio output device"]
    fn native_output_resampling_smoke_plays_and_resumes_a_local_fixture() {
        let duration_frames = 8_000_u64;
        let samples = (0..duration_frames)
            .flat_map(|frame| {
                let sample = (512.0
                    * (2.0 * std::f64::consts::PI * 440.0 * frame as f64 / 16_000.0).sin())
                    as i16;
                [sample, sample]
            })
            .collect::<Vec<_>>();
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 2,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            &samples,
        );
        let descriptor = ChunkDescriptor {
            id: "chunk-1".to_string(),
            path: path.file_name().unwrap().to_string_lossy().into_owned(),
            sequence_no: 1,
            start_ms: 0,
            end_ms: 500,
            byte_size: std::fs::metadata(&path).unwrap().len(),
            frame_start: 0,
            frame_end: duration_frames,
            available: true,
        };
        let prepared = PreparedPlayback {
            project_root: directory.path().to_path_buf(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 500,
            sample_rate: 16_000,
            source_channels: 2,
            duration_frames,
            descriptors: vec![descriptor],
            degraded: false,
            missing_ranges: Vec::new(),
            routed: None,
        };
        let runtime = Arc::new(PlaybackRuntime::new(false, Vec::new()));
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = thread::spawn({
            let runtime = Arc::clone(&runtime);
            move || {
                worker_loop(
                    prepared,
                    runtime,
                    Arc::new(AtomicBool::new(false)),
                    command_rx,
                    ready_tx,
                )
            }
        });

        ready_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("native playback worker did not become ready")
            .expect("native playback output stream could not be opened");
        command_tx.send(WorkerCommand::Play).unwrap();
        let pause_deadline = Instant::now() + Duration::from_secs(2);
        while runtime.position_frames.load(Ordering::Acquire) < 1_600
            && Instant::now() < pause_deadline
        {
            thread::sleep(Duration::from_millis(5));
        }
        command_tx.send(WorkerCommand::Pause).unwrap();
        let pause_deadline = Instant::now() + Duration::from_secs(2);
        while runtime.state.load(Ordering::Acquire) != STATE_PAUSED
            && Instant::now() < pause_deadline
        {
            thread::sleep(Duration::from_millis(5));
        }
        let paused_position = runtime.position_frames.load(Ordering::Acquire);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            runtime.position_frames.load(Ordering::Acquire),
            paused_position,
            "paused playback position continued advancing"
        );

        let resumed_at = Instant::now();
        command_tx.send(WorkerCommand::Play).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while runtime.state.load(Ordering::Acquire) != STATE_ENDED
            && runtime.state.load(Ordering::Acquire) != STATE_ERROR
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        let state = runtime.state.load(Ordering::Acquire);
        let position = runtime.position_frames.load(Ordering::Acquire);
        let _ = command_tx.send(WorkerCommand::Close);
        join.join().unwrap();

        assert_eq!(state, STATE_ENDED, "native playback did not finish");
        assert_eq!(position, duration_frames);
        assert!(paused_position >= 1_600 && paused_position < duration_frames);
        assert!(
            resumed_at.elapsed() >= Duration::from_millis(200),
            "playback skipped the buffered source after resume"
        );
    }

    fn test_session(
        owner: u128,
        handle: &str,
        guard: Arc<NativeCaptureGuard>,
        join: Option<JoinHandle<()>>,
    ) -> PlaybackSession {
        let (command_tx, _command_rx) = mpsc::channel();
        let guard_token = guard
            .active_playback_token_for_test()
            .unwrap_or_else(|| guard.try_open_playback_token().unwrap());
        PlaybackSession {
            process_identity: None,
            guard_token,
            owner,
            handle: handle.to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            duration_frames: 8_000,
            sample_rate: 8_000,
            runtime: Arc::new(PlaybackRuntime::new(false, Vec::new())),
            command_tx,
            join: join.map(WorkerJoin::Direct),
            guard,
        }
    }

    #[test]
    fn close_binding_is_exact_during_retryable_close_and_after_closed_receipt() {
        let manager = PlaybackManager::new();
        let opening = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (command_tx, command_rx) = mpsc::channel();
        let (close_seen_tx, close_seen_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            if matches!(command_rx.recv(), Ok(WorkerCommand::Close)) {
                close_seen_tx.send(()).unwrap();
            }
        });
        let mut session = test_session(17, "lease-bound-close", Arc::clone(&guard), Some(worker));
        session.command_tx = command_tx;
        let lease = Arc::new(PlaybackAdoptionLease::new_at(
            opening.generation,
            session.handle.clone(),
            Arc::clone(&session.runtime.close_requested),
            PlaybackVisibility::default(),
            playback_lease_tick_ms(),
        ));
        let offer = lease.offer();
        session.runtime.adoption_lease.set(lease).unwrap();
        manager.install(opening.generation, session).unwrap();

        assert!(manager
            .validate_close_binding(17, "lease-bound-close", offer.generation, &offer.token)
            .is_ok());
        assert_eq!(
            manager
                .validate_close_binding(17, "lease-bound-close", offer.generation + 1, &offer.token)
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
        assert_eq!(
            manager
                .validate_close_binding(17, "lease-bound-close", offer.generation, "wrong-token")
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
        let spawn_error = tauri::async_runtime::block_on(dispatch_owned_close_with_spawn(
            manager.clone(),
            17,
            "lease-bound-close",
            |_| Err(std::io::Error::other("injected close spawn failure")),
        ))
        .unwrap_err();
        assert!(spawn_error.retryable);
        assert!(manager
            .validate_close_binding(17, "lease-bound-close", offer.generation, &offer.token)
            .is_ok());
        assert_eq!(
            manager
                .validate_close_binding(17, "lease-bound-close", offer.generation + 1, &offer.token)
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
        assert_eq!(
            manager
                .validate_close_binding(17, "lease-bound-close", offer.generation, "wrong-token")
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
        tauri::async_runtime::block_on(dispatch_owned_close(
            manager.clone(),
            17,
            "lease-bound-close",
        ))
        .unwrap();
        close_seen_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(manager
            .validate_close_binding(17, "lease-bound-close", offer.generation, &offer.token)
            .is_ok());
        assert_eq!(
            manager
                .validate_close_binding(17, "lease-bound-close", offer.generation + 1, &offer.token)
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
        assert_eq!(
            manager
                .validate_close_binding(17, "lease-bound-close", offer.generation, "wrong-token")
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
        assert_eq!(
            manager
                .validate_close_binding(18, "lease-bound-close", offer.generation, &offer.token)
                .unwrap_err()
                .code,
            "PLAYBACK_HANDLE_INVALID"
        );
    }

    fn temp_wave(spec: hound::WavSpec, samples: &[i16]) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mic-00001.wav");
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
        (directory, path)
    }

    #[cfg(windows)]
    fn safe_fixture_path(path: Option<&Path>, canonical_root: Option<&Path>) -> String {
        let Some(path) = path else {
            return "<unavailable>".to_string();
        };
        let canonical_path = std::fs::canonicalize(path).ok();
        let inside_by_spelling =
            canonical_root.is_some_and(|root| normalized_path_inside(path, root));
        let inside_by_canonical = canonical_path
            .as_deref()
            .zip(canonical_root)
            .is_some_and(|(candidate, root)| normalized_path_inside(candidate, root));
        if inside_by_spelling || inside_by_canonical {
            path.display().to_string()
        } else {
            "<redacted-outside-fixture>".to_string()
        }
    }

    #[cfg(windows)]
    fn fixture_path_inside_case_insensitive(path: &Path, root: &Path) -> bool {
        fn comparable(path: &Path) -> String {
            let value = path.to_string_lossy();
            value
                .strip_prefix(r"\\?\")
                .unwrap_or(&value)
                .replace('/', "\\")
                .to_ascii_lowercase()
        }

        let path = comparable(path);
        let root = comparable(root);
        path == root || path.starts_with(&(root + "\\"))
    }

    #[cfg(windows)]
    fn emit_fixture_path_diagnostic(
        fixture_root: &Path,
        descriptor: &ChunkDescriptor,
        observed_error: Option<&ReviewError>,
    ) {
        let canonical_root = std::fs::canonicalize(fixture_root).ok();
        let observed_code = observed_error
            .map(|error| error.code.as_str())
            .unwrap_or("<none>");
        let mut stage = "not_started".to_string();
        let mut reject_status: String;
        let mut open_status = "not_run".to_string();
        let mut metadata_status = "not_run".to_string();
        let mut metadata_is_file = None;
        let mut actual_size = None;
        let mut final_path_status = "not_run".to_string();
        let mut resolved_candidate = None;
        let mut final_handle_path = None;

        if let Err(error) = reject_untrusted_path(&descriptor.path) {
            reject_status = format!("error:{}", error.code);
            stage = "reject_untrusted_path".to_string();
        } else {
            reject_status = "pass".to_string();
            match resolve_ledger_path(fixture_root, &descriptor.path) {
                Ok(candidate) => {
                    resolved_candidate = Some(candidate);
                    let candidate = resolved_candidate.as_ref().expect("candidate recorded");
                    match File::open(candidate) {
                        Ok(file) => {
                            open_status = "pass".to_string();
                            match file.metadata() {
                                Ok(metadata) => {
                                    metadata_is_file = Some(metadata.is_file());
                                    actual_size = Some(metadata.len());
                                    if !metadata.is_file() || metadata.len() != descriptor.byte_size
                                    {
                                        metadata_status = "mismatch".to_string();
                                        stage = "metadata".to_string();
                                    } else {
                                        metadata_status = "pass".to_string();
                                        match final_path_from_handle(&file, candidate) {
                                            Ok(path) => {
                                                final_path_status = "pass".to_string();
                                                final_handle_path = Some(path);
                                            }
                                            Err(error) => {
                                                final_path_status = format!("error:{}", error.code);
                                                stage = "finalpath".to_string();
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    metadata_status = "error".to_string();
                                    stage = "metadata".to_string();
                                }
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            open_status = "not_found".to_string();
                            stage = "open".to_string();
                        }
                        Err(error) => {
                            open_status = format!("error:{:?}", error.kind());
                            stage = "open".to_string();
                        }
                    }
                }
                Err(error) => {
                    stage = "resolve".to_string();
                    reject_status = format!("resolve_error:{}", error.code);
                }
            }
        }

        let canonical_candidate = resolved_candidate
            .as_deref()
            .and_then(|candidate| std::fs::canonicalize(candidate).ok());
        let canonical_final_handle = final_handle_path
            .as_deref()
            .and_then(|path| std::fs::canonicalize(path).ok());
        let final_inside_raw_root = final_handle_path
            .as_deref()
            .map(|path| normalized_path_inside(path, fixture_root));
        let final_inside_canonical_root = final_handle_path
            .as_deref()
            .zip(canonical_root.as_deref())
            .map(|(path, root)| normalized_path_inside(path, root));
        let canonical_final_inside_canonical_root = canonical_final_handle
            .as_deref()
            .zip(canonical_root.as_deref())
            .map(|(path, root)| normalized_path_inside(path, root));
        let candidate_inside_raw_root = resolved_candidate
            .as_deref()
            .map(|path| normalized_path_inside(path, fixture_root));
        let canonical_candidate_inside_canonical_root = canonical_candidate
            .as_deref()
            .zip(canonical_root.as_deref())
            .map(|(path, root)| normalized_path_inside(path, root));
        let final_inside_raw_root_case_insensitive = final_handle_path
            .as_deref()
            .map(|path| fixture_path_inside_case_insensitive(path, fixture_root));
        let final_inside_canonical_root_case_insensitive = final_handle_path
            .as_deref()
            .zip(canonical_root.as_deref())
            .map(|(path, root)| fixture_path_inside_case_insensitive(path, root));
        if stage == "not_started" && final_inside_raw_root == Some(false) {
            stage = "containment".to_string();
        }
        if stage == "not_started" {
            stage = "post_custody_or_other".to_string();
        }

        eprintln!(
            "desktop_playback test-only Windows fixture path diagnostic: observed_code={observed_code}; stage={stage}; raw_fixture_root={}; resolved_candidate={}; final_handle_path={}; canonical_root={}; reject_untrusted_path={reject_status}; open={open_status}; metadata={metadata_status}; metadata_is_file={metadata_is_file:?}; actual_size={actual_size:?}; expected_size={}; finalpath={final_path_status}; candidate_inside_raw_root={candidate_inside_raw_root:?}; final_inside_raw_root={final_inside_raw_root:?}; final_inside_canonical_root={final_inside_canonical_root:?}; canonical_candidate_inside_canonical_root={canonical_candidate_inside_canonical_root:?}; canonical_final_inside_canonical_root={canonical_final_inside_canonical_root:?}; final_inside_raw_root_case_insensitive={final_inside_raw_root_case_insensitive:?}; final_inside_canonical_root_case_insensitive={final_inside_canonical_root_case_insensitive:?}",
            fixture_root.display(),
            safe_fixture_path(resolved_candidate.as_deref(), canonical_root.as_deref()),
            safe_fixture_path(final_handle_path.as_deref(), canonical_root.as_deref()),
            safe_fixture_path(canonical_root.as_deref(), canonical_root.as_deref()),
            descriptor.byte_size,
        );
    }

    #[cfg(windows)]
    fn report_fixture_path_failure_if_needed<T>(
        fixture_root: &Path,
        descriptor: &ChunkDescriptor,
        result: &Result<T, ReviewError>,
        expected_behavior: bool,
    ) {
        if !expected_behavior {
            emit_fixture_path_diagnostic(fixture_root, descriptor, result.as_ref().err());
        }
    }

    #[test]
    fn frame_math_respects_millisecond_timestamp_precision() {
        assert_eq!(frames_for_ms(999, 48_000), 47_952);
        assert_eq!(ms_for_frames(frames_for_ms(999, 48_000), 48_000), 999);
        assert!(within_millisecond_precision(48_001, 1_000, 48_000));
        assert!(within_millisecond_precision(231_722, 14_483, 16_000));
        assert!(within_millisecond_precision(231_720, 14_483, 16_000));
        assert!(!within_millisecond_precision(231_719, 14_483, 16_000));
    }

    #[test]
    fn imported_pcm16_duration_rounding_is_validated_at_millisecond_precision() {
        let frames = 231_722_u32;
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            &vec![1_i16; frames as usize],
        );
        let project_root =
            std::fs::canonicalize(directory.path()).expect("fixture project root canonicalization");
        let descriptor = ChunkDescriptor {
            id: "rounded-import".to_string(),
            path: path.display().to_string(),
            sequence_no: 1,
            start_ms: 0,
            end_ms: 14_483,
            byte_size: std::fs::metadata(&path).unwrap().len(),
            frame_start: 0,
            frame_end: frames_for_ms(14_483, 16_000),
            available: true,
        };

        let wave = validate_wave(&project_root, &descriptor)
            .unwrap()
            .expect("rounded whole-file import is a valid source");
        assert_eq!(wave.frames, u64::from(frames));
        let source = open_source(&project_root, &descriptor)
            .unwrap()
            .expect("rounded whole-file import can be opened");
        assert_eq!(source.frame_count, u64::from(frames));
        assert_eq!(source.frame_count, 231_722);
        assert_eq!(frames_for_ms(descriptor.end_ms, 16_000), 231_728);
    }

    #[test]
    fn rounded_import_reader_pads_after_last_pcm_frame_without_reading_past_end() {
        let frames = 231_722_u32;
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            &vec![17_i16; frames as usize],
        );
        let project_root =
            std::fs::canonicalize(directory.path()).expect("fixture project root canonicalization");
        let descriptor = ChunkDescriptor {
            id: "rounded-import-tail".to_string(),
            path: path.display().to_string(),
            sequence_no: 1,
            start_ms: 0,
            end_ms: 14_483,
            byte_size: std::fs::metadata(&path).unwrap().len(),
            frame_start: 0,
            frame_end: frames_for_ms(14_483, 16_000),
            available: true,
        };
        let mut source = open_source(&project_root, &descriptor)
            .unwrap()
            .expect("rounded whole-file import can be opened");

        source
            .reader
            .seek(frames - 1)
            .expect("seek to final PCM frame");
        source.next_frame = u64::from(frames - 1);
        assert_eq!(read_source_frame(&mut source).unwrap(), [17, 17]);
        assert_eq!(source.next_frame, u64::from(frames));
        assert_eq!(read_source_frame(&mut source).unwrap(), [0, 0]);
        assert_eq!(source.next_frame, u64::from(frames));
    }

    #[test]
    fn unsupported_wav_is_rejected_before_worker_creation() {
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 8,
                sample_format: hound::SampleFormat::Int,
            },
            &[0; 48],
        );
        let project_root =
            std::fs::canonicalize(directory.path()).expect("fixture project root canonicalization");
        let descriptor = ChunkDescriptor {
            id: "chunk".to_string(),
            path: path.display().to_string(),
            sequence_no: 1,
            start_ms: 0,
            end_ms: 1,
            byte_size: std::fs::metadata(&path).unwrap().len(),
            frame_start: 0,
            frame_end: 0,
            available: false,
        };
        let result = validate_wave(&project_root, &descriptor);
        #[cfg(windows)]
        report_fixture_path_failure_if_needed(
            &project_root,
            &descriptor,
            &result,
            matches!(result.as_ref(), Err(error) if error.code == "PLAYBACK_FORMAT_UNSUPPORTED"),
        );
        assert_eq!(result.unwrap_err().code, "PLAYBACK_FORMAT_UNSUPPORTED");
    }

    #[test]
    fn valid_pcm16_source_is_validated_from_the_custodied_file() {
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 1,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            &[1; 80],
        );
        let project_root =
            std::fs::canonicalize(directory.path()).expect("fixture project root canonicalization");
        let descriptor = ChunkDescriptor {
            id: "chunk".to_string(),
            path: path.display().to_string(),
            sequence_no: 1,
            start_ms: 0,
            end_ms: 10,
            byte_size: std::fs::metadata(&path).unwrap().len(),
            frame_start: 0,
            frame_end: 80,
            available: true,
        };
        let result = validate_wave(&project_root, &descriptor);
        #[cfg(windows)]
        report_fixture_path_failure_if_needed(
            &project_root,
            &descriptor,
            &result,
            matches!(result.as_ref(), Ok(Some(_))),
        );
        let wave = result.unwrap().unwrap();
        assert_eq!(wave.spec.sample_rate, 8_000);
        assert_eq!(wave.spec.channels, 1);
        assert_eq!(wave.frames, 80);
    }

    #[test]
    fn stereo_wav_duration_is_per_channel_at_supported_rates() {
        for (sample_rate, frames, duration_ms) in [
            (8_000_u32, 8_u32, 1_u64),
            (44_100, 441, 10),
            (96_000, 960, 10),
        ] {
            let samples = vec![1_i16; frames as usize * 2];
            let (directory, path) = temp_wave(
                hound::WavSpec {
                    channels: 2,
                    sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
                &samples,
            );
            let project_root = std::fs::canonicalize(directory.path())
                .expect("fixture project root canonicalization");
            let descriptor = ChunkDescriptor {
                id: "stereo".to_string(),
                path: path.display().to_string(),
                sequence_no: 1,
                start_ms: 0,
                end_ms: duration_ms,
                byte_size: std::fs::metadata(&path).unwrap().len(),
                frame_start: 0,
                frame_end: frames as u64,
                available: true,
            };
            let result = validate_wave(&project_root, &descriptor);
            #[cfg(windows)]
            report_fixture_path_failure_if_needed(
                &project_root,
                &descriptor,
                &result,
                matches!(result.as_ref(), Ok(Some(_))),
            );
            let wave = result.unwrap().unwrap();
            assert_eq!(wave.frames, frames as u64);
            let source_result = open_source(&project_root, &descriptor);
            #[cfg(windows)]
            report_fixture_path_failure_if_needed(
                &project_root,
                &descriptor,
                &source_result,
                matches!(source_result.as_ref(), Ok(Some(_))),
            );
            let source = source_result.unwrap().unwrap();
            assert_eq!(source.reader.duration(), frames);
        }
    }

    fn open_storage() -> (tempfile::TempDir, genesis_block_native::Storage) {
        let directory = tempfile::tempdir().unwrap();
        let storage_path = directory.path().join("genesis");
        let storage = genesis_block_native::Storage::open(genesis_block_native::OpenOptions {
            path: storage_path.display().to_string(),
            page_cache_mb: Some(8),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap();
        genesis_adapter::install(&storage).unwrap();
        (directory, storage)
    }

    fn recording_row(
        project_id: &str,
        recording_id: &str,
        duration_ms: i64,
    ) -> RecordingStorageRow {
        RecordingStorageRow {
            identity: recording_review::RecordingIdentity {
                project_id: project_id.to_string(),
                recording_id: recording_id.to_string(),
            },
            source: "import".to_string(),
            status: "completed".to_string(),
            duration_ms,
            created_at: "2026-09-17T00:00:00Z".to_string(),
            updated_at: "2026-09-17T00:00:00Z".to_string(),
            language: None,
        }
    }

    fn prepared_fixture() -> PreparedPlayback {
        PreparedPlayback {
            project_root: PathBuf::new(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1,
            sample_rate: 8_000,
            source_channels: 1,
            duration_frames: 8,
            descriptors: Vec::new(),
            degraded: false,
            missing_ranges: Vec::new(),
            routed: None,
        }
    }

    type TestReadySender = mpsc::Sender<Result<(), ReviewError>>;

    #[derive(Clone)]
    struct WorkerHarness {
        ready_tx: Arc<Mutex<Option<TestReadySender>>>,
        worker_started_tx: mpsc::Sender<()>,
        close_seen_tx: mpsc::Sender<()>,
        allow_join: Arc<AtomicBool>,
    }

    fn worker_harness() -> (WorkerHarness, mpsc::Receiver<()>, mpsc::Receiver<()>) {
        let (worker_started_tx, worker_started_rx) = mpsc::channel();
        let (close_seen_tx, close_seen_rx) = mpsc::channel();
        (
            WorkerHarness {
                ready_tx: Arc::new(Mutex::new(None)),
                worker_started_tx,
                close_seen_tx,
                allow_join: Arc::new(AtomicBool::new(false)),
            },
            worker_started_rx,
            close_seen_rx,
        )
    }

    fn worker_factory(harness: WorkerHarness) -> WorkerFn {
        Arc::new(move |_prepared, _runtime, _cancel, owner| {
            let (command_tx, command_rx) = mpsc::channel();
            let (ready_tx, ready_rx) = mpsc::channel();
            *harness
                .ready_tx
                .lock()
                .expect("ready sender mutex poisoned") = Some(ready_tx);
            harness
                .worker_started_tx
                .send(())
                .expect("worker-started receiver dropped");
            let close_seen_tx = harness.close_seen_tx.clone();
            let allow_join = Arc::clone(&harness.allow_join);
            let job = Box::new(move || {
                if matches!(command_rx.recv(), Ok(WorkerCommand::Close)) {
                    close_seen_tx.send(()).expect("close-seen receiver dropped");
                    while !allow_join.load(Ordering::Acquire) {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            });
            launch_worker_thread(command_tx, ready_rx, owner, job, |work| {
                thread::Builder::new().spawn(work)
            })
        })
    }

    fn release_ready(harness: &WorkerHarness) {
        let ready_tx = harness
            .ready_tx
            .lock()
            .expect("ready sender mutex poisoned")
            .take()
            .expect("worker readiness sender missing");
        let _ = ready_tx.send(Ok(()));
    }

    fn gated_prepare(started_tx: mpsc::Sender<()>, release: Arc<AtomicBool>) -> PrepareFn {
        Box::new(move |_cancel| {
            started_tx
                .send(())
                .expect("prepare-started receiver dropped");
            while !release.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            Ok(prepared_fixture())
        })
    }

    fn admitted_task(
        playback: PlaybackManager,
        guard: Arc<NativeCaptureGuard>,
        io: OpenPlaybackIo,
        open_timeout: Duration,
    ) -> OpenPlaybackTask {
        let lease = playback.begin_open().unwrap();
        let guard_token = guard.try_open_playback_token().unwrap();
        OpenPlaybackTask {
            playback,
            guard,
            guard_token,
            lease,
            io,
            deadline: Instant::now() + open_timeout,
            owner: 17,
            prepare_owner: None,
            worker_owner: None,
            process_registry: None,
            process_task: None,
            late_owner: None,
            late_collision: None,
        }
    }

    struct ProcessOwnedSessionFixture {
        manager: PlaybackManager,
        guard: Arc<NativeCaptureGuard>,
        registry: Arc<ProcessOwnerRegistry>,
        generation: u64,
        handle: String,
        runtime: Arc<PlaybackRuntime>,
        cleanup: Arc<OpenAttemptCleanup>,
        close_seen: mpsc::Receiver<()>,
        open_claim: ProcessOpenClaim,
    }

    fn process_owned_session_fixture() -> ProcessOwnedSessionFixture {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let guard_token = admission.guard_token;
        let cleanup = Arc::clone(&admission.lease.cleanup);
        let (command_tx, command_rx) = mpsc::channel();
        let (close_seen_tx, close_seen) = mpsc::channel();
        let worker = thread::spawn(move || {
            if matches!(command_rx.recv(), Ok(WorkerCommand::Close)) {
                let _ = close_seen_tx.send(());
            }
        });
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: Arc::new(|_, _, _, _| {
                        Err(WorkerStartFailure::NotStarted(safe_storage_error()))
                    }),
                },
                Duration::from_secs(2),
                17,
            )
            .unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let identity = ProcessPlaybackIdentity {
            registry: task.process_registry.clone().unwrap(),
            task: task.process_task.clone().unwrap(),
            generation,
        };
        let runtime = Arc::new(PlaybackRuntime::new(false, Vec::new()));
        cleanup.runtime.set(Arc::clone(&runtime)).unwrap();
        cleanup.worker_close.set(command_tx.clone()).unwrap();
        {
            let root = registry.slot.lock().unwrap();
            let owner = root.as_ref().unwrap();
            owner.prepare_owner.handle.lock().unwrap().status = PrepareStatus::Joined;
            owner.worker_owner.handle.lock().unwrap().status = PrepareStatus::Joined;
        }
        let handle = "process-owned-cleanup".to_string();
        manager
            .install(
                generation,
                PlaybackSession {
                    process_identity: Some(identity),
                    guard_token,
                    owner: 17,
                    handle: handle.clone(),
                    project_id: "p1".into(),
                    recording_id: "r1".into(),
                    channel: "file".into(),
                    duration_ms: 1,
                    duration_frames: 8,
                    sample_rate: 8_000,
                    runtime: Arc::clone(&runtime),
                    command_tx,
                    join: Some(WorkerJoin::Direct(worker)),
                    guard: Arc::clone(&guard),
                },
            )
            .unwrap();
        drop(task);
        ProcessOwnedSessionFixture {
            manager,
            guard,
            registry,
            generation,
            handle,
            runtime,
            cleanup,
            close_seen,
            open_claim: claim,
        }
    }

    fn spawn_open_dispatch(
        task: OpenPlaybackTask,
    ) -> thread::JoinHandle<Result<PlaybackState, ReviewError>> {
        thread::spawn(move || tauri::async_runtime::block_on(dispatch_open_playback(task)))
    }

    fn spawn_close_dispatch(
        playback: PlaybackManager,
        session: PlaybackSession,
    ) -> thread::JoinHandle<Result<(), ReviewError>> {
        thread::spawn(move || {
            tauri::async_runtime::block_on(dispatch_close_playback(playback, session))
        })
    }

    fn wait_for_guard_release(guard: &NativeCaptureGuard) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while guard.playback_open() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            !guard.playback_open(),
            "owned cleanup did not release admission"
        );
    }

    #[test]
    fn dropped_unpolled_legacy_close_still_has_cleanup_owner() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let got_close = matches!(command_rx.recv(), Ok(WorkerCommand::Close));
            closed_tx.send(got_close).unwrap();
        });
        let mut session = test_session(17, "unpolled-legacy", Arc::clone(&guard), Some(worker));
        session.command_tx = command_tx;
        manager.install_test(session);
        drop(dispatch_owned_close(manager.clone(), 17, "unpolled-legacy"));
        assert!(
            closed_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            "cleanup owner must deliver close"
        );
        wait_for_guard_release(&guard);
        assert!(manager
            .take_for_close(17, "unpolled-legacy")
            .unwrap()
            .is_none());
    }

    #[test]
    fn playback_owner_dropped_unpolled_open_still_runs_cleanup() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_started_tx, prepare_started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let (harness, _started_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_started_tx, Arc::clone(&release)),
            worker: worker_factory(harness),
        };
        let task = admitted_task(
            manager.clone(),
            Arc::clone(&guard),
            io,
            Duration::from_millis(100),
        );
        let cancel = Arc::clone(&task.lease.cancel);
        drop(dispatch_open_playback(task));
        assert!(
            prepare_started_rx.try_recv().is_err(),
            "no worker must start before first poll"
        );
        assert!(cancel.load(Ordering::Acquire));
        assert!(
            !guard.playback_open(),
            "unstarted admission must be rolled back"
        );
        let next = manager.begin_open().unwrap();
        manager.abort_open(next.generation);
    }

    #[test]
    fn playback_owner_stale_unpolled_drop_does_not_release_new_guard_owner() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, _prepare_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        let task = admitted_task(manager, Arc::clone(&guard), io, Duration::from_secs(1));
        guard.release_playback();
        guard.try_open_playback().unwrap();
        drop(dispatch_open_playback(task));
        assert!(
            guard.playback_open(),
            "stale open future must not release the newer guard owner"
        );
        guard.release_playback();
    }

    #[test]
    fn playback_owner_dropped_open_after_prepare_has_no_orphaned_session() {
        use std::future::Future;
        use std::task::{Context, Poll, Waker};
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let (harness, worker_rx, _close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::clone(&release)),
            worker: worker_factory(harness.clone()),
        };
        let task = admitted_task(
            manager.clone(),
            Arc::clone(&guard),
            io,
            Duration::from_secs(2),
        );
        let mut future = Box::pin(dispatch_open_playback(task));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(future);
        release.store(true, Ordering::Release);
        if worker_rx.recv_timeout(Duration::from_secs(2)).is_ok() {
            release_ready(&harness);
        }
        wait_for_guard_release(&guard);
        let slot = manager.inner.lock().unwrap();
        assert!(
            slot.active.is_none(),
            "a dropped response must not leave an unreachable session"
        );
    }

    #[test]
    fn playback_owner_dropped_open_after_install_closes_exact_session() {
        use std::future::Future;
        use std::task::{Context, Poll, Waker};
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(true));
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, release),
            worker: worker_factory(harness.clone()),
        };
        let task = admitted_task(
            manager.clone(),
            Arc::clone(&guard),
            io,
            Duration::from_secs(2),
        );
        let mut future = Box::pin(dispatch_open_playback(task));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let deadline = Instant::now() + Duration::from_secs(2);
        while manager.inner.lock().unwrap().active.is_none() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(
            manager.inner.lock().unwrap().active.is_some(),
            "open must have installed before drop"
        );
        drop(future);
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        wait_for_guard_release(&guard);
    }

    #[test]
    fn owned_close_spawn_failure_retains_join_and_retries() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = rx.recv();
        });
        let mut session = test_session(17, "spawn-fail", Arc::clone(&guard), Some(worker));
        session.command_tx = tx;
        manager.install_test(session);
        let failed = dispatch_owned_close_with_spawn(manager.clone(), 17, "spawn-fail", |_| {
            Err(std::io::Error::other("injected spawn failure"))
        });
        assert!(tauri::async_runtime::block_on(failed).is_err());
        {
            let slot = manager.inner.lock().unwrap();
            let closing = slot.closing.as_ref().unwrap();
            assert_eq!(closing.phase, ClosingPhase::Retryable);
            assert!(closing
                .capsule
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .join
                .is_some());
        }
        assert!(guard.playback_open());
        assert!(manager.begin_open().is_err());
        assert!(tauri::async_runtime::block_on(dispatch_owned_close(
            manager.clone(),
            18,
            "spawn-fail"
        ))
        .is_err());
        assert!(tauri::async_runtime::block_on(dispatch_owned_close(
            manager.clone(),
            17,
            "spawn-fail"
        ))
        .is_ok());
        assert!(!guard.playback_open());
        assert!(manager.take_for_close(17, "spawn-fail").unwrap().is_none());
    }

    #[test]
    fn owned_close_stale_session_cannot_release_newer_guard_or_acknowledge() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = rx.recv();
        });
        let mut session = test_session(17, "stale-close", Arc::clone(&guard), Some(worker));
        session.command_tx = tx;
        manager.install_test(session);
        guard.release_playback();
        guard.try_open_playback().unwrap();
        let result = tauri::async_runtime::block_on(dispatch_owned_close(
            manager.clone(),
            17,
            "stale-close",
        ));
        assert!(result.is_err(), "stale close must not acknowledge cleanup");
        assert!(
            guard.playback_open(),
            "stale close must not release newer guard"
        );
        assert!(
            manager.begin_open().is_err(),
            "unproven cleanup must remain fail-closed"
        );
        guard.release_playback();
    }

    #[test]
    fn owned_close_shutdown_racing_failed_spawn_retains_manager_owner() {
        let root = ProcessPlaybackRoot::start().unwrap();
        let manager = PlaybackManager::new();
        let weak = Arc::downgrade(&manager.inner);
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        let (claim, _, _) = prepare_process_open(
            &root,
            &manager,
            &guard,
            io,
            Duration::from_secs(2),
            17,
            || {},
        )
        .unwrap();
        let supervisor = Arc::clone(&root.supervisor);
        let open = thread::spawn(move || claim.execute(&supervisor));
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let state = open.join().unwrap().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let caller_manager = manager.clone();
        let handle = state.handle.clone();
        let caller = thread::spawn(move || {
            let future = dispatch_owned_close_with_spawn(caller_manager, 17, &handle, |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Err(std::io::Error::other("injected failure after shutdown"))
            });
            drop(future);
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        manager.shutdown();
        drop(manager);
        release_tx.send(()).unwrap();
        caller.join().unwrap();
        assert!(guard.playback_open());
        assert!(
            weak.upgrade().is_some(),
            "process root must retain failed close owner"
        );
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        harness.allow_join.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while root.registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(root.registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
    }

    #[test]
    fn owned_close_failure_before_shutdown_is_retried() {
        let root = ProcessPlaybackRoot::start().unwrap();
        let manager = PlaybackManager::new();
        let weak = Arc::downgrade(&manager.inner);
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        let (claim, _, _) = prepare_process_open(
            &root,
            &manager,
            &guard,
            io,
            Duration::from_secs(2),
            17,
            || {},
        )
        .unwrap();
        let supervisor = Arc::clone(&root.supervisor);
        let open = thread::spawn(move || claim.execute(&supervisor));
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let state = open.join().unwrap().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let caller_manager = manager.clone();
        let handle = state.handle.clone();
        let caller = thread::spawn(move || {
            let future = dispatch_owned_close_with_spawn(caller_manager, 17, &handle, |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Err(std::io::Error::other("injected failure after shutdown"))
            });
            drop(future);
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
        caller.join().unwrap();
        assert_eq!(
            manager
                .inner
                .lock()
                .unwrap()
                .closing
                .as_ref()
                .unwrap()
                .phase,
            ClosingPhase::Retryable
        );
        manager.shutdown();
        drop(manager);
        assert!(guard.playback_open());
        assert!(
            weak.upgrade().is_some(),
            "process root must retain failed close owner"
        );
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        harness.allow_join.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while root.registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(root.registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
    }

    #[test]
    fn owned_close_queued_stale_retry_does_not_touch_new_admission() {
        let root = ProcessPlaybackRoot::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        let (claim, _, _) = prepare_process_open(
            &root,
            &manager,
            &guard,
            io,
            Duration::from_secs(2),
            17,
            || {},
        )
        .unwrap();
        let supervisor = Arc::clone(&root.supervisor);
        let open = thread::spawn(move || claim.execute(&supervisor));
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let state = open.join().unwrap().unwrap();
        // A barrier after the Open job makes the retry ordering deterministic.
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        assert!(root
            .supervisor
            .sender
            .send(ProcessCleanupJob::Barrier {
                entered: entered_tx,
                release: release_rx
            })
            .is_ok());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let failed = dispatch_owned_close_with_spawn(manager.clone(), 17, &state.handle, |_| {
            Err(std::io::Error::other("injected failure"))
        });
        assert!(tauri::async_runtime::block_on(failed).is_err());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let (generation, capsule) = {
            let root_slot = root.registry.slot.lock().unwrap();
            let slot = root_slot.as_ref().unwrap();
            (slot.generation, Arc::clone(slot.reaping.as_ref().unwrap()))
        };
        assert!(root
            .supervisor
            .sender
            .send(ProcessCleanupJob::Close {
                manager: manager.clone(),
                registry: Arc::downgrade(&root.registry),
                generation,
                owner: 17,
                handle: state.handle.clone(),
                capsule
            })
            .is_ok());
        assert!(
            tauri::async_runtime::block_on(dispatch_owned_close(manager, 17, &state.handle))
                .is_ok()
        );
        assert!(root.registry.slot.lock().unwrap().is_none());
        let new_manager = PlaybackManager::new();
        let new_guard = Arc::new(NativeCaptureGuard::default());
        let (new_claim, _, _) = prepare_process_open(
            &root,
            &new_manager,
            &new_guard,
            command_fixture_io(),
            Duration::from_secs(1),
            18,
            || {},
        )
        .unwrap();
        release_tx.send(()).unwrap();
        // A second queue barrier proves the stale job was consumed.
        let (done_tx, done_rx) = mpsc::channel();
        let (done_release_tx, done_release_rx) = mpsc::channel();
        done_release_tx.send(()).unwrap();
        assert!(root
            .supervisor
            .sender
            .send(ProcessCleanupJob::Barrier {
                entered: done_tx,
                release: done_release_rx
            })
            .is_ok());
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(new_guard.playback_open());
        assert!(root
            .registry
            .slot
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|slot| Arc::ptr_eq(&slot.manager.inner, &new_manager.inner)));
        drop(ProcessExecutionTask {
            claim: Some(new_claim),
            supervisor: Arc::clone(&root.supervisor),
        });
        assert!(!new_guard.playback_open());
    }

    fn exercise_shutdown_retry_queue_failure(disconnected: bool) {
        let (sender, receiver) = mpsc::sync_channel::<ProcessCleanupJob>(0);
        let receiver = if disconnected {
            drop(receiver);
            None
        } else {
            Some(receiver)
        };
        let supervisor = Arc::new(ProcessCleanupSupervisor {
            sender,
            failed: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            root_registry: Arc::new(OnceLock::new()),
        });
        let registry = Arc::new(ProcessOwnerRegistry::new());
        supervisor.attach_registry(&registry).unwrap();
        *registry.supervisor.lock().unwrap() = Some(Arc::downgrade(&supervisor));
        let root = Arc::new(ProcessPlaybackRoot {
            registry,
            supervisor,
        });
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        let (mut claim, _, _) = prepare_process_open(
            &root,
            &manager,
            &guard,
            io,
            Duration::from_secs(2),
            17,
            || {},
        )
        .unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let state = open.join().unwrap().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_manager = manager.clone();
        let handle = state.handle.clone();
        let caller = thread::spawn(move || {
            drop(dispatch_owned_close_with_spawn(
                worker_manager,
                17,
                &handle,
                |_| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Err(std::io::Error::other("injected failure"))
                },
            ));
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        manager.shutdown();
        release_tx.send(()).unwrap();
        caller.join().unwrap();
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(guard.playback_open());
        let manager_slot = manager.inner.lock().unwrap();
        let closing = manager_slot.closing.as_ref().unwrap();
        assert_eq!(closing.phase, ClosingPhase::Retryable);
        assert!(closing.retry_enqueued);
        let capsule = Arc::clone(&closing.capsule);
        drop(manager_slot);
        let generation = root
            .registry
            .slot
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .generation;
        let stale = CloseRetryIdentity {
            registry: Arc::downgrade(&root.registry),
            generation: generation + 1,
            capsule: Arc::clone(&capsule),
        };
        assert!(manager
            .reserve_owned_close_checked(17, &state.handle, Some(&stale))
            .is_err());
        let wrong_capsule = CloseRetryIdentity {
            registry: Arc::downgrade(&root.registry),
            generation,
            capsule: Arc::new(Mutex::new(None)),
        };
        assert!(manager
            .reserve_owned_close_checked(17, &state.handle, Some(&wrong_capsule))
            .is_err());
        assert_eq!(
            manager
                .inner
                .lock()
                .unwrap()
                .closing
                .as_ref()
                .unwrap()
                .phase,
            ClosingPhase::Retryable
        );
        assert!(matches!(
            root.supervisor.failed.lock().unwrap().as_ref(),
            Some(ProcessCleanupJob::Close { .. })
        ));
        assert!(root.registry.slot.lock().unwrap().is_some());
        // Explicit test-only recovery proves the exact retained owner can join.
        harness.allow_join.store(true, Ordering::Release);
        assert!(
            tauri::async_runtime::block_on(dispatch_owned_close(manager, 17, &state.handle))
                .is_ok()
        );
        assert!(!guard.playback_open());
        assert!(root.registry.slot.lock().unwrap().is_none());
        drop(receiver);
    }

    #[test]
    fn owned_close_shutdown_retry_full_queue_retains_pending_owner() {
        exercise_shutdown_retry_queue_failure(false);
    }

    #[test]
    fn owned_close_shutdown_retry_disconnected_retains_pending_owner() {
        exercise_shutdown_retry_queue_failure(true);
    }

    #[test]
    fn prepare_capsule_rejected_publish_returns_exact_live_handle() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        let poisoned = Arc::clone(&capsule);
        let _ = thread::spawn(move || {
            let _held = poisoned.handle.lock().unwrap();
            panic!("inject prepare capsule poison");
        })
        .join();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let returned = capsule.publish(worker);
        assert!(
            returned.is_err(),
            "poisoned publish must return the live handle"
        );
        release_tx.send(()).unwrap();
        returned.err().unwrap().join().unwrap();
    }

    #[test]
    fn prepare_capsule_reserves_spawn_and_retains_rejected_handle() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        capsule.reserve_spawn().unwrap();
        assert_eq!(capsule.status(), PrepareStatus::Starting);
        assert!(
            capsule.reserve_spawn().is_err(),
            "a second spawn must be denied"
        );
        let poisoned = Arc::clone(&capsule);
        let _ = thread::spawn(move || {
            let _held = poisoned.handle.lock().unwrap();
            panic!("inject poisoned publish");
        })
        .join();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            release_rx.recv().unwrap();
        });
        let returned = capsule
            .publish(worker)
            .expect_err("poison must reject publication");
        capsule.retain_terminal(returned);
        let handle = capsule
            .handle
            .lock()
            .unwrap_err()
            .into_inner()
            .handle
            .take()
            .unwrap();
        release_tx.send(()).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn prepare_spawn_permit_panic_retains_live_handle_in_capsule() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        let (release_tx, release_rx) = mpsc::channel();
        let mut permit = capsule.reserve_permit().unwrap();
        let worker = thread::spawn(move || {
            release_rx.recv().unwrap();
        });
        permit.attach(worker).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _permit = permit;
            panic!("injected panic after spawn before publish");
        }));
        assert_eq!(capsule.status(), PrepareStatus::Terminal);
        let handle = capsule.handle.lock().unwrap().handle.take().unwrap();
        release_tx.send(()).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn prepare_capsule_claim_drop_restores_exact_handle_and_tracks_join() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        assert_eq!(capsule.status(), PrepareStatus::Reserved);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(capsule.publish(worker).is_ok());
        assert_eq!(capsule.status(), PrepareStatus::Published);
        let claim = capsule.claim().unwrap();
        assert_eq!(capsule.status(), PrepareStatus::Claimed);
        assert!(capsule.claim().is_err());
        drop(claim);
        assert_eq!(capsule.status(), PrepareStatus::Published);
        let claim = capsule.claim().unwrap();
        release_tx.send(()).unwrap();
        claim.join().unwrap();
        assert_eq!(capsule.status(), PrepareStatus::Joined);
        assert!(capsule.claim().is_err());
    }

    #[test]
    fn prepare_capsule_prevents_overwrite_and_releases_handle_once() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        let first = thread::spawn(|| {});
        let second = thread::spawn(|| {});
        assert!(capsule.publish(first).is_ok());
        let returned = capsule
            .publish(second)
            .expect_err("full capsule must not overwrite");
        returned.join().unwrap();
        capsule.claim().unwrap().join().unwrap();
        assert!(capsule.claim().is_err());
    }

    #[test]
    fn prepare_capsule_claimant_panic_restores_unjoined_handle() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        assert!(capsule.publish(thread::spawn(|| {})).is_ok());
        let claim = capsule.claim().unwrap();
        let _ = thread::spawn(move || {
            let _claim = claim;
            panic!("injected claimant panic");
        })
        .join();
        assert_eq!(capsule.status(), PrepareStatus::Published);
        capsule.claim().unwrap().join().unwrap();
        assert_eq!(capsule.status(), PrepareStatus::Joined);
    }

    #[test]
    fn prepare_capsule_panicked_worker_is_terminal_not_joined() {
        let capsule = Arc::new(PrepareHandleCapsule::new());
        assert!(capsule
            .publish(thread::spawn(|| panic!("injected prepare panic")))
            .is_ok());
        assert!(capsule.claim().unwrap().join().is_err());
        assert_eq!(capsule.status(), PrepareStatus::Terminal);
        assert!(capsule.claim().is_err());
    }

    #[test]
    fn process_owner_slot_retains_one_generation_until_proven_release() {
        let registry = ProcessOwnerRegistry::new();
        let manager = PlaybackManager::new();
        let weak = Arc::downgrade(&manager.inner);
        let guard = Arc::new(NativeCaptureGuard::default());
        registry
            .reserve(manager.clone(), Arc::clone(&guard), 7)
            .unwrap();
        assert!(registry
            .reserve(
                PlaybackManager::new(),
                Arc::new(NativeCaptureGuard::default()),
                8
            )
            .is_err());
        assert!(
            registry.release(&manager, 8).is_err(),
            "stale generation must not release an owner"
        );
        drop(manager);
        assert!(
            weak.upgrade().is_some(),
            "process slot must own manager after client drop"
        );
        let owner = registry.owner().unwrap();
        registry.release(&owner, 7).unwrap();
        drop(owner);
        assert!(
            weak.upgrade().is_none(),
            "successful release must break ownership"
        );
    }

    #[test]
    fn process_owner_admission_rolls_back_slot_and_guard_before_handoff() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        assert!(guard.playback_open());
        assert!(manager.begin_open().is_err());
        drop(admission);
        assert!(!guard.playback_open());
        assert!(registry.owner().is_none());
        let lease = manager.begin_open().unwrap();
        manager.abort_open(lease.generation);
    }

    #[test]
    fn process_owner_admission_rejects_busy_guard_without_leaking_reservation() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        assert!(ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry)
        )
        .is_err());
        assert!(
            guard.playback_open(),
            "must not release another owner's guard"
        );
        assert!(registry.owner().is_none());
        let lease = manager.begin_open().unwrap();
        manager.abort_open(lease.generation);
        guard.release_playback();
    }

    #[test]
    fn process_owner_admission_rejects_full_slot_before_guard_use() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let first = PlaybackManager::new();
        let second = PlaybackManager::new();
        let first_guard = Arc::new(NativeCaptureGuard::default());
        let second_guard = Arc::new(NativeCaptureGuard::default());
        let first_admission =
            ProcessOpenAdmission::reserve(first, first_guard, Arc::clone(&registry)).unwrap();
        assert!(ProcessOpenAdmission::reserve(
            second.clone(),
            Arc::clone(&second_guard),
            Arc::clone(&registry)
        )
        .is_err());
        assert!(!second_guard.playback_open());
        let lease = second.begin_open().unwrap();
        second.abort_open(lease.generation);
        drop(first_admission);
    }

    #[test]
    fn process_owner_admission_shutdown_before_return_rolls_back() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let shutdown_manager = manager.clone();
        let result = ProcessOpenAdmission::reserve_with_hook(
            manager,
            Arc::clone(&guard),
            Arc::clone(&registry),
            move || shutdown_manager.shutdown(),
        );
        assert!(result.is_err());
        assert!(!guard.playback_open());
        assert!(registry.owner().is_none());
    }

    #[test]
    fn process_owner_admission_registry_poison_does_not_release_guard() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission =
            ProcessOpenAdmission::reserve(manager, Arc::clone(&guard), Arc::clone(&registry))
                .unwrap();
        let poison = Arc::clone(&registry);
        let _ = thread::spawn(move || {
            let _held = poison.slot.lock().unwrap();
            panic!("inject registry poison");
        })
        .join();
        drop(admission);
        assert!(
            guard.playback_open(),
            "uncertain owner release must retain admission"
        );
    }

    #[test]
    fn process_owner_admission_poisoned_guard_retains_registry_without_panicking() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        guard.poison_for_test();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(admission)));
        assert!(
            result.is_ok(),
            "rollback must not panic on a poisoned capture guard"
        );
        assert!(
            registry.owner().is_some(),
            "uncertain guard release must retain the owner"
        );
        assert!(manager.begin_open().is_err());
    }

    #[test]
    fn process_owner_admission_prepoisoned_guard_returns_error_and_retains_owner() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.poison_for_test();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ProcessOpenAdmission::reserve(
                manager.clone(),
                Arc::clone(&guard),
                Arc::clone(&registry),
            )
        }));
        let error = match result {
            Ok(Err(error)) => error,
            Ok(Ok(_)) => panic!("poisoned guard admitted playback"),
            Err(_) => panic!("poisoned guard must return an error without panicking"),
        };
        assert_eq!(error.code, "NATIVE_UNAVAILABLE");
        assert!(!error.retryable);
        assert!(registry.owner().is_some());
        assert!(manager.begin_open().is_err());
    }

    #[test]
    fn process_owner_handoff_retains_task_and_denies_early_release() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        assert!(guard.playback_open());
        assert!(capsule.lock().unwrap().is_some());
        assert!(
            registry.release(&manager, generation).is_err(),
            "owned work cannot be released as rollback"
        );
        manager.shutdown();
        assert!(guard.playback_open(), "shutdown cannot free an owned task");
        let _task = capsule.lock().unwrap().take().unwrap();
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
    }

    #[test]
    fn process_owner_prepare_capsule_is_reserved_before_task_claim() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let reserved = Arc::clone(
            &registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .prepare_owner,
        );
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        let task = capsule.lock().unwrap().take().unwrap();
        assert!(Arc::ptr_eq(task.prepare_owner.as_ref().unwrap(), &reserved));
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_prepare_spawn_failure_keeps_slot_and_guard() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let owner = Arc::clone(
            &registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .prepare_owner,
        );
        let mut permit = owner.reserve_permit().unwrap();
        assert!(permit
            .spawn_with(Box::new(|| {}), |_| Err(std::io::Error::other(
                "injected spawn failure"
            )))
            .is_err());
        drop(permit);
        assert_eq!(owner.status(), PrepareStatus::Terminal);
        assert!(guard.playback_open());
        assert!(registry.owner().is_some());
        let task_capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        let _task = task_capsule.lock().unwrap().take().unwrap();
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_real_prepare_spawn_publishes_and_joins_reserved_capsule() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::clone(&release)),
            worker: Arc::new(|_, _, _, _| {
                Err(WorkerStartFailure::UnknownOwnership(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "injected worker failure",
                    false,
                )))
            }),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let slot = registry.slot.lock().unwrap();
        let owner = Arc::clone(&slot.as_ref().unwrap().prepare_owner);
        let task_capsule = Arc::clone(slot.as_ref().unwrap().pending_open.as_ref().unwrap());
        drop(slot);
        let task = task_capsule.lock().unwrap().take().unwrap();
        let open = thread::spawn(move || open_playback_owned(task));
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while owner.status() != PrepareStatus::Published && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(owner.status(), PrepareStatus::Published);
        release.store(true, Ordering::Release);
        assert!(open.join().unwrap().is_err());
        assert_eq!(owner.status(), PrepareStatus::Joined);
        assert!(
            guard.playback_open(),
            "generic worker factory failure remains fail-closed"
        );
        assert_eq!(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .worker_owner
                .status(),
            PrepareStatus::Reserved
        );
        // This particular injected factory creates no thread, so the test may
        // dispose its fixture; production treats that fact as unproven.
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_claim_panic_after_take_is_terminal_and_keeps_guard() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let token = admission.guard_token;
        let (harness, _, _) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: Box::new(|_| Ok(prepared_fixture())),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let join = thread::spawn(move || {
            let (_claim, _task) = (claim, task);
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            panic!("injected executor panic after task handoff");
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            registry.slot.lock().unwrap().as_ref().unwrap().phase,
            ProcessOwnerPhase::Executing
        );
        assert!(registry.claim_open(&manager, generation).is_err());
        release_tx.send(()).unwrap();
        assert!(join.join().is_err());
        assert_eq!(
            registry.slot.lock().unwrap().as_ref().unwrap().phase,
            ProcessOwnerPhase::Terminal
        );
        assert!(guard.playback_open());
        assert!(registry.claim_open(&manager, generation).is_err());
        // This fixture proves no worker was started; production cannot infer that.
        registry.slot.lock().unwrap().take();
        assert_eq!(guard.try_release_playback_token(token), Ok(true));
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_production_metadata_rejects_zero_and_partial() {
        for count in 0..6 {
            assert!(validate_process_metadata(count, true).is_err());
        }
        assert!(validate_process_metadata(6, true).is_ok());
        assert!(validate_process_metadata(0, false).is_ok());
        for count in 1..6 {
            assert!(validate_process_metadata(count, false).is_err());
        }
    }

    fn process_close_request(registry: &Arc<ProcessOwnerRegistry>) -> Arc<AtomicBool> {
        Arc::clone(
            &registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .close_requested,
        )
    }

    #[test]
    fn process_owner_partial_metadata_does_not_start_or_release_guard() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (harness, started, _) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: Box::new(|_| panic!("partial ownership must not start prepare")),
            worker: worker_factory(harness),
        };
        let mut task = admitted_task(
            manager.clone(),
            Arc::clone(&guard),
            io,
            Duration::from_secs(1),
        );
        let generation = task.lease.generation;
        let token = task.guard_token;
        task.worker_owner = Some(Arc::new(PrepareHandleCapsule::new()));
        assert!(open_playback_owned(task).is_err());
        assert!(started.try_recv().is_err());
        assert!(guard.playback_open());
        assert_eq!(guard.try_release_playback_token(token), Ok(true));
        manager.abort_open(generation);
    }

    fn disconnected_supervisor() -> Arc<ProcessCleanupSupervisor> {
        let (sender, receiver) = mpsc::sync_channel::<ProcessCleanupJob>(1);
        drop(receiver);
        Arc::new(ProcessCleanupSupervisor {
            sender,
            failed: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            root_registry: Arc::new(OnceLock::new()),
        })
    }

    fn exercise_failed_enqueue_after_install(full: bool) {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let (supervisor, _receiver) = if full {
            let (sender, receiver) = mpsc::sync_channel::<ProcessCleanupJob>(0);
            (
                Arc::new(ProcessCleanupSupervisor {
                    sender,
                    failed: Arc::new(Mutex::new(None)),
                    alive: Arc::new(AtomicBool::new(true)),
                    root_registry: Arc::new(OnceLock::new()),
                }),
                Some(receiver),
            )
        } else {
            (disconnected_supervisor(), None)
        };
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let (runtime_tx, runtime_rx) = mpsc::channel();
        let inner = worker_factory(harness.clone());
        let worker: WorkerFn = Arc::new(move |prepared, runtime, cancel, owner| {
            runtime_tx.send(Arc::clone(&runtime)).unwrap();
            inner(prepared, runtime, cancel, owner)
        });
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker,
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let supervisor_for_open = Arc::clone(&supervisor);
        let open = thread::spawn(move || claim.execute(&supervisor_for_open));
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        assert!(open.join().unwrap().is_err());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let runtime = runtime_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            runtime.close_requested.load(Ordering::Acquire),
            "callback suppression must precede join"
        );
        assert!(guard.playback_open());
        assert!(supervisor.failed.lock().unwrap().is_some());
        harness.allow_join.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
        let next = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let next_generation = next.lease.generation;
        let (next_harness, _, _) = worker_harness();
        next.handoff(
            OpenPlaybackIo {
                prepare: Box::new(|_| Ok(prepared_fixture())),
                worker: worker_factory(next_harness),
            },
            Duration::from_secs(1),
            17,
        )
        .unwrap();
        let next_claim = registry.claim_open(&manager, next_generation).unwrap();
        assert!(supervisor.submit(next_claim).is_err());
        assert_eq!(
            match supervisor.failed.lock().unwrap().as_ref().unwrap() {
                ProcessCleanupJob::Open(claim) => claim.generation,
                _ => panic!("expected open claim"),
            },
            next_generation
        );
        assert!(guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_some());
    }

    #[test]
    fn process_owner_disconnected_enqueue_two_generations() {
        exercise_failed_enqueue_after_install(false);
    }

    #[test]
    fn process_owner_full_enqueue_two_generations() {
        exercise_failed_enqueue_after_install(true);
    }

    #[test]
    fn process_owner_drop_close_is_retained_when_same_generation_open_is_parked() {
        let fixture = process_owned_session_fixture();
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        supervisor.attach_registry(&fixture.registry).unwrap();
        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (first_release_tx, first_release_rx) = mpsc::channel();
        supervisor
            .submit_job(ProcessCleanupJob::Barrier {
                entered: first_entered_tx,
                release: first_release_rx,
            })
            .unwrap();
        first_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first supervisor barrier did not start");

        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let (second_release_tx, second_release_rx) = mpsc::channel();
        supervisor
            .submit_job(ProcessCleanupJob::Barrier {
                entered: second_entered_tx,
                release: second_release_rx,
            })
            .unwrap();
        assert!(supervisor
            .submit_job(ProcessCleanupJob::Open(fixture.open_claim))
            .is_err());
        assert!(matches!(
            supervisor.failed.lock().unwrap().as_ref(),
            Some(ProcessCleanupJob::Open(claim))
                if claim.generation == fixture.generation
        ));

        let root_close_requested = process_close_request(&fixture.registry);

        let delivery = OpenDispatchGuard {
            playback: fixture.manager.clone(),
            guard: Arc::clone(&fixture.guard),
            guard_token: fixture.guard.active_playback_token_for_test().unwrap(),
            lease: OpenLease {
                generation: fixture.generation,
                cancel: Arc::new(AtomicBool::new(false)),
                cleanup: Arc::clone(&fixture.cleanup),
            },
            owner: 17,
            process_cleanup: Some(ProcessOpenCleanupOwner {
                supervisor: Arc::clone(&supervisor),
                close_requested: process_close_request(&fixture.registry),
            }),
            started: true,
            delivered: false,
            installed_handle: Some(fixture.handle.clone()),
        };
        drop(delivery);
        assert!(fixture.runtime.close_requested.load(Ordering::Acquire));
        assert!(root_close_requested.load(Ordering::Acquire));

        first_release_tx.send(()).unwrap();
        second_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("queued supervisor barrier did not start");
        assert!(supervisor.failed.lock().unwrap().is_some());
        second_release_tx.send(()).unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while fixture.registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            fixture.registry.slot.lock().unwrap().is_none(),
            "the root must retain and process the exact generation's close request"
        );
        assert!(!fixture.guard.playback_open());
    }

    #[test]
    fn process_owner_drop_does_not_wait_for_failed_or_registry_locks() {
        let fixture = process_owned_session_fixture();
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        supervisor.attach_registry(&fixture.registry).unwrap();
        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (first_release_tx, first_release_rx) = mpsc::channel();
        supervisor
            .submit_job(ProcessCleanupJob::Barrier {
                entered: first_entered_tx,
                release: first_release_rx,
            })
            .unwrap();
        first_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first supervisor barrier did not start");

        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let (second_release_tx, second_release_rx) = mpsc::channel();
        supervisor
            .submit_job(ProcessCleanupJob::Barrier {
                entered: second_entered_tx,
                release: second_release_rx,
            })
            .unwrap();

        let root_close_requested = process_close_request(&fixture.registry);
        let delivery = OpenDispatchGuard {
            playback: fixture.manager.clone(),
            guard: Arc::clone(&fixture.guard),
            guard_token: fixture.guard.active_playback_token_for_test().unwrap(),
            lease: OpenLease {
                generation: fixture.generation,
                cancel: Arc::new(AtomicBool::new(false)),
                cleanup: Arc::clone(&fixture.cleanup),
            },
            owner: 17,
            process_cleanup: Some(ProcessOpenCleanupOwner {
                supervisor: Arc::clone(&supervisor),
                close_requested: process_close_request(&fixture.registry),
            }),
            started: true,
            delivered: false,
            installed_handle: Some(fixture.handle.clone()),
        };
        let failed_lock = supervisor.failed.lock().unwrap();
        let registry_lock = fixture.registry.slot.lock().unwrap();
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let drop_thread = thread::spawn(move || {
            drop(delivery);
            let _ = dropped_tx.send(());
        });
        let returned_before_unlock = dropped_rx.recv_timeout(Duration::from_millis(200)).is_ok();
        let root_close_was_requested = root_close_requested.load(Ordering::Acquire);
        drop(registry_lock);
        drop(failed_lock);

        first_release_tx.send(()).unwrap();
        second_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("queued supervisor barrier did not start");
        second_release_tx.send(()).unwrap();
        drop_thread.join().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while fixture.registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }

        assert!(
            returned_before_unlock,
            "dropping a timed-out open must not wait for supervisor or registry mutexes"
        );
        assert!(
            root_close_was_requested,
            "drop must set the exact registry slot's close request without locking it"
        );
        assert!(fixture.registry.slot.lock().unwrap().is_none());
        assert!(!fixture.guard.playback_open());
    }

    #[test]
    fn process_owner_capacity_one_queue_full_retains_claim() {
        fn queued_claim() -> (
            ProcessOpenClaim,
            Arc<NativeCaptureGuard>,
            Arc<ProcessOwnerRegistry>,
        ) {
            let registry = Arc::new(ProcessOwnerRegistry::new());
            let manager = PlaybackManager::new();
            let guard = Arc::new(NativeCaptureGuard::default());
            let admission = ProcessOpenAdmission::reserve(
                manager.clone(),
                Arc::clone(&guard),
                Arc::clone(&registry),
            )
            .unwrap();
            let generation = admission.lease.generation;
            let (harness, _, _) = worker_harness();
            admission
                .handoff(
                    OpenPlaybackIo {
                        prepare: Box::new(|_| Ok(prepared_fixture())),
                        worker: worker_factory(harness),
                    },
                    Duration::from_secs(1),
                    17,
                )
                .unwrap();
            let claim = registry.claim_open(&manager, generation).unwrap();
            (claim, guard, registry)
        }
        let (sender, _receiver) = mpsc::sync_channel::<ProcessCleanupJob>(1);
        let (queued, queued_guard, queued_registry) = queued_claim();
        assert!(sender.try_send(ProcessCleanupJob::Open(queued)).is_ok());
        let supervisor = ProcessCleanupSupervisor {
            sender,
            failed: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            root_registry: Arc::new(OnceLock::new()),
        };
        let (claim, guard, registry) = queued_claim();
        assert!(supervisor.submit(claim).is_err());
        assert!(supervisor.failed.lock().unwrap().is_some());
        assert!(guard.playback_open() && queued_guard.playback_open());
        assert!(
            registry.slot.lock().unwrap().is_some()
                && queued_registry.slot.lock().unwrap().is_some()
        );
    }

    #[test]
    fn process_owner_full_supervisor_retains_claim_and_guard() {
        let (sender, _receiver) = mpsc::sync_channel::<ProcessCleanupJob>(0);
        let supervisor = Arc::new(ProcessCleanupSupervisor {
            sender,
            failed: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            root_registry: Arc::new(OnceLock::new()),
        });
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (harness, _, _) = worker_harness();
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: worker_factory(harness),
                },
                Duration::from_secs(1),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        assert!(supervisor.submit(claim).is_err());
        assert!(supervisor.failed.lock().unwrap().is_some());
        assert!(registry.slot.lock().unwrap().is_some());
        assert!(guard.playback_open());
        assert!(registry.claim_open(&manager, generation).is_err());
    }

    #[test]
    fn process_owner_disconnected_supervisor_is_rooted_after_clients_drop() {
        static ROOT: std::sync::OnceLock<Arc<ProcessPlaybackRoot>> = std::sync::OnceLock::new();
        let root = Arc::clone(ROOT.get_or_init(|| {
            Arc::new(ProcessPlaybackRoot {
                registry: Arc::new(ProcessOwnerRegistry::new()),
                supervisor: disconnected_supervisor(),
            })
        }));
        let manager = PlaybackManager::new();
        let weak_manager = Arc::downgrade(&manager.inner);
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&root.registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (harness, _, _) = worker_harness();
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: worker_factory(harness),
                },
                Duration::from_secs(1),
                17,
            )
            .unwrap();
        let claim = root.registry.claim_open(&manager, generation).unwrap();
        assert!(root.supervisor.submit(claim).is_err());
        drop(manager);
        drop(root);
        assert!(weak_manager.upgrade().is_some());
        let root = ROOT.get().unwrap();
        assert!(root.supervisor.failed.lock().unwrap().is_some());
        assert!(root.registry.slot.lock().unwrap().is_some());
        assert!(guard.playback_open());
    }

    fn command_fixture_io() -> OpenPlaybackIo {
        let (harness, _, _) = worker_harness();
        OpenPlaybackIo {
            prepare: Box::new(|_| Ok(prepared_fixture())),
            worker: worker_factory(harness),
        }
    }

    #[test]
    fn process_owner_command_admission_excludes_other_manager_and_guard() {
        let root = ProcessPlaybackRoot::start().unwrap();
        let first_manager = PlaybackManager::new();
        let first_guard = Arc::new(NativeCaptureGuard::default());
        let (claim, _, _) = prepare_process_open(
            &root,
            &first_manager,
            &first_guard,
            command_fixture_io(),
            Duration::from_secs(1),
            17,
            || {},
        )
        .unwrap();
        let second_manager = PlaybackManager::new();
        let second_guard = Arc::new(NativeCaptureGuard::default());
        assert!(prepare_process_open(
            &root,
            &second_manager,
            &second_guard,
            command_fixture_io(),
            Duration::from_secs(1),
            18,
            || {}
        )
        .is_err());
        assert!(first_guard.playback_open());
        assert!(!second_guard.playback_open());
        drop(ProcessExecutionTask {
            claim: Some(claim),
            supervisor: Arc::clone(&root.supervisor),
        });
        assert!(!first_guard.playback_open());
    }

    #[test]
    fn process_owner_command_handoff_shutdown_rolls_back() {
        let root = ProcessPlaybackRoot::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        assert!(prepare_process_open(
            &root,
            &manager,
            &guard,
            command_fixture_io(),
            Duration::from_secs(1),
            17,
            || manager.shutdown()
        )
        .is_err());
        assert!(!guard.playback_open());
        assert!(root.registry.slot.lock().unwrap().is_none());
    }

    #[test]
    fn process_owner_command_handoff_poison_retains_owner() {
        let root = ProcessPlaybackRoot::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let poison = Arc::clone(&root.registry);
        assert!(prepare_process_open(
            &root,
            &manager,
            &guard,
            command_fixture_io(),
            Duration::from_secs(1),
            17,
            || {
                assert!(thread::spawn(move || {
                    let _held = poison.slot.lock().unwrap();
                    panic!("injected claim poison");
                })
                .join()
                .is_err());
            }
        )
        .is_err());
        let slot = match root.registry.slot.lock() {
            Err(poisoned) => poisoned.into_inner(),
            Ok(_) => panic!("expected poisoned registry"),
        };
        assert!(slot.is_some());
        assert!(guard.playback_open());
    }

    #[test]
    fn process_owner_shutdown_between_handoff_and_claim_cancels_unstarted() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (harness, worker_rx, _) = worker_harness();
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| panic!("cancelled open must not start")),
                    worker: worker_factory(harness),
                },
                Duration::from_secs(1),
                17,
            )
            .unwrap();
        manager.shutdown();
        assert!(registry.claim_open(&manager, generation).is_err());
        assert!(registry.cancel_unclaimed(&manager, generation + 1).is_err());
        registry.cancel_unclaimed(&manager, generation).unwrap();
        assert!(worker_rx.try_recv().is_err());
        assert!(!guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(PlaybackManager::new(), guard, registry).unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_accepted_stalled_executor_times_out_then_late_start_rolls_back_exact_claim() {
        // Initialize Tauri's Tokio runtime before starting the one-shot deadline.
        tauri::async_runtime::block_on(async { tokio::task::yield_now().await });

        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (prepare_started_tx, prepare_started_rx) = mpsc::channel();
        let (harness, worker_rx, _) = worker_harness();
        let prepare: PrepareFn = Box::new(move |_| {
            prepare_started_tx.send(()).unwrap();
            Err(safe_storage_error())
        });
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare,
                    worker: worker_factory(harness),
                },
                Duration::from_millis(150),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let queued_job = Arc::new(Mutex::new(None));
        let queued_job_for_scheduler = Arc::clone(&queued_job);
        let future = dispatch_process_open_with_scheduler(
            ProcessOpenDispatch {
                claim,
                supervisor: supervisor.clone(),
                playback: manager.clone(),
                guard: Arc::clone(&guard),
                guard_token: token,
                lease,
                owner: 17,
            },
            move |job| {
                *queued_job_for_scheduler.lock().unwrap() = Some(job);
                std::future::pending::<Result<Result<PlaybackState, ReviewError>, ReviewError>>()
            },
        );

        let caller = tauri::async_runtime::block_on(async {
            tokio::time::timeout(Duration::from_millis(750), future).await
        });
        if caller.is_err() {
            // The RED implementation leaves this scheduler-accepted job waiting.
            // Drop it before asserting so its own exact rollback cleans the fixture.
            drop(queued_job.lock().unwrap().take());
        }
        assert!(
            caller.is_ok(),
            "accepted but stalled executor must not outlive the open deadline"
        );
        assert_eq!(caller.unwrap().unwrap_err().code, "PLAYBACK_OPEN_TIMEOUT");
        assert!(
            guard.playback_open(),
            "timeout is not cleanup acknowledgement"
        );
        assert!(registry.slot.lock().unwrap().is_some());
        assert_eq!(manager.begin_open().unwrap_err().code, "PLAYBACK_BUSY");
        assert!(prepare_started_rx.try_recv().is_err());
        assert!(worker_rx.try_recv().is_err());

        let late_job = queued_job.lock().unwrap().take().unwrap();
        assert_eq!(late_job().unwrap_err().code, "PLAYBACK_OPEN_TIMEOUT");
        assert!(prepare_started_rx.try_recv().is_err());
        assert!(worker_rx.try_recv().is_err());
        assert!(!guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_none());
        drop(ProcessOpenAdmission::reserve(manager, guard, registry).unwrap());
    }

    #[test]
    fn process_owner_ready_result_polled_after_deadline_is_fenced_and_closed() {
        // Tokio polls a ready inner future before checking timeout_at's timer.
        // Execute successfully into a ready future, then delay its first poll
        // until after the admission deadline to exercise the delivery fence.
        tauri::async_runtime::block_on(async { tokio::task::yield_now().await });

        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (harness, worker_rx, close_rx) = worker_harness();
        let ready_harness = harness.clone();
        let ready_worker = thread::spawn(move || {
            worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            release_ready(&ready_harness);
        });
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: worker_factory(harness.clone()),
                },
                Duration::from_millis(500),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let deadline = claim.deadline;
        let future = dispatch_process_open_with_scheduler(
            ProcessOpenDispatch {
                claim,
                supervisor,
                playback: manager.clone(),
                guard: Arc::clone(&guard),
                guard_token: token,
                lease,
                owner: 17,
            },
            move |job| std::future::ready(Ok(job())),
        );
        ready_worker.join().unwrap();
        assert!(manager.inner.lock().unwrap().active.is_some());
        assert!(
            Instant::now() < deadline,
            "executor must become ready before deadline"
        );

        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .saturating_add(Duration::from_millis(25)),
        );
        assert_eq!(
            tauri::async_runtime::block_on(future).unwrap_err().code,
            "PLAYBACK_OPEN_TIMEOUT"
        );
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        harness.allow_join.store(true, Ordering::Release);
        wait_for_guard_release(&guard);
        assert!(manager.inner.lock().unwrap().active.is_none());
        assert!(registry.slot.lock().unwrap().is_none());
    }

    #[test]
    fn process_owner_delivery_timeout_returns_before_held_manager_lock_releases() {
        tauri::async_runtime::block_on(async { tokio::task::yield_now().await });

        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let (barrier_entered_tx, barrier_entered_rx) = mpsc::channel();
        let (barrier_release_tx, barrier_release_rx) = mpsc::channel();
        supervisor
            .submit_job(ProcessCleanupJob::Barrier {
                entered: barrier_entered_tx,
                release: barrier_release_rx,
            })
            .unwrap();
        barrier_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("cleanup supervisor barrier did not start");
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (harness, worker_rx, close_rx) = worker_harness();
        let ready_harness = harness.clone();
        let ready_worker = thread::spawn(move || {
            worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            release_ready(&ready_harness);
        });
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: worker_factory(harness.clone()),
                },
                Duration::from_millis(500),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let deadline = claim.deadline;
        let future = dispatch_process_open_with_scheduler(
            ProcessOpenDispatch {
                claim,
                supervisor: supervisor.clone(),
                playback: manager.clone(),
                guard: Arc::clone(&guard),
                guard_token: token,
                lease,
                owner: 17,
            },
            move |job| std::future::ready(Ok(job())),
        );
        ready_worker.join().unwrap();
        let runtime = Arc::clone(
            &manager
                .inner
                .lock()
                .unwrap()
                .active
                .as_ref()
                .expect("ready process owner must install its session")
                .runtime,
        );
        let root_close_requested = process_close_request(&registry);

        let held_manager = manager.inner.lock().unwrap();
        let (result_tx, result_rx) = mpsc::channel();
        let delivery_thread = thread::spawn(move || {
            result_tx
                .send(tauri::async_runtime::block_on(future))
                .unwrap();
        });
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .saturating_add(Duration::from_millis(40)),
        );
        let timely_result = result_rx.recv_timeout(Duration::from_millis(100)).ok();
        let returned_while_locked = timely_result.is_some();
        let output_silenced_while_locked = runtime.close_requested.load(Ordering::Acquire);
        let close_request_parked = root_close_requested.load(Ordering::Acquire);
        assert!(
            guard.playback_open(),
            "timeout is not cleanup acknowledgement"
        );

        barrier_release_tx.send(()).unwrap();
        drop(held_manager);
        let result = timely_result
            .unwrap_or_else(|| result_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        delivery_thread.join().unwrap();
        assert_eq!(result.unwrap_err().code, "PLAYBACK_OPEN_TIMEOUT");
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        harness.allow_join.store(true, Ordering::Release);
        wait_for_guard_release(&guard);
        assert!(manager.inner.lock().unwrap().active.is_none());
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(
            returned_while_locked,
            "timeout must return while the manager lock is still held"
        );
        assert!(
            output_silenced_while_locked,
            "timeout must silence the installed runtime before manager lock release"
        );
        assert!(
            close_request_parked,
            "the root must retain the generation-scoped close request while the manager is locked"
        );
        assert!(supervisor.failed.lock().unwrap().is_none());
    }

    #[test]
    fn process_close_request_retries_owned_close_after_spawn_failure() {
        let fixture = process_owned_session_fixture();
        fixture.cleanup.signal_close();
        let mut spawn_attempts = 0;
        let result = close_active_process_request_with_spawn(
            fixture.manager.clone(),
            Arc::downgrade(&fixture.registry),
            fixture.generation,
            17,
            Some(fixture.handle.clone()),
            |work| {
                spawn_attempts += 1;
                if spawn_attempts == 1 {
                    Err(std::io::Error::other("injected close spawn failure"))
                } else {
                    thread::Builder::new()
                        .name("fung-test-process-owned-close".into())
                        .spawn(work)
                }
            },
        );

        result.unwrap();
        assert_eq!(spawn_attempts, 2);
        fixture
            .close_seen
            .recv_timeout(Duration::from_secs(1))
            .expect("process-owned retry must close the worker");
        assert!(!fixture.guard.playback_open());
        assert!(fixture.registry.slot.lock().unwrap().is_none());
    }

    #[test]
    fn process_close_request_poison_retains_silenced_terminal_owner() {
        let fixture = process_owned_session_fixture();
        let manager_to_poison = fixture.manager.clone();
        let poison = thread::spawn(move || {
            let _slot = manager_to_poison.inner.lock().unwrap();
            panic!("inject poisoned playback manager");
        })
        .join();
        assert!(poison.is_err());

        fixture.cleanup.signal_close();
        let result = close_active_process_request(
            fixture.manager.clone(),
            Arc::downgrade(&fixture.registry),
            fixture.generation,
            17,
            Some(fixture.handle.clone()),
        );

        assert!(result.is_err());
        assert!(fixture.runtime.close_requested.load(Ordering::Acquire));
        fixture
            .close_seen
            .recv_timeout(Duration::from_secs(1))
            .expect("lock-free worker close signal must still be delivered");
        assert!(fixture.guard.playback_open());
        assert_eq!(
            fixture
                .registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .phase,
            ProcessOwnerPhase::Terminal
        );
        let retained = fixture.manager.inner.lock().unwrap_err().into_inner();
        assert_eq!(
            retained
                .active
                .as_ref()
                .map(|session| session.handle.as_str()),
            Some(fixture.handle.as_str())
        );
    }

    #[test]
    fn process_owner_delivery_rechecks_deadline_after_manager_lock_wait() {
        tauri::async_runtime::block_on(async { tokio::task::yield_now().await });

        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (harness, worker_rx, close_rx) = worker_harness();
        let ready_harness = harness.clone();
        let ready_worker = thread::spawn(move || {
            worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            release_ready(&ready_harness);
        });
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: worker_factory(harness.clone()),
                },
                Duration::from_millis(1_000),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let deadline = claim.deadline;
        let (checked_tx, checked_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let future = dispatch_process_open_with_scheduler_hook(
            ProcessOpenDispatch {
                claim,
                supervisor,
                playback: manager.clone(),
                guard: Arc::clone(&guard),
                guard_token: token,
                lease,
                owner: 17,
            },
            move |job| std::future::ready(Ok(job())),
            move |checked_at| {
                checked_tx.send(checked_at).unwrap();
                resume_rx.recv().unwrap();
            },
        );
        ready_worker.join().unwrap();
        assert!(manager.inner.lock().unwrap().active.is_some());

        let held_manager = manager.inner.lock().unwrap();
        let (result_tx, result_rx) = mpsc::channel();
        let delivery_thread = thread::spawn(move || {
            result_tx
                .send(tauri::async_runtime::block_on(future))
                .unwrap();
        });
        let checked_at = checked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(checked_at < deadline, "pre-lock deadline check must pass");
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .saturating_add(Duration::from_millis(25)),
        );
        resume_tx.send(()).unwrap();
        drop(held_manager);

        assert_eq!(
            result_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap_err()
                .code,
            "PLAYBACK_OPEN_TIMEOUT"
        );
        delivery_thread.join().unwrap();
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        harness.allow_join.store(true, Ordering::Release);
        wait_for_guard_release(&guard);
        assert!(manager.inner.lock().unwrap().active.is_none());
        assert!(registry.slot.lock().unwrap().is_none());
    }

    #[test]
    fn process_owner_scheduler_reject_before_start_rolls_back_exact_owner() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (harness, worker_rx, _) = worker_harness();
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| panic!("rejected job must not run")),
                    worker: worker_factory(harness),
                },
                Duration::from_secs(1),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let execution = ProcessExecutionTask {
            claim: Some(claim),
            supervisor,
        };
        let rejected_scheduler = |job: Box<dyn FnOnce() + Send>| -> Result<(), ()> {
            drop(job);
            Err(())
        };
        assert!(
            schedule_process_execution(execution, |job| rejected_scheduler(Box::new(move || {
                let _ = job();
            })))
            .is_err()
        );
        assert!(worker_rx.try_recv().is_err());
        assert!(!guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(manager, guard, registry).unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_accepted_scheduler_drop_releases_locks_before_task_destructor() {
        struct ReentrantDrop {
            manager: PlaybackManager,
            registry: Arc<ProcessOwnerRegistry>,
            observed: Arc<AtomicBool>,
        }
        impl Drop for ReentrantDrop {
            fn drop(&mut self) {
                let manager_ok = self.manager.inner.try_lock().is_ok();
                let registry_ok = self.registry.slot.try_lock().is_ok();
                self.observed
                    .store(manager_ok && registry_ok, Ordering::Release);
            }
        }
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let observed = Arc::new(AtomicBool::new(false));
        let probe = ReentrantDrop {
            manager: manager.clone(),
            registry: Arc::clone(&registry),
            observed: Arc::clone(&observed),
        };
        let (harness, _, _) = worker_harness();
        let prepare: PrepareFn = Box::new(move |_| {
            let _ = &probe;
            Ok(prepared_fixture())
        });
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare,
                    worker: worker_factory(harness),
                },
                Duration::from_secs(1),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let execution = ProcessExecutionTask {
            claim: Some(claim),
            supervisor,
        };
        let mut accepted = None;
        schedule_process_execution(execution, |job| {
            accepted = Some(job);
        });
        assert!(guard.playback_open());
        drop(accepted.take()); // accepted job discarded during simulated runtime shutdown
        assert!(observed.load(Ordering::Acquire));
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
    }

    #[test]
    fn process_owner_scheduler_reject_with_poison_keeps_terminal_owner() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (harness, _, _) = worker_harness();
        admission
            .handoff(
                OpenPlaybackIo {
                    prepare: Box::new(|_| Ok(prepared_fixture())),
                    worker: worker_factory(harness),
                },
                Duration::from_secs(1),
                17,
            )
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        guard.poison_for_test();
        let execution = ProcessExecutionTask {
            claim: Some(claim),
            supervisor,
        };
        drop(execution);
        let root = registry.slot.lock().unwrap();
        assert_eq!(root.as_ref().unwrap().phase, ProcessOwnerPhase::Terminal);
        assert!(root
            .as_ref()
            .unwrap()
            .pending_open
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .is_some());
    }

    #[test]
    fn process_owner_eager_dispatch_drop_cancels_and_reaps() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let (harness, worker_rx, _) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::clone(&release)),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let future = dispatch_process_open(
            claim,
            supervisor,
            manager.clone(),
            Arc::clone(&guard),
            token,
            lease.clone(),
            17,
        );
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(future);
        assert!(lease.cancel.load(Ordering::Acquire));
        assert!(guard.playback_open());
        release.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
        assert!(worker_rx.try_recv().is_err());
    }

    #[test]
    fn process_owner_eager_dispatch_drop_after_install_closes_before_return() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let future = dispatch_process_open(
            claim,
            supervisor,
            manager.clone(),
            Arc::clone(&guard),
            token,
            lease.clone(),
            17,
        );
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let deadline = Instant::now() + Duration::from_secs(2);
        while manager.inner.lock().unwrap().active.is_none() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(manager.inner.lock().unwrap().active.is_some());
        drop(future);
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(guard.playback_open());
        harness.allow_join.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
    }

    #[test]
    fn process_owner_shutdown_after_install_rejects_stale_delivery() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let lease = admission.lease.clone();
        let token = admission.guard_token;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let future = dispatch_process_open(
            claim,
            supervisor,
            manager.clone(),
            Arc::clone(&guard),
            token,
            lease.clone(),
            17,
        );
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let deadline = Instant::now() + Duration::from_secs(2);
        while manager.inner.lock().unwrap().active.is_none() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(manager.inner.lock().unwrap().active.is_some());
        manager.shutdown();
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(tauri::async_runtime::block_on(future).is_err());
        assert!(guard.playback_open());
        harness.allow_join.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
    }

    #[test]
    fn process_owner_open_timeout_returns_while_supervisor_joins_blocked_prepare() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let supervisor = ProcessCleanupSupervisor::start().unwrap();
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let (harness, worker_rx, _) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::clone(&release)),
            worker: worker_factory(harness),
        };
        admission
            .handoff(io, Duration::from_millis(80), 17)
            .unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        let started = Instant::now();
        let result = claim.execute(&supervisor);
        assert!(result.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout must not wait for blocked cleanup"
        );
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(worker_rx.try_recv().is_err());
        assert!(guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_some());
        release.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(registry.slot.lock().unwrap().is_none());
        assert!(!guard.playback_open());
    }

    #[test]
    fn process_owner_prepare_error_releases_completed_owner_for_new_admission() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (harness, worker_rx, _) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: Box::new(|_| {
                Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "injected prepare failure",
                    false,
                ))
            }),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        assert!(open_playback_owned(task).is_err());
        claim.complete().unwrap();
        drop(claim);
        assert!(worker_rx.try_recv().is_err());
        assert!(!guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(manager, guard, registry).unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_proven_worker_not_started_releases_guard() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(AtomicBool::new(false));
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::clone(&release)),
            worker: Arc::new(|_, _, _, owner| {
                let (command_tx, _) = mpsc::channel();
                let (_, ready_rx) = mpsc::channel();
                launch_worker_thread(command_tx, ready_rx, owner, Box::new(|| {}), |_| {
                    Err(std::io::Error::other("injected spawn failure"))
                })
            }),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let slot = registry.slot.lock().unwrap();
        let owner = Arc::clone(&slot.as_ref().unwrap().prepare_owner);
        drop(slot);
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            drop(claim);
            result
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while owner.status() != PrepareStatus::Published && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(owner.status(), PrepareStatus::Published);
        release.store(true, Ordering::Release);
        assert!(open.join().unwrap().is_err());
        assert_eq!(owner.status(), PrepareStatus::Joined);
        assert!(
            !guard.playback_open(),
            "proven no-worker failure releases exact guard"
        );
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_rejected_close_transition_keeps_active_worker_unsuppressed() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let session = test_session(17, "mismatch", Arc::clone(&guard), None);
        let runtime = Arc::clone(&session.runtime);
        let task = Arc::new(Mutex::new(None));
        manager.install_test(session);
        manager
            .inner
            .lock()
            .unwrap()
            .active
            .as_mut()
            .unwrap()
            .process_identity = Some(ProcessPlaybackIdentity {
            registry: Arc::downgrade(&registry),
            task: Arc::downgrade(&task),
            generation: 1,
        });
        assert!(manager.reserve_owned_close(17, "mismatch").is_err());
        assert!(!runtime.close_requested.load(Ordering::Acquire));
        assert!(manager.inner.lock().unwrap().active.is_some());
        assert!(guard.playback_open());
    }

    #[test]
    fn process_owner_poisoned_close_completion_is_terminal() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            drop(claim);
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_ready(&harness);
        let state = open.join().unwrap().unwrap();
        let close = dispatch_owned_close(manager.clone(), 17, &state.handle);
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let poison = Arc::clone(&registry);
        assert!(thread::spawn(move || {
            let _lock = poison.slot.lock().unwrap();
            panic!("inject close registry poison");
        })
        .join()
        .is_err());
        harness.allow_join.store(true, Ordering::Release);
        assert!(tauri::async_runtime::block_on(close).is_err());
        assert_eq!(
            manager
                .inner
                .lock()
                .unwrap()
                .closing
                .as_ref()
                .unwrap()
                .phase,
            ClosingPhase::Terminal
        );
    }

    #[test]
    fn process_owner_shutdown_after_late_reservation_retains_worker_until_join() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        let inner = worker_factory(harness.clone());
        let (paused_tx, paused_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let worker: WorkerFn = Arc::new(move |prepared, runtime, cancel, owner| {
            paused_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
            inner(prepared, runtime, cancel, owner)
        });
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker,
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let worker_owner =
            Arc::clone(&registry.slot.lock().unwrap().as_ref().unwrap().worker_owner);
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            drop(claim);
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        paused_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .late_reserved
        );
        manager.shutdown();
        release_tx.send(()).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            guard.playback_open(),
            "guard stays held while worker is joining"
        );
        assert!(registry.slot.lock().unwrap().is_some());
        harness.allow_join.store(true, Ordering::Release);
        assert!(open.join().unwrap().is_err());
        let deadline = Instant::now() + Duration::from_secs(2);
        while guard.playback_open() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!guard.playback_open());
        assert_eq!(worker_owner.status(), PrepareStatus::Joined);
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(
            PlaybackManager::new(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_reaper_first_waits_for_executor_before_admission() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            done_tx.send(result.is_err()).unwrap();
            release_rx.recv().unwrap();
            claim.complete().unwrap();
            drop(claim);
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        manager.shutdown();
        assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry
            .slot
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .reap_ready_token
            .is_none()
            && Instant::now() < deadline
        {
            thread::yield_now();
        }
        assert!(registry
            .slot
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .reap_ready_token
            .is_some());
        assert!(guard.playback_open());
        assert!(ProcessOpenAdmission::reserve(
            PlaybackManager::new(),
            Arc::clone(&guard),
            Arc::clone(&registry)
        )
        .is_err());
        release_tx.send(()).unwrap();
        open.join().unwrap();
        assert!(!guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(PlaybackManager::new(), guard, registry).unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_reaper_first_executor_panic_keeps_root() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            done_tx.send(result.is_err()).unwrap();
            release_rx.recv().unwrap();
            let _claim = claim;
            panic!("injected executor panic after reaper");
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        manager.shutdown();
        assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while registry
            .slot
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .reap_ready_token
            .is_none()
            && Instant::now() < deadline
        {
            thread::yield_now();
        }
        assert!(registry
            .slot
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .reap_ready_token
            .is_some());
        assert!(guard.playback_open());
        assert!(ProcessOpenAdmission::reserve(
            PlaybackManager::new(),
            Arc::clone(&guard),
            Arc::clone(&registry)
        )
        .is_err());
        release_tx.send(()).unwrap();
        assert!(open.join().is_err());
        assert!(guard.playback_open());
        assert_eq!(
            registry.slot.lock().unwrap().as_ref().unwrap().phase,
            ProcessOwnerPhase::Terminal
        );
        assert!(ProcessOpenAdmission::reserve(PlaybackManager::new(), guard, registry).is_err());
    }

    #[test]
    fn process_owner_executor_panic_first_reaper_keeps_root() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(false, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            done_tx.send(result.is_err()).unwrap();
            release_rx.recv().unwrap();
            let _claim = claim;
            panic!("injected executor panic after reaper");
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        manager.shutdown();
        assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(guard.playback_open());
        release_tx.send(()).unwrap();
        assert!(open.join().is_err());
        harness.allow_join.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let root = registry.slot.lock().unwrap();
            if root.as_ref().unwrap().worker_owner.status() == PrepareStatus::Joined {
                break;
            }
            drop(root);
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert!(guard.playback_open());
        assert_eq!(
            registry.slot.lock().unwrap().as_ref().unwrap().phase,
            ProcessOwnerPhase::Terminal
        );
        assert!(ProcessOpenAdmission::reserve(PlaybackManager::new(), guard, registry).is_err());
    }

    fn exercise_late_preflight_rejects(poison: bool) {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, _) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let (fallback, collision) = {
            let root = registry.slot.lock().unwrap();
            let slot = root.as_ref().unwrap();
            (
                Arc::clone(&slot.late_fallback),
                Arc::clone(&slot.late_collision),
            )
        };
        *fallback.lock().unwrap() = Some(test_session(17, "first", Arc::clone(&guard), None));
        *collision.lock().unwrap() = Some(test_session(17, "second", Arc::clone(&guard), None));
        if poison {
            let target = Arc::clone(&fallback);
            assert!(thread::spawn(move || {
                let _lock = target.lock().unwrap();
                panic!("injected preflight poison");
            })
            .join()
            .is_err());
        }
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        assert!(registry.claim_open(&manager, generation).is_err());
        assert!(registry.claim_open(&manager, generation + 1).is_err());
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            drop(claim);
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(open.join().unwrap().is_err());
        assert!(worker_rx.try_recv().is_err());
        assert_eq!(
            fallback
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .unwrap()
                .handle,
            "first"
        );
        assert_eq!(collision.lock().unwrap().as_ref().unwrap().handle, "second");
        assert!(guard.playback_open());
        assert_eq!(
            registry.slot.lock().unwrap().as_ref().unwrap().phase,
            ProcessOwnerPhase::Terminal
        );
    }

    #[test]
    fn process_owner_full_late_slots_reject_before_worker_start() {
        exercise_late_preflight_rejects(false);
    }

    #[test]
    fn process_owner_poisoned_late_slots_reject_before_worker_start() {
        exercise_late_preflight_rejects(true);
    }

    fn exercise_occupied_late_fallback(poison: bool) {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let token = admission.guard_token;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let (fallback, collision, worker) = {
            let root = registry.slot.lock().unwrap();
            let slot = root.as_ref().unwrap();
            (
                Arc::clone(&slot.late_fallback),
                Arc::clone(&slot.late_collision),
                Arc::clone(&slot.worker_owner),
            )
        };
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            drop(claim);
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let prior = test_session(17, "prior", Arc::clone(&guard), Some(thread::spawn(|| {})));
        let prior_runtime = Arc::clone(&prior.runtime);
        *fallback.lock().unwrap() = Some(prior);
        if poison {
            let target = Arc::clone(&fallback);
            assert!(thread::spawn(move || {
                let _lock = target.lock().unwrap();
                panic!("injected fallback poison");
            })
            .join()
            .is_err());
        }
        release_ready(&harness);
        assert!(open.join().unwrap().is_err());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(prior_runtime.close_requested.load(Ordering::Acquire));
        assert!(guard.playback_open());
        let mut prior = fallback
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .unwrap();
        assert!(prior.join.take().unwrap().join().is_ok());
        let second = collision.lock().unwrap().take().unwrap();
        assert!(second.runtime.close_requested.load(Ordering::Acquire));
        assert!(matches!(second.join, Some(WorkerJoin::Owned(_))));
        worker.claim().unwrap().join().unwrap();
        assert_eq!(guard.try_release_playback_token(token), Ok(true));
    }

    #[test]
    fn process_owner_occupied_late_fallback_retains_both_sessions() {
        exercise_occupied_late_fallback(false);
    }

    #[test]
    fn process_owner_poisoned_occupied_late_fallback_retains_both_sessions() {
        exercise_occupied_late_fallback(true);
    }

    fn exercise_late_install_failure(poison_registry: bool) {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            let _completion = claim.complete();
            drop(claim);
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if poison_registry {
            let root = Arc::clone(&registry);
            assert!(thread::spawn(move || {
                let _lock = root.slot.lock().unwrap();
                panic!("injected late registry poison");
            })
            .join()
            .is_err());
        } else {
            manager.inner.lock().unwrap().opening = None;
        }
        release_ready(&harness);
        assert!(open.join().unwrap().is_err());
        close_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if poison_registry {
            let root = match registry.slot.lock() {
                Err(poisoned) => poisoned.into_inner(),
                Ok(_) => panic!("expected poisoned registry"),
            };
            let slot = root.as_ref().unwrap();
            assert!(slot.late_fallback.lock().unwrap().is_some());
            assert!(guard.playback_open());
            let worker = Arc::clone(&slot.worker_owner);
            drop(root);
            worker.claim().unwrap().join().unwrap();
        } else {
            let deadline = Instant::now() + Duration::from_secs(2);
            while registry.slot.lock().unwrap().is_some() && Instant::now() < deadline {
                thread::yield_now();
            }
            assert!(registry.slot.lock().unwrap().is_none());
            assert!(!guard.playback_open());
            let next = ProcessOpenAdmission::reserve(
                manager.clone(),
                Arc::clone(&guard),
                Arc::clone(&registry),
            )
            .unwrap();
            drop(next);
        }
    }

    #[test]
    fn process_owner_late_install_failure_reaps_rooted_session() {
        exercise_late_install_failure(false);
    }

    #[test]
    fn process_owner_late_install_registry_poison_retains_and_suppresses_session() {
        exercise_late_install_failure(true);
    }

    #[test]
    fn process_owner_real_worker_is_joined_through_capsule_on_close() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (prepare_tx, prepare_rx) = mpsc::channel();
        let (harness, worker_rx, _close_rx) = worker_harness();
        harness.allow_join.store(true, Ordering::Release);
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_tx, Arc::new(AtomicBool::new(true))),
            worker: worker_factory(harness.clone()),
        };
        admission.handoff(io, Duration::from_secs(2), 17).unwrap();
        let slot = registry.slot.lock().unwrap();
        let owner = Arc::clone(&slot.as_ref().unwrap().worker_owner);
        drop(slot);
        let mut claim = registry.claim_open(&manager, generation).unwrap();
        let task = claim.take_task().unwrap();
        let open = thread::spawn(move || {
            let result = open_playback_owned(task);
            claim.complete().unwrap();
            drop(claim);
            result
        });
        prepare_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while owner.status() != PrepareStatus::Published && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(owner.status(), PrepareStatus::Published);
        release_ready(&harness);
        let state = open.join().unwrap().unwrap();
        assert!(tauri::async_runtime::block_on(dispatch_owned_close(
            manager.clone(),
            17,
            &state.handle
        ))
        .is_ok());
        assert_eq!(owner.status(), PrepareStatus::Joined);
        assert!(!guard.playback_open());
        assert!(registry.slot.lock().unwrap().is_none());
        let next = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        drop(next);
    }

    #[test]
    fn process_owner_failed_reaper_spawn_retains_worker_handle() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let guard_token = admission.guard_token;
        let cancel = Arc::clone(&admission.lease.cancel);
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let capsule = Arc::clone(&registry.slot.lock().unwrap().as_ref().unwrap().worker_owner);
        let mut permit = capsule.reserve_permit().unwrap();
        let (release_tx, release_rx) = mpsc::channel();
        permit
            .attach(thread::spawn(move || {
                release_rx.recv().unwrap();
            }))
            .unwrap();
        permit.publish().unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let (_ready_tx, ready_rx) = mpsc::channel();
        let worker = WorkerLaunch {
            command_tx,
            ready_rx,
            join: WorkerJoin::Owned(Arc::clone(&capsule)),
        };
        let result = spawn_open_reaper_with_spawn(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            cancel,
            generation,
            None,
            Some(worker),
            |_| Err(std::io::Error::other("injected reaper spawn failure")),
        );
        assert!(result.is_err());
        assert_eq!(capsule.status(), PrepareStatus::Published);
        assert!(guard.playback_open());
        release_tx.send(()).unwrap();
        capsule.claim().unwrap().join().unwrap();
        let task_capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        let _task = task_capsule.lock().unwrap().take().unwrap();
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_failed_reaper_spawn_retains_preparation_handle() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let guard_token = admission.guard_token;
        let cancel = Arc::clone(&admission.lease.cancel);
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let capsule = Arc::clone(
            &registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .prepare_owner,
        );
        let mut permit = capsule.reserve_permit().unwrap();
        let (release_tx, release_rx) = mpsc::channel();
        permit
            .attach(thread::spawn(move || {
                release_rx.recv().unwrap();
            }))
            .unwrap();
        permit.publish().unwrap();
        let result = spawn_open_reaper_with_spawn(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            cancel,
            generation,
            Some(PrepareJoin::Owned(Arc::clone(&capsule))),
            None,
            |_| Err(std::io::Error::other("injected reaper spawn failure")),
        );
        assert!(result.is_err());
        assert_eq!(capsule.status(), PrepareStatus::Published);
        assert!(guard.playback_open());
        release_tx.send(()).unwrap();
        capsule.claim().unwrap().join().unwrap();
        let task_capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        let _task = task_capsule.lock().unwrap().take().unwrap();
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_claim_drop_restores_task_and_excludes_second_claimant() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        let claim = registry.claim_open(&manager, generation).unwrap();
        assert!(
            registry.claim_open(&manager, generation).is_err(),
            "only one claimant may own the task"
        );
        drop(claim);
        let claim = registry.claim_open(&manager, generation).unwrap();
        drop(claim);
        assert!(guard.playback_open());
        assert!(registry.release(&manager, generation).is_err());
        // No worker started. Dispose this isolated test's restored task.
        let capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        let _task = capsule.lock().unwrap().take().unwrap();
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
        manager.abort_open(generation);
    }

    #[test]
    fn process_owner_claim_after_shutdown_keeps_untouched_task_for_cleanup() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let generation = admission.lease.generation;
        let (started_tx, started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        admission.handoff(io, Duration::from_secs(1), 17).unwrap();
        manager.shutdown();
        assert!(
            registry.claim_open(&manager, generation).is_err(),
            "shutdown must not start a normal open"
        );
        assert!(started_rx.try_recv().is_err());
        assert!(
            guard.playback_open(),
            "untouched task still requires cleanup"
        );
        let capsule = Arc::clone(
            registry
                .slot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pending_open
                .as_ref()
                .unwrap(),
        );
        let _task = capsule.lock().unwrap().take().unwrap();
        registry.slot.lock().unwrap().as_mut().unwrap().phase = ProcessOwnerPhase::PreHandoff;
        registry.release(&manager, generation).unwrap();
        guard.release_playback();
    }

    #[test]
    fn process_owner_handoff_after_shutdown_rolls_back_without_worker() {
        let registry = Arc::new(ProcessOwnerRegistry::new());
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let admission = ProcessOpenAdmission::reserve(
            manager.clone(),
            Arc::clone(&guard),
            Arc::clone(&registry),
        )
        .unwrap();
        let (started_tx, _started_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(started_tx, Arc::new(AtomicBool::new(false))),
            worker: worker_factory(harness),
        };
        manager.shutdown();
        assert!(admission.handoff(io, Duration::from_secs(1), 17).is_err());
        assert!(!guard.playback_open());
        assert!(registry.owner().is_none());
    }

    #[test]
    fn owned_close_poisoned_capsule_is_terminal_not_pending() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        manager.install_test(test_session(17, "poison", Arc::clone(&guard), None));
        let poison_manager = manager.clone();
        let future = dispatch_owned_close_with_spawn(manager.clone(), 17, "poison", move |work| {
            let capsule = Arc::clone(
                &poison_manager
                    .inner
                    .lock()
                    .unwrap()
                    .closing
                    .as_ref()
                    .unwrap()
                    .capsule,
            );
            let _ = thread::spawn(move || {
                let _held = capsule.lock().unwrap();
                panic!("inject capsule poisoning");
            })
            .join();
            thread::Builder::new().spawn(work)
        });
        assert!(tauri::async_runtime::block_on(future).is_err());
        let error = tauri::async_runtime::block_on(dispatch_owned_close(manager, 17, "poison"))
            .unwrap_err();
        assert_eq!(error.code, "NATIVE_UNAVAILABLE");
        assert!(!error.retryable);
        assert!(guard.playback_open());
    }

    #[test]
    fn owned_close_panicked_worker_keeps_guard_and_no_receipt() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let worker = thread::spawn(|| panic!("injected worker panic"));
        manager.install_test(test_session(17, "panic", Arc::clone(&guard), Some(worker)));
        let error =
            tauri::async_runtime::block_on(dispatch_owned_close(manager.clone(), 17, "panic"))
                .unwrap_err();
        assert_eq!(error.code, "NATIVE_UNAVAILABLE");
        assert!(guard.playback_open());
        assert!(manager.begin_open().is_err());
        let retry =
            tauri::async_runtime::block_on(dispatch_owned_close(manager, 17, "panic")).unwrap_err();
        assert_eq!(retry.code, "NATIVE_UNAVAILABLE");
        assert!(!retry.retryable);
    }

    #[test]
    fn owned_close_duplicate_stays_pending_until_join_finishes() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let (tx, rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = rx.recv();
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        let mut session = test_session(17, "pending", Arc::clone(&guard), Some(worker));
        session.command_tx = tx;
        manager.install_test(session);
        let first = dispatch_owned_close(manager.clone(), 17, "pending");
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let duplicate = dispatch_owned_close(manager.clone(), 17, "pending");
        assert_eq!(
            tauri::async_runtime::block_on(duplicate).unwrap_err().code,
            "PLAYBACK_BUSY"
        );
        assert!(guard.playback_open());
        assert!(manager.begin_open().is_err());
        release_tx.send(()).unwrap();
        assert!(tauri::async_runtime::block_on(first).is_ok());
        assert!(!guard.playback_open());
    }

    #[test]
    fn playback_owner_stale_reaper_cannot_release_newer_guard() {
        let manager = PlaybackManager::new();
        let opening = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let guard_token = guard.try_open_playback_token().unwrap();
        let (release_tx, release_rx) = mpsc::channel();
        let prepare = thread::spawn(move || {
            release_rx.recv().unwrap();
        });
        let reaper = spawn_open_reaper(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&opening.cancel),
            opening.generation,
            Some(PrepareJoin::Direct(prepare)),
            None,
        )
        .unwrap();
        guard.release_playback();
        guard.try_open_playback().unwrap();
        release_tx.send(()).unwrap();
        reaper.join().unwrap();
        assert!(
            guard.playback_open(),
            "stale reaper cannot release the newer playback guard"
        );
        assert!(
            manager.begin_open().is_err(),
            "stale cleanup cannot acknowledge opening cleanup"
        );
        guard.release_playback();
    }

    #[test]
    fn playback_owner_stale_prepare_error_cannot_release_newer_guard() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (harness, _worker_rx, _close_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: Box::new(move |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Err(playback_error(
                    "PLAYBACK_IO_FAILED",
                    "injected failure",
                    false,
                ))
            }),
            worker: worker_factory(harness),
        };
        let task = admitted_task(
            manager.clone(),
            Arc::clone(&guard),
            io,
            Duration::from_secs(2),
        );
        let open = thread::spawn(move || open_playback_owned(task));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        guard.release_playback();
        guard.try_open_playback().unwrap();
        release_tx.send(()).unwrap();
        assert!(open.join().unwrap().is_err());
        assert!(
            guard.playback_open(),
            "stale prepare error must not release newer guard"
        );
        assert!(
            manager.begin_open().is_err(),
            "stale rollback must not acknowledge cleanup"
        );
        guard.release_playback();
    }

    #[test]
    fn playback_owner_open_reaper_does_not_release_guard_after_worker_panic() {
        let manager = PlaybackManager::new();
        let opening = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let guard_token = guard.try_open_playback_token().unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let (_ready_tx, ready_rx) = mpsc::channel();
        let worker = WorkerLaunch {
            command_tx,
            ready_rx,
            join: WorkerJoin::Direct(thread::spawn(|| panic!("simulated opening worker panic"))),
        };
        let reaper = spawn_open_reaper(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&opening.cancel),
            opening.generation,
            None,
            Some(worker),
        )
        .unwrap();
        reaper.join().unwrap();
        assert!(
            guard.playback_open(),
            "unproven worker teardown must retain admission guard"
        );
        assert!(
            manager.begin_open().is_err(),
            "unproven worker teardown must retain the opening reservation"
        );
    }

    #[test]
    fn playback_owner_open_reaper_does_not_release_guard_after_prepare_panic() {
        let manager = PlaybackManager::new();
        let opening = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let guard_token = guard.try_open_playback_token().unwrap();
        let prepare = thread::spawn(|| panic!("simulated prepare panic"));
        let reaper = spawn_open_reaper(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&opening.cancel),
            opening.generation,
            Some(PrepareJoin::Direct(prepare)),
            None,
        )
        .unwrap();
        reaper.join().unwrap();
        assert!(guard.playback_open());
        assert!(manager.begin_open().is_err());
    }

    #[test]
    fn playback_owner_open_reaper_joins_delayed_prepare_after_worker_panic_and_shutdown() {
        let manager = PlaybackManager::new();
        let opening = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let guard_token = guard.try_open_playback_token().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let prepare = thread::spawn(move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        entered_rx.recv().unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let (_ready_tx, ready_rx) = mpsc::channel();
        let worker = WorkerLaunch {
            command_tx,
            ready_rx,
            join: WorkerJoin::Direct(thread::spawn(|| panic!("simulated opening worker panic"))),
        };
        let reaper = spawn_open_reaper(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            Arc::clone(&opening.cancel),
            opening.generation,
            Some(PrepareJoin::Direct(prepare)),
            Some(worker),
        )
        .unwrap();
        manager.shutdown();
        assert!(guard.playback_open());
        assert!(!reaper.is_finished(), "the second owner remains blocked");
        release_tx.send(()).unwrap();
        reaper.join().unwrap();
        assert!(
            guard.playback_open(),
            "panic must not release admission after shutdown"
        );
        assert!(manager.begin_open().is_err());
    }

    #[test]
    fn healthy_legacy_import_prepares_without_an_output_device() {
        let (directory, storage) = open_storage();
        let project_root = directory.path().join("projects").join("p1");
        std::fs::create_dir_all(&project_root).unwrap();
        let path = project_root.join("part-a.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..80 {
            writer.write_sample(1_i16).unwrap();
        }
        writer.finalize().unwrap();
        let byte_size = std::fs::metadata(&path).unwrap().len();
        genesis_adapter::commit_rows(&storage, vec![
            genesis_adapter::upsert("projects", serde_json::json!({
                "id":"p1","name":"P1","storage_path":project_root,"created_at":"t","updated_at":"t"
            })),
            genesis_adapter::upsert("recordings", serde_json::json!({
                "id":"r1","project_id":"p1","source":"import","canonical_audio_path":path,
                "status":"completed","duration_ms":10,"created_at":"t","updated_at":"t"
            })),
            genesis_adapter::upsert("audio_chunks", serde_json::json!({
                "id":"a","recording_id":"r1","sequence_no":1,"file_path":path,
                "start_ms":0,"end_ms":10,"byte_size":byte_size,"checksum":"x","created_at":"t"
            })),
        ]).unwrap();
        let prepared = prepare_playback(
            &storage,
            directory.path(),
            &recording_row("p1", "r1", 10),
            "file".into(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(!prepared.degraded);
        assert!(prepared.missing_ranges.is_empty());
        assert_eq!(prepared.duration_frames, 80);
        assert_eq!(prepared.descriptors.len(), 1);
    }

    #[test]
    fn preparation_discloses_missing_chunks_and_timeline_gaps() {
        let (directory, storage) = open_storage();
        let project_root = directory.path().join("projects").join("p1");
        std::fs::create_dir_all(&project_root).unwrap();
        let path = project_root.join("part-a.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..80 {
            writer.write_sample(1_i16).unwrap();
        }
        writer.finalize().unwrap();
        let byte_size = std::fs::metadata(&path).unwrap().len();
        genesis_adapter::commit_rows(
            &storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({"id":"p1","name":"P1","storage_path":project_root.display().to_string(),"active_recording_id":null,"created_at":"2026-09-17T00:00:00Z","updated_at":"2026-09-17T00:00:00Z"}),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":project_root.display().to_string(),"status":"completed","duration_ms":35,"created_at":"2026-09-17T00:00:00Z","updated_at":"2026-09-17T00:00:00Z"}),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({"id":"a","recording_id":"r1","sequence_no":1,"file_path":path.display().to_string(),"start_ms":5,"end_ms":15,"byte_size":byte_size,"checksum":"x","created_at":"2026-09-17T00:00:00Z"}),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({"id":"b","recording_id":"r1","sequence_no":2,"file_path":project_root.join("missing.wav").display().to_string(),"start_ms":25,"end_ms":30,"byte_size":100,"checksum":"x","created_at":"2026-09-17T00:00:00Z"}),
                ),
            ],
        )
        .unwrap();
        let prepared = prepare_playback(
            &storage,
            directory.path(),
            &recording_row("p1", "r1", 35),
            "file".to_string(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(prepared.degraded);
        assert!(prepared
            .missing_ranges
            .iter()
            .any(|range| range.reason == "missing_chunk"));
        assert!(prepared
            .missing_ranges
            .iter()
            .any(|range| range.reason == "timeline_gap"));
        assert!(prepared
            .missing_ranges
            .iter()
            .any(|range| range.start_ms == 0 && range.end_ms == 5));
        assert!(prepared
            .missing_ranges
            .iter()
            .any(|range| range.start_ms == 30 && range.end_ms == 35));
        drop(storage);
    }

    #[test]
    fn preparation_rejects_overlapping_registered_timelines() {
        let (directory, storage) = open_storage();
        let project_root = directory.path().join("projects").join("p1");
        std::fs::create_dir_all(&project_root).unwrap();
        let path = project_root.join("part-a.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..80 {
            writer.write_sample(1_i16).unwrap();
        }
        writer.finalize().unwrap();
        let byte_size = std::fs::metadata(&path).unwrap().len();
        genesis_adapter::commit_rows(
            &storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    serde_json::json!({"id":"p1","name":"P1","storage_path":project_root.display().to_string(),"active_recording_id":null,"created_at":"2026-09-17T00:00:00Z","updated_at":"2026-09-17T00:00:00Z"}),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    serde_json::json!({"id":"r1","project_id":"p1","source":"import","input_path":null,"canonical_audio_path":project_root.display().to_string(),"status":"completed","duration_ms":15,"created_at":"2026-09-17T00:00:00Z","updated_at":"2026-09-17T00:00:00Z"}),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({"id":"a","recording_id":"r1","sequence_no":1,"file_path":path.display().to_string(),"start_ms":0,"end_ms":10,"byte_size":byte_size,"checksum":"x","created_at":"2026-09-17T00:00:00Z"}),
                ),
                genesis_adapter::upsert(
                    "audio_chunks",
                    serde_json::json!({"id":"b","recording_id":"r1","sequence_no":2,"file_path":path.display().to_string(),"start_ms":5,"end_ms":15,"byte_size":byte_size,"checksum":"x","created_at":"2026-09-17T00:00:00Z"}),
                ),
            ],
        )
        .unwrap();
        let error = prepare_playback(
            &storage,
            directory.path(),
            &recording_row("p1", "r1", 15),
            "file".to_string(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
        assert_eq!(error.code, "PLAYBACK_TIMELINE_INVALID");
        drop(storage);
    }

    #[test]
    fn path_custody_rejects_ads_and_parent_escape() {
        assert!(reject_untrusted_path("chunk.wav").is_ok());
        assert!(reject_untrusted_path("..\\outside.wav").is_err());
        assert!(reject_untrusted_path("chunk.wav:secret").is_err());
        assert!(reject_untrusted_path("C:relative.wav").is_err());
        assert!(reject_untrusted_path(r"\\server\share\chunk.wav").is_err());
    }

    #[test]
    fn queue_never_exceeds_one_megabyte() {
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let frame = [1_i16, -1_i16];
        let mut count = 0;
        while queue_frame(&runtime, frame) {
            count += 1;
            if count > MAX_QUEUE_BYTES {
                break;
            }
        }
        assert!(queue_len(&runtime) * std::mem::size_of::<i16>() <= MAX_QUEUE_BYTES);
    }

    #[test]
    fn source_deleted_after_prepare_returns_truthful_failure() {
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 2,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            &[1_i16; 16],
        );
        let byte_size = std::fs::metadata(&path).unwrap().len();
        let prepared = PreparedPlayback {
            project_root: directory.path().to_path_buf(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1,
            sample_rate: 8_000,
            source_channels: 2,
            duration_frames: 8,
            descriptors: vec![ChunkDescriptor {
                id: "chunk".to_string(),
                path: path.display().to_string(),
                sequence_no: 1,
                start_ms: 0,
                end_ms: 1,
                byte_size,
                frame_start: 0,
                frame_end: 8,
                available: true,
            }],
            degraded: false,
            missing_ranges: Vec::new(),
            routed: None,
        };
        std::fs::remove_file(&path).unwrap();
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let mut cursor = 0;
        let mut source_index = None;
        let mut source = None;
        let error = fill_queue(
            &prepared,
            &runtime,
            &mut cursor,
            &mut source_index,
            &mut source,
        )
        .unwrap_err();
        assert_eq!(error.code, "PLAYBACK_SOURCE_MISSING");
    }

    #[test]
    fn eof_closes_source_and_clears_source_identity() {
        let (directory, path) = temp_wave(
            hound::WavSpec {
                channels: 2,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
            &[1_i16; 16],
        );
        let project_root =
            std::fs::canonicalize(directory.path()).expect("fixture project root canonicalization");
        let prepared = PreparedPlayback {
            project_root: project_root.clone(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1,
            sample_rate: 8_000,
            source_channels: 2,
            duration_frames: 8,
            descriptors: vec![ChunkDescriptor {
                id: "chunk".to_string(),
                path: path.display().to_string(),
                sequence_no: 1,
                start_ms: 0,
                end_ms: 1,
                byte_size: std::fs::metadata(&path).unwrap().len(),
                frame_start: 0,
                frame_end: 8,
                available: true,
            }],
            degraded: false,
            missing_ranges: Vec::new(),
            routed: None,
        };
        let runtime = PlaybackRuntime::new(false, Vec::new());
        let mut cursor = 0;
        let mut source_index = None;
        let mut source = None;
        let result = fill_queue(
            &prepared,
            &runtime,
            &mut cursor,
            &mut source_index,
            &mut source,
        );
        #[cfg(windows)]
        report_fixture_path_failure_if_needed(
            &project_root,
            &prepared.descriptors[0],
            &result,
            result.is_ok(),
        );
        result.unwrap();
        assert_eq!(cursor, prepared.duration_frames);
        assert!(source.is_none());
        assert!(source_index.is_none());
    }

    #[test]
    fn seeking_to_zero_keeps_one_frame_source_playable() {
        let manager = PlaybackManager::new();
        let runtime = Arc::new(PlaybackRuntime::new(false, Vec::new()));
        let (command_tx, _command_rx) = mpsc::channel();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 7,
            handle: "one-frame".into(),
            project_id: "p1".into(),
            recording_id: "r1".into(),
            channel: "source".into(),
            duration_ms: 0,
            duration_frames: 1,
            sample_rate: 8_000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        manager.control(7, "one-frame", 0, "seek", Some(0)).unwrap();
        assert!(!runtime.source_exhausted.load(Ordering::Acquire));
        assert_ne!(runtime.state.load(Ordering::Acquire), STATE_ENDED);
    }

    #[test]
    fn concurrent_seek_controls_serialize_epoch_and_enqueue() {
        let manager = PlaybackManager::new();
        let runtime = Arc::new(PlaybackRuntime::new(false, Vec::new()));
        let (command_tx, command_rx) = mpsc::channel();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 7,
            handle: "playback".to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            duration_frames: 8_000,
            sample_rate: 8_000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let first_barrier = Arc::clone(&barrier);
        let first_manager = manager.clone();
        let first = thread::spawn(move || {
            first_barrier.wait();
            first_manager.control(7, "playback", 0, "seek", Some(100))
        });
        let second_barrier = Arc::clone(&barrier);
        let second_manager = manager.clone();
        let second = thread::spawn(move || {
            second_barrier.wait();
            second_manager.control(7, "playback", 0, "seek", Some(200))
        });
        barrier.wait();
        let first = first.join().unwrap();
        let second = second.join().unwrap();
        let outcomes = [first, second];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|result| {
                    matches!(result, Err(error) if error.code == "PLAYBACK_STALE_EPOCH")
                })
                .count(),
            1
        );
        assert_eq!(runtime.stream_epoch.load(Ordering::Acquire), 1);
        assert!(matches!(
            command_rx.recv().unwrap(),
            WorkerCommand::Seek { epoch: 1, .. }
        ));
    }

    #[test]
    fn concurrent_control_and_close_have_one_serialized_owner() {
        let manager = PlaybackManager::new();
        let (command_tx, command_rx) = mpsc::channel();
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 9,
            handle: "close-race".to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            duration_frames: 8_000,
            sample_rate: 8_000,
            runtime: Arc::new(PlaybackRuntime::new(false, Vec::new())),
            command_tx,
            join: None,
            guard: test_held_guard(),
        });
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let control_barrier = Arc::clone(&barrier);
        let control_manager = manager.clone();
        let control = thread::spawn(move || {
            control_barrier.wait();
            control_manager.control(9, "close-race", 0, "play", None)
        });
        let close_barrier = Arc::clone(&barrier);
        let close_manager = manager.clone();
        let close = thread::spawn(move || {
            close_barrier.wait();
            close_manager.take_for_close(9, "close-race")
        });
        barrier.wait();
        let control = control.join().unwrap();
        let session = close.join().unwrap().unwrap().unwrap();
        match control {
            Ok(_) => {
                let command = command_rx.recv().unwrap();
                assert!(matches!(command, WorkerCommand::Play));
            }
            Err(error) => assert_eq!(error.code, "PLAYBACK_HANDLE_INVALID"),
        }
        let _ = session.command_tx.send(WorkerCommand::Close);
        session.guard.release_playback();
        manager.mark_closed(session.handle, session.owner, None);
        assert!(manager.take_for_close(9, "close-race").unwrap().is_none());
    }

    #[test]
    fn one_open_deadline_expires_delayed_readiness_without_a_second_window() {
        let base = Instant::now();
        let deadline = base + Duration::from_secs(1);
        let ticks = AtomicU64::new(0);
        let clock = || base + Duration::from_millis(ticks.load(Ordering::Acquire));
        let cancel = AtomicBool::new(false);
        let (prepared_tx, prepared_rx) = mpsc::channel();
        prepared_tx.send(()).unwrap();
        assert_eq!(
            recv_until_deadline_or_cancel_with_clock(&prepared_rx, deadline, &cancel, &clock),
            Ok(())
        );
        let (ready_tx, ready_rx) = mpsc::channel();
        ready_tx.send(()).unwrap();
        ticks.store(1_000, Ordering::Release);
        assert_eq!(
            recv_until_deadline_or_cancel_with_clock(&ready_rx, deadline, &cancel, &clock),
            Err(OpenWaitError::Timeout)
        );
        assert_eq!(ready_rx.try_recv(), Err(mpsc::TryRecvError::Empty));
        cancel.store(true, Ordering::Release);
        assert_eq!(
            recv_until_deadline_or_cancel_with_clock(&ready_rx, deadline, &cancel, &clock),
            Err(OpenWaitError::Cancelled)
        );
        cancel.store(false, Ordering::Release);
        let later_deadline = base + Duration::from_secs(2);
        drop(ready_tx);
        assert_eq!(
            recv_until_deadline_or_cancel_with_clock(&ready_rx, later_deadline, &cancel, &clock),
            Err(OpenWaitError::Disconnected)
        );
    }

    #[test]
    fn open_reaper_retains_admission_until_delayed_prepare_quiesces() {
        let manager = PlaybackManager::new();
        let lease = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        let guard_token = guard.try_open_playback_token().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let prepare_cancel = Arc::clone(&cancel);
        let (allow_tx, allow_rx) = mpsc::channel();
        let prepare_join = thread::spawn(move || {
            allow_rx.recv().unwrap();
            while !prepare_cancel.load(Ordering::Acquire) {
                thread::yield_now();
            }
        });
        let reaper = spawn_open_reaper(
            manager.clone(),
            Arc::clone(&guard),
            guard_token,
            cancel,
            lease.generation,
            Some(PrepareJoin::Direct(prepare_join)),
            None,
        )
        .unwrap();
        assert_eq!(manager.begin_open().unwrap_err().code, "PLAYBACK_BUSY");
        assert!(guard.playback_open());
        allow_tx.send(()).unwrap();
        reaper.join().unwrap();
        assert!(!guard.playback_open());
        manager.begin_open().unwrap();
    }

    #[test]
    fn shutdown_cancels_delayed_open_before_late_install_and_owned_cleanup_releases_guard() {
        let manager = PlaybackManager::new();
        let lease = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let cancel = Arc::clone(&lease.cancel);
        let generation = lease.generation;
        let delayed_manager = manager.clone();
        let delayed_guard = Arc::clone(&guard);
        let (prepare_release_tx, prepare_release_rx) = mpsc::channel();
        let delayed_open = thread::spawn(move || {
            prepare_release_rx
                .recv()
                .expect("delayed preparation release");
            assert!(cancel.load(Ordering::Acquire));
            let session = test_session(17, "late-open", delayed_guard, None);
            let failure = delayed_manager
                .install(generation, session)
                .expect_err("disposed generation must reject a late install");
            let PlaybackInstallFailure { error, session } = *failure;
            assert_eq!(error.code, "NATIVE_UNAVAILABLE");
            let cleanup = spawn_session_cleanup(delayed_manager, session).unwrap();
            cleanup.join().unwrap();
        });

        manager.shutdown();
        assert!(manager.begin_open().is_err());
        prepare_release_tx.send(()).unwrap();
        // Completion follows owned cleanup, independent of test-runner scheduling.
        delayed_open.join().unwrap();
        assert!(!guard.playback_open());
    }

    #[test]
    fn shutdown_returns_before_delayed_worker_join_and_cleanup_owner_retains_guard() {
        let manager = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let (close_seen_tx, close_seen_rx) = mpsc::channel();
        let (allow_join_tx, allow_join_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            assert!(matches!(command_rx.recv().unwrap(), WorkerCommand::Close));
            close_seen_tx.send(()).unwrap();
            allow_join_rx.recv().unwrap();
        });
        manager.install_test(PlaybackSession {
            process_identity: None,
            guard_token: 1,
            owner: 23,
            handle: "delayed-close".to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            duration_frames: 8_000,
            sample_rate: 8_000,
            runtime: Arc::new(PlaybackRuntime::new(false, Vec::new())),
            command_tx,
            join: Some(WorkerJoin::Direct(worker)),
            guard: Arc::clone(&guard),
        });

        manager.shutdown();
        close_seen_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("owned cleanup sent close without joining on dispatch");
        assert!(guard.playback_open());
        allow_join_tx.send(()).unwrap();

        let deadline = Instant::now() + Duration::from_millis(100);
        while guard.playback_open() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!guard.playback_open());
        let deadline = Instant::now() + Duration::from_millis(100);
        while manager.take_for_close(23, "delayed-close").is_err() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(manager
            .take_for_close(23, "delayed-close")
            .unwrap()
            .is_none());
    }

    #[test]
    fn production_open_dispatch_runs_delayed_prepare_and_readiness_without_cpal() {
        let playback = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_started_tx, prepare_started_rx) = mpsc::channel();
        let prepare_release = Arc::new(AtomicBool::new(false));
        let (harness, worker_started_rx, _close_seen_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_started_tx, Arc::clone(&prepare_release)),
            worker: worker_factory(harness.clone()),
        };
        let open = spawn_open_dispatch(admitted_task(
            playback.clone(),
            Arc::clone(&guard),
            io,
            MAX_OPEN_TIMEOUT,
        ));

        prepare_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("production open did not start preparation");
        assert!(
            !open.is_finished(),
            "open returned while preparation was pending"
        );
        prepare_release.store(true, Ordering::Release);
        worker_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("production open did not start the worker");
        assert!(
            !open.is_finished(),
            "open returned while readiness was pending"
        );
        release_ready(&harness);

        let state = open
            .join()
            .unwrap()
            .expect("delayed production open failed");
        assert_eq!(state.state, "paused");
        assert!(guard.playback_open());

        let session = playback
            .take_for_close(17, &state.handle)
            .unwrap()
            .expect("opened session must be owned by playback manager");
        harness.allow_join.store(true, Ordering::Release);
        spawn_close_dispatch(playback, session)
            .join()
            .unwrap()
            .expect("production close dispatch failed");
        assert!(!guard.playback_open());
    }

    #[test]
    fn production_open_dispatch_total_deadline_reaps_delayed_preparation() {
        let playback = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (prepare_started_tx, prepare_started_rx) = mpsc::channel();
        let prepare_release = Arc::new(AtomicBool::new(false));
        let (harness, _worker_started_rx, _close_seen_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: gated_prepare(prepare_started_tx, Arc::clone(&prepare_release)),
            worker: worker_factory(harness),
        };
        let open = spawn_open_dispatch(admitted_task(
            playback.clone(),
            Arc::clone(&guard),
            io,
            Duration::from_millis(50),
        ));

        prepare_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("deadline test did not start preparation");
        let result = open
            .join()
            .unwrap()
            .expect_err("delayed open must time out");
        assert_eq!(result.code, "PLAYBACK_OPEN_TIMEOUT");
        assert!(guard.playback_open(), "reaper released admission too early");
        assert_eq!(playback.begin_open().unwrap_err().code, "PLAYBACK_BUSY");

        prepare_release.store(true, Ordering::Release);
        wait_for_guard_release(&guard);
        let lease = playback
            .begin_open()
            .expect("admission remained busy after preparation quiesced");
        playback.abort_open(lease.generation);
    }

    #[test]
    fn production_open_dispatch_rejects_destroyed_late_readiness_without_install() {
        let playback = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (harness, worker_started_rx, close_seen_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: Box::new(|_cancel| Ok(prepared_fixture())),
            worker: worker_factory(harness.clone()),
        };
        let open = spawn_open_dispatch(admitted_task(
            playback.clone(),
            Arc::clone(&guard),
            io,
            MAX_OPEN_TIMEOUT,
        ));
        worker_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("destroy-during-open test did not start the worker");

        playback.shutdown();
        release_ready(&harness);
        let result = open.join().unwrap().expect_err("destroyed open must fail");
        assert_eq!(result.code, "NATIVE_UNAVAILABLE");
        assert!(playback.inner.lock().unwrap().active.is_none());
        assert!(
            guard.playback_open(),
            "cleanup released before worker quiescence"
        );
        close_seen_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("destroyed open did not close the worker");

        harness.allow_join.store(true, Ordering::Release);
        wait_for_guard_release(&guard);
    }

    #[test]
    fn production_close_dispatch_waits_for_worker_quiescence_before_release() {
        let playback = PlaybackManager::new();
        let guard = Arc::new(NativeCaptureGuard::default());
        let (harness, worker_started_rx, close_seen_rx) = worker_harness();
        let io = OpenPlaybackIo {
            prepare: Box::new(|_cancel| Ok(prepared_fixture())),
            worker: worker_factory(harness.clone()),
        };
        let open = spawn_open_dispatch(admitted_task(
            playback.clone(),
            Arc::clone(&guard),
            io,
            MAX_OPEN_TIMEOUT,
        ));
        worker_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("close test did not start the worker");
        release_ready(&harness);
        let state = open.join().unwrap().expect("open for close test failed");
        let session = playback
            .take_for_close(17, &state.handle)
            .unwrap()
            .expect("close test session missing");

        let close = spawn_close_dispatch(playback.clone(), session);
        close_seen_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("close dispatch did not signal worker close");
        assert!(
            !close.is_finished(),
            "close acknowledged before worker join"
        );
        assert!(guard.playback_open(), "close released admission too early");
        assert_eq!(
            guard.try_open_playback().unwrap_err(),
            AdmissionError::PlaybackBusy
        );

        harness.allow_join.store(true, Ordering::Release);
        close
            .join()
            .unwrap()
            .expect("close dispatch did not finish after worker join");
        assert!(!guard.playback_open());
        assert!(playback
            .take_for_close(17, &state.handle)
            .unwrap()
            .is_none());
    }
}
