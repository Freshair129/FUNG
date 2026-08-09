# Phase 2: FUNGWIRE v1 — LAN Tunnel + Desktop Job Worker — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A mobile device offloads transcription to its paired desktop over an encrypted LAN tunnel — mobile streams audio segments, desktop runs the existing Whisper pipeline and streams transcript segments + progress back.

**Architecture:** Fully synchronous std::net + `thread::spawn` (no tokio). Raw length-prefixed TCP frames wrapped in a Noise `KK` channel (mutual static-key auth + encryption via the `snow` crate); the Phase 1 ed25519 identity is converted to X25519 for Noise. Trust is anchored by the full public key published to Supabase `devices.public_key`, bound to the Phase 1 fingerprint. The desktop worker reuses the existing `run_python_worker(transcribe.py)` pipeline unchanged.

**Tech Stack:** Rust (std::net threads), `snow` (Noise), `curve25519-dalek` (ed25519→X25519), existing `ed25519-dalek`/`sha2`/`base64`, React 18 + supabase-js, GenesisBlockDB, rusqlite.

## Global Constraints

- **Depends on Phase 1 (PR #5) being merged to `main` first** — this plan branches from a `main` that contains `device_identity.rs`, desktop `paired_devices.db`, `mobile_pairing_complete`, and the `devices.public_key`-less schema. Do NOT start Task 1 until PR #5 is merged.
- Fully synchronous: no `tokio`, `async`, or `.await` anywhere. Thread-per-connection via `std::thread::spawn`.
- Framing: `u32` big-endian length prefix + payload. Control frames ≤ 64 KiB, binary chunk frames ≤ 4 MiB. Over cap → close connection.
- Noise params string EXACTLY: `Noise_KK_25519_ChaChaPoly_BLAKE2s`.
- The device private key never leaves Rust; no signing/secret API is exposed to the frontend.
- `sha256(peer_ed25519_public_bytes) == stored fingerprint` MUST be verified before trusting any peer static key, every handshake, both sides.
- LAN server binds `0.0.0.0:<port>` ONLY when the user enables FUNGWIRE; default OFF (mirror the `mobile_mcp_set_enabled` toggle pattern: `AtomicBool` stop flag + `Mutex<Option<Control>>` in `AppState`).
- UI labels Thai; identifiers English; named exports only; CSS hardcoded light + `.theme-dark` overrides.
- Supabase project ref `nqnrvqnijzovkrhxslfp`; migration applied ONLY at the controller gate, never by an implementer.
- Build on this host: rustc OOMs at default parallelism — always `cargo test -j 1 --manifest-path src-tauri/Cargo.toml`. `npx tsc --noEmit` must exit 0 after every TS task.
- Genesis `delegated_jobs` states: `queued|running|paused|completed|failed|cancelled`; `progress` 0–100. This plan is the table's first consumer.
- Spec: `docs/superpowers/specs/2026-08-09-phase-2-fungwire-design.md`.

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `supabase/migrations/20260810000000_device_pubkey_endpoint.sql` | 1 | devices +public_key/+lan_endpoint/+updated_at, extend update grant |
| `src-tauri/src/device_identity.rs` | 2 | +public_key_b64 / x25519 secret / x25519-from-ed25519 pub |
| `src-tauri/src/fungwire.rs` (new) | 3,4 | frame codec, control-message enum, manifest hash, Noise KK helpers |
| `src-tauri/src/fungwire_server.rs` (new) | 6,7 | desktop listener + per-connection worker (transcription) |
| `src-tauri/src/fungwire_client.rs` (new) | 8 | mobile connector + job driver |
| `src-tauri/src/lib.rs` | 2,5,6,7,9 | module wiring, commands, `paired_devices.db` +public_key, endpoint publisher |
| `src-tauri/src/mobile.rs` | 8,9 | mobile commands, peer key storage, delegate/poll/reachable |
| `src-tauri/Cargo.toml` | 4 | + snow, curve25519-dalek |
| `src/mobile/model.ts` | 10 | DelegatedJob type |
| `src/mobile/bridge.ts` | 10 | delegateTranscription / pollDelegatedJob / desktopReachable |
| `src/mobile/{MobileApp,CreativeStudio,TimelineScreen}.tsx` | 10 | delegate action + progress UI |
| `src/components/DevicePairingPanel.tsx` | 10 | desktop FUNGWIRE toggle + status |

Task order 1→2→3→4→5→6→7→8→9→10 (mostly sequential; 3 before 4; 6/7 need 2+3+4; 8 needs 3+4+5; 9 needs 1+5; 10 needs 8+9).

---

### Task 1: Supabase migration — devices public_key + LAN endpoint

**Files:** Create `supabase/migrations/20260810000000_device_pubkey_endpoint.sql`

**Interfaces produced:** `devices.public_key text`, `devices.lan_endpoint text`, `devices.lan_endpoint_updated_at timestamptz`; update grant extended to those columns.

- [ ] **Step 1: Write the migration** — exact content:

```sql
-- Phase 2 FUNGWIRE: publish each device's full ed25519 public key (for the Noise
-- KK handshake) and its current LAN endpoint (for discovery). Public keys are not
-- secret; sha256(raw key) must equal the existing public_key_fingerprint.
alter table public.devices
  add column if not exists public_key text,
  add column if not exists lan_endpoint text,
  add column if not exists lan_endpoint_updated_at timestamptz;

comment on column public.devices.public_key is
  'Base64 ed25519 verifying key (44 chars). sha256(raw 32 bytes)=public_key_fingerprint. Public, not secret.';
comment on column public.devices.lan_endpoint is
  'Last-known LAN ip:port of this device''s FUNGWIRE server. Advisory; identity is proven by the Noise handshake.';

-- Phase 1 granted update only on (device_label, last_seen_at). Extend so a device
-- can maintain its own public_key + endpoint. RLS still scopes every row to auth.uid().
grant update (device_label, last_seen_at, public_key, lan_endpoint, lan_endpoint_updated_at)
  on public.devices to authenticated;
```

- [ ] **Step 2: Sanity checks** — all three `add column` are `if not exists`; the grant lists exactly the five columns; no RLS/policy change (existing owner-scoped policies already gate rows); no data migration needed (columns nullable).
- [ ] **Step 3: Do NOT apply.** Applying to `nqnrvqnijzovkrhxslfp` happens at the controller gate.
- [ ] **Step 4: Commit**

```bash
git add supabase/migrations/20260810000000_device_pubkey_endpoint.sql
git commit -m "feat(fungwire): add devices public_key and lan_endpoint columns"
```

---

### Task 2: device_identity.rs — public key export + X25519 conversion

**Files:** Modify `src-tauri/src/device_identity.rs`; Modify `src-tauri/src/lib.rs` (register a command)

**Interfaces:**
- Consumes: existing `KEY_FILE`, `SigningKey` load path in `device_identity.rs`.
- Produces: `public_key_b64_in_dir(dir: &Path) -> AppResult<String>`; `x25519_static_secret_in_dir(dir: &Path) -> AppResult<[u8; 32]>`; `x25519_public_from_ed25519_b64(ed_pub_b64: &str) -> AppResult<[u8; 32]>`; Tauri command `device_public_key(app) -> AppResult<String>` (base64 ed25519 verifying key).

- [ ] **Step 1: Write failing tests** in `device_identity.rs` `#[cfg(test)]` (add to the existing module):

```rust
    #[test]
    fn public_key_b64_hashes_to_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let id = ensure_identity_in_dir(dir.path()).unwrap();
        let pub_b64 = public_key_b64_in_dir(dir.path()).unwrap();
        let raw = base64::engine::general_purpose::STANDARD.decode(&pub_b64).unwrap();
        let digest: String = sha2::Sha256::digest(&raw).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(digest, id.fingerprint);
        assert_eq!(raw.len(), 32);
    }

    #[test]
    fn x25519_conversion_enables_ecdh_agreement() {
        // Two identities; each converts its own secret and the other's public.
        // The X25519 ECDH must agree in both directions (proves the birational map is correct).
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        ensure_identity_in_dir(a.path()).unwrap();
        ensure_identity_in_dir(b.path()).unwrap();
        let a_sec = x25519_static_secret_in_dir(a.path()).unwrap();
        let b_sec = x25519_static_secret_in_dir(b.path()).unwrap();
        let a_pub = x25519_public_from_ed25519_b64(&public_key_b64_in_dir(a.path()).unwrap()).unwrap();
        let b_pub = x25519_public_from_ed25519_b64(&public_key_b64_in_dir(b.path()).unwrap()).unwrap();
        let ab = x25519_dalek::x25519(a_sec, b_pub);
        let ba = x25519_dalek::x25519(b_sec, a_pub);
        assert_eq!(ab, ba);
    }
```

- [ ] **Step 2: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml device_identity` — expect FAIL (functions + `x25519_dalek` missing).

- [ ] **Step 3: Add deps** to `src-tauri/Cargo.toml [dependencies]` (also used in Task 4):

```toml
curve25519-dalek = "4"
x25519-dalek = { version = "2", features = ["static_secrets"] }
```

- [ ] **Step 4: Implement** the three functions in `device_identity.rs`. Add imports and a private loader that returns the raw 32-byte seed, then:

```rust
use ed25519_dalek::VerifyingKey;

/// Load the raw 32-byte ed25519 seed (same file/format as ensure_identity_in_dir).
fn load_seed(dir: &Path) -> AppResult<[u8; 32]> {
    let path = dir.join(KEY_FILE);
    let encoded = fs::read_to_string(&path).map_err(|e| io_error("identity read", e))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| io_error("identity decode", e))?;
    bytes.try_into().map_err(|_| io_error("identity key length", "expected 32 bytes"))
}

