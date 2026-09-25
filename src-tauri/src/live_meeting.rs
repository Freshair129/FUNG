//! Desktop Live Meeting capture + live transcription (Meeting Mode MVP).
//!
//! @req FR-102, FR-103, FR-104, FR-114, NFR-101, NFR-104, NFR-109
//!
//! Design rules, in priority order:
//! 1. **Persist before enrich** — every audio chunk becomes a durable WAV file
//!    plus a Genesis ledger row (`audio_chunks` via the same
//!    `genesis_adapter::append_capture_chunk` path mobile capture uses) BEFORE
//!    any transcription or AI sees it.
//! 2. **Live intelligence is optional** — if the whisper worker dies or the
//!    GPU is unavailable, capture keeps running and the session degrades to
//!    "transcribe after the meeting"; it must never take recording down.
//! 3. **Channel = capture provenance, not identity** — `mic` maps to the
//!    project speaker `me` (เรา) and WASAPI loopback `system` maps to `them`
//!    (อีกฝ่าย). These are editable speaker labels, not verified identities.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager, State};
use uuid::Uuid;

use crate::{genesis_adapter, meeting_intel, now, AppError, AppResult, AppState, WhisperRuntime};

const CHUNK_MS: u64 = 8_000;

/// Capture writes uncompressed 16-bit mono WAV per channel. At 48 kHz with a
/// microphone plus system loopback that is roughly 690 MB/hour, so a long
/// meeting is measured in gigabytes and an unguarded session can fill the
/// volume. Nothing detected that before: a full disk surfaced as a chunk
/// write failure with no explanation, and previously not even that.
///
/// Refuse to start below this.
const MIN_FREE_BYTES_TO_START: u64 = 2 * 1024 * 1024 * 1024;
/// Warn, once, below this while recording.
const LOW_DISK_WARN_BYTES: u64 = 1024 * 1024 * 1024;
/// Stop the session below this, while there is still room to close every
/// chunk cleanly. Stopping with the audio intact beats writing until the
/// volume is full.
const MIN_FREE_BYTES_TO_CONTINUE: u64 = 256 * 1024 * 1024;
/// Chunks between free-space checks. Two channels at 8s chunks produce ~15
/// per minute, so this is about a one-minute cadence.
const DISK_CHECK_EVERY_CHUNKS: usize = 16;
const MIN_FINAL_CHUNK_MS: u64 = 400;
const CAPTURE_SAMPLE_QUEUE_CAPACITY: usize = 64;
const CAPTURE_EVENT_QUEUE_CAPACITY: usize = 32;
const RECENT_SEGMENT_CAP: usize = 240;
/// Model load can include a first-time download; give it room.
const WORKER_READY_TIMEOUT: Duration = Duration::from_secs(180);
const WORKER_CHUNK_TIMEOUT: Duration = Duration::from_secs(120);
const CAPTURE_PREFERENCES_FILE: &str = "live-capture-preferences.json";

pub(crate) const CHANNEL_MIC: &str = "mic";
pub(crate) const CHANNEL_SYSTEM: &str = "system";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveCaptureDevice {
    id: String,
    name: String,
    is_default: bool,
    available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveCaptureDevices {
    inputs: Vec<LiveCaptureDevice>,
    loopback_outputs: Vec<LiveCaptureDevice>,
    selected_mic_device_id: Option<String>,
    selected_system_device_id: Option<String>,
    issue: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptureDevicePreferences {
    mic_device_id: Option<String>,
    system_device_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureDeviceKind {
    Mic,
    SystemLoopback,
}

impl CaptureDeviceKind {
    fn key(self) -> &'static str {
        match self {
            Self::Mic => "mic",
            Self::SystemLoopback => "system",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mic => "ไมโครโฟน",
            Self::SystemLoopback => "เสียงระบบ",
        }
    }

    fn missing_message(self) -> &'static str {
        match self {
            Self::Mic => "ไม่พบไมโครโฟนที่พร้อมใช้งาน",
            Self::SystemLoopback => "ไม่พบอุปกรณ์เสียงออกที่พร้อมใช้สำหรับ loopback",
        }
    }
}

struct ResolvedCaptureDevice {
    descriptor: LiveCaptureDevice,
    device: cpal::Device,
}

fn capture_preferences_path(data_root: &Path) -> PathBuf {
    data_root.join(CAPTURE_PREFERENCES_FILE)
}

fn read_capture_preferences(data_root: &Path) -> Result<CaptureDevicePreferences, String> {
    let path = capture_preferences_path(data_root);
    if !path.is_file() {
        return Ok(CaptureDevicePreferences::default());
    }
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("อ่านการตั้งค่าอุปกรณ์บันทึกไม่ได้: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("การตั้งค่าอุปกรณ์บันทึกไม่ถูกต้อง: {error}"))
}

fn write_capture_preferences(
    data_root: &Path,
    preferences: &CaptureDevicePreferences,
) -> Result<(), String> {
    std::fs::create_dir_all(data_root)
        .map_err(|error| format!("เตรียมพื้นที่เก็บการตั้งค่าอุปกรณ์ไม่ได้: {error}"))?;
    let encoded = serde_json::to_vec_pretty(preferences)
        .map_err(|error| format!("สร้างการตั้งค่าอุปกรณ์ไม่ได้: {error}"))?;
    std::fs::write(capture_preferences_path(data_root), encoded)
        .map_err(|error| format!("บันทึกการตั้งค่าอุปกรณ์ไม่ได้: {error}"))
}

fn normalize_device_id(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn capture_device_id(kind: CaptureDeviceKind, name: &str, ordinal: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.key().as_bytes());
    hasher.update([0]);
    hasher.update(name.as_bytes());
    hasher.update([0]);
    hasher.update(ordinal.to_le_bytes());
    format!("{}-{:x}", kind.key(), hasher.finalize())
}

fn enumerate_capture_devices(
    kind: CaptureDeviceKind,
) -> Result<Vec<ResolvedCaptureDevice>, String> {
    let host = cpal::default_host();
    let default_name = match kind {
        CaptureDeviceKind::Mic => host.default_input_device(),
        CaptureDeviceKind::SystemLoopback => host.default_output_device(),
    }
    .and_then(|device| device.name().ok());
    let devices = match kind {
        CaptureDeviceKind::Mic => host
            .input_devices()
            .map_err(|error| format!("อ่านรายการไมโครโฟนไม่ได้: {error}"))?,
        CaptureDeviceKind::SystemLoopback => host
            .output_devices()
            .map_err(|error| format!("อ่านรายการอุปกรณ์เสียงออกไม่ได้: {error}"))?,
    };

    let mut occurrences = std::collections::HashMap::<String, usize>::new();
    let mut resolved = Vec::new();
    for device in devices {
        let name = device
            .name()
            .unwrap_or_else(|_| "อุปกรณ์เสียงไม่ทราบชื่อ".to_string());
        let ordinal = occurrences.entry(name.clone()).or_insert(0);
        let id = capture_device_id(kind, &name, *ordinal);
        *ordinal += 1;
        resolved.push(ResolvedCaptureDevice {
            descriptor: LiveCaptureDevice {
                id,
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
                available: true,
            },
            device,
        });
    }
    Ok(resolved)
}

fn resolve_capture_device(
    kind: CaptureDeviceKind,
    requested_id: Option<&str>,
) -> Result<cpal::Device, String> {
    if let Some(requested_id) = requested_id {
        return enumerate_capture_devices(kind)?
            .into_iter()
            .find(|candidate| candidate.descriptor.id == requested_id)
            .map(|candidate| candidate.device)
            .ok_or_else(|| {
                format!(
                    "อุปกรณ์{}ที่เลือกไม่พร้อมใช้งาน — รีเฟรชรายการแล้วเลือกใหม่",
                    kind.label()
                )
            });
    }

    match kind {
        CaptureDeviceKind::Mic => cpal::default_host().default_input_device(),
        CaptureDeviceKind::SystemLoopback => cpal::default_host().default_output_device(),
    }
    .ok_or_else(|| kind.missing_message().to_string())
}

fn validate_requested_device(
    kind: CaptureDeviceKind,
    requested_id: Option<&str>,
) -> Result<(), String> {
    if requested_id.is_some() {
        resolve_capture_device(kind, requested_id).map(|_| ())
    } else {
        Ok(())
    }
}

/// Rolling in-memory window of the newest live segments. Shared with the
/// topic tracker and `meeting_ask` so they never need a mid-session DB read.
pub(crate) type SharedRecent = Arc<Mutex<std::collections::VecDeque<RecentSegment>>>;
pub(crate) type LiveState = Arc<Mutex<Option<LiveSessionControl>>>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentSegment {
    pub(crate) speaker: String,
    pub(crate) channel: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
}

pub(crate) struct LiveSessionControl {
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    /// Carried so the session can be tied back to its job row by anything
    /// inspecting live state; the coordinator owns the job it writes to.
    #[allow(dead_code)]
    pub(crate) job_id: String,
    pub(crate) recent: SharedRecent,
    pub(crate) started_at: Instant,
    pub(crate) coordinator: Option<JoinHandle<()>>,
}

/// Keeps native capture ownership until the coordinator has finished closing
/// the source streams, ledger capture and catch-up work.  A failed coordinator
/// path also clears the visible live slot, so a failed start cannot strand the
/// admission guard in an active state.
struct CaptureRuntimeLease {
    app: tauri::AppHandle,
    guard: Arc<crate::recording_review::NativeCaptureGuard>,
    recording_id: String,
}

impl Drop for CaptureRuntimeLease {
    fn drop(&mut self) {
        self.guard.release_capture();
        if let Some(state) = self.app.try_state::<AppState>() {
            let mut live = state.live.lock().expect("live session mutex poisoned");
            if live
                .as_ref()
                .is_some_and(|session| session.recording_id == self.recording_id)
            {
                *live = None;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveStatusEvent {
    recording_id: String,
    state: String,
    detail: Option<String>,
    mic_device: Option<String>,
    system_device: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveSegmentEvent {
    recording_id: String,
    segment_id: String,
    channel: String,
    speaker: String,
    start_ms: i64,
    end_ms: i64,
    text: String,
    confidence: Option<f64>,
}

fn emit_status(
    app: &tauri::AppHandle,
    recording_id: &str,
    state: &str,
    detail: Option<String>,
    mic_device: Option<String>,
    system_device: Option<String>,
) {
    let _ = app.emit(
        "live-status",
        LiveStatusEvent {
            recording_id: recording_id.to_string(),
            state: state.to_string(),
            detail,
            mic_device,
            system_device,
        },
    );
}

// ---------------------------------------------------------------------------
// Audio capture (one thread per channel; the thread owns the cpal stream)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ChannelKind {
    Mic,
    SystemLoopback,
}

#[derive(Clone)]
pub(crate) struct RawChunk {
    pub(crate) channel: &'static str,
    pub(crate) chunk_id: String,
    pub(crate) file_path: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) byte_size: i64,
    pub(crate) checksum: String,
}

struct CaptureSampleBatch {
    first_sample: u64,
    samples: Vec<i16>,
}

/// What a capture thread reports to the coordinator.
///
/// Chunk writes and the OS audio stream can both fail mid-session, and both
/// used to be swallowed by `eprintln!` — a recording could lose audio, or go
/// silent because the microphone was unplugged, while the UI still showed a
/// healthy session. Faults travel on the same channel as chunks so the
/// coordinator sees them in the order they happened, and so a capture thread
/// still needs no access to Genesis or the app handle.
pub(crate) enum CaptureEvent {
    Chunk(RawChunk),
    /// Audio was captured but could not be committed to disk. This is lost
    /// source audio: the samples are already gone from the accumulator.
    ChunkWriteFailed {
        channel: &'static str,
        start_ms: i64,
        end_ms: i64,
        error: String,
    },
    /// Non-blocking audio callback input was shed because the bounded sample
    /// queue filled. The exact missing media interval is retained separately.
    SourceGap {
        channel: &'static str,
        start_ms: i64,
        end_ms: i64,
        reason: &'static str,
    },
    /// The OS reported an error on the audio stream — device removed, format
    /// change, driver reset. Capture may continue but is no longer trustworthy.
    StreamFailed {
        channel: &'static str,
        error: String,
    },
}

fn sample_f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn sample_i16_to_i16(sample: i16) -> i16 {
    sample
}

fn sample_u16_to_i16(sample: u16) -> i16 {
    (sample as i32 - 32_768) as i16
}

/// Downmixes interleaved frames to mono i16 and forwards them off the
/// realtime callback. The per-callback Vec allocation is deliberate: it keeps
/// the callback free of locks shared with slow consumers.
#[allow(clippy::too_many_arguments)]
fn build_stream<T: cpal::SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: mpsc::SyncSender<CaptureSampleBatch>,
    sample_cursor: Arc<AtomicU64>,
    convert: fn(T) -> i16,
    channel: &'static str,
    faults: mpsc::SyncSender<CaptureEvent>,
) -> Result<cpal::Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mut mono = Vec::with_capacity(data.len() / channels.max(1) + 1);
                for frame in data.chunks(channels.max(1)) {
                    let mut acc: i32 = 0;
                    for sample in frame {
                        acc += convert(*sample) as i32;
                    }
                    mono.push((acc / frame.len().max(1) as i32) as i16);
                }
                let first_sample = sample_cursor.fetch_add(mono.len() as u64, Ordering::Relaxed);
                let _ = tx.try_send(CaptureSampleBatch {
                    first_sample,
                    samples: mono,
                });
            },
            move |error| {
                // Runs on the audio callback thread; sending is non-blocking
                // and the coordinator turns this into user-visible state.
                let _ = faults.try_send(CaptureEvent::StreamFailed {
                    channel,
                    error: error.to_string(),
                });
            },
            None,
        )
        .map_err(|error| format!("build_input_stream failed: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn cut_capture_chunk(
    channel: &'static str,
    sample_rate: u32,
    chunks_dir: &Path,
    chunk_tx: &mpsc::SyncSender<CaptureEvent>,
    accumulator: &mut Vec<i16>,
    timeline_samples: &mut u64,
    local_seq: &mut u32,
    take: usize,
) {
    if take == 0 || take > accumulator.len() {
        return;
    }
    let samples: Vec<i16> = accumulator.drain(..take).collect();
    let start_ms = (*timeline_samples * 1000 / sample_rate as u64) as i64;
    *timeline_samples += samples.len() as u64;
    let end_ms = (*timeline_samples * 1000 / sample_rate as u64) as i64;
    *local_seq += 1;
    let chunk_id = Uuid::new_v4().to_string();
    let file_path = chunks_dir.join(format!("{channel}-{local_seq:05}.wav"));
    match write_chunk_wav(&file_path, sample_rate, &samples) {
        Ok((byte_size, checksum)) => {
            let _ = chunk_tx.send(CaptureEvent::Chunk(RawChunk {
                channel,
                chunk_id,
                file_path: file_path.display().to_string(),
                start_ms,
                end_ms,
                byte_size,
                checksum,
            }));
        }
        Err(error) => {
            let _ = chunk_tx.send(CaptureEvent::ChunkWriteFailed {
                channel,
                start_ms,
                end_ms,
                error,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_capture_sample_batch(
    batch: CaptureSampleBatch,
    channel: &'static str,
    sample_rate: u32,
    samples_per_chunk: usize,
    chunks_dir: &Path,
    chunk_tx: &mpsc::SyncSender<CaptureEvent>,
    accumulator: &mut Vec<i16>,
    timeline_samples: &mut u64,
    expected_sample: &mut u64,
    local_seq: &mut u32,
) {
    let overlap = expected_sample.saturating_sub(batch.first_sample) as usize;
    if overlap >= batch.samples.len() {
        return;
    }
    let first_sample = batch.first_sample + overlap as u64;
    if first_sample > *expected_sample {
        if !accumulator.is_empty() {
            let pending = accumulator.len();
            cut_capture_chunk(
                channel,
                sample_rate,
                chunks_dir,
                chunk_tx,
                accumulator,
                timeline_samples,
                local_seq,
                pending,
            );
        }
        let gap_start_ms = (*expected_sample * 1000 / sample_rate as u64) as i64;
        let gap_end_ms = (first_sample * 1000 / sample_rate as u64) as i64;
        *timeline_samples = first_sample;
        if gap_end_ms > gap_start_ms {
            let _ = chunk_tx.send(CaptureEvent::SourceGap {
                channel,
                start_ms: gap_start_ms,
                end_ms: gap_end_ms,
                reason: "sample_queue_overflow",
            });
        }
    }
    let samples = &batch.samples[overlap..];
    accumulator.extend_from_slice(samples);
    *expected_sample = first_sample + samples.len() as u64;
    while accumulator.len() >= samples_per_chunk {
        cut_capture_chunk(
            channel,
            sample_rate,
            chunks_dir,
            chunk_tx,
            accumulator,
            timeline_samples,
            local_seq,
            samples_per_chunk,
        );
    }
}

/// Free bytes available on the volume that holds `path`.
///
/// `None` means "cannot tell" — an unsupported platform or a failed call. No
/// caller may treat that as "full": refusing to record because a disk query
/// failed would be worse than the problem it guards against.
#[cfg(windows)]
pub(crate) fn free_disk_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // The directory may not exist yet when this runs before a session; walk up
    // to the nearest existing ancestor, which is on the same volume.
    let mut probe = path;
    while !probe.exists() {
        probe = probe.parent()?;
    }
    let mut wide: Vec<u16> = probe.as_os_str().encode_wide().collect();
    wide.push(0);

    let mut available: u64 = 0;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call,
    // and `available` is a valid writable u64. The other two out-params are
    // optional per the API contract and passed as null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

#[cfg(not(windows))]
pub(crate) fn free_disk_bytes(_path: &Path) -> Option<u64> {
    None
}

fn human_gib(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

fn write_chunk_wav(
    path: &Path,
    sample_rate: u32,
    samples: &[i16],
) -> Result<(i64, String), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|error| format!("wav create failed: {error}"))?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|error| format!("wav write failed: {error}"))?;
    }
    writer
        .finalize()
        .map_err(|error| format!("wav finalize failed: {error}"))?;

    let bytes = std::fs::read(path).map_err(|error| format!("wav read-back failed: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((bytes.len() as i64, format!("{:x}", hasher.finalize())))
}

pub(crate) struct CaptureReady {
    pub(crate) device_name: String,
}

/// Spawns the capture thread for one channel. Returns once the cpal stream is
/// actually playing (or failed to build). The thread cuts durable WAV chunks
/// and hands them to the coordinator; it never touches Genesis itself so the
/// ledger has a single writer.
pub(crate) fn spawn_capture_thread(
    kind: ChannelKind,
    channel: &'static str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    chunk_tx: mpsc::SyncSender<CaptureEvent>,
    chunks_dir: PathBuf,
) -> Result<CaptureReady, String> {
    spawn_capture_thread_with_fragment_ms(
        kind, channel, device_id, stop, chunk_tx, chunks_dir, CHUNK_MS,
    )
}

fn spawn_capture_thread_with_fragment_ms(
    kind: ChannelKind,
    channel: &'static str,
    device_id: Option<String>,
    stop: Arc<AtomicBool>,
    chunk_tx: mpsc::SyncSender<CaptureEvent>,
    chunks_dir: PathBuf,
    fragment_ms: u64,
) -> Result<CaptureReady, String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();

    thread::spawn(move || {
        let device_kind = match kind {
            ChannelKind::Mic => CaptureDeviceKind::Mic,
            ChannelKind::SystemLoopback => CaptureDeviceKind::SystemLoopback,
        };
        // WASAPI exposes render endpoints as loopback capture sources:
        // opening an *input* stream on the selected *output* device records
        // everything the machine plays (the "them" side of an online meeting).
        let device = match resolve_capture_device(device_kind, device_id.as_deref()) {
            Ok(device) => device,
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };
        let device_name = device.name().unwrap_or_else(|_| "unknown device".into());

        // WASAPI loopback records a *render* endpoint through an input
        // stream, and cpal turns on AUDCLNT_STREAMFLAGS_LOOPBACK by itself
        // whenever `build_input_stream` targets an `eRender` device. What it
        // does NOT do is describe that device's input side: both
        // `default_input_config()` and `supported_input_configs()` are gated
        // on `eCapture`, so a render endpoint answers "not supported" and an
        // empty list. Loopback delivers the render mix format, so ask the
        // output side for the format and hand it to `build_input_stream`.
        let supported = match kind {
            ChannelKind::Mic => device.default_input_config(),
            ChannelKind::SystemLoopback => device.default_output_config(),
        };
        let supported = match supported {
            Ok(config) => config,
            Err(config_error) => {
                // Fall back to enumeration on the matching side; some drivers
                // report no default but still advertise usable ranges.
                let enumerated = match kind {
                    ChannelKind::Mic => device
                        .supported_input_configs()
                        .ok()
                        .and_then(|mut c| c.next()),
                    ChannelKind::SystemLoopback => device
                        .supported_output_configs()
                        .ok()
                        .and_then(|mut c| c.next()),
                };
                match enumerated.map(|range| range.with_max_sample_rate()) {
                    Some(config) => config,
                    None => {
                        let _ = ready_tx.send(Err(format!(
                            "อุปกรณ์ '{device_name}' ไม่รองรับการจับเสียง ({config_error})"
                        )));
                        return;
                    }
                }
            }
        };

        let sample_format = supported.sample_format();
        let stream_config: cpal::StreamConfig = supported.config();
        let sample_rate = stream_config.sample_rate.0;
        let channels = stream_config.channels as usize;

        let (sample_tx, sample_rx) =
            mpsc::sync_channel::<CaptureSampleBatch>(CAPTURE_SAMPLE_QUEUE_CAPACITY);
        let sample_cursor = Arc::new(AtomicU64::new(0));
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_stream::<f32>(
                &device,
                &stream_config,
                channels,
                sample_tx,
                Arc::clone(&sample_cursor),
                sample_f32_to_i16,
                channel,
                chunk_tx.clone(),
            ),
            cpal::SampleFormat::I16 => build_stream::<i16>(
                &device,
                &stream_config,
                channels,
                sample_tx,
                Arc::clone(&sample_cursor),
                sample_i16_to_i16,
                channel,
                chunk_tx.clone(),
            ),
            cpal::SampleFormat::U16 => build_stream::<u16>(
                &device,
                &stream_config,
                channels,
                sample_tx,
                Arc::clone(&sample_cursor),
                sample_u16_to_i16,
                channel,
                chunk_tx.clone(),
            ),
            other => Err(format!("รูปแบบตัวอย่างเสียง {other:?} ยังไม่รองรับ")),
        };
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };
        if let Err(error) = stream.play() {
            let _ = ready_tx.send(Err(format!("เริ่มสตรีมเสียงไม่สำเร็จ: {error}")));
            return;
        }
        let _ = ready_tx.send(Ok(device_name));

        let samples_per_chunk = (sample_rate as u64 * fragment_ms / 1000) as usize;
        let min_final_samples = (sample_rate as u64 * MIN_FINAL_CHUNK_MS / 1000) as usize;
        let mut accumulator: Vec<i16> = Vec::with_capacity(samples_per_chunk + 4096);
        let mut timeline_samples: u64 = 0;
        let mut expected_sample: u64 = 0;
        let mut local_seq: u32 = 0;

        loop {
            match sample_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(batch) => process_capture_sample_batch(
                    batch,
                    channel,
                    sample_rate,
                    samples_per_chunk,
                    &chunks_dir,
                    &chunk_tx,
                    &mut accumulator,
                    &mut timeline_samples,
                    &mut expected_sample,
                    &mut local_seq,
                ),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }

        // Stop the callback source first, then flush whatever already arrived.
        drop(stream);
        for batch in sample_rx.try_iter() {
            process_capture_sample_batch(
                batch,
                channel,
                sample_rate,
                samples_per_chunk,
                &chunks_dir,
                &chunk_tx,
                &mut accumulator,
                &mut timeline_samples,
                &mut expected_sample,
                &mut local_seq,
            );
        }
        let captured_end_sample = sample_cursor.load(Ordering::Acquire);
        if captured_end_sample > expected_sample {
            if !accumulator.is_empty() {
                let pending = accumulator.len();
                cut_capture_chunk(
                    channel,
                    sample_rate,
                    &chunks_dir,
                    &chunk_tx,
                    &mut accumulator,
                    &mut timeline_samples,
                    &mut local_seq,
                    pending,
                );
            }
            let gap_start_ms = (expected_sample * 1000 / sample_rate as u64) as i64;
            let gap_end_ms = (captured_end_sample * 1000 / sample_rate as u64) as i64;
            if gap_end_ms > gap_start_ms {
                let _ = chunk_tx.send(CaptureEvent::SourceGap {
                    channel,
                    start_ms: gap_start_ms,
                    end_ms: gap_end_ms,
                    reason: "sample_queue_overflow",
                });
            }
            timeline_samples = captured_end_sample;
            expected_sample = captured_end_sample;
        }
        if accumulator.len() >= min_final_samples {
            let take = accumulator.len();
            cut_capture_chunk(
                channel,
                sample_rate,
                &chunks_dir,
                &chunk_tx,
                &mut accumulator,
                &mut timeline_samples,
                &mut local_seq,
                take,
            );
        } else if !accumulator.is_empty() {
            let gap_start_sample = expected_sample.saturating_sub(accumulator.len() as u64);
            let gap_start_ms = (gap_start_sample * 1000 / sample_rate as u64) as i64;
            let gap_end_ms = (expected_sample * 1000 / sample_rate as u64) as i64;
            if gap_end_ms > gap_start_ms {
                let _ = chunk_tx.send(CaptureEvent::SourceGap {
                    channel,
                    start_ms: gap_start_ms,
                    end_ms: gap_end_ms,
                    reason: "final_tail_too_short",
                });
            }
            accumulator.clear();
        }
        // chunk_tx drops here; the coordinator sees Disconnected once every
        // channel thread has flushed.
    });

    ready_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "อุปกรณ์เสียงไม่ตอบสนองภายใน 10 วินาที".to_string())?
        .map(|device_name| CaptureReady { device_name })
}

// ---------------------------------------------------------------------------
// Persistent whisper worker (transcribe_live.py over JSONL stdin/stdout)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkerSegment {
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) text: String,
    pub(crate) confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkerResponse {
    // Present in the worker's JSON and deserialized for shape fidelity: the
    // coordinator correlates by request order, so it reads neither field.
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    channel: Option<String>,
    #[serde(default)]
    pub(crate) start_ms: i64,
    #[serde(default)]
    pub(crate) segments: Vec<WorkerSegment>,
    language: Option<String>,
    pub(crate) error: Option<String>,
    #[serde(default)]
    ready: bool,
    model: Option<String>,
    device: Option<String>,
    #[serde(rename = "computeType")]
    compute_type: Option<String>,
    backend: Option<String>,
    #[serde(rename = "windowId")]
    window_id: Option<String>,
    #[serde(rename = "windowStartMs")]
    window_start_ms: Option<i64>,
    #[serde(rename = "windowEndMs")]
    window_end_ms: Option<i64>,
    #[serde(rename = "finalWindow")]
    final_window: Option<bool>,
}

#[derive(Clone, Debug)]
struct WorkerRuntimeInfo {
    model: String,
    device: String,
    compute_type: String,
    backend: String,
}

pub(crate) struct LiveWorker {
    child: Child,
    stdin: std::process::ChildStdin,
    lines: mpsc::Receiver<String>,
    runtime_info: Option<WorkerRuntimeInfo>,
}

impl LiveWorker {
    pub(crate) fn spawn(runtime: &WhisperRuntime, language: Option<&str>) -> Result<Self, String> {
        crate::require_general_transcription_profile()?;
        crate::require_bundled_whisper_model(runtime)?;
        let profile = crate::transcription_profile()?;
        let script = crate::whisper_worker_script(runtime, true)?;

        // GPU profile needs the staged CUDA DLLs on PATH, same as the batch
        // path. If they are missing we degrade to CPU instead of refusing to
        // start the meeting — live latency suffers, capture does not.
        let mut effective_profile = profile.clone();
        let mut path_prefix: Option<&Path> = None;
        if profile == "gpu" {
            let missing = crate::REQUIRED_CUDA_DLLS
                .iter()
                .any(|dll| !runtime.cuda_bin.join(dll).is_file());
            if missing {
                effective_profile = "cpu".to_string();
            } else {
                path_prefix = Some(runtime.cuda_bin.as_path());
            }
        }

        if !runtime.python.exists() {
            return Err(format!(
                "FUNG Python runtime is missing at {}",
                runtime.python.display()
            ));
        }
        if !script.exists() {
            return Err(format!(
                "live worker script is missing at {}",
                script.display()
            ));
        }

        let mut command = Command::new(&runtime.python);
        command
            .arg(&script)
            .arg("--profile")
            .arg(&effective_profile);
        if let Some(model) = crate::bundled_whisper_model(runtime) {
            command.env("FUNG_WHISPER_MODEL", model);
        }
        command.env("HF_HUB_OFFLINE", "1");
        if let Some(language) = language {
            command.arg("--language").arg(language);
        }
        if let Some(prefix) = path_prefix {
            let inherited = std::env::var_os("PATH").unwrap_or_default();
            let joined = std::env::join_paths([prefix.as_os_str(), inherited.as_os_str()])
                .map_err(|error| format!("could not compose PATH: {error}"))?;
            command.env("PATH", joined);
        }

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to launch live worker: {error}"))?;

        let stdin = child.stdin.take().ok_or("live worker stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("live worker stdout unavailable")?;
        let stderr = child
            .stderr
            .take()
            .ok_or("live worker stderr unavailable")?;

        // Drain stderr so the child never blocks on a full pipe; keep a
        // bounded tail for diagnostics.
        thread::spawn(move || {
            let mut tail = String::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                crate::append_bounded(&mut tail, &line);
            }
            if !tail.trim().is_empty() {
                eprintln!("[live-worker stderr tail]\n{}", tail.trim());
            }
        });

        let (line_tx, line_rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            lines: line_rx,
            runtime_info: None,
        })
    }

    pub(crate) fn wait_ready(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + WORKER_READY_TIMEOUT;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("live worker did not become ready in time")?;
            let line = self
                .lines
                .recv_timeout(remaining)
                .map_err(|_| "live worker exited or stalled before ready".to_string())?;
            if let Ok(response) = serde_json::from_str::<WorkerResponse>(&line) {
                if response.ready {
                    self.runtime_info = Some(WorkerRuntimeInfo {
                        model: response.model.unwrap_or_else(|| "unknown".to_string()),
                        device: response.device.unwrap_or_else(|| "unknown".to_string()),
                        compute_type: response
                            .compute_type
                            .unwrap_or_else(|| "unknown".to_string()),
                        backend: response
                            .backend
                            .unwrap_or_else(|| "faster-whisper".to_string()),
                    });
                    return Ok(());
                }
                if let Some(error) = response.error {
                    return Err(error);
                }
            }
        }
    }

    pub(crate) fn transcribe_chunk(&mut self, chunk: &RawChunk) -> Result<WorkerResponse, String> {
        let request = serde_json::json!({
            "id": chunk.chunk_id,
            "path": chunk.file_path,
            "channel": chunk.channel,
            "startMs": chunk.start_ms,
        });
        writeln!(self.stdin, "{request}")
            .map_err(|error| format!("worker stdin closed: {error}"))?;
        let deadline = Instant::now() + WORKER_CHUNK_TIMEOUT;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("live worker timed out on a chunk")?;
            let line = self
                .lines
                .recv_timeout(remaining)
                .map_err(|_| "live worker stopped responding".to_string())?;
            match serde_json::from_str::<WorkerResponse>(&line) {
                Ok(response) if response.ready => continue,
                Ok(response) => return Ok(response),
                Err(_) => continue, // non-JSON noise on stdout — skip
            }
        }
    }

    fn transcribe_window(
        &mut self,
        window: &crate::live_transcript::DecodeWindow,
    ) -> Result<WorkerResponse, String> {
        let request = serde_json::json!({
            "id": window.id,
            "startMs": window.start_ms,
            "window": {
                "id": window.id,
                "startMs": window.start_ms,
                "endMs": window.end_ms,
                "final": window.is_final,
                "fragments": window.fragments.iter().map(|fragment| serde_json::json!({
                    "id": fragment.id,
                    "path": fragment.path,
                    "fragmentStartMs": fragment.fragment_start_ms,
                    "fragmentEndMs": fragment.fragment_end_ms,
                    "clipStartMs": fragment.clip_start_ms,
                    "clipEndMs": fragment.clip_end_ms,
                })).collect::<Vec<_>>(),
            },
        });
        writeln!(self.stdin, "{request}")
            .map_err(|error| format!("worker stdin closed: {error}"))?;
        let deadline = Instant::now() + WORKER_CHUNK_TIMEOUT;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("live worker timed out on a rolling window")?;
            let line = self
                .lines
                .recv_timeout(remaining)
                .map_err(|_| "live worker stopped responding".to_string())?;
            match serde_json::from_str::<WorkerResponse>(&line) {
                Ok(response) if response.ready => continue,
                Ok(response) => return Ok(response),
                Err(_) => continue,
            }
        }
    }

    fn runtime_info(&self) -> Option<&WorkerRuntimeInfo> {
        self.runtime_info.as_ref()
    }

    pub(crate) fn shutdown(mut self) {
        let _ = writeln!(self.stdin, "{}", serde_json::json!({"cmd": "shutdown"}));
        drop(self.stdin);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(200)),
                _ => break,
            }
        }
        let _ = self.child.kill();
    }
}

