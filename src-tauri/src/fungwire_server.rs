//! Desktop FUNGWIRE LAN server: listens for paired mobile peers, performs a
//! paired-only Noise KK handshake, and (Task 7) hands the encrypted channel
//! off to the job loop that streams audio chunks and returns transcripts.
//!
//! Lifecycle mirrors `mobile::mobile_mcp_set_enabled` (bind on enable, poll
//! an `AtomicBool` every 40ms, `thread::spawn` the accept loop) with one
//! deliberate difference: each accepted connection gets its own thread
//! (Task 6 brief Step 2) instead of being handled inline, because a Noise
//! handshake + job loop can block far longer than the tiny HTTP-ish requests
//! the mobile gateway serves.

use crate::fungwire::{
    is_expired, manifest_hash, noise_responder, read_frame, write_frame, Control, NoiseChannel,
    Segment, CHUNK_MAX, CTRL_MAX, JOB_DIR_TTL,
};
use crate::{AppResult, AppState, WhisperOutput, WhisperRuntime};
use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, RecvTimeoutError},
    Arc,
};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::State;

/// How often a keepalive `Progress{stage:"transcribing"}` frame is sent to
/// the client while `transcribe.py` runs on its own thread (see
/// `receive_and_transcribe`'s doc comment). Must stay comfortably under the
/// mobile client's `set_read_timeout` (`fungwire_client::READ_TIMEOUT`, 60s)
/// so a socket that would otherwise sit silent for the whole transcription
/// never trips that timeout — the BLOCKING regression this module fixes: a
/// desktop that transcribes correctly but stays silent for >60s used to look
/// identical, from the client's point of view, to a dead connection.
const TRANSCRIBE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(20);

/// Cap on simultaneously-handled connections (final review #6a). Without
/// this, a flood of idle LAN sockets -- each accepted and handed its own
/// thread, then blocked for up to the 60s read timeout waiting for a Hello
/// frame that never arrives -- would spawn unbounded OS threads. 16 is far
/// more than one desktop legitimately serving a handful of paired phones
/// needs at once, while small enough that hitting it is cheap to recover
/// from (the rejected peer's client just retries).
const MAX_CONCURRENT_CONNECTIONS: usize = 16;

/// RAII decrement for the `connected_peers` counter that also serves as the
/// concurrency-cap semaphore (final review #6a). Held for the lifetime of a
/// spawned connection-handler thread's closure so the slot is freed on every
/// exit path -- a normal return, an early `?`/`return`, or a panic unwinding
/// out of `handle_connection` -- not just the happy path.
struct ConnectionSlotGuard(Arc<AtomicUsize>);

impl Drop for ConnectionSlotGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Handle kept in `AppState` while the server is running; dropping/replacing
/// it does nothing by itself — `stop` is what actually tells the accept-loop
/// thread to exit.
pub(crate) struct FungwireServerControl {
    pub(crate) bind: String,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) active_jobs: Arc<AtomicUsize>,
    pub(crate) connected_peers: Arc<AtomicUsize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FungwireStatus {
    pub(crate) enabled: bool,
    pub(crate) bind: Option<String>,
    pub(crate) active_jobs: usize,
    pub(crate) connected_peers: usize,
}

#[tauri::command]
pub(crate) fn fungwire_server_set_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<FungwireStatus> {
    let mut guard = state.fungwire.lock().expect("fungwire mutex poisoned");

    if !enabled {
        if let Some(control) = guard.take() {
            control.stop.store(true, Ordering::SeqCst);
        }
        return Ok(FungwireStatus {
            enabled: false,
            bind: None,
            active_jobs: 0,
            connected_peers: 0,
        });
    }

    if let Some(control) = guard.as_ref() {
        return Ok(FungwireStatus {
            enabled: true,
            bind: Some(control.bind.clone()),
            active_jobs: control.active_jobs.load(Ordering::SeqCst),
            connected_peers: control.connected_peers.load(Ordering::SeqCst),
        });
    }

    // Sweep orphaned job dirs left behind by clients that vanished mid-
    // transfer (app killed, dead socket retry budget exhausted) before
    // accepting any new connections (final review #4). Deliberately tied to
    // this explicit enable action rather than a background timer: the
    // server is only ever running while a user has it toggled on, so "each
    // time it starts" is the natural, simplest place to reclaim orphaned
    // disk space. Never fatal to enabling -- see the function's own doc.
    sweep_stale_job_dirs(&state.data_root, JOB_DIR_TTL);

    // Bind on all interfaces only because the server is being explicitly
    // enabled here; default (no control present) is unbound/off.
    let listener = TcpListener::bind("0.0.0.0:0")?;
    listener.set_nonblocking(true)?;
    let bind = listener.local_addr()?.to_string();

    let stop = Arc::new(AtomicBool::new(false));
    let active_jobs = Arc::new(AtomicUsize::new(0));
    let connected_peers = Arc::new(AtomicUsize::new(0));

    let loop_stop = stop.clone();
    let loop_jobs = active_jobs.clone();
    let loop_peers = connected_peers.clone();
    let storage = state.genesis.clone();
    let app_data = state.data_root.clone();
    let runtime = state.whisper_runtime_clone();

    thread::spawn(move || {
        while !loop_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    // Concurrency cap (final review #6a): reject outright,
                    // before spawning anything, once `connected_peers` (which
                    // already tracks in-flight connections end-to-end -- see
                    // its increment/decrement below) is at the max. This
                    // accept loop is the only place that ever increments it,
                    // so checking then incrementing here has no TOCTOU race
                    // even though decrements happen concurrently from
                    // handler threads.
                    if loop_peers.load(Ordering::SeqCst) >= MAX_CONCURRENT_CONNECTIONS {
                        drop(stream); // close immediately instead of spawning
                        continue;
                    }

                    // Per-connection thread (the fix vs. the mobile gateway's
                    // inline handling): a handshake + job loop can run for
                    // the duration of a whole transcription job, so it must
                    // never block the accept loop or other peers.
                    let storage = storage.clone();
                    let app_data = app_data.clone();
                    let runtime = runtime.clone();
                    let jobs = loop_jobs.clone();
                    let peers = loop_peers.clone();
                    peers.fetch_add(1, Ordering::SeqCst);
                    thread::spawn(move || {
                        // Guard, not a bare fetch_sub after the call: this
                        // must decrement on EVERY exit path, including a
                        // panic unwinding out of `handle_connection`, or the
                        // slot would leak and the cap would ratchet down to
                        // zero over time.
                        let _slot = ConnectionSlotGuard(peers);
                        if let Err(e) = handle_connection(stream, &storage, &app_data, &runtime, &jobs) {
                            eprintln!("fungwire connection ended: {e}");
                        }
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(40));
                }
                Err(_) => break,
            }
        }
    });

    *guard = Some(FungwireServerControl {
        bind: bind.clone(),
        stop,
        active_jobs,
        connected_peers,
    });

    Ok(FungwireStatus {
        enabled: true,
        bind: Some(bind),
        active_jobs: 0,
        connected_peers: 0,
    })
}

#[tauri::command]
pub(crate) fn fungwire_status(state: State<'_, AppState>) -> AppResult<FungwireStatus> {
    let guard = state.fungwire.lock().expect("fungwire mutex poisoned");
    Ok(match guard.as_ref() {
        Some(control) => FungwireStatus {
            enabled: true,
            bind: Some(control.bind.clone()),
            active_jobs: control.active_jobs.load(Ordering::SeqCst),
            connected_peers: control.connected_peers.load(Ordering::SeqCst),
        },
        None => FungwireStatus {
            enabled: false,
            bind: None,
            active_jobs: 0,
            connected_peers: 0,
        },
    })
}

