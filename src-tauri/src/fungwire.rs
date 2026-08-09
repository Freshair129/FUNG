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
}