#[derive(Clone)]
struct RevisionedSessionContext {
    meeting_session_id: String,
    sources: HashMap<String, crate::meeting_intelligence_schema::MeetingScope>,
}

fn create_revisioned_session(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    channels: &[&str],
) -> Result<RevisionedSessionContext, String> {
    let meeting_session_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let mut mutations = vec![genesis_adapter::upsert(
        "meeting_sessions",
        serde_json::json!({
            "id": meeting_session_id,
            "project_id": project_id,
            "recording_id": recording_id,
            "session_generation": 1,
            "source_mode": "desktop_local_capture",
            "state": "active",
            "owner_scope": "local_unverified",
            "policy_version": "meeting-intelligence-v1",
            "revision": 1,
            "contract_version": crate::meeting_intelligence_schema::CONTRACT_VERSION,
            "created_at": timestamp,
            "updated_at": timestamp,
        }),
    )];
    let mut sources = HashMap::new();
    for (index, channel) in channels.iter().enumerate() {
        let source_generation = index as i64 + 1;
        let source_session_id = Uuid::new_v4().to_string();
        let scope = crate::meeting_intelligence_schema::MeetingScope {
            project_id: project_id.to_string(),
            recording_id: recording_id.to_string(),
            meeting_session_id: meeting_session_id.clone(),
            source_session_id: source_session_id.clone(),
            track_id: (*channel).to_string(),
            source_generation,
        };
        mutations.push(genesis_adapter::upsert(
            "meeting_source_sessions",
            serde_json::json!({
                "id": source_session_id,
                "project_id": project_id,
                "recording_id": recording_id,
                "meeting_session_id": meeting_session_id,
                "source_kind": if *channel == CHANNEL_MIC { "microphone_capture" } else { "system_loopback_capture" },
                "source_generation": source_generation,
                "state": "active",
                "contract_version": crate::meeting_intelligence_schema::CONTRACT_VERSION,
                "created_at": timestamp,
                "ended_at": null,
            }),
        ));
        sources.insert((*channel).to_string(), scope);
    }
    genesis_adapter::commit_rows(storage, mutations)?;
    Ok(RevisionedSessionContext {
        meeting_session_id,
        sources,
    })
}

fn update_revisioned_session_state(
    storage: &genesis_block_native::Storage,
    context: &RevisionedSessionContext,
    state: &str,
) {
    let timestamp = now();
    let mut mutations = vec![genesis_adapter::upsert(
        "meeting_sessions",
        serde_json::json!({
            "id": context.meeting_session_id,
            "state": state,
            "updated_at": timestamp,
        }),
    )];
    for scope in context.sources.values() {
        let source_unavailable = match genesis_adapter::query(
            storage,
            "meeting_source_sessions",
            &["state"],
            vec![genesis_adapter::eq(
                "meeting_source_sessions",
                "id",
                serde_json::json!(scope.source_session_id),
            )],
            1,
        ) {
            Ok(rows) => rows
                .first()
                .and_then(|row| row.get("meeting_source_sessions.state"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|state| state == "unavailable"),
            Err(error) => {
                eprintln!(
                    "[live-transcript] could not read source state before session close: {error}"
                );
                continue;
            }
        };
        if source_unavailable {
            continue;
        }
        mutations.push(genesis_adapter::upsert(
            "meeting_source_sessions",
            serde_json::json!({
                "id": scope.source_session_id,
                "state": state,
                "ended_at": timestamp,
            }),
        ));
    }
    if let Err(error) = genesis_adapter::commit_rows(storage, mutations) {
        eprintln!("[live-transcript] could not update revisioned session state: {error}");
    }
}

fn update_revisioned_source_state(
    storage: &genesis_block_native::Storage,
    context: &RevisionedSessionContext,
    channel: &str,
    state: &str,
) {
    let Some(scope) = context.sources.get(channel) else {
        return;
    };
    let timestamp = now();
    if let Err(error) = genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "meeting_source_sessions",
            serde_json::json!({
                "id": scope.source_session_id,
                "state": state,
                "ended_at": timestamp,
            }),
        )],
    ) {
        eprintln!("[live-transcript] could not update source state: {error}");
    }
}

fn create_live_model_run(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
    run: &WorkerRuntimeInfo,
    language: Option<&str>,
) -> Result<String, String> {
    let timestamp = now();
    let provider_id = format!("local-whisper-live-{}", run.backend);
    let model_name = Path::new(&run.model)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(&run.model);
    let run_id = Uuid::new_v4().to_string();
    genesis_adapter::commit_rows(
        storage,
        vec![
            genesis_adapter::upsert(
                "model_providers",
                serde_json::json!({
                    "id": provider_id,
                    "label": format!("Local {}", run.backend),
                    "runtime_location": "local",
                    "kind": "transcription",
                    "enabled": true,
                    "config_json": {"backend": run.backend},
                    "created_at": timestamp,
                    "updated_at": timestamp,
                }),
            ),
            genesis_adapter::upsert(
                "model_runs",
                serde_json::json!({
                    "id": run_id,
                    "recording_id": recording_id,
                    "provider_id": provider_id,
                    "model_name": model_name,
                    "task_kind": "transcription",
                    "runtime_location": "local",
                    "input_ref": format!("audio_chunks:{recording_id}"),
                    "output_ref": format!("transcript_revisions:{recording_id}"),
                    "parameters_json": {
                        "device": run.device,
                        "computeType": run.compute_type,
                        "language": language,
                    },
                    "created_at": timestamp,
                }),
            ),
        ],
    )?;
    Ok(run_id)
}

