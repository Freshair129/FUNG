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
//! ## Resume across reconnects
//! `Control::JobStart.resume_from_seq` is computed as `last_acked_seq + 1`
//! and only the not-yet-acked segments (`seq >= resume_from_seq`) are
//! resent. `fungwire_server::receive_and_transcribe` honors this: it keeps a
//! per-`job_id` directory that persists across connections (rather than
//! deleting it on every disconnect), reloads and re-verifies segments
//! `0..resume_from_seq` from that directory, and only waits for `Chunk`s in
//! `resume_from_seq..segment_count` — so a genuine mid-transfer reconnect
//! (`last_acked_seq >= 0`) resumes for real instead of silently blocking
//! until the server's 60s read timeout. See
//! `fungwire_server::receive_and_transcribe`'s doc comment for the worker
//! side of this contract.
//!
//! If the worker's persisted state and this client's `resume_from_seq` have
//! diverged (e.g. the worker's job dir was lost between connections), it
//! answers with `Control::Error{code: "resume_gap"}` instead of hanging;
//! `run_transfer` treats that the same as a transport error except that it
//! also resets `last_acked_seq` to `-1` first, so the next attempt restarts
//! the whole job from seq 0 rather than repeating a resume that can never
//! succeed.

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
use std::time::{Duration, Instant};
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
/// Total wall-clock budget for the reconnect loop (i.e. not counting the
/// initial attempt) before the job is given up on and the peer is marked
/// `unreachable`. A fixed attempt-count budget (the previous design) adds
/// up to about 1.4s of retrying, which is far shorter than a desktop app
/// restart — spec §12's "kill desktop mid-job -> resumes on restart"
/// acceptance step needs a budget on the order of minutes, not attempts.
const RECONNECT_BUDGET: Duration = Duration::from_secs(120);
/// Initial backoff delay before the first reconnect attempt; doubles on
/// each subsequent attempt up to [`RECONNECT_BACKOFF_CAP`].
const RECONNECT_BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Ceiling on the exponential backoff delay between reconnect attempts.
const RECONNECT_BACKOFF_CAP: Duration = Duration::from_secs(5);

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
    /// `"local"` / `"cloud"` as recorded at delegation time, or `None` for a
    /// row written before Genesis schema v7 added the column. Reported here
    /// so the mobile provenance badge reads the persisted fact rather than
    /// whatever the UI happened to remember choosing.
    pub(crate) executor: Option<String>,
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
    /// Which engine the desktop was asked to run this job on — `"local"` or
    /// `"cloud"`, already normalized by [`normalize_executor`]. Lives on the
    /// context rather than being passed alongside it because every writer of
    /// the `delegated_jobs` row ([`update_job`]) and the one sender of
    /// `Control::JobStart` ([`attempt_transfer`]) already take `&JobContext`.
    executor: String,
    input_manifest_hash: String,
    observed_at: String,
    created_at: String,
}

/// Pins a caller-supplied executor to the two values the wire protocol and
/// the `delegated_jobs.executor` column actually define. Anything else —
/// a typo, a future value this build doesn't know, a hostile string — becomes
/// `"local"`: the conservative choice, since `"local"` is the option that
/// never sends audio off the desktop.
fn normalize_executor(requested: &str) -> &'static str {
    if requested == "cloud" {
        "cloud"
    } else {
        "local"
    }
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
/// ed25519 public key. Accepts `trust_state == "paired"` *or*
/// `"unreachable"` — an unreachable peer must still be dialable so a
/// reachability probe or a delegate's reconnect loop can attempt it and, on
/// success, reset it back to `"paired"` (see [`mark_peer_reachable`]);
/// otherwise a peer that ever went unreachable could never recover. Only
/// `"revoked"` (a stronger, user-driven signal) is rejected.
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
    if trust_state != "paired" && trust_state != "unreachable" {
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
        // Handshake succeeded: reset an unreachable peer back to paired
        // (spec §7) rather than leaving it stuck forever.
        mark_peer_reachable(storage, desktop_device_id);
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

/// What the mobile side learns from a paired desktop's
/// [`Control::StatusReply`]. A subset of the desktop's own `FungwireStatus`
/// — see `Control::StatusReply`'s doc for why the rest of that struct has no
/// meaning to a remote peer.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopStatusProbe {
    pub(crate) stt_cloud_enabled: bool,
}