pub fn public_key_b64_in_dir(dir: &Path) -> AppResult<String> {
    let seed = load_seed(dir)?;
    let key = SigningKey::from_bytes(&seed);
    Ok(base64::engine::general_purpose::STANDARD.encode(key.verifying_key().as_bytes()))
}

/// X25519 static secret derived from the ed25519 seed via the standard map:
/// SHA-512(seed)[..32], clamped. (x25519_dalek clamps on use, but we clamp here
/// so the raw bytes handed to `snow` are already a valid X25519 scalar.)
pub fn x25519_static_secret_in_dir(dir: &Path) -> AppResult<[u8; 32]> {
    let seed = load_seed(dir)?;
    let hash = sha2::Sha512::digest(seed);
    let mut s = [0u8; 32];
    s.copy_from_slice(&hash[..32]);
    s[0] &= 248;
    s[31] &= 127;
    s[31] |= 64;
    Ok(s)
}

/// Convert a peer's published ed25519 public key to its X25519 (Montgomery u) form.
pub fn x25519_public_from_ed25519_b64(ed_pub_b64: &str) -> AppResult<[u8; 32]> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(ed_pub_b64.trim())
        .map_err(|e| io_error("peer pubkey decode", e))?;
    let arr: [u8; 32] = raw.try_into().map_err(|_| io_error("peer pubkey length", "expected 32 bytes"))?;
    let vk = VerifyingKey::from_bytes(&arr).map_err(|e| io_error("peer pubkey parse", e))?;
    Ok(vk.to_montgomery().to_bytes())
}
```

Add `use sha2::Sha512;` (or reference `sha2::Sha512` fully-qualified). If `VerifyingKey::to_montgomery()` is not available in the pinned `ed25519-dalek` version, decompress via `curve25519_dalek::edwards::CompressedEdwardsY(arr).decompress()` then `.to_montgomery().to_bytes()` — the interop test in Step 1 is the correctness gate; make it pass by whichever path compiles.

- [ ] **Step 5: Add the command** in `device_identity.rs`:

```rust
#[tauri::command]
pub fn device_public_key(app: tauri::AppHandle) -> AppResult<String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| io_error("app data dir", e))?;
    public_key_b64_in_dir(&dir)
}
```

Register `device_identity::device_public_key` in `lib.rs`'s `generate_handler![]`.

- [ ] **Step 6: Run** the module tests — expect PASS. Then full `cargo test -j 1` + `npx tsc --noEmit`.
- [ ] **Step 7: Commit** — `feat(fungwire): device public key export and ed25519→x25519 conversion`

---

### Task 3: fungwire.rs — frame codec + control messages + manifest hash

**Files:** Create `src-tauri/src/fungwire.rs`; Modify `src-tauri/src/lib.rs` (`mod fungwire;`)

**Interfaces produced:**
- `write_frame(w: &mut impl Write, payload: &[u8]) -> io::Result<()>` (u32-BE len + bytes)
- `read_frame(r: &mut impl Read, max: usize) -> io::Result<Vec<u8>>` (errors if len > max)
- `const CTRL_MAX: usize = 64 * 1024; const CHUNK_MAX: usize = 4 * 1024 * 1024;`
- `enum Control` (serde-tagged) with variants below; `Control::encode()/decode()`
- `manifest_hash(segment_checksums: &[String]) -> String` (sha256 hex of the ordered checksums joined by `\n`)

- [ ] **Step 1: Write failing tests** in `fungwire.rs` `#[cfg(test)]`:

```rust
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
            job_id: "j1".into(), operation: "transcript.transcribe".into(),
            manifest_hash: "abc".into(), segment_count: 3, total_bytes: 900, profile: "cpu".into(),
            resume_from_seq: 0,
        };
        let bytes = c.encode();
        match Control::decode(&bytes).unwrap() {
            Control::JobStart { job_id, segment_count, .. } => { assert_eq!(job_id, "j1"); assert_eq!(segment_count, 3); }
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
```

- [ ] **Step 2: Run** `cargo test -j 1 --manifest-path src-tauri/Cargo.toml fungwire` — expect FAIL.

- [ ] **Step 3: Implement `fungwire.rs`:**

```rust
use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CTRL_MAX: usize = 64 * 1024;
pub const CHUNK_MAX: usize = 4 * 1024 * 1024;

pub fn write_frame(w: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let len = payload.len() as u32;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

pub fn read_frame(r: &mut impl Read, max: usize) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("frame {len} exceeds cap {max}")));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Control {
    Hello { device_id: String },
    JobStart { job_id: String, operation: String, manifest_hash: String, segment_count: u32, total_bytes: u64, profile: String, resume_from_seq: u32 },
    Chunk { job_id: String, seq: u32, len: u32 },
    ChunkAck { job_id: String, seq: u32 },
    Progress { job_id: String, percent: u8, stage: String },
    Result { job_id: String, duration_ms: i64, segments: Vec<Segment> },
    Error { job_id: String, code: String, message: String },
    Cancel { job_id: String },
    Heartbeat,
    HeartbeatAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment { pub start_ms: i64, pub end_ms: i64, pub text: String, pub confidence: Option<f64> }

impl Control {
    pub fn encode(&self) -> Vec<u8> { serde_json::to_vec(self).expect("control serialize") }
    pub fn decode(bytes: &[u8]) -> serde_json::Result<Control> { serde_json::from_slice(bytes) }
}

pub fn manifest_hash(segment_checksums: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(segment_checksums.join("\n").as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
```

- [ ] **Step 4: Run** the tests — expect PASS. Full `cargo test -j 1`.
- [ ] **Step 5: Commit** — `feat(fungwire): frame codec, control protocol, manifest hashing`

---

### Task 4: fungwire.rs — Noise KK handshake helpers

**Files:** Modify `src-tauri/src/fungwire.rs`; Modify `src-tauri/Cargo.toml` (`snow`)

**Interfaces produced:**
- `NOISE_PARAMS: &str = "Noise_KK_25519_ChaChaPoly_BLAKE2s"`
- `noise_initiator(local_x25519_secret: &[u8;32], remote_x25519_public: &[u8;32]) -> Result<snow::HandshakeState, String>`
- `noise_responder(local_x25519_secret: &[u8;32], remote_x25519_public: &[u8;32]) -> Result<snow::HandshakeState, String>`
- A helper `NoiseChannel` wrapping a `TcpStream` + `snow::TransportState` with `send(&mut self, &Control)` and `send_binary`/`recv(&mut self) -> io::Result<Vec<u8>>` used by Tasks 6–8. (Transport-mode framing = each Noise message inside one length-prefixed frame.)