struct RevisionedTrack {
    scope: crate::meeting_intelligence_schema::MeetingScope,
    next_sequence_no: i64,
    last_source_end_ms: i64,
    scheduler: crate::live_transcript::RollingWindowScheduler,
    tracker: crate::live_transcript::RevisionTracker,
    pending_sources: Vec<crate::meeting_intelligence_schema::SourceCoverageInput>,
    pending_chunks: Vec<RawChunk>,
    retry_batch: Option<(
        crate::meeting_intelligence_schema::MeetingIngestBatchRequest,
        Vec<RevisionCandidate>,
    )>,
}

impl RevisionedTrack {
    fn new(scope: crate::meeting_intelligence_schema::MeetingScope) -> Self {
        Self {
            scope,
            next_sequence_no: 0,
            last_source_end_ms: 0,
            scheduler: crate::live_transcript::RollingWindowScheduler::new(),
            tracker: crate::live_transcript::RevisionTracker::new(),
            pending_sources: Vec::new(),
            pending_chunks: Vec::new(),
            retry_batch: None,
        }
    }

    fn add_chunk(
        &mut self,
        chunk: RawChunk,
    ) -> Result<crate::live_transcript::WindowBatch, crate::live_transcript::ScheduleError> {
        if chunk.start_ms > self.last_source_end_ms {
            let sequence_no = self.next_sequence_no;
            self.next_sequence_no += 1;
            self.pending_sources
                .push(crate::meeting_intelligence_schema::SourceCoverageInput {
                    id: format!("coverage::{}::{sequence_no}", self.scope.source_session_id),
                    source_session_id: self.scope.source_session_id.clone(),
                    track_id: self.scope.track_id.clone(),
                    source_generation: self.scope.source_generation,
                    sequence_no,
                    start_ms: self.last_source_end_ms,
                    end_ms: chunk.start_ms,
                    kind: crate::meeting_intelligence_schema::SourceCoverageKind::Gap,
                    audio_chunk_id: None,
                    file_path: None,
                    byte_size: None,
                    checksum: None,
                    gap_reason: Some("capture_interval_unavailable".to_string()),
                });
        }
        let sequence_no = self.next_sequence_no;
        self.next_sequence_no += 1;
        let coverage_id = format!("coverage::{}::{sequence_no}", self.scope.source_session_id);
        self.pending_sources
            .push(crate::meeting_intelligence_schema::SourceCoverageInput {
                id: coverage_id.clone(),
                source_session_id: self.scope.source_session_id.clone(),
                track_id: self.scope.track_id.clone(),
                source_generation: self.scope.source_generation,
                sequence_no,
                start_ms: chunk.start_ms,
                end_ms: chunk.end_ms,
                kind: crate::meeting_intelligence_schema::SourceCoverageKind::Audio,
                audio_chunk_id: Some(chunk.chunk_id.clone()),
                file_path: Some(chunk.file_path.clone()),
                byte_size: Some(chunk.byte_size),
                checksum: Some(chunk.checksum.clone()),
                gap_reason: None,
            });
        self.last_source_end_ms = chunk.end_ms;
        self.pending_chunks.push(chunk);
        let latest = self
            .pending_chunks
            .last()
            .expect("just appended pending audio chunk");
        self.scheduler.push(
            &self.scope.track_id,
            crate::live_transcript::AudioFragmentRef {
                id: coverage_id,
                path: latest.file_path.clone(),
                sequence_no,
                start_ms: latest.start_ms,
                end_ms: latest.end_ms,
            },
        )
    }

    fn add_gap(
        &mut self,
        start_ms: i64,
        end_ms: i64,
        reason: &str,
    ) -> Result<(), crate::live_transcript::ScheduleError> {
        if start_ms < self.last_source_end_ms || end_ms <= start_ms {
            return Err(crate::live_transcript::ScheduleError::InvalidRange);
        }
        let sequence_no = self.next_sequence_no;
        self.next_sequence_no += 1;
        self.pending_sources
            .push(crate::meeting_intelligence_schema::SourceCoverageInput {
                id: format!("coverage::{}::{sequence_no}", self.scope.source_session_id),
                source_session_id: self.scope.source_session_id.clone(),
                track_id: self.scope.track_id.clone(),
                source_generation: self.scope.source_generation,
                sequence_no,
                start_ms,
                end_ms,
                kind: crate::meeting_intelligence_schema::SourceCoverageKind::Gap,
                audio_chunk_id: None,
                file_path: None,
                byte_size: None,
                checksum: None,
                gap_reason: Some(reason.to_string()),
            });
        self.last_source_end_ms = end_ms;
        self.scheduler.reset();
        Ok(())
    }

    fn replay_pending_chunk(
        &mut self,
        chunk: &RawChunk,
    ) -> Result<crate::live_transcript::WindowBatch, crate::live_transcript::ScheduleError> {
        let Some(source) = self
            .pending_sources
            .iter()
            .find(|source| source.audio_chunk_id.as_deref() == Some(chunk.chunk_id.as_str()))
        else {
            return Err(crate::live_transcript::ScheduleError::InvalidRange);
        };
        self.scheduler.push(
            &self.scope.track_id,
            crate::live_transcript::AudioFragmentRef {
                id: source.id.clone(),
                path: chunk.file_path.clone(),
                sequence_no: source.sequence_no,
                start_ms: chunk.start_ms,
                end_ms: chunk.end_ms,
            },
        )
    }
}

#[derive(Clone)]
struct RevisionCandidate {
    utterance_id: String,
    revision: u64,
    expected_persisted_revision: i64,
    hypothesis: crate::live_transcript::TranscriptHypothesis,
    coverage_ids: Vec<String>,
}