/// How long to wait for a [`Control::StatusReply`] after the handshake.
///
/// Much shorter than [`READ_TIMEOUT`], which is sized for a whole
/// transcription job: answering a status request is a policy-row read, so a
/// desktop that has not replied within seconds is not busy — it is a peer
/// that does not understand `StatusRequest` at all (a build predating Phase
/// 3, whose job loop silently ignores unknown control messages). Without
/// this, that case would block a UI probe for the full 60s.
const STATUS_PROBE_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Reads the paired desktop's cloud-tier policy over FUNGWIRE.
///
/// Connection setup is [`connect_and_handshake`] — the exact same peer
/// lookup, X25519 key resolution and Noise KK initiator handshake
/// [`desktop_reachable_inner`] performs, deliberately shared rather than
/// re-derived. The only difference is what happens once the channel is up:
/// one `StatusRequest` out, one `StatusReply` in.
///
/// Read-only by construction: there is no wire message that lets a mobile
/// peer *change* this policy, because the keys it governs live on the
/// desktop (spec §10).
fn desktop_status_probe_inner(
    storage: &genesis_block_native::Storage,
    app_data: &Path,
    desktop_device_id: &str,
    endpoint: &str,
    own_device_id: &str,
) -> Result<DesktopStatusProbe, String> {
    let peer_pub_b64 = lookup_peer_public_key(storage, desktop_device_id)?;
    let own_secret = x25519_static_secret_in_dir(app_data).map_err(|e| e.to_string())?;
    let peer_pub = x25519_public_from_ed25519_b64(&peer_pub_b64).map_err(|e| e.to_string())?;
    let mut channel = connect_and_handshake(endpoint, own_device_id, &own_secret, &peer_pub)?;
    channel
        .get_mut()
        .set_read_timeout(Some(STATUS_PROBE_READ_TIMEOUT))
        .ok();
    channel
        .send(&Control::StatusRequest)
        .map_err(|e| e.to_string())?;
    match channel.recv_control().map_err(|e| e.to_string())? {
        Control::StatusReply { stt_cloud_enabled } => {
            // A completed exchange is at least as strong a liveness signal as
            // `fungwire_desktop_reachable`'s bare handshake, so it resets an
            // `unreachable` peer back to `paired` the same way.
            mark_peer_reachable(storage, desktop_device_id);
            Ok(DesktopStatusProbe { stt_cloud_enabled })
        }
        other => Err(format!("expected StatusReply, got {other:?}")),
    }
}

#[tauri::command]
pub(crate) fn fungwire_desktop_status_probe(
    desktop_device_id: String,
    endpoint: String,
    own_device_id: String,
    state: State<'_, AppState>,
) -> AppResult<DesktopStatusProbe> {
    desktop_status_probe_inner(
        &state.genesis,
        &state.data_root,
        &desktop_device_id,
        &endpoint,
        &own_device_id,
    )
    .map_err(AppError::InvalidInput)
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
                // Re-stated on every write: `upsert` replaces the whole row,
                // so omitting this would blank the column the moment the job
                // moved out of "queued".
                "executor": ctx.executor,
                "input_manifest_hash": ctx.input_manifest_hash,
                "checkpoint_json": checkpoint,
                "observed_at": ctx.observed_at,
                "created_at": ctx.created_at,
                "updated_at": timestamp,
            }),
        )],
    )
}

