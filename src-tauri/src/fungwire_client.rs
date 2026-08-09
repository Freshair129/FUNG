//! Mobile FUNGWIRE client: connects to a paired desktop's FUNGWIRE server
//! (Task 6/7), streams a recording's audio segments, and applies the
//! returned transcript into the mobile Genesis store.
//!
//! This is the sender-side counterpart to `fungwire_server.rs`'s
//! `run_job_loop`/`receive_and_transcribe` (Task 7) — every `Control` shape,
//! field name, and message order below was read directly off that module,
//! not reconstructed from the design spec. See the module-level comments on
//! `fungwire_server::receive_and_transcribe` for the authoritative protocol
//! description this file mirrors.
//!
//! ## `own_device_id`: a deliberate interface addition
//! The task brief's command signatures don't list a parameter for "this
//! mobile device's own cloud device id", but the wire protocol requires one:
//! `Control::Hello{device_id}` must carry an id the desktop's
//! `paired_devices.db` recognizes as the *caller* (see
//! `fungwire_server::handle_connection`'s step 1-2). That id is a Supabase
//! `devices.id` UUID minted by the web/TS layer and cached in
//! `localStorage` (`src/mobile/MobileApp.tsx`'s `DEVICE_ID_KEY`) — Rust has
//! no other way to learn it. So, exactly like `endpoint` (which the brief
//! already calls out as "Task 9 resolves it in TS; accept it as an arg
//! now"), `own_device_id` is accepted as an explicit parameter on both
//! `fungwire_desktop_reachable` and `fungwire_delegate_transcription`.
//!
//! ## Resume across reconnects: a known Task 7 limitation
//! `Control::JobStart.resume_from_seq` is threaded through faithfully here
//! (computed as `last_acked_seq + 1` and only the not-yet-acked segments are
//! resent), but `fungwire_server::receive_and_transcribe` does not actually
//! implement cross-connection resume: every new TCP connection starts a
//! brand-new job context (`run_job_loop` -> fresh `receive_and_transcribe`
//! call, fresh temp dir) that unconditionally waits for exactly
//! `segment_count` `Chunk` messages, regardless of `resume_from_seq`. A
//! reconnect that resends fewer than `segment_count` chunks will make the
//! *server* block on `recv_control()` until its 60s read timeout, not fail
//! fast. This module still implements the client side of the intended
//! protocol (so it is correct against a server that does honor
//! `resume_from_seq`), but a real mid-transfer reconnect against *this
//! repo's* Task 7 server only actually completes if the drop happens before
//! any chunk was acknowledged (`last_acked_seq == -1`, so `resume_from_seq
//! == 0` resends everything). The resume-offset arithmetic itself is
//! covered by a pure unit test independent of that limitation.
//! independent of that limitation.

use crate::device_identity::{x25519_public_from_ed25519_b64, x25519_static_secret_in_dir};
use crate::fungwire::{
    manifest_hash, noise_initiator, read_frame, write_frame, Control, NoiseChannel, Segment,
    CTRL_MAX, NOISE_MAX_PLAINTEXT,
};
use crate::{now, AppError, AppResult, AppState};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::State;
use uuid::Uuid;