fn window_coverage_for_range(
    window: &crate::live_transcript::DecodeWindow,
    start_ms: i64,
    end_ms: i64,
) -> Vec<String> {
    window
        .fragments
        .iter()
        .filter(|fragment| fragment.clip_start_ms < end_ms && start_ms < fragment.clip_end_ms)
        .map(|fragment| fragment.id.clone())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn emit_transcript_event(
    app: &tauri::AppHandle,
    context: &RevisionedSessionContext,
    scope: &crate::meeting_intelligence_schema::MeetingScope,
    event_type: &str,
    state: &str,
    utterance_id: Option<String>,
    revision: Option<u64>,
    hypothesis: Option<&crate::live_transcript::TranscriptHypothesis>,
    audio_refs: Vec<String>,
    model_run_id: Option<&str>,
    committed_cursor: Option<i64>,
    persisted_revision: Option<u64>,
) {
    let (label, cluster) = if scope.track_id == CHANNEL_MIC {
        ("ไมโครโฟน", "mic")
    } else {
        ("เสียงระบบ", "system")
    };
    let emitted_at = now();
    let event = crate::live_transcript::LiveTranscriptEvent {
        schema_version: 2,
        event_id: Uuid::new_v4().to_string(),
        event_type: event_type.to_string(),
        project_id: scope.project_id.clone(),
        recording_id: scope.recording_id.clone(),
        meeting_session_id: context.meeting_session_id.clone(),
        source_session_id: scope.source_session_id.clone(),
        track_id: scope.track_id.clone(),
        source_generation: scope.source_generation,
        utterance_id,
        revision,
        supersedes_revision: revision.filter(|value| *value > 1).map(|value| value - 1),
        persisted_revision,
        state: state.to_string(),
        origin: "local_asr".to_string(),
        start_ms: hypothesis.map(|value| value.start_ms),
        end_ms: hypothesis.map(|value| value.end_ms),
        text: hypothesis.map(|value| value.text.clone()),
        language: hypothesis.and_then(|value| value.language.clone()),
        confidence: hypothesis.and_then(|value| value.confidence),
        attribution: crate::live_transcript::TranscriptAttribution {
            kind: "capture_channel".to_string(),
            participant_session_id: None,
            speaker_cluster_id: cluster.to_string(),
            label_snapshot: label.to_string(),
            evidence_revision: 0,
        },
        audio_refs,
        model_run_id: model_run_id.map(str::to_string),
        committed_cursor,
        emitted_at: emitted_at.clone(),
        received_at: Some(emitted_at),
        source_clock_uncertainty_ms: None,
        review_state: "unreviewed".to_string(),
        quality_flags: Vec::new(),
    };
    let _ = app.emit("live-transcript-v2", event);
}

fn decode_revision_windows(
    app: &tauri::AppHandle,
    context: &RevisionedSessionContext,
    track: &mut RevisionedTrack,
    worker: &mut LiveWorker,
    model_run_id: &str,
    windows: &[crate::live_transcript::DecodeWindow],
) -> Result<Vec<RevisionCandidate>, String> {
    let mut candidates: HashMap<String, RevisionCandidate> = HashMap::new();
    for window in windows {
        let response = worker.transcribe_window(window)?;
        if let Some(error) = response.error {
            return Err(error);
        }
        if response.window_id.as_deref() != Some(window.id.as_str())
            || response.window_start_ms != Some(window.start_ms)
            || response.window_end_ms != Some(window.end_ms)
            || response.final_window != Some(window.is_final)
        {
            return Err("live worker returned a mismatched decode-window response".to_string());
        }
        let hypotheses = response
            .segments
            .into_iter()
            .map(|segment| crate::live_transcript::TranscriptHypothesis {
                start_ms: window.start_ms.saturating_add(segment.start_ms),
                end_ms: window.start_ms.saturating_add(segment.end_ms),
                text: segment.text,
                language: response.language.clone(),
                confidence: segment.confidence,
            })
            .collect::<Vec<_>>();
        for update in track.tracker.observe(window, hypotheses) {
            match update {
                crate::live_transcript::TranscriptUpdate::Provisional {
                    utterance_id,
                    revision,
                    hypothesis,
                } => emit_transcript_event(
                    app,
                    context,
                    &track.scope,
                    "transcript.provisional",
                    "provisional",
                    Some(utterance_id),
                    Some(revision),
                    Some(&hypothesis),
                    window_coverage_for_range(window, hypothesis.start_ms, hypothesis.end_ms),
                    Some(model_run_id),
                    None,
                    None,
                ),
                crate::live_transcript::TranscriptUpdate::Discard {
                    utterance_id,
                    revision,
                } => emit_transcript_event(
                    app,
                    context,
                    &track.scope,
                    "transcript.discarded",
                    "discarded",
                    Some(utterance_id),
                    Some(revision),
                    None,
                    Vec::new(),
                    Some(model_run_id),
                    None,
                    None,
                ),
                crate::live_transcript::TranscriptUpdate::CommitCandidate {
                    utterance_id,
                    revision,
                    expected_persisted_revision,
                    hypothesis,
                } => {
                    let coverage_ids =
                        window_coverage_for_range(window, hypothesis.start_ms, hypothesis.end_ms);
                    if !coverage_ids.is_empty() {
                        candidates.insert(
                            utterance_id.clone(),
                            RevisionCandidate {
                                utterance_id,
                                revision,
                                expected_persisted_revision,
                                hypothesis,
                                coverage_ids,
                            },
                        );
                    }
                }
            }
        }
    }
    Ok(candidates.into_values().collect())
}

#[allow(clippy::too_many_arguments)]
fn commit_revisioned_sources(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    context: &RevisionedSessionContext,
    track: &mut RevisionedTrack,
    candidates: Vec<RevisionCandidate>,
    model_run_id: &str,
    through_sequence: i64,
) -> Result<(), String> {
    let (request, candidates) = if let Some(retry) = track.retry_batch.take() {
        retry
    } else {
        let sources = track
            .pending_sources
            .iter()
            .filter(|source| source.sequence_no <= through_sequence)
            .cloned()
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Ok(());
        }
        if sources.len() > 64 {
            return Err("revisioned source batch exceeded the 64-source bound".to_string());
        }
        let mut candidates = candidates
            .into_iter()
            .filter(|candidate| candidate.expected_persisted_revision >= 0)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.hypothesis
                .start_ms
                .cmp(&right.hypothesis.start_ms)
                .then_with(|| left.utterance_id.cmp(&right.utterance_id))
        });
        let revisions = candidates
            .iter()
            .map(|candidate| {
                let revision = candidate.expected_persisted_revision + 1;
                crate::meeting_intelligence_schema::TranscriptRevisionInput {
                    id: Uuid::new_v4().to_string(),
                    utterance_id: candidate.utterance_id.clone(),
                    revision,
                    supersedes_revision: (revision > 1).then_some(revision - 1),
                    expected_revision: Some(candidate.expected_persisted_revision),
                    origin: crate::meeting_intelligence_schema::TranscriptOrigin::LocalAsr,
                    raw_text: candidate.hypothesis.text.clone(),
                    effective_text: candidate.hypothesis.text.clone(),
                    language: candidate.hypothesis.language.clone(),
                    confidence: candidate.hypothesis.confidence,
                    start_ms: candidate.hypothesis.start_ms,
                    end_ms: candidate.hypothesis.end_ms,
                    model_run_id: Some(model_run_id.to_string()),
                    review_state: "unreviewed".to_string(),
                }
            })
            .collect::<Vec<_>>();
        let revision_coverage = revisions
            .iter()
            .zip(candidates.iter())
            .map(|(revision, candidate)| {
                crate::meeting_intelligence_schema::RevisionCoverageBinding {
                    revision_id: revision.id.clone(),
                    coverage_ids: candidate.coverage_ids.clone(),
                }
            })
            .collect::<Vec<_>>();
        let request = crate::meeting_intelligence_schema::MeetingIngestBatchRequest {
            operation_id: Uuid::new_v4().to_string(),
            scope: track.scope.clone(),
            sources: sources.clone(),
            revisions,
            revision_coverage,
        };
        (request, candidates)
    };
    let mut result = None;
    let mut last_error = None;
    for _ in 0..2 {
        let attempt = match genesis_adapter::begin_meeting_commit(
            storage,
            &Uuid::new_v4().to_string(),
            &now(),
        ) {
            Ok(attempt) => attempt,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        match genesis_adapter::commit_meeting_ingest_batch(storage, &attempt, &request) {
            Ok(committed) => {
                result = Some(committed);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let result = match result {
        Some(result) => result,
        None => {
            let error =
                last_error.unwrap_or_else(|| "revisioned source batch did not commit".to_string());
            track.retry_batch = Some((request, candidates));
            return Err(error);
        }
    };

    let replay_cursor = crate::meeting_intelligence_schema::MeetingReplayCursor {
        recording_id: track.scope.recording_id.clone(),
        after_cursor: result.first_cursor.saturating_sub(1),
    };
    if let Ok(page) = genesis_adapter::replay_meeting_events(
        storage,
        &track.scope.project_id,
        &track.scope.recording_id,
        &replay_cursor,
        64,
    ) {
        for event in page.events {
            let _ = app.emit("meeting-transcript-event", event);
        }
    }
    if !result.revision_ids.is_empty() {
        crate::observe_committed_meeting_agent_transcript_event(
            app,
            &track.scope.project_id,
            &track.scope.recording_id,
            result.last_cursor,
        );
    }

    for (index, candidate) in candidates.iter().enumerate() {
        let persisted_revision = candidate.expected_persisted_revision + 1;
        if let Err(error) = track.tracker.mark_committed(
            &candidate.utterance_id,
            persisted_revision,
            &candidate.hypothesis.text,
        ) {
            eprintln!(
                "[live-transcript] committed revision could not update in-memory tracker: {error}"
            );
        }
        let cursor = result.first_cursor + index as i64;
        emit_transcript_event(
            app,
            context,
            &track.scope,
            "transcript.committed",
            "committed",
            Some(candidate.utterance_id.clone()),
            Some(candidate.revision),
            Some(&candidate.hypothesis),
            candidate.coverage_ids.clone(),
            request
                .revisions
                .get(index)
                .and_then(|revision| revision.model_run_id.as_deref())
                .or(Some(model_run_id)),
            Some(cursor),
            Some(persisted_revision as u64),
        );
        let (speaker, label) = if track.scope.track_id == CHANNEL_MIC {
            ("me", "เรา")
        } else {
            ("them", "อีกฝ่าย")
        };
        let segment = LiveSegmentEvent {
            recording_id: track.scope.recording_id.clone(),
            segment_id: candidate.utterance_id.clone(),
            channel: track.scope.track_id.clone(),
            speaker: label.to_string(),
            start_ms: candidate.hypothesis.start_ms,
            end_ms: candidate.hypothesis.end_ms,
            text: candidate.hypothesis.text.clone(),
            confidence: candidate.hypothesis.confidence,
        };
        {
            let mut window = recent.lock().expect("recent buffer mutex poisoned");
            window.push_back(RecentSegment {
                speaker: speaker.to_string(),
                channel: segment.channel.clone(),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                text: segment.text.clone(),
            });
            while window.len() > RECENT_SEGMENT_CAP {
                window.pop_front();
            }
        }
        let _ = app.emit("live-segment", segment);
    }
    let committed_ids = request
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    track
        .pending_sources
        .retain(|source| !committed_ids.contains(source.id.as_str()));
    let committed_audio_ids = request
        .sources
        .iter()
        .filter_map(|source| source.audio_chunk_id.as_deref())
        .collect::<std::collections::HashSet<_>>();
    track
        .pending_chunks
        .retain(|chunk| !committed_audio_ids.contains(chunk.chunk_id.as_str()));
    track.retry_batch = None;
    Ok(())
}

fn flush_revisioned_track_before_capture_gap(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    context: &RevisionedSessionContext,
    track: &mut RevisionedTrack,
    worker: &mut LiveWorker,
    model_run_id: &str,
) -> Result<(), String> {
    let final_window = track
        .scheduler
        .flush(&track.scope.track_id)
        .map_err(|error| format!("revisioned pre-gap flush failed: {error:?}"))?;
    if let Some(window) = final_window {
        let through_sequence = track.next_sequence_no.saturating_sub(1);
        let candidates = decode_revision_windows(
            app,
            context,
            track,
            worker,
            model_run_id,
            std::slice::from_ref(&window),
        )?;
        commit_revisioned_sources(
            app,
            storage,
            recent,
            context,
            track,
            candidates,
            model_run_id,
            through_sequence,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_revisioned_capture_gap(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    recording_id: &str,
    context: Option<&RevisionedSessionContext>,
    model_run_id: &mut Option<String>,
    worker: &mut Option<LiveWorker>,
    tracks: &mut HashMap<String, RevisionedTrack>,
    channel: &str,
    start_ms: i64,
    end_ms: i64,
    reason: &str,
) {
    let flush_error = match (
        context,
        model_run_id.as_deref(),
        worker.as_mut(),
        tracks.get_mut(channel),
    ) {
        (Some(context), Some(active_model_run_id), Some(active_worker), Some(track)) => {
            flush_revisioned_track_before_capture_gap(
                app,
                storage,
                recent,
                context,
                track,
                active_worker,
                active_model_run_id,
            )
            .err()
        }
        _ => None,
    };
    if let Some(error) = flush_error {
        emit_status(
            app,
            recording_id,
            "degraded",
            Some(format!("ปิดหน้าต่างก่อน audio gap ไม่สำเร็จ ({error})")),
            None,
            None,
        );
        if let Some(dead) = worker.take() {
            dead.shutdown();
        }
        *model_run_id = None;
    }

    let Some(track) = tracks.get_mut(channel) else {
        return;
    };
    if let Err(error) = track.add_gap(start_ms, end_ms, reason) {
        emit_status(
            app,
            recording_id,
            "degraded",
            Some(format!("บันทึกขอบเขต audio gap ไม่สำเร็จ ({error:?})")),
            None,
            None,
        );
        return;
    }
    let discarded = track.tracker.discard_uncommitted();
    if let Some(context) = context {
        for update in discarded {
            if let crate::live_transcript::TranscriptUpdate::Discard {
                utterance_id,
                revision,
            } = update
            {
                emit_transcript_event(
                    app,
                    context,
                    &track.scope,
                    "transcript.discarded",
                    "discarded",
                    Some(utterance_id),
                    Some(revision),
                    None,
                    Vec::new(),
                    model_run_id.as_deref(),
                    None,
                    None,
                );
            }
        }
    }
}

fn revisioned_uncovered_chunks(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
    context: &RevisionedSessionContext,
) -> Result<HashMap<String, Vec<RawChunk>>, String> {
    let source_ids = context
        .sources
        .values()
        .map(|scope| scope.source_session_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let coverage = genesis_adapter::query_all(
        storage,
        "meeting_source_coverage",
        &["source_session_id", "audio_chunk_id"],
        vec![genesis_adapter::eq(
            "meeting_source_coverage",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    let covered = coverage
        .iter()
        .filter(|row| {
            row.get("meeting_source_coverage.source_session_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|source_id| source_ids.contains(source_id))
        })
        .filter_map(|row| {
            row.get("meeting_source_coverage.audio_chunk_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<std::collections::HashSet<_>>();
    let rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &[
            "id",
            "file_path",
            "start_ms",
            "end_ms",
            "byte_size",
            "checksum",
        ],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    let mut missing: HashMap<String, Vec<RawChunk>> = HashMap::new();
    for row in rows {
        let chunk_id = genesis_adapter::string(&row, "audio_chunks.id")?;
        if covered.contains(&chunk_id) {
            continue;
        }
        let file_path = genesis_adapter::string(&row, "audio_chunks.file_path")?;
        let channel = Path::new(&file_path)
            .file_name()
            .and_then(|value| value.to_str())
            .and_then(channel_for_file_name);
        let Some(channel) = channel.filter(|channel| context.sources.contains_key(*channel)) else {
            continue;
        };
        missing
            .entry(channel.to_string())
            .or_default()
            .push(RawChunk {
                channel,
                chunk_id,
                file_path,
                start_ms: genesis_adapter::integer(&row, "audio_chunks.start_ms")?,
                end_ms: genesis_adapter::integer(&row, "audio_chunks.end_ms")?,
                byte_size: genesis_adapter::integer(&row, "audio_chunks.byte_size")?,
                checksum: genesis_adapter::string(&row, "audio_chunks.checksum")?,
            });
    }
    for chunks in missing.values_mut() {
        chunks.sort_by_key(|chunk| (chunk.start_ms, chunk.end_ms));
    }
    Ok(missing)
}

#[allow(clippy::too_many_arguments)]
fn transcribe_revisioned_pending(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    runtime: &WhisperRuntime,
    language: Option<&str>,
    recording_id: &str,
    context: &RevisionedSessionContext,
    tracks: &mut HashMap<String, RevisionedTrack>,
) -> usize {
    let pending_by_channel = match revisioned_uncovered_chunks(storage, recording_id, context) {
        Ok(pending) => pending,
        Err(error) => {
            record_capture_fault(
                app,
                storage,
                context
                    .sources
                    .values()
                    .next()
                    .map(|scope| scope.project_id.as_str())
                    .unwrap_or_default(),
                recording_id,
                "live_meeting.transcript_pending",
                "session",
                &error,
                format!("ตรวจสอบเสียงค้างสำหรับ revisioned catch-up ไม่ได้ ({error})"),
            );
            return 1;
        }
    };
    let total = pending_by_channel.values().map(Vec::len).sum::<usize>();
    if total == 0
        && tracks
            .values()
            .all(|track| track.pending_sources.is_empty())
    {
        return 0;
    }
    emit_status(
        app,
        recording_id,
        "transcribing",
        Some(format!(
            "กำลังถอดและ reconcile เสียง revisioned ที่ค้าง {total} ช่วง"
        )),
        None,
        None,
    );
    let mut worker = match LiveWorker::spawn(runtime, language).and_then(|mut worker| {
        worker.wait_ready()?;
        Ok(worker)
    }) {
        Ok(worker) => worker,
        Err(error) => {
            record_capture_fault(
                app,
                storage,
                context
                    .sources
                    .values()
                    .next()
                    .map(|scope| scope.project_id.as_str())
                    .unwrap_or_default(),
                recording_id,
                "live_meeting.transcript_pending",
                "session",
                &error,
                format!("เปิด worker สำหรับ revisioned catch-up ไม่ได้ ({error}) — เสียงยังอยู่ในเครื่อง"),
            );
            return total.max(1);
        }
    };
    let project_id = context
        .sources
        .values()
        .next()
        .map(|scope| scope.project_id.as_str())
        .unwrap_or_default();
    let Some(runtime_info) = worker.runtime_info() else {
        worker.shutdown();
        return total.max(1);
    };
    let model_run_id = match create_live_model_run(storage, recording_id, runtime_info, language) {
        Ok(run_id) => run_id,
        Err(error) => {
            worker.shutdown();
            record_capture_fault(
                app,
                storage,
                project_id,
                recording_id,
                "live_meeting.transcript_pending",
                "session",
                &error,
                format!("บันทึก provenance ของ worker catch-up ไม่ได้ ({error})"),
            );
            return total.max(1);
        }
    };

    let mut remaining = total;
    for (channel, track) in tracks.iter_mut() {
        let mut queued = track.pending_chunks.clone();
        let queued_ids = queued
            .iter()
            .map(|chunk| chunk.chunk_id.clone())
            .collect::<std::collections::HashSet<_>>();
        queued.extend(
            pending_by_channel
                .get(channel)
                .into_iter()
                .flatten()
                .filter(|chunk| !queued_ids.contains(&chunk.chunk_id))
                .cloned(),
        );
        queued.sort_by_key(|chunk| (chunk.start_ms, chunk.end_ms));
        if queued.is_empty() && track.pending_sources.is_empty() {
            continue;
        }
        if track.retry_batch.is_none() {
            for update in track.tracker.discard_uncommitted() {
                if let crate::live_transcript::TranscriptUpdate::Discard {
                    utterance_id,
                    revision,
                } = update
                {
                    emit_transcript_event(
                        app,
                        context,
                        &track.scope,
                        "transcript.discarded",
                        "discarded",
                        Some(utterance_id),
                        Some(revision),
                        None,
                        Vec::new(),
                        Some(&model_run_id),
                        None,
                        None,
                    );
                }
            }
        }
        track.scheduler.reset();
        for chunk in queued {
            let existing_pending = track
                .pending_chunks
                .iter()
                .any(|pending| pending.chunk_id == chunk.chunk_id);
            let batch = if existing_pending {
                track.replay_pending_chunk(&chunk)
            } else {
                track.add_chunk(chunk.clone())
            };
            let batch = match batch {
                Ok(batch) => batch,
                Err(error) => {
                    emit_status(
                        app,
                        recording_id,
                        "degraded",
                        Some(format!("catch-up window schedule failed: {error:?}")),
                        None,
                        None,
                    );
                    remaining += 1;
                    continue;
                }
            };
            if batch.gap.is_some() {
                for update in track.tracker.discard_uncommitted() {
                    if let crate::live_transcript::TranscriptUpdate::Discard {
                        utterance_id,
                        revision,
                    } = update
                    {
                        emit_transcript_event(
                            app,
                            context,
                            &track.scope,
                            "transcript.discarded",
                            "discarded",
                            Some(utterance_id),
                            Some(revision),
                            None,
                            Vec::new(),
                            Some(&model_run_id),
                            None,
                            None,
                        );
                    }
                }
            }
            if batch.windows.is_empty() {
                continue;
            }
            let through_sequence = track.next_sequence_no.saturating_sub(1);
            let candidates = match decode_revision_windows(
                app,
                context,
                track,
                &mut worker,
                &model_run_id,
                &batch.windows,
            ) {
                Ok(candidates) => candidates,
                Err(error) => {
                    record_capture_fault(
                        app,
                        storage,
                        project_id,
                        recording_id,
                        "live_meeting.transcript_pending",
                        &track.scope.track_id,
                        &error,
                        format!("revisioned catch-up หยุดทำงาน ({error})"),
                    );
                    remaining += 1;
                    worker.shutdown();
                    return remaining;
                }
            };
            match commit_revisioned_sources(
                app,
                storage,
                recent,
                context,
                track,
                candidates,
                &model_run_id,
                through_sequence,
            ) {
                Ok(()) => remaining = remaining.saturating_sub(1),
                Err(error) => {
                    record_capture_fault(
                        app,
                        storage,
                        project_id,
                        recording_id,
                        "live_meeting.transcript_pending",
                        &track.scope.track_id,
                        &error,
                        format!("บันทึก revisioned catch-up ไม่สำเร็จ ({error})"),
                    );
                    remaining += 1;
                    worker.shutdown();
                    return remaining;
                }
            }
        }
        if let Some(window) = track.scheduler.flush(&track.scope.track_id).ok().flatten() {
            let through_sequence = track.next_sequence_no.saturating_sub(1);
            let candidates = match decode_revision_windows(
                app,
                context,
                track,
                &mut worker,
                &model_run_id,
                std::slice::from_ref(&window),
            ) {
                Ok(candidates) => candidates,
                Err(error) => {
                    record_capture_fault(
                        app,
                        storage,
                        project_id,
                        recording_id,
                        "live_meeting.transcript_pending",
                        &track.scope.track_id,
                        &error,
                        format!("revisioned catch-up final window failed ({error})"),
                    );
                    remaining += 1;
                    worker.shutdown();
                    return remaining;
                }
            };
            if let Err(error) = commit_revisioned_sources(
                app,
                storage,
                recent,
                context,
                track,
                candidates,
                &model_run_id,
                through_sequence,
            ) {
                record_capture_fault(
                    app,
                    storage,
                    project_id,
                    recording_id,
                    "live_meeting.transcript_pending",
                    &track.scope.track_id,
                    &error,
                    format!("revisioned catch-up final commit failed ({error})"),
                );
                remaining += 1;
                worker.shutdown();
                return remaining;
            }
            remaining = remaining.saturating_sub(1);
        }
        if !track.pending_sources.is_empty() {
            let through_sequence = track.next_sequence_no.saturating_sub(1);
            if commit_revisioned_sources(
                app,
                storage,
                recent,
                context,
                track,
                Vec::new(),
                &model_run_id,
                through_sequence,
            )
            .is_ok()
            {
                remaining = remaining.saturating_sub(1);
            } else {
                remaining += 1;
            }
        }
    }
    worker.shutdown();
    match revisioned_uncovered_chunks(storage, recording_id, context) {
        Ok(still_missing) => still_missing.values().map(Vec::len).sum::<usize>(),
        Err(_) => remaining.max(1),
    }
}

// ---------------------------------------------------------------------------
// Coordinator: single Genesis writer + live transcription + post-meeting kick
// ---------------------------------------------------------------------------

pub(crate) fn speaker_id_for(project_id: &str, key: &str) -> String {
    format!("{project_id}::speaker::{key}")
}

/// Opens a capture against an **existing** project without touching the
/// project row.
///
/// `genesis_adapter::start_capture` is the mobile capture entry point: it
/// begins by calling `ensure_project_mutations`, which unconditionally upserts
/// the project with the hardcoded name `"FUNG Mobile"`. On desktop the project
/// already exists and is user-named, so routing through that helper silently
/// renames whatever project the meeting is recorded into — the Markdown export
/// header then reports the wrong meeting. Writing the two capture rows here
/// keeps the desktop path off that helper without changing mobile behavior.
pub(crate) fn start_desktop_capture(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    manifest_path: &str,
    timestamp: &str,
    language: Option<&str>,
) -> Result<genesis_adapter::CaptureRecord, String> {
    genesis_adapter::commit_rows(
        storage,
        vec![
            genesis_adapter::upsert(
                "recordings",
                serde_json::json!({
                    "id": recording_id,
                    "project_id": project_id,
                    "source": "microphone",
                    "input_path": null,
                    "canonical_audio_path": manifest_path,
                    "status": "recording",
                    "duration_ms": 0,
                    "created_at": timestamp,
                    "updated_at": timestamp,
                    // Stored, not just handed to the worker: every later pass
                    // over this audio needs the same answer, and until now
                    // the choice died with the session.
                    "language": language,
                }),
            ),
            genesis_adapter::upsert(
                "mobile_recording_checkpoints",
                serde_json::json!({
                    "id": recording_id,
                    "recording_id": recording_id,
                    "safe_offset_ms": 0,
                    "segment_count": 0,
                    "last_checksum": null,
                    "updated_at": timestamp,
                }),
            ),
        ],
    )?;
    genesis_adapter::capture(storage, recording_id)
}

#[allow(clippy::too_many_arguments)]
/// How a finished capture must be reported.
///
/// Separated from the coordinator because this is the truthfulness rule, not
/// plumbing: a session that lost source audio must never be recorded as a
/// completed capture, and a transcript with gaps must say so. Both are easy to
/// get wrong in a way no type checker catches.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CaptureOutcome {
    /// `Some` marks the job failed, carrying the durable reason. `None`
    /// completes it.
    pub(crate) failure_reason: Option<String>,
    pub(crate) message: String,
}

pub(crate) fn capture_outcome(
    duration_ms: i64,
    lost_chunks: usize,
    stream_faults: usize,
    still_pending: usize,
) -> CaptureOutcome {
    let seconds = duration_ms / 1000;
    if lost_chunks > 0 {
        return CaptureOutcome {
            failure_reason: Some(format!(
                "{lost_chunks} audio chunk(s) or media interval(s) were lost; \
                 {stream_faults} audio stream fault(s)"
            )),
            message: format!(
                "บันทึกจบแต่ไม่ครบ — เสียงหาย {lost_chunks} ช่วง ความยาวที่บันทึกได้ {seconds} วินาที"
            ),
        };
    }
    let mut message = format!("บันทึกเสร็จ ความยาว {seconds} วินาที");
    if stream_faults > 0 {
        message.push_str(&format!(" (พบสตรีมเสียงผิดพลาด {stream_faults} ครั้ง)"));
    }
    if still_pending > 0 {
        message.push_str(&format!(" — ยังถอดความไม่ได้ {still_pending} ช่วง"));
    }
    CaptureOutcome {
        failure_reason: None,
        message,
    }
}

/// Turns a capture fault into durable, user-visible state: an audit row that
/// survives restart and a status event the panel renders. Both matter — a
/// toast the user missed is not a record that audio was lost.
#[allow(clippy::too_many_arguments)]
fn record_capture_fault(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
    event_type: &str,
    channel: &str,
    error: &str,
    message: String,
) {
    let timestamp = now();
    let _ = genesis_adapter::commit_rows(
        storage,
        vec![genesis_adapter::upsert(
            "audit_events",
            serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "project_id": project_id,
                "event_type": event_type,
                "actor": "system",
                "payload_json": {
                    "recordingId": recording_id,
                    "channel": channel,
                    "error": error,
                },
                "created_at": timestamp,
            }),
        )],
    );
    emit_status(app, recording_id, "degraded", Some(message), None, None);
}

#[allow(clippy::too_many_arguments)]
fn spawn_coordinator(
    app: tauri::AppHandle,
    storage: Arc<genesis_block_native::Storage>,
    runtime: WhisperRuntime,
    language: Option<String>,
    chunk_rx: mpsc::Receiver<CaptureEvent>,
    recent: SharedRecent,
    project_id: String,
    recording_id: String,
    job_id: String,
    session_dir: PathBuf,
    stop: Arc<AtomicBool>,
    native_capture: Arc<crate::recording_review::NativeCaptureGuard>,
    transcript_profile: crate::live_transcript::LiveTranscriptProfile,
    revisioned_context: Option<RevisionedSessionContext>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let capture_lease = CaptureRuntimeLease {
            app: app.clone(),
            guard: native_capture,
            recording_id: recording_id.clone(),
        };
        let mut capture_record = match genesis_adapter::capture(&storage, &recording_id) {
            Ok(record) => record,
            Err(error) => {
                emit_status(&app, &recording_id, "error", Some(error), None, None);
                return;
            }
        };

        let mut worker = match LiveWorker::spawn(&runtime, language.as_deref()) {
            Ok(mut worker) => match worker.wait_ready() {
                Ok(()) => {
                    emit_status(
                        &app,
                        &recording_id,
                        "listening",
                        Some("โมเดลถอดความพร้อมแล้ว".to_string()),
                        None,
                        None,
                    );
                    Some(worker)
                }
                Err(error) => {
                    emit_status(
                        &app,
                        &recording_id,
                        "degraded",
                        Some(format!(
                            "ถอดสดใช้ไม่ได้ ({error}) — เสียงยังถูกบันทึกครบ และจะถอดหลังจบ"
                        )),
                        None,
                        None,
                    );
                    None
                }
            },
            Err(error) => {
                emit_status(
                    &app,
                    &recording_id,
                    "degraded",
                    Some(format!("เปิดตัวถอดสดไม่สำเร็จ ({error}) — เสียงยังถูกบันทึกครบ")),
                    None,
                    None,
                );
                None
            }
        };

        let mut revisioned_tracks = revisioned_context
            .as_ref()
            .map(|context| {
                context
                    .sources
                    .iter()
                    .map(|(channel, scope)| (channel.clone(), RevisionedTrack::new(scope.clone())))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let mut revisioned_model_run_id = None;
        if transcript_profile == crate::live_transcript::LiveTranscriptProfile::Revisioned {
            if let Some(active_worker) = worker.as_ref() {
                if let Some(runtime_info) = active_worker.runtime_info() {
                    match create_live_model_run(
                        &storage,
                        &recording_id,
                        runtime_info,
                        language.as_deref(),
                    ) {
                        Ok(run_id) => revisioned_model_run_id = Some(run_id),
                        Err(error) => emit_status(
                            &app,
                            &recording_id,
                            "degraded",
                            Some(format!("ไม่สามารถบันทึก provenance ของโมเดลได้ ({error})")),
                            None,
                            None,
                        ),
                    }
                }
            }
            if revisioned_model_run_id.is_none() {
                if let Some(worker) = worker.take() {
                    worker.shutdown();
                }
                emit_status(
                    &app,
                    &recording_id,
                    "degraded",
                    Some(
                        "ถอดสดแบบ revisioned รอ worker ที่มี provenance ครบ; จะลองถอดย้อนหลังหลังจบ"
                            .to_string(),
                    ),
                    None,
                    None,
                );
            }
        }

        // Faults are counted, not just displayed: the totals decide whether
        // this capture may be reported as a clean recording.
        let mut lost_chunks: usize = 0;
        let mut stream_faults: usize = 0;
        let mut chunks_since_disk_check: usize = 0;
        let mut low_disk_warned = false;
        // `append_capture_chunk` stamps the recording's duration with the
        // chunk it just wrote. That is exact for mobile's single channel, but
        // here mic and system keep independent timelines and interleave, so
        // the stored duration ends up being whichever channel happened to
        // write last. Track the real high-water mark and correct the row when
        // the session closes.
        let mut max_end_ms: i64 = 0;

        // Ledger first, transcription second, for every chunk until all
        // channel threads hang up.
        while let Ok(event) = chunk_rx.recv() {
            let chunk = match event {
                CaptureEvent::Chunk(chunk) => chunk,
                CaptureEvent::ChunkWriteFailed {
                    channel,
                    start_ms,
                    end_ms,
                    error,
                } => {
                    lost_chunks += 1;
                    max_end_ms = max_end_ms.max(end_ms);
                    record_capture_fault(
                        &app,
                        &storage,
                        &project_id,
                        &recording_id,
                        "live_meeting.chunk_write_failed",
                        channel,
                        &error,
                        format!("เขียนไฟล์เสียงช่อง {channel} ไม่สำเร็จ ({error}) — ช่วง {start_ms}–{end_ms} ms สูญหาย; บันทึก gap #{lost_chunks}"),
                    );
                    if transcript_profile
                        == crate::live_transcript::LiveTranscriptProfile::Revisioned
                    {
                        record_revisioned_capture_gap(
                            &app,
                            &storage,
                            &recent,
                            &recording_id,
                            revisioned_context.as_ref(),
                            &mut revisioned_model_run_id,
                            &mut worker,
                            &mut revisioned_tracks,
                            channel,
                            start_ms,
                            end_ms,
                            "chunk_write_failed",
                        );
                    }
                    continue;
                }
                CaptureEvent::StreamFailed { channel, error } => {
                    stream_faults += 1;
                    record_capture_fault(
                        &app,
                        &storage,
                        &project_id,
                        &recording_id,
                        "live_meeting.stream_failed",
                        channel,
                        &error,
                        format!("สตรีมเสียงช่อง {channel} ผิดพลาด ({error}) — ตรวจสอบอุปกรณ์เสียง"),
                    );
                    continue;
                }
                CaptureEvent::SourceGap {
                    channel,
                    start_ms,
                    end_ms,
                    reason,
                } => {
                    lost_chunks += 1;
                    max_end_ms = max_end_ms.max(end_ms);
                    record_capture_fault(
                        &app,
                        &storage,
                        &project_id,
                        &recording_id,
                        "live_meeting.capture_source_gap",
                        channel,
                        &format!("missing captured media interval {start_ms}..{end_ms} ms ({reason})"),
                        format!("ช่อง {channel} เสียงช่วง {start_ms}–{end_ms} ms สูญหาย ({reason}); บันทึก gap #{lost_chunks}"),
                    );
                    if transcript_profile
                        == crate::live_transcript::LiveTranscriptProfile::Revisioned
                    {
                        record_revisioned_capture_gap(
                            &app,
                            &storage,
                            &recent,
                            &recording_id,
                            revisioned_context.as_ref(),
                            &mut revisioned_model_run_id,
                            &mut worker,
                            &mut revisioned_tracks,
                            channel,
                            start_ms,
                            end_ms,
                            reason,
                        );
                    }
                    continue;
                }
            };
            let timestamp = now();
            max_end_ms = max_end_ms.max(chunk.end_ms);
            match genesis_adapter::append_capture_chunk(
                &storage,
                &capture_record,
                genesis_adapter::AudioChunk {
                    id: &chunk.chunk_id,
                    file_path: &chunk.file_path,
                    start_ms: chunk.start_ms,
                    end_ms: chunk.end_ms,
                    byte_size: chunk.byte_size,
                    checksum: &chunk.checksum,
                    timestamp: &timestamp,
                },
            ) {
                Ok(updated) => capture_record = updated,
                Err(error) => {
                    // Ledger write failed: keep the file on disk, surface it,
                    // and keep capturing — the WAV itself is not lost.
                    emit_status(
                        &app,
                        &recording_id,
                        "degraded",
                        Some(format!("บันทึก ledger ไม่สำเร็จ: {error}")),
                        None,
                        None,
                    );
                }
            }

            chunks_since_disk_check += 1;
            if chunks_since_disk_check >= DISK_CHECK_EVERY_CHUNKS {
                chunks_since_disk_check = 0;
                if let Some(free) = free_disk_bytes(&session_dir) {
                    if free < MIN_FREE_BYTES_TO_CONTINUE {
                        record_capture_fault(
                            &app,
                            &storage,
                            &project_id,
                            &recording_id,
                            "live_meeting.disk_exhausted",
                            "session",
                            &format!("{free} bytes free"),
                            format!(
                                "พื้นที่ดิสก์เหลือ {} — หยุดบันทึกเพื่อรักษาเสียงที่บันทึกไว้แล้ว",
                                human_gib(free)
                            ),
                        );
                        // Closes the capture threads; every chunk already cut
                        // stays on disk and in the ledger.
                        capture_lease.guard.mark_capture_stopping();
                        stop.store(true, Ordering::SeqCst);
                    } else if free < LOW_DISK_WARN_BYTES && !low_disk_warned {
                        low_disk_warned = true;
                        emit_status(
                            &app,
                            &recording_id,
                            "degraded",
                            Some(format!(
                                "พื้นที่ดิสก์เหลือน้อย {} — บันทึกต่อได้อีกไม่นาน",
                                human_gib(free)
                            )),
                            None,
                            None,
                        );
                    }
                }
            }

            if transcript_profile == crate::live_transcript::LiveTranscriptProfile::Revisioned {
                let Some(context) = revisioned_context.as_ref() else {
                    continue;
                };
                let Some(track) = revisioned_tracks.get_mut(chunk.channel) else {
                    emit_status(
                        &app,
                        &recording_id,
                        "degraded",
                        Some(format!("ไม่มี source scope สำหรับช่อง {}", chunk.channel)),
                        None,
                        None,
                    );
                    continue;
                };
                let batch = match track.add_chunk(chunk) {
                    Ok(batch) => batch,
                    Err(error) => {
                        emit_status(
                            &app,
                            &recording_id,
                            "degraded",
                            Some(format!("สร้างหน้าต่างถอดความไม่ได้: {error:?}")),
                            None,
                            None,
                        );
                        continue;
                    }
                };
                if batch.gap.is_some() {
                    for update in track.tracker.discard_uncommitted() {
                        if let crate::live_transcript::TranscriptUpdate::Discard {
                            utterance_id,
                            revision,
                        } = update
                        {
                            emit_transcript_event(
                                &app,
                                context,
                                &track.scope,
                                "transcript.discarded",
                                "discarded",
                                Some(utterance_id),
                                Some(revision),
                                None,
                                Vec::new(),
                                revisioned_model_run_id.as_deref(),
                                None,
                                None,
                            );
                        }
                    }
                }
                let (Some(active_worker), Some(model_run_id)) =
                    (worker.as_mut(), revisioned_model_run_id.as_deref())
                else {
                    continue;
                };
                if batch.windows.is_empty() {
                    continue;
                }
                let through_sequence = track.next_sequence_no.saturating_sub(1);
                let candidates = match decode_revision_windows(
                    &app,
                    context,
                    track,
                    active_worker,
                    model_run_id,
                    &batch.windows,
                ) {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        for update in track.tracker.discard_uncommitted() {
                            if let crate::live_transcript::TranscriptUpdate::Discard {
                                utterance_id,
                                revision,
                            } = update
                            {
                                emit_transcript_event(
                                    &app,
                                    context,
                                    &track.scope,
                                    "transcript.discarded",
                                    "discarded",
                                    Some(utterance_id),
                                    Some(revision),
                                    None,
                                    Vec::new(),
                                    Some(model_run_id),
                                    None,
                                    None,
                                );
                            }
                        }
                        emit_status(
                            &app,
                            &recording_id,
                            "degraded",
                            Some(format!(
                                "ตัวถอด revisioned หยุดทำงาน ({error}) — เก็บเสียงไว้ถอดย้อนหลัง"
                            )),
                            None,
                            None,
                        );
                        if let Some(dead) = worker.take() {
                            dead.shutdown();
                        }
                        revisioned_model_run_id = None;
                        continue;
                    }
                };
                if let Err(error) = commit_revisioned_sources(
                    &app,
                    &storage,
                    &recent,
                    context,
                    track,
                    candidates,
                    model_run_id,
                    through_sequence,
                ) {
                    emit_status(
                        &app,
                        &recording_id,
                        "degraded",
                        Some(format!(
                            "บันทึก transcript revision ไม่สำเร็จ ({error}) — จะ reconcile หลังจบ"
                        )),
                        None,
                        None,
                    );
                    if let Some(dead) = worker.take() {
                        dead.shutdown();
                    }
                    revisioned_model_run_id = None;
                }
                continue;
            }

            let Some(active_worker) = worker.as_mut() else {
                continue;
            };
            match active_worker.transcribe_chunk(&chunk) {
                Ok(response) => {
                    if let Some(error) = response.error {
                        eprintln!("[live-worker] chunk {} failed: {error}", chunk.chunk_id);
                        continue;
                    }
                    persist_and_emit_segments(
                        &app,
                        &storage,
                        &recent,
                        &project_id,
                        &recording_id,
                        &chunk,
                        response,
                    );
                }
                Err(error) => {
                    emit_status(
                        &app,
                        &recording_id,
                        "degraded",
                        Some(format!("ตัวถอดสดหยุดทำงาน ({error}) — เสียงยังถูกบันทึกต่อ")),
                        None,
                        None,
                    );
                    if let Some(dead) = worker.take() {
                        dead.shutdown();
                    }
                }
            }
        }

        if transcript_profile == crate::live_transcript::LiveTranscriptProfile::Revisioned {
            let mut flush_failed = false;
            if let (Some(context), Some(model_run_id), Some(active_worker)) = (
                revisioned_context.as_ref(),
                revisioned_model_run_id.as_deref(),
                worker.as_mut(),
            ) {
                for track in revisioned_tracks.values_mut() {
                    let final_window = match track.scheduler.flush(&track.scope.track_id) {
                        Ok(window) => window,
                        Err(error) => {
                            emit_status(
                                &app,
                                &recording_id,
                                "degraded",
                                Some(format!("ปิดหน้าต่างเสียงสุดท้ายไม่ได้: {error:?}")),
                                None,
                                None,
                            );
                            flush_failed = true;
                            break;
                        }
                    };
                    if let Some(window) = final_window {
                        let through_sequence = track.next_sequence_no.saturating_sub(1);
                        match decode_revision_windows(
                            &app,
                            context,
                            track,
                            active_worker,
                            model_run_id,
                            std::slice::from_ref(&window),
                        ) {
                            Ok(candidates) => {
                                if let Err(error) = commit_revisioned_sources(
                                    &app,
                                    &storage,
                                    &recent,
                                    context,
                                    track,
                                    candidates,
                                    model_run_id,
                                    through_sequence,
                                ) {
                                    emit_status(
                                        &app,
                                        &recording_id,
                                        "degraded",
                                        Some(format!("บันทึกหน้าต่างสุดท้ายไม่สำเร็จ ({error})")),
                                        None,
                                        None,
                                    );
                                    flush_failed = true;
                                    break;
                                }
                            }
                            Err(error) => {
                                emit_status(
                                    &app,
                                    &recording_id,
                                    "degraded",
                                    Some(format!("ถอดหน้าต่างสุดท้ายไม่ได้ ({error})")),
                                    None,
                                    None,
                                );
                                flush_failed = true;
                                break;
                            }
                        }
                    }
                    if !track.pending_sources.is_empty() {
                        let through_sequence = track.next_sequence_no.saturating_sub(1);
                        if let Err(error) = commit_revisioned_sources(
                            &app,
                            &storage,
                            &recent,
                            context,
                            track,
                            Vec::new(),
                            model_run_id,
                            through_sequence,
                        ) {
                            emit_status(
                                &app,
                                &recording_id,
                                "degraded",
                                Some(format!("บันทึก source tail ไม่สำเร็จ ({error})")),
                                None,
                                None,
                            );
                            flush_failed = true;
                            break;
                        }
                    }
                }
            } else {
                flush_failed = true;
            }
            if flush_failed {
                if let Some(active_worker) = worker.take() {
                    active_worker.shutdown();
                }
            }
        }
        if let Some(active_worker) = worker.take() {
            active_worker.shutdown();
        }

        capture_lease.guard.mark_capture_stopping();
        let finished_at = now();
        capture_record.duration_ms = capture_record.duration_ms.max(max_end_ms);
        if let Err(error) = genesis_adapter::finish_capture(&storage, &capture_record, &finished_at)
        {
            emit_status(&app, &recording_id, "error", Some(error), None, None);
        }

        // Catch-up transcription. The degraded-mode message tells the user the
        // audio "will be transcribed after it ends"; until this existed, the
        // chunks were collected into a vector and dropped, so the app stated
        // something it never did. Runs before the summary so recovered text is
        // part of it, not missing from it.
        let still_pending =
            if transcript_profile == crate::live_transcript::LiveTranscriptProfile::Revisioned {
                revisioned_context.as_ref().map_or(1, |context| {
                    transcribe_revisioned_pending(
                        &app,
                        &storage,
                        &recent,
                        &runtime,
                        language.as_deref(),
                        &recording_id,
                        context,
                        &mut revisioned_tracks,
                    )
                })
            } else {
                match chunks_missing_transcript(&storage, &project_id, &recording_id) {
                    Ok(pending) if pending.is_empty() => 0,
                    Ok(pending) => transcribe_pending_chunks(
                        &app,
                        &storage,
                        &recent,
                        &runtime,
                        language.as_deref(),
                        &project_id,
                        &recording_id,
                        &pending,
                    ),
                    Err(error) => {
                        record_capture_fault(
                            &app,
                            &storage,
                            &project_id,
                            &recording_id,
                            "live_meeting.transcript_pending_lookup_failed",
                            "session",
                            &error,
                            format!("ตรวจรายการเสียงที่ยังไม่ได้ถอดความไม่ได้ ({error})"),
                        );
                        1
                    }
                }
            };

        // A capture that lost source audio is not a completed capture. The job
        // row is the durable record, so it must say so even if nobody was
        // watching the status events.
        let outcome = capture_outcome(
            capture_record.duration_ms,
            lost_chunks,
            stream_faults,
            still_pending,
        );
        match &outcome.failure_reason {
            Some(reason) => {
                let _ = crate::set_job_status(&storage, &job_id, "failed", None, Some(reason));
            }
            None => {
                let _ = crate::set_job_status(&storage, &job_id, "completed", Some(100), None);
            }
        }
        if let Some(context) = revisioned_context.as_ref() {
            update_revisioned_session_state(
                &storage,
                context,
                if still_pending > 0 || lost_chunks > 0 {
                    "degraded"
                } else {
                    "completed"
                },
            );
        }
        emit_status(
            &app,
            &recording_id,
            "stopped",
            Some(outcome.message),
            None,
            None,
        );

        // Post-meeting pipeline: speaker pass → summary → export. Queued rather than run
        // here, so a meeting that ends while the local model is down keeps
        // its summary as pending work instead of losing it to a thread that
        // dies with the process.
        if let Some(state) = app.try_state::<AppState>() {
            meeting_intel::queue_post_meeting(&app, &state.jobs, &project_id, &recording_id, true);
        }

        // Release the in-memory session slot last, so `live_meeting_status`
        // keeps answering "stopping" while the tail work runs.
        if let Some(state) = app.try_state::<AppState>() {
            let mut live = state.live.lock().expect("live session mutex poisoned");
            *live = None;
        }
    })
}

/// Maps a chunk filename back to the capture channel that wrote it.
///
/// The channel decides which speaker a recovered segment is attributed to, so
/// an unrecognised name yields `None` rather than a guess — mislabelling who
/// spoke is worse than leaving a chunk untranscribed.
pub(crate) fn channel_for_file_name(name: &str) -> Option<&'static str> {
    match crate::recovery::parse_chunk_file_name(name)?.0.as_str() {
        CHANNEL_MIC => Some(CHANNEL_MIC),
        CHANNEL_SYSTEM => Some(CHANNEL_SYSTEM),
        _ => None,
    }
}

/// What filling a recording's transcript gaps achieved.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GapFillOutcome {
    /// Chunks with no transcript covering them when the pass started.
    pub(crate) chunks_missing_transcript: usize,
    pub(crate) chunks_transcribed: usize,
    pub(crate) still_missing: usize,
    /// Set when the pass declined to run. Reported rather than silently
    /// treated as "nothing to do".
    pub(crate) skipped_reason: Option<String>,
}

/// Finds chunks of a recording that no transcript segment covers.
///
/// A chunk counts as covered when a segment attributed to *its channel's*
/// speaker starts inside its time range — which is exactly what
/// `persist_and_emit_segments` writes. Checking the speaker as well as the
/// time matters: the two channels share a timeline, so a microphone chunk
/// would otherwise look covered by system-audio text.
fn chunks_missing_transcript(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<Vec<RawChunk>, String> {
    let segments = genesis_adapter::query_all(
        storage,
        "transcript_segments",
        &["speaker_id", "start_ms"],
        vec![genesis_adapter::eq(
            "transcript_segments",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    let covered: Vec<(String, i64)> = segments
        .iter()
        .map(|row| {
            (
                row.get("transcript_segments.speaker_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                row.get("transcript_segments.start_ms")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
            )
        })
        .collect();

    let chunks = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &[
            "id",
            "file_path",
            "start_ms",
            "end_ms",
            "byte_size",
            "checksum",
            "transcribed_at",
        ],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;

    let mut missing = Vec::new();
    for row in &chunks {
        let text = |key: &str| {
            row.get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let integer = |key: &str| {
            row.get(key)
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0)
        };
        let file_path = text("audio_chunks.file_path");
        let name = std::path::Path::new(&file_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(channel) = channel_for_file_name(&name) else {
            continue;
        };
        // Only chunks whose audio is actually present can be transcribed.
        if !std::path::Path::new(&file_path).is_file() {
            continue;
        }
        // The transcriber has already seen this chunk. Silence is a real
        // answer, and re-offering it every pass made a quiet meeting look
        // permanently unfinished.
        if genesis_adapter::optional_string(row, "audio_chunks.transcribed_at").is_some() {
            continue;
        }
        let speaker_key = if channel == CHANNEL_MIC { "me" } else { "them" };
        let speaker_id = speaker_id_for(project_id, speaker_key);
        let start_ms = integer("audio_chunks.start_ms");
        let end_ms = integer("audio_chunks.end_ms");
        // Rows written before `transcribed_at` existed carry NULL, so a chunk
        // that does have text is still recognised by its segments. Only a
        // pre-existing silent chunk is offered once more, and this pass
        // stamps it.
        let already = covered
            .iter()
            .any(|(id, at)| *id == speaker_id && *at >= start_ms && *at < end_ms.max(start_ms + 1));
        if already {
            continue;
        }
        missing.push(RawChunk {
            channel,
            chunk_id: text("audio_chunks.id"),
            file_path,
            start_ms,
            end_ms,
            byte_size: integer("audio_chunks.byte_size"),
            checksum: text("audio_chunks.checksum"),
        });
    }
    missing.sort_by_key(|chunk| (chunk.start_ms, chunk.channel));
    Ok(missing)
}

/// Transcribes whatever text a recording is still missing.
///
/// Recovery adopts orphaned audio back into the ledger, but adoption alone
/// leaves a recovered recording showing chunks with no words — the audio is
/// safe and unreadable at the same time. This is the same catch-up pass a
/// degraded live session runs at its end, aimed at a recording that already
/// finished.
pub(crate) fn fill_transcript_gaps(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    runtime: &WhisperRuntime,
    project_id: &str,
    recording_id: &str,
) -> GapFillOutcome {
    let missing = match chunks_missing_transcript(storage, project_id, recording_id) {
        Ok(missing) => missing,
        Err(reason) => {
            return GapFillOutcome {
                skipped_reason: Some(reason),
                ..GapFillOutcome::default()
            }
        }
    };
    if missing.is_empty() {
        return GapFillOutcome::default();
    }

    let total = missing.len();
    let recent: SharedRecent = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    // Read from the ledger rather than taken as an argument: the callers of
    // this pass — recovery, the job engine — are not the session that chose
    // the language, and passing `None` from them meant a recovered Thai
    // meeting was re-transcribed with per-chunk detection.
    let language = genesis_adapter::recording_language(storage, recording_id);
    let still = transcribe_pending_chunks(
        app,
        storage,
        &recent,
        runtime,
        language.as_deref(),
        project_id,
        recording_id,
        &missing,
    );
    GapFillOutcome {
        chunks_missing_transcript: total,
        chunks_transcribed: total - still,
        still_missing: still,
        skipped_reason: None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DetailedDraftOutcome {
    pub(crate) proposals_created: usize,
    pub(crate) unmatched_candidate_segments: usize,
}

pub(crate) fn detailed_proposals_pending(
    storage: &genesis_block_native::Storage,
    project_id: &str,
    recording_id: &str,
) -> Result<bool, String> {
    let segment_ids = genesis_adapter::query_all(
        storage,
        "transcript_segments",
        &["id"],
        vec![
            genesis_adapter::eq(
                "transcript_segments",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_segments",
                "recording_id",
                serde_json::json!(recording_id),
            ),
        ],
    )?
    .into_iter()
    .filter_map(|row| {
        row.get("transcript_segments.id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    })
    .collect::<std::collections::HashSet<_>>();
    if segment_ids.is_empty() {
        return Ok(false);
    }
    let proposals = genesis_adapter::query_all(
        storage,
        "transcript_refinement_proposals",
        &["transcript_segment_id", "policy", "status"],
        vec![
            genesis_adapter::eq(
                "transcript_refinement_proposals",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_refinement_proposals",
                "status",
                serde_json::json!("proposed"),
            ),
        ],
    )?;
    Ok(proposals.iter().any(|row| {
        let segment_id = row
            .get("transcript_refinement_proposals.transcript_segment_id")
            .and_then(serde_json::Value::as_str);
        let policy = row
            .get("transcript_refinement_proposals.policy")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
        segment_id.is_some_and(|id| segment_ids.contains(id))
            && policy
                .as_ref()
                .and_then(|value| value.get("kind"))
                .and_then(serde_json::Value::as_str)
                == Some("detailed_asr_candidate")
    }))
}

#[derive(Debug)]
struct DetailedSourceChunk {
    path: String,
    start_ms: i64,
    channel: Option<&'static str>,
}

#[derive(Debug)]
struct DetailedSourceSegment {
    id: String,
    speaker_id: Option<String>,
    start_ms: i64,
    end_ms: i64,
    text: String,
    updated_at: String,
    expected_revision: Option<i64>,
}

fn unique_detailed_segment_match<'a>(
    segments: &'a [DetailedSourceSegment],
    start_ms: i64,
    end_ms: i64,
    expected_speaker: Option<&str>,
) -> Option<&'a DetailedSourceSegment> {
    let overlapping = segments
        .iter()
        .filter(|segment| end_ms.min(segment.end_ms) - start_ms.max(segment.start_ms) > 0)
        .collect::<Vec<_>>();
    if let Some(speaker) = expected_speaker {
        let exact = overlapping
            .iter()
            .copied()
            .filter(|segment| segment.speaker_id.as_deref() == Some(speaker))
            .collect::<Vec<_>>();
        if exact.len() == 1 {
            return exact.first().copied();
        }
        if exact.is_empty() && overlapping.len() == 1 && overlapping[0].speaker_id.is_none() {
            // Revisioned transcripts may retain source attribution outside
            // the legacy speaker column. Accept only a sole time match;
            // overlapping speech remains unmatched.
            return overlapping.first().copied();
        }
        return None;
    }
    (overlapping.len() == 1).then(|| overlapping[0])
}

fn join_detailed_candidate_texts(candidates: impl IntoIterator<Item = String>) -> String {
    let mut joined = String::new();
    for candidate in candidates {
        let candidate = candidate.trim();
        let first = candidate.chars().next();
        if candidate.is_empty() {
            continue;
        }
        let previous = joined.chars().last();
        let both_are_word_characters = previous
            .is_some_and(|character| character.is_alphanumeric())
            && first.is_some_and(|character| character.is_alphanumeric());
        let boundary_contains_ascii = previous.is_some_and(|character| character.is_ascii())
            || first.is_some_and(|character| character.is_ascii());
        if both_are_word_characters && boundary_contains_ascii {
            joined.push(' ');
        }
        joined.push_str(candidate);
    }
    joined
}

/// Re-runs all locally custodied audio through the pinned Thai model and
/// records only transcript differences as reviewable proposals. This path
/// intentionally never writes `transcript_segments` or the transcript
/// projection; acceptance happens through the desktop review command.
pub(crate) fn create_detailed_transcript_draft(
    storage: &genesis_block_native::Storage,
    runtime: &WhisperRuntime,
    project_id: &str,
    recording_id: &str,
    job_id: &str,
) -> Result<DetailedDraftOutcome, String> {
    if detailed_proposals_pending(storage, project_id, recording_id)? {
        return Err("รีวิวหรือปฏิเสธข้อเสนอจากโหมดละเอียดที่ค้างอยู่ก่อนเริ่มรอบใหม่".to_string());
    }
    let readiness = crate::detailed_transcription_readiness_for_job(runtime);
    if !readiness.available {
        return Err(readiness.reason);
    }

    let source_rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &["file_path", "start_ms"],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            serde_json::json!(recording_id),
        )],
    )?;
    if source_rows.is_empty() {
        return Err("การบันทึกนี้ไม่มีช่วงเสียงที่เก็บไว้ในเครื่องสำหรับถอดละเอียด".to_string());
    }
    let mut groups: std::collections::BTreeMap<String, Vec<DetailedSourceChunk>> =
        std::collections::BTreeMap::new();
    for (index, row) in source_rows.iter().enumerate() {
        let path = genesis_adapter::string(row, "audio_chunks.file_path")?;
        if !std::path::Path::new(&path).is_file() {
            return Err(format!(
                "ไม่พบไฟล์เสียงในคลังของ FUNG: {}",
                std::path::Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        }
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let channel = channel_for_file_name(file_name);
        let group_key = channel
            .map(str::to_string)
            .unwrap_or_else(|| format!("unknown-{index}"));
        groups
            .entry(group_key)
            .or_default()
            .push(DetailedSourceChunk {
                path,
                start_ms: genesis_adapter::integer(row, "audio_chunks.start_ms")?,
                channel,
            });
    }

    let projections = genesis_adapter::query_all(
        storage,
        "transcript_projection",
        &["utterance_id", "revision"],
        vec![
            genesis_adapter::eq(
                "transcript_projection",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_projection",
                "recording_id",
                serde_json::json!(recording_id),
            ),
        ],
    )?
    .into_iter()
    .filter_map(|row| {
        Some((
            row.get("transcript_projection.utterance_id")?
                .as_str()?
                .to_string(),
            row.get("transcript_projection.revision")?.as_i64()?,
        ))
    })
    .collect::<std::collections::HashMap<_, _>>();
    let segments = genesis_adapter::query_all(
        storage,
        "transcript_segments",
        &[
            "id",
            "speaker_id",
            "start_ms",
            "end_ms",
            "text",
            "updated_at",
        ],
        vec![
            genesis_adapter::eq(
                "transcript_segments",
                "project_id",
                serde_json::json!(project_id),
            ),
            genesis_adapter::eq(
                "transcript_segments",
                "recording_id",
                serde_json::json!(recording_id),
            ),
        ],
    )?
    .into_iter()
    .map(|row| {
        let id = genesis_adapter::string(&row, "transcript_segments.id")?;
        Ok(DetailedSourceSegment {
            expected_revision: projections.get(&id).copied(),
            id,
            speaker_id: genesis_adapter::optional_string(&row, "transcript_segments.speaker_id"),
            start_ms: genesis_adapter::integer(&row, "transcript_segments.start_ms")?,
            end_ms: genesis_adapter::integer(&row, "transcript_segments.end_ms")?,
            text: genesis_adapter::string(&row, "transcript_segments.text")?,
            updated_at: genesis_adapter::string(&row, "transcript_segments.updated_at")?,
        })
    })
    .collect::<Result<Vec<_>, String>>()?;
    if segments.is_empty() {
        return Err(
            "ยังไม่มี transcript เดิมให้เปรียบเทียบ; โหมดละเอียดทำงานได้หลังถอดโหมดทั่วไปแล้ว".to_string(),
        );
    }

    let language = genesis_adapter::recording_language(storage, recording_id);
    let mut candidate_by_segment: std::collections::HashMap<String, Vec<(i64, String)>> =
        std::collections::HashMap::new();
    let mut unmatched_candidate_segments = 0usize;
    for (_channel_key, mut chunks) in groups {
        chunks.sort_by_key(|chunk| chunk.start_ms);
        let specs = chunks
            .iter()
            .map(|chunk| {
                serde_json::json!({
                    "path": chunk.path,
                    "startMs": chunk.start_ms.max(0),
                })
            })
            .collect::<Vec<_>>();
        let manifest_path =
            std::env::temp_dir().join(format!("fung-detailed-transcript-{}.json", Uuid::new_v4()));
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&specs).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("สร้างรายการเสียงชั่วคราวไม่สำเร็จ: {error}"))?;
        let result =
            crate::run_detailed_candidate_worker(runtime, &manifest_path, language.as_deref());
        let _ = std::fs::remove_file(&manifest_path);
        let output = result?;

        let expected_speaker = chunks.first().and_then(|chunk| {
            chunk.channel.map(|channel| {
                let key = if channel == CHANNEL_MIC { "me" } else { "them" };
                speaker_id_for(project_id, key)
            })
        });
        for candidate in output.segments {
            let start_ms = candidate.start_ms.max(0);
            let end_ms = candidate.end_ms.max(start_ms + 1);
            if let Some(segment) = unique_detailed_segment_match(
                &segments,
                start_ms,
                end_ms,
                expected_speaker.as_deref(),
            ) {
                candidate_by_segment
                    .entry(segment.id.clone())
                    .or_default()
                    .push((start_ms, candidate.text));
            } else {
                unmatched_candidate_segments += 1;
            }
        }
    }

    let mut mutations = Vec::new();
    for segment in &segments {
        let Some(mut candidates) = candidate_by_segment.remove(&segment.id) else {
            continue;
        };
        candidates.sort_by_key(|(start_ms, _)| *start_ms);
        let proposed_text =
            join_detailed_candidate_texts(candidates.into_iter().map(|(_, text)| text));
        if proposed_text.is_empty() || proposed_text == segment.text.trim() {
            continue;
        }
        let policy = serde_json::json!({
            "kind": "detailed_asr_candidate",
            "jobId": job_id,
            "backend": "transformers",
            "model": crate::THAI_CANDIDATE_MODEL,
            "revision": crate::THAI_CANDIDATE_MODEL_REVISION,
            "expectedUpdatedAt": segment.updated_at,
            "expectedRevision": segment.expected_revision,
        })
        .to_string();
        let timestamp = now();
        mutations.push(genesis_adapter::upsert(
            "transcript_refinement_proposals",
            serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "project_id": project_id,
                "transcript_segment_id": segment.id,
                "original_text": segment.text,
                "proposed_text": proposed_text,
                "policy": policy,
                "model_run_id": null,
                "status": "proposed",
                "reviewed_at": null,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ));
    }
    let proposals_created = mutations.len();
    if !mutations.is_empty() {
        genesis_adapter::commit_rows(storage, mutations)?;
    }
    Ok(DetailedDraftOutcome {
        proposals_created,
        unmatched_candidate_segments,
    })
}

// Every argument is a distinct collaborator the pass genuinely needs; a
// context struct here would just move the same list behind one name.
#[allow(clippy::too_many_arguments)]
fn transcribe_pending_chunks(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    runtime: &WhisperRuntime,
    language: Option<&str>,
    project_id: &str,
    recording_id: &str,
    pending: &[RawChunk],
) -> usize {
    let total = pending.len();
    emit_status(
        app,
        recording_id,
        "transcribing",
        Some(format!("กำลังถอดความย้อนหลัง {total} ช่วงที่ค้างไว้")),
        None,
        None,
    );

    let worker = LiveWorker::spawn(runtime, language).and_then(|mut worker| {
        worker.wait_ready()?;
        Ok(worker)
    });
    let mut worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            record_capture_fault(
                app,
                storage,
                project_id,
                recording_id,
                "live_meeting.transcript_pending",
                "session",
                &error,
                format!(
                    "ถอดความย้อนหลังไม่ได้ ({error}) — เสียง {total} ช่วงยังอยู่ครบในเครื่อง แต่ยังไม่มีข้อความ"
                ),
            );
            return total;
        }
    };

    let mut recovered = 0usize;
    for chunk in pending {
        match worker.transcribe_chunk(chunk) {
            Ok(response) if response.error.is_none() => {
                persist_and_emit_segments(
                    app,
                    storage,
                    recent,
                    project_id,
                    recording_id,
                    chunk,
                    response,
                );
                recovered += 1;
            }
            // A chunk the worker rejects stays counted as pending; the audio
            // is still on disk and in the ledger.
            Ok(_) => {}
            Err(error) => {
                record_capture_fault(
                    app,
                    storage,
                    project_id,
                    recording_id,
                    "live_meeting.transcript_pending",
                    chunk.channel,
                    &error,
                    format!("ตัวถอดความหยุดทำงานระหว่างถอดย้อนหลัง ({error})"),
                );
                break;
            }
        }
    }
    worker.shutdown();
    total - recovered
}

/// Writes a chunk's transcript and marks the chunk as transcribed.
///
/// The stamp goes on whether or not the worker returned any text. A chunk of
/// silence produces no segments, and without the stamp it is indistinguishable
/// from one that was never transcribed — which is why every catch-up pass
/// used to offer the same silent chunks again.
fn persist_and_emit_segments(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    project_id: &str,
    recording_id: &str,
    chunk: &RawChunk,
    response: WorkerResponse,
) {
    let speaker_key = if chunk.channel == CHANNEL_MIC {
        "me"
    } else {
        "them"
    };
    let speaker_label = if speaker_key == "me" {
        "เรา"
    } else {
        "อีกฝ่าย"
    };
    let speaker_id = speaker_id_for(project_id, speaker_key);

    let mut mutations = Vec::new();
    let mut events = Vec::new();
    for segment in &response.segments {
        let segment_id = Uuid::new_v4().to_string();
        let timestamp = now();
        let start_ms = chunk.start_ms + segment.start_ms;
        let end_ms = chunk.start_ms + segment.end_ms;
        mutations.push(genesis_adapter::upsert(
            "transcript_segments",
            serde_json::json!({
                "id": segment_id,
                "project_id": project_id,
                "recording_id": recording_id,
                "speaker_id": speaker_id,
                "start_ms": start_ms,
                "end_ms": end_ms,
                "text": segment.text,
                "confidence": segment.confidence,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ));
        events.push(LiveSegmentEvent {
            recording_id: recording_id.to_string(),
            segment_id,
            channel: chunk.channel.to_string(),
            speaker: speaker_label.to_string(),
            start_ms,
            end_ms,
            text: segment.text.clone(),
            confidence: segment.confidence,
        });
    }
    // Stamping is what stops a silent chunk being offered again forever, and
    // it must not run ahead of the text: a chunk marked transcribed whose
    // segments failed to commit would lose its words with no trace and no
    // second attempt.
    let stamp = |storage: &genesis_block_native::Storage| {
        if let Err(error) =
            genesis_adapter::mark_chunk_transcribed(storage, &chunk.chunk_id, &now())
        {
            // The transcript is already safe; failing to record that fact
            // only costs a repeated pass, so it is reported rather than
            // treated as a transcription failure.
            eprintln!(
                "[live] could not mark chunk {} transcribed: {error}",
                chunk.chunk_id
            );
        }
    };

    if mutations.is_empty() {
        // Silence is an answer. Before this the chunk looked identical to one
        // that had never been through the transcriber.
        stamp(storage);
        return;
    }
    if let Err(error) = genesis_adapter::commit_rows(storage, mutations) {
        eprintln!("[live] transcript segment commit failed: {error}");
        return;
    }
    stamp(storage);
    {
        let mut window = recent.lock().expect("recent buffer mutex poisoned");
        for event in &events {
            window.push_back(RecentSegment {
                speaker: event.speaker.clone(),
                channel: event.channel.clone(),
                start_ms: event.start_ms,
                end_ms: event.end_ms,
                text: event.text.clone(),
            });
        }
        while window.len() > RECENT_SEGMENT_CAP {
            window.pop_front();
        }
    }
    for event in events {
        let _ = app.emit("live-segment", event);
    }
    let _ = response.start_ms; // chunk offset already applied from the Rust side
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveStartOutput {
    project_id: String,
    recording_id: String,
    job_id: String,
    mic_device: String,
    system_device: Option<String>,
    transcript_profile: String,
    warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveStatusOutput {
    active: bool,
    stopping: bool,
    project_id: Option<String>,
    recording_id: Option<String>,
    elapsed_ms: Option<u64>,
}

/// Marks any capture left `recording` by a previous crash as finished, and
/// fails its stale `recording.capture` jobs. Chunks already written remain on
/// disk and in the ledger — nothing durable is discarded.
fn recover_stale_capture(
    storage: &genesis_block_native::Storage,
    project_id: &str,
) -> Result<(), String> {
    if let Some(stale) = genesis_adapter::active_capture(storage, project_id)? {
        // Previously this just called `finish_capture`, which marked an
        // interrupted session `completed` and discarded any audio written
        // after the last committed chunk. Recover it properly instead: the
        // orphaned chunks are adopted with digests, and the interruption is
        // recorded rather than erased.
        crate::recovery::recover_recording(storage, &stale.recording_id)?;
    }
    let jobs = genesis_adapter::query(
        storage,
        "jobs",
        &["id", "type", "status"],
        vec![genesis_adapter::eq(
            "jobs",
            "project_id",
            serde_json::json!(project_id),
        )],
        1000,
    )?;
    for row in jobs {
        let job_type = row
            .get("jobs.type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let status = row
            .get("jobs.status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if job_type == "recording.capture"
            && matches!(status, "queued" | "running" | "paused" | "retrying")
        {
            if let Some(id) = row.get("jobs.id").and_then(serde_json::Value::as_str) {
                let _ = crate::set_job_status(
                    storage,
                    id,
                    "failed",
                    None,
                    Some("interrupted: desktop session was not shut down cleanly"),
                );
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn live_capture_devices(state: State<'_, AppState>) -> AppResult<LiveCaptureDevices> {
    let mut issues = Vec::new();
    let preferences = match read_capture_preferences(&state.data_root) {
        Ok(preferences) => preferences,
        Err(error) => {
            issues.push(error);
            CaptureDevicePreferences::default()
        }
    };

    let inputs = match enumerate_capture_devices(CaptureDeviceKind::Mic) {
        Ok(devices) => devices
            .into_iter()
            .map(|device| device.descriptor)
            .collect(),
        Err(error) => {
            issues.push(error);
            Vec::new()
        }
    };
    let loopback_outputs = match enumerate_capture_devices(CaptureDeviceKind::SystemLoopback) {
        Ok(devices) => devices
            .into_iter()
            .map(|device| device.descriptor)
            .collect(),
        Err(error) => {
            issues.push(error);
            Vec::new()
        }
    };

    Ok(LiveCaptureDevices {
        inputs,
        loopback_outputs,
        selected_mic_device_id: preferences.mic_device_id,
        selected_system_device_id: preferences.system_device_id,
        issue: if issues.is_empty() {
            None
        } else {
            Some(issues.join("; "))
        },
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) fn live_meeting_start(
    project_id: Option<String>,
    capture_system: Option<bool>,
    language: Option<String>,
    transcript_profile: Option<String>,
    mic_device_id: Option<String>,
    system_device_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<LiveStartOutput> {
    let capture_system = capture_system.unwrap_or(true);
    let transcript_profile =
        crate::live_transcript::LiveTranscriptProfile::parse(transcript_profile.as_deref())
            .map_err(AppError::InvalidInput)?;
    let mic_device_id = normalize_device_id(mic_device_id);
    let system_device_id = normalize_device_id(system_device_id);

    validate_requested_device(CaptureDeviceKind::Mic, mic_device_id.as_deref())
        .map_err(AppError::InvalidInput)?;
    if capture_system {
        validate_requested_device(
            CaptureDeviceKind::SystemLoopback,
            system_device_id.as_deref(),
        )
        .map_err(AppError::InvalidInput)?;
    }
    write_capture_preferences(
        &state.data_root,
        &CaptureDevicePreferences {
            mic_device_id: mic_device_id.clone(),
            system_device_id: system_device_id.clone(),
        },
    )
    .map_err(AppError::InvalidInput)?;

    let capture_reservation = match crate::recording_review::NativeCaptureGuard::reserve_capture(
        Arc::clone(&state.native_capture),
    ) {
        Ok(reservation) => reservation,
        Err(crate::recording_review::AdmissionError::PlaybackBusy) => {
            return Err(AppError::InvalidInput(
                "ปิดการเล่นเสียงก่อนเริ่มบันทึกประชุม".to_string(),
            ));
        }
        Err(crate::recording_review::AdmissionError::CaptureActive) => {
            return Err(AppError::InvalidInput(
                "มีเซสชันประชุมสดทำงานอยู่แล้ว — หยุดเซสชันเดิมก่อน".to_string(),
            ));
        }
    };

    {
        let live = state.live.lock().expect("live session mutex poisoned");
        if live.is_some() {
            return Err(AppError::InvalidInput(
                "มีเซสชันประชุมสดทำงานอยู่แล้ว — หยุดเซสชันเดิมก่อน".to_string(),
            ));
        }
    }

    // Resolve or create the project. Only the id travels onward: the capture
    // rows reference the project, they never rewrite it.
    let project_id = match project_id {
        Some(id) => {
            let rows = genesis_adapter::query(
                &state.genesis,
                "projects",
                &["id"],
                vec![genesis_adapter::eq("projects", "id", serde_json::json!(id))],
                1,
            )
            .map_err(AppError::Genesis)?;
            let row = rows
                .first()
                .ok_or_else(|| AppError::InvalidInput(format!("ไม่พบโปรเจกต์ {id}")))?;
            genesis_adapter::string(row, "projects.id").map_err(AppError::Genesis)?
        }
        None => {
            let output_root = state
                .recording_output
                .lock()
                .expect("recording output mutex poisoned")
                .ensure_current_writable()
                .map_err(AppError::InvalidInput)?;
            let id = Uuid::new_v4().to_string();
            let timestamp = now();
            let name = format!("Live Meeting {}", &timestamp[..16]);
            let storage_path = output_root.join("projects").join(&id).display().to_string();
            genesis_adapter::commit_rows(&state.genesis, vec![genesis_adapter::upsert(
                "projects",
                serde_json::json!({"id": id, "name": name, "storage_path": storage_path, "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp}),
            )])
            .map_err(AppError::Genesis)?;
            id
        }
    };

    recover_stale_capture(&state.genesis, &project_id).map_err(AppError::Genesis)?;

    // The ledger owns a project's storage location. New projects point at the
    // selected output root; existing AppData projects remain on their legacy
    // path so changing the destination never strands old recordings.
    let storage_root = crate::project_storage_path(&state.genesis, &project_id)?;
    let recording_id = Uuid::new_v4().to_string();
    let session_dir = storage_root.join("live").join(&recording_id);
    let chunks_dir = session_dir.join("chunks");
    std::fs::create_dir_all(&chunks_dir)?;

    // Refuse rather than start a session the volume cannot hold. An unknown
    // answer is not a refusal — see `free_disk_bytes`.
    if let Some(free) = free_disk_bytes(&chunks_dir) {
        if free < MIN_FREE_BYTES_TO_START {
            return Err(AppError::InvalidInput(format!(
                "พื้นที่ดิสก์เหลือ {} ซึ่งน้อยกว่าขั้นต่ำ {} สำหรับเริ่มบันทึก — ปล่อยพื้นที่ก่อนเริ่มประชุม",
                human_gib(free),
                human_gib(MIN_FREE_BYTES_TO_START)
            )));
        }
    }

    // Channel provenance speakers (editable labels, not identities).
    let timestamp = now();
    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("speakers", serde_json::json!({"id": speaker_id_for(&project_id, "me"), "project_id": project_id, "key": "me", "display_name": "เรา", "confidence": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("speakers", serde_json::json!({"id": speaker_id_for(&project_id, "them"), "project_id": project_id, "key": "them", "display_name": "อีกฝ่าย", "confidence": null, "created_at": timestamp, "updated_at": timestamp})),
    ]).map_err(AppError::Genesis)?;

    start_desktop_capture(
        &state.genesis,
        &project_id,
        &recording_id,
        &session_dir.display().to_string(),
        &timestamp,
        language.as_deref(),
    )
    .map_err(AppError::Genesis)?;

    let job_id = Uuid::new_v4().to_string();
    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id": job_id, "project_id": project_id, "type": "recording.capture", "status": "running", "progress": 0, "input_refs_json": [recording_id], "output_refs_json": [], "provider_id": null, "error_code": null, "error_message": null, "attempt_no": 1, "started_at": timestamp, "finished_at": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "job_id": job_id, "status": "running", "message": "live capture started", "created_at": timestamp})),
        genesis_adapter::upsert("audit_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "event_type": "live_meeting.started", "actor": "user", "payload_json": {"recordingId": recording_id, "captureSystem": capture_system}, "created_at": timestamp})),
    ]).map_err(AppError::Genesis)?;

    let revisioned_context = if transcript_profile
        == crate::live_transcript::LiveTranscriptProfile::Revisioned
    {
        let mut channels = vec![CHANNEL_MIC];
        if capture_system {
            channels.push(CHANNEL_SYSTEM);
        }
        match create_revisioned_session(&state.genesis, &project_id, &recording_id, &channels) {
            Ok(context) => Some(context),
            Err(error) => {
                let _ =
                    crate::set_job_status(&state.genesis, &job_id, "failed", None, Some(&error));
                if let Ok(record) = genesis_adapter::capture(&state.genesis, &recording_id) {
                    let _ = genesis_adapter::finish_capture(&state.genesis, &record, &now());
                }
                return Err(AppError::Genesis(error));
            }
        }
    } else {
        None
    };

    let stop = Arc::new(AtomicBool::new(false));
    let recent: SharedRecent = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let (chunk_tx, chunk_rx) = mpsc::sync_channel::<CaptureEvent>(CAPTURE_EVENT_QUEUE_CAPACITY);

    // Microphone is mandatory: without it there is no session.
    let mic_ready = spawn_capture_thread_with_fragment_ms(
        ChannelKind::Mic,
        CHANNEL_MIC,
        mic_device_id.clone(),
        stop.clone(),
        chunk_tx.clone(),
        chunks_dir.clone(),
        transcript_profile.fragment_ms() as u64,
    );
    let mic_ready = match mic_ready {
        Ok(ready) => ready,
        Err(error) => {
            stop.store(true, Ordering::SeqCst);
            if let Some(context) = &revisioned_context {
                update_revisioned_session_state(&state.genesis, context, "failed");
            }
            let _ = crate::set_job_status(&state.genesis, &job_id, "failed", None, Some(&error));
            if let Ok(record) = genesis_adapter::capture(&state.genesis, &recording_id) {
                let _ = genesis_adapter::finish_capture(&state.genesis, &record, &now());
            }
            return Err(AppError::InvalidInput(format!(
                "เปิดไมโครโฟนไม่สำเร็จ: {error}"
            )));
        }
    };

    // System loopback is best-effort: a failure downgrades to mic-only.
    let mut warning = None;
    let system_device = if capture_system {
        match spawn_capture_thread_with_fragment_ms(
            ChannelKind::SystemLoopback,
            CHANNEL_SYSTEM,
            system_device_id.clone(),
            stop.clone(),
            chunk_tx.clone(),
            chunks_dir.clone(),
            transcript_profile.fragment_ms() as u64,
        ) {
            Ok(ready) => Some(ready.device_name),
            Err(error) => {
                if system_device_id.is_some() {
                    stop.store(true, Ordering::SeqCst);
                    if let Some(context) = &revisioned_context {
                        update_revisioned_session_state(&state.genesis, context, "failed");
                    }
                    let _ = crate::set_job_status(
                        &state.genesis,
                        &job_id,
                        "failed",
                        None,
                        Some(&error),
                    );
                    if let Ok(record) = genesis_adapter::capture(&state.genesis, &recording_id) {
                        let _ = genesis_adapter::finish_capture(&state.genesis, &record, &now());
                    }
                    return Err(AppError::InvalidInput(format!(
                        "เปิดเสียงระบบที่เลือกไม่สำเร็จ: {error}"
                    )));
                }
                warning = Some(format!("จับเสียงระบบไม่ได้ ({error}) — อัดเฉพาะไมค์"));
                None
            }
        }
    } else {
        None
    };
    if capture_system && system_device.is_none() {
        if let Some(context) = &revisioned_context {
            update_revisioned_source_state(&state.genesis, context, CHANNEL_SYSTEM, "unavailable");
        }
    }
    drop(chunk_tx); // coordinator's Disconnected now depends only on channel threads

    capture_reservation.commit_active();
    {
        let mut live = state.live.lock().expect("live session mutex poisoned");
        *live = Some(LiveSessionControl {
            stop: stop.clone(),
            project_id: project_id.clone(),
            recording_id: recording_id.clone(),
            job_id: job_id.clone(),
            recent: recent.clone(),
            started_at: Instant::now(),
            coordinator: None,
        });
    }

    let coordinator = spawn_coordinator(
        app.clone(),
        state.genesis.clone(),
        state.whisper_runtime_clone(),
        language,
        chunk_rx,
        recent.clone(),
        project_id.clone(),
        recording_id.clone(),
        job_id.clone(),
        session_dir.clone(),
        stop.clone(),
        Arc::clone(&state.native_capture),
        transcript_profile,
        revisioned_context,
    );
    let mut cleanup_coordinator = None;
    {
        let mut live = state.live.lock().expect("live session mutex poisoned");
        if let Some(session) = live
            .as_mut()
            .filter(|session| session.recording_id == recording_id)
        {
            if session.stop.load(Ordering::Acquire) {
                cleanup_coordinator = Some(coordinator);
            } else {
                session.coordinator = Some(coordinator);
            }
        } else {
            cleanup_coordinator = Some(coordinator);
        }
    }
    if let Some(coordinator) = cleanup_coordinator {
        stop.store(true, Ordering::SeqCst);
        let _ = spawn_capture_cleanup(
            Arc::clone(&state.live),
            Arc::clone(&state.native_capture),
            coordinator,
        );
    }

    meeting_intel::spawn_topic_tracker(
        app.clone(),
        state.genesis.clone(),
        recent.clone(),
        stop.clone(),
        recording_id.clone(),
    );

    emit_status(
        &app,
        &recording_id,
        "starting",
        warning.clone(),
        Some(mic_ready.device_name.clone()),
        system_device.clone(),
    );

    Ok(LiveStartOutput {
        project_id,
        recording_id,
        job_id,
        mic_device: mic_ready.device_name,
        system_device,
        transcript_profile: match transcript_profile {
            crate::live_transcript::LiveTranscriptProfile::Chunked => "chunked".to_string(),
            crate::live_transcript::LiveTranscriptProfile::Revisioned => "revisioned".to_string(),
        },
        warning,
    })
}

fn spawn_capture_cleanup(
    live_state: LiveState,
    native_capture: Arc<crate::recording_review::NativeCaptureGuard>,
    coordinator: JoinHandle<()>,
) -> Option<JoinHandle<()>> {
    match thread::Builder::new()
        .name("fung-live-shutdown-cleanup".to_string())
        .spawn(move || {
            let _ = coordinator.join();
            native_capture.release_capture();
            *live_state.lock().expect("live session mutex poisoned") = None;
        }) {
        Ok(cleanup) => Some(cleanup),
        Err(error) => {
            eprintln!("live capture cleanup could not be scheduled: {error}");
            // The coordinator was not joined, so the admission lease remains
            // closed. Releasing it here would permit a late stream to overlap
            // a new capture.
            None
        }
    }
}

/// Requests capture shutdown and transfers coordinator ownership to an
/// off-dispatch cleanup thread. The native admission lease is released only
/// after that coordinator has quiesced.
fn shutdown_capture(
    live_state: &LiveState,
    native_capture: &Arc<crate::recording_review::NativeCaptureGuard>,
) -> Option<JoinHandle<()>> {
    let (coordinator, had_session) = {
        let mut live = live_state.lock().expect("live session mutex poisoned");
        match live.as_mut() {
            Some(session) => {
                session.stop.store(true, Ordering::SeqCst);
                native_capture.mark_capture_stopping();
                (session.coordinator.take(), true)
            }
            None => (None, false),
        }
    };

    if let Some(coordinator) = coordinator {
        spawn_capture_cleanup(
            Arc::clone(live_state),
            Arc::clone(native_capture),
            coordinator,
        )
    } else if !had_session {
        native_capture.release_capture();
        None
    } else {
        // No handle means ownership could not be proven quiescent.  Keep the
        // admission lease closed rather than allowing a late coordinator to
        // overlap a new native session.
        eprintln!("live capture coordinator handle was unavailable during shutdown");
        None
    }
}

pub(crate) fn shutdown(state: &AppState) {
    let _ = shutdown_capture(&state.live, &state.native_capture);
}

#[tauri::command]
pub(crate) fn live_meeting_stop(state: State<'_, AppState>) -> AppResult<String> {
    let live = state.live.lock().expect("live session mutex poisoned");
    match live.as_ref() {
        Some(session) => {
            session.stop.store(true, Ordering::SeqCst);
            state.native_capture.mark_capture_stopping();
            Ok(session.recording_id.clone())
        }
        None => Err(AppError::InvalidInput(
            "ไม่มีเซสชันประชุมสดที่กำลังทำงาน".to_string(),
        )),
    }
}

#[tauri::command]
pub(crate) fn live_meeting_status(state: State<'_, AppState>) -> AppResult<LiveStatusOutput> {
    let live = state.live.lock().expect("live session mutex poisoned");
    Ok(match live.as_ref() {
        Some(session) => LiveStatusOutput {
            active: true,
            stopping: session.stop.load(Ordering::SeqCst),
            project_id: Some(session.project_id.clone()),
            recording_id: Some(session.recording_id.clone()),
            elapsed_ms: Some(session.started_at.elapsed().as_millis() as u64),
        },
        None => LiveStatusOutput {
            active: false,
            stopping: false,
            project_id: None,
            recording_id: None,
            elapsed_ms: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::{OpenOptions, Storage};

    #[test]
    fn capture_sample_queue_holes_are_emitted_as_ordered_media_gaps() {
        let temp = tempfile::tempdir().expect("temporary chunk root");
        let (tx, rx) = mpsc::sync_channel(CAPTURE_EVENT_QUEUE_CAPACITY);
        let mut accumulator = Vec::new();
        let mut timeline_samples = 0;
        let mut expected_sample = 0;
        let mut sequence = 0;

        process_capture_sample_batch(
            CaptureSampleBatch {
                first_sample: 0,
                samples: vec![1; 8],
            },
            CHANNEL_MIC,
            1_000,
            4,
            temp.path(),
            &tx,
            &mut accumulator,
            &mut timeline_samples,
            &mut expected_sample,
            &mut sequence,
        );
        process_capture_sample_batch(
            CaptureSampleBatch {
                first_sample: 12,
                samples: vec![1; 4],
            },
            CHANNEL_MIC,
            1_000,
            4,
            temp.path(),
            &tx,
            &mut accumulator,
            &mut timeline_samples,
            &mut expected_sample,
            &mut sequence,
        );

        let events = rx.try_iter().collect::<Vec<_>>();
        assert_eq!(events.len(), 4);
        assert!(
            matches!(&events[0], CaptureEvent::Chunk(chunk) if (chunk.start_ms, chunk.end_ms) == (0, 4))
        );
        assert!(
            matches!(&events[1], CaptureEvent::Chunk(chunk) if (chunk.start_ms, chunk.end_ms) == (4, 8))
        );
        assert!(matches!(
            &events[2],
            CaptureEvent::SourceGap {
                start_ms: 8,
                end_ms: 12,
                reason: "sample_queue_overflow",
                ..
            }
        ));
        assert!(
            matches!(&events[3], CaptureEvent::Chunk(chunk) if (chunk.start_ms, chunk.end_ms) == (12, 16))
        );
        assert_eq!(expected_sample, 16);
    }

    #[test]
    fn failed_chunk_write_reports_its_exact_source_interval() {
        let temp = tempfile::tempdir().expect("temporary chunk root");
        let not_a_directory = temp.path().join("file");
        std::fs::write(&not_a_directory, b"x").expect("create non-directory path");
        let (tx, rx) = mpsc::sync_channel(CAPTURE_EVENT_QUEUE_CAPACITY);
        let mut timeline_samples = 0;
        let mut sequence = 0;
        let mut samples = vec![1; 4];

        cut_capture_chunk(
            CHANNEL_MIC,
            1_000,
            &not_a_directory,
            &tx,
            &mut samples,
            &mut timeline_samples,
            &mut sequence,
            4,
        );

        assert!(matches!(
            rx.try_recv(),
            Ok(CaptureEvent::ChunkWriteFailed {
                channel: CHANNEL_MIC,
                start_ms: 0,
                end_ms: 4,
                ..
            })
        ));
    }

    fn open_storage() -> (PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-live-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .expect("open storage");
        genesis_adapter::install(&storage).expect("install schema");
        (path, storage)
    }

    #[test]
    fn capture_device_keys_are_direction_scoped_and_deterministic() {
        let mic_a = capture_device_id(CaptureDeviceKind::Mic, "USB Mic", 0);
        let mic_b = capture_device_id(CaptureDeviceKind::Mic, "USB Mic", 0);
        let system = capture_device_id(CaptureDeviceKind::SystemLoopback, "USB Mic", 0);

        assert_eq!(mic_a, mic_b);
        assert_ne!(mic_a, system);
        assert!(mic_a.starts_with("mic-"));
        assert!(system.starts_with("system-"));
    }

    #[test]
    fn capture_preferences_round_trip_without_entering_the_genesis_ledger() {
        let root = tempfile::tempdir().expect("temp preferences root");
        let preferences = CaptureDevicePreferences {
            mic_device_id: Some("mic-selected".to_string()),
            system_device_id: Some("system-selected".to_string()),
        };

        write_capture_preferences(root.path(), &preferences).expect("write preferences");
        assert_eq!(
            read_capture_preferences(root.path()).expect("read preferences"),
            preferences
        );
        assert!(capture_preferences_path(root.path()).is_file());
    }

    #[test]
    fn blank_device_ids_mean_system_default() {
        assert_eq!(normalize_device_id(None), None);
        assert_eq!(normalize_device_id(Some("  ".to_string())), None);
        assert_eq!(
            normalize_device_id(Some(" mic-selected ".to_string())),
            Some("mic-selected".to_string())
        );
    }

    #[test]
    fn a_chunk_filename_maps_only_to_a_real_capture_channel() {
        // The channel decides which speaker recovered text is attributed to,
        // so an unrecognised name must not be guessed at.
        assert_eq!(channel_for_file_name("mic-00001.wav"), Some(CHANNEL_MIC));
        assert_eq!(
            channel_for_file_name("system-00042.wav"),
            Some(CHANNEL_SYSTEM)
        );
        assert_eq!(channel_for_file_name("other-00001.wav"), None);
        assert_eq!(channel_for_file_name("mic.wav"), None);
        assert_eq!(channel_for_file_name("notes.txt"), None);
    }

    #[test]
    fn shutdown_keeps_capture_admission_until_coordinator_quiesces() {
        let guard = Arc::new(crate::recording_review::NativeCaptureGuard::default());
        guard.try_start_capture().unwrap();
        guard.mark_capture_active();
        let stop = Arc::new(AtomicBool::new(false));
        let coordinator_stop = Arc::clone(&stop);
        let (allow_tx, allow_rx) = mpsc::channel();
        let coordinator = thread::spawn(move || {
            while !coordinator_stop.load(Ordering::Acquire) {
                thread::yield_now();
            }
            allow_rx.recv().unwrap();
        });
        let live = Arc::new(Mutex::new(Some(LiveSessionControl {
            stop,
            project_id: "p1".to_string(),
            recording_id: "r1".to_string(),
            job_id: "j1".to_string(),
            recent: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            started_at: Instant::now(),
            coordinator: Some(coordinator),
        })));
        let cleanup = shutdown_capture(&live, &guard).unwrap();

        let deadline = Instant::now() + Duration::from_millis(100);
        while guard.capture_phase() != crate::recording_review::CapturePhase::Stopping
            && Instant::now() < deadline
        {
            thread::yield_now();
        }
        assert_eq!(
            guard.capture_phase(),
            crate::recording_review::CapturePhase::Stopping
        );
        assert_ne!(
            guard.capture_phase(),
            crate::recording_review::CapturePhase::Inactive
        );
        allow_tx.send(()).unwrap();
        cleanup.join().unwrap();
        assert_eq!(
            guard.capture_phase(),
            crate::recording_review::CapturePhase::Inactive
        );
        assert!(live.lock().unwrap().is_none());
    }

    /// Builds a recording with two chunks — one per channel, same time range —
    /// so coverage can be checked per channel rather than per timestamp.
    fn seed_two_channel_recording(storage: &Storage, dir: &std::path::Path) -> (String, String) {
        let project_id = "gap-project".to_string();
        let recording_id = "gap-recording".to_string();
        let timestamp = now();
        std::fs::create_dir_all(dir).unwrap();
        let mic_path = dir.join("mic-00001.wav");
        let system_path = dir.join("system-00001.wav");
        std::fs::write(&mic_path, b"mic audio").unwrap();
        std::fs::write(&system_path, b"system audio").unwrap();

        genesis_adapter::commit_rows(storage, vec![
            genesis_adapter::upsert("projects", serde_json::json!({"id": project_id, "name": "Gap", "storage_path": dir.display().to_string(), "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})),
            genesis_adapter::upsert("recordings", serde_json::json!({"id": recording_id, "project_id": project_id, "source": "microphone", "input_path": null, "canonical_audio_path": dir.display().to_string(), "status": "completed", "duration_ms": 8000, "created_at": timestamp, "updated_at": timestamp})),
            genesis_adapter::upsert("speakers", serde_json::json!({"id": speaker_id_for(&project_id, "me"), "project_id": project_id, "key": "me", "display_name": "เรา", "confidence": null, "created_at": timestamp, "updated_at": timestamp})),
            genesis_adapter::upsert("speakers", serde_json::json!({"id": speaker_id_for(&project_id, "them"), "project_id": project_id, "key": "them", "display_name": "อีกฝ่าย", "confidence": null, "created_at": timestamp, "updated_at": timestamp})),
            genesis_adapter::upsert("audio_chunks", serde_json::json!({"id": "chunk-mic", "recording_id": recording_id, "sequence_no": 1, "file_path": mic_path.display().to_string(), "start_ms": 0, "end_ms": 8000, "byte_size": 9, "checksum": "aa", "created_at": timestamp})),
            genesis_adapter::upsert("audio_chunks", serde_json::json!({"id": "chunk-system", "recording_id": recording_id, "sequence_no": 2, "file_path": system_path.display().to_string(), "start_ms": 0, "end_ms": 8000, "byte_size": 12, "checksum": "bb", "created_at": timestamp})),
        ]).unwrap();
        (project_id, recording_id)
    }

    /// The project a capture is recorded into. Created separately because
    /// `start_desktop_capture` must not touch it.
    fn seed_project(storage: &Storage, project_id: &str, path: &std::path::Path) {
        let timestamp = now();
        genesis_adapter::commit_rows(storage, vec![
            genesis_adapter::upsert("projects", serde_json::json!({"id": project_id, "name": "Lang", "storage_path": path.display().to_string(), "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp})),
        ]).unwrap();
    }

    #[test]
    fn a_transcribed_chunk_with_no_words_is_not_offered_again() {
        // A chunk of silence produces no segments, so coverage-by-segment
        // alone re-queued it on every catch-up pass — a quiet meeting looked
        // permanently unfinished and the whisper worker was restarted to
        // re-transcribe audio that had nothing in it.
        let (path, storage) = open_storage();
        let dir = path.join("chunks");
        let (project_id, recording_id) = seed_two_channel_recording(&storage, &dir);

        assert_eq!(
            chunks_missing_transcript(&storage, &project_id, &recording_id)
                .unwrap()
                .len(),
            2
        );

        // The transcriber saw the microphone chunk and it held silence.
        genesis_adapter::mark_chunk_transcribed(&storage, "chunk-mic", &now()).unwrap();

        let missing = chunks_missing_transcript(&storage, &project_id, &recording_id).unwrap();
        assert_eq!(
            missing
                .iter()
                .map(|chunk| chunk.chunk_id.as_str())
                .collect::<Vec<_>>(),
            vec!["chunk-system"],
            "only the chunk the transcriber has not seen is still pending"
        );
    }

    #[test]
    fn a_chunk_transcribed_before_the_stamp_existed_is_still_recognised() {
        // Rows written before `transcribed_at` carry NULL. One that produced
        // text must not be re-transcribed just because it has no stamp, or
        // the migration would duplicate every existing recording's segments.
        let (path, storage) = open_storage();
        let dir = path.join("chunks");
        let (project_id, recording_id) = seed_two_channel_recording(&storage, &dir);
        let timestamp = now();
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "transcript_segments",
                serde_json::json!({
                    "id": "seg-legacy",
                    "project_id": project_id,
                    "recording_id": recording_id,
                    "speaker_id": speaker_id_for(&project_id, "me"),
                    "start_ms": 10,
                    "end_ms": 900,
                    "text": "มีข้อความอยู่แล้ว",
                    "confidence": null,
                    "created_at": timestamp,
                    "updated_at": timestamp,
                }),
            )],
        )
        .unwrap();

        let missing = chunks_missing_transcript(&storage, &project_id, &recording_id).unwrap();
        assert_eq!(
            missing
                .iter()
                .map(|chunk| chunk.chunk_id.as_str())
                .collect::<Vec<_>>(),
            vec!["chunk-system"],
            "an unstamped chunk that has segments is still covered"
        );
    }

    #[test]
    fn the_session_language_survives_the_rows_rewritten_on_every_chunk() {
        // `append_capture_chunk` and `finish_capture` rewrite the whole
        // recordings row, so a column they do not carry is cleared on the
        // first chunk. That is what would have made storing the language
        // pointless.
        let (path, storage) = open_storage();
        // `start_desktop_capture` deliberately does not create the project —
        // that is what keeps it off the mobile helper that renames one.
        seed_project(&storage, "lang-project", &path);
        let record = start_desktop_capture(
            &storage,
            "lang-project",
            "lang-recording",
            &path.display().to_string(),
            &now(),
            Some("th"),
        )
        .unwrap();
        assert_eq!(record.language.as_deref(), Some("th"));

        let chunk_path = path.join("mic-00001.wav");
        let record = genesis_adapter::append_capture_chunk(
            &storage,
            &record,
            genesis_adapter::AudioChunk {
                id: "lang-chunk",
                file_path: &chunk_path.display().to_string(),
                start_ms: 0,
                end_ms: 2000,
                byte_size: 4,
                checksum: "cc",
                timestamp: &now(),
            },
        )
        .unwrap();
        genesis_adapter::finish_capture(&storage, &record, &now()).unwrap();

        assert_eq!(
            genesis_adapter::recording_language(&storage, "lang-recording").as_deref(),
            Some("th"),
            "the catch-up pass reads this back; clearing it would re-detect per chunk"
        );
    }

    #[test]
    fn a_session_started_without_a_language_reports_none_not_a_guess() {
        // "auto" is a real choice. Storing a default here would tell every
        // later pass the user picked a language they never picked.
        let (path, storage) = open_storage();
        // `start_desktop_capture` deliberately does not create the project —
        // that is what keeps it off the mobile helper that renames one.
        seed_project(&storage, "auto-project", &path);
        start_desktop_capture(
            &storage,
            "auto-project",
            "auto-recording",
            &path.display().to_string(),
            &now(),
            None,
        )
        .unwrap();
        assert_eq!(
            genesis_adapter::recording_language(&storage, "auto-recording"),
            None
        );
    }

    #[test]
    fn a_chunk_is_only_covered_by_text_from_its_own_channel() {
        // The two channels share one timeline. Matching on time alone would
        // let system-audio text mark a microphone chunk as transcribed, and
        // that chunk's words would be lost for good.
        let (path, storage) = open_storage();
        let dir = path.join("chunks");
        let (project_id, recording_id) = seed_two_channel_recording(&storage, &dir);

        // Nothing transcribed yet: both chunks need text.
        let missing = chunks_missing_transcript(&storage, &project_id, &recording_id).unwrap();
        assert_eq!(missing.len(), 2);

        // Transcribe only the system channel, in the same time range.
        let timestamp = now();
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "transcript_segments",
                serde_json::json!({
                    "id": "seg-1", "project_id": project_id, "recording_id": recording_id,
                    "speaker_id": speaker_id_for(&project_id, "them"),
                    "start_ms": 100, "end_ms": 900, "text": "จากอีกฝ่าย", "confidence": 0.9,
                    "created_at": timestamp, "updated_at": timestamp,
                }),
            )],
        )
        .unwrap();

        let missing = chunks_missing_transcript(&storage, &project_id, &recording_id).unwrap();
        assert_eq!(missing.len(), 1, "only the microphone chunk should remain");
        assert_eq!(missing[0].channel, CHANNEL_MIC);
    }

    #[test]
    fn a_chunk_whose_audio_is_gone_is_not_offered_for_transcription() {
        let (path, storage) = open_storage();
        let dir = path.join("chunks");
        let (project_id, recording_id) = seed_two_channel_recording(&storage, &dir);
        std::fs::remove_file(dir.join("mic-00001.wav")).unwrap();

        let missing = chunks_missing_transcript(&storage, &project_id, &recording_id).unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(
            missing[0].channel, CHANNEL_SYSTEM,
            "a chunk with no file on disk cannot be transcribed and must not be queued"
        );
    }

    #[test]
    fn a_recording_with_full_coverage_reports_no_gaps() {
        let (path, storage) = open_storage();
        let dir = path.join("chunks");
        let (project_id, recording_id) = seed_two_channel_recording(&storage, &dir);
        let timestamp = now();
        genesis_adapter::commit_rows(&storage, vec![
            genesis_adapter::upsert("transcript_segments", serde_json::json!({"id": "seg-me", "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id_for(&project_id, "me"), "start_ms": 10, "end_ms": 900, "text": "เรา", "confidence": 0.9, "created_at": timestamp, "updated_at": timestamp})),
            genesis_adapter::upsert("transcript_segments", serde_json::json!({"id": "seg-them", "project_id": project_id, "recording_id": recording_id, "speaker_id": speaker_id_for(&project_id, "them"), "start_ms": 20, "end_ms": 900, "text": "อีกฝ่าย", "confidence": 0.9, "created_at": timestamp, "updated_at": timestamp})),
        ]).unwrap();

        let missing = chunks_missing_transcript(&storage, &project_id, &recording_id).unwrap();
        assert!(
            missing.is_empty(),
            "a fully transcribed recording has no gaps to fill"
        );
    }

    #[test]
    fn a_gap_fill_that_declines_says_why_instead_of_reporting_nothing_to_do() {
        // An empty outcome and a refused outcome must not look alike: one
        // means the transcript is complete, the other means it was not checked.
        let declined = GapFillOutcome {
            skipped_reason: Some("too many segments to enumerate".into()),
            ..GapFillOutcome::default()
        };
        assert_eq!(declined.chunks_missing_transcript, 0);
        assert!(declined.skipped_reason.is_some());
        assert!(GapFillOutcome::default().skipped_reason.is_none());
    }

    /// Regression: routing desktop capture through
    /// `genesis_adapter::start_capture` renamed the recorded project to
    /// "FUNG Mobile" (its `ensure_project_mutations` hardcodes that name), so
    /// the meeting export header reported the wrong meeting.
    #[test]
    fn a_capture_that_lost_audio_is_never_reported_as_completed() {
        // The job row is the durable record. If it says "completed" after
        // chunks failed to write, nothing downstream can tell the recording is
        // short — and the user is told the session finished normally.
        let lossy = capture_outcome(120_000, 3, 1, 0);
        assert!(
            lossy.failure_reason.is_some(),
            "a capture that lost chunks must fail its job"
        );
        assert!(lossy
            .failure_reason
            .unwrap()
            .contains("3 audio chunk(s) or media interval(s) were lost"));
        assert!(lossy.message.contains("เสียงหาย 3 ช่วง"));

        let clean = capture_outcome(120_000, 0, 0, 0);
        assert_eq!(clean.failure_reason, None);
        assert_eq!(clean.message, "บันทึกเสร็จ ความยาว 120 วินาที");
    }

    #[test]
    fn a_stream_fault_is_surfaced_without_failing_an_otherwise_intact_capture() {
        // A device glitch that cost no chunks is worth saying, but the audio
        // is complete, so the job did complete.
        let outcome = capture_outcome(60_000, 0, 2, 0);
        assert_eq!(outcome.failure_reason, None);
        assert!(outcome.message.contains("สตรีมเสียงผิดพลาด 2 ครั้ง"));
    }

    #[test]
    fn an_incomplete_transcript_is_stated_rather_than_implied_complete() {
        // Regression for the degraded-mode promise: chunks the catch-up pass
        // could not transcribe must be counted in what the user is told.
        let outcome = capture_outcome(60_000, 0, 0, 4);
        assert_eq!(outcome.failure_reason, None);
        assert!(outcome.message.contains("ยังถอดความไม่ได้ 4 ช่วง"));

        let full = capture_outcome(60_000, 0, 0, 0);
        assert!(!full.message.contains("ยังถอดความไม่ได้"));
    }

    // Threshold ordering is the whole guarantee, and it is knowable without
    // running anything: the stop floor must sit above zero and below the
    // warning, starting must demand more headroom than continuing, and the
    // start floor must cover more than an hour of dual-channel WAV (~690
    // MB/hour) so a normal meeting is never refused. Asserting it at compile
    // time makes an unsafe edit fail the build rather than a test run.
    const _: () = {
        assert!(MIN_FREE_BYTES_TO_CONTINUE > 0);
        assert!(LOW_DISK_WARN_BYTES > MIN_FREE_BYTES_TO_CONTINUE);
        assert!(MIN_FREE_BYTES_TO_START > LOW_DISK_WARN_BYTES);
        assert!(MIN_FREE_BYTES_TO_START > 690 * 1024 * 1024);
    };

    #[test]
    fn free_space_is_readable_for_a_path_that_does_not_exist_yet() {
        // The pre-start guard runs against a session directory that may not
        // exist, so the probe has to walk up to a real ancestor rather than
        // report "unknown" and silently disable the guard.
        let temp = tempfile::tempdir().expect("temp dir");
        let unborn = temp
            .path()
            .join("projects")
            .join("p1")
            .join("live")
            .join("r1");
        assert!(!unborn.exists());

        match free_disk_bytes(&unborn) {
            Some(free) => assert!(free > 0, "an existing volume reports some free space"),
            // Non-Windows builds answer "unknown" by design; callers must not
            // treat that as a refusal.
            #[cfg(windows)]
            None => panic!("windows must be able to report free space"),
            #[cfg(not(windows))]
            None => {}
        }
    }

    #[test]
    fn human_gib_is_readable_at_the_sizes_the_guard_reports() {
        assert_eq!(human_gib(2 * 1024 * 1024 * 1024), "2.0 GB");
        assert_eq!(human_gib(256 * 1024 * 1024), "0.2 GB");
    }

    #[test]
    fn starting_a_desktop_capture_preserves_the_project_name() {
        let (path, storage) = open_storage();
        let timestamp = "2026-08-10T00:00:00Z";
        genesis_adapter::commit_rows(
            &storage,
            vec![genesis_adapter::upsert(
                "projects",
                serde_json::json!({
                    "id": "p1",
                    "name": "ประชุมทีมขาย",
                    "storage_path": "C:/tmp/p1",
                    "active_recording_id": null,
                    "created_at": timestamp,
                    "updated_at": timestamp,
                }),
            )],
        )
        .expect("seed project");

        let record = start_desktop_capture(
            &storage,
            "p1",
            "r1",
            "C:/tmp/p1/live/r1",
            timestamp,
            Some("th"),
        )
        .expect("start desktop capture");
        assert_eq!(record.status, "recording");
        assert_eq!(record.segment_count, 0);

        let rows = genesis_adapter::query(
            &storage,
            "projects",
            &["id", "name"],
            vec![genesis_adapter::eq(
                "projects",
                "id",
                serde_json::json!("p1"),
            )],
            1,
        )
        .expect("query project");
        assert_eq!(
            rows[0]["projects.name"], "ประชุมทีมขาย",
            "desktop capture must never rewrite the user's project name"
        );

        drop(storage);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn detailed_candidate_alignment_requires_unambiguous_time_or_speaker_evidence() {
        let source = vec![
            DetailedSourceSegment {
                id: "mic".to_string(),
                speaker_id: Some("speaker-mic".to_string()),
                start_ms: 100,
                end_ms: 300,
                text: "ไมค์เดิม".to_string(),
                updated_at: "u1".to_string(),
                expected_revision: None,
            },
            DetailedSourceSegment {
                id: "system".to_string(),
                speaker_id: Some("speaker-system".to_string()),
                start_ms: 120,
                end_ms: 280,
                text: "เสียงระบบเดิม".to_string(),
                updated_at: "u2".to_string(),
                expected_revision: None,
            },
        ];

        assert_eq!(
            unique_detailed_segment_match(&source, 140, 220, Some("speaker-mic"))
                .map(|segment| segment.id.as_str()),
            Some("mic")
        );
        assert!(unique_detailed_segment_match(&source, 140, 220, None).is_none());
        assert!(unique_detailed_segment_match(&source, 400, 500, Some("speaker-mic")).is_none());
    }

    #[test]
    fn detailed_candidate_can_align_a_single_legacy_segment_without_speaker_metadata() {
        let source = vec![DetailedSourceSegment {
            id: "legacy".to_string(),
            speaker_id: None,
            start_ms: 100,
            end_ms: 300,
            text: "ข้อความเดิม".to_string(),
            updated_at: "u1".to_string(),
            expected_revision: None,
        }];

        assert_eq!(
            unique_detailed_segment_match(&source, 120, 240, Some("speaker-mic"))
                .map(|segment| segment.id.as_str()),
            Some("legacy")
        );
    }

    #[test]
    fn detailed_candidate_join_preserves_english_boundaries_without_splitting_thai() {
        assert_eq!(
            join_detailed_candidate_texts(["Hello".to_string(), "world".to_string()]),
            "Hello world"
        );
        assert_eq!(
            join_detailed_candidate_texts(["สวัสดี".to_string(), "ครับ".to_string()]),
            "สวัสดีครับ"
        );
        assert_eq!(
            join_detailed_candidate_texts(["Hello".to_string(), "ไทย".to_string()]),
            "Hello ไทย"
        );
    }
}
