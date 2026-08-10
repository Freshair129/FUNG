//! FUNGWIRE wire protocol primitives: length-prefixed framing, the tagged
//! `Control` message enum, and manifest hashing. Pure logic — no networking
//! or crypto. The Noise transport (Task 4) and server/client (Tasks 6-8)
//! consume these.
//!
//! Items here are not yet called from any other module, so `cargo build`
//! reports dead-code warnings until Task 4+ wires them in. That's expected
//! and left as-is (no project-wide `deny(warnings)` is configured).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::time::Duration;

/// Max frame size for control-channel messages (JSON-encoded `Control`).
pub const CTRL_MAX: usize = 64 * 1024;
/// Max frame size for a single audio chunk payload.
pub const CHUNK_MAX: usize = 4 * 1024 * 1024;

/// Writes `payload` as a length-prefixed frame: a 4-byte big-endian length
/// followed by the raw bytes.
pub fn write_frame(w: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let len = payload.len() as u32;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

/// Reads a length-prefixed frame written by [`write_frame`]. Errors (rather
/// than panics) if the declared length exceeds `max`.
pub fn read_frame(r: &mut impl Read, max: usize) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame {len} exceeds cap {max}"),
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Default for [`Control::JobStart`]'s `executor` field when a peer's
/// `JobStart` JSON omits it entirely — i.e. a mobile client built before
/// Phase 3's BYOM cloud keys existed. Running on the desktop's own local
/// faster-whisper pipeline is exactly what those peers already expected, so
/// decoding an old message must keep doing that; the cloud path is only ever
/// entered when a peer asks for it by name.
fn default_executor() -> String {
    "local".to_string()
}

/// All control-channel messages exchanged over the FUNGWIRE tunnel, tagged
/// by `type` in JSON for forward-compatible decoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Control {
    Hello {
        device_id: String,
    },
    JobStart {
        job_id: String,
        operation: String,
        manifest_hash: String,
        segment_count: u32,
        total_bytes: u64,
        profile: String,
        resume_from_seq: u32,
        /// Ordered per-segment sha256 hex checksums the sender is about to
        /// transfer. `manifest_hash` binds this set (see [`manifest_hash`]);
        /// the receiver (Task 7) recomputes `manifest_hash(&checksums)` and
        /// rejects the job if it doesn't match, then verifies each received
        /// segment's own digest against `checksums[seq]` as it arrives.
        checksums: Vec<String>,
        /// Which executor the desktop should run this job on: `"local"` (the
        /// on-device faster-whisper pipeline) or `"cloud"` (BYOM cloud STT
        /// through the desktop user's own API key, gated by that desktop's
        /// tier policy — see `fungwire_server::dispatch_cloud_stt`). Any
        /// other value is treated as `"local"`.
        ///
        /// Defaulted rather than required so a pre-Phase-3 client, whose
        /// `JobStart` JSON has no `executor` key at all, still decodes here
        /// and still transcribes locally (see [`default_executor`]).
        #[serde(default = "default_executor")]
        executor: String,
    },
    Chunk {
        job_id: String,
        seq: u32,
        len: u32,
    },
    ChunkAck {
        job_id: String,
        seq: u32,
    },
    Progress {
        job_id: String,
        percent: u8,
        stage: String,
    },
    Result {
        job_id: String,
        duration_ms: i64,
        segments: Vec<Segment>,
    },
    Error {
        job_id: String,
        code: String,
        message: String,
    },
    Cancel {
        job_id: String,
    },
    Heartbeat,
    HeartbeatAck,
    /// Phase 3 status probe: asks an already-authenticated desktop what its
    /// own cloud-tier policy currently says, so the mobile UI can decide
    /// whether to offer a cloud delegate action at all.
    ///
    /// Deliberately carries NO `device_id`: unlike the cleartext
    /// [`Control::Hello`] that opens a connection, this travels inside the
    /// completed Noise KK session, where the caller's identity is already
    /// proven cryptographically. Re-stating it here would add a field the
    /// receiver must either ignore or — worse — trust over the handshake.
    StatusRequest,
    /// Answer to [`Control::StatusRequest`].
    ///
    /// A deliberate *subset* of `fungwire_server::FungwireStatus`, not a
    /// mirror of it: `enabled` is trivially true for anyone who got far
    /// enough to ask, `bind` is the address the asker just dialled, and
    /// `connected_peers`/`active_jobs` are desktop-local diagnostics with no
    /// consumer on the mobile side. Only the policy bit crosses the wire.
    StatusReply {
        /// Whether the answering desktop's tier policy currently permits
        /// cloud STT. Purely informational to the peer — the desktop
        /// re-checks this (plus its key and daily cap) when a cloud job
        /// actually arrives, so a `true` here is never a standing grant.
        stt_cloud_enabled: bool,
    },
}