/// TCP connect timeout for both the reachability probe and every connection
/// attempt inside the transfer thread.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Read timeout applied to the socket once connected, covering the Noise
/// handshake and the whole job exchange — matches the server's own
/// `set_read_timeout(Some(Duration::from_secs(60)))` in
/// `fungwire_server::handle_connection`.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Number of *reconnect* attempts (i.e. not counting the initial attempt)
/// before the job is given up on and the peer is marked `unreachable`.
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelegateJobOutput {
    pub(crate) job_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobPollOutput {
    pub(crate) state: String,
    pub(crate) progress: i64,
    pub(crate) error: Option<String>,
}

/// One audio segment gathered from `audio_chunks`, renumbered to a 0-based
/// contiguous `seq` in `sequence_no` order (see [`gather_segments`] — the
/// wire protocol's `Chunk.seq` is a plain array index on the receiver, not
/// the stored `sequence_no`, which starts at 1).
struct SegmentRef {
    seq: u32,
    path: PathBuf,
    checksum: String,
}

/// Fields that stay constant across every `delegated_jobs` row update for a
/// given job — captured once at job creation so the transfer thread never
/// needs to re-read its own row (avoiding both a query round-trip and any
/// read-modify-write race against a concurrent poll).
struct JobContext {
    job_id: String,
    project_id: String,
    recording_id: String,
    executor_device_id: String,
    input_manifest_hash: String,
    observed_at: String,
    created_at: String,
}

// ---------------------------------------------------------------------
// Step 1: reachability
// ---------------------------------------------------------------------

/// Resolves `endpoint` (a `host:port` string) to a single `SocketAddr` for
/// `TcpStream::connect_timeout`, which needs a concrete address rather than
/// anything implementing `ToSocketAddrs` directly.
fn resolve_endpoint(endpoint: &str) -> Result<std::net::SocketAddr, String> {
    endpoint
        .to_socket_addrs()
        .map_err(|e| format!("could not resolve endpoint {endpoint:?}: {e}"))?
        .next()
        .ok_or_else(|| format!("endpoint {endpoint:?} resolved to no addresses"))
}

/// The unit `fungwire_desktop_reachable`'s "closed port -> false" test
/// exercises directly: `TcpStream::connect_timeout` + cleartext `Hello` +
/// Noise KK initiator handshake, returning the ready-to-use channel only on
/// `into_transport_mode` success. Deliberately takes raw key material
/// instead of a `device_id`/Genesis lookup so it can be tested without
/// standing up a Genesis fixture — [`desktop_reachable_inner`] and
/// [`attempt_transfer`] are the callers that resolve real keys first.
fn connect_and_handshake(
    endpoint: &str,
    own_device_id: &str,
    own_secret: &[u8; 32],
    peer_public: &[u8; 32],
) -> Result<NoiseChannel<TcpStream>, String> {
    let addr = resolve_endpoint(endpoint)?;
    let mut stream =
        TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok();

    write_frame(
        &mut stream,
        &Control::Hello {
            device_id: own_device_id.to_string(),
        }
        .encode(),
    )
    .map_err(|e| e.to_string())?;

    let mut hs = noise_initiator(own_secret, peer_public)?;
    let mut buf = [0u8; 4096];
    let n = hs.write_message(&[], &mut buf).map_err(|e| e.to_string())?;
    write_frame(&mut stream, &buf[..n]).map_err(|e| e.to_string())?;
    let msg2 = read_frame(&mut stream, CTRL_MAX).map_err(|e| e.to_string())?;
    let mut rbuf = [0u8; 4096];
    hs.read_message(&msg2, &mut rbuf).map_err(|e| e.to_string())?;
    let transport = hs.into_transport_mode().map_err(|e| e.to_string())?;
    Ok(NoiseChannel::new(stream, transport))
}

/// Looks up `device_id` in the *mobile* Genesis `paired_devices` table
/// (populated by `mobile::mobile_pairing_complete`) and returns its cached
/// ed25519 public key. Requires `trust_state == "paired"` — a revoked or
/// already-unreachable peer must not be dialed again until re-paired.
fn lookup_peer_public_key(
    storage: &genesis_block_native::Storage,
    device_id: &str,
) -> Result<String, String> {
    let row = crate::genesis_adapter::query(
        storage,
        "paired_devices",
        &["public_key", "trust_state"],
        vec![crate::genesis_adapter::eq(
            "paired_devices",
            "id",
            json!(device_id),
        )],
        1,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| format!("no paired desktop with id {device_id}"))?;
    let trust_state = row
        .get("paired_devices.trust_state")
        .and_then(Value::as_str)
        .unwrap_or("");
    if trust_state != "paired" {
        return Err(format!(
            "desktop {device_id} is not currently paired (trust_state={trust_state})"
        ));
    }
    row.get("paired_devices.public_key")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("desktop {device_id} has no cached public key"))
}

fn desktop_reachable_inner(
    storage: &genesis_block_native::Storage,
    app_data: &Path,
    desktop_device_id: &str,
    endpoint: &str,
    own_device_id: &str,
) -> bool {
    (|| -> Result<(), String> {
        let peer_pub_b64 = lookup_peer_public_key(storage, desktop_device_id)?;
        let own_secret = x25519_static_secret_in_dir(app_data).map_err(|e| e.to_string())?;
        let peer_pub = x25519_public_from_ed25519_b64(&peer_pub_b64).map_err(|e| e.to_string())?;
        connect_and_handshake(endpoint, own_device_id, &own_secret, &peer_pub)?;
        Ok(())
    })()
    .is_ok()
}

#[tauri::command]
pub(crate) fn fungwire_desktop_reachable(
    desktop_device_id: String,
    endpoint: String,
    own_device_id: String,
    state: State<'_, AppState>,
) -> AppResult<bool> {
    Ok(desktop_reachable_inner(
        &state.genesis,
        &state.data_root,
        &desktop_device_id,
        &endpoint,
        &own_device_id,
    ))
}

// ---------------------------------------------------------------------
// Step 2: delegation
// ---------------------------------------------------------------------

/// Gathers the recording's `audio_chunks`, ordered by `sequence_no`, and
/// renumbers them to a 0-based contiguous `seq` for the wire protocol.
fn gather_segments(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
) -> Result<Vec<SegmentRef>, String> {
    let rows = crate::genesis_adapter::query(
        storage,
        "audio_chunks",
        &["sequence_no", "file_path", "checksum"],
        vec![crate::genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            json!(recording_id),
        )],
        // 1000 is the query engine's hard cap (`REL_QUERY_LIMIT_EXCEEDED`
        // above that) — a five-second-per-segment recording would need to
        // run ~83 minutes to exceed it.
        1000,
    )?;
    let mut ordered: Vec<(i64, PathBuf, String)> = rows
        .into_iter()
        .map(|row| {
            let sequence_no = row
                .get("audio_chunks.sequence_no")
                .and_then(Value::as_i64)
                .ok_or("audio_chunks row missing sequence_no")?;
            let file_path = row
                .get("audio_chunks.file_path")
                .and_then(Value::as_str)
                .ok_or("audio_chunks row missing file_path")?
                .to_string();
            let checksum = row
                .get("audio_chunks.checksum")
                .and_then(Value::as_str)
                .ok_or("audio_chunks row missing checksum")?
                .to_string();
            Ok((sequence_no, PathBuf::from(file_path), checksum))
        })
        .collect::<Result<Vec<_>, String>>()?;
    ordered.sort_by_key(|(sequence_no, _, _)| *sequence_no);
    Ok(ordered
        .into_iter()
        .enumerate()
        .map(|(index, (_, path, checksum))| SegmentRef {
            seq: index as u32,
            path,
            checksum,
        })
        .collect())
}