/// Sets the peer's `paired_devices.trust_state` to `new_state`, unless the
/// peer row is missing, already in `new_state`, or currently `"revoked"`.
/// The `"revoked"` guard is shared by both callers below —
/// [`mark_peer_unreachable`] (run after the reconnect budget is exhausted)
/// and [`mark_peer_reachable`] (run after every successful handshake):
/// revocation is a stronger, user-driven signal that neither an unreachable
/// flip nor a reachable reset may paper over. `commit_rows`'s `upsert`
/// requires every NOT NULL column, not just the one changing, hence the
/// full-row re-read.
fn set_peer_trust_state(storage: &genesis_block_native::Storage, device_id: &str, new_state: &str) {
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
    let current = get("paired_devices.trust_state");
    if current.as_str() == Some("revoked") || current.as_str() == Some(new_state) {
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
                "trust_state": new_state,
                "pairing_proof_hash": get("paired_devices.pairing_proof_hash"),
                "capabilities_json": get("paired_devices.capabilities_json"),
                "created_at": get("paired_devices.created_at"),
                "updated_at": timestamp,
                "public_key": get("paired_devices.public_key"),
            }),
        )],
    );
}

/// Flips the peer's `trust_state` to `"unreachable"` after the reconnect
/// budget is exhausted (brief step 2.3). See [`set_peer_trust_state`] for
/// the shared revoked/no-op guard.
fn mark_peer_unreachable(storage: &genesis_block_native::Storage, device_id: &str) {
    set_peer_trust_state(storage, device_id, "unreachable");
}