/// Runs the full per-connection lifecycle: read the cleartext `Hello` (frame
/// 0), look the claimed device up in `paired_devices.db`, verify its public
/// key is bound to its stored fingerprint, run the Noise KK responder
/// handshake against that device's key, then hand off to the job loop.
///
/// Sending `device_id` in the clear before Noise starts is safe: it only
/// tells the responder *which* static key to expect next, it does not by
/// itself authenticate anything. A dishonest or confused caller can claim
/// any `device_id` it likes, but it can only complete the subsequent KK
/// handshake if it actually holds the private key matching that device's
/// registered public key — `read_message`/`into_transport_mode` fail
/// otherwise (see `fungwire::tests::noise_kk_rejects_wrong_remote_static`).
/// So identity is still proven cryptographically by Noise; the Hello frame
/// is only a routing hint.
pub(crate) fn handle_connection(
    mut stream: TcpStream,
    storage: &genesis_block_native::Storage,
    app_data: &Path,
    runtime: &WhisperRuntime,
    jobs: &Arc<AtomicUsize>,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .ok();

    // 1) Frame 0: cleartext Hello carrying the claimed device_id.
    let hello_frame = read_frame(&mut stream, CTRL_MAX).map_err(|e| e.to_string())?;
    let hello = Control::decode(&hello_frame).map_err(|e| e.to_string())?;
    let device_id = match hello {
        Control::Hello { device_id } => device_id,
        _ => return Err("expected Hello".into()),
    };

    // 2) Peer must be paired, not revoked, and have a published public key.
    let peer = crate::lookup_paired_peer(app_data, &device_id)
        .map_err(|e| e.to_string())?
        .ok_or("unknown or revoked peer")?;
    let peer_public_key = peer.public_key.as_deref().ok_or("peer has no public key")?;

    // Binding check: the key the peer will use in the handshake must hash to
    // the fingerprint recorded at pairing time.
    let raw = base64::engine::general_purpose::STANDARD
        .decode(peer_public_key.trim())
        .map_err(|e| e.to_string())?;
    let digest: String = Sha256::digest(&raw).iter().map(|b| format!("{b:02x}")).collect();
    if digest != peer.fingerprint {
        return Err("peer key/fingerprint mismatch".into());
    }

    // 3) Noise KK responder using our secret + the peer's X25519 key.
    let local =
        crate::device_identity::x25519_static_secret_in_dir(app_data).map_err(|e| e.to_string())?;
    let remote = crate::device_identity::x25519_public_from_ed25519_b64(peer_public_key)
        .map_err(|e| e.to_string())?;
    let mut hs = noise_responder(&local, &remote)?;
    let mut buf = [0u8; 4096];
    let m1 = read_frame(&mut stream, CTRL_MAX).map_err(|e| e.to_string())?;
    hs.read_message(&m1, &mut buf).map_err(|e| e.to_string())?;
    let n = hs.write_message(&[], &mut buf).map_err(|e| e.to_string())?;
    write_frame(&mut stream, &buf[..n]).map_err(|e| e.to_string())?;
    let transport = hs.into_transport_mode().map_err(|e| e.to_string())?;
    let mut channel = NoiseChannel::new(stream, transport);

    // 4) Hand off to the job loop (Task 7).
    run_job_loop(&mut channel, storage, app_data, runtime, &device_id, jobs)
}

/// A job outcome that must NOT become a `Control::Error` sent back to the
/// peer: the peer asked us to stop, so the correct response is silence — we
/// just tear the connection down. Distinguishing this from `Failed` is the
/// whole reason `receive_and_transcribe` doesn't return a plain
/// `Result<_, (String, String)>` like the handshake path does.
enum JobFailure {
    Cancelled,
    Failed(String, String),
}

/// Per-job directory that PERSISTS across connections (unlike a temp-dir
/// RAII guard that deletes on every drop): a mid-transfer disconnect / read
/// timeout must leave already-received segments on disk so a later
/// reconnect's `resume_from_seq` has something to load. Cleanup is therefore
/// explicit, not automatic — call [`cleanup_job_dir`] only at the specific
/// points in `run_job_loop` where the job has reached a state from which no
/// future connection can meaningfully resume: a `Result` was produced, a
/// terminal `Error` (see [`TERMINAL_JOB_ERROR_CODES`]), or a `Cancel`.
fn job_dir_path(app_data: &Path, job_id: &str) -> PathBuf {
    app_data.join("fungwire").join("jobs").join(job_id)
}

fn cleanup_job_dir(job_dir: &Path) {
    let _ = fs::remove_dir_all(job_dir);
}

/// Deletes every subdirectory of `<app_data>/fungwire/jobs/` whose mtime is
/// older than `ttl` (final review #4).
///
/// This is the disk-leak counterpart to [`cleanup_job_dir`]: that function
/// only ever runs when a connection reaches a *known* terminal outcome
/// (`Result`, a terminal `Error`, or `Cancel` -- see
/// [`TERMINAL_JOB_ERROR_CODES`]). A client that goes away for good mid-
/// transfer -- app killed, or its own retry budget exhausted against a dead
/// socket -- never produces any of those; the worker's outcome is
/// `transport_error`, which is deliberately left non-terminal so a real
/// reconnect can still resume. Nothing else ever revisits that directory, so
/// without this sweep its partial audio segments (privacy-adjacent user
/// data) would accumulate on disk forever.
///
/// A directory's own mtime is used as the "last activity" signal rather than
/// walking its files: every new segment written into it during a transfer
/// (`receive_and_transcribe`'s `fs::write(&seg_path, ...)`) creates a new
/// directory entry, which bumps the directory's own mtime on every mainstream
/// filesystem (NTFS included) -- so mtime tracks the most recent chunk
/// received, not merely the moment the directory was first created.
///
/// Deliberately non-fatal end to end: any I/O error (can't read the jobs
/// root, can't stat an entry, can't remove a stale one) is logged and
/// skipped rather than propagated, because this runs inline in
/// `fungwire_server_set_enabled` and must never block the server from
/// starting over a leftover-cleanup failure.
fn sweep_stale_job_dirs(app_data: &Path, ttl: Duration) {
    let jobs_root = app_data.join("fungwire").join("jobs");
    let entries = match fs::read_dir(&jobs_root) {
        Ok(entries) => entries,
        // Nothing to sweep yet (server has never received a job) -- not an
        // error worth logging.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            eprintln!(
                "fungwire: stale-job-dir sweep could not read {}: {e}",
                jobs_root.display()
            );
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("fungwire: stale-job-dir sweep could not read a dir entry: {e}");
                continue;
            }
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "fungwire: stale-job-dir sweep could not stat {}: {e}",
                    path.display()
                );
                continue;
            }
        };
        if !metadata.is_dir() {
            continue; // job dirs are directories; ignore stray files.
        }
        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "fungwire: stale-job-dir sweep could not read mtime of {}: {e}",
                    path.display()
                );
                continue;
            }
        };
        let age = match SystemTime::now().duration_since(modified) {
            Ok(age) => age,
            // mtime is in the future (clock skew) -- treat as freshly
            // touched rather than erroring.
            Err(_) => continue,
        };
        if is_expired(age, ttl) {
            match fs::remove_dir_all(&path) {
                Ok(()) => eprintln!("fungwire: swept stale job dir {}", path.display()),
                Err(e) => eprintln!(
                    "fungwire: stale-job-dir sweep could not remove {}: {e}",
                    path.display()
                ),
            }
        }
    }
}

/// `JobFailure::Failed` codes after which resuming from the persisted job
/// dir can never succeed, so it must be deleted immediately instead of kept
/// around for a reconnect: the manifest itself is inconsistent
/// (`manifest_mismatch`), a segment's bytes don't match its declared
/// checksum (`checksum_mismatch`), a chunk declared an impossible length
/// (`chunk_length`), transcription itself failed (`transcribe_failed`), or
/// the worker's on-disk state and the client's `resume_from_seq` have
/// diverged (`resume_gap` — see `receive_and_transcribe`'s preload loop).
/// Everything else (`transport_error`, `unexpected_control`, `bad_seq`,
/// `io_error`, ...) is treated as potentially transient: the directory is
/// left in place so the next connection can resume.
const TERMINAL_JOB_ERROR_CODES: &[&str] = &[
    "manifest_mismatch",
    "checksum_mismatch",
    "chunk_length",
    "transcribe_failed",
    "resume_gap",
];

