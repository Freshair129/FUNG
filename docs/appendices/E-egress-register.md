# Appendix E — Network egress register

**Audited:** 2026-08-19 against `217e0b9`.
**Amended:** 2026-08-20 — §1.6 added with the URL-ingest path
(`feature/media-fetch-url-ingest`), declared at the time it was written rather
than found by the next audit.
**Method:** every network primitive in the tree read at its call site — `reqwest`
clients, raw sockets, subprocess workers, and the webview's own reach. Not a
survey of what the product claims; a list of what the code does.

FUNG's central claim is that a meeting stays on the machine that recorded it.
This register exists so that claim can be checked rather than believed, and so
that a path added later has somewhere it must be declared. Nothing here is
proof that a build behaves this way on a real network — see
[Not proven](#not-proven).

---

## 1. What can leave, and under whose consent

Ordered by how much of the user's material crosses the boundary.

### 1.1 Cloud STT — recorded audio leaves the machine

| | |
|---|---|
| **Payload** | The concatenated meeting audio, as a file upload |
| **Destination** | `api.openai.com`, or an operator-supplied endpoint |
| **Evidence** | `src-tauri/src/cloud_executor.rs:102`, `:142` |
| **Consent gate** | `policy::decide_cloud_tier` — off by default (`TierPolicy::default` sets `stt_cloud_enabled: false`), needs a stored key, capped per day |
| **Reached from** | `fungwire_server::dispatch_cloud_stt` only |

This is the only path on which **source audio** leaves. It is not reachable
from the desktop's own capture flow at all: the desktop transcribes locally and
never calls it. It exists for a paired mobile device delegating a job to the
desktop, and the decision is taken against **the desktop's** tier policy, not
the phone's — `fungwire_server.rs:1140`. A phone cannot enable cloud STT by
asking.

### 1.2 Cloud LLM fallback — transcript text leaves the machine

| | |
|---|---|
| **Payload** | A prompt containing transcript excerpts and entity labels |
| **Destination** | `api.anthropic.com`, `api.openai.com`, or an operator-supplied endpoint |
| **Evidence** | `src-tauri/src/cloud_executor.rs:183`, `:235`, `:283` |
| **Consent gate** | `decide_cloud_tier` (`llm_cloud_enabled`, off by default) **and** local Ollama having failed with a connection error |
| **Reached from** | `graph_build.rs:715` only |

Two conditions, not one: cloud is attempted only after the local call fails
*and* the policy allows it. `cloud_executor::is_connection_error` decides which
local failures count — a model that answered badly is not a reason to go to the
cloud, only one that did not answer at all.

The summariser (`meeting_intel.rs`) calls `graph_build::call_llm` directly and
has **no** cloud fallback. Summarisation fails when Ollama is down rather than
falling back.

Where the text actually went is recorded, not assumed: `call_llm_with_fallback`
returns `RUNTIME_LOCAL` / `RUNTIME_CLOUD` and the caller writes it to
`model_runs.runtime_location`. Only that function knows which transport ran.

### 1.3 Local LLM — transcript text leaves the *process*

| | |
|---|---|
| **Payload** | The same prompts as 1.2, plus the model-list probe |
| **Destination** | `model_providers.config_json.endpoint`, default `http://127.0.0.1:11434` |
| **Evidence** | `src-tauri/src/graph_build.rs:267` (`/api/tags`), `:305` (`/api/chat`) |
| **Consent gate** | **None** — no tier policy, no scheme check, no host check |

The seeded row is labelled `runtime_location: 'local'`, and **nothing enforces
that label**. If the endpoint column named a remote host, `call_llm` would post
the prompt there over plain HTTP and the audit row would still read `local`.

Mitigating, and the reason this is recorded rather than fixed: there is no
command that writes this column. `tts_provider_register` /
`tts_provider_update` (`lib.rs:1178`, `:1226`) are the only writers of
`model_providers`, they hard-code `runtime_location: "local"`, and they only
ever write `kind: "tts"` rows. Changing the summary/intent endpoint requires
editing the database directly. **Open gap:** the field is a claim, not a
constraint — see [§4](#4-open-gaps).

### 1.4 TTS — synthesis text leaves the machine

| | |
|---|---|
| **Payload** | `request.text` — content derived from the meeting |
| **Destination** | Whatever the operator registered |
| **Evidence** | `src-tauri/src/tts_executor.rs:122` |
| **Consent gate** | Registration-time **warning** only; a public endpoint is permitted |

Deliberate: the operator chose the endpoint. The warning is the only signal
they get, which is why the classifier behind it is now an IP parse rather than
a string prefix — see [§3.2](#32-a-hostname-shaped-like-a-private-address-suppressed-the-tts-warning).

### 1.5 Zoom — inbound only

| | |
|---|---|
| **Payload out** | OAuth code / refresh token, and the account's bearer on each request |
| **Payload in** | Recording metadata and the recording files |
| **Destination** | `zoom.us`, `api.zoom.us` (constants at `zoom_sync.rs:18–19`) |
| **Evidence** | `zoom_sync.rs:166`, `:248`, `:436`, `:656`, `:844` |
| **Consent gate** | The user completing the OAuth flow |

**No local material is uploaded on this path.** It reads the user's own Zoom
account and pulls files down. Every host is a compile-time constant except the
per-file `download_url`, which arrives in a response body — now checked before
the bearer is attached ([§3.3](#33-the-zoom-bearer-followed-a-url-from-a-response-body)).

### 1.6 URL ingest — a URL leaves, media comes back

| | |
|---|---|
| **Payload out** | The URL the user typed, and this machine's IP address |
| **Payload in** | The audio track of that URL's media |
| **Destination** | Whatever host is in the URL — chosen per fetch, by the user |
| **Evidence** | `src-tauri/src/media_fetch.rs:fetch`, `scripts/fetch_media.py` |
| **Consent gate** | `policy::media_fetch_consent` — off by default, revocable; **and** the yt-dlp runtime being staged by hand |
| **Reached from** | `lib.rs::fetch_and_transcribe` only |

**No FUNG-held material leaves on this path.** Not audio, not transcript text,
not project names — only the address someone pasted, to the host in it. That
makes it the *least* exposing outbound path in this table, which is why it sits
here rather than beside cloud STT.

It is also the only path whose destination is not known in advance. Every other
row above names a host, a compile-time constant, or a value an operator
configured once. This one is whatever the user typed, so the checks are on the
shape rather than the identity: `media_fetch::require_http_url` allows `http`
and `https` and nothing else, at the command boundary and again in the worker.
A `file://` URL would otherwise make an arbitrary-file read reachable from a
text box, since yt-dlp accepts one.

Two gates, not one, and they are independent on purpose. Staging the runtime
(`scripts/stage_media_fetch_runtime.ps1`) makes the capability *possible*;
`media_fetch_consent` makes it *permitted*. Someone who ran the staging script
once has not thereby authorised every future fetch, and revoking consent is a
plain flag flip that leaves the installation intact. Neither gate is reachable
from a paired mobile device: `fetch_and_transcribe` is a desktop command, and
the consent row is the desktop's.

What arrives enters the ordinary import path — `audio_custody::take_custody_of_import`,
digest, ledger — so a fetched recording is backed up and integrity-checked
exactly like a dragged-in file. The staging directory it lands in first is
removed on both the success and failure paths, so a partial download is never
left where something could mistake it for a recording.

The worker is handed no Hugging Face cache, so `run_python_worker` gives it
`HF_HUB_OFFLINE=1` like every other worker — the one process here that is
allowed to reach the network still has no business reaching the hub.

### 1.7 FUNGWIRE — audio leaves for a paired device on the LAN

| | |
|---|---|
| **Payload** | Audio segments and job state |
| **Destination** | A paired device's LAN address |
| **Evidence** | `fungwire_client.rs:176` (`TcpStream::connect_timeout`) |
| **Consent gate** | Explicit pairing; Noise-encrypted channel |

Off the machine, but not off the network, and only to a device the user paired.
Note the chain: a mobile device may delegate a job here, and the desktop may
then take path 1.1 with it — governed, as above, by the desktop's policy.

### 1.8 Model fetches — no user data, but real network use

| Path | Fetches | When |
|---|---|---|
| `scripts/diarize.py:40` | `pyannote/speaker-diarization-3.1` from HuggingFace | First diarization run, with the operator's own token |
| `scripts/stage_diarization_runtime.ps1` | wheels + optionally the model | Only when run by hand |

Both are opt-in by construction: the dependencies are not in the installer and
the model is licence-gated. `diarization::token_configured` reads only *whether*
a token is set, never its value (`diarization.rs`).

The transcription worker is now pinned offline rather than merely believed to
be — [§3.1](#31-the-transcription-worker-was-offline-by-habit-not-by-constraint).

### 1.9 Not egress, despite appearances

- **`lib.rs:552` — `UdpSocket::connect("8.8.8.8:80")`.** No packet is sent. A
  UDP `connect` only makes the OS pick a local route so `local_addr()` can be
  read back; it is how `primary_lan_ipv4` finds the LAN address without
  traffic. No DNS, no handshake, no bytes.
- **`external_mcp_transport`.** Stdio only. `ConnectorTransport::StreamableHttp`
  exists in the enum (`external_mcp.rs:270`) and **is never matched on
  anywhere**; execution refuses any connector whose transport is not `"stdio"`
  (`external_mcp_commands.rs:607`). A schema variant with no implementation,
  failing closed. The *connector process itself* is of course unconstrained —
  FUNG bounds its time, output size, and capability set, not its sockets.
- **Backup.** `backup.rs`, `backup_archive.rs`, `backup_payload.rs`,
  `filesystem_backup.rs` contain no network primitive of any kind. Backup is
  filesystem-only; there is no upload path.
- **The webview.** No `fetch`, `XMLHttpRequest`, `WebSocket`, or HTTP client
  anywhere in `src/`. The CSP pins it shut regardless:
  `connect-src ipc: http://127.0.0.1:*` (`tauri.conf.json`). The frontend cannot
  reach a remote host even if someone later writes the code to try.

### 1.10 Supabase native session broker — authentication and authorization

| Path | Payload | Destination | Consent gate |
|---|---|---|---|
| `auth_session.rs` | PKCE code/verifier, refresh credential, session metadata, device/pairing requests, and audit RPC bodies | The configured Supabase HTTPS origin, plus `oauth2.googleapis.com` for Google Drive token exchange | User-initiated login/Drive/pairing operation; access and refresh credentials remain native and are read from the OS keyring |

The native broker owns these requests. It does not expose access or refresh
credentials to the webview or persist them in GenesisBlockDB/Supabase. Request
responses are reduced to typed, non-secret lifecycle results before crossing
the Tauri command boundary. This entry is separate from the cloud STT/LLM
paths above because it sends authorization material and metadata, not meeting
audio or transcript content.

---

## 2. Inbound exposure

Listeners are not egress, but they are the same boundary read from the other
side, so they belong in the same register.

| Listener | Binds | Auth | Serves |
|---|---|---|---|
| `fungwire_server` (`:152`) | `0.0.0.0:0` | Noise + pairing | Job protocol |
| Mobile gateway (`mobile.rs:2630`) | `0.0.0.0:0` **or** `127.0.0.1:0` | Per-session token | MCP tool surface |
| `start_local_api` (`lib.rs:2127`) | `127.0.0.1:0` | **None** | `/health` only |
| `auth_loopback_listen` (`lib.rs:2194`) | `127.0.0.1:0` | One-shot | OAuth callback |

Both LAN binds are opt-in and unbound by default. The mobile gateway's LAN
exposure is a separate `expose_lan` argument from its enablement, so the
loopback-only mode is a real choice rather than a comment.

`start_local_api` is unauthenticated. It answers `/health` and nothing else,
but that response includes the absolute database path — a small disclosure to
any process on the machine.

---

## 3. Fixed by this audit

### 3.1 The transcription worker was offline by habit, not by constraint

`run_python_worker` set `HF_HOME` for the diarization worker and left the
transcription worker's environment alone, with a comment asserting it "never
touches the Hugging Face hub". The comment was correct about intent and
enforced by nothing.

The worker resolves its model from `FUNG_WHISPER_MODEL`, and
`bundled_whisper_model` returns `None` if the runtime layout ever fails to
resolve. `scripts/transcribe.py:65` then falls back to the string `"small"` —
which faster-whisper resolves by **downloading from huggingface.co**. The one
pass this product's local-first claim rests on would have become a silent
network fetch, on the machine of someone who chose FUNG so their audio would
not leave it.

Now: a worker handed no HF cache is given `HF_HUB_OFFLINE=1`. The same
condition surfaces as a legible error instead of a download. `None` no longer
means "leave the environment alone"; it means "this worker does not talk to the
hub", which is what the comment always claimed.

### 3.2 A hostname shaped like a private address suppressed the TTS warning

`tts_config::is_private_ip` matched string prefixes:
`host.starts_with("10.")`. So `http://10.example.invalid/tts` was classified as
private, and the single warning telling an operator their meeting text was
about to leave the building never appeared — for exactly the name someone would
choose to make it not appear. `http://10.0.0.1@example.invalid/` had the same
effect through userinfo, and any IPv6 endpoint was cut in half at its first
colon.

Now the host is extracted properly (userinfo stripped, brackets handled) and
parsed as an `IpAddr`. A DNS name is never private, even one that will resolve
to `10.x`: what it resolves to is not knowable at registration time, and an
unnecessary warning is a far cheaper mistake than a missing one.

### 3.3 The Zoom bearer followed a URL from a response body

`download_to_file` attached the account's OAuth bearer to whatever host
`download_url` named, and that value arrives inside an API response.

Exploiting it needs Zoom itself or the TLS chain to be compromised — at which
point the token is already exposed — so this is a narrow finding. But nothing
in the code said the URL had to point at Zoom, and "we trust the response body"
is not a property that should be implicit. `is_zoom_download_url` now requires
`https` and a `zoom.us` host before the token is handed over; anything else is
refused rather than fetched uncredentialed, because downloading the wrong file
quietly is not the safer failure.

---

## 4. Open gaps

- **`model_providers.runtime_location` is a label, not a constraint.** §1.3.
  Not reachable through any command today, which is why it is recorded rather
  than fixed — but the column asserts something no code checks. Closing it
  means either validating the endpoint at the point of use, or dropping the
  claim.
- **The daily cloud cap is advisory under concurrency.** Documented at
  `policy.rs:37` — the read-check-increment across `decide_cloud_tier` and
  `increment_calls_today` is not transactional, so concurrent FUNGWIRE
  dispatches can exceed it by a bounded amount. Acceptable for a rate limit;
  not an invariant.
- **`start_local_api` discloses the database path unauthenticated.** §2.
- **The connector subprocess is unbounded on the network.** §1.9. Bounding it
  would mean sandboxing the child process, which FUNG does not do.

---

## Not proven

- **No build has been run with the network disabled.** Everything above is read
  from source. NFR-101 in [D-traceability](D-traceability.md) already records
  the network-disabled UAT as outstanding, and this audit does not discharge
  it. The check worth running: capture and transcribe a meeting end to end on a
  machine with no route, and confirm it completes rather than merely not
  crashing.
- **No packet capture was taken.** The claim "these are all the paths" is a
  claim about the source tree. A build links transitive dependencies —
  `tauri`, `keyring`, `genesis-block-native` — whose own network behaviour was
  not traced. `tests/egressRegister.test.mjs` pins FUNG's own call sites, not
  its dependencies'.
- **§3.3 was not tested against real Zoom.** `is_zoom_download_url` is unit
  tested; that Zoom only ever returns `*.zoom.us` download URLs is an
  assumption from the API's documented behaviour. If a real import fails with
  the refusal message, the allowlist is wrong and this is where to look.
- **§3.1 changes what a broken install does.** An installation missing its
  bundled Whisper model previously might have downloaded one; it now fails.
  That is the intended direction, and it has not been observed on a real
  broken install.
