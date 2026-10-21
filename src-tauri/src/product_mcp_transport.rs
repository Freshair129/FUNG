//! Bounded, loopback-only Streamable HTTP transport for the inbound Product MCP server.
//!
//! This module owns HTTP framing, MCP transport validation, per-grant request
//! limits, and listener lifecycle. Authentication and JSON-RPC/tool dispatch
//! are supplied by the application. In particular, `clientInfo` is untrusted
//! request data and is never used as a grant or rate-limit identity.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MCP_PROTOCOL_VERSION: &str = "2026-07-28";
const MCP_PATH: &str = "/mcp";
const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
const MAX_IN_FLIGHT: usize = 8;
const MAX_RATE_BUCKETS: usize = 4096;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(3);
const RATE_BUCKET_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Listener configuration. The address is always IPv4 loopback; `port` is
/// bound exactly as requested (`0` is useful for isolated tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductMcpConfig {
    pub(crate) port: u16,
    pub(crate) rate_limits: RateLimitConfig,
}

impl Default for ProductMcpConfig {
    fn default() -> Self {
        Self {
            port: 49_175,
            rate_limits: RateLimitConfig::default(),
        }
    }
}

/// Authenticated caller identity returned by the application's authenticator.
/// Construct this only from the verified local grant record, never from
/// `params._meta.clientInfo` or another client-supplied field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrantIdentity {
    pub(crate) grant_id: String,
}

impl GrantIdentity {
    pub(crate) fn new(grant_id: impl Into<String>) -> Option<Self> {
        let grant_id = grant_id.into();
        if grant_id.is_empty() || grant_id.len() > 256 || grant_id.contains('\0') {
            return None;
        }
        Some(Self { grant_id })
    }
}

/// One fully framed and transport-validated JSON-RPC request or notification.
/// `rpc` and `params` still contain untrusted client input.
#[derive(Debug, Clone)]
pub(crate) struct ParsedRequest {
    pub(crate) request_id: Option<Value>,
    pub(crate) notification: bool,
    pub(crate) method: String,
    pub(crate) params: Value,
    pub(crate) tool_name: Option<String>,
    pub(crate) authorization: Option<String>,
}

/// Reply returned by the caller's authenticator or JSON-RPC/tool dispatcher.
/// The transport owns response framing and does not permit arbitrary headers.
#[derive(Debug, Clone)]
pub(crate) struct HandlerReply {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
}

impl HandlerReply {
    pub(crate) fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: Some(body),
        }
    }

    #[cfg(test)]
    pub(crate) fn empty(status: u16) -> Self {
        Self { status, body: None }
    }
}

pub(crate) type Authenticator =
    Arc<dyn Fn(&ParsedRequest) -> Result<GrantIdentity, HandlerReply> + Send + Sync + 'static>;
pub(crate) type RequestHandler =
    Arc<dyn Fn(&ParsedRequest, &GrantIdentity) -> HandlerReply + Send + Sync + 'static>;

/// General token bucket applies to every authenticated grant. The second
/// bucket additionally limits `tools/call` for `fung.job.control`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RateLimitConfig {
    pub(crate) requests_per_minute: u32,
    pub(crate) burst: u32,
    pub(crate) job_control_per_minute: u32,
    pub(crate) job_control_burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            burst: 20,
            job_control_per_minute: 5,
            job_control_burst: 5,
        }
    }
}

impl RateLimitConfig {
    fn valid(self) -> bool {
        self.requests_per_minute > 0
            && self.burst > 0
            && self.job_control_per_minute > 0
            && self.job_control_burst > 0
    }
}

struct BucketState {
    tokens: f64,
    updated_at: Instant,
}

struct TokenBuckets {
    config: RateLimitConfig,
    general: Mutex<HashMap<String, BucketState>>,
    job_control: Mutex<HashMap<String, BucketState>>,
}

impl TokenBuckets {
    fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            general: Mutex::new(HashMap::new()),
            job_control: Mutex::new(HashMap::new()),
        }
    }

    fn check(&self, grant_id: &str, is_job_control: bool) -> Result<(), u64> {
        if let Err(retry) = consume_bucket(
            &self.general,
            grant_id,
            self.config.burst,
            self.config.requests_per_minute,
        ) {
            return Err(retry);
        }
        if is_job_control {
            consume_bucket(
                &self.job_control,
                grant_id,
                self.config.job_control_burst,
                self.config.job_control_per_minute,
            )?;
        }
        Ok(())
    }
}

