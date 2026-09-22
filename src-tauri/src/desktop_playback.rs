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
use std::sync::{mpsc, Arc, Mutex};
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
const OUTPUT_CHANNELS: usize = 2;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 96_000;

const STATE_PAUSED: u8 = 0;
const STATE_PLAYING: u8 = 1;
const STATE_ENDED: u8 = 2;
const STATE_ERROR: u8 = 3;

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
    pub(crate) state: String,
    pub(crate) position_ms: u64,
    pub(crate) duration_ms: u64,
    pub(crate) stream_epoch: u64,
    pub(crate) degraded: bool,
    pub(crate) missing_ranges: Vec<MissingRange>,
    pub(crate) output_latency_ms: Option<u64>,
    pub(crate) error: Option<ReviewError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloseReceipt {
    pub(crate) closed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PlaybackManager {
    inner: Arc<Mutex<PlaybackSlot>>,
    lifecycle: Arc<PlaybackLifecycle>,
}

#[derive(Debug)]
struct PlaybackLifecycle {
    disposed: AtomicBool,
    generation: AtomicU64,
}

#[derive(Debug, Clone)]
struct OpenLease {
    generation: u64,
    cancel: Arc<AtomicBool>,
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
    closed: std::collections::HashMap<String, u128>,
}

#[derive(Debug)]
struct PlaybackSession {
    owner: u128,
    handle: String,
    project_id: String,
    recording_id: String,
    channel: String,
    duration_ms: u64,
    sample_rate: u32,
    runtime: Arc<PlaybackRuntime>,
    command_tx: mpsc::Sender<WorkerCommand>,
    join: Option<JoinHandle<()>>,
    guard: Arc<NativeCaptureGuard>,
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
            }),
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
        if slot.opening.is_some() || slot.active.is_some() {
            return Err(playback_error(
                "PLAYBACK_BUSY",
                "Another playback session is open.",
                false,
            ));
        }
        let generation = self.lifecycle.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        slot.opening = Some(OpeningPlayback {
            generation,
            cancel: Arc::clone(&cancel),
        });
        Ok(OpenLease { generation, cancel })
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
        slot.opening = None;
        slot.active = Some(session);
        Ok(())
    }

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
            .is_some_and(|issued_owner| *issued_owner == owner)
        {
            return Ok(None);
        }
        Err(playback_error(
            "PLAYBACK_HANDLE_INVALID",
            "The playback handle is invalid.",
            false,
        ))
    }

    fn mark_closed(&self, handle: String, owner: u128) {
        let mut slot = self.inner.lock().expect("playback mutex poisoned");
        if slot.closed.len() >= 16 {
            if let Some(oldest) = slot.closed.keys().next().cloned() {
                slot.closed.remove(&oldest);
            }
        }
        slot.closed.insert(handle, owner);
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

    #[allow(clippy::too_many_arguments)]
    fn control(
        &self,
        owner: u128,
        handle: &str,
        expected_epoch: u64,
        action: &str,
        position_ms: Option<u64>,
    ) -> Result<PlaybackSnapshot, ReviewError> {
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
                session
                    .runtime
                    .position_frames
                    .store(frame, Ordering::Release);
                session.runtime.clear_queue();
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
                session.runtime.stream_epoch.store(epoch, Ordering::Release);
                if frame >= frames_for_ms(session.duration_ms, session.sample_rate) {
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

    pub(crate) fn shutdown(&self) {
        let session = {
            let mut slot = self.inner.lock().expect("playback mutex poisoned");
            self.lifecycle.disposed.store(true, Ordering::SeqCst);
            self.lifecycle.generation.fetch_add(1, Ordering::SeqCst);
            if let Some(opening) = slot.opening.take() {
                opening.cancel.store(true, Ordering::SeqCst);
            }
            slot.active.take()
        };
        if let Some(session) = session {
            if let Err(error) = spawn_session_cleanup(self.clone(), session) {
                eprintln!(
                    "playback shutdown cleanup could not be scheduled: {}",
                    error.code
                );
            }
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

#[derive(Debug)]
struct PlaybackRuntime {
    state: AtomicU8,
    position_frames: AtomicU64,
    stream_epoch: AtomicU64,
    queue: Mutex<VecDeque<i16>>,
    error: Mutex<Option<ReviewError>>,
    degraded: bool,
    missing_ranges: Vec<MissingRange>,
    started_at: Instant,
    underrun_started_ms: AtomicU64,
    device_lost: AtomicBool,
}

impl PlaybackRuntime {
    fn new(degraded: bool, missing_ranges: Vec<MissingRange>) -> Self {
        Self {
            state: AtomicU8::new(STATE_PAUSED),
            position_frames: AtomicU64::new(0),
            stream_epoch: AtomicU64::new(0),
            queue: Mutex::new(VecDeque::with_capacity(MAX_QUEUE_BYTES / 2)),
            error: Mutex::new(None),
            degraded,
            missing_ranges,
            started_at: Instant::now(),
            underrun_started_ms: AtomicU64::new(0),
            device_lost: AtomicBool::new(false),
        }
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

    fn position_ms(&self, sample_rate: u32, duration_ms: u64) -> u64 {
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
            state: state.to_string(),
            position_ms: self.position_ms(sample_rate, duration_ms),
            duration_ms,
            stream_epoch: self.stream_epoch.load(Ordering::Acquire),
            degraded: self.degraded,
            missing_ranges: self.missing_ranges.clone(),
            output_latency_ms: None,
            error: self.error.lock().ok().and_then(|error| error.clone()),
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
    Close,
}

struct OpenSource {
    reader: AudioReader,
    channels: u16,
    sample_rate: u32,
}

fn playback_error(code: &str, message: &str, retryable: bool) -> ReviewError {
    ReviewError::new(code, message, retryable)
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

fn check_open_cancelled(cancel: &AtomicBool) -> Result<(), ReviewError> {
    if cancel.load(Ordering::Acquire) {
        Err(open_timeout_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn recv_until_deadline<T>(
    receiver: &mpsc::Receiver<T>,
    deadline: Instant,
) -> Result<T, OpenWaitError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(OpenWaitError::Timeout)?;
    match receiver.recv_timeout(remaining) {
        Ok(value) if Instant::now() < deadline => Ok(value),
        Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => Err(OpenWaitError::Timeout),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(OpenWaitError::Disconnected),
    }
}

fn recv_until_deadline_or_cancel<T>(
    receiver: &mpsc::Receiver<T>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<T, OpenWaitError> {
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(OpenWaitError::Cancelled);
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(OpenWaitError::Timeout)?;
        let wait = remaining.min(Duration::from_millis(20));
        match receiver.recv_timeout(wait) {
            Ok(value) if !cancel.load(Ordering::Acquire) && Instant::now() < deadline => {
                return Ok(value)
            }
            Ok(_) => {
                if cancel.load(Ordering::Acquire) {
                    return Err(OpenWaitError::Cancelled);
                }
                return Err(OpenWaitError::Timeout);
            }
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() >= deadline => {
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

#[cfg(test)]
fn ms_for_frames(frames: u64, sample_rate: u32) -> u64 {
    (u128::from(frames) * 1_000 / u128::from(sample_rate.max(1))) as u64
}

fn within_one_frame(left: u64, right: u64) -> bool {
    left.abs_diff(right) <= 1
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

fn normalized_path_inside(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    fn comparable(path: &Path) -> PathBuf {
        let value = path.to_string_lossy();
        value
            .strip_prefix(r"\\?\")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf())
    }

    #[cfg(not(windows))]
    fn comparable(path: &Path) -> PathBuf {
        path.to_path_buf()
    }

    comparable(path).starts_with(comparable(root))
}

#[cfg(windows)]
fn final_path_from_handle(file: &File, _candidate: &Path) -> Result<PathBuf, ReviewError> {
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
fn final_path_from_handle(file: &File, candidate: &Path) -> Result<PathBuf, ReviewError> {
    let _ = file;
    std::fs::canonicalize(candidate).map_err(|_| {
        playback_error(
            "PLAYBACK_PATH_DENIED",
            "The playback source path is not permitted.",
            false,
        )
    })
}

fn resolve_ledger_path(project_root: &Path, raw: &str) -> Result<PathBuf, ReviewError> {
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
        || !within_one_frame(
            frames,
            frames_for_ms(descriptor.end_ms - descriptor.start_ms, spec.sample_rate),
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
    }))
}

fn project_custody_root(data_root: &Path, storage_path: &str) -> Result<PathBuf, ReviewError> {
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
            let expected_frames = frames_for_ms(descriptor.end_ms - descriptor.start_ms, rate);
            if !within_one_frame(wave.frames, expected_frames) {
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
    })
}

fn exact_output_config(
    device: &cpal::Device,
    sample_rate: u32,
) -> Result<(StreamConfig, SampleFormat), ReviewError> {
    let configs = device.supported_output_configs().map_err(|_| {
        playback_error(
            "PLAYBACK_OUTPUT_UNSUPPORTED",
            "The output device cannot play this recording rate.",
            false,
        )
    })?;
    for config in configs {
        if config.channels() == OUTPUT_CHANNELS as u16
            && matches!(
                config.sample_format(),
                SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
            )
            && config.min_sample_rate().0 <= sample_rate
            && config.max_sample_rate().0 >= sample_rate
        {
            let format = config.sample_format();
            return Ok((
                config
                    .with_sample_rate(cpal::SampleRate(sample_rate))
                    .config(),
                format,
            ));
        }
    }
    Err(playback_error(
        "PLAYBACK_OUTPUT_UNSUPPORTED",
        "The output device cannot play this recording rate.",
        false,
    ))
}

fn set_silence<T: SizedSample + FromSample<i16>>(data: &mut [T]) {
    for sample in data {
        *sample = T::from_sample(0_i16);
    }
}

fn build_output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    runtime: Arc<PlaybackRuntime>,
) -> Result<Stream, ReviewError>
where
    T: SizedSample + FromSample<i16> + Send + 'static,
{
    let stream_runtime = Arc::clone(&runtime);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                if stream_runtime.state.load(Ordering::Acquire) != STATE_PLAYING {
                    set_silence(data);
                    return;
                }
                let mut queue = match stream_runtime.queue.try_lock() {
                    Ok(queue) => queue,
                    Err(_) => {
                        set_silence(data);
                        return;
                    }
                };
                let mut missing = false;
                for frame in data.chunks_mut(OUTPUT_CHANNELS) {
                    let left = queue.pop_front();
                    let right = queue.pop_front();
                    if left.is_none() || right.is_none() {
                        missing = true;
                    }
                    frame[0] = T::from_sample(left.unwrap_or(0));
                    if frame.len() > 1 {
                        frame[1] = T::from_sample(right.unwrap_or(0));
                    }
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
                stream_runtime
                    .position_frames
                    .fetch_add((data.len() / OUTPUT_CHANNELS) as u64, Ordering::AcqRel);
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
    let source = open_source(&prepared.project_root, descriptor)?;
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
    let mut frames_written = 0usize;
    let max_frames = MAX_READ_BYTES / (OUTPUT_CHANNELS * std::mem::size_of::<i16>());
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
                        reader
                            .reader
                            .seek(local_frame as u32)
                            .map_err(|_| safe_storage_error())?;
                    }
                }
            }
        }
        let frame = if let Some(reader) = source.as_mut() {
            read_frame(&mut reader.reader, reader.channels)?
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

fn worker_loop(
    prepared: PreparedPlayback,
    runtime: Arc<PlaybackRuntime>,
    cancel: Arc<AtomicBool>,
    command_rx: mpsc::Receiver<WorkerCommand>,
    ready_tx: mpsc::Sender<Result<(), ReviewError>>,
) {
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
    let (config, sample_format) = match exact_output_config(&device, prepared.sample_rate) {
        Ok(config) => config,
        Err(error) => {
            publish_ready(&ready_tx, Err(error), &cancel);
            return;
        }
    };
    if cancel.load(Ordering::Acquire) {
        return;
    }
    let stream = match sample_format {
        SampleFormat::F32 => build_output_stream::<f32>(&device, &config, Arc::clone(&runtime)),
        SampleFormat::I16 => build_output_stream::<i16>(&device, &config, Arc::clone(&runtime)),
        SampleFormat::U16 => build_output_stream::<u16>(&device, &config, Arc::clone(&runtime)),
        _ => Err(playback_error(
            "PLAYBACK_OUTPUT_UNSUPPORTED",
            "The output device cannot play this recording rate.",
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
        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(WorkerCommand::Close) => break,
            Ok(WorkerCommand::Play) => {
                if runtime.state.load(Ordering::Acquire) == STATE_ENDED
                    || runtime.position_frames.load(Ordering::Acquire) >= prepared.duration_frames
                {
                    cursor = 0;
                    runtime.position_frames.store(0, Ordering::Release);
                    runtime.stream_epoch.fetch_add(1, Ordering::AcqRel);
                    runtime.clear_queue();
                    source = None;
                    source_index = None;
                }
                runtime.state.store(STATE_PLAYING, Ordering::Release);
            }
            Ok(WorkerCommand::Pause) => {
                runtime.state.store(STATE_PAUSED, Ordering::Release);
                runtime.clear_queue();
                source = None;
                source_index = None;
            }
            Ok(WorkerCommand::Seek { frame, epoch }) => {
                cursor = frame.min(prepared.duration_frames);
                runtime.stream_epoch.store(epoch, Ordering::Release);
                runtime.position_frames.store(cursor, Ordering::Release);
                runtime.clear_queue();
                source = None;
                source_index = None;
                if cursor >= prepared.duration_frames {
                    runtime.state.store(STATE_ENDED, Ordering::Release);
                }
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
    runtime.state.store(STATE_PAUSED, Ordering::Release);
    drop(stream);
}

struct WorkerLaunch {
    command_tx: mpsc::Sender<WorkerCommand>,
    ready_rx: mpsc::Receiver<Result<(), ReviewError>>,
    join: JoinHandle<()>,
}

type PrepareFn =
    Box<dyn FnOnce(Arc<AtomicBool>) -> Result<PreparedPlayback, ReviewError> + Send + 'static>;
type WorkerFn = Arc<
    dyn Fn(
            PreparedPlayback,
            Arc<PlaybackRuntime>,
            Arc<AtomicBool>,
        ) -> Result<WorkerLaunch, ReviewError>
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
}

struct OpenPlaybackTask {
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    lease: OpenLease,
    io: OpenPlaybackIo,
    open_timeout: Duration,
    owner: u128,
}

fn cleanup_session(mut session: PlaybackSession) {
    let _ = session.command_tx.send(WorkerCommand::Close);
    if let Some(join) = session.join.take() {
        let _ = join.join();
    }
    session.guard.release_playback();
}

fn spawn_session_cleanup(
    playback: PlaybackManager,
    session: PlaybackSession,
) -> Result<JoinHandle<()>, ReviewError> {
    let handle = session.handle.clone();
    let owner = session.owner;
    thread::Builder::new()
        .name("fung-playback-cleanup".to_string())
        .spawn(move || {
            cleanup_session(session);
            playback.mark_closed(handle, owner);
        })
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback cleanup owner is unavailable.",
                true,
            )
        })
}

fn spawn_worker(
    prepared: PreparedPlayback,
    runtime: Arc<PlaybackRuntime>,
    cancel: Arc<AtomicBool>,
) -> Result<WorkerLaunch, ReviewError> {
    let (command_tx, command_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let join = thread::Builder::new()
        .name("fung-playback-owner".to_string())
        .spawn(move || worker_loop(prepared, runtime, cancel, command_rx, ready_tx))
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback owner is unavailable.",
                true,
            )
        })?;
    Ok(WorkerLaunch {
        command_tx,
        ready_rx,
        join,
    })
}

fn spawn_open_reaper(
    playback: PlaybackManager,
    guard: Arc<NativeCaptureGuard>,
    cancel: Arc<AtomicBool>,
    generation: u64,
    prepare_join: Option<JoinHandle<()>>,
    worker: Option<WorkerLaunch>,
) -> Result<JoinHandle<()>, ReviewError> {
    cancel.store(true, Ordering::Release);
    thread::Builder::new()
        .name("fung-playback-open-reaper".to_string())
        .spawn(move || {
            if let Some(worker) = worker {
                let WorkerLaunch {
                    command_tx, join, ..
                } = worker;
                let _ = command_tx.send(WorkerCommand::Close);
                let _ = join.join();
            }
            if let Some(join) = prepare_join {
                let _ = join.join();
            }
            guard.release_playback();
            playback.abort_open(generation);
        })
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback cleanup owner is unavailable.",
                true,
            )
        })
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

fn open_playback_owned(task: OpenPlaybackTask) -> Result<PlaybackState, ReviewError> {
    let OpenPlaybackTask {
        playback,
        guard,
        lease,
        io,
        open_timeout,
        owner,
    } = task;
    let OpenPlaybackIo { prepare, worker } = io;
    let deadline = Instant::now() + open_timeout;
    let cancel = Arc::clone(&lease.cancel);
    let (prepared_tx, prepared_rx) = mpsc::channel();
    let prepare_cancel = Arc::clone(&cancel);
    let prepare_join = thread::Builder::new()
        .name("fung-playback-prepare".to_string())
        .spawn(move || {
            let _ = prepared_tx.send(prepare(prepare_cancel));
        })
        .map_err(|_| {
            guard.release_playback();
            playback.abort_open(lease.generation);
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback owner is unavailable.",
                true,
            )
        })?;

    let prepared = match recv_until_deadline_or_cancel(&prepared_rx, deadline, &cancel) {
        Ok(result) => {
            if prepare_join.join().is_err() {
                guard.release_playback();
                playback.abort_open(lease.generation);
                return Err(playback_error(
                    "NATIVE_UNAVAILABLE",
                    "The native playback preparation owner is unavailable.",
                    true,
                ));
            }
            match result {
                Ok(prepared) => prepared,
                Err(error) => {
                    guard.release_playback();
                    playback.abort_open(lease.generation);
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
            if let Err(cleanup_error) = spawn_open_reaper(
                playback.clone(),
                Arc::clone(&guard),
                Arc::clone(&cancel),
                lease.generation,
                Some(prepare_join),
                None,
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
        guard.release_playback();
        playback.abort_open(lease.generation);
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

    let runtime = Arc::new(PlaybackRuntime::new(
        prepared.degraded,
        prepared.missing_ranges.clone(),
    ));
    let worker = match worker(prepared.clone(), Arc::clone(&runtime), Arc::clone(&cancel)) {
        Ok(worker) => worker,
        Err(error) => {
            guard.release_playback();
            playback.abort_open(lease.generation);
            return Err(error);
        }
    };

    match recv_until_deadline_or_cancel(&worker.ready_rx, deadline, &cancel) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            if let Err(cleanup_error) = spawn_open_reaper(
                playback.clone(),
                Arc::clone(&guard),
                Arc::clone(&cancel),
                lease.generation,
                None,
                Some(worker),
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
            if let Err(cleanup_error) = spawn_open_reaper(
                playback.clone(),
                Arc::clone(&guard),
                Arc::clone(&cancel),
                lease.generation,
                None,
                Some(worker),
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
        if let Err(cleanup_error) = spawn_open_reaper(
            playback.clone(),
            Arc::clone(&guard),
            Arc::clone(&cancel),
            lease.generation,
            None,
            Some(worker),
        ) {
            eprintln!(
                "playback open cleanup could not be scheduled: {}",
                cleanup_error.code
            );
        }
        return Err(error);
    }

    let WorkerLaunch {
        command_tx, join, ..
    } = worker;
    let handle = recording_review::random_opaque_id();
    let session = PlaybackSession {
        owner,
        handle: handle.clone(),
        project_id: prepared.project_id.clone(),
        recording_id: prepared.recording_id.clone(),
        channel: prepared.channel.clone(),
        duration_ms: prepared.duration_ms,
        sample_rate: prepared.sample_rate,
        runtime: Arc::clone(&runtime),
        command_tx,
        join: Some(join),
        guard: Arc::clone(&guard),
    };
    if let Err(failure) = playback.install(lease.generation, session) {
        let PlaybackInstallFailure { error, session } = *failure;
        if let Err(cleanup_error) = spawn_session_cleanup(playback.clone(), session) {
            eprintln!(
                "late playback install cleanup could not be scheduled: {}",
                cleanup_error.code
            );
        }
        return Err(error);
    }
    let snapshot = playback.active_snapshot(owner, &handle)?;
    Ok(public_state(&snapshot))
}

async fn dispatch_open_playback(task: OpenPlaybackTask) -> Result<PlaybackState, ReviewError> {
    tauri::async_runtime::spawn_blocking(move || open_playback_owned(task))
        .await
        .map_err(|_| {
            playback_error(
                "NATIVE_UNAVAILABLE",
                "The native playback owner is unavailable.",
                true,
            )
        })?
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
    let lease = state.playback.begin_open()?;
    if let Err(error) = state.native_capture.try_open_playback() {
        state.playback.abort_open(lease.generation);
        return Err(map_admission_error(error));
    }
    let playback = state.playback.clone();
    let guard = Arc::clone(&state.native_capture);
    let io = OpenPlaybackIo::production(
        Arc::clone(&state.genesis),
        state.data_root.clone(),
        row,
        channel,
    );
    dispatch_open_playback(OpenPlaybackTask {
        playback,
        guard,
        lease,
        io,
        open_timeout: MAX_OPEN_TIMEOUT,
        owner,
    })
    .await
}

#[tauri::command]
pub(crate) fn desktop_playback_control(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
    expected_epoch: u64,
    action: String,
    position_ms: Option<u64>,
) -> Result<PlaybackState, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
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
) -> Result<PlaybackState, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    let snapshot = state.playback.active_snapshot(owner, &handle)?;
    Ok(public_state(&snapshot))
}

#[tauri::command]
pub(crate) async fn desktop_playback_close(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
) -> Result<CloseReceipt, ReviewError> {
    let owner = recording_review::trusted_main_owner(&window, &state)?;
    recording_review::validate_opaque_id(&handle, "handle")?;
    let Some(session) = state.playback.take_for_close(owner, &handle)? else {
        return Ok(CloseReceipt { closed: true });
    };
    let playback = state.playback.clone();
    dispatch_close_playback(playback, session).await?;
    Ok(CloseReceipt { closed: true })
}

async fn dispatch_close_playback(
    playback: PlaybackManager,
    session: PlaybackSession,
) -> Result<(), ReviewError> {
    let session_handle = session.handle.clone();
    let session_owner = session.owner;
    tauri::async_runtime::spawn_blocking(move || {
        cleanup_session(session);
        playback.mark_closed(session_handle, session_owner);
    })
    .await
    .map_err(|_| {
        playback_error(
            "NATIVE_UNAVAILABLE",
            "The native playback cleanup owner is unavailable.",
            true,
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session(
        owner: u128,
        handle: &str,
        guard: Arc<NativeCaptureGuard>,
        join: Option<JoinHandle<()>>,
    ) -> PlaybackSession {
        let (command_tx, _command_rx) = mpsc::channel();
        PlaybackSession {
            owner,
            handle: handle.to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            sample_rate: 8_000,
            runtime: Arc::new(PlaybackRuntime::new(false, Vec::new())),
            command_tx,
            join,
            guard,
        }
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
    fn frame_math_aligns_seek_down_without_resampling() {
        assert_eq!(frames_for_ms(999, 48_000), 47_952);
        assert_eq!(ms_for_frames(frames_for_ms(999, 48_000), 48_000), 999);
        assert!(within_one_frame(48_000, 48_001));
        assert!(!within_one_frame(48_000, 48_002));
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
        Arc::new(move |_prepared, _runtime, _cancel| {
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
            let join = thread::spawn(move || {
                if matches!(command_rx.recv(), Ok(WorkerCommand::Close)) {
                    close_seen_tx.send(()).expect("close-seen receiver dropped");
                    while !allow_join.load(Ordering::Acquire) {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            });
            Ok(WorkerLaunch {
                command_tx,
                ready_rx,
                join,
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
        guard.try_open_playback().unwrap();
        OpenPlaybackTask {
            playback,
            guard,
            lease,
            io,
            open_timeout,
            owner: 17,
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
    fn concurrent_seek_controls_serialize_epoch_and_enqueue() {
        let manager = PlaybackManager::new();
        let runtime = Arc::new(PlaybackRuntime::new(false, Vec::new()));
        let (command_tx, command_rx) = mpsc::channel();
        manager.install_test(PlaybackSession {
            owner: 7,
            handle: "playback".to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            sample_rate: 8_000,
            runtime: Arc::clone(&runtime),
            command_tx,
            join: None,
            guard: Arc::new(NativeCaptureGuard::default()),
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
            owner: 9,
            handle: "close-race".to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            sample_rate: 8_000,
            runtime: Arc::new(PlaybackRuntime::new(false, Vec::new())),
            command_tx,
            join: None,
            guard: Arc::new(NativeCaptureGuard::default()),
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
        manager.mark_closed(session.handle, session.owner);
        assert!(manager.take_for_close(9, "close-race").unwrap().is_none());
    }

    #[test]
    fn one_open_deadline_expires_delayed_readiness_without_a_second_window() {
        let deadline = Instant::now() + Duration::from_millis(100);
        let (prepared_tx, prepared_rx) = mpsc::channel();
        let prepare_join = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let _ = prepared_tx.send(());
        });
        assert_eq!(recv_until_deadline(&prepared_rx, deadline), Ok(()));

        let (ready_release_tx, ready_release_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let ready_join = thread::spawn(move || {
            let _ = ready_release_rx.recv();
            let _ = ready_tx.send(());
        });
        assert_eq!(
            recv_until_deadline(&ready_rx, deadline),
            Err(OpenWaitError::Timeout)
        );
        let _ = ready_release_tx.send(());
        prepare_join.join().unwrap();
        ready_join.join().unwrap();
    }

    #[test]
    fn open_reaper_retains_admission_until_delayed_prepare_quiesces() {
        let manager = PlaybackManager::new();
        let lease = manager.begin_open().unwrap();
        let guard = Arc::new(NativeCaptureGuard::default());
        guard.try_open_playback().unwrap();
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
            cancel,
            lease.generation,
            Some(prepare_join),
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
        let (install_checked_tx, install_checked_rx) = mpsc::channel();
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
            install_checked_tx.send(()).unwrap();
        });

        manager.shutdown();
        assert!(manager.begin_open().is_err());
        prepare_release_tx.send(()).unwrap();
        install_checked_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("late install check completed");
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
            owner: 23,
            handle: "delayed-close".to_string(),
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            channel: "file".to_string(),
            duration_ms: 1_000,
            sample_rate: 8_000,
            runtime: Arc::new(PlaybackRuntime::new(false, Vec::new())),
            command_tx,
            join: Some(worker),
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