/// A single transcript segment carried in a [`Control::Result`] message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub confidence: Option<f64>,
}

impl Control {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("control serialize")
    }

    pub fn decode(bytes: &[u8]) -> serde_json::Result<Control> {
        serde_json::from_slice(bytes)
    }
}

/// Noise protocol string: KK pattern (both peers' static keys known in
/// advance — mutual authentication), Curve25519 DH, ChaCha20-Poly1305 AEAD,
/// BLAKE2s hash.
pub const NOISE_PARAMS: &str = "Noise_KK_25519_ChaChaPoly_BLAKE2s";

/// Maximum plaintext bytes that fit in a single Noise transport message.
///
/// The Noise spec hard-caps every transport message (ciphertext + 16-byte
/// AEAD tag) at 65535 bytes, and `snow` enforces that limit internally. That
/// is independent of, and much smaller than, [`CHUNK_MAX`] (4 MiB) — the cap
/// Task 3 defined for a *frame's* payload on the wire. A `NoiseChannel` frame
/// carries exactly one Noise transport message, so `send_bytes` cannot pass
/// a 4 MiB audio chunk through in one call.
///
/// We do not attempt to silently split large plaintext across multiple Noise
/// messages here: `send_bytes` rejects anything over this cap with an
/// explicit `io::Error` instead. Callers that need to move payloads larger
/// than ~64 KiB (Tasks 7/8, streaming audio chunks) must split them into
/// multiple `send_bytes` calls themselves. In practice this is a non-issue:
/// a 5s m4a audio chunk is tens of KB, well under the cap.
pub const NOISE_MAX_PLAINTEXT: usize = 65535 - 16;

/// Hard cap on a single Noise transport message (ciphertext + 16-byte AEAD
/// tag) per the Noise spec; `snow` enforces this internally. `NoiseChannel`'s
/// scratch buffer (see [`NoiseChannel::new`]) is sized against this, NOT
/// [`CHUNK_MAX`] -- a Noise frame on the wire is always exactly one Noise
/// message, however large the caller's logical payload (an audio chunk) is,
/// because larger payloads are split into multiple `send_bytes`/`recv_bytes`
/// calls (see [`NOISE_MAX_PLAINTEXT`]) rather than ever appearing as a single
/// oversized Noise message.
pub const NOISE_MAX_MESSAGE: usize = 65535;

/// Builds a Noise `KK` handshake state as the initiator, using `local`'s
/// X25519 static secret and `remote`'s X25519 static public key.
pub fn noise_initiator(local: &[u8; 32], remote: &[u8; 32]) -> Result<snow::HandshakeState, String> {
    snow::Builder::new(NOISE_PARAMS.parse().map_err(|e| format!("noise params: {e}"))?)
        .local_private_key(local)
        .remote_public_key(remote)
        .build_initiator()
        .map_err(|e| format!("noise initiator: {e}"))
}

/// Builds a Noise `KK` handshake state as the responder, using `local`'s
/// X25519 static secret and `remote`'s expected X25519 static public key.
/// If `remote` does not match the initiator's actual static key, the
/// handshake's `read_message` call fails (mutual authentication).
pub fn noise_responder(local: &[u8; 32], remote: &[u8; 32]) -> Result<snow::HandshakeState, String> {
    snow::Builder::new(NOISE_PARAMS.parse().map_err(|e| format!("noise params: {e}"))?)
        .local_private_key(local)
        .remote_public_key(remote)
        .build_responder()
        .map_err(|e| format!("noise responder: {e}"))
}