fn consume_bucket(
    buckets: &Mutex<HashMap<String, BucketState>>,
    key: &str,
    capacity: u32,
    refill_per_minute: u32,
) -> Result<(), u64> {
    let now = Instant::now();
    let mut buckets = buckets
        .lock()
        .expect("Product MCP rate-limit mutex poisoned");
    if !buckets.contains_key(key) && buckets.len() >= MAX_RATE_BUCKETS {
        buckets.retain(|_, bucket| {
            now.saturating_duration_since(bucket.updated_at) < RATE_BUCKET_IDLE_TIMEOUT
        });
        if buckets.len() >= MAX_RATE_BUCKETS {
            // Fail closed if every retained entry is still active rather than
            // growing memory in response to a valid but unbounded grant set.
            return Err(60);
        }
    }
    let bucket = buckets.entry(key.to_owned()).or_insert(BucketState {
        tokens: f64::from(capacity),
        updated_at: now,
    });
    let elapsed = now
        .saturating_duration_since(bucket.updated_at)
        .as_secs_f64();
    let refill_per_second = f64::from(refill_per_minute) / 60.0;
    bucket.tokens = (bucket.tokens + elapsed * refill_per_second).min(f64::from(capacity));
    bucket.updated_at = now;
    if bucket.tokens >= 1.0 {
        bucket.tokens -= 1.0;
        Ok(())
    } else {
        let wait_seconds = (1.0 - bucket.tokens) / refill_per_second;
        Err(wait_seconds.ceil().max(1.0) as u64)
    }
}

/// A running Product MCP listener. `stop` waits for the accept loop and
/// active request workers to finish; request socket reads are time-bounded.
pub(crate) struct ProductMcpServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
}

impl ProductMcpServer {
    pub(crate) fn start<A, H>(
        config: ProductMcpConfig,
        authenticate: A,
        handler: H,
    ) -> io::Result<Self>
    where
        A: Fn(&ParsedRequest) -> Result<GrantIdentity, HandlerReply> + Send + Sync + 'static,
        H: Fn(&ParsedRequest, &GrantIdentity) -> HandlerReply + Send + Sync + 'static,
    {
        if !config.rate_limits.valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Product MCP rate limits must be nonzero",
            ));
        }

        let listener = TcpListener::bind(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            config.port,
        ))?;
        let addr = listener.local_addr()?;
        if addr.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
            return Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "Product MCP must bind IPv4 loopback only",
            ));
        }
        listener.set_nonblocking(true)?;

        let stop = Arc::new(AtomicBool::new(false));
        let listener_stop = Arc::clone(&stop);
        let authenticate: Authenticator = Arc::new(authenticate);
        let handler: RequestHandler = Arc::new(handler);
        let buckets = Arc::new(TokenBuckets::new(config.rate_limits));
        let workers = Arc::new(Mutex::new(Vec::<JoinHandle<()>>::new()));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let accept_thread = thread::Builder::new()
            .name("fung-product-mcp-accept".to_owned())
            .spawn(move || {
                accept_loop(
                    listener,
                    addr,
                    listener_stop,
                    authenticate,
                    handler,
                    buckets,
                    Arc::clone(&workers),
                    Arc::clone(&in_flight),
                );
            })?;

        Ok(Self {
            addr,
            stop,
            accept_thread: Some(accept_thread),
        })
    }

    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub(crate) fn stop(&mut self) {
        if self.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(accept_thread) = self.accept_thread.take() {
            let _ = accept_thread.join();
        }
    }
}

impl Drop for ProductMcpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn accept_loop(
    listener: TcpListener,
    local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    authenticate: Authenticator,
    handler: RequestHandler,
    buckets: Arc<TokenBuckets>,
    workers: Arc<Mutex<Vec<JoinHandle<()>>>>,
    in_flight: Arc<AtomicUsize>,
) {
    while !stop.load(Ordering::SeqCst) {
        reap_finished_workers(&workers);
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if peer.ip().is_loopback() {
                    let count = in_flight.load(Ordering::SeqCst);
                    if count >= MAX_IN_FLIGHT {
                        // This request is rejected before dispatch. Parse it
                        // synchronously under bounded socket deadlines so a
                        // rate-limit reply can echo its JSON-RPC request ID.
                        let _ = stream.set_read_timeout(Some(SOCKET_TIMEOUT));
                        let _ = stream.set_write_timeout(Some(SOCKET_TIMEOUT));
                        let reply = handle_at_capacity(&mut stream, local_addr, &authenticate);
                        write_reply(&mut stream, reply);
                        continue;
                    }
                    if reserve_slot(&in_flight).is_err() {
                        continue;
                    }
                    let auth = Arc::clone(&authenticate);
                    let dispatch = Arc::clone(&handler);
                    let rate_buckets = Arc::clone(&buckets);
                    let worker_count = Arc::clone(&in_flight);
                    let worker = thread::Builder::new()
                        .name("fung-product-mcp-request".to_owned())
                        .spawn(move || {
                            let _slot = SlotGuard(worker_count);
                            let _ = stream.set_read_timeout(Some(SOCKET_TIMEOUT));
                            let _ = stream.set_write_timeout(Some(SOCKET_TIMEOUT));
                            let reply = handle_connection(
                                &mut stream,
                                peer,
                                local_addr,
                                &auth,
                                &dispatch,
                                &rate_buckets,
                            );
                            write_reply(&mut stream, reply);
                        });
                    match worker {
                        Ok(worker) => workers
                            .lock()
                            .expect("Product MCP worker mutex poisoned")
                            .push(worker),
                        Err(_) => {
                            in_flight.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                } else {
                    write_reply(
                        &mut stream,
                        HttpReply::json(
                            403,
                            json_rpc_error(None, -32000, "Loopback clients only", None),
                        ),
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(_) => thread::sleep(ACCEPT_POLL_INTERVAL),
        }
    }

    let remaining = {
        let mut workers = workers.lock().expect("Product MCP worker mutex poisoned");
        std::mem::take(&mut *workers)
    };
    for worker in remaining {
        let _ = worker.join();
    }
}

fn reap_finished_workers(workers: &Mutex<Vec<JoinHandle<()>>>) {
    let finished = {
        let mut workers = workers.lock().expect("Product MCP worker mutex poisoned");
        let mut finished = Vec::new();
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                finished.push(workers.swap_remove(index));
            } else {
                index += 1;
            }
        }
        finished
    };
    for worker in finished {
        let _ = worker.join();
    }
}

struct SlotGuard(Arc<AtomicUsize>);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn reserve_slot(in_flight: &AtomicUsize) -> Result<(), ()> {
    let mut current = in_flight.load(Ordering::SeqCst);
    loop {
        if current >= MAX_IN_FLIGHT {
            return Err(());
        }
        match in_flight.compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(()),
            Err(actual) => current = actual,
        }
    }
}

