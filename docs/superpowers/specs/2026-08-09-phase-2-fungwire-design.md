# Phase 2: FUNGWIRE v1 — LAN Tunnel + Desktop Job Worker — Design Spec

| Field | Value |
|---|---|
| Date | 2026-08-09 |
| Status | draft — pending Boss review |
| Sub-project | Phase 2 (FUNGWIRE) — master plan REQ-W-01…05 |
| Depends on | Phase 1 (device pairing + ed25519 identity), PR #5 — must be merged first |
| Master plan | `2026-08-09-fung-master-implementation-plan.md` Phase 2 |

## 1. Overview & Scope

**Goal:** A mobile device offloads a transcription job to its paired desktop over an encrypted LAN tunnel. Mobile streams the recording's audio segments to the desktop; the desktop runs its existing Whisper pipeline and streams transcript segments + progress back. Only a device that is paired (Phase 1) and holds the matching ed25519 private key can connect.

**In scope:**
- Desktop **FUNGWIRE server**: LAN TCP listener, Noise-encrypted, accepts only paired non-revoked peers; off by default (user toggle).
- Mobile **FUNGWIRE client**: connect / handshake / stream job / heartbeat / reconnect / mark `unreachable`.
- **Transport**: length-prefixed binary frames inside a Noise `KK` channel (mutual static-key auth + encryption). ed25519 identity → X25519 for Noise.
- **Trust anchor**: full device public key published to Supabase `devices.public_key`, bound to the Phase 1 fingerprint by `sha256(key) == public_key_fingerprint`.
- **Discovery**: desktop publishes its LAN `ip:port` to its `devices` row; mobile reads it; manual entry is the fallback.
- **Job execution**: desktop worker consumes a delegated transcription job (`delegated_jobs`' first-ever consumer), verifies the transferred audio's manifest hash, runs the existing `transcribe.py` subprocess, streams `Progress` + `Result`.
- **Resumability**: resumable file transfer (resume upload from last acked chunk); transcription re-runs from the reassembled file. Cancel supported.
- **Fallback UX**: mobile offers "ถอดเสียงบน FUNG Desktop" when a paired desktop is reachable, highlighted when `admit_task` would defer.

**Out of scope (parked):**
- Cloud relay / NAT traversal (devices on different networks) — LAN only. Parked per master plan §11.
- Automatic `admit_task` → local-or-delegate routing — no real on-device-AI dispatch site exists yet (it's a stubbed probe); automatic routing lands with Phase 3.
- Non-transcription operations (`transcript.refine`, `story.export`, `voice.synthesize`, `model.install`) — the protocol is operation-generic but only `transcript.transcribe` is wired end-to-end in v1.
- Mid-inference checkpointing (Whisper re-runs from the whole file).
- iOS.

## 2. Locked Decisions

| Question | Decision | Date |
|---|---|---|
| Peer public-key distribution | Publish full ed25519 public key to Supabase `devices.public_key`; verify `sha256(key)==fingerprint` | 2026-08-09 |
| Transport security | Mutual signed auth + encrypted payload, realized via Noise `KK` (`snow` crate) | 2026-08-09 |
| Fallback trigger | User-offered delegation (not automatic) | 2026-08-09 |
| Discovery | Desktop publishes LAN endpoint to cloud; mobile reads it; manual `ip:port` fallback | 2026-08-09 (controller call, spec-veto pending) |
| Framing | Raw length-prefixed TCP frames (NOT WebSocket) — no browser in the loop, both ends Rust | 2026-08-09 (controller call, deviates from master-plan "WebSocket" wording — spec-veto pending) |
| First operation | `transcript.transcribe` only, end-to-end | 2026-08-09 |
| Runtime model | Fully synchronous std::net + `thread::spawn` per connection (no tokio — none exists) | 2026-08-09 |

## 3. Architecture Overview

```
┌───────────────── Mobile (Tauri, Android) ─────────────────┐
│ React: TimelineScreen / ProcessingStudio                  │
│   "ถอดเสียงบน FUNG Desktop" action + progress bar          │
│ bridge.ts: delegateTranscription / pollDelegatedJob /      │
│            desktopReachable                                │
│ Rust: fungwire_client.rs                                   │
│   • reads .m4a segments from audio_chunks files            │
│   • Noise KK initiator → desktop                           │
│   • streams JobStart+Chunks, applies Progress/Result       │
│   • writes transcript_segments + advances delegated_jobs   │
└───────────┬───────────────────────────────────────────────┘
            │  encrypted TCP frames (LAN)     ▲ Progress/Result
            ▼                                 │
┌───────────┴───────────────── Desktop (Tauri, Windows) ─────┐
│ Rust: fungwire_server.rs (bind 0.0.0.0:<port>, off default) │
│   • Noise KK responder, verify peer paired+non-revoked      │
│   • thread-per-connection                                   │
│ Rust: fungwire_worker (per job)                             │
│   • reassemble audio → verify manifest hash                 │
│   • run_python_worker(transcribe.py)  ← EXISTING pipeline   │
│   • stream Progress + Result frames                         │
│ Rust: endpoint publisher → Supabase devices.lan_endpoint    │
└───────────┬────────────────────────────────────────────────┘
            │ anon-key HTTPS (RLS-scoped)
     ┌──────▼──────── Supabase ─────────────────┐
     │ devices: + public_key, lan_endpoint,     │
     │           lan_endpoint_updated_at         │
     │ device_audit_events (existing)            │
     └───────────────────────────────────────────┘
```

**Trust chain:** Phase 1 established that a `(user, fingerprint)` pair is verified (6-digit code via Supabase). Phase 2 extends that: the full public key sits in the cloud row alongside the fingerprint, and the Noise handshake proves the connecting device holds the matching private key. `sha256(public_key) == public_key_fingerprint` binds the LAN identity to the Phase-1-verified pairing, so the tunnel inherits Phase 1's trust without a new pairing step.

## 4. Trust Anchor & Key Handling

### 4.1 Supabase migration `20260810000000_device_pubkey_endpoint.sql`

```sql
alter table public.devices
  add column if not exists public_key text,
  add column if not exists lan_endpoint text,
  add column if not exists lan_endpoint_updated_at timestamptz;

comment on column public.devices.public_key is
  'Base64 ed25519 verifying key (44 chars). sha256(raw key)=public_key_fingerprint. Public, not secret.';

-- Extend the column-scoped update grant so a device can maintain its own
-- public_key + lan_endpoint. (Phase 1 granted only device_label, last_seen_at.)
grant update (device_label, last_seen_at, public_key, lan_endpoint, lan_endpoint_updated_at)
  on public.devices to authenticated;
```

RLS is unchanged — the existing owner-scoped policies already gate every row to `auth.uid() = user_id`. No new table.

### 4.2 `device_identity.rs` additions

The Phase 1 module stores the 32-byte ed25519 seed as base64 in `device_identity.key`. Phase 2 adds (re-reading that same file — storage format unchanged):

- `public_key_b64_in_dir(dir) -> AppResult<String>` — base64 of the 32-byte verifying key (what registration publishes to `devices.public_key`).
- `x25519_static_secret_in_dir(dir) -> AppResult<[u8;32]>` — the ed25519 seed converted to an X25519 secret (clamped scalar from the SHA-512 expansion of the seed, the standard ed25519→X25519 derivation). Used to seed the Noise responder/initiator static key. The private key never leaves Rust.
- `x25519_public_from_ed25519(ed_pub_b64) -> AppResult<[u8;32]>` — convert a peer's published ed25519 public key (from the cloud) to its X25519 form, to use as the Noise `KK` remote static key.

No signing API is exposed to the frontend; all key material stays inside these functions. (A generic `sign()` is not needed — Noise KK proves key possession via the handshake, so there is no separate challenge to sign.)

### 4.3 Registration change (small addition to Phase 1 code)

`AccountLoginPanel.tsx` (desktop) and `MobileApp.tsx` (mobile) already register the device by inserting/selecting on `devices`. Phase 2 adds: after `device_identity_ensure`, call a new `device_public_key()` command and include `public_key` in the insert, and on the "row exists" path `update({ public_key })` if it's null (back-fill for devices registered under Phase 1). Column grant from §4.1 permits it.

## 5. FUNGWIRE Transport

### 5.1 Framing

All bytes on the socket after the Noise handshake are Noise transport messages. Inside the decrypted channel, the application uses **length-prefixed frames**: `u32 big-endian length` + `payload`. Two payload kinds:
- **Control** — JSON, `{ "type": "...", ... }` (see §6).
- **Binary chunk** — a control `Chunk` frame (JSON) announces `{ job_id, seq, len }`, immediately followed by one binary frame of exactly `len` bytes (the audio segment). Keeps JSON parsing off the large payloads.

Max control frame 64 KiB; max binary frame 4 MiB (a 5s `.m4a` is far smaller). Frames over the cap → connection closed.

### 5.2 Noise KK handshake

Pattern `Noise_KK_25519_ChaChaPoly_BLAKE2s` via the `snow` crate (pure Rust, synchronous, no tokio):
- **KK** = both parties know each other's static public key beforehand. Desktop looks up the connecting `device_id`'s X25519 static (converted from the `public_key` it fetched from Supabase and cached locally at pairing/first-contact); mobile likewise knows the desktop's.
- The handshake authenticates both sides (each proves possession of the private key matching the known static) and establishes the ChaChaPoly transport keys. No separate signature step.
- The initiator (mobile) sends its `device_id` in the first handshake payload (Noise KK allows a handshake payload) so the responder knows which static key to expect; the responder aborts if that `device_id` is not a paired, non-revoked peer.

### 5.3 Peer key cache

Desktop `paired_devices.db` (Phase 1, currently `id, name, platform, fingerprint, paired_at, revoked_at, pairing_session_id`) gains a `public_key TEXT` column. Populated when the desktop pairs (fetch from Supabase in `DevicePairingPanel`) or lazily on first FUNGWIRE contact (fetch the peer's `devices` row by id, verify `sha256(public_key)==fingerprint`, store). Mobile stores the desktop's public key in its Genesis `paired_devices.capabilities_json`-adjacent field or a small new column; the handshake needs it before connecting.

**Binding check (mandatory, both sides, every handshake):** before trusting a peer static key, verify `sha256(peer_ed25519_public) == stored fingerprint` for that paired device. A cloud row whose `public_key` doesn't hash to the paired fingerprint is rejected (defends against a tampered//substituted cloud row).

### 5.4 Server lifecycle

`fungwire_server.rs` mirrors the existing MCP-gateway toggle pattern (`AtomicBool` stop flag + `Mutex<Option<Control>>` in app state), with one fix: **each accepted connection is handled on its own `thread::spawn`**, not inline on the accept loop (the existing servers handle inline — a smell we do not copy, since a compute tunnel needs a long-running transfer + control concurrently). Bind `0.0.0.0:<port>` only when the user enables FUNGWIRE; default off. A `fungwire_status()` command reports `{ enabled, bind, active_jobs, connected_peers }`.

## 6. Job Protocol

Control frame `type` values (all carry `job_id` except `Hello`):

| Type | Direction | Payload |
|---|---|---|
| `JobStart` | client→server | `operation` ("transcript.transcribe"), `manifest_hash` (sha256 of the concatenated ordered segment checksums), `segment_count`, `total_bytes`, `profile` ("gpu"\|"cpu"), `resume_from_seq` (0 for fresh) |
| `Chunk` | client→server | `seq`, `len` — followed by one binary frame of `len` bytes |
| `ChunkAck` | server→client | `seq` (server persisted this segment) |
| `Progress` | server→client | `percent` (0–100), `stage` ("receiving"\|"transcribing") |
| `Result` | server→client | `duration_ms`, `segments: [{ start_ms, end_ms, text, confidence }]` |
| `Error` | either | `code`, `message` |
| `Cancel` | client→server | (abort the job; server kills the subprocess, deletes temp) |
| `Heartbeat` / `HeartbeatAck` | client↔server | (liveness, every 10 s idle) |

**Server worker flow** per job:
1. Receive `JobStart`. Re-check the peer is still paired+non-revoked. Create a temp dir.
2. Receive `Chunk`+binary × N. Each segment's sha256 is verified against the checksum embedded in the manifest ordering; a mismatch → `Error{code:"chunk_checksum"}` and abort. `ChunkAck` each. (Resume: if `resume_from_seq > 0` and a prior partial exists, skip already-received seqs.)
3. After the last chunk, verify the reassembled manifest hash == `JobStart.manifest_hash`. Concatenate/muxing not required — pass segment files to the pipeline the same way desktop already handles multi-segment recordings, OR concatenate into one file if the pipeline needs a single input (decide at plan time from `run_transcription`'s input contract; the existing pipeline takes one file path, so v1 concatenates the ordered `.m4a` segments into one file before invoking).
4. `run_python_worker(transcribe.py, [file, "--profile", profile])`, forwarding its `PROGRESS <pct>` lines as `Progress{stage:"transcribing"}` frames.
5. On success stream `Result`; on non-zero exit stream `Error{code:"transcribe_failed", message: <stderr tail>}`. Delete temp.

**Client flow:** advance the local `delegated_jobs` row `queued → running` on first `Progress`, update `progress`, and on `Result` write `transcript_segments` into Genesis + mark the row `completed`; on `Error`/timeout mark `failed` and surface a Thai message. On socket loss during transfer, reconnect (backoff) and resume from `last_acked_seq + 1`.

## 7. Discovery & Reachability

- **Publish (desktop):** on app start and on a periodic tick (e.g. every 60 s while FUNGWIRE enabled), determine the primary LAN IPv4 via the dependency-free UDP trick (`UdpSocket::bind("0.0.0.0:0")`, `connect("8.8.8.8:80")` — no packet sent — read `local_addr().ip()`), and `update` the device's `lan_endpoint = "<ip>:<port>"` + `lan_endpoint_updated_at = now()`. Never blocks startup; failures are logged, not fatal.
- **Resolve (mobile):** to delegate, read the paired desktop's `lan_endpoint` from its `devices` row (RLS-scoped). If null/stale (older than, say, 10 min) or the connection fails, fall back to the manually entered endpoint (the field already exists on the pairing sheet). Verification of identity is always the Noise handshake — a stale/wrong endpoint just fails to connect, it never weakens trust.
- **Reachability probe:** `desktopReachable(device_id)` opens a TCP connect (short timeout) + Noise handshake to the resolved endpoint; success → reachable. Used to decide whether to show the delegate action and to flip `trust_state` to `unreachable` after repeated failures (reset to `paired` on next success).

## 8. Fallback UX (mobile)

- `ProcessingStudio` / `TimelineScreen`: when a paired desktop is reachable, show **"ถอดเสียงบน FUNG Desktop"**. When `mobile_on_device_ai_status` returns a `Deferred` admission (e.g. `device_tier_core`), present this as the **recommended** path with a short Thai explanation ("อุปกรณ์นี้ประมวลผลเองไม่ไหว — ส่งไปที่ FUNG Desktop").
- Tapping it calls `delegateTranscription(project_id, recording_id, desktop_device_id)`, which enqueues the `delegated_jobs` row (existing `mobile_diarization_start`/processing insert path, `operation: "transcript.transcribe"`) and starts the FUNGWIRE client. A progress bar polls `pollDelegatedJob(job_id)` (new bridge wrapper reading the `delegated_jobs` row) and renders `state`/`progress`. Cancel button sends `Cancel`.
- No automatic routing in v1 (honest to the stubbed `admit_task`).

## 9. Data Model Changes

| Store | Change |
|---|---|
| Supabase `devices` | + `public_key`, `lan_endpoint`, `lan_endpoint_updated_at`; extend update grant (§4.1) |
| Desktop `paired_devices.db` | + `public_key TEXT` (peer key cache) |
| Mobile Genesis `paired_devices` | + peer `public_key` (small column or reuse capabilities-adjacent meta — decide at plan time) |
| Mobile Genesis `delegated_jobs` | no schema change — Phase 2 is its first *consumer*; rows now transition `queued→running→completed/failed/cancelled` |
| Mobile Genesis `transcript_segments`-equivalent | results written via existing segment-insert path |

## 10. New Components

**Rust (new files):**
- `src-tauri/src/fungwire.rs` — shared: frame codec (length-prefix read/write), control-message enum (serde), Noise KK helpers (build initiator/responder from `snow`), manifest hashing.
- `src-tauri/src/fungwire_server.rs` — desktop listener + per-connection worker (transcription execution via existing `run_python_worker`).
- `src-tauri/src/fungwire_client.rs` — mobile connector + job driver.

**Rust (modified):**
- `device_identity.rs` — §4.2 key helpers.
- `lib.rs` — desktop: register `fungwire_server_set_enabled`, `fungwire_status`, endpoint publisher; add `public_key` column to `paired_devices.db`; `device_public_key` command.
- `mobile.rs` — mobile: `fungwire_delegate_transcription`, `fungwire_job_poll`, `fungwire_desktop_reachable` commands; peer public-key storage.
- `Cargo.toml` — `+ snow = "0.9"` (Noise). ed25519→x25519 conversion via `curve25519-dalek` (already transitive through `ed25519-dalek`; add as a direct dep if needed).

**Frontend (modified):**
- `src/mobile/model.ts` — `DelegatedJob { id, operation, state, progress, executorDeviceId, error? }`.
- `src/mobile/bridge.ts` — `delegateTranscription`, `pollDelegatedJob`, `desktopReachable`.
- `src/mobile/MobileApp.tsx` / `CreativeStudio.tsx` / `TimelineScreen.tsx` — the delegate action + progress UI.
- `src/components/DevicePairingPanel.tsx` (desktop) — a FUNGWIRE section: enable/disable toggle + status (bind, active jobs, connected peers).

**Supabase:** one migration (§4.1).

## 11. Security

| Concern | Mechanism |
|---|---|
| Only paired devices connect | Noise KK: handshake completes only if the peer holds the private key for the known static; responder aborts on unpaired/revoked `device_id` |
| Cloud row tampering | `sha256(public_key) == public_key_fingerprint` checked every handshake, both sides |
| Payload confidentiality | Noise ChaChaPoly transport encryption end-to-end |
| Revocation | Desktop re-checks `paired_devices.revoked_at` on every `JobStart`; a revoked peer's jobs are refused; periodic reachability flips stale peers to `unreachable` |
| Audio integrity | Per-segment sha256 (existing) + `manifest_hash` over the ordered set, verified before inference |
| Attack surface off by default | LAN `0.0.0.0` bind only when the user enables FUNGWIRE; loopback-less until then |
| Public key is not a secret | Publishing it to the cloud is safe; only the private key (never transmitted, file in app-data) authenticates |
| DoS via huge frames | Frame size caps (§5.1); connection closed on violation |

**Residual (documented):** the private key remains a base64 file (Phase 1 backlog — keyring/Keystore); LAN traffic metadata (that a transfer is happening) is not hidden; a malicious paired device (user's own, already trusted) is out of threat model.

## 12. Testing Strategy

- **Rust unit:** frame codec round-trip + oversize rejection; Noise KK handshake success between two in-process endpoints; handshake **rejection** when the initiator's static doesn't match the expected (unpaired) key; `sha256(pubkey)==fingerprint` binding-check rejection; manifest-hash mismatch rejection; per-chunk checksum mismatch rejection; ed25519→X25519 conversion determinism.
- **Rust integration (loopback):** client + server in one test over `127.0.0.1` — full job: JobStart → chunks → (stub `transcribe.py` that emits fixed `PROGRESS` lines + JSON) → Progress → Result; assert segments delivered. Cancel mid-job kills the subprocess. Reconnect/resume: drop the socket after seq K, reconnect, assert resume from K+1 and eventual completion.
- **TS:** `DelegatedJob` state-mapping pure logic; `pollDelegatedJob` shape.
- **Manual acceptance:** two real devices on one LAN — delegate a real recording, watch progress, transcript lands on mobile; revoke the desktop from mobile → next delegate refused; kill desktop mid-job → mobile shows `unreachable`, resumes on restart.

## 13. Controller Gate (after final review, before merge)

1. Apply migration `20260810000000_device_pubkey_endpoint.sql` to `nqnrvqnijzovkrhxslfp` (Boss confirm).
2. No dashboard change (no new auth URLs).
3. Manual acceptance run (§12) on two real LAN devices.

## 14. Open Questions for Spec Review

- **Framing (raw TCP vs WebSocket):** spec recommends raw length-prefixed TCP; master plan said "WebSocket." Confirm the deviation.
- **`snow` dependency:** acceptable to add the Noise crate, or prefer hand-rolled AEAD (not recommended)?
- **Segment handling:** concatenate ordered `.m4a` into one file for the existing single-file pipeline (v1 choice) vs teach the pipeline multi-file input. Plan-time decision from `run_transcription`'s contract.
- **Resumability depth:** resumable transfer + re-run (v1) is confirmed adequate; mid-inference checkpoint stays out.

## 15. Requirement Traceability

| Master-plan REQ | Section |
|---|---|
| REQ-W-01 desktop tunnel server, paired-only | §5, §11 |
| REQ-W-02 mobile client (connect/heartbeat/reconnect/unreachable) | §6, §7 |
| REQ-W-03 desktop drains delegated_jobs (transcription), progress back | §6 |
| REQ-W-04 manifest-hash verify, resumable checkpoint, cancel | §5.1, §6 |
| REQ-W-05 fallback wiring on admit_task deferral | §8 |
