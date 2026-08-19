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

use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
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
/// A trailing partial chunk shorter than this is dropped: whisper yields
/// nothing useful for it and the ledger row would be noise.
const MIN_FINAL_CHUNK_MS: u64 = 400;
const RECENT_SEGMENT_CAP: usize = 240;
/// Model load can include a first-time download; give it room.
const WORKER_READY_TIMEOUT: Duration = Duration::from_secs(180);
const WORKER_CHUNK_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) const CHANNEL_MIC: &str = "mic";
pub(crate) const CHANNEL_SYSTEM: &str = "system";

/// Rolling in-memory window of the newest live segments. Shared with the
/// topic tracker and `meeting_ask` so they never need a mid-session DB read.
pub(crate) type SharedRecent = Arc<Mutex<std::collections::VecDeque<RecentSegment>>>;

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
    pub(crate) job_id: String,
    pub(crate) recent: SharedRecent,
    pub(crate) started_at: Instant,
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

pub(crate) struct RawChunk {
    pub(crate) channel: &'static str,
    pub(crate) chunk_id: String,
    pub(crate) file_path: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) byte_size: i64,
    pub(crate) checksum: String,
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
        error: String,
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
fn build_stream<T: cpal::SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: mpsc::Sender<Vec<i16>>,
    convert: fn(T) -> i16,
    channel: &'static str,
    faults: mpsc::Sender<CaptureEvent>,
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
                let _ = tx.send(mono);
            },
            move |error| {
                // Runs on the audio callback thread; sending is non-blocking
                // and the coordinator turns this into user-visible state.
                let _ = faults.send(CaptureEvent::StreamFailed {
                    channel,
                    error: error.to_string(),
                });
            },
            None,
        )
        .map_err(|error| format!("build_input_stream failed: {error}"))
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