fn handle_at_capacity(
    stream: &mut TcpStream,
    local_addr: SocketAddr,
    authenticate: &Authenticator,
) -> HttpReply {
    match read_and_validate_request(stream, local_addr) {
        Ok(request) => match authenticate(&request) {
            Ok(_) => rate_limit_reply(&request, 1),
            Err(reply) => HttpReply::from_handler(reply),
        },
        Err(failure) => failure.into_reply(),
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    peer: SocketAddr,
    local_addr: SocketAddr,
    authenticate: &Authenticator,
    handler: &RequestHandler,
    buckets: &TokenBuckets,
) -> HttpReply {
    if !peer.ip().is_loopback() {
        return HttpReply::json(
            403,
            json_rpc_error(None, -32000, "Loopback clients only", None),
        );
    }
    let request = match read_and_validate_request(stream, local_addr) {
        Ok(request) => request,
        Err(failure) => return failure.into_reply(),
    };
    let grant = match authenticate(&request) {
        Ok(grant) => grant,
        Err(reply) => return HttpReply::from_handler(reply),
    };
    let job_control =
        request.method == "tools/call" && request.tool_name.as_deref() == Some("fung.job.control");
    if let Err(retry_after) = buckets.check(&grant.grant_id, job_control) {
        return rate_limit_reply(&request, retry_after);
    }

    let reply = handler(&request, &grant);
    if request.notification && (200..300).contains(&reply.status) {
        HttpReply::empty(202)
    } else {
        HttpReply::from_handler(reply)
    }
}

fn rate_limit_reply(request: &ParsedRequest, retry_after_seconds: u64) -> HttpReply {
    HttpReply {
        status: 429,
        body: Some(json_rpc_error(
            request.request_id.clone(),
            -32000,
            "Rate limit exceeded",
            Some(retry_after_seconds),
        )),
        retry_after_seconds: Some(retry_after_seconds),
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_and_validate_request(
    stream: &mut TcpStream,
    local_addr: SocketAddr,
) -> Result<ParsedRequest, RequestFailure> {
    let mut bounded_reader = DeadlineReader::new(stream, SOCKET_TIMEOUT);
    let request = read_http_request(&mut bounded_reader)?;
    if request.method != "POST" {
        let status = if matches!(request.method.as_str(), "GET" | "DELETE") {
            405
        } else {
            405
        };
        return Err(RequestFailure::http(
            status,
            -32000,
            "HTTP method not allowed",
        ));
    }
    if request.target != MCP_PATH {
        return Err(RequestFailure::http(404, -32000, "MCP endpoint not found"));
    }
    let host = request
        .headers
        .get("host")
        .ok_or_else(|| RequestFailure::http(400, -32000, "Host header required"))?;
    if !authority_is_loopback(host, local_addr.port()) {
        return Err(RequestFailure::http(403, -32000, "Loopback Host required"));
    }
    if let Some(origin) = request.headers.get("origin") {
        if !origin_is_loopback(origin) {
            return Err(RequestFailure::http(
                403,
                -32000,
                "Loopback Origin required",
            ));
        }
    }
    let content_type = request
        .headers
        .get("content-type")
        .ok_or_else(|| RequestFailure::http(415, -32000, "application/json required"))?;
    if !is_application_json(content_type) {
        return Err(RequestFailure::http(
            415,
            -32000,
            "application/json required",
        ));
    }
    let accept = request
        .headers
        .get("accept")
        .ok_or_else(|| RequestFailure::http(406, -32000, "MCP Accept media types required"))?;
    if !accept_has_mcp_media_types(accept) {
        return Err(RequestFailure::http(
            406,
            -32000,
            "MCP Accept media types required",
        ));
    }

    let rpc: Value = serde_json::from_slice(&request.body)
        .map_err(|_| RequestFailure::json(400, Some(Value::Null), -32700, "Parse error"))?;
    let mut parsed = parse_json_rpc(rpc)?;
    parsed.authorization = request.headers.get("authorization").cloned();
    validate_mcp_headers(&request.headers, &parsed)?;
    Ok(parsed)
}

fn read_http_request<R: Read>(stream: &mut R) -> Result<HttpRequest, RequestFailure> {
    let max_frame = MAX_REQUEST_HEAD_BYTES + MAX_REQUEST_BODY_BYTES + 1;
    let mut bytes = Vec::with_capacity(4096);
    let mut header_end = None;
    let mut chunk = [0_u8; 8192];
    while header_end.is_none() {
        if bytes.len() >= MAX_REQUEST_HEAD_BYTES {
            return Err(RequestFailure::http(
                431,
                -32000,
                "Request headers too large",
            ));
        }
        let available = (MAX_REQUEST_HEAD_BYTES - bytes.len()).min(chunk.len());
        let read = stream
            .read(&mut chunk[..available])
            .map_err(|error| RequestFailure::io(&error))?;
        if read == 0 {
            return Err(RequestFailure::http(400, -32000, "Incomplete HTTP request"));
        }
        bytes.extend_from_slice(&chunk[..read]);
        header_end = find_header_end(&bytes);
    }
    let header_end = header_end.expect("checked above");
    if header_end > MAX_REQUEST_HEAD_BYTES {
        return Err(RequestFailure::http(
            431,
            -32000,
            "Request headers too large",
        ));
    }
    let (method, target, headers, content_length) = parse_request_head(&bytes[..header_end])?;
    if content_length > MAX_REQUEST_BODY_BYTES {
        return Err(RequestFailure::http(413, -32000, "Request body too large"));
    }
    if bytes.len() > max_frame {
        return Err(RequestFailure::http(413, -32000, "Request body too large"));
    }
    let body_offset = header_end + 4;
    let mut body = if bytes.len() > body_offset {
        bytes[body_offset..bytes.len().min(body_offset + content_length)].to_vec()
    } else {
        Vec::with_capacity(content_length)
    };
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let chunk_limit = remaining.min(chunk.len());
        let read = stream
            .read(&mut chunk[..chunk_limit])
            .map_err(|error| RequestFailure::io(&error))?;
        if read == 0 {
            return Err(RequestFailure::http(400, -32000, "Incomplete HTTP body"));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(HttpRequest {
        method,
        target,
        headers,
        body,
    })
}

/// Applies one total deadline to the entire HTTP head and body, including
/// clients that trickle data often enough to avoid an ordinary idle timeout.
struct DeadlineReader<'a> {
    stream: &'a mut TcpStream,
    deadline: Instant,
}

impl<'a> DeadlineReader<'a> {
    fn new(stream: &'a mut TcpStream, budget: Duration) -> Self {
        Self {
            stream,
            deadline: Instant::now() + budget,
        }
    }
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Product MCP request deadline exceeded",
            ));
        }
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buffer)
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_request_head(
    bytes: &[u8],
) -> Result<(String, String, HashMap<String, String>, usize), RequestFailure> {
    let header = std::str::from_utf8(bytes)
        .map_err(|_| RequestFailure::http(400, -32000, "Invalid HTTP header encoding"))?;
    let mut lines = header.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| RequestFailure::http(400, -32000, "HTTP request line required"))?;
    let mut request_parts = request_line.split(' ');
    let method = request_parts.next().unwrap_or_default();
    let target = request_parts.next().unwrap_or_default();
    let version = request_parts.next().unwrap_or_default();
    if method.is_empty()
        || !method.bytes().all(is_http_token)
        || target.is_empty()
        || request_parts.next().is_some()
        || version != "HTTP/1.1"
        || target
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return Err(RequestFailure::http(
            400,
            -32000,
            "Malformed HTTP request line",
        ));
    }

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(RequestFailure::http(
                400,
                -32000,
                "Obsolete folded headers are not accepted",
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| RequestFailure::http(400, -32000, "Malformed HTTP header"))?;
        if name.is_empty() || !name.bytes().all(is_http_token) {
            return Err(RequestFailure::http(
                400,
                -32000,
                "Malformed HTTP header name",
            ));
        }
        let value = value.trim_matches([' ', '\t']);
        if value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(RequestFailure::http(
                400,
                -32000,
                "Malformed HTTP header value",
            ));
        }
        let key = name.to_ascii_lowercase();
        if headers.insert(key, value.to_owned()).is_some() {
            return Err(RequestFailure::http(400, -32000, "Duplicate HTTP header"));
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(RequestFailure::http(
            400,
            -32000,
            "Transfer-Encoding is not supported",
        ));
    }
    let content_length = headers
        .get("content-length")
        .ok_or_else(|| RequestFailure::http(411, -32000, "Content-Length required"))?
        .parse::<usize>()
        .map_err(|_| RequestFailure::http(400, -32000, "Invalid Content-Length"))?;
    Ok((
        method.to_owned(),
        target.to_owned(),
        headers,
        content_length,
    ))
}