- [ ] **Step 1: Add dep** to `src-tauri/Cargo.toml`:

```toml
snow = "0.9"
```

- [ ] **Step 2: Write a failing handshake round-trip test** in `fungwire.rs` tests:

```rust
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
```

- [ ] **Step 3: Run** — expect FAIL (helpers missing).

- [ ] **Step 4: Implement** in `fungwire.rs`:

```rust
pub const NOISE_PARAMS: &str = "Noise_KK_25519_ChaChaPoly_BLAKE2s";

pub fn noise_initiator(local: &[u8; 32], remote: &[u8; 32]) -> Result<snow::HandshakeState, String> {
    snow::Builder::new(NOISE_PARAMS.parse().map_err(|e| format!("noise params: {e}"))?)
        .local_private_key(local).remote_public_key(remote)
        .build_initiator().map_err(|e| format!("noise initiator: {e}"))
}

pub fn noise_responder(local: &[u8; 32], remote: &[u8; 32]) -> Result<snow::HandshakeState, String> {
    snow::Builder::new(NOISE_PARAMS.parse().map_err(|e| format!("noise params: {e}"))?)
        .local_private_key(local).remote_public_key(remote)
        .build_responder().map_err(|e| format!("noise responder: {e}"))
}
```

Then add the `NoiseChannel` transport wrapper (used by Tasks 6–8):

```rust
pub struct NoiseChannel<S: Read + Write> {
    stream: S,
    transport: snow::TransportState,
    buf: Vec<u8>,
}

impl<S: Read + Write> NoiseChannel<S> {
    pub fn new(stream: S, transport: snow::TransportState) -> Self {
        Self { stream, transport, buf: vec![0u8; CHUNK_MAX + 256] }
    }
    /// Encrypt `plain`, write as one length-prefixed frame.
    pub fn send_bytes(&mut self, plain: &[u8]) -> io::Result<()> {
        let n = self.transport.write_message(plain, &mut self.buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("noise encrypt: {e}")))?;
        write_frame(&mut self.stream, &self.buf[..n])
    }
    pub fn send(&mut self, c: &Control) -> io::Result<()> { self.send_bytes(&c.encode()) }
    /// Read one frame, decrypt, return plaintext.
    pub fn recv_bytes(&mut self, max: usize) -> io::Result<Vec<u8>> {
        let frame = read_frame(&mut self.stream, max + 256)?;
        let n = self.transport.read_message(&frame, &mut self.buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("noise decrypt: {e}")))?;
        Ok(self.buf[..n].to_vec())
    }
    pub fn recv_control(&mut self) -> io::Result<Control> {
        let bytes = self.recv_bytes(CTRL_MAX)?;
        Control::decode(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("control decode: {e}")))
    }
    pub fn get_mut(&mut self) -> &mut S { &mut self.stream }
}
```

- [ ] **Step 5: Run** — expect PASS. Full `cargo test -j 1`.
- [ ] **Step 6: Commit** — `feat(fungwire): Noise KK handshake helpers and encrypted channel wrapper`

---

### Task 5: Registration publishes public_key + peer key columns

**Files:** Modify `src-tauri/src/lib.rs` (desktop `paired_devices.db` schema + `paired_device_upsert`), `src-tauri/src/mobile.rs` (mobile peer key storage), `src/components/AccountLoginPanel.tsx`, `src/mobile/MobileApp.tsx`, `src/mobile/bridge.ts`

**Interfaces:**
- Consumes: `device_public_key` command (Task 2), Phase 1 registration effects.
- Produces: `devices.public_key` populated on registration; desktop `paired_devices.db` gains `public_key TEXT`; mobile stores the desktop peer's public key.

- [ ] **Step 1: Desktop schema** — in `lib.rs`, add `public_key TEXT` to the `CREATE TABLE IF NOT EXISTS paired_devices` (in `paired_devices_connection`) and an idempotent `ALTER TABLE paired_devices ADD COLUMN public_key TEXT` guarded by a `PRAGMA table_info` check (SQLite has no `ADD COLUMN IF NOT EXISTS`). Extend `PairedDeviceInput`/`PairedDeviceRow` with `public_key: Option<String>` and the upsert/list SQL. Failing test first (extend the existing `paired_device_roundtrip` test to set + read back `public_key`).

- [ ] **Step 2: Run** the focused test → FAIL → implement → PASS (`cargo test -j 1 --manifest-path src-tauri/Cargo.toml paired_device`).

- [ ] **Step 3: bridge.ts** — add:

```typescript
export async function devicePublicKey(): Promise<string | null> {
  if (!isTauri()) return null;
  return invoke<string>("device_public_key");
}
```

- [ ] **Step 4: Desktop registration** — in `AccountLoginPanel.tsx`'s device-registration effect, after `device_identity_ensure`, also `const publicKey = await invoke<string>("device_public_key")`, include `public_key: publicKey` in the `.insert({...})`, and on the "row exists" branch add `.update({ last_seen_at: ..., public_key: publicKey })` (grant permits it).