fn write_chunk_wav(path: &Path, sample_rate: u32, samples: &[i16]) -> Result<(i64, String), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|error| format!("wav create failed: {error}"))?;
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
    stop: Arc<AtomicBool>,
    chunk_tx: mpsc::Sender<CaptureEvent>,
    chunks_dir: PathBuf,
) -> Result<CaptureReady, String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();

    thread::spawn(move || {
        let host = cpal::default_host();
        let device = match kind {
            ChannelKind::Mic => host.default_input_device(),
            // WASAPI exposes render endpoints as loopback capture sources:
            // opening an *input* stream on the default *output* device
            // records everything the machine plays (the "them" side of an
            // online meeting).
            ChannelKind::SystemLoopback => host.default_output_device(),
        };
        let Some(device) = device else {
            let _ = ready_tx.send(Err(match kind {
                ChannelKind::Mic => "ไม่พบไมโครโฟนเริ่มต้นของระบบ".to_string(),
                ChannelKind::SystemLoopback => "ไม่พบอุปกรณ์เสียงออกเริ่มต้น (loopback)".to_string(),
            }));
            return;
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
                    ChannelKind::Mic => device.supported_input_configs().ok().and_then(|mut c| c.next()),
                    ChannelKind::SystemLoopback => {
                        device.supported_output_configs().ok().and_then(|mut c| c.next())
                    }
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

        let (sample_tx, sample_rx) = mpsc::channel::<Vec<i16>>();
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_stream::<f32>(
                &device, &stream_config, channels, sample_tx, sample_f32_to_i16,
                channel, chunk_tx.clone(),
            ),
            cpal::SampleFormat::I16 => build_stream::<i16>(
                &device, &stream_config, channels, sample_tx, sample_i16_to_i16,
                channel, chunk_tx.clone(),
            ),
            cpal::SampleFormat::U16 => build_stream::<u16>(
                &device, &stream_config, channels, sample_tx, sample_u16_to_i16,
                channel, chunk_tx.clone(),
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

        let samples_per_chunk = (sample_rate as u64 * CHUNK_MS / 1000) as usize;
        let min_final_samples = (sample_rate as u64 * MIN_FINAL_CHUNK_MS / 1000) as usize;
        let mut accumulator: Vec<i16> = Vec::with_capacity(samples_per_chunk + 4096);
        let mut written_samples: u64 = 0;
        let mut local_seq: u32 = 0;

        let cut_chunk = |accumulator: &mut Vec<i16>, written_samples: &mut u64, local_seq: &mut u32, take: usize| {
            let samples: Vec<i16> = accumulator.drain(..take).collect();
            let start_ms = (*written_samples * 1000 / sample_rate as u64) as i64;
            *written_samples += samples.len() as u64;
            let end_ms = (*written_samples * 1000 / sample_rate as u64) as i64;
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
                // These samples are gone. Report it instead of printing to a
                // stderr no user reads.
                Err(error) => {
                    let _ = chunk_tx.send(CaptureEvent::ChunkWriteFailed { channel, error });
                }
            }
        };

        loop {
            match sample_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(batch) => accumulator.extend_from_slice(&batch),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            while accumulator.len() >= samples_per_chunk {
                cut_chunk(&mut accumulator, &mut written_samples, &mut local_seq, samples_per_chunk);
            }
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }

        // Stop the callback source first, then flush whatever already arrived.
        drop(stream);
        for batch in sample_rx.try_iter() {
            accumulator.extend_from_slice(&batch);
        }
        while accumulator.len() >= samples_per_chunk {
            cut_chunk(&mut accumulator, &mut written_samples, &mut local_seq, samples_per_chunk);
        }
        if accumulator.len() >= min_final_samples {
            let take = accumulator.len();
            cut_chunk(&mut accumulator, &mut written_samples, &mut local_seq, take);
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
    id: Option<String>,
    channel: Option<String>,
    #[serde(default)]
    pub(crate) start_ms: i64,
    #[serde(default)]
    pub(crate) segments: Vec<WorkerSegment>,
    pub(crate) error: Option<String>,
    #[serde(default)]
    ready: bool,
}

pub(crate) struct LiveWorker {
    child: Child,
    stdin: std::process::ChildStdin,
    lines: mpsc::Receiver<String>,
}

impl LiveWorker {
    pub(crate) fn spawn(runtime: &WhisperRuntime, language: Option<&str>) -> Result<Self, String> {
        let profile = crate::transcription_profile()?;
        let script = runtime
            .script
            .parent()
            .ok_or_else(|| "scripts directory not found".to_string())?
            .join("transcribe_live.py");

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
            return Err(format!("live worker script is missing at {}", script.display()));
        }

        let mut command = Command::new(&runtime.python);
        command
            .arg(&script)
            .arg("--profile")
            .arg(&effective_profile);
        if let Some(model) = crate::bundled_whisper_model(runtime) {
            command.env("FUNG_WHISPER_MODEL", model);
        }
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
        let stdout = child.stdout.take().ok_or("live worker stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("live worker stderr unavailable")?;

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
        writeln!(self.stdin, "{request}").map_err(|error| format!("worker stdin closed: {error}"))?;
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

// ---------------------------------------------------------------------------
// Coordinator: single Genesis writer + live transcription + post-meeting kick
// ---------------------------------------------------------------------------

fn speaker_id_for(project_id: &str, key: &str) -> String {
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
                "{lost_chunks} audio chunk(s) could not be written to disk; \
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
    channel: &'static str,
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
) {
    thread::spawn(move || {
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
                        Some(format!("ถอดสดใช้ไม่ได้ ({error}) — เสียงยังถูกบันทึกครบ และจะถอดหลังจบ")),
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

        // Chunks whose audio is safely on disk but which the live worker never
        // transcribed. The session-end catch-up pass below is what makes the
        // degraded-mode promise ("will be transcribed after it ends") true.
        let mut pending_transcription: Vec<RawChunk> = Vec::new();
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
                CaptureEvent::ChunkWriteFailed { channel, error } => {
                    lost_chunks += 1;
                    record_capture_fault(
                        &app,
                        &storage,
                        &project_id,
                        &recording_id,
                        "live_meeting.chunk_write_failed",
                        channel,
                        &error,
                        format!("เขียนไฟล์เสียงช่อง {channel} ไม่สำเร็จ ({error}) — เสียงช่วงนี้สูญหาย {lost_chunks} ช่วงแล้ว"),
                    );
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

            let Some(active_worker) = worker.as_mut() else {
                pending_transcription.push(chunk);
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
                    pending_transcription.push(chunk);
                }
            }
        }

        if let Some(active_worker) = worker.take() {
            active_worker.shutdown();
        }

        let finished_at = now();
        capture_record.duration_ms = capture_record.duration_ms.max(max_end_ms);
        if let Err(error) = genesis_adapter::finish_capture(&storage, &capture_record, &finished_at) {
            emit_status(&app, &recording_id, "error", Some(error), None, None);
        }

        // Catch-up transcription. The degraded-mode message tells the user the
        // audio "will be transcribed after it ends"; until this existed, the
        // chunks were collected into a vector and dropped, so the app stated
        // something it never did. Runs before the summary so recovered text is
        // part of it, not missing from it.
        let still_pending = if pending_transcription.is_empty() {
            0
        } else {
            transcribe_pending_chunks(
                &app,
                &storage,
                &recent,
                &runtime,
                language.as_deref(),
                &project_id,
                &recording_id,
                &pending_transcription,
            )
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
        emit_status(&app, &recording_id, "stopped", Some(outcome.message), None, None);

        // Post-meeting pipeline: summary → export. Runs on this same thread —
        // the session is over, so there is nothing left to block.
        meeting_intel::run_post_meeting(&app, &storage, &project_id, &recording_id);

        // Release the in-memory session slot last, so `live_meeting_status`
        // keeps answering "stopping" while the tail work runs.
        if let Some(state) = app.try_state::<AppState>() {
            let mut live = state.live.lock().expect("live session mutex poisoned");
            *live = None;
        }
    });
}

/// Transcribes chunks the live worker missed, after the session has ended.
/// Returns how many are *still* untranscribed, so the caller can report the
/// shortfall rather than implying the transcript is complete.
///
/// Failure here is not fatal: the audio is already durable, and a recording
/// with a partial transcript is a truthful state as long as it is stated.
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
                persist_and_emit_segments(app, storage, recent, project_id, recording_id, chunk, response);
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

fn persist_and_emit_segments(
    app: &tauri::AppHandle,
    storage: &genesis_block_native::Storage,
    recent: &SharedRecent,
    project_id: &str,
    recording_id: &str,
    chunk: &RawChunk,
    response: WorkerResponse,
) {
    let speaker_key = if chunk.channel == CHANNEL_MIC { "me" } else { "them" };
    let speaker_label = if speaker_key == "me" { "เรา" } else { "อีกฝ่าย" };
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
    if mutations.is_empty() {
        return;
    }
    if let Err(error) = genesis_adapter::commit_rows(storage, mutations) {
        eprintln!("[live] transcript segment commit failed: {error}");
        return;
    }
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
        let timestamp = now();
        genesis_adapter::finish_capture(storage, &stale, &timestamp)?;
    }
    let jobs = genesis_adapter::query(
        storage,
        "jobs",
        &["id", "type", "status"],
        vec![genesis_adapter::eq("jobs", "project_id", serde_json::json!(project_id))],
        1000,
    )?;
    for row in jobs {
        let job_type = row.get("jobs.type").and_then(serde_json::Value::as_str).unwrap_or("");
        let status = row.get("jobs.status").and_then(serde_json::Value::as_str).unwrap_or("");
        if job_type == "recording.capture" && matches!(status, "queued" | "running" | "paused" | "retrying") {
            if let Some(id) = row.get("jobs.id").and_then(serde_json::Value::as_str) {
                let _ = crate::set_job_status(storage, id, "failed", None, Some("interrupted: desktop session was not shut down cleanly"));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn live_meeting_start(
    project_id: Option<String>,
    capture_system: Option<bool>,
    language: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<LiveStartOutput> {
    let capture_system = capture_system.unwrap_or(true);

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
            let id = Uuid::new_v4().to_string();
            let timestamp = now();
            let name = format!("Live Meeting {}", &timestamp[..16]);
            let storage_path = state.data_root.join("projects").join(&id).display().to_string();
            genesis_adapter::commit_rows(&state.genesis, vec![genesis_adapter::upsert(
                "projects",
                serde_json::json!({"id": id, "name": name, "storage_path": storage_path, "active_recording_id": null, "created_at": timestamp, "updated_at": timestamp}),
            )])
            .map_err(AppError::Genesis)?;
            id
        }
    };

    recover_stale_capture(&state.genesis, &project_id).map_err(AppError::Genesis)?;

    let recording_id = Uuid::new_v4().to_string();
    let session_dir = state
        .data_root
        .join("projects")
        .join(&project_id)
        .join("live")
        .join(&recording_id);
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
    )
    .map_err(AppError::Genesis)?;

    let job_id = Uuid::new_v4().to_string();
    genesis_adapter::commit_rows(&state.genesis, vec![
        genesis_adapter::upsert("jobs", serde_json::json!({"id": job_id, "project_id": project_id, "type": "recording.capture", "status": "running", "progress": 0, "input_refs_json": [recording_id], "output_refs_json": [], "provider_id": null, "error_code": null, "error_message": null, "attempt_no": 1, "started_at": timestamp, "finished_at": null, "created_at": timestamp, "updated_at": timestamp})),
        genesis_adapter::upsert("job_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "job_id": job_id, "status": "running", "message": "live capture started", "created_at": timestamp})),
        genesis_adapter::upsert("audit_events", serde_json::json!({"id": Uuid::new_v4().to_string(), "project_id": project_id, "event_type": "live_meeting.started", "actor": "user", "payload_json": {"recordingId": recording_id, "captureSystem": capture_system}, "created_at": timestamp})),
    ]).map_err(AppError::Genesis)?;

    let stop = Arc::new(AtomicBool::new(false));
    let recent: SharedRecent = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let (chunk_tx, chunk_rx) = mpsc::channel::<CaptureEvent>();

    // Microphone is mandatory: without it there is no session.
    let mic_ready = spawn_capture_thread(
        ChannelKind::Mic,
        CHANNEL_MIC,
        stop.clone(),
        chunk_tx.clone(),
        chunks_dir.clone(),
    );
    let mic_ready = match mic_ready {
        Ok(ready) => ready,
        Err(error) => {
            stop.store(true, Ordering::SeqCst);
            let _ = crate::set_job_status(&state.genesis, &job_id, "failed", None, Some(&error));
            if let Ok(record) = genesis_adapter::capture(&state.genesis, &recording_id) {
                let _ = genesis_adapter::finish_capture(&state.genesis, &record, &now());
            }
            return Err(AppError::InvalidInput(format!("เปิดไมโครโฟนไม่สำเร็จ: {error}")));
        }
    };

    // System loopback is best-effort: a failure downgrades to mic-only.
    let mut warning = None;
    let system_device = if capture_system {
        match spawn_capture_thread(
            ChannelKind::SystemLoopback,
            CHANNEL_SYSTEM,
            stop.clone(),
            chunk_tx.clone(),
            chunks_dir.clone(),
        ) {
            Ok(ready) => Some(ready.device_name),
            Err(error) => {
                warning = Some(format!("จับเสียงระบบไม่ได้ ({error}) — อัดเฉพาะไมค์"));
                None
            }
        }
    } else {
        None
    };
    drop(chunk_tx); // coordinator's Disconnected now depends only on channel threads

    spawn_coordinator(
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
    );

    meeting_intel::spawn_topic_tracker(
        app.clone(),
        state.genesis.clone(),
        recent.clone(),
        stop.clone(),
        recording_id.clone(),
    );

    {
        let mut live = state.live.lock().expect("live session mutex poisoned");
        *live = Some(LiveSessionControl {
            stop,
            project_id: project_id.clone(),
            recording_id: recording_id.clone(),
            job_id: job_id.clone(),
            recent,
            started_at: Instant::now(),
        });
    }

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
        warning,
    })
}

#[tauri::command]
pub(crate) fn live_meeting_stop(state: State<'_, AppState>) -> AppResult<String> {
    let live = state.live.lock().expect("live session mutex poisoned");
    match live.as_ref() {
        Some(session) => {
            session.stop.store(true, Ordering::SeqCst);
            Ok(session.recording_id.clone())
        }
        None => Err(AppError::InvalidInput("ไม่มีเซสชันประชุมสดที่กำลังทำงาน".to_string())),
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

    fn open_storage() -> (PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-live-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .expect("open storage");
        genesis_adapter::install(&storage).expect("install schema");
        (path, storage)
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
        assert!(lossy.failure_reason.unwrap().contains("3 audio chunk(s)"));
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

    #[test]
    fn disk_thresholds_leave_room_to_close_the_session_cleanly() {
        // Ordering is the whole guarantee: the stop threshold must sit above
        // zero and below the warning, and starting must demand more headroom
        // than continuing.
        assert!(MIN_FREE_BYTES_TO_CONTINUE > 0);
        assert!(LOW_DISK_WARN_BYTES > MIN_FREE_BYTES_TO_CONTINUE);
        assert!(MIN_FREE_BYTES_TO_START > LOW_DISK_WARN_BYTES);
        // Roughly 690 MB/hour of dual-channel WAV: the start floor must cover
        // more than an hour so a normal meeting is not refused.
        assert!(MIN_FREE_BYTES_TO_START > 690 * 1024 * 1024);
    }

    #[test]
    fn free_space_is_readable_for_a_path_that_does_not_exist_yet() {
        // The pre-start guard runs against a session directory that may not
        // exist, so the probe has to walk up to a real ancestor rather than
        // report "unknown" and silently disable the guard.
        let temp = tempfile::tempdir().expect("temp dir");
        let unborn = temp.path().join("projects").join("p1").join("live").join("r1");
        assert!(!unborn.exists());

        match free_disk_bytes(&unborn) {
            Some(free) => assert!(free > 0, "an existing volume reports some free space"),
            // Non-Windows builds answer "unknown" by design; callers must not
            // treat that as a refusal.
            None => assert!(!cfg!(windows), "windows must be able to answer"),
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

        let record = start_desktop_capture(&storage, "p1", "r1", "C:/tmp/p1/live/r1", timestamp)
            .expect("start desktop capture");
        assert_eq!(record.status, "recording");
        assert_eq!(record.segment_count, 0);

        let rows = genesis_adapter::query(
            &storage,
            "projects",
            &["id", "name"],
            vec![genesis_adapter::eq("projects", "id", serde_json::json!("p1"))],
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
}