fn is_http_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn parse_json_rpc(rpc: Value) -> Result<ParsedRequest, RequestFailure> {
    let Some(object) = rpc.as_object() else {
        return Err(RequestFailure::json(
            400,
            Some(Value::Null),
            -32600,
            "Invalid Request",
        ));
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || object.contains_key("result")
        || object.contains_key("error")
    {
        return Err(RequestFailure::json(
            400,
            object.get("id").cloned().or(Some(Value::Null)),
            -32600,
            "Invalid Request",
        ));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Err(RequestFailure::json(
            400,
            object.get("id").cloned().or(Some(Value::Null)),
            -32600,
            "Invalid Request",
        ));
    };
    if method.is_empty() {
        return Err(RequestFailure::json(
            400,
            object.get("id").cloned().or(Some(Value::Null)),
            -32600,
            "Invalid Request",
        ));
    }
    let request_id = object.get("id").cloned();
    if let Some(id) = &request_id {
        if !(id.is_string() || id.is_number() || id.is_null()) {
            return Err(RequestFailure::json(
                400,
                Some(Value::Null),
                -32600,
                "Invalid Request",
            ));
        }
    }
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() && !params.is_array() {
        return Err(RequestFailure::json(
            400,
            request_id.or(Some(Value::Null)),
            -32600,
            "Invalid Request",
        ));
    }
    let tool_name = if method == "tools/call" {
        params
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    } else {
        None
    };
    let method = method.to_owned();
    Ok(ParsedRequest {
        request_id: request_id.clone(),
        notification: request_id.is_none(),
        method,
        params,
        tool_name,
        authorization: None,
    })
}

fn validate_mcp_headers(
    headers: &HashMap<String, String>,
    request: &ParsedRequest,
) -> Result<(), RequestFailure> {
    let protocol_header = headers.get("mcp-protocol-version").ok_or_else(|| {
        RequestFailure::json(400, request.request_id.clone(), -32020, "HeaderMismatch")
    })?;
    let body_protocol = request
        .params
        .get("_meta")
        .and_then(|meta| meta.get("io.modelcontextprotocol/protocolVersion"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RequestFailure::json(400, request.request_id.clone(), -32020, "HeaderMismatch")
        })?;
    if protocol_header != body_protocol {
        return Err(RequestFailure::json(
            400,
            request.request_id.clone(),
            -32020,
            "HeaderMismatch",
        ));
    }
    if protocol_header != MCP_PROTOCOL_VERSION {
        return Err(RequestFailure::json(
            400,
            request.request_id.clone(),
            -32020,
            "Unsupported protocol version",
        ));
    }
    let method_header = headers.get("mcp-method").ok_or_else(|| {
        RequestFailure::json(400, request.request_id.clone(), -32020, "HeaderMismatch")
    })?;
    if method_header != &request.method {
        return Err(RequestFailure::json(
            400,
            request.request_id.clone(),
            -32020,
            "HeaderMismatch",
        ));
    }
    if request.method == "tools/call" {
        let name_header = headers.get("mcp-name").ok_or_else(|| {
            RequestFailure::json(400, request.request_id.clone(), -32020, "HeaderMismatch")
        })?;
        if request.tool_name.as_deref() != Some(name_header.as_str()) {
            return Err(RequestFailure::json(
                400,
                request.request_id.clone(),
                -32020,
                "HeaderMismatch",
            ));
        }
    }
    Ok(())
}

fn is_application_json(value: &str) -> bool {
    value
        .split(';')
        .next()
        .map(str::trim)
        .is_some_and(|media_type| media_type.eq_ignore_ascii_case("application/json"))
}

fn accept_has_mcp_media_types(value: &str) -> bool {
    let mut application_json = false;
    let mut event_stream = false;
    for item in value.split(',') {
        let mut parts = item.trim().split(';');
        let media_type = parts.next().unwrap_or_default().trim();
        let mut quality = 1.0_f32;
        for parameter in parts {
            if let Some((name, value)) = parameter.trim().split_once('=') {
                if name.trim().eq_ignore_ascii_case("q") {
                    quality = value.trim().parse().unwrap_or(0.0);
                }
            }
        }
        if quality <= 0.0 || quality > 1.0 {
            continue;
        }
        application_json |= media_type.eq_ignore_ascii_case("application/json");
        event_stream |= media_type.eq_ignore_ascii_case("text/event-stream");
    }
    application_json && event_stream
}

fn authority_is_loopback(authority: &str, expected_port: u16) -> bool {
    parse_authority(authority)
        .is_some_and(|(host, port)| host_is_loopback(&host) && port_matches(port, expected_port))
}

fn origin_is_loopback(origin: &str) -> bool {
    let Some((scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    if authority.is_empty() || authority.contains(['/', '?', '#', '@']) {
        return false;
    }
    parse_authority(authority).is_some_and(|(host, _port)| host_is_loopback(&host))
}

fn parse_authority(authority: &str) -> Option<(String, Option<u16>)> {
    if authority.is_empty() || authority.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    if authority.starts_with('[') {
        let close = authority.find(']')?;
        let host = authority[1..close]
            .parse::<std::net::Ipv6Addr>()
            .ok()?
            .to_string();
        let rest = &authority[close + 1..];
        let port = if rest.is_empty() {
            None
        } else {
            Some(rest.strip_prefix(':')?.parse::<u16>().ok()?)
        };
        return Some((host, port));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port.parse::<u16>().ok()?)),
        Some(_) => return None,
        None => (authority, None),
    };
    if host.is_empty() {
        return None;
    }
    Some((host.to_owned(), port))
}

fn host_is_loopback(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn port_matches(port: Option<u16>, expected_port: u16) -> bool {
    port.is_none_or(|port| port == expected_port)
}

#[derive(Debug)]
struct RequestFailure {
    status: u16,
    id: Option<Value>,
    code: i64,
    message: &'static str,
}

impl RequestFailure {
    fn http(status: u16, code: i64, message: &'static str) -> Self {
        Self {
            status,
            id: None,
            code,
            message,
        }
    }

    fn json(status: u16, id: Option<Value>, code: i64, message: &'static str) -> Self {
        Self {
            status,
            id,
            code,
            message,
        }
    }

    fn io(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                Self::http(408, -32000, "Request read timed out")
            }
            _ => Self::http(400, -32000, "Unable to read request"),
        }
    }

    fn into_reply(self) -> HttpReply {
        HttpReply::json(
            self.status,
            json_rpc_error(self.id, self.code, self.message, None),
        )
    }
}