/// Real job loop: exchanges `Heartbeat`/`JobStart`/`Cancel` control messages
/// with an already-authenticated peer until it disconnects. Only
/// `transcript.transcribe` is wired; any other `operation` gets a
/// `Control::Error{code:"unsupported_operation"}` and the loop keeps going
/// (the connection itself is still healthy — just that job request isn't).
///
/// `channel.recv_bytes(max)` calls anywhere in this module must keep
/// `max <= CHUNK_MAX` — `NoiseChannel::recv_bytes` sizes its scratch buffer
/// against that assumption (see fungwire.rs Task 4 review note).
pub(crate) fn run_job_loop(
    channel: &mut NoiseChannel<TcpStream>,
    _storage: &genesis_block_native::Storage,
    app_data: &Path,
    runtime: &WhisperRuntime,
    device_id: &str,
    jobs: &Arc<AtomicUsize>,
) -> Result<(), String> {
    eprintln!("fungwire job loop starting for device {device_id}");
    loop {
        let control = match channel.recv_control() {
            Ok(c) => c,
            Err(_) => return Ok(()), // peer closed the connection
        };
        match control {
            Control::Heartbeat => {
                channel.send(&Control::HeartbeatAck).map_err(|e| e.to_string())?;
            }
            Control::JobStart {
                job_id,
                operation,
                manifest_hash: expected_manifest_hash,
                segment_count,
                // `profile` (the mobile sender's guess at gpu/cpu) is
                // deliberately NOT threaded through to `receive_and_transcribe`
                // (Final review #5): the desktop is the one that actually runs
                // inference, so it must pick its own profile the same way
                // `import_and_transcribe` does, via `transcription_profile()`.
                // The field stays in the wire format (still decoded here) so
                // older/newer peers don't break on it; it's just unused.
                profile: _mobile_profile,
                checksums,
                resume_from_seq,
                ..
            } => {
                if operation != "transcript.transcribe" {
                    channel
                        .send(&Control::Error {
                            job_id,
                            code: "unsupported_operation".into(),
                            message: operation,
                        })
                        .ok();
                    continue;
                }

                let job_dir = job_dir_path(app_data, &job_id);
                jobs.fetch_add(1, Ordering::SeqCst);
                let outcome = receive_and_transcribe(
                    channel,
                    runtime,
                    &job_dir,
                    &job_id,
                    &expected_manifest_hash,
                    &checksums,
                    segment_count,
                    resume_from_seq,
                );
                jobs.fetch_sub(1, Ordering::SeqCst);

                match outcome {
                    Ok((duration_ms, segments)) => {
                        // The job is genuinely done regardless of whether
                        // this final send succeeds, so clean up before
                        // propagating a possible transport error from it —
                        // otherwise a dead peer that never reads the Result
                        // would leave a completed job's directory behind
                        // forever.
                        let send_result = channel.send(&Control::Result {
                            job_id: job_id.clone(),
                            duration_ms,
                            segments,
                        });
                        cleanup_job_dir(&job_dir);
                        send_result.map_err(|e| e.to_string())?;
                    }
                    Err(JobFailure::Cancelled) => {
                        cleanup_job_dir(&job_dir);
                        return Ok(());
                    }
                    Err(JobFailure::Failed(code, message)) => {
                        let terminal = TERMINAL_JOB_ERROR_CODES.contains(&code.as_str());
                        let send_result = channel.send(&Control::Error {
                            job_id: job_id.clone(),
                            code,
                            message,
                        });
                        if terminal {
                            cleanup_job_dir(&job_dir);
                        }
                        send_result.map_err(|e| e.to_string())?;
                    }
                }
            }
            Control::Cancel { .. } => return Ok(()),
            _ => { /* ignore unexpected control on the server */ }
        }
    }
}