/// Wraps a byte stream with a completed Noise transport session, framing
/// each Noise transport message inside one [`write_frame`]/[`read_frame`]
/// length-prefixed frame.
///
/// Each `send_bytes`/`recv_bytes` call maps to exactly one Noise transport
/// message — see [`NOISE_MAX_PLAINTEXT`] for the per-call plaintext cap.
pub struct NoiseChannel<S: Read + Write> {
    stream: S,
    transport: snow::TransportState,
    buf: Vec<u8>,
}

impl<S: Read + Write> NoiseChannel<S> {
    pub fn new(stream: S, transport: snow::TransportState) -> Self {
        Self {
            stream,
            transport,
            // Sized for exactly one Noise transport message (65535 bytes max
            // per the Noise spec), not CHUNK_MAX (4 MiB): a Noise frame never
            // carries more than one Noise message (see NOISE_MAX_MESSAGE),
            // so a CHUNK_MAX-sized scratch buffer was a ~64x over-allocation
            // per connection that only mattered because nothing had checked
            // it against the actual wire format before (final review #6b).
            // +16 is slack headroom, not a second AEAD tag.
            buf: vec![0u8; NOISE_MAX_MESSAGE + 16],
        }
    }

    /// Encrypt `plain` as a single Noise transport message, write it as one
    /// length-prefixed frame. Errors (rather than silently truncating or
    /// splitting) if `plain` exceeds [`NOISE_MAX_PLAINTEXT`]; the caller must
    /// split larger payloads into multiple calls.
    pub fn send_bytes(&mut self, plain: &[u8]) -> io::Result<()> {
        if plain.len() > NOISE_MAX_PLAINTEXT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "plaintext {} exceeds Noise per-message cap {NOISE_MAX_PLAINTEXT}; caller must split",
                    plain.len()
                ),
            ));
        }
        let n = self
            .transport
            .write_message(plain, &mut self.buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("noise encrypt: {e}")))?;
        write_frame(&mut self.stream, &self.buf[..n])
    }

    /// Encode and encrypt a [`Control`] message.
    pub fn send(&mut self, c: &Control) -> io::Result<()> {
        self.send_bytes(&c.encode())
    }

    /// Read one frame and decrypt it, returning the plaintext. `max` is a
    /// caller-side hint (e.g. `CHUNK_MAX` for an audio sub-frame, `CTRL_MAX`
    /// for a control message) about how big the *logical* payload it's
    /// expecting might be -- but the frame actually on the wire is always at
    /// most one Noise transport message (see [`NOISE_MAX_MESSAGE`]),
    /// regardless of how large `max` is, because larger payloads are always
    /// pre-split by the sender into multiple `send_bytes` calls. So the
    /// frame-read cap here is `min(max, NOISE_MAX_MESSAGE) + 16`, not
    /// `max + 256`: capping at the Noise hard limit keeps this bounded by
    /// `self.buf`'s actual size (see [`NoiseChannel::new`]) even if a peer's
    /// declared frame length lies up near a large `max`.
    pub fn recv_bytes(&mut self, max: usize) -> io::Result<Vec<u8>> {
        let frame_cap = max.min(NOISE_MAX_MESSAGE) + 16;
        let frame = read_frame(&mut self.stream, frame_cap)?;
        let n = self
            .transport
            .read_message(&frame, &mut self.buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("noise decrypt: {e}")))?;
        Ok(self.buf[..n].to_vec())
    }

    /// Read and decode a [`Control`] message (bounded by [`CTRL_MAX`]).
    pub fn recv_control(&mut self) -> io::Result<Control> {
        let bytes = self.recv_bytes(CTRL_MAX)?;
        Control::decode(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("control decode: {e}")))
    }

    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }
}