#[derive(Debug)]
struct HttpReply {
    status: u16,
    body: Option<Value>,
    retry_after_seconds: Option<u64>,
}

impl HttpReply {
    fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: Some(body),
            retry_after_seconds: None,
        }
    }

    fn empty(status: u16) -> Self {
        Self {
            status,
            body: None,
            retry_after_seconds: None,
        }
    }

    fn from_handler(reply: HandlerReply) -> Self {
        Self {
            status: reply.status,
            body: reply.body,
            retry_after_seconds: None,
        }
    }
}

fn write_reply(stream: &mut TcpStream, reply: HttpReply) {
    let mut status = reply.status;
    let mut body = match reply.body {
        Some(body) => match serde_json::to_vec(&body) {
            Ok(body) if body.len() <= MAX_RESPONSE_BODY_BYTES => body,
            _ => {
                status = 500;
                serde_json::to_vec(&json_rpc_error(None, -32603, "Internal error", None))
                    .unwrap_or_default()
            }
        },
        None => Vec::new(),
    };
    if !(100..=599).contains(&status) {
        status = 500;
    }
    let reason = reason_phrase(status);
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !body.is_empty() {
        head.push_str("Content-Type: application/json\r\n");
    }
    if let Some(retry_after) = reply.retry_after_seconds {
        head.push_str(&format!("Retry-After: {retry_after}\r\n"));
    }
    head.push_str("Cache-Control: no-store\r\n\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
    body.clear();
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

fn json_rpc_error(
    id: Option<Value>,
    code: i64,
    message: &str,
    retry_after_seconds: Option<u64>,
) -> Value {
    let mut error = json!({ "code": code, "message": message });
    if let Some(retry) = retry_after_seconds {
        error["data"] = json!({ "retry_after_seconds": retry });
    }
    let mut response = json!({ "jsonrpc": "2.0", "error": error });
    if let Some(id) = id {
        response["id"] = id;
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;

    fn request_value(method: &str, id: Option<Value>, name: Option<&str>) -> Value {
        let mut params = json!({
            "_meta": { "io.modelcontextprotocol/protocolVersion": MCP_PROTOCOL_VERSION }
        });
        if let Some(name) = name {
            params["name"] = json!(name);
        }
        let mut rpc = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if let Some(id) = id {
            rpc["id"] = id;
        }
        rpc
    }

    fn wire_request(
        method: &str,
        target: &str,
        body: &Value,
        extra_headers: &[(&str, &str)],
    ) -> Vec<u8> {
        let body = serde_json::to_vec(body).unwrap();
        let mut request = format!(
            "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:49175\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in extra_headers {
            request.push_str(name);
            request.push_str(": ");
            request.push_str(value);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        let mut bytes = request.into_bytes();
        bytes.extend_from_slice(&body);
        bytes
    }

    fn valid_headers(method: &str, name: Option<&str>) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("MCP-Protocol-Version", MCP_PROTOCOL_VERSION.to_owned()),
            ("Mcp-Method", method.to_owned()),
        ];
        if let Some(name) = name {
            headers.push(("Mcp-Name", name.to_owned()));
        }
        headers
    }

    fn loopback_http_roundtrip(
        method: &str,
        target: &str,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> String {
        let mut server = ProductMcpServer::start(
            ProductMcpConfig {
                port: 0,
                ..ProductMcpConfig::default()
            },
            |_| Ok(GrantIdentity::new("test-grant").unwrap()),
            |request, _| {
                HandlerReply::json(
                    200,
                    json!({ "jsonrpc":"2.0", "id":request.request_id, "result":{} }),
                )
            },
        )
        .unwrap();
        let address = server.local_addr();
        let mut stream = TcpStream::connect(address).unwrap();
        let mut raw = wire_request(method, target, body, headers);
        let request_text = String::from_utf8_lossy(&raw)
            .replace("127.0.0.1:49175", &format!("127.0.0.1:{}", address.port()));
        raw = request_text.into_bytes();
        stream.write_all(&raw).unwrap();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        server.stop();
        response
    }

    #[test]
    fn binds_ipv4_loopback_only_and_stops_cleanly() {
        let mut server = ProductMcpServer::start(
            ProductMcpConfig {
                port: 0,
                ..ProductMcpConfig::default()
            },
            |_| Ok(GrantIdentity::new("test-grant").unwrap()),
            |_, _| HandlerReply::empty(204),
        )
        .unwrap();
        assert_eq!(server.local_addr().ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        server.stop();
        assert!(
            TcpStream::connect_timeout(&server.local_addr(), Duration::from_millis(100)).is_err()
        );
    }

    #[test]
    fn parses_http_framing_with_case_insensitive_header_names() {
        let body = request_value("ping", Some(json!(7)), None);
        let headers = valid_headers("ping", None);
        let headers = headers
            .iter()
            .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
            .collect::<Vec<_>>();
        let pairs = headers
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let bytes = wire_request("POST", MCP_PATH, &body, &pairs);
        let mut cursor = Cursor::new(bytes);
        let request = read_http_request(&mut cursor).unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.target, MCP_PATH);
        assert_eq!(request.headers.get("mcp-method").unwrap(), "ping");
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap(),
            body
        );
    }

    #[test]
    fn json_rpc_parse_and_invalid_request_errors_include_null_id() {
        let parse_failure = serde_json::from_slice::<Value>(b"{")
            .map_err(|_| RequestFailure::json(400, Some(Value::Null), -32700, "Parse error"))
            .unwrap_err();
        assert_eq!(parse_failure.into_reply().body.unwrap()["id"], Value::Null);

        for request in [
            json!({"jsonrpc":"2.0"}),
            json!({"jsonrpc":"2.0","method":"","params":{}}),
            json!({"jsonrpc":"2.0","method":"ping","params":"invalid"}),
        ] {
            let failure = parse_json_rpc(request).unwrap_err();
            assert_eq!(failure.into_reply().body.unwrap()["id"], Value::Null);
        }
    }

    #[test]
    fn validates_host_and_origin_as_loopback_only() {
        assert!(authority_is_loopback("127.0.0.1:49175", 49175));
        assert!(authority_is_loopback("localhost:49175", 49175));
        assert!(authority_is_loopback("[::1]:49175", 49175));
        assert!(!authority_is_loopback("example.com:49175", 49175));
        assert!(!authority_is_loopback("127.0.0.1:4444", 49175));
        assert!(origin_is_loopback("http://localhost:49175"));
        assert!(origin_is_loopback("http://127.0.0.1:1420"));
        assert!(!origin_is_loopback("https://example.com"));
        assert!(!origin_is_loopback("http://localhost:49175.evil.test"));
        assert!(!origin_is_loopback("null"));
    }

    #[test]
    fn required_mcp_headers_are_mirrored_and_case_sensitive_in_value() {
        let rpc = request_value("tools/call", Some(json!(1)), Some("fung.health"));
        let request = parse_json_rpc(rpc).unwrap();
        let mut headers = HashMap::new();
        headers.insert("mcp-protocol-version".into(), MCP_PROTOCOL_VERSION.into());
        headers.insert("mcp-method".into(), "tools/call".into());
        headers.insert("mcp-name".into(), "fung.health".into());
        validate_mcp_headers(&headers, &request).unwrap();
        headers.insert("mcp-method".into(), "TOOLS/CALL".into());
        assert!(validate_mcp_headers(&headers, &request).is_err());
        headers.insert("mcp-method".into(), "tools/call".into());
        headers.remove("mcp-name");
        assert!(validate_mcp_headers(&headers, &request).is_err());
        headers.insert("mcp-name".into(), "fung.health".into());
        headers.remove("mcp-protocol-version");
        assert!(validate_mcp_headers(&headers, &request).is_err());
    }

    #[test]
    fn notification_is_accepted_with_202_and_empty_body() {
        let body = request_value("ping", None, None);
        let headers = valid_headers("ping", None);
        let pairs = headers
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        let response = loopback_http_roundtrip("POST", MCP_PATH, &body, &pairs);
        assert!(response.starts_with("HTTP/1.1 202 Accepted\r\n"));
        assert!(response.ends_with("Content-Length: 0\r\nCache-Control: no-store\r\n\r\n"));
    }

    #[test]
    fn rejects_get_delete_and_other_methods() {
        for method in ["GET", "DELETE", "PUT"] {
            let body = request_value("ping", Some(json!(1)), None);
            let headers = valid_headers("ping", None);
            let pairs = headers
                .iter()
                .map(|(key, value)| (*key, value.as_str()))
                .collect::<Vec<_>>();
            let response = loopback_http_roundtrip(method, MCP_PATH, &body, &pairs);
            assert!(
                response.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"),
                "{method}: {response}"
            );
        }
    }

    #[test]
    fn token_bucket_is_per_authenticated_grant_and_enforces_burst() {
        let limiter = TokenBuckets::new(RateLimitConfig {
            requests_per_minute: 60,
            burst: 2,
            job_control_per_minute: 60,
            job_control_burst: 2,
        });
        assert_eq!(limiter.check("grant-a", false), Ok(()));
        assert_eq!(limiter.check("grant-a", false), Ok(()));
        assert!(limiter.check("grant-a", false).is_err());
        assert_eq!(limiter.check("grant-b", false), Ok(()));
    }

    #[test]
    fn active_request_counter_never_reserves_more_than_eight() {
        let active = AtomicUsize::new(0);
        for _ in 0..MAX_IN_FLIGHT {
            reserve_slot(&active).unwrap();
        }
        assert!(reserve_slot(&active).is_err());
        assert_eq!(active.load(Ordering::SeqCst), MAX_IN_FLIGHT);
    }

    #[test]
    fn concurrent_slot_reservations_never_exceed_eight() {
        const CONTENDERS: usize = 32;
        let active = Arc::new(AtomicUsize::new(0));
        let admitted = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(CONTENDERS + 1));
        let attempted = Arc::new(Barrier::new(CONTENDERS + 1));
        let release = Arc::new(Barrier::new(CONTENDERS + 1));
        let workers = (0..CONTENDERS)
            .map(|_| {
                let active = Arc::clone(&active);
                let admitted = Arc::clone(&admitted);
                let start = Arc::clone(&start);
                let attempted = Arc::clone(&attempted);
                let release = Arc::clone(&release);
                thread::spawn(move || {
                    start.wait();
                    let guard = if reserve_slot(&active).is_ok() {
                        admitted.fetch_add(1, Ordering::SeqCst);
                        Some(SlotGuard(active))
                    } else {
                        None
                    };
                    attempted.wait();
                    release.wait();
                    drop(guard);
                })
            })
            .collect::<Vec<_>>();

        start.wait();
        attempted.wait();
        assert_eq!(admitted.load(Ordering::SeqCst), MAX_IN_FLIGHT);
        assert_eq!(active.load(Ordering::SeqCst), MAX_IN_FLIGHT);
        release.wait();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn rejects_oversize_and_chunked_request_framing() {
        let large = format!(
            "POST /mcp HTTP/1.1\r\nHost: localhost:49175\r\nContent-Length: {}\r\n\r\n",
            MAX_REQUEST_BODY_BYTES + 1
        );
        let mut cursor = Cursor::new(large.into_bytes());
        assert_eq!(read_http_request(&mut cursor).unwrap_err().status, 413);

        let chunked = b"POST /mcp HTTP/1.1\r\nHost: localhost:49175\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n";
        let mut cursor = Cursor::new(chunked);
        assert_eq!(read_http_request(&mut cursor).unwrap_err().status, 400);
    }

    #[test]
    fn in_flight_guard_releases_after_worker_exit() {
        let active = Arc::new(AtomicUsize::new(0));
        reserve_slot(&active).unwrap();
        {
            let _guard = SlotGuard(Arc::clone(&active));
            assert_eq!(active.load(Ordering::SeqCst), 1);
        }
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
}
