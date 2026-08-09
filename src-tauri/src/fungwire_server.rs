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

use crate::fungwire::{noise_responder, read_frame, write_frame, Control, NoiseChannel, CTRL_MAX};
use crate::{AppResult, AppState};
use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;
use tauri::State;

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

    thread::spawn(move || {
        while !loop_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    // Per-connection thread (the fix vs. the mobile gateway's
                    // inline handling): a handshake + job loop can run for
                    // the duration of a whole transcription job, so it must
                    // never block the accept loop or other peers.
                    let storage = storage.clone();
                    let app_data = app_data.clone();
                    let jobs = loop_jobs.clone();
                    let peers = loop_peers.clone();
                    peers.fetch_add(1, Ordering::SeqCst);
                    thread::spawn(move || {
                        if let Err(e) = handle_connection(stream, &storage, &app_data, &jobs) {
                            eprintln!("fungwire connection ended: {e}");
                        }
                        peers.fetch_sub(1, Ordering::SeqCst);
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
    run_job_loop(&mut channel, storage, app_data, &device_id, jobs)
}

/// Placeholder job loop — Task 7 implements the real `JobStart`/`Chunk`/
/// `Result` protocol exchange over `channel`. For Task 6 this only proves
/// the handshake reached transport mode, so the handshake path is
/// independently testable before the job protocol exists.
///
/// Note for Task 7: when this loop calls `channel.recv_bytes(max)`, `max`
/// must stay `<= CHUNK_MAX` — `NoiseChannel::recv_bytes` sizes its read
/// against that assumption (see fungwire.rs Task 4 review note).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_job_loop(
    _channel: &mut NoiseChannel<TcpStream>,
    _storage: &genesis_block_native::Storage,
    _app_data: &Path,
    device_id: &str,
    _jobs: &Arc<AtomicUsize>,
) -> Result<(), String> {
    eprintln!("fungwire handshake complete with device {device_id}");
    Ok(())
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
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &jobs)
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
        let server_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &storage, &server_app_data_path, &jobs)
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
}