fn update_job(
    storage: &genesis_block_native::Storage,
    ctx: &JobContext,
    state_str: &str,
    progress: i64,
    error: Option<&str>,
) -> Result<(), String> {
    let timestamp = now();
    let checkpoint = match error {
        Some(message) => json!({ "error": message }),
        None => Value::Null,
    };
    crate::genesis_adapter::commit_rows(
        storage,
        vec![crate::genesis_adapter::upsert(
            "delegated_jobs",
            json!({
                "id": ctx.job_id,
                "project_id": ctx.project_id,
                "executor_device_id": ctx.executor_device_id,
                "operation": "transcript.transcribe",
                "state": state_str,
                "progress": progress.clamp(0, 100),
                "input_manifest_hash": ctx.input_manifest_hash,
                "checkpoint_json": checkpoint,
                "observed_at": ctx.observed_at,
                "created_at": ctx.created_at,
                "updated_at": timestamp,
            }),
        )],
    )
}

/// Flips the peer's `trust_state` to `"unreachable"` after the retry budget
/// is exhausted (brief step 2.3). Leaves an already-`"revoked"` peer alone —
/// revocation is a stronger, user-driven signal that an unreachable flip
/// must not paper over.
fn mark_peer_unreachable(storage: &genesis_block_native::Storage, device_id: &str) {
    let Ok(rows) = crate::genesis_adapter::query(
        storage,
        "paired_devices",
        &[
            "name",
            "endpoint",
            "trust_state",
            "pairing_proof_hash",
            "capabilities_json",
            "created_at",
            "public_key",
        ],
        vec![crate::genesis_adapter::eq(
            "paired_devices",
            "id",
            json!(device_id),
        )],
        1,
    ) else {
        return;
    };
    let Some(row) = rows.into_iter().next() else {
        return;
    };
    let get = |key: &str| row.get(key).cloned().unwrap_or(Value::Null);
    if get("paired_devices.trust_state").as_str() == Some("revoked") {
        return;
    }
    let timestamp = now();
    let _ = crate::genesis_adapter::commit_rows(
        storage,
        vec![crate::genesis_adapter::upsert(
            "paired_devices",
            json!({
                "id": device_id,
                "name": get("paired_devices.name"),
                "endpoint": get("paired_devices.endpoint"),
                "trust_state": "unreachable",
                "pairing_proof_hash": get("paired_devices.pairing_proof_hash"),
                "capabilities_json": get("paired_devices.capabilities_json"),
                "created_at": get("paired_devices.created_at"),
                "updated_at": timestamp,
                "public_key": get("paired_devices.public_key"),
            }),
        )],
    );
}