/// TTL after which an orphaned job directory (`<app_data>/fungwire/jobs/
/// <job_id>/`) is considered abandoned and safe for the sweep in
/// `fungwire_server` to delete.
///
/// A job dir outlives its connection on purpose (see `fungwire_server::
/// job_dir_path`'s doc comment) so a client that drops mid-transfer can
/// resume later. But if the mobile client is gone for good -- app killed, or
/// its own retry budget exhausted against a dead socket -- nothing ever
/// finishes the job or sends `Cancel`, so the outcome is `transport_error`
/// (deliberately non-terminal) and the directory, containing partial audio
/// segments, would otherwise sit on disk forever. 24h is generous enough
/// that any transfer a user is actually still going to resume today will
/// have done so, while bounding how long stray audio can accumulate
/// (final review #4).
pub const JOB_DIR_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Pure predicate behind the job-dir sweep: is something `age` old considered
/// expired against `ttl`? Factored out of the filesystem-walking sweep so it
/// can be unit-tested directly -- setting a file/dir mtime in the past is
/// awkward to do portably in a test (especially on Windows), but this needs
/// no filesystem at all.
pub fn is_expired(age: Duration, ttl: Duration) -> bool {
    age >= ttl
}

/// Sha256 hex digest of the ordered segment checksums, joined by `\n`.
/// Order-sensitive by design: it stands in for the manifest of chunks a job
/// was built from, so reordering must change the hash.
pub fn manifest_hash(segment_checksums: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(segment_checksums.join("\n").as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn frame_roundtrip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"hello").unwrap();
        let mut cur = Cursor::new(buf);
        assert_eq!(read_frame(&mut cur, CTRL_MAX).unwrap(), b"hello");
    }

    #[test]
    fn frame_over_cap_errors() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &vec![0u8; 100]).unwrap();
        let mut cur = Cursor::new(buf);
        assert!(read_frame(&mut cur, 10).is_err());
    }

    #[test]
    fn control_encode_decode() {
        let c = Control::JobStart {
            job_id: "j1".into(),
            operation: "transcript.transcribe".into(),
            manifest_hash: "abc".into(),
            segment_count: 3,
            total_bytes: 900,
            profile: "cpu".into(),
            resume_from_seq: 0,
            checksums: vec!["a".into(), "b".into(), "c".into()],
            executor: "local".into(),
        };
        let bytes = c.encode();
        match Control::decode(&bytes).unwrap() {
            Control::JobStart {
                job_id,
                segment_count,
                ..
            } => {
                assert_eq!(job_id, "j1");
                assert_eq!(segment_count, 3);
            }
            _ => panic!("wrong variant"),
        }
    }

    /// Wire tolerance for pre-Phase-3 peers: a `JobStart` serialized before
    /// `executor` existed has no such key at all, and must still decode —
    /// as a LOCAL job, never a cloud one.
    #[test]
    fn job_start_without_executor_decodes_as_local() {
        let legacy = br#"{"type":"JobStart","job_id":"j-old","operation":"transcript.transcribe",
            "manifest_hash":"abc","segment_count":1,"total_bytes":10,"profile":"cpu",
            "resume_from_seq":0,"checksums":["a"]}"#;
        match Control::decode(legacy).expect("legacy JobStart must still decode") {
            Control::JobStart { executor, .. } => assert_eq!(executor, "local"),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    /// The Phase 3 status-probe pair must survive the same tagged-JSON
    /// round-trip as every other control message — including `StatusRequest`,
    /// which is a *unit* variant and so serializes to nothing but its tag.
    #[test]
    fn status_request_and_reply_roundtrip() {
        match Control::decode(&Control::StatusRequest.encode())
            .expect("StatusRequest must decode")
        {
            Control::StatusRequest => {}
            other => panic!("wrong variant: {other:?}"),
        }
        for enabled in [true, false] {
            let encoded = Control::StatusReply {
                stt_cloud_enabled: enabled,
            }
            .encode();
            match Control::decode(&encoded).expect("StatusReply must decode") {
                Control::StatusReply { stt_cloud_enabled } => assert_eq!(
                    stt_cloud_enabled, enabled,
                    "the policy bit must survive the round-trip unchanged"
                ),
                other => panic!("wrong variant: {other:?}"),
            }
        }
    }

    #[test]
    fn is_expired_true_when_age_at_or_past_ttl() {
        let ttl = Duration::from_secs(60 * 60 * 24);
        assert!(is_expired(ttl, ttl), "age exactly equal to ttl counts as expired");
        assert!(is_expired(ttl + Duration::from_secs(1), ttl));
        assert!(is_expired(Duration::from_secs(60 * 60 * 24 * 7), ttl));
    }

    #[test]
    fn is_expired_false_when_age_under_ttl() {
        let ttl = Duration::from_secs(60 * 60 * 24);
        assert!(!is_expired(Duration::ZERO, ttl));
        assert!(!is_expired(ttl - Duration::from_secs(1), ttl));
    }

    #[test]
    fn manifest_hash_is_order_sensitive() {
        let a = manifest_hash(&["x".into(), "y".into()]);
        let b = manifest_hash(&["y".into(), "x".into()]);
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn noise_kk_handshake_and_transport_roundtrip() {
        // Simulate two peers over an in-memory duplex using paired X25519 keys.
        use crate::device_identity::{ensure_identity_in_dir, x25519_static_secret_in_dir, x25519_public_from_ed25519_b64, public_key_b64_in_dir};
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(a.path()).unwrap();
        ensure_identity_in_dir(b.path()).unwrap();
        let a_sec = x25519_static_secret_in_dir(a.path()).unwrap();
        let b_sec = x25519_static_secret_in_dir(b.path()).unwrap();
        let a_pub = x25519_public_from_ed25519_b64(&public_key_b64_in_dir(a.path()).unwrap()).unwrap();
        let b_pub = x25519_public_from_ed25519_b64(&public_key_b64_in_dir(b.path()).unwrap()).unwrap();

        let mut ini = noise_initiator(&a_sec, &b_pub).unwrap();
        let mut resp = noise_responder(&b_sec, &a_pub).unwrap();
        let mut buf = [0u8; 1024];

        // KK is a two-message handshake: -> e, es, ss  ;  <- e, ee, se
        let n = ini.write_message(&[], &mut buf).unwrap();
        let mut rbuf = [0u8; 1024];
        resp.read_message(&buf[..n], &mut rbuf).unwrap();
        let n2 = resp.write_message(&[], &mut buf).unwrap();
        ini.read_message(&buf[..n2], &mut rbuf).unwrap();

        let mut ini_t = ini.into_transport_mode().unwrap();
        let mut resp_t = resp.into_transport_mode().unwrap();
        let n3 = ini_t.write_message(b"secret", &mut buf).unwrap();
        let m = resp_t.read_message(&buf[..n3], &mut rbuf).unwrap();
        assert_eq!(&rbuf[..m], b"secret");
    }

    #[test]
    fn noise_kk_rejects_wrong_remote_static() {
        use crate::device_identity::{ensure_identity_in_dir, x25519_static_secret_in_dir, x25519_public_from_ed25519_b64, public_key_b64_in_dir};
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let c = tempfile::tempdir().unwrap();
        for d in [&a,&b,&c] { ensure_identity_in_dir(d.path()).unwrap(); }
        let a_sec = x25519_static_secret_in_dir(a.path()).unwrap();
        let b_pub = x25519_public_from_ed25519_b64(&public_key_b64_in_dir(b.path()).unwrap()).unwrap();
        // Responder expects C, but initiator used B's key → handshake must fail.
        let c_sec = x25519_static_secret_in_dir(c.path()).unwrap();
        let a_pub = x25519_public_from_ed25519_b64(&public_key_b64_in_dir(a.path()).unwrap()).unwrap();
        let mut ini = noise_initiator(&a_sec, &b_pub).unwrap();
        let mut resp = noise_responder(&c_sec, &a_pub).unwrap();
        let mut buf = [0u8; 1024]; let mut rbuf = [0u8; 1024];
        let n = ini.write_message(&[], &mut buf).unwrap();
        assert!(resp.read_message(&buf[..n], &mut rbuf).is_err());
    }
}