/// Resets the peer's `trust_state` back to `"paired"` on the very next
/// successful handshake (design spec §7: "reset to paired on next
/// success"). Called unconditionally after every successful
/// `connect_and_handshake` (both the reachability probe and the delegate
/// transfer's reconnect loop) — it is a no-op unless the peer was actually
/// `"unreachable"`, since without this nothing else in this file ever
/// clears that flag once set.
fn mark_peer_reachable(storage: &genesis_block_native::Storage, device_id: &str) {
    set_peer_trust_state(storage, device_id, "paired");
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
    /// The peer answered with `Control::Error` — normally a terminal,
    /// protocol-level failure (bad manifest, checksum mismatch, unsupported
    /// op, ...) that retrying the same job against the same peer will not
    /// fix. The one exception `run_transfer` special-cases is
    /// `code == "resume_gap"`: the worker's persisted state and our
    /// `resume_from_seq` have diverged, so that case is handled like a
    /// transport error but additionally resets `last_acked_seq` to restart
    /// the whole job from seq 0.
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
    desktop_device_id: &str,
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
    // Handshake succeeded: reset an unreachable peer back to paired (spec
    // §7) as soon as we know the connection works, independent of whether
    // the rest of this job attempt goes on to succeed or fail.
    mark_peer_reachable(storage, desktop_device_id);

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
        // The mobile side REQUESTS an executor; it does not decide one. The
        // API key, the tier policy and the daily cap all live on the desktop
        // (see `fungwire_server::dispatch_cloud_stt`), which re-checks every
        // one of them before any audio leaves it and answers
        // `Control::Error{code:"cloud_disabled"}` if the answer is no. Asking
        // for "cloud" here can therefore never bypass a desktop's policy —
        // it can only decline to use a policy that is already permissive.
        executor: ctx.executor.clone(),
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

    // All segments sent (or nothing left to send on a resumed attempt). The
    // server now transcribes every segment in a single worker invocation
    // (Final review #3), sending one Progress{stage:"transcribing"} before
    // the final Result -- this loop makes no assumption about the count, so
    // it keeps working regardless of how many Progress messages arrive.
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

/// Exponential backoff for reconnect attempt `attempt` (1-indexed): 500ms,
/// 1s, 2s, 4s, ... capped at [`RECONNECT_BACKOFF_CAP`] so a long-running
/// reconnect loop (see [`RECONNECT_BUDGET`]) doesn't end up sleeping for
/// minutes between individual attempts.
fn backoff_delay(attempt: u32) -> Duration {
    let millis =
        RECONNECT_BACKOFF_BASE.as_millis() as u64 * 2u64.pow(attempt.saturating_sub(1).min(4));
    Duration::from_millis(millis).min(RECONNECT_BACKOFF_CAP)
}

/// Runs on a dedicated `std::thread` (spawned by
/// `delegate_transcription_inner`); drives the whole job to completion or
/// failure, writing every state transition into the `delegated_jobs` row so
/// `fungwire_job_poll` can observe it. `reconnect_budget` is
/// [`RECONNECT_BUDGET`] in production; tests inject a short budget instead
/// so the exhausted-retries path doesn't have to actually wait minutes.
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
    reconnect_budget: Duration,
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
    // Wall-clock budget for the whole reconnect loop (started once, not
    // reset per attempt) — the initial attempt above always runs regardless
    // of this budget; only reconnects after a failure count against it.
    let reconnect_deadline = Instant::now();
    // Shared give-up path for both the transport-error and resume_gap
    // branches below: exceeding the budget marks the job failed and the
    // peer unreachable; otherwise sleeps the next backoff and lets the
    // caller's loop retry.
    let give_up_or_backoff = |reconnect_attempts: &mut u32, ctx: &JobContext, detail: &str| -> bool {
        *reconnect_attempts += 1;
        if reconnect_deadline.elapsed() >= reconnect_budget {
            // A terminal job state is visible to pollers immediately. Update
            // the paired-peer state first so observers can never see
            // "failed because unreachable" while the peer still appears
            // reachable.
            mark_peer_unreachable(&storage, &desktop_device_id);
            let _ = update_job(
                &storage,
                ctx,
                "failed",
                0,
                Some(&format!(
                    "unreachable after {reconnect_attempts} reconnect attempts over {:?}: {detail}",
                    reconnect_deadline.elapsed()
                )),
            );
            true
        } else {
            thread::sleep(backoff_delay(*reconnect_attempts));
            false
        }
    };

    loop {
        let resume_from = (last_acked + 1) as u32;
        let outcome = attempt_transfer(
            &storage,
            &ctx,
            &endpoint,
            &own_device_id,
            &desktop_device_id,
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
            AttemptOutcome::ServerError(code, message) if code == "resume_gap" => {
                // The worker's persisted job dir and our idea of
                // "already acked" have diverged (e.g. its dir was lost
                // between connections). There is no partial recovery from
                // this: reset to seq 0 and retry under the same
                // reconnect/backoff budget as a transport error, so a
                // permanently confused peer still gives up eventually
                // instead of looping forever.
                last_acked = -1;
                if give_up_or_backoff(&mut reconnect_attempts, &ctx, &format!("{code}: {message}")) {
                    return;
                }
                // Loop back and try again from seq 0.
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
                if give_up_or_backoff(&mut reconnect_attempts, &ctx, &message) {
                    return;
                }
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
    executor: String,
) -> AppResult<String> {
    delegate_transcription_inner_with_budget(
        storage,
        app_data,
        project_id,
        recording_id,
        desktop_device_id,
        endpoint,
        own_device_id,
        executor,
        RECONNECT_BUDGET,
    )
}

/// Same as [`delegate_transcription_inner`] but with the reconnect budget
/// exposed as a parameter, so tests can inject a short budget instead of
/// [`RECONNECT_BUDGET`]'s production value (which would otherwise force an
/// exhausted-retries test to actually wait minutes).
#[allow(clippy::too_many_arguments)]
fn delegate_transcription_inner_with_budget(
    storage: Arc<genesis_block_native::Storage>,
    app_data: PathBuf,
    project_id: String,
    recording_id: String,
    desktop_device_id: String,
    endpoint: String,
    own_device_id: String,
    executor: String,
    reconnect_budget: Duration,
) -> AppResult<String> {
    let executor = normalize_executor(&executor).to_string();
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
                "executor": executor,
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
        executor,
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
            reconnect_budget,
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
    executor: String,
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
        executor,
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
        &["state", "progress", "executor", "checkpoint_json"],
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
    let executor = row
        .get("delegated_jobs.executor")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let error = row
        .get("delegated_jobs.checkpoint_json")
        .and_then(|checkpoint| checkpoint.get("error"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    Ok(JobPollOutput {
        state: job_state,
        progress,
        executor,
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
    /// gets a real, valid key. `trust_state` is exposed so tests can seed a
    /// peer that starts `"unreachable"` or `"revoked"` instead of the usual
    /// `"paired"`.
    fn pair_desktop_on_mobile(
        mobile_storage: &Storage,
        desktop_device_id: &str,
        desktop_app_data: &Path,
        trust_state: &str,
    ) {
        let pub_b64 = public_key_b64_in_dir(desktop_app_data).unwrap();
        let timestamp = now();
        crate::genesis_adapter::commit_rows(
            mobile_storage,
            vec![crate::genesis_adapter::upsert(
                "paired_devices",
                json!({
                    "id": desktop_device_id, "name": "FUNG Desktop", "endpoint": "",
                    "trust_state": trust_state, "pairing_proof_hash": "sess-desktop",
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
        pair_desktop_on_mobile(&mobile_storage, "desktop-reach", desktop_dir.path(), "paired");
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

    /// The mobile half of the Phase 3 status probe, against the real desktop
    /// server: the policy written on the desktop side is what the mobile
    /// command returns. Covers both values so a hardcoded answer can't pass.
    #[test]
    fn desktop_status_probe_inner_reads_the_desktops_cloud_policy() {
        for policy_enabled in [true, false] {
            let mobile_dir = tempfile::tempdir().unwrap();
            let desktop_dir = tempfile::tempdir().unwrap();
            ensure_identity_in_dir(mobile_dir.path()).unwrap();
            ensure_identity_in_dir(desktop_dir.path()).unwrap();
            let (genesis_path, mobile_storage) = open_genesis();
            pair_desktop_on_mobile(&mobile_storage, "desktop-status", desktop_dir.path(), "paired");
            pair_mobile_on_desktop(desktop_dir.path(), "mobile-status", mobile_dir.path());

            let policy_conn = paired_devices_connection_at(desktop_dir.path()).unwrap();
            crate::policy::save_policy(
                &policy_conn,
                &crate::policy::TierPolicy {
                    stt_cloud_enabled: policy_enabled,
                    llm_cloud_enabled: false,
                    daily_cap: 20,
                },
            )
            .unwrap();

            let (addr, server_handle) = spawn_server(desktop_dir.path().to_path_buf());

            let probe = desktop_status_probe_inner(
                &mobile_storage,
                mobile_dir.path(),
                "desktop-status",
                &addr.to_string(),
                "mobile-status",
            )
            .expect("status probe against a paired, listening desktop must succeed");
            assert_eq!(
                probe.stt_cloud_enabled, policy_enabled,
                "the probe must report the DESKTOP's policy, not a local default"
            );

            server_handle.join().unwrap().ok();
            drop(mobile_storage);
            let _ = std::fs::remove_dir_all(genesis_path);
        }
    }

    /// An unknown/unpaired peer must fail the probe outright rather than
    /// yielding a value — `bridge.ts`'s `desktopCloudEnabled` turns this Err
    /// into a fail-closed `false`, which is only correct if a probe that
    /// cannot establish trust never returns `Ok`.
    #[test]
    fn desktop_status_probe_inner_errors_for_unknown_peer() {
        let mobile_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();

        let result = desktop_status_probe_inner(
            &mobile_storage,
            mobile_dir.path(),
            "desktop-never-paired",
            "127.0.0.1:1",
            "mobile-status",
        );
        assert!(result.is_err(), "unpaired peer must not yield a policy answer");

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

        pair_desktop_on_mobile(&mobile_storage, "desktop-e2e", desktop_dir.path(), "paired");
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
            "local".to_string(),
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

    /// The cloud delegate action's real contract. Three things must hold for
    /// the "☁ คลาวด์" badge to mean anything:
    ///  1. the executor the mobile UI picked is written onto the job row at
    ///     delegation time (Genesis schema v7's `delegated_jobs.executor`),
    ///  2. `job_poll_inner` reads it back, so the badge survives a remount
    ///     rather than depending on in-memory component state, and
    ///  3. it survives the run-to-completion `update_job` writes, which
    ///     re-upsert the whole row and would otherwise blank the column.
    ///
    /// Run against the real loopback desktop so (3) is exercised by genuine
    /// state transitions. The desktop's default tier policy has cloud STT
    /// off, so a `"cloud"` job legitimately ends `failed` here — which is the
    /// stronger case for (3) anyway: the executor must still be readable on a
    /// job whose cloud dispatch was refused.
    #[test]
    fn delegated_job_persists_the_requested_executor() {
        for executor in ["cloud", "local"] {
            let mobile_dir = tempfile::tempdir().unwrap();
            let desktop_dir = tempfile::tempdir().unwrap();
            let segment_dir = tempfile::tempdir().unwrap();
            ensure_identity_in_dir(mobile_dir.path()).unwrap();
            ensure_identity_in_dir(desktop_dir.path()).unwrap();
            let (genesis_path, mobile_storage) = open_genesis();
            let mobile_storage = Arc::new(mobile_storage);

            pair_desktop_on_mobile(&mobile_storage, "desktop-exec", desktop_dir.path(), "paired");
            pair_mobile_on_desktop(desktop_dir.path(), "mobile-exec", mobile_dir.path());
            seed_recording_with_segments(
                &mobile_storage,
                "proj-exec",
                "rec-exec",
                segment_dir.path(),
                &[512],
            );

            let (addr, server_handle) = spawn_server(desktop_dir.path().to_path_buf());

            let job_id = delegate_transcription_inner(
                mobile_storage.clone(),
                mobile_dir.path().to_path_buf(),
                "proj-exec".to_string(),
                "rec-exec".to_string(),
                "desktop-exec".to_string(),
                addr.to_string(),
                "mobile-exec".to_string(),
                executor.to_string(),
            )
            .expect("delegate transcription");

            assert_eq!(
                job_poll_inner(&mobile_storage, &job_id).unwrap().executor.as_deref(),
                Some(executor),
                "the requested executor must be on the row the instant the job is created"
            );

            let mut final_state = String::new();
            for _ in 0..100 {
                let poll = job_poll_inner(&mobile_storage, &job_id).expect("poll job");
                if poll.state == "completed" || poll.state == "failed" {
                    final_state = poll.state;
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
            assert!(!final_state.is_empty(), "job must reach a terminal state");
            // The terminal state itself is the signal that the real executor
            // value reached the wire: the loopback desktop's default policy
            // (see `TierPolicy::default`) has cloud STT off, so a genuine
            // `"cloud"` `Control::JobStart` is refused and the job ends
            // `"failed"`, while a genuine `"local"` one runs the desktop's
            // fake-whisper pipeline and ends `"completed"`. If
            // `Control::JobStart`'s `executor` field were ever hardcoded back
            // to `"local"` (its old pre-Task-10 value), the `"cloud"`
            // iteration would silently complete instead of failing, and this
            // assertion is what would catch that.
            assert_eq!(
                final_state,
                if executor == "cloud" { "failed" } else { "completed" },
                "a '{executor}' request must reach the desktop as '{executor}' and be handled \
                 accordingly (cloud is refused by the loopback desktop's default policy; local \
                 succeeds via the fake pipeline)"
            );
            assert_eq!(
                job_poll_inner(&mobile_storage, &job_id).unwrap().executor.as_deref(),
                Some(executor),
                "the executor must survive every update_job write, not just the first"
            );

            server_handle.join().unwrap().ok();
            drop(mobile_storage);
            let _ = std::fs::remove_dir_all(genesis_path);
        }
    }

    /// Anything that isn't literally `"cloud"` must be pinned to `"local"`
    /// before it can reach the wire. The desktop already defends itself (it
    /// treats an unknown `JobStart.executor` as local), but the value is also
    /// persisted and read back as a provenance claim — so a junk value must
    /// never be able to sit on a row and later render as a cloud badge.
    #[test]
    fn unknown_executor_values_normalize_to_local() {
        assert_eq!(normalize_executor("cloud"), "cloud");
        assert_eq!(normalize_executor("local"), "local");
        assert_eq!(normalize_executor("Cloud"), "local");
        assert_eq!(normalize_executor(""), "local");
        assert_eq!(normalize_executor("gpu-farm"), "local");
    }

    // -------------------------------------------------------------
    // Reconnect / resume
    // -------------------------------------------------------------

    /// Pure unit test for the resume-offset arithmetic used on every
    /// reconnect: `resume_from_seq` is always `last_acked_seq + 1`,
    /// independent of any networking. The true end-to-end K>0 case (a real
    /// mid-transfer drop after at least one segment is acked, followed by a
    /// genuine resume) is covered against the real worker in
    /// `fungwire_server::tests::resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes`.
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
    /// reach `completed` — the K=0 edge of the reconnect-with-backoff loop.
    /// The K>0 case (a real mid-transfer resume after at least one segment
    /// is genuinely acked) is covered in
    /// `fungwire_server::tests::resume_from_seq_reloads_persisted_segments_after_reconnect_and_completes`,
    /// which drives the wire protocol directly against the real worker so
    /// it can force the drop at an exact `last_acked_seq`.
    #[test]
    fn delegate_transcription_reconnects_after_early_drop_and_completes() {
        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        let segment_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        let mobile_storage = Arc::new(mobile_storage);

        pair_desktop_on_mobile(&mobile_storage, "desktop-resume", desktop_dir.path(), "paired");
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
            "local".to_string(),
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

    /// After the reconnect time budget is exhausted, `run_transfer` must
    /// mark the `delegated_jobs` row `failed` AND flip the peer's
    /// `paired_devices.trust_state` to `"unreachable"` on the mobile side
    /// (brief step 2.3). Every connection attempt fails immediately with
    /// "connection refused" (the port is bound then dropped before any
    /// client ever connects, same trick as `closed_port_is_not_reachable`),
    /// so this exercises the exhausted-retries path end-to-end without
    /// needing a real or fake server on the other end.
    ///
    /// The production budget ([`RECONNECT_BUDGET`], 2 minutes) would make
    /// this test itself take minutes, so it drives
    /// `delegate_transcription_inner_with_budget` with a short injected
    /// budget instead — long enough to exercise at least one real
    /// backoff-and-retry cycle before giving up, short enough that the test
    /// finishes in about a second.
    #[test]
    fn exhausted_reconnects_marks_job_failed_and_peer_unreachable() {
        const SHORT_RECONNECT_BUDGET: Duration = Duration::from_millis(700);

        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        let segment_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        let mobile_storage = Arc::new(mobile_storage);

        pair_desktop_on_mobile(&mobile_storage, "desktop-unreachable", desktop_dir.path(), "paired");
        seed_recording_with_segments(
            &mobile_storage,
            "proj-unreachable",
            "rec-unreachable",
            segment_dir.path(),
            &[512],
        );

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let job_id = delegate_transcription_inner_with_budget(
            mobile_storage.clone(),
            mobile_dir.path().to_path_buf(),
            "proj-unreachable".to_string(),
            "rec-unreachable".to_string(),
            "desktop-unreachable".to_string(),
            addr.to_string(),
            "mobile-unreachable".to_string(),
            "local".to_string(),
            SHORT_RECONNECT_BUDGET,
        )
        .expect("delegate transcription");

        // Bounded to well under the production budget: the short injected
        // budget above must make this resolve in ~1-2s, never anywhere near
        // SHORT_RECONNECT_BUDGET's real-world equivalent of minutes.
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
            final_state, "failed",
            "job must fail once the reconnect budget is exhausted"
        );

        let rows = crate::genesis_adapter::query(
            &mobile_storage,
            "paired_devices",
            &["trust_state"],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                json!("desktop-unreachable"),
            )],
            1,
        )
        .expect("query paired_devices");
        let trust_state = rows[0]
            .get("paired_devices.trust_state")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert_eq!(
            trust_state, "unreachable",
            "peer must be flipped to unreachable after exhausting the retry budget"
        );

        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }

    // -------------------------------------------------------------
    // Unreachable -> paired reset (final-review fix wave B)
    // -------------------------------------------------------------

    /// `lookup_peer_public_key` (used before connecting, for both the
    /// reachability probe and the delegate's reconnect loop) must accept a
    /// peer that is currently `"unreachable"` — otherwise a peer that ever
    /// went unreachable could never be dialed again to recover — but must
    /// still reject `"revoked"`, which is a stronger, user-driven signal.
    #[test]
    fn lookup_peer_public_key_accepts_unreachable_but_rejects_revoked() {
        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();

        pair_desktop_on_mobile(&mobile_storage, "desktop-unreach-ok", desktop_dir.path(), "unreachable");
        assert!(
            lookup_peer_public_key(&mobile_storage, "desktop-unreach-ok").is_ok(),
            "an unreachable peer must still be dialable so a retry/probe can recover it"
        );

        pair_desktop_on_mobile(&mobile_storage, "desktop-revoked", desktop_dir.path(), "revoked");
        assert!(
            lookup_peer_public_key(&mobile_storage, "desktop-revoked").is_err(),
            "a revoked peer must never be dialed again"
        );

        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }

    /// End-to-end: a peer seeded as `"unreachable"` in Genesis
    /// `paired_devices` must be reset to `"paired"` once a real handshake
    /// against it succeeds (design spec §7: "reset to paired on next
    /// success") — driven against the real loopback Task 6/7 server, not a
    /// mock, so this proves both halves of the fix together: the lookup
    /// that allows dialing an unreachable peer at all, and the reset that
    /// runs after `into_transport_mode` succeeds.
    #[test]
    fn desktop_reachable_inner_resets_unreachable_peer_to_paired_on_success() {
        let mobile_dir = tempfile::tempdir().unwrap();
        let desktop_dir = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(mobile_dir.path()).unwrap();
        ensure_identity_in_dir(desktop_dir.path()).unwrap();
        let (genesis_path, mobile_storage) = open_genesis();
        pair_desktop_on_mobile(&mobile_storage, "desktop-recover", desktop_dir.path(), "unreachable");
        pair_mobile_on_desktop(desktop_dir.path(), "mobile-recover", mobile_dir.path());

        let (addr, server_handle) = spawn_server(desktop_dir.path().to_path_buf());

        let reachable = desktop_reachable_inner(
            &mobile_storage,
            mobile_dir.path(),
            "desktop-recover",
            &addr.to_string(),
            "mobile-recover",
        );
        assert!(
            reachable,
            "an unreachable-marked peer with an open port must still be dialable and reachable"
        );

        let rows = crate::genesis_adapter::query(
            &mobile_storage,
            "paired_devices",
            &["trust_state"],
            vec![crate::genesis_adapter::eq(
                "paired_devices",
                "id",
                json!("desktop-recover"),
            )],
            1,
        )
        .expect("query paired_devices");
        let trust_state = rows[0]
            .get("paired_devices.trust_state")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert_eq!(
            trust_state, "paired",
            "a successful handshake must reset trust_state back to paired"
        );

        server_handle.join().unwrap().ok();
        drop(mobile_storage);
        let _ = std::fs::remove_dir_all(genesis_path);
    }
}