- [ ] **Step 5: Mobile registration** — same addition in `MobileApp.tsx`'s registration effect (`platform: "android"`), plus when pairing completes, fetch and store the desktop peer's `public_key` (extend the pairing-complete path to select `public_key` alongside the fields it already reads, and pass it to `mobile_pairing_complete` / store in the mobile Genesis `paired_devices` row — add a `public_key` field to that write, mirroring Phase 1's shape).

- [ ] **Step 6: Verify** `npx tsc --noEmit` 0, `npm run build` green, full `cargo test -j 1` green.
- [ ] **Step 7: Commit** — `feat(fungwire): publish device public_key at registration; cache peer keys locally`

---

### Task 6: Desktop FUNGWIRE server — listener + handshake + toggle

**Files:** Create `src-tauri/src/fungwire_server.rs`; Modify `src-tauri/src/lib.rs` (module, `AppState` field, commands)

**Interfaces:**
- Consumes: `fungwire::{noise_responder, NoiseChannel, Control, read_frame}` (Tasks 3–4), `device_identity::x25519_static_secret_in_dir` + peer lookup, `paired_devices.db` (Task 5).
- Produces: commands `fungwire_server_set_enabled(enabled: bool) -> FungwireStatus`, `fungwire_status() -> FungwireStatus { enabled, bind, active_jobs, connected_peers }`. Per-connection worker is Task 7.

- [ ] **Step 1: AppState + control struct.** Add `fungwire: Mutex<Option<FungwireServerControl>>` to `AppState` (struct: `bind: String`, `stop: Arc<AtomicBool>`, `active_jobs: Arc<AtomicUsize>`). Mirror the `mobile_mcp_set_enabled` lifecycle exactly (bind `0.0.0.0:0`, `set_nonblocking(true)`, accept loop polling the `AtomicBool` every 40 ms, `thread::spawn` the loop).

- [ ] **Step 2: The accept loop spawns per-connection** (the fix vs the inline pattern):

```rust
// inside the accept loop thread, on Ok((stream, _)):
let storage = storage.clone();       // Arc handles to whatever the worker needs
let app_data = app_data.clone();
let jobs = active_jobs.clone();
std::thread::spawn(move || {
    if let Err(e) = handle_connection(stream, &storage, &app_data, &jobs) {
        eprintln!("fungwire connection ended: {e}");
    }
});
```

- [ ] **Step 3: Handshake in `handle_connection`** (Task 7 adds the job loop after it):

```rust
pub(crate) fn handle_connection(mut stream: std::net::TcpStream, storage: &Storage, app_data: &std::path::Path, jobs: &std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Result<(), String> {
    use crate::fungwire::{noise_responder, NoiseChannel, Control, read_frame, CTRL_MAX};
    stream.set_read_timeout(Some(std::time::Duration::from_secs(60))).ok();
    // 1) Read the initiator's first handshake message (contains Hello payload with device_id).
    let msg1 = read_frame(&mut stream, CTRL_MAX).map_err(|e| e.to_string())?;
    // Peek the claimed device_id: Noise KK carries it as the handshake payload.
    // Build responder AFTER we know which peer static to expect.
    // (Two-pass: parse Hello from an unauthenticated pre-frame BEFORE Noise, OR
    //  send device_id in the clear as frame 0, then start Noise. Use frame-0-Hello:)
    let hello: Control = Control::decode(&msg1).map_err(|e| e.to_string())?;
    let device_id = match hello { Control::Hello { device_id } => device_id, _ => return Err("expected Hello".into()) };

    // 2) Look up the peer in paired_devices.db: must exist, not revoked, have a public_key.
    let peer = crate::lookup_paired_peer(&device_id).map_err(|e| e.to_string())?
        .ok_or("unknown or revoked peer")?;
    // Binding check: sha256(peer public key) == stored fingerprint.
    let raw = base64::engine::general_purpose::STANDARD.decode(peer.public_key.as_deref().ok_or("peer has no public key")?.trim()).map_err(|e| e.to_string())?;
    let digest: String = sha2::Sha256::digest(&raw).iter().map(|b| format!("{b:02x}")).collect();
    if digest != peer.fingerprint { return Err("peer key/fingerprint mismatch".into()); }

    // 3) Noise KK responder using our secret + the peer's X25519 (from its ed25519 pubkey).
    let local = crate::device_identity::x25519_static_secret_in_dir(app_data).map_err(|e| e.to_string())?;
    let remote = crate::device_identity::x25519_public_from_ed25519_b64(peer.public_key.as_deref().unwrap()).map_err(|e| e.to_string())?;
    let mut hs = noise_responder(&local, &remote)?;
    let mut buf = [0u8; 4096];
    // KK message 1 from initiator:
    let m1 = read_frame(&mut stream, CTRL_MAX).map_err(|e| e.to_string())?;
    hs.read_message(&m1, &mut buf).map_err(|e| e.to_string())?;
    // KK message 2 to initiator:
    let n = hs.write_message(&[], &mut buf).map_err(|e| e.to_string())?;
    crate::fungwire::write_frame(&mut stream, &buf[..n]).map_err(|e| e.to_string())?;
    let transport = hs.into_transport_mode().map_err(|e| e.to_string())?;
    let mut channel = NoiseChannel::new(stream, transport);

    // 4) Hand off to the job loop (Task 7).
    crate::fungwire_server::run_job_loop(&mut channel, storage, app_data, &device_id, jobs)
}
```

Note: the `Hello` (frame 0, cleartext device_id) precedes Noise so the responder knows which static key to expect — this is safe because identity is still proven by the subsequent KK handshake (a wrong claimant cannot complete it). Add a helper `lookup_paired_peer(device_id) -> AppResult<Option<PairedDeviceRow>>` in `lib.rs` querying `paired_devices.db` where `id = ? and revoked_at is null`.

- [ ] **Step 4: Commands** — `fungwire_server_set_enabled` / `fungwire_status`, registered in `generate_handler![]`, returning `FungwireStatus`.

- [ ] **Step 5: Test** — a loopback handshake test (`fungwire_server` tests): start the responder side against a `TcpStream` pair, run a minimal initiator (reuse Task 4 helpers) through `Hello` + KK, assert `into_transport_mode` succeeds and an unpaired device_id is rejected. `cargo test -j 1`.
- [ ] **Step 6: Commit** — `feat(fungwire): desktop server with paired-only Noise handshake and toggle`

---

### Task 7: Desktop worker — receive job, transcribe, stream results

**Files:** Modify `src-tauri/src/fungwire_server.rs`

**Interfaces:**
- Consumes: `NoiseChannel`, `Control`, existing `run_python_worker`/`WhisperRuntime`/`WhisperOutput` (lib.rs), `active_jobs` counter.
- Produces: `run_job_loop(channel, storage, app_data, device_id, jobs) -> Result<(), String>`.

- [ ] **Step 1: Write a loopback integration test** `fungwire_job_stub` using a stub script. Create `tests/fixtures/fake_transcribe.py` that prints `PROGRESS 50` to stderr then a fixed JSON (`{"durationMs":1000,"segments":[{"startMs":0,"endMs":1000,"text":"hi","confidence":0.9}]}`) to stdout. The test wires an in-process client (Task 4 initiator) + `run_job_loop` responder over a `TcpStream` loopback pair, sends `JobStart` + one `Chunk` (a tiny valid `.m4a` fixture or any bytes if the stub ignores content) whose checksum matches the manifest, and asserts a `Result` frame with one segment comes back. Point the worker at the fake script via a test-only `WhisperRuntime` override (add a constructor that takes explicit paths, or an env var the worker reads in tests).

- [ ] **Step 2: Run** → FAIL (`run_job_loop` missing).

- [ ] **Step 3: Implement `run_job_loop`:**

```rust
pub(crate) fn run_job_loop(
    channel: &mut crate::fungwire::NoiseChannel<std::net::TcpStream>,
    storage: &crate::Storage, app_data: &std::path::Path, device_id: &str,
    jobs: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> Result<(), String> {
    use crate::fungwire::{Control, CHUNK_MAX};
    loop {
        let control = match channel.recv_control() { Ok(c) => c, Err(_) => return Ok(()) }; // peer closed
        match control {
            Control::Heartbeat => { channel.send(&Control::HeartbeatAck).map_err(|e| e.to_string())?; }
            Control::JobStart { job_id, operation, manifest_hash, segment_count, profile, .. } => {
                if operation != "transcript.transcribe" {
                    channel.send(&Control::Error { job_id, code: "unsupported_operation".into(), message: operation }).ok();
                    continue;
                }
                jobs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let result = receive_and_transcribe(channel, storage, app_data, &job_id, &manifest_hash, segment_count, &profile);
                jobs.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                match result {
                    Ok((duration_ms, segments)) => channel.send(&Control::Result { job_id, duration_ms, segments }).map_err(|e| e.to_string())?,
                    Err((code, message)) => channel.send(&Control::Error { job_id, code, message }).map_err(|e| e.to_string())?,
                }
            }
            Control::Cancel { .. } => return Ok(()),
            _ => { /* ignore unexpected control on the server */ }
        }
    }
}
```

Then `receive_and_transcribe`: create a temp dir under `app_data/fungwire/<job_id>/`; loop `recv_control` expecting `Chunk { seq, len }` immediately followed by `recv_bytes(CHUNK_MAX)`; verify each segment's `sha256` is a member of the manifest set (recompute `manifest_hash` over the ordered received checksums at the end and compare to `JobStart.manifest_hash` → mismatch = `("manifest_mismatch", ...)`); write each to `segment-<seq>.m4a`; `send(ChunkAck)`; send `Progress{stage:"receiving"}`. After `segment_count` chunks, concatenate the ordered `.m4a` files into one `input.m4a` (binary append is NOT valid for m4a — instead call the existing pipeline once per segment and merge, OR — decision from spec §14 — reassemble via the same mechanism desktop already uses for multi-segment recordings; if none, invoke `transcribe.py` per segment and offset the timestamps by cumulative segment duration). Then `run_python_worker(runtime, transcribe_script, &[path, "--profile", profile], None, |pct| { let _ = tx.send(pct); })` forwarding `Progress{stage:"transcribing"}`; parse `WhisperOutput`; map to `Vec<Segment>`; return.

**Segment concatenation note (spec §14, resolve here):** m4a files cannot be byte-concatenated. v1 approach: invoke `transcribe.py` once per received segment, adding the cumulative start offset (each segment is 5 s) to the returned `startMs`/`endMs`, and concatenate the segment lists. This avoids a muxing dependency and reuses the pipeline verbatim. Progress = `(segments_done / segment_count) * 100`.

- [ ] **Step 4: Run** the integration test → PASS. Add a cancel test (send `Cancel` mid-transfer → loop returns, temp cleaned). Full `cargo test -j 1`.
- [ ] **Step 5: Commit** — `feat(fungwire): desktop job worker runs transcription and streams results`

---

### Task 8: Mobile FUNGWIRE client — connect, stream, apply results

**Files:** Create `src-tauri/src/fungwire_client.rs`; Modify `src-tauri/src/mobile.rs` (commands), `src-tauri/src/lib.rs` (module)

**Interfaces:**
- Consumes: `fungwire::{noise_initiator, NoiseChannel, Control}`, `device_identity` x25519 helpers, mobile Genesis `audio_chunks`/`paired_devices`/`delegated_jobs`/`transcript_segments`.
- Produces: commands `fungwire_delegate_transcription(project_id, recording_id, desktop_device_id) -> { job_id }`, `fungwire_job_poll(job_id) -> { state, progress, error }`, `fungwire_desktop_reachable(desktop_device_id) -> bool`.

- [ ] **Step 1: Reachability first (smallest testable unit).** `fungwire_desktop_reachable`: resolve the peer endpoint (Task 9's resolver; for now accept an endpoint arg or read the mobile `paired_devices.endpoint`), `TcpStream::connect_timeout` (2 s) + `Hello` + KK handshake; return `true` on `into_transport_mode` success, `false` otherwise. Unit-test the "closed port → false" path.

- [ ] **Step 2: Delegation.** `fungwire_delegate_transcription`:
  1. Insert a `delegated_jobs` row `state:"queued"` (reuse the existing insert idiom in `mobile_diarization_start`, `operation:"transcript.transcribe"`, `executor_device_id: desktop_device_id`, `input_manifest_hash: <computed>`), return its `job_id`.
  2. Spawn a `std::thread` that runs the transfer (mobile Rust is sync; the command returns immediately, the thread drives the job and writes progress into the `delegated_jobs` row so `fungwire_job_poll` can read it).
  3. In the thread: gather the recording's `audio_chunks` rows (ordered by `sequence_no`, each has `file_path` + `checksum`); compute `manifest_hash(ordered checksums)`; connect + `Hello` + KK initiator; send `JobStart`; stream each segment as `Chunk`+binary, wait for `ChunkAck` (resume: track `last_acked_seq`); on `Progress` update the row `state:"running"`, `progress`; on `Result` write `transcript_segments` into Genesis (existing segment-insert path) + mark row `completed`; on `Error`/timeout mark `failed` with the message; on socket loss reconnect with backoff and resume from `last_acked_seq + 1`; after N failed reconnects mark the row `failed` and flip the peer `trust_state` to `unreachable`.

- [ ] **Step 3: Poll.** `fungwire_job_poll` reads the `delegated_jobs` row and returns `{ state, progress, error }` (error stored in `checkpoint_json` or a dedicated column — reuse `checkpoint_json` as `{"error": "..."}`).

- [ ] **Step 4: Tests.** Loopback integration mirroring Task 7 but driving the real client thread against the real server `run_job_loop` over `127.0.0.1`: assert the `delegated_jobs` row reaches `completed` and `transcript_segments` are written. Reconnect/resume test: kill the socket after seq K server-side, assert the client resumes and completes. `cargo test -j 1`.
- [ ] **Step 5: Commit** — `feat(fungwire): mobile client streams jobs and applies transcripts`

---

### Task 9: Endpoint publisher + discovery resolver

**Files:** Modify `src-tauri/src/lib.rs` (desktop publisher), `src-tauri/src/mobile.rs` (resolver)

**Interfaces:**
- Consumes: Supabase `devices.lan_endpoint` (Task 1), the FUNGWIRE server bind port (Task 6).
- Produces: desktop publishes `lan_endpoint` on start/tick; mobile `resolve_desktop_endpoint(device_id) -> Option<String>` used by Tasks 8's client.

- [ ] **Step 1: LAN IP helper (dependency-free)** in `lib.rs`, with a test that it returns a non-loopback IPv4 or `None`:

```rust
pub(crate) fn primary_lan_ipv4() -> Option<String> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;      // no packet sent; just sets the local addr
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_loopback() => Some(v4.to_string()),
        _ => None,
    }
}
```

- [ ] **Step 2: Publisher.** When `fungwire_server_set_enabled(true)` binds the port, compute `primary_lan_ipv4()` + the bound port and expose a command `fungwire_local_endpoint() -> Option<String>` (`"<ip>:<port>"`). The frontend (Task 10, desktop) writes it to Supabase `devices.lan_endpoint` + `lan_endpoint_updated_at = now()` via supabase-js on enable and on a 60 s interval while enabled (keeps cloud/anon-key writes in TypeScript, consistent with the Phase 1 "auth/DB in TS" split; Rust only reports the string).

- [ ] **Step 3: Resolver (mobile).** `resolve_desktop_endpoint`: the mobile client reads the desktop peer's `lan_endpoint` from Supabase — but mobile Rust has no supabase-js. So the resolver is TypeScript: `bridge.ts` `desktopEndpoint(deviceId)` queries `devices.lan_endpoint`/`lan_endpoint_updated_at`, and `delegateTranscription` passes the resolved endpoint (or the manual fallback from the paired row) INTO the Rust command as an argument. Adjust Task 8's command signature to `fungwire_delegate_transcription(project_id, recording_id, desktop_device_id, endpoint)` and `fungwire_desktop_reachable(desktop_device_id, endpoint)` — the endpoint is resolved in TS and handed to Rust. Document this as the division of labor (cloud reads in TS, socket work in Rust).

- [ ] **Step 4: Test** `primary_lan_ipv4` (non-panicking; returns Some/None). Full `cargo test -j 1`.
- [ ] **Step 5: Commit** — `feat(fungwire): desktop LAN endpoint publishing and mobile resolution`

---

### Task 10: Frontend — delegate action, progress UI, desktop toggle

**Files:** Modify `src/mobile/model.ts`, `src/mobile/bridge.ts`, `src/mobile/TimelineScreen.tsx`, `src/mobile/CreativeStudio.tsx`, `src/components/DevicePairingPanel.tsx`, `.github/workflows/ci.yml` (if a new TS test is added)

**Interfaces:** consumes Task 8 commands + Task 2 `device_public_key`; produces the user-facing delegation flow.

- [ ] **Step 1: model.ts** — add:

```typescript
export interface DelegatedJob {
  id: string;
  operation: string;
  state: "queued" | "running" | "paused" | "completed" | "failed" | "cancelled";
  progress: number;
  executorDeviceId: string | null;
  error?: string;
}
```

- [ ] **Step 2: bridge.ts** — add wrappers:

```typescript
export async function desktopEndpoint(deviceId: string): Promise<string | null> {
  const { data } = await supabase.from("devices")
    .select("lan_endpoint, lan_endpoint_updated_at").eq("id", deviceId).maybeSingle();
  return (data?.lan_endpoint as string | undefined) ?? null;
}

export async function desktopReachable(deviceId: string, endpoint: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("fungwire_desktop_reachable", { desktopDeviceId: deviceId, endpoint });
}

export async function delegateTranscription(projectId: string, recordingId: string, desktopDeviceId: string, endpoint: string): Promise<{ jobId: string } | null> {
  if (!isTauri()) return null;
  return invoke("fungwire_delegate_transcription", { projectId, recordingId, desktopDeviceId, endpoint });
}

export async function pollDelegatedJob(jobId: string): Promise<DelegatedJob | null> {
  if (!isTauri()) return null;
  return invoke<DelegatedJob>("fungwire_job_poll", { jobId });
}
```

(`supabase` import already present in bridge/mobile via Phase 1; if bridge.ts lacks it, import from `../lib/supabase`.)

- [ ] **Step 3: TimelineScreen / ProcessingStudio delegate action.** Where diarization/transcription is triggered, when a paired desktop exists: resolve its endpoint (`desktopEndpoint`), check `desktopReachable`; if reachable show **"ถอดเสียงบน FUNG Desktop"**. When `onDeviceAiStatus()` returns a `Deferred` admission, render it as the recommended path with the Thai copy "อุปกรณ์นี้ประมวลผลเองไม่ไหว — ส่งไปที่ FUNG Desktop". On tap: `delegateTranscription(...)` → poll `pollDelegatedJob(jobId)` every 1.5 s, render a progress bar from `state`/`progress`; a cancel button (calls a `fungwire_job_cancel` you add to Task 8 if not present — else omit cancel from v1 UI and note it). Results appear via the normal timeline once segments land in Genesis.

- [ ] **Step 4: Desktop FUNGWIRE toggle** — in `DevicePairingPanel.tsx` add a section: an enable/disable switch calling `fungwire_server_set_enabled`, showing `fungwire_status()` (`enabled`, `bind`, `active_jobs`, `connected_peers`); on enable, read `fungwire_local_endpoint()` and write it to `devices.lan_endpoint` + `lan_endpoint_updated_at` (supabase-js), repeating on a 60 s interval while enabled (clear on disable/unmount). Thai labels ("การเชื่อมต่อ FUNGWIRE", "เปิดให้มือถือส่งงานมาประมวลผล").

- [ ] **Step 5: Verify** `npx tsc --noEmit` 0 · `npm run build` green · `npm run test:auth` 5/5 · `npm run test:mobile` 4/4.
- [ ] **Step 6: Commit** — `feat(fungwire): mobile delegate+progress UI and desktop server toggle`

---

## Controller Gate (after final review, before merge)

1. Apply migration `20260810000000_device_pubkey_endpoint.sql` to `nqnrvqnijzovkrhxslfp` (Boss confirm) — additive columns + grant, no data migration.
2. No dashboard change.
3. Manual acceptance (spec §12): two real devices on one LAN — delegate a real recording, watch progress, transcript lands on mobile; revoke desktop from mobile → next delegate refused; kill desktop mid-job → mobile shows `unreachable`, resumes on restart.

## Self-Review

**Spec coverage:** §4.1 migration → T1; §4.2 key helpers → T2; §5.1 codec → T3; §5.2 Noise → T4; §4.3 registration → T5; §5.4 server → T6; §6 worker/protocol → T7; §6 client → T8; §7 discovery → T9; §8 fallback UX → T10; §9 data model → T1/T5/T8; §11 security → T2/T6 (binding check, paired-only, encryption); §12 testing → per-task tests. REQ-W-01…05 mapped in spec §15 → T6/T7/T8/T9/T10.

**Placeholder scan:** the one genuinely-deferred decision (m4a segment reassembly) is resolved inline in T7 Step 3 (per-segment transcribe + timestamp offset) — not left open. No TBD/TODO.

**Type consistency:** `Control` variants (T3) consumed identically in T6/T7/T8; `Segment {start_ms,end_ms,text,confidence}` matches `WhisperSegment` field names; `NoiseChannel` API (T4) used in T6/T7/T8; command names match between bridge.ts (T10) and Rust registration (T6/T8/T9); `DelegatedJob.state` union matches the `delegated_jobs` CHECK states; the resolved-in-TS endpoint is threaded through `fungwire_delegate_transcription`/`fungwire_desktop_reachable` in both T8 and T9/T10 consistently (signature includes `endpoint`).

**Known cross-task adjustment:** T8's command signatures gain an `endpoint` parameter (resolved by T9/T10 in TS) — reflected in the bridge wrappers (T10) and the command definitions (T8). Implementers of T8 must include `endpoint` from the start.