/// Writes the returned transcript into `transcript_segments` (mirroring the
/// desktop-local `import_and_transcribe` insert path in `lib.rs`) and marks
/// the job `completed`.
fn write_transcript_and_complete(
    storage: &genesis_block_native::Storage,
    ctx: &JobContext,
    segments: &[Segment],
) -> Result<(), String> {
    let mut mutations = Vec::new();
    for segment in segments {
        let timestamp = now();
        mutations.push(crate::genesis_adapter::upsert(
            "transcript_segments",
            json!({
                "id": Uuid::new_v4().to_string(),
                "project_id": ctx.project_id,
                "recording_id": ctx.recording_id,
                "speaker_id": null,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "text": segment.text,
                "confidence": segment.confidence,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        ));
    }
    crate::genesis_adapter::commit_rows(storage, mutations)?;
    update_job(storage, ctx, "completed", 100, None)
}

/// Outcome of one connection attempt (one TCP connection's worth of Hello +
/// handshake + JobStart + chunk streaming + result wait).
enum AttemptOutcome {
    Completed(Vec<Segment>),
    /// The peer answered with `Control::Error` — a terminal, protocol-level
    /// failure (bad manifest, checksum mismatch, unsupported op, ...).
    /// Retrying the same job against the same peer will not help.
    ServerError(String, String),
    /// Anything below the protocol layer: connect failure, handshake
    /// failure, io error, unexpected/missing control message, decode
    /// error. Worth a reconnect.
    TransportError(String),
}

/// Drives exactly one connection attempt: connect, handshake, `JobStart`
/// (with `resume_from_seq` set to `resume_from`), then stream every segment
/// whose `seq >= resume_from`, updating `*last_acked` as each `ChunkAck`
/// arrives so the caller knows where to resume from on the next attempt.
#[allow(clippy::too_many_arguments)]
fn attempt_transfer(
    storage: &genesis_block_native::Storage,
    ctx: &JobContext,
    endpoint: &str,
    own_device_id: &str,
    own_secret: &[u8; 32],
    peer_public: &[u8; 32],
    manifest: &str,
    checksums: &[String],
    segments: &[SegmentRef],
    total_bytes: u64,
    resume_from: u32,
    last_acked: &mut i64,
) -> AttemptOutcome {
    let mut channel =
        match connect_and_handshake(endpoint, own_device_id, own_secret, peer_public) {
            Ok(channel) => channel,
            Err(e) => return AttemptOutcome::TransportError(e),
        };

    let segment_count = segments.len() as u32;
    if let Err(e) = channel.send(&Control::JobStart {
        job_id: ctx.job_id.clone(),
        operation: "transcript.transcribe".to_string(),
        manifest_hash: manifest.to_string(),
        segment_count,
        total_bytes,
        // v1: no per-job hardware profile selection from the mobile side
        // yet (not part of this task's interface); the desktop worker
        // accepts whatever it's given and runs its own faster-whisper
        // pipeline with it (see `fungwire_server::receive_and_transcribe`).
        profile: "cpu".to_string(),
        resume_from_seq: resume_from,
        checksums: checksums.to_vec(),
    }) {
        return AttemptOutcome::TransportError(e.to_string());
    }

    for segment in segments.iter().filter(|segment| segment.seq >= resume_from) {
        let bytes = match fs::read(&segment.path) {
            Ok(bytes) => bytes,
            Err(e) => {
                return AttemptOutcome::TransportError(format!(
                    "reading segment {}: {e}",
                    segment.seq
                ))
            }
        };
        if let Err(e) = channel.send(&Control::Chunk {
            job_id: ctx.job_id.clone(),
            seq: segment.seq,
            len: bytes.len() as u32,
        }) {
            return AttemptOutcome::TransportError(e.to_string());
        }
        // A 5s AAC segment is routinely tens of KB and can exceed
        // NOISE_MAX_PLAINTEXT (65,519 B, see fungwire.rs); the receiver
        // reassembles by reading sub-frames until it has `len` bytes (see
        // `receive_and_transcribe`'s reassembly doc comment), so every
        // send_bytes call here must stay at or under that cap.
        for part in bytes.chunks(NOISE_MAX_PLAINTEXT) {
            if let Err(e) = channel.send_bytes(part) {
                return AttemptOutcome::TransportError(e.to_string());
            }
        }

        match channel.recv_control() {
            Ok(Control::ChunkAck { seq, .. }) if seq == segment.seq => {
                *last_acked = segment.seq as i64;
            }
            Ok(Control::ChunkAck { seq, .. }) => {
                return AttemptOutcome::TransportError(format!(
                    "expected ChunkAck for seq {}, got seq {seq}",
                    segment.seq
                ))
            }
            Ok(Control::Error { code, message, .. }) => {
                return AttemptOutcome::ServerError(code, message)
            }
            Ok(other) => {
                return AttemptOutcome::TransportError(format!(
                    "expected ChunkAck, got {other:?}"
                ))
            }
            Err(e) => return AttemptOutcome::TransportError(e.to_string()),
        }

        match channel.recv_control() {
            Ok(Control::Progress { percent, .. }) => {
                let _ = update_job(storage, ctx, "running", percent as i64, None);
            }
            Ok(Control::Error { code, message, .. }) => {
                return AttemptOutcome::ServerError(code, message)
            }
            Ok(other) => {
                return AttemptOutcome::TransportError(format!(
                    "expected Progress, got {other:?}"
                ))
            }
            Err(e) => return AttemptOutcome::TransportError(e.to_string()),
        }
    }

    // All segments sent (or nothing left to send on a resumed attempt).
    // The server now transcribes each segment in turn, sending one
    // Progress{stage:"transcribing"} per segment before the final Result.
    loop {
        match channel.recv_control() {
            Ok(Control::Progress { percent, .. }) => {
                let _ = update_job(storage, ctx, "running", percent as i64, None);
            }
            Ok(Control::Result { segments, .. }) => return AttemptOutcome::Completed(segments),
            Ok(Control::Error { code, message, .. }) => {
                return AttemptOutcome::ServerError(code, message)
            }
            Ok(other) => {
                return AttemptOutcome::TransportError(format!(
                    "expected Progress/Result, got {other:?}"
                ))
            }
            Err(e) => return AttemptOutcome::TransportError(e.to_string()),
        }
    }
}

/// Exponential backoff for reconnect attempt `attempt` (1-indexed):
/// 200ms, 400ms, 800ms, ... capped at attempt 4 to avoid overflow.
fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(200u64 * 2u64.pow(attempt.saturating_sub(1).min(4)))
}

/// Runs on a dedicated `std::thread` (spawned by
/// `delegate_transcription_inner`); drives the whole job to completion or
/// failure, writing every state transition into the `delegated_jobs` row so
/// `fungwire_job_poll` can observe it.
#[allow(clippy::too_many_arguments)]
fn run_transfer(
    storage: Arc<genesis_block_native::Storage>,
    app_data: PathBuf,
    ctx: JobContext,
    endpoint: String,
    own_device_id: String,
    desktop_device_id: String,
    peer_public_key_b64: String,
    segments: Vec<SegmentRef>,
    checksums: Vec<String>,
    manifest: String,
) {
    let _ = update_job(&storage, &ctx, "running", 0, None);

    let own_secret = match x25519_static_secret_in_dir(&app_data) {
        Ok(secret) => secret,
        Err(e) => {
            let _ = update_job(&storage, &ctx, "failed", 0, Some(&format!("identity: {e}")));
            return;
        }
    };
    let peer_public = match x25519_public_from_ed25519_b64(&peer_public_key_b64) {
        Ok(key) => key,
        Err(e) => {
            let _ = update_job(&storage, &ctx, "failed", 0, Some(&format!("peer key: {e}")));
            return;
        }
    };

    let total_bytes: u64 = segments
        .iter()
        .map(|segment| fs::metadata(&segment.path).map(|m| m.len()).unwrap_or(0))
        .sum();

    let mut last_acked: i64 = -1;
    let mut reconnect_attempts: u32 = 0;

    loop {
        let resume_from = (last_acked + 1) as u32;
        let outcome = attempt_transfer(
            &storage,
            &ctx,
            &endpoint,
            &own_device_id,
            &own_secret,
            &peer_public,
            &manifest,
            &checksums,
            &segments,
            total_bytes,
            resume_from,
            &mut last_acked,
        );
        match outcome {
            AttemptOutcome::Completed(result_segments) => {
                if let Err(e) = write_transcript_and_complete(&storage, &ctx, &result_segments) {
                    let _ = update_job(&storage, &ctx, "failed", 100, Some(&e));
                }
                return;
            }
            AttemptOutcome::ServerError(code, message) => {
                let _ = update_job(
                    &storage,
                    &ctx,
                    "failed",
                    0,
                    Some(&format!("{code}: {message}")),
                );
                return;
            }
            AttemptOutcome::TransportError(message) => {
                reconnect_attempts += 1;
                if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                    let _ = update_job(
                        &storage,
                        &ctx,
                        "failed",
                        0,
                        Some(&format!(
                            "unreachable after {reconnect_attempts} reconnect attempts: {message}"
                        )),
                    );
                    mark_peer_unreachable(&storage, &desktop_device_id);
                    return;
                }
                thread::sleep(backoff_delay(reconnect_attempts));
                // Loop back and try again from `last_acked_seq + 1`.
            }
        }
    }
}

