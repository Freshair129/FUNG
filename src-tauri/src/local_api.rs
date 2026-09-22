//! Loopback HTTP API the desktop exposes to same-machine clients — today the
//! FUNG web dashboard opened in a browser on this PC. It is what lets the web
//! surface list and play recordings **without any audio leaving the machine**:
//! the browser reads straight from the desktop's own GenesisBlockDB ledger and
//! chunk files over `127.0.0.1`.
//!
//! Hand-rolled HTTP/1.1 over `TcpListener`, like the mobile gateway
//! (`mobile::handle_gateway_stream`), rather than a server crate: the whole
//! surface is a handful of GET routes, one upload, and a CORS preflight.
//!
//! The upload (`POST /recordings/import`) is how a recording made *in the
//! browser* — which has no Whisper and no GenesisBlockDB — gets transcribed:
//! the page sends the bytes to the desktop on the same machine, the desktop
//! takes custody of the file and runs the normal import pipeline, and the
//! page polls `/jobs/{id}` then reads `/recordings/{id}/transcript`. Still
//! nothing leaves the PC.
//!
//! Boundary (see `docs/appendices/E-egress-register.md` §2):
//! - Binds loopback by default. A second, opt-in listener on `0.0.0.0`
//!   (`set_lan`) serves the same routes plus the embedded phone page at `/`
//!   so a browser on the same Wi-Fi can play recordings without the cloud
//!   web or a certificate; it is stoppable and gone when the app exits.
//! - Every route except `/`, `/index.html` and `/health` needs the per-launch
//!   bearer token. The desktop hands it to the user as a *connect URL*
//!   (`http://127.0.0.1:PORT/#TOKEN`, or the LAN address in a QR) to paste
//!   or scan once; the browser then sends it as `Authorization: Bearer` for
//!   JSON and as `?token=` for `<audio src>`, which cannot carry headers.
//! - CORS reflects only allow-listed origins (the production web origin and
//!   local dev servers), so a random page open in the same browser cannot
//!   read the response even if it somehow learned the token.

use crate::{
    audio_custody, genesis_adapter, recording_output::RecordingOutputManager, AppState,
    WhisperRuntime,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

/// Browser origins allowed to read this API cross-origin. Local dev servers
/// (`http://localhost:*`, `http://127.0.0.1:*`) are allowed by rule in
/// [`origin_allowed`]; anything else must be listed here or in the
/// `FUNG_WEB_ORIGINS` environment variable (comma-separated).
const WEB_ORIGINS: &[&str] = &["https://fung-seven.vercel.app"];

/// A request head larger than this is not a browser asking for a recording.
const MAX_REQUEST_HEAD: usize = 16 * 1024;
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Largest upload accepted on `POST /recordings/import`. WebM/Opus at the
/// dashboard's 128 kbit/s is ~58 MB per hour; this leaves room for a long
/// meeting or a WAV without letting a page exhaust memory.
const MAX_UPLOAD_BYTES: usize = 512 * 1024 * 1024;

/// What the upload route needs beyond the ledger: where to keep the file
/// and the worker that transcribes it. `None` on a listener that has no
/// runtime (tests), in which case the route answers `503`.
pub(crate) struct ImportHost {
    pub(crate) genesis: Arc<genesis_block_native::Storage>,
    pub(crate) recording_output: Arc<Mutex<RecordingOutputManager>>,
    pub(crate) runtime: WhisperRuntime,
}

/// Stitching an hour of audio into one WAV takes real time and memory, and
/// each request runs on its own thread. Cap in-flight connections so a
/// misbehaving page cannot pile them up; a rejected request simply retries.
const MAX_IN_FLIGHT: usize = 8;

/// The page a phone's browser gets at `/` when LAN sharing is on: a
/// self-contained recordings list + player that talks to this same origin,
/// so there is no cross-origin, no mixed content, and nothing fetched from
/// the internet. Served token-less; every API call it makes carries the
/// token the user scanned.
const PHONE_PAGE: &str = include_str!("../assets/local_recordings.html");

/// The opt-in LAN half of the listener. Separate from the loopback one so it
/// can be turned off without disturbing a same-machine web session, and so
/// its exposure is a deliberate act rather than a side effect of starting
/// the API.
#[derive(Debug, Clone)]
pub(crate) struct LanShare {
    pub(crate) bind: String,
    stop: Arc<AtomicBool>,
}

/// Server-side state for a running listener. `token` is generated per launch
/// and never persisted: restarting the desktop invalidates every pasted or
/// scanned connect URL, which is the intended lifetime.
#[derive(Debug, Clone)]
pub(crate) struct LocalApiControl {
    pub(crate) bind: String,
    pub(crate) token: String,
    pub(crate) lan: Option<LanShare>,
}

impl LocalApiControl {
    /// The one string a user pastes into the web dashboard. The token rides in
    /// the fragment so it never appears in a request line or a server log.
    pub(crate) fn connect_url(&self) -> String {
        format!("http://{}/#{}", self.bind, self.token)
    }

    /// What the phone scans: this machine's LAN address plus the LAN
    /// listener's port. `None` until LAN sharing is on or when no non-loopback
    /// IPv4 can be determined (the UI then says so instead of guessing).
    pub(crate) fn lan_url(&self) -> Option<String> {
        let lan = self.lan.as_ref()?;
        let (_, port) = lan.bind.rsplit_once(':')?;
        let ip = crate::primary_lan_ipv4()?;
        Some(format!("http://{ip}:{port}/#{}", self.token))
    }
}

/// What the `start_local_api` / `set_local_api_lan` commands return to the
/// desktop UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalApiInfo {
    pub(crate) bind: String,
    pub(crate) token: String,
    pub(crate) connect_url: String,
    pub(crate) lan_enabled: bool,
    pub(crate) lan_bind: Option<String>,
    pub(crate) lan_url: Option<String>,
}

fn info(control: &LocalApiControl) -> LocalApiInfo {
    LocalApiInfo {
        bind: control.bind.clone(),
        token: control.token.clone(),
        connect_url: control.connect_url(),
        lan_enabled: control.lan.is_some(),
        lan_bind: control.lan.as_ref().map(|lan| lan.bind.clone()),
        lan_url: control.lan_url(),
    }
}

