//! Native recording-review registry and capture/playback admission boundary.
//!
//! This module owns the bounded recording snapshot protocol used by the
//! desktop bridge.  It deliberately does not expose storage paths, provider
//! configuration, or renderer-controlled ownership values.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Manager, WebviewWindow};

use crate::genesis_adapter;
use crate::{now, AppState};

pub(crate) const MAX_SNAPSHOT_ROWS: usize = 10_000;
pub(crate) const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_SNAPSHOTS_PER_OWNER: usize = 2;
const MAX_EXPIRED_CURSORS: usize = 4_096;
pub(crate) const SNAPSHOT_IDLE_TTL: Duration = Duration::from_secs(5 * 60);
pub(crate) const SNAPSHOT_ABSOLUTE_TTL: Duration = Duration::from_secs(15 * 60);
pub(crate) const MAX_PAGE_LIMIT: usize = 100;
pub(crate) const DEFAULT_PAGE_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewError {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) retryable: bool,
}

impl ReviewError {
    pub(crate) fn new(code: &str, message: &str, retryable: bool) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
        }
    }

    pub(crate) fn invalid(message: &str) -> Self {
        Self::new("INVALID_ARGUMENT", message, false)
    }

    pub(crate) fn storage_read() -> Self {
        Self::new(
            "STORAGE_READ_FAILED",
            "Recording data could not be read.",
            true,
        )
    }

    pub(crate) fn resource_limit() -> Self {
        Self::new(
            "RESOURCE_LIMIT",
            "The requested recording data exceeds the native limit.",
            false,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingRow {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) source: String,
    pub(crate) status: String,
    pub(crate) duration_ms: i64,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) language: Option<String>,
    pub(crate) channels: Vec<String>,
    pub(crate) capture_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingPage {
    pub(crate) project_id: String,
    pub(crate) snapshot_id: String,
    pub(crate) as_of: String,
    pub(crate) items: Vec<RecordingRow>,
    pub(crate) next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReleaseReceipt {
    pub(crate) released: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RecordingIdentity {
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RecordingStorageRow {
    pub(crate) identity: RecordingIdentity,
    pub(crate) source: String,
    pub(crate) status: String,
    pub(crate) duration_ms: i64,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    CaptureActive,
    PlaybackBusy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapturePhase {
    Inactive,
    Starting,
    Active,
    Stopping,
}

#[derive(Debug)]
struct AdmissionState {
    capture: CapturePhase,
    playback_open: bool,
}

/// Serializes the native capture and playback ownership boundary.
///
/// The state intentionally remains in the process, rather than being
/// renderer-controlled.  A failed start must call `release_capture`; a
/// successful capture transitions through active/stopping until its
/// coordinator has actually released resources.
#[derive(Debug)]
pub(crate) struct NativeCaptureGuard {
    inner: Mutex<AdmissionState>,
}

pub(crate) struct CaptureReservation {
    guard: std::sync::Arc<NativeCaptureGuard>,
    committed: bool,
}

impl Default for NativeCaptureGuard {
    fn default() -> Self {
        Self {
            inner: Mutex::new(AdmissionState {
                capture: CapturePhase::Inactive,
                playback_open: false,
            }),
        }
    }
}

impl NativeCaptureGuard {
    pub(crate) fn reserve_capture(
        guard: std::sync::Arc<Self>,
    ) -> Result<CaptureReservation, AdmissionError> {
        guard.try_start_capture()?;
        Ok(CaptureReservation {
            guard,
            committed: false,
        })
    }

    pub(crate) fn try_start_capture(&self) -> Result<(), AdmissionError> {
        let mut state = self.inner.lock().expect("capture admission mutex poisoned");
        if state.playback_open {
            return Err(AdmissionError::PlaybackBusy);
        }
        if state.capture != CapturePhase::Inactive {
            return Err(AdmissionError::CaptureActive);
        }
        state.capture = CapturePhase::Starting;
        Ok(())
    }

    pub(crate) fn mark_capture_active(&self) {
        let mut state = self.inner.lock().expect("capture admission mutex poisoned");
        if state.capture == CapturePhase::Starting {
            state.capture = CapturePhase::Active;
        }
    }

    pub(crate) fn mark_capture_stopping(&self) {
        let mut state = self.inner.lock().expect("capture admission mutex poisoned");
        if matches!(state.capture, CapturePhase::Starting | CapturePhase::Active) {
            state.capture = CapturePhase::Stopping;
        }
    }

    pub(crate) fn release_capture(&self) {
        let mut state = self.inner.lock().expect("capture admission mutex poisoned");
        state.capture = CapturePhase::Inactive;
    }

    pub(crate) fn try_open_playback(&self) -> Result<(), AdmissionError> {
        let mut state = self.inner.lock().expect("capture admission mutex poisoned");
        if state.playback_open {
            return Err(AdmissionError::PlaybackBusy);
        }
        if state.capture != CapturePhase::Inactive {
            return Err(AdmissionError::CaptureActive);
        }
        state.playback_open = true;
        Ok(())
    }

    pub(crate) fn release_playback(&self) {
        let mut state = self.inner.lock().expect("capture admission mutex poisoned");
        state.playback_open = false;
    }

    pub(crate) fn capture_is_active(&self) -> bool {
        self.inner
            .lock()
            .expect("capture admission mutex poisoned")
            .capture
            != CapturePhase::Inactive
    }

    #[cfg(test)]
    pub(crate) fn capture_phase(&self) -> CapturePhase {
        self.inner
            .lock()
            .expect("capture admission mutex poisoned")
            .capture
    }

    #[cfg(test)]
    pub(crate) fn playback_open(&self) -> bool {
        self.inner
            .lock()
            .expect("capture admission mutex poisoned")
            .playback_open
    }
}

impl CaptureReservation {
    pub(crate) fn commit_active(mut self) {
        self.guard.mark_capture_active();
        self.committed = true;
    }
}

impl Drop for CaptureReservation {
    fn drop(&mut self) {
        if !self.committed {
            self.guard.release_capture();
        }
    }
}

#[derive(Debug, Clone)]
struct Snapshot {
    id: String,
    owner: u128,
    project_id: String,
    as_of: String,
    limit: usize,
    items: Vec<RecordingRow>,
    created_at: Instant,
    last_used: Instant,
}

#[derive(Debug, Clone)]
struct CursorRecord {
    snapshot_id: String,
    owner: u128,
    project_id: String,
    limit: usize,
    next_index: usize,
    following_cursor: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ReviewRegistry {
    snapshots: Vec<Snapshot>,
    cursors: HashMap<String, CursorRecord>,
    expired_cursors: HashSet<String>,
    expired_cursor_order: VecDeque<String>,
}

impl ReviewRegistry {
    fn remember_expired_cursor(&mut self, cursor: String) {
        if self.expired_cursors.insert(cursor.clone()) {
            self.expired_cursor_order.push_back(cursor);
        }
        while self.expired_cursor_order.len() > MAX_EXPIRED_CURSORS {
            if let Some(oldest) = self.expired_cursor_order.pop_front() {
                self.expired_cursors.remove(&oldest);
            }
        }
    }

    fn evict_expired(&mut self, now: Instant) {
        let mut expired_snapshot_ids = Vec::new();
        self.snapshots.retain(|snapshot| {
            let expired = now.duration_since(snapshot.last_used) > SNAPSHOT_IDLE_TTL
                || now.duration_since(snapshot.created_at) > SNAPSHOT_ABSOLUTE_TTL;
            if expired {
                expired_snapshot_ids.push(snapshot.id.clone());
            }
            !expired
        });

        let mut expired_cursor_ids = Vec::new();
        self.cursors.retain(|cursor, record| {
            if expired_snapshot_ids
                .iter()
                .any(|id| id == &record.snapshot_id)
            {
                expired_cursor_ids.push(cursor.clone());
                false
            } else {
                true
            }
        });
        for cursor in expired_cursor_ids {
            self.remember_expired_cursor(cursor);
        }
    }

    fn insert_snapshot(
        &mut self,
        owner: u128,
        project_id: String,
        limit: usize,
        items: Vec<RecordingRow>,
        as_of: String,
        now: Instant,
    ) -> String {
        // Keep at most two snapshots for the owner.  The caller has already
        // used `evict_expired`; this removes the oldest live snapshot.
        while self
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.owner == owner)
            .count()
            >= MAX_SNAPSHOTS_PER_OWNER
        {
            if let Some((index, _)) = self
                .snapshots
                .iter()
                .enumerate()
                .filter(|(_, snapshot)| snapshot.owner == owner)
                .min_by_key(|(_, snapshot)| snapshot.created_at)
            {
                let removed = self.snapshots.remove(index);
                let cursors = self
                    .cursors
                    .iter()
                    .filter(|(_, record)| record.snapshot_id == removed.id)
                    .map(|(cursor, _)| cursor.clone())
                    .collect::<Vec<_>>();
                for cursor in cursors {
                    self.cursors.remove(&cursor);
                    self.remember_expired_cursor(cursor);
                }
            } else {
                break;
            }
        }

        let id = random_opaque_id();
        self.snapshots.push(Snapshot {
            id: id.clone(),
            owner,
            project_id,
            as_of,
            limit,
            items,
            created_at: now,
            last_used: now,
        });
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn page(
        &mut self,
        owner: u128,
        project_id: &str,
        limit: usize,
        snapshot_id: &str,
        start: usize,
        cursor_token: Option<&str>,
        now: Instant,
    ) -> Result<RecordingPage, ReviewError> {
        self.evict_expired(now);
        let snapshot_index = self
            .snapshots
            .iter()
            .position(|snapshot| snapshot.id == snapshot_id)
            .ok_or_else(|| {
                ReviewError::new("CURSOR_EXPIRED", "The recording cursor expired.", false)
            })?;
        let snapshot = &mut self.snapshots[snapshot_index];
        if snapshot.owner != owner || snapshot.project_id != project_id || snapshot.limit != limit {
            return Err(ReviewError::new(
                "CURSOR_INVALID",
                "The recording cursor is not valid for this request.",
                false,
            ));
        }
        snapshot.last_used = now;
        let item_count = snapshot.items.len();
        let end = (start + limit).min(item_count);
        let items = snapshot.items[start..end].to_vec();
        let snapshot_id_owned = snapshot.id.clone();
        let snapshot_project_id = snapshot.project_id.clone();
        let snapshot_as_of = snapshot.as_of.clone();
        let next_cursor = if end < item_count {
            if let Some(cursor_token) = cursor_token {
                if let Some(existing) = self
                    .cursors
                    .get(cursor_token)
                    .and_then(|record| record.following_cursor.clone())
                {
                    Some(existing)
                } else {
                    let cursor = random_opaque_id();
                    if let Some(record) = self.cursors.get_mut(cursor_token) {
                        record.following_cursor = Some(cursor.clone());
                    }
                    self.cursors.insert(
                        cursor.clone(),
                        CursorRecord {
                            snapshot_id: snapshot_id_owned.clone(),
                            owner,
                            project_id: project_id.to_string(),
                            limit,
                            next_index: end,
                            following_cursor: None,
                        },
                    );
                    Some(cursor)
                }
            } else {
                let cursor = random_opaque_id();
                self.cursors.insert(
                    cursor.clone(),
                    CursorRecord {
                        snapshot_id: snapshot_id_owned.clone(),
                        owner,
                        project_id: project_id.to_string(),
                        limit,
                        next_index: end,
                        following_cursor: None,
                    },
                );
                Some(cursor)
            }
        } else {
            None
        };
        Ok(RecordingPage {
            project_id: snapshot_project_id,
            snapshot_id: snapshot_id_owned,
            as_of: snapshot_as_of,
            items,
            next_cursor,
        })
    }

    fn first_page(
        &mut self,
        owner: u128,
        project_id: String,
        limit: usize,
        items: Vec<RecordingRow>,
        as_of: String,
        now: Instant,
    ) -> Result<RecordingPage, ReviewError> {
        let snapshot_id = self.insert_snapshot(owner, project_id.clone(), limit, items, as_of, now);
        self.page(owner, &project_id, limit, &snapshot_id, 0, None, now)
    }

    fn cursor_page(
        &mut self,
        owner: u128,
        project_id: &str,
        limit: usize,
        cursor: &str,
        now: Instant,
    ) -> Result<RecordingPage, ReviewError> {
        self.evict_expired(now);
        let record = if let Some(record) = self.cursors.get(cursor) {
            record.clone()
        } else if self.expired_cursors.contains(cursor) {
            return Err(ReviewError::new(
                "CURSOR_EXPIRED",
                "The recording cursor expired.",
                false,
            ));
        } else {
            return Err(ReviewError::new(
                "CURSOR_INVALID",
                "The recording cursor is invalid.",
                false,
            ));
        };
        if record.owner != owner || record.project_id != project_id || record.limit != limit {
            return Err(ReviewError::new(
                "CURSOR_INVALID",
                "The recording cursor is not valid for this request.",
                false,
            ));
        }
        self.page(
            owner,
            project_id,
            limit,
            &record.snapshot_id,
            record.next_index,
            Some(cursor),
            now,
        )
    }

    fn release(
        &mut self,
        owner: u128,
        snapshot_id: &str,
        now: Instant,
    ) -> Result<ReleaseReceipt, ReviewError> {
        self.evict_expired(now);
        if let Some(index) = self
            .snapshots
            .iter()
            .position(|snapshot| snapshot.id == snapshot_id)
        {
            if self.snapshots[index].owner != owner {
                return Err(ReviewError::new(
                    "CURSOR_INVALID",
                    "The recording snapshot is not owned by this window.",
                    false,
                ));
            }
            self.snapshots.remove(index);
            let cursors = self
                .cursors
                .iter()
                .filter(|(_, record)| record.snapshot_id == snapshot_id)
                .map(|(cursor, _)| cursor.clone())
                .collect::<Vec<_>>();
            for cursor in cursors {
                self.cursors.remove(&cursor);
                self.remember_expired_cursor(cursor);
            }
        }
        Ok(ReleaseReceipt { released: true })
    }

    pub(crate) fn clear(&mut self) {
        self.snapshots.clear();
        self.cursors.clear();
        self.expired_cursors.clear();
        self.expired_cursor_order.clear();
    }
}

pub(crate) fn random_opaque_id() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn random_owner_id() -> u128 {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    u128::from_le_bytes(bytes)
}

pub(crate) fn validate_opaque_id(value: &str, field: &str) -> Result<(), ReviewError> {
    let scalar_count = value.chars().count();
    if value.is_empty() || scalar_count > 256 || value.contains('\0') {
        return Err(ReviewError::invalid(&format!("{field} is invalid.")));
    }
    if value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(ReviewError::invalid(&format!("{field} is invalid.")));
    }
    Ok(())
}

pub(crate) fn validate_limit(limit: Option<usize>) -> Result<usize, ReviewError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(ReviewError::invalid("limit is invalid."));
    }
    Ok(limit)
}

pub(crate) fn trusted_main_owner(
    window: &WebviewWindow,
    state: &AppState,
) -> Result<u128, ReviewError> {
    let main = window
        .app_handle()
        .get_webview_window("main")
        .ok_or_else(|| {
            ReviewError::new(
                "NATIVE_UNAVAILABLE",
                "The native desktop window is unavailable.",
                true,
            )
        })?;
    if main != *window {
        return Err(ReviewError::new(
            "NATIVE_UNAVAILABLE",
            "The native desktop window is not trusted.",
            false,
        ));
    }
    let url = window.url().map_err(|_| {
        ReviewError::new(
            "NATIVE_UNAVAILABLE",
            "The native desktop origin is unavailable.",
            false,
        )
    })?;
    let trusted_origin =
        (url.scheme() == "http" && url.host_str() == Some("localhost") && url.port() == Some(1420))
            || (url.scheme() == "http"
                && url.host_str() == Some("tauri.localhost")
                && url.port().is_none());
    if !trusted_origin {
        return Err(ReviewError::new(
            "NATIVE_UNAVAILABLE",
            "The native desktop origin is not trusted.",
            false,
        ));
    }
    Ok(state.main_window_owner)
}

fn query_error() -> ReviewError {
    ReviewError::storage_read()
}

fn string_field(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn required_string(row: &Value, key: &str) -> Result<String, ReviewError> {
    string_field(row, key).ok_or_else(|| {
        ReviewError::new(
            "INVALID_RECORDING_METADATA",
            "Recording metadata is invalid.",
            false,
        )
    })
}

fn required_i64(row: &Value, key: &str) -> Result<i64, ReviewError> {
    row.get(key).and_then(Value::as_i64).ok_or_else(|| {
        ReviewError::new(
            "INVALID_RECORDING_METADATA",
            "Recording metadata is invalid.",
            false,
        )
    })
}

fn parse_storage_row(
    row: &Value,
    expected_project: &str,
    expected_recording: &str,
) -> Result<RecordingStorageRow, ReviewError> {
    let project_id = required_string(row, "recordings.project_id")?;
    let recording_id = required_string(row, "recordings.id")?;
    if project_id != expected_project || recording_id != expected_recording {
        return Err(ReviewError::new(
            "SCOPE_MISMATCH",
            "The recording does not belong to the requested project.",
            false,
        ));
    }
    validate_opaque_id(&project_id, "projectId")?;
    validate_opaque_id(&recording_id, "recordingId")?;
    let duration_ms = required_i64(row, "recordings.duration_ms")?;
    if duration_ms < 0 {
        return Err(ReviewError::new(
            "INVALID_RECORDING_METADATA",
            "Recording metadata is invalid.",
            false,
        ));
    }
    let created_at = required_string(row, "recordings.created_at")?;
    let updated_at = required_string(row, "recordings.updated_at")?;
    if chrono::DateTime::parse_from_rfc3339(&created_at).is_err()
        || chrono::DateTime::parse_from_rfc3339(&updated_at).is_err()
    {
        return Err(ReviewError::new(
            "INVALID_RECORDING_METADATA",
            "Recording metadata is invalid.",
            false,
        ));
    }
    Ok(RecordingStorageRow {
        identity: RecordingIdentity {
            project_id,
            recording_id,
        },
        source: required_string(row, "recordings.source")?,
        status: required_string(row, "recordings.status")?,
        duration_ms,
        created_at,
        updated_at,
        language: row
            .get("recordings.language")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

pub(crate) fn channel_for_path(path: &str, source: &str) -> Option<&'static str> {
    let normalized = path.replace('\\', "/");
    let name = normalized
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(rest) = name.strip_prefix("mic-") {
        if rest.ends_with(".wav") && rest[..rest.len() - 4].chars().all(|ch| ch.is_ascii_digit()) {
            return Some("mic");
        }
    }
    if let Some(rest) = name.strip_prefix("system-") {
        if rest.ends_with(".wav") && rest[..rest.len() - 4].chars().all(|ch| ch.is_ascii_digit()) {
            return Some("system");
        }
    }
    if source.eq_ignore_ascii_case("import") || source.eq_ignore_ascii_case("file") {
        return Some("file");
    }
    None
}

fn recording_channels(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
    source: &str,
) -> Result<Vec<String>, ReviewError> {
    let rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &["file_path"],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )
    .map_err(|_| query_error())?;
    let mut channels = HashSet::new();
    for row in rows {
        if let Some(path) = string_field(&row, "audio_chunks.file_path") {
            if let Some(channel) = channel_for_path(&path, source) {
                channels.insert(channel);
            }
        }
    }
    let mut result = ["mic", "system", "file"]
        .into_iter()
        .filter(|channel| channels.contains(channel))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    result.shrink_to_fit();
    Ok(result)
}

fn row_to_public_with_state(
    storage: &genesis_block_native::Storage,
    row: &RecordingStorageRow,
    capture_state: &str,
) -> Result<RecordingRow, ReviewError> {
    Ok(RecordingRow {
        id: row.identity.recording_id.clone(),
        project_id: row.identity.project_id.clone(),
        source: row.source.clone(),
        status: row.status.clone(),
        duration_ms: row.duration_ms,
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
        language: row.language.clone(),
        channels: recording_channels(storage, &row.identity.recording_id, &row.source)?,
        capture_state: capture_state.to_string(),
    })
}

fn live_capture_state(state: &AppState, recording_id: &str) -> String {
    let live = state.live.lock().expect("live session mutex poisoned");
    match live
        .as_ref()
        .filter(|session| session.recording_id == recording_id)
    {
        Some(session) if session.stop.load(std::sync::atomic::Ordering::Acquire) => {
            "stopping".to_string()
        }
        Some(_) => "active".to_string(),
        None => "inactive".to_string(),
    }
}

pub(crate) fn validate_recording_pair(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<RecordingStorageRow, ReviewError> {
    validate_opaque_id(project_id, "projectId")?;
    validate_opaque_id(recording_id, "recordingId")?;
    let projects = genesis_adapter::query(
        storage,
        "projects",
        &["id"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(project_id),
        )],
        1,
    )
    .map_err(|_| query_error())?;
    if projects.is_empty() {
        return Err(ReviewError::new(
            "PROJECT_NOT_FOUND",
            "The project was not found.",
            false,
        ));
    }
    let recordings = genesis_adapter::query(
        storage,
        "recordings",
        &[
            "id",
            "project_id",
            "source",
            "status",
            "duration_ms",
            "created_at",
            "updated_at",
            "language",
        ],
        vec![genesis_adapter::eq(
            "recordings",
            "id",
            serde_json::json!(recording_id),
        )],
        1,
    )
    .map_err(|_| query_error())?;
    let row = recordings.first().ok_or_else(|| {
        ReviewError::new("RECORDING_NOT_FOUND", "The recording was not found.", false)
    })?;
    let actual_project = string_field(row, "recordings.project_id").ok_or_else(|| {
        ReviewError::new(
            "INVALID_RECORDING_METADATA",
            "Recording metadata is invalid.",
            false,
        )
    })?;
    if actual_project != project_id {
        return Err(ReviewError::new(
            "SCOPE_MISMATCH",
            "The recording does not belong to the requested project.",
            false,
        ));
    }
    parse_storage_row(row, project_id, recording_id)
}

fn snapshot_size(items: &[RecordingRow]) -> Result<usize, ReviewError> {
    items.iter().try_fold(0usize, |total, item| {
        let bytes = serde_json::to_vec(item)
            .map_err(|_| ReviewError::storage_read())?
            .len();
        total
            .checked_add(bytes)
            .ok_or_else(ReviewError::resource_limit)
    })
}

#[tauri::command]
pub(crate) fn desktop_recordings_list(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
    project_id: String,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<RecordingPage, ReviewError> {
    let owner = trusted_main_owner(&window, &state)?;
    let limit = validate_limit(limit)?;
    validate_opaque_id(&project_id, "projectId")?;
    let now_instant = Instant::now();
    if let Some(cursor) = cursor {
        if cursor.is_empty() || cursor.chars().count() > 256 {
            return Err(ReviewError::invalid("cursor is invalid."));
        }
        return state
            .review_registry
            .lock()
            .expect("review registry mutex poisoned")
            .cursor_page(owner, &project_id, limit, &cursor, now_instant);
    }

    let projects = genesis_adapter::query(
        &state.genesis,
        "projects",
        &["id"],
        vec![genesis_adapter::eq(
            "projects",
            "id",
            serde_json::json!(&project_id),
        )],
        1,
    )
    .map_err(|_| query_error())?;
    if projects.is_empty() {
        return Err(ReviewError::new(
            "PROJECT_NOT_FOUND",
            "The project was not found.",
            false,
        ));
    }
    let mut rows = genesis_adapter::query_all(
        &state.genesis,
        "recordings",
        &[
            "id",
            "project_id",
            "source",
            "status",
            "duration_ms",
            "created_at",
            "updated_at",
            "language",
        ],
        vec![genesis_adapter::eq(
            "recordings",
            "project_id",
            serde_json::json!(&project_id),
        )],
    )
    .map_err(|_| query_error())?;
    if rows.len() > MAX_SNAPSHOT_ROWS {
        return Err(ReviewError::resource_limit());
    }
    rows.sort_by(|left, right| {
        let left_created = string_field(left, "recordings.created_at")
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value).ok());
        let right_created = string_field(right, "recordings.created_at")
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value).ok());
        right_created.cmp(&left_created).then_with(|| {
            string_field(left, "recordings.id")
                .unwrap_or_default()
                .cmp(&string_field(right, "recordings.id").unwrap_or_default())
        })
    });
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let storage_row =
            parse_storage_row(&row, &project_id, &required_string(&row, "recordings.id")?)?;
        let capture_state = live_capture_state(&state, &storage_row.identity.recording_id);
        items.push(row_to_public_with_state(
            &state.genesis,
            &storage_row,
            &capture_state,
        )?);
    }
    if snapshot_size(&items)? > MAX_SNAPSHOT_BYTES {
        return Err(ReviewError::resource_limit());
    }
    let as_of = now();
    state
        .review_registry
        .lock()
        .expect("review registry mutex poisoned")
        .first_page(owner, project_id, limit, items, as_of, now_instant)
}

#[tauri::command]
pub(crate) fn desktop_recordings_release(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
    snapshot_id: String,
) -> Result<ReleaseReceipt, ReviewError> {
    let owner = trusted_main_owner(&window, &state)?;
    validate_opaque_id(&snapshot_id, "snapshotId")?;
    state
        .review_registry
        .lock()
        .expect("review registry mutex poisoned")
        .release(owner, &snapshot_id, Instant::now())
}

#[tauri::command]
pub(crate) fn desktop_recording_get(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
    project_id: String,
    recording_id: String,
) -> Result<RecordingRow, ReviewError> {
    let _owner = trusted_main_owner(&window, &state)?;
    let row = validate_recording_pair(&state.genesis, &project_id, &recording_id)?;
    let capture_state = live_capture_state(&state, &recording_id);
    row_to_public_with_state(&state.genesis, &row, &capture_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_ids_are_bounded_and_path_free() {
        assert!(validate_opaque_id("legacy-id", "id").is_ok());
        assert_eq!(
            validate_opaque_id("", "id").unwrap_err().code,
            "INVALID_ARGUMENT"
        );
        assert!(validate_opaque_id("a/b", "id").is_err());
        assert!(validate_opaque_id(&"x".repeat(257), "id").is_err());
    }

    #[test]
    fn native_admission_blocks_both_directions_until_release() {
        let guard = NativeCaptureGuard::default();
        assert!(!guard.capture_is_active());
        guard.try_start_capture().unwrap();
        assert!(guard.capture_is_active());
        assert_eq!(guard.capture_phase(), CapturePhase::Starting);
        assert_eq!(
            guard.try_open_playback(),
            Err(AdmissionError::CaptureActive)
        );
        guard.mark_capture_active();
        guard.mark_capture_stopping();
        guard.release_capture();
        assert!(!guard.capture_is_active());
        guard.try_open_playback().unwrap();
        assert!(guard.playback_open());
        assert_eq!(guard.try_start_capture(), Err(AdmissionError::PlaybackBusy));
        guard.release_playback();
        guard.try_start_capture().unwrap();
    }

    #[test]
    fn snapshot_registry_replays_same_page_and_expires_released_cursor() {
        let owner = random_owner_id();
        let now = Instant::now();
        let items = (0..5)
            .map(|index| RecordingRow {
                id: format!("r-{index}"),
                project_id: "p".to_string(),
                source: "import".to_string(),
                status: "complete".to_string(),
                duration_ms: 100,
                created_at: "2026-09-17T00:00:00Z".to_string(),
                updated_at: "2026-09-17T00:00:00Z".to_string(),
                language: None,
                channels: vec!["file".to_string()],
                capture_state: "inactive".to_string(),
            })
            .collect::<Vec<_>>();
        let mut registry = ReviewRegistry::default();
        let first = registry
            .first_page(owner, "p".to_string(), 2, items, "as-of".to_string(), now)
            .unwrap();
        let cursor = first.next_cursor.clone().unwrap();
        let replay = registry.cursor_page(owner, "p", 2, &cursor, now).unwrap();
        assert_eq!(replay.items[0].id, "r-2");
        let replay_again = registry.cursor_page(owner, "p", 2, &cursor, now).unwrap();
        assert_eq!(replay.next_cursor, replay_again.next_cursor);
        registry.release(owner, &first.snapshot_id, now).unwrap();
        assert_eq!(
            registry
                .cursor_page(owner, "p", 2, &cursor, now)
                .unwrap_err()
                .code,
            "CURSOR_EXPIRED"
        );
    }

    #[test]
    fn snapshot_limit_is_rejected_before_storage_is_truncated() {
        assert_eq!(
            validate_limit(Some(0)).unwrap_err().code,
            "INVALID_ARGUMENT"
        );
        assert_eq!(
            validate_limit(Some(101)).unwrap_err().code,
            "INVALID_ARGUMENT"
        );
        assert_eq!(validate_limit(None).unwrap(), DEFAULT_PAGE_LIMIT);
    }

    #[test]
    fn expired_cursor_retention_is_bounded_but_recent_expiry_stays_explicit() {
        let owner = random_owner_id();
        let items = vec![
            RecordingRow {
                id: "r-0".to_string(),
                project_id: "p".to_string(),
                source: "import".to_string(),
                status: "complete".to_string(),
                duration_ms: 100,
                created_at: "2026-09-17T00:00:00Z".to_string(),
                updated_at: "2026-09-17T00:00:00Z".to_string(),
                language: None,
                channels: vec!["file".to_string()],
                capture_state: "inactive".to_string(),
            },
            RecordingRow {
                id: "r-1".to_string(),
                project_id: "p".to_string(),
                source: "import".to_string(),
                status: "complete".to_string(),
                duration_ms: 100,
                created_at: "2026-09-17T00:00:01Z".to_string(),
                updated_at: "2026-09-17T00:00:01Z".to_string(),
                language: None,
                channels: vec!["file".to_string()],
                capture_state: "inactive".to_string(),
            },
        ];
        let now = Instant::now();
        let mut registry = ReviewRegistry::default();
        let mut oldest = None;
        let mut newest = None;

        for index in 0..=MAX_EXPIRED_CURSORS {
            let page = registry
                .first_page(
                    owner,
                    "p".to_string(),
                    1,
                    items.clone(),
                    format!("as-of-{index}"),
                    now,
                )
                .unwrap();
            let cursor = page.next_cursor.clone().unwrap();
            registry.release(owner, &page.snapshot_id, now).unwrap();
            if index == 0 {
                oldest = Some(cursor.clone());
            }
            newest = Some(cursor);
        }

        assert_eq!(registry.expired_cursors.len(), MAX_EXPIRED_CURSORS);
        assert_eq!(registry.expired_cursor_order.len(), MAX_EXPIRED_CURSORS);
        assert_eq!(
            registry
                .cursor_page(owner, "p", 1, &oldest.unwrap(), now)
                .unwrap_err()
                .code,
            "CURSOR_INVALID"
        );
        assert_eq!(
            registry
                .cursor_page(owner, "p", 1, &newest.unwrap(), now)
                .unwrap_err()
                .code,
            "CURSOR_EXPIRED"
        );
    }

    #[test]
    fn expired_snapshot_cursor_is_retained_until_bounded_eviction() {
        let owner = random_owner_id();
        let now = Instant::now();
        let items = vec![RecordingRow {
            id: "r-0".to_string(),
            project_id: "p".to_string(),
            source: "import".to_string(),
            status: "complete".to_string(),
            duration_ms: 100,
            created_at: "2026-09-17T00:00:00Z".to_string(),
            updated_at: "2026-09-17T00:00:00Z".to_string(),
            language: None,
            channels: vec!["file".to_string()],
            capture_state: "inactive".to_string(),
        }];
        let mut registry = ReviewRegistry::default();
        let first = registry
            .first_page(owner, "p".to_string(), 1, items, "as-of".to_string(), now)
            .unwrap();
        let snapshot_id = first.snapshot_id.clone();
        let cursor = random_opaque_id();
        registry.cursors.insert(
            cursor.clone(),
            CursorRecord {
                snapshot_id,
                owner,
                project_id: "p".to_string(),
                limit: 1,
                next_index: 1,
                following_cursor: None,
            },
        );

        let expired_at = now + SNAPSHOT_IDLE_TTL + Duration::from_secs(1);
        assert_eq!(
            registry
                .cursor_page(owner, "p", 1, &cursor, expired_at)
                .unwrap_err()
                .code,
            "CURSOR_EXPIRED"
        );
    }
}