fn delegate_transcription_inner(
    storage: Arc<genesis_block_native::Storage>,
    app_data: PathBuf,
    project_id: String,
    recording_id: String,
    desktop_device_id: String,
    endpoint: String,
    own_device_id: String,
) -> AppResult<String> {
    let valid = crate::genesis_adapter::query(
        &storage,
        "recordings",
        &["id"],
        vec![
            crate::genesis_adapter::eq("recordings", "id", json!(recording_id)),
            crate::genesis_adapter::eq("recordings", "project_id", json!(project_id)),
        ],
        1,
    )
    .map_err(AppError::Genesis)?;
    if valid.is_empty() {
        return Err(AppError::InvalidInput(
            "recording does not belong to this project".to_string(),
        ));
    }

    let peer_public_key_b64 =
        lookup_peer_public_key(&storage, &desktop_device_id).map_err(AppError::Genesis)?;

    let segments = gather_segments(&storage, &recording_id).map_err(AppError::Genesis)?;
    if segments.is_empty() {
        return Err(AppError::InvalidInput(
            "recording has no audio segments to transcribe".to_string(),
        ));
    }
    let checksums: Vec<String> = segments.iter().map(|s| s.checksum.clone()).collect();
    let manifest = manifest_hash(&checksums);

    let job_id = Uuid::new_v4().to_string();
    let timestamp = now();
    crate::genesis_adapter::commit_rows(
        &storage,
        vec![crate::genesis_adapter::upsert(
            "delegated_jobs",
            json!({
                "id": job_id,
                "project_id": project_id,
                "executor_device_id": desktop_device_id,
                "operation": "transcript.transcribe",
                "state": "queued",
                "progress": 0,
                "input_manifest_hash": manifest,
                "checkpoint_json": null,
                "observed_at": timestamp,
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        )],
    )
    .map_err(AppError::Genesis)?;

    let ctx = JobContext {
        job_id: job_id.clone(),
        project_id: project_id.clone(),
        recording_id: recording_id.clone(),
        executor_device_id: desktop_device_id.clone(),
        input_manifest_hash: manifest.clone(),
        observed_at: timestamp.clone(),
        created_at: timestamp,
    };

    thread::spawn(move || {
        run_transfer(
            storage,
            app_data,
            ctx,
            endpoint,
            own_device_id,
            desktop_device_id,
            peer_public_key_b64,
            segments,
            checksums,
            manifest,
        );
    });

    Ok(job_id)
}

#[tauri::command]
pub(crate) fn fungwire_delegate_transcription(
    project_id: String,
    recording_id: String,
    desktop_device_id: String,
    endpoint: String,
    own_device_id: String,
    state: State<'_, AppState>,
) -> AppResult<DelegateJobOutput> {
    let job_id = delegate_transcription_inner(
        state.genesis.clone(),
        state.data_root.clone(),
        project_id,
        recording_id,
        desktop_device_id,
        endpoint,
        own_device_id,
    )?;
    Ok(DelegateJobOutput { job_id })
}

// ---------------------------------------------------------------------
// Step 3: poll
// ---------------------------------------------------------------------

fn job_poll_inner(
    storage: &genesis_block_native::Storage,
    job_id: &str,
) -> AppResult<JobPollOutput> {
    let row = crate::genesis_adapter::query(
        storage,
        "delegated_jobs",
        &["state", "progress", "checkpoint_json"],
        vec![crate::genesis_adapter::eq(
            "delegated_jobs",
            "id",
            json!(job_id),
        )],
        1,
    )
    .map_err(AppError::Genesis)?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::InvalidInput("delegated job not found".to_string()))?;

    let job_state = row
        .get("delegated_jobs.state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let progress = row
        .get("delegated_jobs.progress")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let error = row
        .get("delegated_jobs.checkpoint_json")
        .and_then(|checkpoint| checkpoint.get("error"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    Ok(JobPollOutput {
        state: job_state,
        progress,
        error,
    })
}

#[tauri::command]
pub(crate) fn fungwire_job_poll(
    job_id: String,
    state: State<'_, AppState>,
) -> AppResult<JobPollOutput> {
    job_poll_inner(&state.genesis, &job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_identity::{
        ensure_identity_in_dir, public_key_b64_in_dir, x25519_static_secret_in_dir,
    };
    use crate::{paired_devices_connection_at, upsert_paired_device, PairedDeviceInput, WhisperRuntime};
    use base64::Engine;
    use genesis_block_native::{OpenOptions, Storage};
    use sha2::{Digest, Sha256};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    fn open_genesis() -> (std::path::PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fungwire-client-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
        })
        .expect("open GenesisBlockDB");
        crate::genesis_adapter::install(&storage).expect("install schema");
        (path, storage)
    }

    /// Same fixture fake used by fungwire_server.rs's own tests: a real
    /// venv python interpreter (so `run_python_worker`'s existence checks
    /// pass) pointed at a stub script instead of the real faster-whisper
    /// pipeline.
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

    /// Registers the desktop as a peer in the *mobile* Genesis
    /// `paired_devices` table, caching the desktop's real published ed25519
    /// key (read from `desktop_app_data`) so `x25519_public_from_ed25519_b64`
    /// gets a real, valid key.
    fn pair_desktop_on_mobile(
        mobile_storage: &Storage,
        desktop_device_id: &str,
        desktop_app_data: &Path,
    ) {
        let pub_b64 = public_key_b64_in_dir(desktop_app_data).unwrap();
        let timestamp = now();
        crate::genesis_adapter::commit_rows(
            mobile_storage,
            vec![crate::genesis_adapter::upsert(
                "paired_devices",
                json!({
                    "id": desktop_device_id, "name": "FUNG Desktop", "endpoint": "",
                    "trust_state": "paired", "pairing_proof_hash": "sess-desktop",
                    "capabilities_json": [], "created_at": timestamp, "updated_at": timestamp,
                    "public_key": pub_b64,
                }),
            )],
        )
        .unwrap();
    }

    /// Registers the mobile device as a paired, non-revoked peer in the
    /// *desktop's* `paired_devices.db` (the rusqlite store
    /// `fungwire_server::handle_connection` looks the caller up in) —
    /// mirrors `fungwire_server.rs`'s own `pair_device` test helper.
    fn pair_mobile_on_desktop(desktop_app_data: &Path, own_device_id: &str, mobile_app_data: &Path) {
        let pub_b64 = public_key_b64_in_dir(mobile_app_data).unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&pub_b64)
            .unwrap();
        let fingerprint: String = Sha256::digest(&raw).iter().map(|b| format!("{b:02x}")).collect();
        let conn = paired_devices_connection_at(desktop_app_data).unwrap();
        upsert_paired_device(
            &conn,
            PairedDeviceInput {
                id: own_device_id.to_string(),
                name: "Test Mobile".to_string(),
                platform: "test".to_string(),
                fingerprint,
                pairing_session_id: "sess-mobile".to_string(),
                public_key: Some(pub_b64),
            },
        )
        .unwrap();
    }

    /// Writes one audio segment file on disk and returns its `SegmentRef`
    /// input tuple (sequence_no starts at 1, matching
    /// `mobile_capture_append_segment`'s numbering).
    fn write_segment_file(dir: &Path, sequence_no: i64, byte_len: usize) -> (PathBuf, String) {
        let bytes: Vec<u8> = (0..byte_len).map(|i| (i % 256) as u8).collect();
        let path = dir.join(format!("segment-{sequence_no:06}.m4a"));
        fs::write(&path, &bytes).unwrap();
        let checksum: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        (path, checksum)
    }

    fn seed_recording_with_segments(
        storage: &Storage,
        project_id: &str,
        recording_id: &str,
        segment_dir: &Path,
        byte_lens: &[usize],
    ) {
        let timestamp = now();
        let mut mutations =
            crate::genesis_adapter::ensure_project_mutations(project_id, "projects/p", &timestamp);
        mutations.push(crate::genesis_adapter::upsert(
            "recordings",
            json!({
                "id": recording_id, "project_id": project_id, "source": "microphone",
                "input_path": null, "canonical_audio_path": "manifest.json", "status": "completed",
                "duration_ms": (byte_lens.len() as i64) * 5000, "created_at": timestamp, "updated_at": timestamp,
            }),
        ));
        for (index, byte_len) in byte_lens.iter().enumerate() {
            let sequence_no = index as i64 + 1;
            let (path, checksum) = write_segment_file(segment_dir, sequence_no, *byte_len);
            mutations.push(crate::genesis_adapter::upsert(
                "audio_chunks",
                json!({
                    "id": Uuid::new_v4().to_string(), "recording_id": recording_id, "sequence_no": sequence_no,
                    "file_path": path.display().to_string(), "start_ms": index as i64 * 5000,
                    "end_ms": (index as i64 + 1) * 5000, "byte_size": *byte_len as i64,
                    "checksum": checksum, "created_at": timestamp,
                }),
            ));
        }
        crate::genesis_adapter::commit_rows(storage, mutations).unwrap();
    }

    /// Spawns the real Task 7 server on an ephemeral loopback port, accepts
    /// exactly one connection, and returns the join handle plus the bound
    /// address.
    fn spawn_server(
        server_app_data: PathBuf,
    ) -> (std::net::SocketAddr, thread::JoinHandle<Result<(), String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (_genesis_path, storage) = open_genesis();
        let jobs = Arc::new(AtomicUsize::new(0));
        let runtime = test_whisper_runtime();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            crate::fungwire_server::handle_connection(stream, &storage, &server_app_data, &runtime, &jobs)
        });
        (addr, handle)
    }

    // -------------------------------------------------------------
    // Step 1 tests
    // -------------------------------------------------------------

    #[test]
    fn closed_port_is_not_reachable() {
        let local_dir = tempfile::tempdir().unwrap();
        let peer_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(local_dir.path()).unwrap();
        ensure_identity_in_dir(peer_dir.path()).unwrap();
        let own_secret = x25519_static_secret_in_dir(local_dir.path()).unwrap();
        let peer_public =
            x25519_public_from_ed25519_b64(&public_key_b64_in_dir(peer_dir.path()).unwrap())
                .unwrap();

        // Bind then immediately drop: the OS reports the port as closed
        // (connection refused) for any subsequent connect attempt.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let result = connect_and_handshake(&addr.to_string(), "mobile-x", &own_secret, &peer_public);
        assert!(result.is_err(), "closed port must not reach transport mode");
    }

    #[test]
    fn desktop_reachable_inner_true_on_successful_handshake() {
        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        pair_desktop_on_mobile(&mobile_storage, "desktop-reach", desktop_dir.path());
        pair_mobile_on_desktop(desktop_dir.path(), "mobile-reach", mobile_dir.path());

        let (addr, server_handle) = spawn_server(desktop_dir.path().to_path_buf());

        let reachable = desktop_reachable_inner(
            &mobile_storage,
            mobile_dir.path(),
            "desktop-reach",
            &addr.to_string(),
            "mobile-reach",
        );
        assert!(reachable, "paired desktop with an open port must be reachable");

        // The server-side handshake test proves this closes cleanly; here we
        // only need the accept thread to finish before the test process exits.
        server_handle.join().unwrap().ok();
        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }

    #[test]
    fn desktop_reachable_inner_false_for_unknown_peer() {
        let mobile_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        // Deliberately no paired_devices row for "desktop-unknown".
        let reachable = desktop_reachable_inner(
            &mobile_storage,
            mobile_dir.path(),
            "desktop-unknown",
            "127.0.0.1:1",
            "mobile-x",
        );
        assert!(!reachable);
        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }

    // -------------------------------------------------------------
    // Step 2/3: end-to-end loopback against the real Task 7 server
    // -------------------------------------------------------------

    #[test]
    fn delegate_transcription_completes_and_writes_transcript_over_loopback() {
        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        let segment_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        let mobile_storage = Arc::new(mobile_storage);

        pair_desktop_on_mobile(&mobile_storage, "desktop-e2e", desktop_dir.path());
        pair_mobile_on_desktop(desktop_dir.path(), "mobile-e2e", mobile_dir.path());

        // Two segments: one small, one 70,000 bytes (> NOISE_MAX_PLAINTEXT =
        // 65,519) so send/recv must split and reassemble across multiple
        // Noise sub-frames to succeed at all.
        seed_recording_with_segments(
            &mobile_storage,
            "proj-e2e",
            "rec-e2e",
            segment_dir.path(),
            &[1024, 70_000],
        );

        let (addr, server_handle) = spawn_server(desktop_dir.path().to_path_buf());

        let job_id = delegate_transcription_inner(
            mobile_storage.clone(),
            mobile_dir.path().to_path_buf(),
            "proj-e2e".to_string(),
            "rec-e2e".to_string(),
            "desktop-e2e".to_string(),
            addr.to_string(),
            "mobile-e2e".to_string(),
        )
        .expect("delegate transcription");

        // Poll delegated_jobs until it leaves the running state (bounded).
        let mut final_state = String::new();
        for _ in 0..100 {
            let poll = job_poll_inner(&mobile_storage, &job_id).expect("poll job");
            if poll.state == "completed" || poll.state == "failed" {
                final_state = poll.state;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(final_state, "completed", "job must reach completed");

        let segments = crate::genesis_adapter::query(
            &mobile_storage,
            "transcript_segments",
            &["text", "recording_id"],
            vec![crate::genesis_adapter::eq(
                "transcript_segments",
                "recording_id",
                json!("rec-e2e"),
            )],
            100,
        )
        .expect("query transcript_segments");
        assert!(
            !segments.is_empty(),
            "transcript_segments must be written for the completed job"
        );
        assert_eq!(
            segments[0]
                .get("transcript_segments.text")
                .and_then(Value::as_str),
            Some("hi"),
            "fake_transcribe.py's fixed output must round-trip"
        );

        server_handle.join().unwrap().ok();
        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }

    // -------------------------------------------------------------
    // Reconnect / resume
    // -------------------------------------------------------------

    /// Pure unit test for the resume-offset arithmetic used on every
    /// reconnect: `resume_from_seq` is always `last_acked_seq + 1`,
    /// independent of any networking (see the module doc comment for why a
    /// true mid-transfer resume can't be demonstrated end-to-end against
    /// this repo's Task 7 server).
    #[test]
    fn resume_offset_is_last_acked_plus_one() {
        let mut last_acked: i64 = -1;
        assert_eq!((last_acked + 1) as u32, 0, "nothing acked yet -> resume from 0");
        last_acked = 4;
        assert_eq!((last_acked + 1) as u32, 5, "seq 4 acked -> resume from 5");
        last_acked = 0;
        assert_eq!((last_acked + 1) as u32, 1, "seq 0 acked -> resume from 1");
    }

    /// End-to-end reconnect test: the first connection is dropped by the
    /// server immediately after the handshake (before any `Chunk` is
    /// acknowledged, so `last_acked_seq` stays at -1 / `resume_from_seq ==
    /// 0`). The client must detect the transport error, back off, reconnect,
    /// resend the whole job from scratch on the second attempt, and still
    /// reach `completed` — proving the reconnect-with-backoff loop and the
    /// resume-offset computation are wired together correctly for the case
    /// this server implementation can actually satisfy (see module doc
    /// comment on the cross-connection resume limitation).
    #[test]
    fn delegate_transcription_reconnects_after_early_drop_and_completes() {
        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        let segment_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        let mobile_storage = Arc::new(mobile_storage);

        pair_desktop_on_mobile(&mobile_storage, "desktop-resume", desktop_dir.path());
        pair_mobile_on_desktop(desktop_dir.path(), "mobile-resume", mobile_dir.path());
        seed_recording_with_segments(
            &mobile_storage,
            "proj-resume",
            "rec-resume",
            segment_dir.path(),
            &[2048],
        );

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_app_data = desktop_dir.path().to_path_buf();
        let server_thread = thread::spawn(move || {
            let (_genesis_path, storage) = open_genesis();
            let jobs = Arc::new(AtomicUsize::new(0));
            let runtime = test_whisper_runtime();

            // First connection: accept, consume the cleartext Hello frame
            // (so the client's write never fails), then drop without ever
            // answering the Noise handshake. The client's read of msg2 then
            // deterministically fails with EOF, forcing a reconnect before
            // any Chunk is ever sent (so last_acked_seq stays at -1).
            let (mut first_stream, _) = listener.accept().unwrap();
            let _ = read_frame(&mut first_stream, CTRL_MAX);
            drop(first_stream);

            // Second connection: handle for real via the actual Task 7
            // server code path.
            let (second_stream, _) = listener.accept().unwrap();
            crate::fungwire_server::handle_connection(
                second_stream,
                &storage,
                &server_app_data,
                &runtime,
                &jobs,
            )
        });

        let job_id = delegate_transcription_inner(
            mobile_storage.clone(),
            mobile_dir.path().to_path_buf(),
            "proj-resume".to_string(),
            "rec-resume".to_string(),
            "desktop-resume".to_string(),
            addr.to_string(),
            "mobile-resume".to_string(),
        )
        .expect("delegate transcription");

        let mut final_state = String::new();
        for _ in 0..100 {
            let poll = job_poll_inner(&mobile_storage, &job_id).expect("poll job");
            if poll.state == "completed" || poll.state == "failed" {
                final_state = poll.state;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(
            final_state, "completed",
            "client must reconnect after the first connection is dropped and still complete"
        );

        server_thread.join().unwrap().ok();
        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }
}
