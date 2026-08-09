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
            buf: vec![0u8; CHUNK_MAX + 256],
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

    /// Read one frame (up to `max` + framing/AEAD headroom) and decrypt it,
    /// returning the plaintext.
    pub fn recv_bytes(&mut self, max: usize) -> io::Result<Vec<u8>> {
        let frame = read_frame(&mut self.stream, max + 256)?;
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