/// RAII decrement for the in-flight counter, held for the lifetime of a
/// connection thread so the slot is freed on every exit path including a
/// panic unwinding out of the handler (same shape as `fungwire_server`).
struct SlotGuard(Arc<AtomicUsize>);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Starts the listener if it is not already running and returns its
/// connection details. Idempotent: a second call returns the same bind and
/// token rather than rotating them, so the desktop UI can re-read the connect
/// URL without invalidating a web session that already pasted it.
pub(crate) fn start(state: &AppState) -> std::io::Result<LocalApiInfo> {
    let mut current = state.local_api.lock().expect("local api mutex poisoned");
    if let Some(control) = current.as_ref() {
        return Ok(info(control));
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    // Two v4 UUIDs give 256 bits of randomness as 64 hex characters.
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let control = LocalApiControl {
        bind,
        token,
        lan: None,
    };

    // The loopback listener lives as long as the app; its stop flag is
    // never set, it exists so both listeners share one accept loop.
    spawn_listener(
        listener,
        state.genesis.clone(),
        state.genesis_path.clone(),
        Arc::new(control.clone()),
        Some(import_host(state)),
        Arc::new(AtomicBool::new(false)),
    )?;

    *current = Some(control.clone());
    Ok(info(&control))
}

/// Turns the opt-in LAN listener on or off. Binds `0.0.0.0:0` only while
/// enabled — the default is loopback-only — and serves the same routes with
/// the same per-launch token, so a phone that scanned the QR and the web
/// tab that pasted the loopback URL are the same trust decision. Starts the
/// loopback listener first if it is not running yet (the token lives there).
pub(crate) fn set_lan(state: &AppState, enabled: bool) -> std::io::Result<LocalApiInfo> {
    if state
        .local_api
        .lock()
        .expect("local api mutex poisoned")
        .is_none()
    {
        start(state)?;
    }
    let mut current = state.local_api.lock().expect("local api mutex poisoned");
    let control = current.as_mut().expect("started above");

    match (enabled, control.lan.as_ref()) {
        (true, Some(_)) | (false, None) => {}
        (false, Some(lan)) => {
            lan.stop.store(true, Ordering::SeqCst);
            control.lan = None;
        }
        (true, None) => {
            let listener = TcpListener::bind("0.0.0.0:0")?;
            let bind = listener.local_addr()?.to_string();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_listener(
                listener,
                state.genesis.clone(),
                state.genesis_path.clone(),
                Arc::new(control.clone()),
                Some(import_host(state)),
                stop.clone(),
            )?;
            control.lan = Some(LanShare { bind, stop });
        }
    }
    Ok(info(control))
}

fn import_host(state: &AppState) -> Arc<ImportHost> {
    Arc::new(ImportHost {
        genesis: state.genesis.clone(),
        recording_output: Arc::clone(&state.recording_output),
        runtime: state.whisper_runtime_clone(),
    })
}

/// One accept loop for either bind. Non-blocking accept polled every 40 ms
/// (the `fungwire_server` / mobile gateway shape) so `stop` is honoured
/// promptly and the listener socket is released when the loop exits.
fn spawn_listener(
    listener: TcpListener,
    storage: Arc<genesis_block_native::Storage>,
    genesis_path: std::path::PathBuf,
    control: Arc<LocalApiControl>,
    host: Option<Arc<ImportHost>>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<()> {
    listener.set_nonblocking(true)?;
    let in_flight = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    // The accepted socket must block: the handler reads with
                    // a timeout and writes a whole response.
                    if stream.set_nonblocking(false).is_err() {
                        continue;
                    }
                    if in_flight.load(Ordering::SeqCst) >= MAX_IN_FLIGHT {
                        drop(stream);
                        continue;
                    }
                    in_flight.fetch_add(1, Ordering::SeqCst);
                    let storage = storage.clone();
                    let genesis_path = genesis_path.clone();
                    let control = control.clone();
                    let host = host.clone();
                    let slots = in_flight.clone();
                    thread::spawn(move || {
                        let _slot = SlotGuard(slots);
                        handle_stream(stream, &storage, &genesis_path, &control, host.as_deref());
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(40));
                }
                Err(_) => break,
            }
        }
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP plumbing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: BTreeMap<String, String>,
    /// Header names lower-cased; values trimmed.
    pub(crate) headers: BTreeMap<String, String>,
    /// Raw body; empty for every method but `POST`.
    pub(crate) body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

#[derive(Debug)]
pub(crate) struct Response {
    pub(crate) status: &'static str,
    pub(crate) content_type: &'static str,
    pub(crate) body: Vec<u8>,
    pub(crate) extra_headers: Vec<(String, String)>,
}

impl Response {
    fn json(status: &'static str, value: Value) -> Self {
        Response {
            status,
            content_type: "application/json",
            body: value.to_string().into_bytes(),
            extra_headers: Vec::new(),
        }
    }

    fn empty(status: &'static str) -> Self {
        Response {
            status,
            content_type: "text/plain",
            body: Vec::new(),
            extra_headers: Vec::new(),
        }
    }
}

fn head_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Reads the request head, then — for `POST` only — exactly `Content-Length`
/// bytes of body, capped at [`MAX_UPLOAD_BYTES`]. A body on any other
/// method is left unread; the router rejects those methods anyway. `Err`
/// carries the response to send instead (`400` for a malformed head,
/// `413` for an oversized upload).
fn read_request(stream: &mut TcpStream) -> Result<Request, Response> {
    let bad = || Response::json("400 Bad Request", json!({"error": "BAD_REQUEST"}));
    stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT)).ok();
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 4096];
    loop {
        if head_end(&buffer).is_some() {
            break;
        }
        if buffer.len() > MAX_REQUEST_HEAD {
            return Err(bad());
        }
        let read = stream.read(&mut chunk).map_err(|_| bad())?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let end = head_end(&buffer).ok_or_else(bad)?;
    let mut request = parse_request(&String::from_utf8_lossy(&buffer[..end])).ok_or_else(bad)?;
    if request.method != "POST" {
        return Ok(request);
    }
    let length: usize = request
        .header("content-length")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            Response::json("411 Length Required", json!({"error": "LENGTH_REQUIRED"}))
        })?;
    if length > MAX_UPLOAD_BYTES {
        return Err(Response::json(
            "413 Payload Too Large",
            json!({"error": "PAYLOAD_TOO_LARGE", "maxBytes": MAX_UPLOAD_BYTES}),
        ));
    }
    let mut body = buffer[end + 4..].to_vec();
    body.reserve(length.saturating_sub(body.len()));
    while body.len() < length {
        let read = stream.read(&mut chunk).map_err(|_| bad())?;
        if read == 0 {
            return Err(bad());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    request.body = body;
    Ok(request)
}

pub(crate) fn parse_request(head: &str) -> Option<Request> {
    let mut lines = head.lines();
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_ascii_uppercase();
    let target = request_line.next()?;
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, parse_query(query)),
        None => (target, BTreeMap::new()),
    };
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    Some(Request {
        method,
        path: path.to_string(),
        query,
        headers,
        body: Vec::new(),
    })
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (percent_decode(key), percent_decode(value)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                // Work on bytes, not a `str` slice: the two characters after
                // `%` may not be ASCII, and slicing a `str` mid-codepoint panics.
                let decoded = std::str::from_utf8(&bytes[index + 1..index + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                match decoded {
                    Some(byte) => {
                        out.push(byte);
                        index += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn write_response(stream: &mut TcpStream, response: Response, cors_origin: Option<&str>) {
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    if let Some(origin) = cors_origin {
        head.push_str(&format!("Access-Control-Allow-Origin: {origin}\r\n"));
        head.push_str("Vary: Origin\r\n");
        head.push_str("Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n");
        head.push_str(
            "Access-Control-Allow-Headers: Authorization, Range, Content-Type, X-Fung-Filename\r\n",
        );
        head.push_str(
            "Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges, X-Fung-Missing-Chunks\r\n",
        );
        // Chrome's Private Network Access / Local Network Access preflight:
        // a public https page reaching loopback must be explicitly allowed.
        head.push_str("Access-Control-Allow-Private-Network: true\r\n");
        head.push_str("Access-Control-Max-Age: 600\r\n");
    }
    for (name, value) in &response.extra_headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
}

fn handle_stream(
    mut stream: TcpStream,
    storage: &genesis_block_native::Storage,
    genesis_path: &Path,
    control: &LocalApiControl,
    host: Option<&ImportHost>,
) {
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(response) => {
            write_response(&mut stream, response, None);
            return;
        }
    };
    let origin = request.header("origin").map(str::to_string);
    let cors = origin.as_deref().filter(|origin| origin_allowed(origin));
    let response = route(&request, storage, genesis_path, control, host);
    write_response(&mut stream, response, cors);
}

// ---------------------------------------------------------------------------
// Policy: who may talk to this, and from where
// ---------------------------------------------------------------------------

/// Whether a browser `Origin` may read responses. Exact match against the
/// production web origin(s), plus any `http(s)://localhost` or
/// `http(s)://127.0.0.1` origin on any port for local development.
pub(crate) fn origin_allowed(origin: &str) -> bool {
    if WEB_ORIGINS.contains(&origin) {
        return true;
    }
    if let Ok(extra) = std::env::var("FUNG_WEB_ORIGINS") {
        if extra
            .split(',')
            .map(str::trim)
            .any(|allowed| allowed == origin)
        {
            return true;
        }
    }
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = rest.split(':').next().unwrap_or("");
    host == "localhost" || host == "127.0.0.1"
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Bearer header for `fetch`, `?token=` for `<audio src>`; either proves the
/// caller was handed the connect URL.
pub(crate) fn authorized(request: &Request, token: &str) -> bool {
    let header = request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    let query = request.query.get("token").map(String::as_str);
    header.is_some_and(|value| constant_time_eq(value, token))
        || query.is_some_and(|value| constant_time_eq(value, token))
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub(crate) fn route(
    request: &Request,
    storage: &genesis_block_native::Storage,
    genesis_path: &Path,
    control: &LocalApiControl,
    host: Option<&ImportHost>,
) -> Response {
    if request.method == "OPTIONS" {
        return Response::empty("204 No Content");
    }
    if request.method == "POST" {
        if request.path != "/recordings/import" {
            return Response::json("404 Not Found", json!({"error": "NOT_FOUND"}));
        }
        if !authorized(request, &control.token) {
            return Response::json("401 Unauthorized", json!({"error": "AUTH_REQUIRED"}));
        }
        return import_recording(request, storage, host);
    }
    if request.method != "GET" {
        return Response::json(
            "405 Method Not Allowed",
            json!({"error": "METHOD_NOT_ALLOWED"}),
        );
    }
    // The phone page is token-less by design: it contains no secret, and
    // every request it makes is gated like any other client's.
    if request.path == "/" || request.path == "/index.html" {
        return Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: PHONE_PAGE.as_bytes().to_vec(),
            extra_headers: vec![
                ("X-Content-Type-Options".to_string(), "nosniff".to_string()),
                ("Referrer-Policy".to_string(), "no-referrer".to_string()),
            ],
        };
    }
    // `/health` predates the token and stays open: it is how an operator
    // checks the process is alive before they have a connect URL.
    if request.path == "/health" {
        return Response::json(
            "200 OK",
            json!({
                "app": "FUNG",
                "version": env!("CARGO_PKG_VERSION"),
                "databasePath": genesis_path.display().to_string(),
                "storageAuthority": "GenesisBlockDB signed WAL",
                "stableFrontier": storage.stable_frontier()
            }),
        );
    }
    if !authorized(request, &control.token) {
        return Response::json("401 Unauthorized", json!({"error": "AUTH_REQUIRED"}));
    }
    if request.path == "/recordings" {
        return match list_recordings(storage) {
            Ok(value) => Response::json("200 OK", value),
            Err(error) => Response::json(
                "500 Internal Server Error",
                json!({"error": "LEDGER_READ_FAILED", "detail": error}),
            ),
        };
    }
    if let Some(job_id) = request
        .path
        .strip_prefix("/jobs/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
    {
        return match crate::job_by_id(storage, job_id) {
            Ok(job) => Response::json("200 OK", json!({"job": job})),
            Err(crate::AppError::InvalidInput(detail)) => Response::json(
                "404 Not Found",
                json!({"error": "NOT_FOUND", "detail": detail}),
            ),
            Err(error) => Response::json(
                "500 Internal Server Error",
                json!({"error": "LEDGER_READ_FAILED", "detail": error.to_string()}),
            ),
        };
    }
    if let Some(recording_id) = request
        .path
        .strip_prefix("/recordings/")
        .and_then(|rest| rest.strip_suffix("/transcript"))
        .filter(|id| !id.is_empty() && !id.contains('/'))
    {
        return transcript(storage, recording_id);
    }
    if let Some(recording_id) = request
        .path
        .strip_prefix("/recordings/")
        .and_then(|rest| rest.strip_suffix("/audio"))
        .filter(|id| !id.is_empty() && !id.contains('/'))
    {
        let channel = request.query.get("channel").map(String::as_str);
        return match build_audio(storage, recording_id, channel) {
            Ok(audio) => serve_bytes(audio, request.header("range")),
            Err(AudioError::NotFound(detail)) => Response::json(
                "404 Not Found",
                json!({"error": "NOT_FOUND", "detail": detail}),
            ),
            Err(AudioError::Unsupported(detail)) => Response::json(
                "415 Unsupported Media Type",
                json!({"error": "UNSUPPORTED_AUDIO", "detail": detail}),
            ),
            Err(AudioError::Internal(detail)) => Response::json(
                "500 Internal Server Error",
                json!({"error": "AUDIO_READ_FAILED", "detail": detail}),
            ),
        };
    }
    Response::json(
        "404 Not Found",
        json!({
            "error": "NOT_FOUND",
            "available": [
                "/health",
                "/recordings",
                "/recordings/{id}/audio?channel=mic|system|file",
                "/recordings/{id}/transcript",
                "/jobs/{id}",
                "POST /recordings/import"
            ]
        }),
    )
}

// ---------------------------------------------------------------------------
// Upload → import → transcript
// ---------------------------------------------------------------------------

/// File extension for an uploaded body, from its `Content-Type`. The worker
/// decodes by content, not by name, so an unknown type still transcribes;
/// the extension only keeps the stored file recognisable.
pub(crate) fn upload_extension(content_type: Option<&str>) -> &'static str {
    let container = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match container.as_str() {
        "audio/webm" | "video/webm" => "webm",
        "audio/mp4" | "audio/x-m4a" | "audio/aac" => "m4a",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/flac" => "flac",
        _ => "bin",
    }
}

/// A display name for the project the upload lands in, from the optional
/// `X-Fung-Filename` header: the stem, restricted to characters that are
/// harmless in a project name and a log line. Empty → a dated default.
pub(crate) fn upload_project_name(filename: Option<&str>) -> String {
    let stem = filename
        .map(|name| name.rsplit(['/', '\\']).next().unwrap_or(name))
        .map(|name| name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name))
        .map(|stem| {
            stem.chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
                .take(80)
                .collect::<String>()
        })
        .map(|stem| stem.trim().to_string())
        .filter(|stem| !stem.is_empty());
    stem.unwrap_or_else(|| format!("Web recording {}", crate::now()))
}

fn import_recording(
    request: &Request,
    storage: &genesis_block_native::Storage,
    host: Option<&ImportHost>,
) -> Response {
    let Some(host) = host else {
        return Response::json(
            "503 Service Unavailable",
            json!({"error": "IMPORT_UNAVAILABLE", "detail": "this listener has no transcription runtime"}),
        );
    };
    if request.body.is_empty() {
        return Response::json("400 Bad Request", json!({"error": "EMPTY_BODY"}));
    }
    let recording_id = Uuid::new_v4().to_string();
    let extension = upload_extension(request.header("content-type"));
    let name = upload_project_name(request.header("x-fung-filename"));
    let output_root = match host
        .recording_output
        .lock()
        .expect("recording output mutex poisoned")
        .ensure_current_writable()
    {
        Ok(path) => path,
        Err(error) => {
            return Response::json(
                "503 Service Unavailable",
                json!({"error": "OUTPUT_UNAVAILABLE", "detail": error}),
            )
        }
    };
    let directory = output_root.join("imports").join("web");
    let path = directory.join(format!("{recording_id}.{extension}"));
    if let Err(error) = fs::create_dir_all(&directory).and_then(|_| fs::write(&path, &request.body))
    {
        return Response::json(
            "500 Internal Server Error",
            json!({"error": "WRITE_FAILED", "detail": error.to_string()}),
        );
    }
    let path_string = path.display().to_string();
    let project_id = match crate::create_project_named(storage, &output_root, &name) {
        Ok(id) => id,
        Err(error) => {
            return Response::json(
                "500 Internal Server Error",
                json!({"error": "LEDGER_WRITE_FAILED", "detail": error.to_string()}),
            )
        }
    };
    let job = match crate::create_import_job(&host.genesis, &project_id, &path_string) {
        Ok(job) => job,
        Err(error) => {
            return Response::json(
                "500 Internal Server Error",
                json!({"error": "LEDGER_WRITE_FAILED", "detail": error.to_string()}),
            )
        }
    };

    let genesis = host.genesis.clone();
    let runtime = host.runtime.clone();
    let job_id = job.id.clone();
    let worker_project = project_id.clone();
    let worker_recording = recording_id.clone();
    thread::spawn(move || {
        crate::run_import_pipeline_as(
            &genesis,
            &runtime,
            &worker_project,
            &job_id,
            "web-import",
            &path_string,
            Path::new(&path_string),
            crate::ImportProgress::whole_job(),
            &worker_recording,
        );
    });

    Response::json(
        "202 Accepted",
        json!({
            "jobId": job.id,
            "projectId": project_id,
            "recordingId": recording_id,
            "bytes": request.body.len(),
            "extension": extension,
        }),
    )
}

/// The recording's transcript segments, resolved through its own project
/// so the project-scoped read (`transcript_view`) applies unchanged.
fn transcript(storage: &genesis_block_native::Storage, recording_id: &str) -> Response {
    let project_id = match genesis_adapter::query(
        storage,
        "recordings",
        &["project_id"],
        vec![genesis_adapter::eq("recordings", "id", json!(recording_id))],
        1,
    ) {
        Ok(rows) => rows
            .into_iter()
            .next()
            .and_then(|row| str_col(&row, "recordings", "project_id")),
        Err(error) => {
            return Response::json(
                "500 Internal Server Error",
                json!({"error": "LEDGER_READ_FAILED", "detail": error}),
            )
        }
    };
    let Some(project_id) = project_id else {
        return Response::json("404 Not Found", json!({"error": "NOT_FOUND"}));
    };
    match crate::transcript_view(storage, &project_id, recording_id) {
        Ok(view) => Response::json(
            "200 OK",
            json!({"projectId": project_id, "recordingId": recording_id, "transcript": view}),
        ),
        Err(crate::AppError::InvalidInput(detail)) => Response::json(
            "404 Not Found",
            json!({"error": "NOT_FOUND", "detail": detail}),
        ),
        Err(error) => Response::json(
            "500 Internal Server Error",
            json!({"error": "LEDGER_READ_FAILED", "detail": error.to_string()}),
        ),
    }
}

// ---------------------------------------------------------------------------
// Ledger reads
// ---------------------------------------------------------------------------

fn str_col(row: &Value, table: &str, column: &str) -> Option<String> {
    row.get(format!("{table}.{column}"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn int_col(row: &Value, table: &str, column: &str) -> Option<i64> {
    let value = row.get(format!("{table}.{column}"))?;
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Desktop live capture writes one WAV per channel per slice as
/// `{channel}-{seq:05}.wav` (`live_meeting::cut_chunk`). Anything that does
/// not follow that naming is a whole-file source (an import) and is served
/// as-is under the pseudo-channel `file`.
fn channel_of(path: &str) -> &'static str {
    let name = file_name(path);
    if name.starts_with("mic-") && name.ends_with(".wav") {
        "mic"
    } else if name.starts_with("system-") && name.ends_with(".wav") {
        "system"
    } else {
        "file"
    }
}

/// The per-channel ordinal from `{channel}-{seq:05}.wav`, when present.
fn chunk_ordinal(path: &str) -> Option<u32> {
    let name = file_name(path);
    let stem = name.strip_suffix(".wav")?;
    let (_, ordinal) = stem.rsplit_once('-')?;
    ordinal.parse().ok()
}

fn channels_of(paths: &[String]) -> Vec<String> {
    ["mic", "system", "file"]
        .into_iter()
        .filter(|channel| paths.iter().any(|path| channel_of(path) == *channel))
        .map(str::to_string)
        .collect()
}

pub(crate) fn list_recordings(storage: &genesis_block_native::Storage) -> Result<Value, String> {
    let projects = genesis_adapter::query_all(storage, "projects", &["id", "name"], vec![])?;
    let names: BTreeMap<String, String> = projects
        .iter()
        .filter_map(|row| {
            Some((
                str_col(row, "projects", "id")?,
                str_col(row, "projects", "name")?,
            ))
        })
        .collect();

    let chunks = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &["recording_id", "file_path"],
        vec![],
    )?;
    let mut paths_by_recording: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in &chunks {
        if let (Some(recording_id), Some(path)) = (
            str_col(row, "audio_chunks", "recording_id"),
            str_col(row, "audio_chunks", "file_path"),
        ) {
            paths_by_recording
                .entry(recording_id)
                .or_default()
                .push(path);
        }
    }

    let mut recordings = genesis_adapter::query_all(
        storage,
        "recordings",
        &[
            "id",
            "project_id",
            "source",
            "status",
            "duration_ms",
            "created_at",
            "language",
        ],
        vec![],
    )?;
    // Newest first; `created_at` is RFC 3339 so lexical order is time order.
    recordings.sort_by(|a, b| {
        str_col(b, "recordings", "created_at").cmp(&str_col(a, "recordings", "created_at"))
    });

    let list: Vec<Value> = recordings
        .iter()
        .filter_map(|row| {
            let id = str_col(row, "recordings", "id")?;
            let project_id = str_col(row, "recordings", "project_id").unwrap_or_default();
            let paths = paths_by_recording
                .get(&id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            Some(json!({
                "id": id,
                "projectId": project_id,
                "projectName": names.get(&project_id),
                "source": str_col(row, "recordings", "source"),
                "status": str_col(row, "recordings", "status"),
                "durationMs": int_col(row, "recordings", "duration_ms").unwrap_or(0),
                "createdAt": str_col(row, "recordings", "created_at"),
                "language": str_col(row, "recordings", "language"),
                "channels": channels_of(paths),
                "chunkCount": paths.len(),
            }))
        })
        .collect();

    Ok(json!({ "recordings": list }))
}

// ---------------------------------------------------------------------------
// Audio assembly
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) enum AudioError {
    NotFound(String),
    Unsupported(String),
    Internal(String),
}

#[derive(Debug)]
pub(crate) struct Audio {
    pub(crate) bytes: Vec<u8>,
    pub(crate) mime: &'static str,
    /// Chunks the ledger lists that are not on disk. Reported in a response
    /// header rather than failing the whole playback: the user hears what
    /// survived and can run the integrity check to learn what did not.
    pub(crate) missing_chunks: usize,
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav") => "audio/wav",
        Some("m4a") | Some("mp4") | Some("aac") => "audio/mp4",
        Some("webm") => "audio/webm",
        Some("mp3") => "audio/mpeg",
        Some("ogg") | Some("opus") => "audio/ogg",
        Some("flac") => "audio/flac",
        _ => "application/octet-stream",
    }
}

/// Assembles one playable stream for a recording. Desktop live captures are
/// stitched from their per-channel WAV slices in order; whole-file imports
/// are returned as the file on disk.
///
/// The stitched WAV is built in memory. An hour of 48 kHz mono is ~345 MB,
/// which a desktop tolerates for a same-machine playback; if this ever serves
/// more than one browser tab at a time it should stream instead.
pub(crate) fn build_audio(
    storage: &genesis_block_native::Storage,
    recording_id: &str,
    channel: Option<&str>,
) -> Result<Audio, AudioError> {
    let rows = genesis_adapter::query(
        storage,
        "recordings",
        &["id", "project_id"],
        vec![genesis_adapter::eq("recordings", "id", json!(recording_id))],
        1,
    )
    .map_err(AudioError::Internal)?;
    let row = rows
        .first()
        .ok_or_else(|| AudioError::NotFound("recording not found".to_string()))?;
    let project_id = str_col(row, "recordings", "project_id").unwrap_or_default();

    let project_rows = genesis_adapter::query(
        storage,
        "projects",
        &["id", "storage_path"],
        vec![genesis_adapter::eq("projects", "id", json!(project_id))],
        1,
    )
    .map_err(AudioError::Internal)?;
    let project_storage = project_rows
        .first()
        .and_then(|row| str_col(row, "projects", "storage_path"))
        .unwrap_or_default();
    let root = Path::new(&project_storage);

    let chunk_rows = genesis_adapter::query_all(
        storage,
        "audio_chunks",
        &["file_path", "sequence_no"],
        vec![genesis_adapter::eq(
            "audio_chunks",
            "recording_id",
            json!(recording_id),
        )],
    )
    .map_err(AudioError::Internal)?;
    let chunks: Vec<(i64, String)> = chunk_rows
        .iter()
        .filter_map(|row| {
            Some((
                int_col(row, "audio_chunks", "sequence_no").unwrap_or(0),
                str_col(row, "audio_chunks", "file_path")?,
            ))
        })
        .collect();
    if chunks.is_empty() {
        return Err(AudioError::NotFound(
            "recording has no audio chunks".to_string(),
        ));
    }

    let paths: Vec<String> = chunks.iter().map(|(_, path)| path.clone()).collect();
    let available = channels_of(&paths);
    let channel = match channel {
        Some(channel) => channel.to_string(),
        None => available
            .first()
            .cloned()
            .ok_or_else(|| AudioError::NotFound("no playable channel".to_string()))?,
    };
    if !available.contains(&channel) {
        return Err(AudioError::NotFound(format!(
            "channel {channel} not present; available: {}",
            available.join(",")
        )));
    }

    if channel == "file" {
        let (_, recorded) = chunks
            .iter()
            .find(|(_, path)| channel_of(path) == "file")
            .expect("channel presence was checked above");
        // Recorded paths are data. `resolve_chunk_path` refuses to follow one
        // that climbs out of the project directory.
        let resolved = audio_custody::resolve_chunk_path(root, recorded)
            .ok_or_else(|| AudioError::NotFound("audio file is not on disk".to_string()))?;
        let bytes = fs::read(&resolved).map_err(|error| AudioError::Internal(error.to_string()))?;
        return Ok(Audio {
            mime: mime_for(&resolved),
            bytes,
            missing_chunks: 0,
        });
    }

    let mut selected: Vec<(u32, String)> = chunks
        .into_iter()
        .filter(|(_, path)| channel_of(path) == channel)
        .map(|(sequence_no, path)| {
            (
                chunk_ordinal(&path).unwrap_or(sequence_no.max(0) as u32),
                path,
            )
        })
        .collect();
    selected.sort_by_key(|(ordinal, _)| *ordinal);
    stitch_wav(root, &selected)
}

fn stitch_wav(root: &Path, chunks: &[(u32, String)]) -> Result<Audio, AudioError> {
    let mut spec: Option<hound::WavSpec> = None;
    let mut samples: Vec<i16> = Vec::new();
    let mut missing_chunks = 0;

    for (_, recorded) in chunks {
        let Some(path) = audio_custody::resolve_chunk_path(root, recorded) else {
            missing_chunks += 1;
            continue;
        };
        let mut reader = hound::WavReader::open(&path)
            .map_err(|error| AudioError::Internal(format!("{}: {error}", path.display())))?;
        let this = reader.spec();
        if this.bits_per_sample != 16 || this.sample_format != hound::SampleFormat::Int {
            return Err(AudioError::Unsupported(format!(
                "{} is not 16-bit integer PCM",
                path.display()
            )));
        }
        match spec {
            None => spec = Some(this),
            Some(first)
                if first.sample_rate != this.sample_rate || first.channels != this.channels =>
            {
                return Err(AudioError::Unsupported(format!(
                    "{} does not match the recording's format",
                    path.display()
                )));
            }
            Some(_) => {}
        }
        for sample in reader.samples::<i16>() {
            samples.push(sample.map_err(|error| AudioError::Internal(error.to_string()))?);
        }
    }

    let spec = spec.ok_or_else(|| {
        AudioError::NotFound(format!("none of the {} chunks are on disk", chunks.len()))
    })?;

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|error| AudioError::Internal(error.to_string()))?;
        for sample in &samples {
            writer
                .write_sample(*sample)
                .map_err(|error| AudioError::Internal(error.to_string()))?;
        }
        writer
            .finalize()
            .map_err(|error| AudioError::Internal(error.to_string()))?;
    }
    Ok(Audio {
        bytes: cursor.into_inner(),
        mime: "audio/wav",
        missing_chunks,
    })
}

/// Single-range `bytes=` parsing per RFC 9110 §14.1.2, returning an inclusive
/// `(start, end)` clamped to the body, or `None` when unsatisfiable.
pub(crate) fn parse_range(header: &str, total: usize) -> Option<(usize, usize)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    if total == 0 {
        return None;
    }
    let last = total - 1;
    match (start.trim(), end.trim()) {
        ("", suffix) => {
            let length: usize = suffix.parse().ok()?;
            if length == 0 {
                return None;
            }
            Some((total - length.min(total), last))
        }
        (start, "") => {
            let start: usize = start.parse().ok()?;
            (start <= last).then_some((start, last))
        }
        (start, end) => {
            let start: usize = start.parse().ok()?;
            let end: usize = end.parse().ok()?;
            if start > end || start > last {
                return None;
            }
            Some((start, end.min(last)))
        }
    }
}

fn serve_bytes(audio: Audio, range: Option<&str>) -> Response {
    let total = audio.bytes.len();
    let mut extra_headers = vec![("Accept-Ranges".to_string(), "bytes".to_string())];
    if audio.missing_chunks > 0 {
        extra_headers.push((
            "X-Fung-Missing-Chunks".to_string(),
            audio.missing_chunks.to_string(),
        ));
    }
    match range.map(|header| parse_range(header, total)) {
        None => Response {
            status: "200 OK",
            content_type: audio.mime,
            body: audio.bytes,
            extra_headers,
        },
        Some(Some((start, end))) => {
            extra_headers.push((
                "Content-Range".to_string(),
                format!("bytes {start}-{end}/{total}"),
            ));
            Response {
                status: "206 Partial Content",
                content_type: audio.mime,
                body: audio.bytes[start..=end].to_vec(),
                extra_headers,
            }
        }
        Some(None) => {
            extra_headers.push(("Content-Range".to_string(), format!("bytes */{total}")));
            Response {
                status: "416 Range Not Satisfiable",
                content_type: "application/json",
                body: br#"{"error":"RANGE_NOT_SATISFIABLE"}"#.to_vec(),
                extra_headers,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_block_native::{OpenOptions, Storage};
    use std::path::PathBuf;

    fn open_genesis() -> (PathBuf, Storage) {
        let path = std::env::temp_dir().join(format!("fung-local-api-test-{}", Uuid::new_v4()));
        let storage = Storage::open(OpenOptions {
            path: path.display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .expect("open GenesisBlockDB");
        genesis_adapter::install(&storage).expect("install schema");
        (path, storage)
    }

    fn write_wav(path: &Path, sample_rate: u32, samples: &[i16]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn chunk_row(
        id: &str,
        recording_id: &str,
        seq: i64,
        path: &Path,
    ) -> genesis_block_native::RelationalRowMutation {
        genesis_adapter::upsert(
            "audio_chunks",
            json!({
                "id": id,
                "recording_id": recording_id,
                "sequence_no": seq,
                "file_path": path.display().to_string(),
                "start_ms": 0,
                "end_ms": 1000,
                "byte_size": 0,
                "checksum": "",
                "created_at": "2026-09-13T00:00:00Z",
            }),
        )
    }

    /// One project with a two-channel live recording (two mic slices, one
    /// system slice) and a whole-file import.
    fn seed(storage: &Storage, project_dir: &Path) {
        let live = project_dir.join("live").join("rec-live").join("chunks");
        write_wav(&live.join("mic-00001.wav"), 16_000, &[1, 2, 3]);
        write_wav(&live.join("mic-00002.wav"), 16_000, &[4, 5]);
        write_wav(&live.join("system-00001.wav"), 16_000, &[9, 9, 9, 9]);
        let import = project_dir
            .join("imports")
            .join("rec-import")
            .join("meeting.m4a");
        fs::create_dir_all(import.parent().unwrap()).unwrap();
        fs::write(&import, b"not really aac").unwrap();

        genesis_adapter::commit_rows(
            storage,
            vec![
                genesis_adapter::upsert(
                    "projects",
                    json!({"id": "p1", "name": "Standup", "storage_path": project_dir.display().to_string(), "created_at": "t", "updated_at": "t"}),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    json!({"id": "rec-live", "project_id": "p1", "source": "microphone", "input_path": null, "canonical_audio_path": "manifest", "status": "completed", "duration_ms": 5000, "created_at": "2026-09-13T10:00:00Z", "updated_at": "t"}),
                ),
                genesis_adapter::upsert(
                    "recordings",
                    json!({"id": "rec-import", "project_id": "p1", "source": "import", "input_path": "meeting.m4a", "canonical_audio_path": "manifest", "status": "completed", "duration_ms": 9000, "created_at": "2026-09-12T10:00:00Z", "updated_at": "t"}),
                ),
                chunk_row("c-mic-2", "rec-live", 3, &live.join("mic-00002.wav")),
                chunk_row("c-mic-1", "rec-live", 1, &live.join("mic-00001.wav")),
                chunk_row("c-sys-1", "rec-live", 2, &live.join("system-00001.wav")),
                chunk_row("c-import", "rec-import", 1, &import),
            ],
        )
        .expect("seed rows");
    }

    fn control() -> LocalApiControl {
        LocalApiControl {
            bind: "127.0.0.1:1".to_string(),
            token: "secret-token".to_string(),
            lan: None,
        }
    }

    fn get(path: &str, headers: &[(&str, &str)]) -> Request {
        let mut head = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        parse_request(&head).unwrap()
    }

    #[test]
    fn parses_request_line_query_and_headers() {
        let request = get(
            "/recordings/abc/audio?channel=mic&token=t%20k",
            &[("Authorization", "Bearer x"), ("RANGE", " bytes=0-1 ")],
        );
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/recordings/abc/audio");
        assert_eq!(request.query["channel"], "mic");
        assert_eq!(request.query["token"], "t k");
        assert_eq!(request.headers["authorization"], "Bearer x");
        assert_eq!(request.headers["range"], "bytes=0-1");
        assert!(parse_request("").is_none());
    }

    #[test]
    fn range_parsing_follows_rfc_9110() {
        assert_eq!(parse_range("bytes=0-1", 10), Some((0, 1)));
        assert_eq!(parse_range("bytes=5-", 10), Some((5, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(
            parse_range("bytes=0-99", 10),
            Some((0, 9)),
            "end is clamped"
        );
        assert_eq!(parse_range("bytes=10-", 10), None, "start past the end");
        assert_eq!(parse_range("bytes=4-2", 10), None);
        assert_eq!(
            parse_range("bytes=0-1,4-5", 10),
            None,
            "multi-range unsupported"
        );
        assert_eq!(parse_range("items=0-1", 10), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
    }

    #[test]
    fn origin_allowlist_is_production_web_plus_loopback_dev() {
        assert!(origin_allowed("https://fung-seven.vercel.app"));
        assert!(origin_allowed("http://localhost:5173"));
        assert!(origin_allowed("http://127.0.0.1:1420"));
        assert!(origin_allowed("http://localhost"));
        assert!(!origin_allowed("https://evil.example"));
        assert!(!origin_allowed("http://localhost.evil.example"));
        assert!(!origin_allowed("http://127.0.0.1.evil.example"));
        assert!(!origin_allowed("null"));
        assert!(
            !origin_allowed("fung-seven.vercel.app"),
            "scheme is part of the origin"
        );
    }

    #[test]
    fn token_is_accepted_from_header_or_query_only_when_exact() {
        let token = "secret-token";
        assert!(authorized(
            &get("/recordings", &[("Authorization", "Bearer secret-token")]),
            token
        ));
        assert!(authorized(
            &get("/recordings?token=secret-token", &[]),
            token
        ));
        assert!(!authorized(
            &get("/recordings", &[("Authorization", "Bearer secret-toke")]),
            token
        ));
        assert!(!authorized(
            &get("/recordings", &[("Authorization", "Bearer secret-token2")]),
            token
        ));
        assert!(!authorized(&get("/recordings?token=", &[]), token));
        assert!(!authorized(&get("/recordings", &[]), token));
        assert!(
            !authorized(
                &get("/recordings", &[("Authorization", "secret-token")]),
                token
            ),
            "scheme required"
        );
    }

    #[test]
    fn health_is_open_but_everything_else_needs_the_token() {
        let (dir, storage) = open_genesis();
        let control = control();
        let health = route(&get("/health", &[]), &storage, &dir, &control, None);
        assert_eq!(health.status, "200 OK");

        let denied = route(&get("/recordings", &[]), &storage, &dir, &control, None);
        assert_eq!(denied.status, "401 Unauthorized");

        let denied = route(
            &get("/recordings/x/audio?token=wrong", &[]),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(denied.status, "401 Unauthorized");

        let preflight = route(
            &parse_request(
                "OPTIONS /recordings HTTP/1.1\r\nOrigin: https://fung-seven.vercel.app\r\n",
            )
            .unwrap(),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(
            preflight.status, "204 No Content",
            "preflight carries no token and must not 401"
        );

        let post = route(
            &parse_request("POST /recordings HTTP/1.1\r\nAuthorization: Bearer secret-token\r\n")
                .unwrap(),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(
            post.status, "404 Not Found",
            "POST exists for /recordings/import only"
        );
        let put = route(
            &parse_request("PUT /recordings HTTP/1.1\r\nAuthorization: Bearer secret-token\r\n")
                .unwrap(),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(put.status, "405 Method Not Allowed");
    }

    #[test]
    fn lists_recordings_newest_first_with_channels_from_chunk_names() {
        let (dir, storage) = open_genesis();
        let project = dir.join("project");
        seed(&storage, &project);

        let value = list_recordings(&storage).unwrap();
        let list = value["recordings"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["id"], "rec-live", "newest first");
        assert_eq!(list[0]["projectName"], "Standup");
        assert_eq!(list[0]["durationMs"], 5000);
        assert_eq!(list[0]["channels"], json!(["mic", "system"]));
        assert_eq!(list[0]["chunkCount"], 3);
        assert_eq!(list[1]["id"], "rec-import");
        assert_eq!(list[1]["channels"], json!(["file"]));
    }

    #[test]
    fn stitches_a_channel_in_slice_order_regardless_of_row_order() {
        let (dir, storage) = open_genesis();
        let project = dir.join("project");
        seed(&storage, &project);

        // mic-00002 was inserted with a higher sequence_no than system-00001
        // and before mic-00001; the filename ordinal is what orders slices.
        let audio = build_audio(&storage, "rec-live", Some("mic")).unwrap();
        assert_eq!(audio.mime, "audio/wav");
        assert_eq!(audio.missing_chunks, 0);
        let reader = hound::WavReader::new(Cursor::new(&audio.bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        let samples: Vec<i16> = reader.into_samples::<i16>().map(Result::unwrap).collect();
        assert_eq!(samples, vec![1, 2, 3, 4, 5]);

        let system = build_audio(&storage, "rec-live", Some("system")).unwrap();
        let samples: Vec<i16> = hound::WavReader::new(Cursor::new(&system.bytes))
            .unwrap()
            .into_samples::<i16>()
            .map(Result::unwrap)
            .collect();
        assert_eq!(samples, vec![9, 9, 9, 9]);

        // No channel asked for: the first available (mic) is served.
        let default = build_audio(&storage, "rec-live", None).unwrap();
        assert_eq!(default.bytes, audio.bytes);
    }

    #[test]
    fn whole_file_imports_are_served_as_the_file_with_their_mime() {
        let (dir, storage) = open_genesis();
        let project = dir.join("project");
        seed(&storage, &project);

        let audio = build_audio(&storage, "rec-import", None).unwrap();
        assert_eq!(audio.mime, "audio/mp4");
        assert_eq!(audio.bytes, b"not really aac");
    }

    #[test]
    fn missing_recording_channel_or_chunks_are_not_found_not_panics() {
        let (dir, storage) = open_genesis();
        let project = dir.join("project");
        seed(&storage, &project);

        assert!(matches!(
            build_audio(&storage, "nope", None),
            Err(AudioError::NotFound(_))
        ));
        assert!(matches!(
            build_audio(&storage, "rec-import", Some("mic")),
            Err(AudioError::NotFound(_))
        ));

        // A ledger row whose path climbs out of the project is treated as
        // absent, never followed.
        genesis_adapter::commit_rows(
            &storage,
            vec![
                genesis_adapter::upsert(
                    "recordings",
                    json!({"id": "rec-evil", "project_id": "p1", "source": "microphone", "input_path": null, "canonical_audio_path": "m", "status": "completed", "duration_ms": 1, "created_at": "2026-09-13T11:00:00Z", "updated_at": "t"}),
                ),
                chunk_row("c-evil", "rec-evil", 1, Path::new("../../../live/x/chunks/mic-00001.wav")),
            ],
        )
        .unwrap();
        match build_audio(&storage, "rec-evil", Some("mic")) {
            Err(AudioError::NotFound(detail)) => assert!(detail.contains("none of the 1 chunks")),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn missing_slices_are_skipped_and_counted() {
        let (dir, storage) = open_genesis();
        let project = dir.join("project");
        seed(&storage, &project);
        fs::remove_file(project.join("live/rec-live/chunks/mic-00001.wav")).unwrap();

        let audio = build_audio(&storage, "rec-live", Some("mic")).unwrap();
        assert_eq!(audio.missing_chunks, 1);
        let samples: Vec<i16> = hound::WavReader::new(Cursor::new(&audio.bytes))
            .unwrap()
            .into_samples::<i16>()
            .map(Result::unwrap)
            .collect();
        assert_eq!(samples, vec![4, 5]);

        let response = route(
            &get(
                "/recordings/rec-live/audio?channel=mic&token=secret-token",
                &[],
            ),
            &storage,
            &dir,
            &control(),
            None,
        );
        assert_eq!(response.status, "200 OK");
        assert!(response
            .extra_headers
            .contains(&("X-Fung-Missing-Chunks".to_string(), "1".to_string())));
    }

    #[test]
    fn audio_route_honours_range_requests() {
        let (dir, storage) = open_genesis();
        let project = dir.join("project");
        seed(&storage, &project);
        let control = control();

        let full = route(
            &get("/recordings/rec-import/audio?token=secret-token", &[]),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(full.status, "200 OK");
        assert_eq!(full.content_type, "audio/mp4");
        assert_eq!(full.body, b"not really aac");
        assert!(full
            .extra_headers
            .contains(&("Accept-Ranges".to_string(), "bytes".to_string())));

        let partial = route(
            &get(
                "/recordings/rec-import/audio?token=secret-token",
                &[("Range", "bytes=4-9")],
            ),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(partial.status, "206 Partial Content");
        assert_eq!(partial.body, b"really");
        assert!(partial
            .extra_headers
            .contains(&("Content-Range".to_string(), "bytes 4-9/14".to_string())));

        let bad = route(
            &get(
                "/recordings/rec-import/audio?token=secret-token",
                &[("Range", "bytes=50-")],
            ),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(bad.status, "416 Range Not Satisfiable");
        assert!(bad
            .extra_headers
            .contains(&("Content-Range".to_string(), "bytes */14".to_string())));
    }

    #[test]
    fn audio_route_rejects_nested_or_empty_ids() {
        let (dir, storage) = open_genesis();
        let control = control();
        for path in [
            "/recordings//audio",
            "/recordings/a/b/audio",
            "/recordings/a/audio/",
        ] {
            let response = route(
                &get(&format!("{path}?token=secret-token"), &[]),
                &storage,
                &dir,
                &control,
                None,
            );
            assert_eq!(response.status, "404 Not Found", "{path}");
        }
    }

    #[test]
    fn connect_url_carries_the_token_in_the_fragment() {
        let control = LocalApiControl {
            bind: "127.0.0.1:4321".to_string(),
            token: "abc".to_string(),
            lan: None,
        };
        assert_eq!(control.connect_url(), "http://127.0.0.1:4321/#abc");
    }

    /// Accepts exactly one connection and runs the production handler on it,
    /// so the framing a browser sees (head reading, `Content-Length`, CORS
    /// headers) is exercised over a real socket, not just the router.
    fn spawn_once(storage: Arc<Storage>, dir: PathBuf, control: LocalApiControl) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let bind = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_stream(stream, &storage, &dir, &control, None);
        });
        bind
    }

    fn raw_request(bind: &str, head: &str) -> String {
        let mut stream = TcpStream::connect(bind).unwrap();
        stream.write_all(head.as_bytes()).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        String::from_utf8_lossy(&response).into_owned()
    }

    #[test]
    fn serves_health_over_a_real_socket_with_an_exact_content_length() {
        let (dir, storage) = open_genesis();
        let bind = spawn_once(Arc::new(storage), dir, control());
        let response = raw_request(&bind, "GET /health HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(body.len(), length);
        assert!(body.contains("\"app\":\"FUNG\""));
        assert!(
            !head.contains("Access-Control-Allow-Origin"),
            "no Origin on the request, no CORS grant on the response"
        );
    }

    #[test]
    fn cors_grant_follows_the_origin_allowlist_over_a_real_socket() {
        let (dir, storage) = open_genesis();
        let storage = Arc::new(storage);

        // A browser preflight from the production web origin: no token yet,
        // must be 204 with the grant and the private-network allowance.
        let bind = spawn_once(storage.clone(), dir.clone(), control());
        let response = raw_request(
            &bind,
            "OPTIONS /recordings HTTP/1.1\r\nHost: x\r\nOrigin: https://fung-seven.vercel.app\r\nAccess-Control-Request-Method: GET\r\nAccess-Control-Request-Headers: authorization\r\nAccess-Control-Request-Private-Network: true\r\n\r\n",
        );
        assert!(
            response.starts_with("HTTP/1.1 204 No Content\r\n"),
            "{response}"
        );
        assert!(response.contains("Access-Control-Allow-Origin: https://fung-seven.vercel.app\r\n"));
        assert!(response.contains(
            "Access-Control-Allow-Headers: Authorization, Range, Content-Type, X-Fung-Filename\r\n"
        ));
        assert!(response.contains("Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n"));
        assert!(response.contains("Access-Control-Allow-Private-Network: true\r\n"));
        assert!(response.contains("Content-Length: 0\r\n"));

        // A disallowed origin with a valid token: the server still answers
        // (the token is the authorisation), but grants no CORS read, which
        // is what keeps another page in the same browser from seeing it.
        let bind = spawn_once(storage, dir, control());
        let response = raw_request(
            &bind,
            "GET /recordings HTTP/1.1\r\nHost: x\r\nOrigin: https://evil.example\r\nAuthorization: Bearer secret-token\r\n\r\n",
        );
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        assert!(response.contains("\"recordings\":[]"));
        assert!(
            !response.contains("Access-Control-Allow-Origin"),
            "a disallowed origin must get no CORS grant: {response}"
        );
    }

    #[test]
    fn root_serves_the_phone_page_without_the_token_in_it() {
        let (dir, storage) = open_genesis();
        let control = control();
        for path in ["/", "/index.html"] {
            let response = route(&get(path, &[]), &storage, &dir, &control, None);
            assert_eq!(response.status, "200 OK", "{path}");
            assert!(response.content_type.starts_with("text/html"));
            let body = String::from_utf8(response.body).unwrap();
            assert!(
                body.contains("ไฟล์ที่อัดไว้"),
                "the page is the recordings player"
            );
            assert!(
                body.contains("/recordings"),
                "the page talks to this origin's API"
            );
            assert!(
                !body.contains("secret-token"),
                "the page must never embed the token"
            );
            // The only URL-looking text is the paste placeholder; no script,
            // style, image, or fetch target may point off this origin.
            for needle in [
                "src=\"http",
                "href=\"http",
                "fetch(\"http",
                "@import",
                "url(",
            ] {
                assert!(
                    !body.contains(needle),
                    "the page must fetch nothing off-origin: {needle}"
                );
            }
            assert!(
                body.contains("connect-src 'self'"),
                "the page pins itself to its own origin with a CSP"
            );
            assert!(response
                .extra_headers
                .contains(&("X-Content-Type-Options".to_string(), "nosniff".to_string())));
        }
    }

    #[test]
    fn lan_url_needs_a_lan_listener_and_carries_its_port() {
        let mut control = control();
        assert_eq!(control.lan_url(), None, "off by default");
        control.lan = Some(LanShare {
            bind: "0.0.0.0:45678".to_string(),
            stop: Arc::new(AtomicBool::new(false)),
        });
        match control.lan_url() {
            // Runners without a default route have no LAN address; the UI
            // handles `None` explicitly, so both outcomes are legitimate.
            None => assert!(crate::primary_lan_ipv4().is_none()),
            Some(url) => {
                assert!(url.starts_with("http://"), "{url}");
                assert!(url.ends_with(":45678/#secret-token"), "{url}");
                assert!(
                    !url.contains("0.0.0.0"),
                    "the wildcard bind is not an address to dial"
                );
            }
        }
    }

    #[test]
    fn a_stopped_listener_releases_its_port() {
        let (dir, storage) = open_genesis();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let bind = listener.local_addr().unwrap().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        spawn_listener(
            listener,
            Arc::new(storage),
            dir,
            Arc::new(control()),
            None,
            stop.clone(),
        )
        .unwrap();

        let response = raw_request(&bind, "GET /health HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");

        stop.store(true, Ordering::SeqCst);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if TcpStream::connect(&bind).is_err() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "listener still accepting 3s after stop"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn post(path: &str, headers: &[(&str, &str)], body: &[u8]) -> Request {
        let mut head = format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        let mut request = parse_request(&head).unwrap();
        request.body = body.to_vec();
        request
    }

    fn import_host(storage: Arc<Storage>, data_root: PathBuf) -> ImportHost {
        let recording_output = RecordingOutputManager::load(data_root.clone(), data_root.clone())
            .expect("test recording output manager");
        ImportHost {
            genesis: storage,
            recording_output: Arc::new(Mutex::new(recording_output)),
            runtime: WhisperRuntime {
                python: PathBuf::from("python-that-does-not-exist"),
                script: PathBuf::from("transcribe.py"),
                cuda_bin: PathBuf::from("cuda"),
            },
        }
    }

    #[test]
    fn upload_metadata_is_derived_defensively() {
        assert_eq!(upload_extension(Some("audio/webm;codecs=opus")), "webm");
        assert_eq!(upload_extension(Some("audio/mp4")), "m4a");
        assert_eq!(upload_extension(Some("AUDIO/WAV")), "wav");
        assert_eq!(upload_extension(None), "bin");
        assert_eq!(upload_extension(Some("text/html")), "bin");

        assert_eq!(
            upload_project_name(Some("fung-web-2026-09-16-1203.webm")),
            "fung-web-2026-09-16-1203"
        );
        assert_eq!(
            upload_project_name(Some("..\\..\\evil<script>.webm")),
            "evilscript"
        );
        assert!(upload_project_name(None).starts_with("Web recording "));
        assert!(upload_project_name(Some("   ")).starts_with("Web recording "));
    }

    #[test]
    fn upload_needs_the_token_the_right_path_and_a_runtime() {
        let (dir, storage) = open_genesis();
        let control = control();
        let denied = route(
            &post("/recordings/import", &[], b"abc"),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(denied.status, "401 Unauthorized");

        let elsewhere = route(
            &post(
                "/recordings",
                &[("Authorization", "Bearer secret-token")],
                b"abc",
            ),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(
            elsewhere.status, "404 Not Found",
            "only /recordings/import takes a POST"
        );

        let no_runtime = route(
            &post(
                "/recordings/import",
                &[("Authorization", "Bearer secret-token")],
                b"abc",
            ),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(no_runtime.status, "503 Service Unavailable");
    }

    /// The whole browser → desktop hand-off short of the Whisper worker
    /// itself: the bytes land under the data root, a project and a running
    /// job exist in the ledger, the `202` names the recording the worker
    /// will write, and `/jobs/{id}` can be polled for it. The worker is
    /// pointed at a python that does not exist, so the job settles as
    /// `failed` rather than hanging the test on a real transcription.
    #[test]
    fn upload_takes_custody_creates_a_job_and_is_pollable() {
        let (dir, storage) = open_genesis();
        let storage = Arc::new(storage);
        let data_root = dir.join("data");
        let host = import_host(storage.clone(), data_root.clone());
        let control = control();

        let accepted = route(
            &post(
                "/recordings/import",
                &[
                    ("Authorization", "Bearer secret-token"),
                    ("Content-Type", "audio/webm;codecs=opus"),
                    ("X-Fung-Filename", "fung-web-2026-09-16-1203.webm"),
                ],
                b"\x1aE\xdf\xa3 not really webm",
            ),
            &storage,
            &dir,
            &control,
            Some(&host),
        );
        assert_eq!(
            accepted.status,
            "202 Accepted",
            "{}",
            String::from_utf8_lossy(&accepted.body)
        );
        let body: Value = serde_json::from_slice(&accepted.body).unwrap();
        let job_id = body["jobId"].as_str().unwrap().to_string();
        let project_id = body["projectId"].as_str().unwrap().to_string();
        let recording_id = body["recordingId"].as_str().unwrap().to_string();
        assert_eq!(body["extension"], "webm");

        let stored = data_root
            .join("imports")
            .join("web")
            .join(format!("{recording_id}.webm"));
        assert!(
            stored.is_file(),
            "upload must be written under the data root"
        );
        assert_eq!(fs::read(&stored).unwrap(), b"\x1aE\xdf\xa3 not really webm");

        let projects = genesis_adapter::query(
            &storage,
            "projects",
            &["name"],
            vec![genesis_adapter::eq("projects", "id", json!(project_id))],
            1,
        )
        .unwrap();
        assert_eq!(
            str_col(&projects[0], "projects", "name").as_deref(),
            Some("fung-web-2026-09-16-1203")
        );

        // Poll until the worker (with no python) settles the job.
        let mut last = String::new();
        for _ in 0..100 {
            let polled = route(
                &get(&format!("/jobs/{job_id}?token=secret-token"), &[]),
                &storage,
                &dir,
                &control,
                Some(&host),
            );
            assert_eq!(polled.status, "200 OK");
            let job: Value = serde_json::from_slice(&polled.body).unwrap();
            last = job["job"]["status"].as_str().unwrap_or("").to_string();
            if last == "failed" || last == "completed" {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            last, "failed",
            "no python on this host, so the job must fail — not hang"
        );

        let missing = route(
            &get("/jobs/nope?token=secret-token", &[]),
            &storage,
            &dir,
            &control,
            Some(&host),
        );
        assert_eq!(missing.status, "404 Not Found");
    }

    #[test]
    fn transcript_route_resolves_the_project_and_404s_the_unknown() {
        let (dir, storage) = open_genesis();
        let project_dir = dir.join("project");
        seed(&storage, &project_dir);
        let control = control();

        let ok = route(
            &get("/recordings/rec-live/transcript?token=secret-token", &[]),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(ok.status, "200 OK", "{}", String::from_utf8_lossy(&ok.body));
        let body: Value = serde_json::from_slice(&ok.body).unwrap();
        assert_eq!(body["projectId"], "p1");
        assert_eq!(body["recordingId"], "rec-live");
        assert!(body["transcript"]["segments"]
            .as_array()
            .unwrap()
            .is_empty());

        let missing = route(
            &get("/recordings/nope/transcript?token=secret-token", &[]),
            &storage,
            &dir,
            &control,
            None,
        );
        assert_eq!(missing.status, "404 Not Found");
    }

    /// Body framing over a real socket: the head and the first body bytes
    /// arrive in one read, the rest in later ones, and `Content-Length`
    /// decides where the body ends. Without a runtime the route answers
    /// `503`, which is enough to prove the body was read and parsed.
    #[test]
    fn upload_body_is_read_to_content_length_over_a_real_socket() {
        let (dir, storage) = open_genesis();
        let bind = spawn_once(Arc::new(storage), dir, control());
        let body = vec![b'x'; 10_000];
        let mut stream = TcpStream::connect(&bind).unwrap();
        let head = format!(
            "POST /recordings/import HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer secret-token\r\nContent-Type: audio/webm\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(&body[..100]).unwrap();
        thread::sleep(Duration::from_millis(50));
        stream.write_all(&body[100..]).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let response = String::from_utf8_lossy(&response);
        assert!(
            response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"),
            "{response}"
        );

        let (dir, storage) = open_genesis();
        let bind = spawn_once(Arc::new(storage), dir, control());
        let oversized = raw_request(
            &bind,
            &format!(
                "POST /recordings/import HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer secret-token\r\nContent-Length: {}\r\n\r\n",
                MAX_UPLOAD_BYTES + 1
            ),
        );
        assert!(
            oversized.starts_with("HTTP/1.1 413 Payload Too Large\r\n"),
            "{oversized}"
        );
    }

    /// The real thing, end to end below the browser: the repo's staged
    /// `.venv-whisper` runtime transcribes an uploaded WAV through the same
    /// route → pipeline → transcript path the web dashboard uses. Needs the
    /// runtime staged on this machine (`scripts/stage_whisper_runtime.ps1`)
    /// and a speech fixture in `FUNG_UPLOAD_FIXTURE`, so it is opt-in:
    /// `cargo test --lib upload_transcribes_with_the_real_runtime -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn upload_transcribes_with_the_real_runtime() {
        let fixture = std::env::var("FUNG_UPLOAD_FIXTURE")
            .expect("FUNG_UPLOAD_FIXTURE=<path to a speech wav>");
        let root = crate::source_root();
        let runtime = WhisperRuntime {
            python: root
                .join(".venv-whisper")
                .join("Scripts")
                .join("python.exe"),
            script: root.join("scripts").join("transcribe.py"),
            cuda_bin: root.join("runtime").join("cuda12").join("bin"),
        };
        assert!(
            runtime.python.is_file(),
            "stage the whisper runtime first: {}",
            runtime.python.display()
        );

        let (dir, storage) = open_genesis();
        let storage = Arc::new(storage);
        let data_root = dir.join("data");
        let recording_output = RecordingOutputManager::load(data_root.clone(), data_root.clone())
            .expect("test recording output manager");
        let host = ImportHost {
            genesis: storage.clone(),
            recording_output: Arc::new(Mutex::new(recording_output)),
            runtime,
        };
        let control = control();
        let bytes = fs::read(&fixture).expect("read fixture");
        let content_type = if fixture.to_ascii_lowercase().ends_with(".wav") {
            "audio/wav"
        } else {
            "audio/webm"
        };

        let accepted = route(
            &post(
                "/recordings/import",
                &[
                    ("Authorization", "Bearer secret-token"),
                    ("Content-Type", content_type),
                    ("X-Fung-Filename", "real-runtime-check.wav"),
                ],
                &bytes,
            ),
            &storage,
            &dir,
            &control,
            Some(&host),
        );
        assert_eq!(
            accepted.status,
            "202 Accepted",
            "{}",
            String::from_utf8_lossy(&accepted.body)
        );
        let receipt: Value = serde_json::from_slice(&accepted.body).unwrap();
        let job_id = receipt["jobId"].as_str().unwrap().to_string();
        let recording_id = receipt["recordingId"].as_str().unwrap().to_string();

        let started = std::time::Instant::now();
        let mut status = String::new();
        let mut job = Value::Null;
        while started.elapsed() < Duration::from_secs(300) {
            let polled = route(
                &get(&format!("/jobs/{job_id}?token=secret-token"), &[]),
                &storage,
                &dir,
                &control,
                Some(&host),
            );
            job = serde_json::from_slice(&polled.body).unwrap();
            status = job["job"]["status"].as_str().unwrap_or("").to_string();
            if status == "completed" || status == "failed" {
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
        assert_eq!(status, "completed", "job did not complete: {job}");

        let transcript = route(
            &get(
                &format!("/recordings/{recording_id}/transcript?token=secret-token"),
                &[],
            ),
            &storage,
            &dir,
            &control,
            Some(&host),
        );
        assert_eq!(
            transcript.status,
            "200 OK",
            "{}",
            String::from_utf8_lossy(&transcript.body)
        );
        let body: Value = serde_json::from_slice(&transcript.body).unwrap();
        let segments = body["transcript"]["segments"].as_array().unwrap();
        assert!(
            !segments.is_empty(),
            "the fixture has speech; expected at least one segment"
        );
        let text: Vec<String> = segments
            .iter()
            .filter_map(|segment| segment["text"].as_str().map(str::to_string))
            .collect();
        eprintln!(
            "real-runtime upload transcribed in {:.1}s: {}",
            started.elapsed().as_secs_f64(),
            text.join(" | ")
        );

        // The import is now a first-class desktop recording too.
        let listed = route(
            &get("/recordings?token=secret-token", &[]),
            &storage,
            &dir,
            &control,
            Some(&host),
        );
        let listed: Value = serde_json::from_slice(&listed.body).unwrap();
        assert!(listed["recordings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|recording| recording["id"] == recording_id));
    }
}