/// Receives the not-yet-acked chunks for one job (`resume_from_seq..
/// segment_count`; segments `0..resume_from_seq` are reloaded from
/// `job_dir`, which persists across connections — see [`job_dir_path`]),
/// verifies everything against the manifest, transcribes each segment
/// separately, and returns the combined result. Cleanup of `job_dir` is the
/// caller's (`run_job_loop`'s) responsibility, not this function's — see
/// [`TERMINAL_JOB_ERROR_CODES`] for which outcomes warrant it.
///
/// ## Resume (`resume_from_seq`)
/// The client computes `resume_from_seq = last_acked_seq + 1` and, on a
/// reconnect, sends `Chunk`s only for `seq >= resume_from_seq` (see
/// `fungwire_client::attempt_transfer`). So that the worker's expectations
/// match, segments `0..resume_from_seq` are loaded back from
/// `job_dir/segment-<seq>.m4a` (written by a *previous* connection's call to
/// this function against the same `job_id`) and re-verified against
/// `checksums[seq]` before the receive loop below ever runs. If any of those
/// files is missing or fails its checksum, the worker's persisted state and
/// the client's `resume_from_seq` have diverged — there is no safe partial
/// recovery, so this returns `Error{code:"resume_gap"}` and the client is
/// expected to restart the whole job from seq 0 (see
/// `fungwire_client::run_transfer`'s handling of that code).
///
/// ## Chunk reassembly
/// The sender splits each segment into ≤`NOISE_MAX_PLAINTEXT`-byte
/// sub-frames (a Noise transport message tops out at 65535 bytes including
/// its AEAD tag, far under a ~80 KB 5s AAC segment). For each `Chunk{seq,
/// len}` control message, `len` is the *full* segment byte length; we then
/// call `channel.recv_bytes` in a loop, appending each returned sub-frame,
/// until the accumulator reaches exactly `len` bytes. A single `Chunk`
/// therefore maps to one control-message read plus one-or-more
/// `recv_bytes` reads.
///
/// ## Manifest verification (defense in depth, two checks)
/// 1. Up front: `manifest_hash(&checksums) == JobStart.manifest_hash`. This
///    catches a `JobStart` whose own declared fields are already
///    inconsistent, before any transfer or disk work happens.
/// 2. After all segments arrive: recompute `manifest_hash` over the
///    checksums we actually *received* (in seq order) and compare again.
///    Combined with the per-segment `sha256(acc) == checksums[seq]` check
///    below, this is redundant when everything is honest, but it means a
///    bug that skips the per-segment check still gets caught here.
///
/// Integrity note: the per-segment checksums and manifest_hash both originate
/// from the sender's JobStart. These checks detect transport
/// corruption/truncation/reordering, NOT adversarial tampering by the peer —
/// the peer is already mutually authenticated as a paired device by the
/// Noise KK handshake, which is the trust boundary. Do not rely on this
/// manifest as a content-authenticity anchor against a malicious peer.
///
/// ## Transcription (single invocation, model loaded once, on its own thread)
/// m4a segments cannot be byte-concatenated into one file, but they no
/// longer need to be transcribed one subprocess per segment either (Final
/// review #3): all ordered segment paths are passed to a *single*
/// `run_python_worker(transcribe.py)` call, so `WhisperModel(...)` is
/// constructed exactly once for the whole job instead of once per segment
/// (~720 reloads for a 1-hour recording at 5s segments). `transcribe.py`
/// transcribes each path in order and offsets each file's segments by the
/// cumulative REAL duration of the files before it, so the worker here just
/// parses the single combined `WhisperOutput` and maps its `segments`
/// straight to `fungwire::Segment` — no per-segment offsetting on this side.
/// The profile is the desktop's own choice (see `transcription_profile`),
/// never the mobile-sent `JobStart.profile` (Final review #5): a GPU desktop
/// must actually use its GPU regardless of what the sending phone guessed.
///
/// Segment paths are handed to `transcribe.py` via a newline-delimited
/// manifest file (`<job_dir>/segments.txt`, `--manifest <path>`), never as
/// positional argv (Important fix): a medium-or-longer recording can produce
/// hundreds of segments, and one argv entry per path risks overflowing the
/// ~32KB Windows command-line length limit.
///
/// `run_python_worker` blocks for the whole transcription, which for a real
/// recording can easily exceed the mobile client's 60s socket read timeout.
/// Since the *main* thread here is the only thing holding `channel` (a
/// `NoiseChannel` is not `Clone`/shareable), the call runs on a **separate**
/// `std::thread` instead, forwarding each `PROGRESS <pct>` line from
/// `transcribe.py` into an `mpsc::Sender<i64>`. The main thread then loops on
/// `progress_rx.recv_timeout(TRANSCRIBE_KEEPALIVE_INTERVAL)`, sending a
/// `Progress{stage:"transcribing"}` frame on every real update *and* on every
/// timeout tick (re-sending the last known percent as a pure keepalive) —
/// guaranteeing a frame at least every `TRANSCRIBE_KEEPALIVE_INTERVAL`
/// (well under the client's 60s read timeout) regardless of how sparsely
/// `transcribe.py` reports progress. The loop exits on
/// `RecvTimeoutError::Disconnected` (the worker thread's `Sender` was
/// dropped, i.e. it finished), after which the worker's `JoinHandle` is
/// joined for the actual `Result<String, String>`. A final
/// `Progress{stage:"transcribing", percent:100}` is sent once parsing
/// succeeds, as a definitive "done" marker independent of whatever the last
/// worker-reported percent happened to be.
#[allow(clippy::too_many_arguments)]
fn receive_and_transcribe(
    channel: &mut NoiseChannel<TcpStream>,
    runtime: &WhisperRuntime,
    job_dir: &Path,
    job_id: &str,
    expected_manifest_hash: &str,
    checksums: &[String],
    segment_count: u32,
    resume_from_seq: u32,
) -> Result<(i64, Vec<Segment>), JobFailure> {
    if checksums.len() != segment_count as usize {
        return Err(JobFailure::Failed(
            "manifest_mismatch".into(),
            format!(
                "JobStart declared {segment_count} segments but {} checksums",
                checksums.len()
            ),
        ));
    }
    if manifest_hash(checksums) != expected_manifest_hash {
        return Err(JobFailure::Failed(
            "manifest_mismatch".into(),
            "JobStart checksums do not hash to its own manifest_hash".into(),
        ));
    }
    if resume_from_seq > segment_count {
        return Err(JobFailure::Failed(
            "resume_gap".into(),
            format!("resume_from_seq {resume_from_seq} exceeds segment_count {segment_count}"),
        ));
    }

    fs::create_dir_all(job_dir)
        .map_err(|e| JobFailure::Failed("io_error".into(), format!("job dir: {e}")))?;

    let mut received_checksums: Vec<Option<String>> = vec![None; segment_count as usize];
    let mut seg_paths: Vec<Option<PathBuf>> = vec![None; segment_count as usize];

    // Preload segments 0..resume_from_seq from the persistent job dir (see
    // the doc comment above): these were already accepted and acked on a
    // prior connection, so the client will not resend them.
    for seq in 0..resume_from_seq {
        let idx = seq as usize;
        let seg_path = job_dir.join(format!("segment-{seq}.m4a"));
        let bytes = match fs::read(&seg_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(JobFailure::Failed(
                    "resume_gap".into(),
                    format!("segment {seq} missing from persisted job dir ({e}); client must restart from seq 0"),
                ))
            }
        };
        let digest: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if checksums[idx] != digest {
            return Err(JobFailure::Failed(
                "resume_gap".into(),
                format!("segment {seq} on disk does not match manifest checksum; client must restart from seq 0"),
            ));
        }
        received_checksums[idx] = Some(digest);
        seg_paths[idx] = Some(seg_path);
    }

    for _ in resume_from_seq..segment_count {
        let control = channel
            .recv_control()
            .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;
        match control {
            Control::Chunk { seq, len, .. } => {
                let idx = seq as usize;
                if idx >= segment_count as usize {
                    return Err(JobFailure::Failed(
                        "bad_seq".into(),
                        format!("seq {seq} is out of range for segment_count {segment_count}"),
                    ));
                }

                // Bound the peer-declared length before allocating anything:
                // `len` comes straight off the wire from `Control::Chunk`, and
                // without this check a compromised-but-authenticated peer
                // could send `len: u32::MAX` and force a ~4GB allocation here
                // (OOM/abort) before a single byte is even read. A real audio
                // segment is a few hundred KB, far under CHUNK_MAX (4 MiB), so
                // this rejects only impossible/hostile lengths.
                if len as usize > CHUNK_MAX {
                    return Err(JobFailure::Failed(
                        "chunk_length".into(),
                        format!("segment {seq} declared len {len} exceeds cap {CHUNK_MAX}"),
                    ));
                }

                // Reassemble: len is the full segment length; keep pulling
                // Noise sub-frames (each <= NOISE_MAX_PLAINTEXT) until we
                // have exactly that many bytes.
                let mut acc = Vec::with_capacity(len as usize);
                while acc.len() < len as usize {
                    let part = channel
                        .recv_bytes(CHUNK_MAX)
                        .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;
                    acc.extend_from_slice(&part);
                }
                if acc.len() != len as usize {
                    return Err(JobFailure::Failed(
                        "chunk_length".into(),
                        format!("seg {seq}: expected {len} bytes, reassembled {}", acc.len()),
                    ));
                }

                let digest: String = Sha256::digest(&acc)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                if checksums[idx] != digest {
                    return Err(JobFailure::Failed(
                        "checksum_mismatch".into(),
                        format!("seg {seq}"),
                    ));
                }

                let seg_path = job_dir.join(format!("segment-{seq}.m4a"));
                fs::write(&seg_path, &acc)
                    .map_err(|e| JobFailure::Failed("io_error".into(), format!("seg {seq}: {e}")))?;
                received_checksums[idx] = Some(digest);
                seg_paths[idx] = Some(seg_path);

                channel
                    .send(&Control::ChunkAck {
                        job_id: job_id.to_string(),
                        seq,
                    })
                    .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;

                let received_so_far = received_checksums.iter().filter(|c| c.is_some()).count();
                let percent = ((received_so_far as f64 / segment_count as f64) * 100.0) as u8;
                channel
                    .send(&Control::Progress {
                        job_id: job_id.to_string(),
                        percent,
                        stage: "receiving".into(),
                    })
                    .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;
            }
            Control::Cancel { .. } => return Err(JobFailure::Cancelled),
            other => {
                return Err(JobFailure::Failed(
                    "unexpected_control".into(),
                    format!("expected Chunk while receiving, got {other:?}"),
                ))
            }
        }
    }

    let ordered_checksums: Vec<String> = match received_checksums.into_iter().collect::<Option<Vec<String>>>() {
        Some(v) => v,
        None => {
            return Err(JobFailure::Failed(
                "manifest_mismatch".into(),
                "one or more segments were never received".into(),
            ))
        }
    };
    if manifest_hash(&ordered_checksums) != expected_manifest_hash {
        return Err(JobFailure::Failed(
            "manifest_mismatch".into(),
            "received segments do not hash to JobStart.manifest_hash".into(),
        ));
    }
    let seg_paths: Vec<PathBuf> = seg_paths.into_iter().flatten().collect();

    // Desktop picks its own profile -- never the mobile-sent
    // `JobStart.profile` (Final review #5). This is the same helper
    // `import_and_transcribe`'s `run_transcription` uses.
    let profile = crate::transcription_profile().map_err(|e| {
        JobFailure::Failed(
            "transcribe_failed".into(),
            format!("could not determine transcription profile: {e}"),
        )
    })?;

    // Single invocation for the whole job (Final review #3): all ordered
    // segment paths go to one `run_python_worker` call so the Whisper model
    // is loaded exactly once, not once per segment. `transcribe.py` does its
    // own cross-file offsetting by real duration, so the result here is
    // already one continuous, correctly-timed segment list.
    //
    // Paths go through a manifest file, not positional argv (Important fix
    // -- see this function's doc comment): a several-hundred-segment job can
    // overflow the ~32KB Windows command-line limit if every path is passed
    // positionally.
    let manifest_path = job_dir.join("segments.txt");
    let manifest_contents = seg_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&manifest_path, manifest_contents).map_err(|e| {
        JobFailure::Failed("io_error".into(), format!("segment manifest: {e}"))
    })?;
    let manifest_path_string = manifest_path.to_string_lossy().to_string();
    let worker_args: Vec<String> = vec![
        "--manifest".to_string(),
        manifest_path_string,
        "--profile".to_string(),
        profile,
    ];

    // Run the (possibly long) transcription on its own thread so the main
    // thread -- which owns `channel`, the only handle to the socket -- stays
    // free to send keepalive Progress frames throughout (BLOCKING fix; see
    // this function's doc comment). `on_progress` forwards each
    // `PROGRESS <pct>` line into `progress_tx`; `progress_tx` (and therefore
    // the channel) is dropped when the worker thread's closure ends, which
    // is how the main loop below detects completion
    // (`RecvTimeoutError::Disconnected`).
    let (progress_tx, progress_rx) = mpsc::channel::<i64>();
    let worker_runtime = runtime.clone();
    let worker_script = runtime.script.clone();
    let worker_handle: thread::JoinHandle<Result<String, String>> = thread::spawn(move || {
        let arg_refs: Vec<&str> = worker_args.iter().map(String::as_str).collect();
        crate::run_python_worker(&worker_runtime, &worker_script, &arg_refs, None, move |pct| {
            let _ = progress_tx.send(pct);
        })
    });

    let mut last_pct: u8 = 0;
    loop {
        let tick = progress_rx.recv_timeout(TRANSCRIBE_KEEPALIVE_INTERVAL);
        match tick {
            Ok(pct) => last_pct = pct.clamp(0, 100) as u8,
            Err(RecvTimeoutError::Timeout) => { /* keepalive: re-send last_pct below */ }
            Err(RecvTimeoutError::Disconnected) => break, // worker thread is done
        }
        // A send failure here means the client actually disconnected (not
        // just a slow transcription) -- surface it as a transport error
        // instead of panicking. The job dir is left in place (transport
        // errors are not in `TERMINAL_JOB_ERROR_CODES`) so a reconnect can
        // resume: since every segment was already received, the resumed
        // connection will re-run this transcription step from the persisted
        // segments rather than re-receiving anything.
        channel
            .send(&Control::Progress {
                job_id: job_id.to_string(),
                percent: last_pct,
                stage: "transcribing".into(),
            })
            .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;
    }

    let raw = worker_handle
        .join()
        .map_err(|_| {
            JobFailure::Failed(
                "transcribe_failed".into(),
                "transcription worker thread panicked".into(),
            )
        })?
        .map_err(|e| JobFailure::Failed("transcribe_failed".into(), e))?;
    let output: WhisperOutput = serde_json::from_str(raw.trim()).map_err(|e| {
        JobFailure::Failed(
            "transcribe_failed".into(),
            format!("bad worker output: {e}"),
        )
    })?;

    let segments: Vec<Segment> = output
        .segments
        .into_iter()
        .map(|seg| Segment {
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            text: seg.text,
            confidence: seg.confidence,
        })
        .collect();

    // Definitive "done" marker, independent of whatever the last
    // worker-reported (or keepalive-repeated) percent happened to be.
    channel
        .send(&Control::Progress {
            job_id: job_id.to_string(),
            percent: 100,
            stage: "transcribing".into(),
        })
        .map_err(|e| JobFailure::Failed("transport_error".into(), e.to_string()))?;

    Ok((output.duration_ms, segments))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_identity::{
        ensure_identity_in_dir, public_key_b64_in_dir, x25519_public_from_ed25519_b64,
        x25519_static_secret_in_dir,
    };
    use crate::fungwire::noise_initiator;
    use crate::{paired_devices_connection_at, upsert_paired_device, PairedDeviceInput};
    use genesis_block_native::{OpenOptions, Storage};
    use std::net::TcpListener;
    use uuid::Uuid;

    fn open_genesis() -> (std::path::PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fungwire-server-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .expect("open GenesisBlockDB");
        (path, storage)
    }

    /// A `WhisperRuntime` pointed at the real venv python interpreter (so
    /// `run_python_worker`'s existence checks pass and the subprocess
    /// plumbing is genuinely exercised) but at `tests/fixtures/fake_transcribe.py`
    /// instead of the real faster-whisper pipeline, so tests don't need a
    /// model download or GPU/CPU inference to run.
    fn test_whisper_runtime() -> WhisperRuntime {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let python = manifest_dir
            .parent()
            .expect("src-tauri has a parent")
            .join(".venv-whisper")
            .join("Scripts")
            .join("python.exe");
        let script = manifest_dir
            .join("tests")
            .join("fixtures")
            .join("fake_transcribe.py");
        WhisperRuntime::for_test(python, script)
    }

    /// Registers `device_id` as a paired, non-revoked device in the server's
    /// `paired_devices.db`, using the client identity's real published
    /// ed25519 key so the binding check (sha256(pubkey) == fingerprint)
    /// passes.
    fn pair_device(server_app_data: &Path, device_id: &str, client_app_data: &Path) {
        let pub_b64 = public_key_b64_in_dir(client_app_data).unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&pub_b64)
            .unwrap();
        let fingerprint: String = Sha256::digest(&raw).iter().map(|b| format!("{b:02x}")).collect();
        let conn = paired_devices_connection_at(server_app_data).unwrap();
        upsert_paired_device(
            &conn,
            PairedDeviceInput {
                id: device_id.to_string(),
                name: "Test Client".to_string(),
                platform: "test".to_string(),
                fingerprint,
                pairing_session_id: "sess-1".to_string(),
                public_key: Some(pub_b64),
            },
        )
        .unwrap();
    }

    #[test]
    fn paired_peer_completes_handshake_to_transport_mode() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-a", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));

        let server_app_data_path = server_app_data.path().to_path_buf();
        let runtime = test_whisper_runtime();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        // Minimal initiator using Task 3/4 helpers directly (Task 8 builds
        // the real client). Drives Hello + Noise KK from the other side.
        let mut client_stream = TcpStream::connect(addr).unwrap();
        write_frame(
            &mut client_stream,
            &Control::Hello {
                device_id: "device-a".into(),
            }
            .encode(),
        )
        .unwrap();

        let client_secret = x25519_static_secret_in_dir(client_app_data.path()).unwrap();
        let server_pub_b64 = public_key_b64_in_dir(server_app_data.path()).unwrap();
        let server_remote = x25519_public_from_ed25519_b64(&server_pub_b64).unwrap();
        let mut ini = noise_initiator(&client_secret, &server_remote).unwrap();
        let mut buf = [0u8; 4096];
        let n = ini.write_message(&[], &mut buf).unwrap();
        write_frame(&mut client_stream, &buf[..n]).unwrap();
        let msg2 = read_frame(&mut client_stream, CTRL_MAX).unwrap();
        let mut rbuf = [0u8; 4096];
        ini.read_message(&msg2, &mut rbuf).unwrap();
        ini.into_transport_mode()
            .expect("initiator must reach transport mode");

        // This test only cares that the handshake reaches transport mode;
        // it never speaks the job protocol. Close the socket so the real
        // `run_job_loop`'s first `recv_control()` sees the peer hang up
        // (-> `Ok(())`) instead of blocking on the 60s read timeout.
        drop(client_stream);

        let result = server_thread.join().unwrap();
        assert!(result.is_ok(), "handshake should complete: {result:?}");
    }

    #[test]
    fn unpaired_device_id_is_rejected() {
        let server_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        // Deliberately not registering "device-x" in paired_devices.db.

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));

        let server_app_data_path = server_app_data.path().to_path_buf();
        let runtime = test_whisper_runtime();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        let mut client_stream = TcpStream::connect(addr).unwrap();
        write_frame(
            &mut client_stream,
            &Control::Hello {
                device_id: "device-x".into(),
            }
            .encode(),
        )
        .unwrap();

        let result = server_thread.join().unwrap();
        assert!(
            result.is_err(),
            "unpaired device must be rejected before/at handshake"
        );
    }

    /// Drives a full `Hello` + Noise KK handshake as the initiator (same as
    /// `paired_peer_completes_handshake_to_transport_mode`) and returns a
    /// `NoiseChannel` ready to speak the job protocol.
    fn initiator_handshake(
        client_stream: TcpStream,
        device_id: &str,
        client_app_data: &Path,
        server_app_data: &Path,
    ) -> NoiseChannel<TcpStream> {
        let mut client_stream = client_stream;
        write_frame(
            &mut client_stream,
            &Control::Hello {
                device_id: device_id.to_string(),
            }
            .encode(),
        )
        .unwrap();

        let client_secret = x25519_static_secret_in_dir(client_app_data).unwrap();
        let server_pub_b64 = public_key_b64_in_dir(server_app_data).unwrap();
        let server_remote = x25519_public_from_ed25519_b64(&server_pub_b64).unwrap();
        let mut ini = noise_initiator(&client_secret, &server_remote).unwrap();
        let mut buf = [0u8; 4096];
        let n = ini.write_message(&[], &mut buf).unwrap();
        write_frame(&mut client_stream, &buf[..n]).unwrap();
        let msg2 = read_frame(&mut client_stream, CTRL_MAX).unwrap();
        let mut rbuf = [0u8; 4096];
        ini.read_message(&msg2, &mut rbuf).unwrap();
        let transport = ini
            .into_transport_mode()
            .expect("initiator must reach transport mode");
        NoiseChannel::new(client_stream, transport)
    }

    /// Reads controls from `channel` until a `Progress{stage:"transcribing",
    /// percent:100}` is seen -- the definitive completion marker
    /// `receive_and_transcribe` always sends last, regardless of whatever
    /// interim percents the worker thread reported -- collecting every
    /// transcribing percent observed along the way (in order, including the
    /// final 100). Panics if a non-transcribing-Progress control shows up
    /// first, since nothing else is expected on the wire between the last
    /// "receiving" Progress and the eventual `Result`.
    fn recv_transcribing_progress_until_done(channel: &mut NoiseChannel<TcpStream>) -> Vec<u8> {
        let mut percents = Vec::new();
        loop {
            match channel.recv_control().unwrap() {
                Control::Progress { stage, percent, .. } => {
                    assert_eq!(stage, "transcribing");
                    percents.push(percent);
                    if percent == 100 {
                        return percents;
                    }
                }
                other => panic!("expected transcribing Progress, got {other:?}"),
            }
        }
    }

    /// Dedicated proof for the BLOCKING-regression fix: `receive_and_transcribe`
    /// must stream real `transcribe.py` progress to the client as it happens
    /// (via the worker-thread + `mpsc` handoff), not just discard it and send
    /// a single 100% at the very end (which is what silently regressed when
    /// the single-invocation fix passed a no-op `|_pct| {}` closure and left
    /// the socket dark for the whole transcription). `fake_transcribe.py`
    /// emits `PROGRESS 30` then, after a short (~200ms, far under
    /// `TRANSCRIBE_KEEPALIVE_INTERVAL`'s 20s) sleep, `PROGRESS 70` -- so a
    /// correct implementation must deliver at least one "transcribing"
    /// `Progress` with `percent < 100` to the client before the final 100%
    /// and `Result`, proving the socket carries live progress rather than
    /// sitting silent.
    #[test]
    fn transcribing_progress_is_streamed_before_result() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-progress", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));
        let runtime = test_whisper_runtime();

        let server_app_data_path = server_app_data.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        let client_stream = TcpStream::connect(addr).unwrap();
        let mut channel = initiator_handshake(
            client_stream,
            "device-progress",
            client_app_data.path(),
            server_app_data.path(),
        );

        let seg_bytes = vec![9u8; 1024];
        let checksum: String = Sha256::digest(&seg_bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let checksums = vec![checksum];
        let manifest = manifest_hash(&checksums);

        channel
            .send(&Control::JobStart {
                job_id: "job-progress".into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest,
                segment_count: 1,
                total_bytes: seg_bytes.len() as u64,
                profile: "cpu".into(),
                resume_from_seq: 0,
                checksums,
            })
            .unwrap();
        channel
            .send(&Control::Chunk {
                job_id: "job-progress".into(),
                seq: 0,
                len: seg_bytes.len() as u32,
            })
            .unwrap();
        channel.send_bytes(&seg_bytes).unwrap();

        match channel.recv_control().unwrap() {
            Control::ChunkAck { seq, .. } => assert_eq!(seq, 0),
            other => panic!("expected ChunkAck, got {other:?}"),
        }
        match channel.recv_control().unwrap() {
            Control::Progress { stage, .. } => assert_eq!(stage, "receiving"),
            other => panic!("expected receiving Progress, got {other:?}"),
        }

        let transcribing_percents = recv_transcribing_progress_until_done(&mut channel);
        assert!(
            transcribing_percents.iter().any(|&p| p < 100),
            "expected the client to observe at least one transcribing Progress \
             below 100% before the final marker (i.e. progress streamed live, \
             not just sent once at the end): {transcribing_percents:?}"
        );

        match channel.recv_control().unwrap() {
            Control::Result { job_id, .. } => assert_eq!(job_id, "job-progress"),
            other => panic!("expected Result, got {other:?}"),
        }

        drop(channel);
        let result = server_thread.join().unwrap();
        assert!(result.is_ok(), "job loop should exit cleanly: {result:?}");
    }

    /// End-to-end job loop test over a real loopback `TcpStream` pair: a
    /// `JobStart` for one segment whose byte length (70,000) deliberately
    /// exceeds `NOISE_MAX_PLAINTEXT` (65,519), split by the test into two
    /// sub-frames on the wire — this is what actually exercises
    /// `receive_and_transcribe`'s reassembly loop (a single sub-frame would
    /// never prove the accumulate-until-`len` logic does anything). The
    /// worker is pointed at `fake_transcribe.py` so the test doesn't need a
    /// real model, and asserts the returned `Result` segment matches the
    /// fixture's fixed output.
    #[test]
    fn job_loop_reassembles_multi_subframe_chunk_and_returns_transcript() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-job", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));
        let runtime = test_whisper_runtime();

        let server_app_data_path = server_app_data.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        let client_stream = TcpStream::connect(addr).unwrap();
        let mut channel = initiator_handshake(
            client_stream,
            "device-job",
            client_app_data.path(),
            server_app_data.path(),
        );

        // 70,000 bytes > NOISE_MAX_PLAINTEXT (65,519): the sender must split
        // this into >= 2 Noise sub-frames, and the server must reassemble
        // them before it can even compute the checksum, let alone match it.
        let seg_bytes: Vec<u8> = (0..70_000u32).map(|i| (i % 256) as u8).collect();
        assert!(seg_bytes.len() > crate::fungwire::NOISE_MAX_PLAINTEXT);
        let checksum: String = Sha256::digest(&seg_bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let checksums = vec![checksum];
        let manifest = manifest_hash(&checksums);

        channel
            .send(&Control::JobStart {
                job_id: "job-1".into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest.clone(),
                segment_count: 1,
                total_bytes: seg_bytes.len() as u64,
                profile: "cpu".into(),
                resume_from_seq: 0,
                checksums,
            })
            .unwrap();
        channel
            .send(&Control::Chunk {
                job_id: "job-1".into(),
                seq: 0,
                len: seg_bytes.len() as u32,
            })
            .unwrap();
        for part in seg_bytes.chunks(crate::fungwire::NOISE_MAX_PLAINTEXT) {
            channel.send_bytes(part).unwrap();
        }

        match channel.recv_control().unwrap() {
            Control::ChunkAck { seq, .. } => assert_eq!(seq, 0),
            other => panic!("expected ChunkAck, got {other:?}"),
        }
        match channel.recv_control().unwrap() {
            Control::Progress { stage, .. } => assert_eq!(stage, "receiving"),
            other => panic!("expected receiving Progress, got {other:?}"),
        }
        // Transcription now runs on a separate thread while the main thread
        // streams progress (BLOCKING fix), so the fixture's two interim
        // `PROGRESS` lines (30, 70) arrive as their own "transcribing"
        // frames before the definitive 100% completion marker -- not just a
        // single "transcribing -> 100" transition.
        let transcribing_percents = recv_transcribing_progress_until_done(&mut channel);
        assert!(
            transcribing_percents.iter().any(|&p| p < 100),
            "expected at least one interim transcribing Progress, got {transcribing_percents:?}"
        );
        assert_eq!(*transcribing_percents.last().unwrap(), 100);
        match channel.recv_control().unwrap() {
            Control::Result {
                job_id,
                duration_ms,
                segments,
            } => {
                assert_eq!(job_id, "job-1");
                assert_eq!(duration_ms, 1000);
                assert_eq!(segments.len(), 1);
                assert_eq!(segments[0].start_ms, 0);
                assert_eq!(segments[0].end_ms, 1000);
                assert_eq!(segments[0].text, "hi");
            }
            other => panic!("expected Result, got {other:?}"),
        }

        drop(channel);
        let result = server_thread.join().unwrap();
        assert!(result.is_ok(), "job loop should exit cleanly: {result:?}");
    }

    /// A `Chunk` whose peer-declared `len` exceeds `CHUNK_MAX` must be
    /// rejected with `Control::Error{code:"chunk_length"}` *before* the
    /// reassembly buffer is allocated — i.e. the server must never attempt
    /// `Vec::with_capacity(len)` for a hostile `len`. We never send any body
    /// bytes for this chunk: if the server tried to `recv_bytes` waiting for
    /// `len` bytes instead of rejecting up front, this test would hang/time
    /// out rather than pass, which is what proves the check runs
    /// pre-allocation and pre-read.
    #[test]
    fn oversized_chunk_len_is_rejected_before_allocation() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-oversized", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));
        let runtime = test_whisper_runtime();

        let server_app_data_path = server_app_data.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        let client_stream = TcpStream::connect(addr).unwrap();
        let mut channel = initiator_handshake(
            client_stream,
            "device-oversized",
            client_app_data.path(),
            server_app_data.path(),
        );

        // The checksum value itself is never reached (the len check fires
        // first), so any well-formed placeholder is fine.
        let checksums = vec!["0".repeat(64)];
        let manifest = manifest_hash(&checksums);
        let bogus_len = (crate::fungwire::CHUNK_MAX as u32).saturating_add(1);

        channel
            .send(&Control::JobStart {
                job_id: "job-oversized".into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest,
                segment_count: 1,
                total_bytes: bogus_len as u64,
                profile: "cpu".into(),
                resume_from_seq: 0,
                checksums,
            })
            .unwrap();
        channel
            .send(&Control::Chunk {
                job_id: "job-oversized".into(),
                seq: 0,
                len: bogus_len,
            })
            .unwrap();
        // Deliberately send no body bytes: a correct server rejects based on
        // `len` alone, without ever calling `recv_bytes`.

        match channel.recv_control().unwrap() {
            Control::Error { job_id, code, .. } => {
                assert_eq!(job_id, "job-oversized");
                assert_eq!(code, "chunk_length");
            }
            other => panic!("expected Error{{code: chunk_length}}, got {other:?}"),
        }

        drop(channel);
        let result = server_thread.join().unwrap();
        assert!(
            result.is_ok(),
            "job loop should send Error and exit cleanly: {result:?}"
        );

        let job_dir = job_dir_path(server_app_data.path(), "job-oversized");
        assert!(
            !job_dir.exists(),
            "job dir must be cleaned up after an oversized-chunk rejection (terminal error)"
        );
    }

    /// A `Cancel` sent instead of the expected `Chunk` mid-transfer must end
    /// the job loop (`run_job_loop` returns `Ok(())`, not `Control::Error`)
    /// and must leave no temp directory behind for the job.
    #[test]
    fn cancel_mid_transfer_ends_job_loop_and_cleans_temp_dir() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-cancel", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));
        let runtime = test_whisper_runtime();

        let server_app_data_path = server_app_data.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        let client_stream = TcpStream::connect(addr).unwrap();
        let mut channel = initiator_handshake(
            client_stream,
            "device-cancel",
            client_app_data.path(),
            server_app_data.path(),
        );

        let seg_bytes = vec![7u8; 1024];
        let checksum: String = Sha256::digest(&seg_bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let checksums = vec![checksum];
        let manifest = manifest_hash(&checksums);

        channel
            .send(&Control::JobStart {
                job_id: "job-cancel".into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest,
                segment_count: 1,
                total_bytes: seg_bytes.len() as u64,
                profile: "cpu".into(),
                resume_from_seq: 0,
                checksums,
            })
            .unwrap();
        // Cancel instead of sending the Chunk the server is waiting for.
        channel
            .send(&Control::Cancel {
                job_id: "job-cancel".into(),
            })
            .unwrap();

        drop(channel);
        let result = server_thread.join().unwrap();
        assert!(
            result.is_ok(),
            "cancel mid-transfer should end the loop cleanly: {result:?}"
        );

        let job_dir = job_dir_path(server_app_data.path(), "job-cancel");
        assert!(
            !job_dir.exists(),
            "job dir must be cleaned up after a mid-transfer cancel"
        );
    }

    /// True K>0 resume across a real reconnect: the first connection is
    /// driven (by hand, as initiator) through `JobStart{resume_from_seq: 0}`
    /// and Chunks for segments 0 and 1 — so `last_acked_seq` genuinely
    /// reaches 1 (K=1), not -1 — and is then dropped *before* segment 2 is
    /// sent, exercising the real `receive_and_transcribe` on a live
    /// `TcpStream` via `handle_connection`. That first connection must end
    /// in an error (its peer vanished mid-wait for the next Chunk) but must
    /// leave segments 0 and 1 persisted under the job's directory. A second
    /// connection then resumes with `JobStart{resume_from_seq: 2}` against
    /// the *same* `job_id` and `app_data`: the worker must reload segments 0
    /// and 1 from disk (re-verifying their checksums), receive only segment
    /// 2 over the wire, and still produce a `Result` covering all three
    /// segments — proving the persistent per-job directory survives a
    /// connection drop and `resume_from_seq` is honored end-to-end, not
    /// ignored. Segment 1 is 70,000 bytes (> `NOISE_MAX_PLAINTEXT`), so both
    /// the original receive and the resumed reload must handle a
    /// multi-sub-frame / on-disk segment correctly.
    #[test]
    fn resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-resume-k", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let job_id = "job-resume-k";

        let checksum_of = |bytes: &[u8]| -> String {
            Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
        };
        let seg0: Vec<u8> = (0..1024u32).map(|i| (i % 256) as u8).collect();
        let seg1: Vec<u8> = (0..70_000u32).map(|i| ((i * 3) % 256) as u8).collect();
        let seg2: Vec<u8> = (0..2048u32).map(|i| ((i * 7) % 256) as u8).collect();
        assert!(seg1.len() > crate::fungwire::NOISE_MAX_PLAINTEXT);
        let checksums = vec![checksum_of(&seg0), checksum_of(&seg1), checksum_of(&seg2)];
        let manifest = manifest_hash(&checksums);

        let server_app_data_path = server_app_data.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            // First connection: real `handle_connection`, but the peer
            // vanishes before segment 2, so this must end in an error while
            // leaving the job dir's already-accepted segments on disk.
            let (_genesis_path1, storage1) = open_genesis();
            let jobs1 = Arc::new(AtomicUsize::new(0));
            let runtime1 = test_whisper_runtime();
            let (stream1, _) = listener.accept().unwrap();
            let first_result =
                handle_connection(stream1, &storage1, &server_app_data_path, &runtime1, &jobs1);
            assert!(
                first_result.is_err(),
                "first connection must end in a transport error once the peer drops mid-transfer"
            );

            // Second connection: real `handle_connection` again, same
            // app_data/job_id, must resume and complete for real.
            let (_genesis_path2, storage2) = open_genesis();
            let jobs2 = Arc::new(AtomicUsize::new(0));
            let runtime2 = test_whisper_runtime();
            let (stream2, _) = listener.accept().unwrap();
            handle_connection(stream2, &storage2, &server_app_data_path, &runtime2, &jobs2)
        });

        // --- First connection: seq 0 and seq 1 only, then drop. ---
        let client_stream1 = TcpStream::connect(addr).unwrap();
        let mut channel1 = initiator_handshake(
            client_stream1,
            "device-resume-k",
            client_app_data.path(),
            server_app_data.path(),
        );
        channel1
            .send(&Control::JobStart {
                job_id: job_id.into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest.clone(),
                segment_count: 3,
                total_bytes: (seg0.len() + seg1.len() + seg2.len()) as u64,
                profile: "cpu".into(),
                resume_from_seq: 0,
                checksums: checksums.clone(),
            })
            .unwrap();

        for (seq, bytes) in [(0u32, &seg0), (1u32, &seg1)] {
            channel1
                .send(&Control::Chunk {
                    job_id: job_id.into(),
                    seq,
                    len: bytes.len() as u32,
                })
                .unwrap();
            for part in bytes.chunks(crate::fungwire::NOISE_MAX_PLAINTEXT) {
                channel1.send_bytes(part).unwrap();
            }
            match channel1.recv_control().unwrap() {
                Control::ChunkAck { seq: acked, .. } => assert_eq!(acked, seq),
                other => panic!("expected ChunkAck({seq}), got {other:?}"),
            }
            match channel1.recv_control().unwrap() {
                Control::Progress { stage, .. } => assert_eq!(stage, "receiving"),
                other => panic!("expected receiving Progress, got {other:?}"),
            }
        }

        // K=1: last_acked_seq is genuinely 1 here. Drop before segment 2 —
        // this is the real mid-transfer disconnect the fix is for.
        drop(channel1);

        // --- Second connection: resume from seq 2. ---
        let client_stream2 = TcpStream::connect(addr).unwrap();
        let mut channel2 = initiator_handshake(
            client_stream2,
            "device-resume-k",
            client_app_data.path(),
            server_app_data.path(),
        );
        channel2
            .send(&Control::JobStart {
                job_id: job_id.into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest.clone(),
                segment_count: 3,
                total_bytes: (seg0.len() + seg1.len() + seg2.len()) as u64,
                profile: "cpu".into(),
                resume_from_seq: 2,
                checksums: checksums.clone(),
            })
            .unwrap();
        channel2
            .send(&Control::Chunk {
                job_id: job_id.into(),
                seq: 2,
                len: seg2.len() as u32,
            })
            .unwrap();
        for part in seg2.chunks(crate::fungwire::NOISE_MAX_PLAINTEXT) {
            channel2.send_bytes(part).unwrap();
        }

        match channel2.recv_control().unwrap() {
            Control::ChunkAck { seq, .. } => assert_eq!(seq, 2),
            other => panic!("expected ChunkAck(2), got {other:?}"),
        }
        match channel2.recv_control().unwrap() {
            Control::Progress { stage, percent, .. } => {
                assert_eq!(stage, "receiving");
                assert_eq!(percent, 100, "all 3 segments now accounted for");
            }
            other => panic!("expected receiving Progress, got {other:?}"),
        }
        // Transcription now runs in a SINGLE invocation covering all 3
        // segments (2 reloaded from disk + 1 received this connection), on
        // its own thread while the main thread streams progress (BLOCKING
        // fix) -- so the fixture's two interim `PROGRESS` lines (30, 70)
        // arrive as their own "transcribing" frames, not one Progress
        // message per segment and not just a single final 100%.
        let transcribing_percents = recv_transcribing_progress_until_done(&mut channel2);
        assert!(
            transcribing_percents.iter().any(|&p| p < 100),
            "expected at least one interim transcribing Progress, got {transcribing_percents:?}"
        );
        assert_eq!(*transcribing_percents.last().unwrap(), 100);
        match channel2.recv_control().unwrap() {
            Control::Result {
                job_id: result_job_id,
                duration_ms,
                segments,
            } => {
                assert_eq!(result_job_id, job_id);
                assert_eq!(
                    duration_ms, 3000,
                    "fake_transcribe.py returns 1000ms per manifest path, 3 paths in one call"
                );
                assert_eq!(segments.len(), 3, "all three segments must be transcribed");
                assert_eq!(segments[0].start_ms, 0);
                assert_eq!(
                    segments[1].start_ms, 1000,
                    "segment 1 offset by segment 0's real duration (1000ms), not a fixed 5s window"
                );
                assert_eq!(
                    segments[2].start_ms, 2000,
                    "segment 2 offset by segments 0+1's real cumulative duration (2000ms)"
                );
            }
            other => panic!("expected Result, got {other:?}"),
        }

        drop(channel2);
        let result = server_thread.join().unwrap();
        assert!(result.is_ok(), "resumed job loop should exit cleanly: {result:?}");

        let job_dir = job_dir_path(server_app_data.path(), job_id);
        assert!(
            !job_dir.exists(),
            "job dir must be cleaned up after the job completes successfully"
        );
    }

    /// A `resume_from_seq` that exceeds `segment_count` (here: claiming 5
    /// already-acked segments against a brand-new job dir that has never
    /// received a byte) must be rejected up front as `Error{code:
    /// "resume_gap"}` rather than the worker trying to preload/read segments
    /// that were never written and hanging or panicking. This is the
    /// "worker's on-disk state and the client's resume_from_seq have
    /// diverged" branch described on `receive_and_transcribe`'s doc comment.
    #[test]
    fn resume_from_seq_out_of_range_is_rejected_as_resume_gap() {
        let server_app_data = tempfile::tempdir().unwrap();
        let client_app_data = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(server_app_data.path()).unwrap();
        ensure_identity_in_dir(client_app_data.path()).unwrap();
        pair_device(server_app_data.path(), "device-resume-gap", client_app_data.path());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));
        let runtime = test_whisper_runtime();

        let server_app_data_path = server_app_data.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &runtime, &jobs)
        });

        let client_stream = TcpStream::connect(addr).unwrap();
        let mut channel = initiator_handshake(
            client_stream,
            "device-resume-gap",
            client_app_data.path(),
            server_app_data.path(),
        );

        // resume_from_seq (5) claims 5 already-acked segments exist on a
        // brand-new job dir that has never seen a single byte -- the worker
        // must reject this as a broken resume chain, not hang waiting for
        // Chunks that will never come at the wrong offset.
        let checksums = vec!["0".repeat(64); 5];
        let manifest = manifest_hash(&checksums);
        channel
            .send(&Control::JobStart {
                job_id: "job-resume-gap".into(),
                operation: "transcript.transcribe".into(),
                manifest_hash: manifest,
                segment_count: 5,
                total_bytes: 0,
                profile: "cpu".into(),
                resume_from_seq: 5,
                checksums,
            })
            .unwrap();

        match channel.recv_control().unwrap() {
            Control::Error { job_id, code, .. } => {
                assert_eq!(job_id, "job-resume-gap");
                assert_eq!(code, "resume_gap");
            }
            other => panic!("expected Error{{code: resume_gap}}, got {other:?}"),
        }

        drop(channel);
        let result = server_thread.join().unwrap();
        assert!(result.is_ok(), "resume_gap should be sent and the loop exit cleanly: {result:?}");

        let job_dir = job_dir_path(server_app_data.path(), "job-resume-gap");
        assert!(
            !job_dir.exists(),
            "job dir must be cleaned up after a resume_gap rejection (terminal error)"
        );
    }

    /// Sets a directory's mtime directly, without going through `fs::write`
    /// (which would just set it to "now"). Opening a directory as a `File`
    /// needs `FILE_FLAG_BACKUP_SEMANTICS` on Windows (a plain `File::open`
    /// on a directory fails there); Unix has no such restriction.
    #[cfg(windows)]
    fn touch_dir_mtime(path: &Path, when: std::time::SystemTime) {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        let file = fs::OpenOptions::new()
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .expect("open dir with backup semantics to set its mtime");
        file.set_modified(when).expect("set dir mtime");
    }

    #[cfg(not(windows))]
    fn touch_dir_mtime(path: &Path, when: std::time::SystemTime) {
        let file = fs::File::open(path).expect("open dir to set its mtime");
        file.set_modified(when).expect("set dir mtime");
    }

    /// Core sweep behavior (final review #4): a job dir whose mtime is older
    /// than the TTL is removed; one that's still fresh survives.
    #[test]
    fn sweep_removes_only_dirs_older_than_ttl() {
        let app_data = tempfile::tempdir().unwrap();
        let jobs_root = app_data.path().join("fungwire").join("jobs");
        let stale = jobs_root.join("job-stale-and-abandoned");
        let fresh = jobs_root.join("job-still-in-progress");
        fs::create_dir_all(&stale).unwrap();
        fs::create_dir_all(&fresh).unwrap();

        let ttl = Duration::from_secs(60 * 60 * 24);
        touch_dir_mtime(
            &stale,
            SystemTime::now() - ttl - Duration::from_secs(60),
        );
        // `fresh` keeps the mtime it got from `create_dir_all` a moment ago.

        sweep_stale_job_dirs(app_data.path(), ttl);

        assert!(
            !stale.exists(),
            "a job dir older than the TTL must be swept"
        );
        assert!(
            fresh.exists(),
            "a job dir younger than the TTL must survive the sweep"
        );
    }

    /// A job dir created moments before the sweep runs (the common case: the
    /// server is toggled on while a resume is still realistically pending)
    /// must never be touched, regardless of how the sweep is implemented
    /// internally -- this is the "doesn't hang/misfire on the happy path"
    /// check called for alongside the pure `is_expired` unit tests.
    #[test]
    fn sweep_does_not_touch_a_just_created_dir() {
        let app_data = tempfile::tempdir().unwrap();
        let jobs_root = app_data.path().join("fungwire").join("jobs");
        let brand_new = jobs_root.join("job-just-started");
        fs::create_dir_all(&brand_new).unwrap();

        sweep_stale_job_dirs(app_data.path(), JOB_DIR_TTL);

        assert!(
            brand_new.exists(),
            "a job dir created moments ago must never be swept"
        );
    }

    /// The server enables successfully even the very first time, before any
    /// job has ever run and `fungwire/jobs/` doesn't exist yet -- the sweep
    /// must treat a missing jobs root as a no-op, not an error that could
    /// block enabling.
    #[test]
    fn sweep_on_missing_jobs_root_is_a_harmless_noop() {
        let app_data = tempfile::tempdir().unwrap();
        sweep_stale_job_dirs(app_data.path(), JOB_DIR_TTL);
        // Reaching this line without panicking is the assertion.
    }
}
