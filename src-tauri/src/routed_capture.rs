//! Production routed capture coordinator. No provider, transcript, or publication authority.
#[path = "routed_capture_state.rs"]
mod state;

use crate::{
    capture_routing::{self as routing, CaptureRoutePlan, RouteReadiness},
    capture_source_custody::{self as custody, CaptureSourceRegistration},
    capture_source_writer::{
        CaptureSourceWriter, CaptureSourceWriterGroup, SourcePacketDisposition, SourcePcmPacket,
        SourceWriterOptions, SourceWriterReport,
    },
    endpoint_packet_sink::{self as sink, EndpointPacket, EndpointPacketReceiver, SinkRead},
    genesis_adapter as ga,
    live_meeting::CaptureEvent,
    native_capture_group::{self, NativeGroupObserver, PreparedCapture},
    recording_review::{CaptureReservation, NativeCaptureGuard},
};
use genesis_block_native::Storage;
use serde::Serialize;
use serde_json::json;
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

const RATE: u32 = 48_000;
const ORIGIN_LEAD_HNS: u64 = 50_000_000;
const QUEUE_CAPACITY: usize = 64;
const MAX_SOURCE_FRAMES: u64 = RATE as u64 * 86400;
const GUARD_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoutedSourceStatus {
    pub(crate) source_id: String,
    pub(crate) epoch: i64,
    pub(crate) ready: bool,
    pub(crate) native_terminated: bool,
    pub(crate) drained: bool,
    pub(crate) durable_counts_known: bool,
    pub(crate) durable_frames: u64,
    pub(crate) durable_chunks: u64,
    pub(crate) accepted_packets: u64,
    pub(crate) private_orphans: Vec<String>,
    pub(crate) first_failure: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoutedCaptureStatus {
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    pub(crate) phase: String,
    pub(crate) duration_ms: u64,
    pub(crate) sources: Vec<RoutedSourceStatus>,
    pub(crate) first_failure: Option<String>,
    pub(crate) warnings: Vec<String>,
}

struct Shared {
    status: Mutex<RoutedCaptureStatus>,
    first: Mutex<state::FailureLedger>,
    armed: AtomicBool,
    cancel_requested: AtomicBool,
    stop: Arc<AtomicBool>,
    observer: Mutex<Option<Arc<NativeGroupObserver>>>,
}
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}
fn bounded(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
impl Shared {
    fn cancelling(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst) || self.stop.load(Ordering::SeqCst)
    }
    fn cancel(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
        if let Some(observer) = lock(&self.observer).as_ref() {
            observer.revoke_all();
        }
        self.stop.store(true, Ordering::SeqCst);
    }
    fn fail(&self, source: Option<&str>, reason: impl AsRef<str>) {
        let publication = self.record_failure(source, reason.as_ref());
        self.publish_failure(publication);
        self.cancel();
    }
    fn record_failure(&self, source: Option<&str>, reason: &str) -> FailurePublication {
        let mut ledger = lock(&self.first);
        let canonical = ledger.record(source, bounded(reason));
        let own = source.map(|source| {
            (
                source.to_owned(),
                ledger
                    .for_source(source)
                    .expect("source failure recorded")
                    .to_owned(),
            )
        });
        FailurePublication { canonical, own }
    }
    fn publish_failure(&self, publication: FailurePublication) {
        let mut status = lock(&self.status);
        // Every publication carries the immutable canonical first value, even
        // when another thread publishes a later failure before this one.
        status.first_failure = Some(publication.canonical);
        if let Some((source, reason)) = publication.own {
            if let Some(item) = status
                .sources
                .iter_mut()
                .find(|item| item.source_id == source)
            {
                if item.first_failure.is_none() {
                    item.first_failure = Some(reason);
                }
            }
        }
    }
    fn failed(&self) -> bool {
        lock(&self.first).get().is_some()
    }
    fn source_failure(&self, source: &str) -> Option<String> {
        lock(&self.first).for_source(source).map(str::to_owned)
    }
    fn global_failure(&self) -> Option<String> {
        lock(&self.first).global().map(str::to_owned)
    }
}
struct FailurePublication {
    canonical: String,
    own: Option<(String, String)>,
}
enum ExecutionFailure {
    Recorded,
    Global(String),
}
impl From<String> for ExecutionFailure {
    fn from(reason: String) -> Self {
        Self::Global(reason)
    }
}
impl From<&str> for ExecutionFailure {
    fn from(reason: &str) -> Self {
        Self::Global(reason.into())
    }
}

/// Native lifetime capability; deliberately neither Clone nor Serde.
pub(crate) struct RoutedCaptureHandle {
    shared: Arc<Shared>,
    join: Option<JoinHandle<()>>,
}
impl RoutedCaptureHandle {
    pub(crate) fn status(&self) -> RoutedCaptureStatus {
        lock(&self.shared.status).clone()
    }
    pub(crate) fn is_finished(&self) -> bool {
        self.join.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub(crate) fn request_stop(&self) {
        self.shared.cancel();
    }
    pub(crate) fn stop_and_join(&mut self) -> RoutedCaptureStatus {
        self.request_stop();
        self.join();
        self.status()
    }
    pub(crate) fn join(&mut self) {
        if let Some(join) = self.join.take() {
            if join.join().is_err() {
                self.shared
                    .fail(None, "routed coordinator panicked; completion unproven");
                lock(&self.shared.status).phase = "failed".into();
            }
        }
    }
}
impl Drop for RoutedCaptureHandle {
    fn drop(&mut self) {
        self.request_stop();
        if self.join.as_ref().is_some_and(|join| join.is_finished()) {
            self.join();
            return;
        }
        if let Some(join) = self.join.take() {
            // Keep ownership in a reaper: UI shutdown does not synchronously wait for COM.
            // The coordinator retains Storage, workers and capture admission until quiescent.
            let pending = Arc::new(Mutex::new(Some(join)));
            let owned = pending.clone();
            let reaper = thread::Builder::new()
                .name("fung-routed-reaper".into())
                .spawn(move || {
                    if let Some(join) = lock(&owned).take() {
                        let _ = join.join();
                    }
                });
            if reaper.is_err() {
                // Failure to create the reaper cannot detach the native owner.
                if let Some(join) = lock(&pending).take() {
                    let _ = join.join();
                }
            }
        }
    }
}

fn canonical_uuid(value: &str) -> Result<(), String> {
    let id = Uuid::parse_str(value).map_err(|_| "invalid UUID")?;
    if id.is_nil() || id.to_string() != value {
        return Err("noncanonical/nil UUID".into());
    }
    Ok(())
}
fn validate_plan(plan: &CaptureRoutePlan) -> Result<(), String> {
    if plan.policy_version != routing::ROUTE_POLICY_VERSION || !(1..=2).contains(&plan.routes.len())
    {
        return Err("routed capture requires one or two explicit policy-v1 routes".into());
    }
    let mut sources = HashSet::new();
    let mut endpoints = HashSet::new();
    for route in &plan.routes {
        canonical_uuid(&route.source_id)?;
        if !sources.insert(&route.source_id)
            || !endpoints.insert(&route.endpoint_id)
            || route.endpoint_id.is_empty()
            || route.endpoint_id.len() > 4096
            || route.endpoint_id.contains('\0')
            || route.label.is_empty()
            || route.label.len() > 256
            || route.allowed_children.len() > 32
        {
            return Err("ambiguous source, endpoint, label, or owner selection".into());
        }
    }
    Ok(())
}
fn strict(plan: &CaptureRoutePlan) -> Result<(), String> {
    let admission = routing::admit_owned_route_plan(plan).map_err(|error| error.message)?;
    if admission.routes().len() != plan.routes.len()
        || admission
            .routes()
            .iter()
            .any(|route| route.readiness() != &RouteReadiness::Armed)
    {
        return Err("all selected routes must contain their active authorized target".into());
    }
    Ok(())
}
fn final_admission(plan: &CaptureRoutePlan) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // Strict policy without the revoked observer; an idle target is valid.
        match routing::admit_owned_route_plan(plan) {
            Ok(_) => return Ok(()),
            Err(error) if Instant::now() >= deadline => {
                return Err(format!("final native admission: {}", error.message))
            }
            Err(_) => thread::sleep(GUARD_INTERVAL),
        }
    }
}

/// Validate the complete requested path before reading or creating any prefix.
fn ordinary_local_path(path: &Path) -> Result<(), String> {
    let text = path.to_str().ok_or("nonUTF8 project path")?;
    if !path.is_absolute() || text.contains('\0') {
        return Err("ordinary absolute local path required".into());
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if !matches!(components.next(),Some(Component::Prefix(prefix)) if matches!(prefix.kind(),Prefix::Disk(_)))
            || !matches!(components.next(), Some(Component::RootDir))
            || components.any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("ordinary local drive path required".into());
        }
        if text.as_bytes().get(2) != Some(&b'\\') || text.contains('/') {
            return Err("ordinary local path separators required".into());
        }
        let tail = &text[3..];
        if !tail.is_empty() {
            for name in tail.split('\\') {
                let base = name
                    .split('.')
                    .next()
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                let device = matches!(
                    base.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                ) || ["COM", "LPT"].iter().any(|prefix| {
                    base.strip_prefix(*prefix).is_some_and(|suffix| {
                        matches!(
                            suffix,
                            "0" | "1"
                                | "2"
                                | "3"
                                | "4"
                                | "5"
                                | "6"
                                | "7"
                                | "8"
                                | "9"
                                | "¹"
                                | "²"
                                | "³"
                        )
                    })
                });
                if name.is_empty()
                    || name == "."
                    || name == ".."
                    || name.ends_with('.')
                    || name.ends_with(' ')
                    || name.chars().any(|value| {
                        value < ' ' || matches!(value, '<' | '>' | ':' | '"' | '|' | '?' | '*')
                    })
                    || device
                {
                    return Err("aliased or nonordinary project path component".into());
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        use std::path::Component;
        if path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
            || (text != "/"
                && text
                    .split('/')
                    .skip(1)
                    .any(|name| name.is_empty() || name == "." || name == ".."))
        {
            return Err("ordinary absolute path components required".into());
        }
    }
    Ok(())
}
/// Existing local ordinary paths only: reject aliases and reparse ancestors.
fn local_root(path: &Path) -> Result<PathBuf, String> {
    ordinary_local_path(path)?;
    if !path.is_dir() {
        return Err("existing absolute project root required".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        for ancestor in path.ancestors() {
            if fs::symlink_metadata(ancestor)
                .map_err(|e| e.to_string())?
                .file_attributes()
                & 0x400
                != 0
            {
                return Err("reparse project root/ancestor rejected".into());
            }
        }
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        let canonical = canonical.to_str().ok_or("nonUTF8 project root")?;
        if !canonical
            .strip_prefix(r"\\?\")
            .unwrap_or(canonical)
            .eq_ignore_ascii_case(&path.to_string_lossy())
        {
            return Err("project root alias rejected".into());
        }
    }
    #[cfg(not(windows))]
    {
        if path.canonicalize().map_err(|e| e.to_string())? != path {
            return Err("project root alias rejected".into());
        }
        for ancestor in path.ancestors() {
            if fs::symlink_metadata(ancestor)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("symlink ancestor rejected".into());
            }
        }
    }
    Ok(path.into())
}
fn project_root(storage: &Storage, project: &str) -> Result<PathBuf, String> {
    let rows = ga::query(
        storage,
        "projects",
        &["storage_path"],
        vec![ga::eq("projects", "id", json!(project))],
        1,
    )?;
    let path = ga::string(
        rows.first().ok_or("project missing")?,
        "projects.storage_path",
    )?;
    let root = PathBuf::from(path);
    checked_create_dir(&root)?;
    let probe = root.join(format!(".fung-write-{}", Uuid::new_v4()));
    let test = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| e.to_string())?;
    let synced = test.sync_all().map_err(|e| e.to_string());
    drop(test);
    fs::remove_file(&probe).map_err(|e| e.to_string())?;
    synced?;
    Ok(root)
}
fn checked_create_dir(path: &Path) -> Result<(), String> {
    ordinary_local_path(path)?;
    let mut existing = None;
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                existing = Some(ancestor);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot verify project ancestor: {error}")),
        }
    }
    let ancestor = existing.ok_or("private directory has no existing ancestor")?;
    local_root(ancestor)?;
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    local_root(path)?;
    Ok(())
}
fn qpc() -> Result<u64, String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Performance::{
            QueryPerformanceCounter, QueryPerformanceFrequency,
        };
        let mut frequency = 0;
        let mut counter = 0;
        unsafe { QueryPerformanceFrequency(&mut frequency) }.map_err(|e| e.to_string())?;
        unsafe { QueryPerformanceCounter(&mut counter) }.map_err(|e| e.to_string())?;
        if frequency <= 0 || counter < 0 {
            return Err("invalid monotonic QPC".into());
        }
        u64::try_from(counter as u128 * 10_000_000 / frequency as u128).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Err("routed endpoint capture requires Windows".into())
    }
}
fn attempt(storage: &Storage) -> Result<custody::SourceCommitAttempt, String> {
    custody::begin_source_commit(
        storage,
        &Uuid::new_v4().to_string(),
        &chrono::Utc::now().to_rfc3339(),
    )
}
// Pending is fenced from the first row; it never receives legacy microphone or
// enrichment authority while native registration has not committed.
fn start_rows(
    storage: &Storage,
    project: &str,
    recording: &str,
    job: &str,
    root: &Path,
) -> Result<(), String> {
    let at = chrono::Utc::now().to_rfc3339();
    ga::commit_rows(
        storage,
        vec![
            ga::upsert(
                "recordings",
                json!({"id":recording,"project_id":project,"source":"routed_endpoint_pending","input_path":null,"canonical_audio_path":root,"status":"recording","duration_ms":0,"created_at":at,"updated_at":at,"language":null}),
            ),
            ga::upsert(
                "mobile_recording_checkpoints",
                json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":at}),
            ),
            ga::upsert(
                "jobs",
                json!({"id":job,"project_id":project,"type":"recording.capture","status":"running","progress":0,"input_refs_json":[recording],"output_refs_json":[],"provider_id":null,"error_code":null,"error_message":null,"attempt_no":1,"started_at":at,"finished_at":null,"created_at":at,"updated_at":at}),
            ),
            ga::upsert(
                "job_events",
                json!({"id":Uuid::new_v4().to_string(),"job_id":job,"status":"running","message":"routed capture preparing native owners","created_at":at}),
            ),
            ga::upsert(
                "audit_events",
                json!({"id":Uuid::new_v4().to_string(),"project_id":project,"event_type":"routed_capture.started","actor":"native-capture","payload_json":{"recordingId":recording,"jobId":job},"created_at":at}),
            ),
        ],
    )
}
fn recording_terminal(storage: &Storage, recording: &str, status: &str) -> Result<(), String> {
    let record = ga::capture(storage, recording)?;
    ga::commit_rows(
        storage,
        vec![ga::upsert(
            "recordings",
            json!({"id":record.recording_id,"project_id":record.project_id,"source":record.source,"input_path":record.input_path,"canonical_audio_path":record.canonical_audio_path,"status":status,"duration_ms":record.duration_ms,"created_at":record.created_at,"updated_at":chrono::Utc::now().to_rfc3339(),"language":record.language}),
        )],
    )
}

pub(crate) fn start(
    storage: Arc<Storage>,
    project_id: String,
    plan: CaptureRoutePlan,
    capture_guard: Arc<NativeCaptureGuard>,
) -> Result<RoutedCaptureHandle, String> {
    canonical_uuid(&project_id)?;
    validate_plan(&plan)?;
    let reservation = NativeCaptureGuard::reserve_capture(capture_guard.clone())
        .map_err(|error| format!("capture admission: {error:?}"))?;
    let root = project_root(&storage, &project_id)?;
    if ga::active_capture(&storage, &project_id)?.is_some() {
        return Err("project already has an unfinished recording".into());
    }
    // Fresh native facts, never a client-supplied inventory/validated token.
    strict(&plan)?;
    let recording = Uuid::new_v4().to_string();
    let job = Uuid::new_v4().to_string();
    let session = root.join("live").join(&recording);
    checked_create_dir(&session)?;
    start_rows(&storage, &project_id, &recording, &job, &session)?;
    let shared = Arc::new(Shared {
        status: Mutex::new(RoutedCaptureStatus { project_id:project_id.clone(), recording_id:recording.clone(), phase:"starting".into(), duration_ms:0,
            sources:plan.routes.iter().map(|route| RoutedSourceStatus { source_id:route.source_id.clone(), epoch:1, ready:false,native_terminated:false,drained:false,durable_counts_known:false,durable_frames:0,durable_chunks:0,accepted_packets:0,private_orphans:vec![],first_failure:None }).collect(), first_failure:None,
            warnings:vec!["Capture custody only; source-aware consumer UI, export, recovery and enrichment acceptance remain open.".into()] }),
        first:Mutex::new(state::FailureLedger::default()), armed: AtomicBool::new(false), cancel_requested:AtomicBool::new(false), stop:Arc::new(AtomicBool::new(false)),observer:Mutex::new(None),
    });
    let thread_shared = shared.clone();
    let thread_guard = capture_guard.clone();
    let thread_storage = storage.clone();
    let thread_job = job.clone();
    let result = thread::Builder::new()
        .name("fung-routed-coordinator".into())
        .spawn(move || {
            let mut release = ReleaseGuard {
                guard: thread_guard,
                reservation: Some(reservation),
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                coordinate(
                    thread_storage.clone(),
                    plan,
                    session,
                    thread_job.clone(),
                    thread_shared.clone(),
                    &mut release,
                )
            }));
            if result.is_err() {
                thread_shared.fail(None, "coordinator panic; source cleanup required");
                let snapshot = lock(&thread_shared.status).clone();
                let _ = recording_terminal(&thread_storage, &snapshot.recording_id, "partial");
                let _ = crate::set_job_status(
                    &thread_storage,
                    &thread_job,
                    "failed",
                    None,
                    Some("coordinator panic"),
                );
                lock(&thread_shared.status).phase = "failed".into();
            }
        });
    match result {
        Ok(join) => Ok(RoutedCaptureHandle {
            shared,
            join: Some(join),
        }),
        Err(error) => {
            let reason = format!("start coordinator: {error}");
            let _ = recording_terminal(&storage, &recording, "failed");
            let _ = crate::set_job_status(&storage, &job, "failed", None, Some(&reason));
            Err(reason)
        }
    }
}
struct ReleaseGuard {
    guard: Arc<NativeCaptureGuard>,
    reservation: Option<CaptureReservation>,
}
impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        // Suppress the reservation's independent release before doing the sole release.
        // No stale destructor can release a subsequently admitted capture.
        if let Some(reservation) = self.reservation.take() {
            reservation.commit_active();
        }
        self.guard.release_capture();
    }
}

/// On every exit, stop and join every launched worker before releasing native admission.
struct Workers {
    shared: Arc<Shared>,
    joins: Vec<JoinHandle<SourceWriterReport>>,
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.shared.cancel();
        for join in self.joins.drain(..) {
            let _ = join.join();
        }
    }
}
fn coordinate(
    storage: Arc<Storage>,
    plan: CaptureRoutePlan,
    session: PathBuf,
    job: String,
    shared: Arc<Shared>,
    lease: &mut ReleaseGuard,
) {
    let ids = lock(&shared.status).clone();
    let mut workers = Workers {
        shared: shared.clone(),
        joins: vec![],
    };
    let mut registered = false;
    let mut reports = vec![];
    let execution = (|| -> Result<(), ExecutionFailure> {
        let origin = qpc()?.checked_add(ORIGIN_LEAD_HNS).ok_or("QPC overflow")?;
        let registrations = plan.routes.iter().map(|route| CaptureSourceRegistration {
            id:route.source_id.clone(), epoch:1,kind:"endpoint".into(),endpoint_id:Some(route.endpoint_id.clone()),label:route.label.clone(),sample_rate:i64::from(RATE),clock_origin_qpc_hns:origin.to_string(),
            metadata:json!({"routePolicyVersion":plan.policy_version,"application":route.application,"allowedChildren":route.allowed_children,"origin":"native-routed-capture"}),
        }).collect::<Vec<_>>();
        custody::register_capture_sources(
            &storage,
            &attempt(&storage)?,
            &ids.project_id,
            &ids.recording_id,
            &registrations,
        )?;
        registered = true;
        let epochs = registrations
            .iter()
            .map(|reg| (reg.id.clone(), reg.epoch))
            .collect::<Vec<_>>();
        let (tickets, observer) = native_capture_group::prepare_group(
            storage.clone(),
            ids.project_id.clone(),
            ids.recording_id.clone(),
            plan.clone(),
            &epochs,
        )?;
        let observer = Arc::new(observer);
        *lock(&shared.observer) = Some(observer.clone());
        let mut group = CaptureSourceWriterGroup::new(
            storage.clone(),
            ids.project_id.clone(),
            ids.recording_id.clone(),
        )?;
        for (reg, ticket) in registrations.into_iter().zip(tickets) {
            if ticket.source_id() != reg.id {
                return Err("factory ticket order mismatch".into());
            }
            let private = session.join("sources").join(&reg.id).join("epoch-1");
            checked_create_dir(&private)?;
            let writer = group.spawn(SourceWriterOptions::new(reg.clone()))?;
            let spool = PrivateSpool::new(&private)?;
            let (producer, receiver) = sink::packet_channel(ticket.scope().clone(), QUEUE_CAPACITY)
                .map_err(|error| format!("sink channel: {error:?}"))?;
            let state = shared.clone();
            let join = thread::Builder::new()
                .name(format!("fung-routed-{}", reg.id))
                .spawn(move || {
                    source_worker(
                        reg, writer, receiver, producer, ticket, private, spool, state,
                    )
                })
                .map_err(|error| format!("source worker spawn: {error}"))?;
            workers.joins.push(join);
        }
        await_ready(&observer, &plan, &shared, origin)?;
        lease
            .reservation
            .take()
            .ok_or("capture reservation missing")?
            .commit_active();
        shared.armed.store(true, Ordering::SeqCst);
        lock(&shared.status).phase = "recording".into();
        while !shared.stop.load(Ordering::SeqCst) {
            let check = Instant::now();
            validate_runtime(&observer, &plan, &shared).map_err(|_| ExecutionFailure::Recorded)?;
            if qpc()? >= origin.saturating_add(86400 * 10_000_000) {
                return Err("maximum 24-hour capture reached".into());
            }
            thread::sleep(GUARD_INTERVAL.saturating_sub(check.elapsed()));
        }
        lease.guard.mark_capture_stopping();
        lock(&shared.status).phase = "stopping".into();
        Ok(())
    })();
    lease.guard.mark_capture_stopping();
    let final_strict = drain_and_finalize(
        execution,
        &mut workers.joins,
        &mut reports,
        &plan,
        &shared,
        final_admission,
    );
    let all_ready = lock(&shared.status)
        .sources
        .iter()
        .all(|source| source.ready);
    let all_stopped = lock(&shared.status)
        .sources
        .iter()
        .all(|source| source.native_terminated);
    let complete = state::can_complete(
        all_ready,
        all_stopped,
        reports.len() == plan.routes.len() && reports.iter().all(|report| report.drained),
        reports.iter().all(|report| report.durable_counts_known),
        reports.iter().all(|report| report.durable_frames > 0),
        shared.failed() || reports.iter().any(|report| report.first_failure.is_some()),
        final_strict,
    );
    if !complete && !shared.failed() {
        shared.fail(None, "capture completion evidence incomplete");
    }
    if registered {
        close_registered_sources(
            &storage,
            &ids.project_id,
            &ids.recording_id,
            &plan
                .routes
                .iter()
                .map(|route| route.source_id.clone())
                .collect::<Vec<_>>(),
            &shared,
        );
        if complete && !shared.failed() {
            if let Err(error) = custody::require_capture_source_access(&storage, &ids.recording_id)
            {
                shared.fail(None, format!("completed binding reconciliation: {error}"));
            }
        }
    }
    let healthy = complete && !shared.failed();
    if !healthy {
        if let Err(error) = recording_terminal(&storage, &ids.recording_id, "partial") {
            shared.fail(None, format!("terminal recording persistence: {error}"));
        }
    }
    let reason = lock(&shared.status).first_failure.clone();
    if let Err(error) = crate::set_job_status(
        &storage,
        &job,
        if healthy { "completed" } else { "failed" },
        None,
        reason.as_deref(),
    ) {
        shared.fail(None, format!("job finalization: {error}"));
    }
    let mut snapshot = lock(&shared.status).clone();
    snapshot.duration_ms = snapshot
        .sources
        .iter()
        .map(|source| source.durable_frames * 1000 / u64::from(RATE))
        .max()
        .unwrap_or(0);
    snapshot.phase = if healthy && snapshot.first_failure.is_none() {
        "completed"
    } else {
        "partial"
    }
    .into();
    let at = chrono::Utc::now().to_rfc3339();
    if let Err(error) = ga::commit_rows(
        &storage,
        vec![ga::upsert(
            "audit_events",
            json!({"id":Uuid::new_v4().to_string(),"project_id":ids.project_id,"event_type":"routed_capture.stopped","actor":"native-capture","payload_json":snapshot,"created_at":at}),
        )],
    ) {
        shared.fail(None, format!("terminal audit persistence: {error}"));
    }
    if shared.global_failure().is_some() {
        quarantine_affected_sources(
            &storage,
            &ids.project_id,
            &ids.recording_id,
            &plan
                .routes
                .iter()
                .map(|route| route.source_id.clone())
                .collect::<Vec<_>>(),
            &shared,
        );
        let reason = lock(&shared.status).first_failure.clone();
        let _ = recording_terminal(&storage, &ids.recording_id, "partial");
        let _ = crate::set_job_status(&storage, &job, "failed", None, reason.as_deref());
    }
    let mut status = lock(&shared.status);
    status.duration_ms = status
        .sources
        .iter()
        .map(|source| source.durable_frames * 1000 / u64::from(RATE))
        .max()
        .unwrap_or(0);
    status.phase = if healthy && status.first_failure.is_none() {
        "completed"
    } else {
        "partial"
    }
    .into();
    lock(&shared.observer).take();
}
fn quarantine_affected_sources(
    storage: &Storage,
    project: &str,
    recording: &str,
    sources: &[String],
    shared: &Shared,
) {
    // A quarantine persistence failure is global uncertainty. The second pass
    // applies that global scope to peers without rewriting an existing cause.
    for _ in 0..2 {
        for source in sources {
            let Some(reason) = shared.source_failure(source) else {
                continue;
            };
            // A late global fault can reach a peer after its writer finished.
            // Export that peer's actual scoped/global cause as well as custody.
            shared.fail(Some(source), &reason);
            let result = (|| -> Result<(), String> {
                let rows = ga::query(
                    storage,
                    "capture_sources",
                    &[
                        "project_id",
                        "recording_id",
                        "capture_epoch",
                        "state",
                        "quarantine_json",
                    ],
                    vec![ga::eq("capture_sources", "id", json!(source))],
                    1,
                )?;
                let row = rows.first().ok_or("missing source during quarantine")?;
                if row["capture_sources.project_id"] != project
                    || row["capture_sources.recording_id"] != recording
                    || row["capture_sources.capture_epoch"] != 1
                {
                    return Err("quarantine source scope changed".into());
                }
                if row["capture_sources.state"] == "quarantined" {
                    if row["capture_sources.quarantine_json"]["reason"]
                        .as_str()
                        .is_some_and(|reason| !reason.is_empty())
                    {
                        return Ok(());
                    }
                    return Err("quarantine cause missing".into());
                }
                custody::quarantine_capture_source(storage, &attempt(storage)?, source, &reason)
            })();
            if let Err(error) = result {
                shared.fail(
                    None,
                    format!("source {source} quarantine persistence uncertain: {error}"),
                );
                lock(&shared.status)
                    .warnings
                    .push(bounded(format!("quarantine {source}: {error}")));
            }
        }
    }
}
fn close_registered_sources(
    storage: &Storage,
    project: &str,
    recording: &str,
    sources: &[String],
    shared: &Shared,
) {
    quarantine_affected_sources(storage, project, recording, sources, shared);
    if let Err(error) = attempt(storage)
        .and_then(|attempt| custody::complete_capture_sources(storage, &attempt, recording))
    {
        shared.fail(None, format!("source finalization: {error}"));
        quarantine_affected_sources(storage, project, recording, sources, shared);
        if let Err(error) = attempt(storage)
            .and_then(|attempt| custody::complete_capture_sources(storage, &attempt, recording))
        {
            lock(&shared.status)
                .warnings
                .push(bounded(format!("source closure incomplete: {error}")));
        }
    }
}
fn check_final_sources(
    plan: &CaptureRoutePlan,
    shared: &Shared,
    mut check: impl FnMut(&CaptureRoutePlan) -> Result<(), String>,
) -> bool {
    let mut all_passed = true;
    for route in &plan.routes {
        if shared.source_failure(&route.source_id).is_some() {
            continue;
        }
        let selected = CaptureRoutePlan {
            policy_version: plan.policy_version,
            routes: vec![route.clone()],
        };
        if let Err(error) = check(&selected) {
            // This fresh strict observation selects exactly this source; an
            // earlier peer failure cannot mask its independent final rejection.
            shared.fail(
                Some(&route.source_id),
                format!("source {} final native admission: {error}", route.source_id),
            );
            all_passed = false;
        }
    }
    all_passed
}
fn drain_and_finalize(
    execution: Result<(), ExecutionFailure>,
    joins: &mut Vec<JoinHandle<SourceWriterReport>>,
    reports: &mut Vec<SourceWriterReport>,
    plan: &CaptureRoutePlan,
    shared: &Shared,
    check: impl FnMut(&CaptureRoutePlan) -> Result<(), String>,
) -> bool {
    shared.cancel();
    match execution {
        Ok(()) | Err(ExecutionFailure::Recorded) => {}
        Err(ExecutionFailure::Global(reason)) => shared.fail(None, reason),
    }
    let mut final_sources_passed = false;
    let gate = join_then_snapshot(joins, reports, shared, plan.routes.len(), || {
        final_sources_passed = check_final_sources(plan, shared, check);
        Ok(())
    });
    if let Err(error) = gate {
        shared.fail(None, error);
        return false;
    }
    final_sources_passed
}
fn join_then_snapshot(
    joins: &mut Vec<JoinHandle<SourceWriterReport>>,
    reports: &mut Vec<SourceWriterReport>,
    shared: &Shared,
    expected: usize,
    snapshot: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    while let Some(join) = joins.pop() {
        match join.join() {
            Ok(report) => reports.push(report),
            Err(_) => shared.fail(
                None,
                "source coordinator panic; source termination unproven",
            ),
        }
    }
    let sources = lock(&shared.status).sources.clone();
    final_snapshot(reports, &sources, expected, snapshot)
}
fn final_snapshot(
    reports: &[SourceWriterReport],
    sources: &[RoutedSourceStatus],
    expected: usize,
    snapshot: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if reports.len() != expected
        || sources.len() != expected
        || !sources.iter().all(|source| source.native_terminated)
    {
        return Err("final native snapshot blocked: source termination unproven".into());
    }
    snapshot()
}
fn validate_runtime(
    observer: &NativeGroupObserver,
    plan: &CaptureRoutePlan,
    shared: &Shared,
) -> Result<(), String> {
    validate_runtime_decisions(
        plan,
        shared,
        || observer.ready(),
        |source| observer.validate_source(source),
    )
}
fn validate_runtime_decisions(
    plan: &CaptureRoutePlan,
    shared: &Shared,
    mut ready: impl FnMut() -> bool,
    mut validate_source: impl FnMut(&str) -> Result<routing::ValidatedRoutePlan, routing::RouteError>,
) -> Result<(), String> {
    if shared.cancelling() {
        return Ok(());
    }
    if !ready() {
        if shared.cancelling() {
            return Ok(());
        }
        let reason = "native owner group revoked or not ready";
        shared.fail(None, reason);
        return Err(reason.into());
    }
    for route in &plan.routes {
        if shared.cancelling() {
            return Ok(());
        }
        let result = validate_source(&route.source_id);
        if shared.cancelling()
            && !matches!(&result,Err(error) if error.code!=routing::Rejection::InventoryError)
        {
            // InventoryError conflates owner revocation, not-live and unknown
            // native/binding observations. Cancellation invalidates that evidence.
            return Ok(());
        }
        let validated = result.map_err(|error| {
            let reason = format!(
                "source {} runtime route: {}",
                route.source_id, error.message
            );
            shared.fail(Some(&route.source_id), &reason);
            reason
        })?;
        if validated.routes().len() != 1
            || validated.routes()[0].source_id().to_string() != route.source_id
        {
            let reason = format!("source {} runtime decision scope mismatch", route.source_id);
            shared.fail(None, &reason);
            return Err(reason);
        }
    }
    Ok(())
}

fn await_ready(
    observer: &NativeGroupObserver,
    plan: &CaptureRoutePlan,
    shared: &Shared,
    origin: u64,
) -> Result<(), ExecutionFailure> {
    loop {
        if shared.cancelling() {
            return Err(if shared.failed() {
                ExecutionFailure::Recorded
            } else {
                ExecutionFailure::Global("capture stopped during native startup".into())
            });
        }
        if qpc()? >= origin {
            return Err("common-origin deadline passed before all source witnesses".into());
        }
        if observer.ready()
            && lock(&shared.status)
                .sources
                .iter()
                .all(|source| source.ready)
        {
            validate_runtime(observer, plan, shared).map_err(|_| ExecutionFailure::Recorded)?;
            if qpc()? >= origin {
                return Err("runtime admission missed original common origin".into());
            }
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Bounded streaming, source-private accepted PCM evidence. Never an audio_chunks row.
struct PrivateSpool {
    pcm: BufWriter<File>,
    metadata: BufWriter<File>,
    pcm_path: PathBuf,
    metadata_path: PathBuf,
    frames: u64,
    packets: u64,
    failed: bool,
}
impl PrivateSpool {
    fn new(root: &Path) -> Result<Self, String> {
        let pcm_path = root.join("accepted-native.pcm");
        let metadata_path = root.join("accepted-native.jsonl");
        let pcm = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&pcm_path)
                .map_err(|e| e.to_string())?,
        );
        let metadata = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&metadata_path)
                .map_err(|e| e.to_string())?,
        );
        Ok(Self {
            pcm,
            metadata,
            pcm_path,
            metadata_path,
            frames: 0,
            packets: 0,
            failed: false,
        })
    }
    fn append(&mut self, packet: &EndpointPacket) -> Result<(), String> {
        if self.failed {
            return Err("private accepted stream already failed; no offset reopening".into());
        }
        let result = self.append_inner(packet);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn append_inner(&mut self, packet: &EndpointPacket) -> Result<(), String> {
        let frames = packet.samples.len() as u64;
        if self
            .frames
            .checked_add(frames)
            .is_none_or(|total| total > MAX_SOURCE_FRAMES)
            || self.packets >= MAX_SOURCE_FRAMES
        {
            return Err("private accepted stream exceeded 24h bound".into());
        }
        let offset = self.frames * 2;
        // Constant-sized buffer, no packet clones or unbounded in-memory receipts.
        let mut buffer = [0u8; 8192];
        for chunk in packet.samples.chunks(buffer.len() / 2) {
            for (sample, bytes) in chunk.iter().zip(buffer.chunks_exact_mut(2)) {
                bytes.copy_from_slice(&sample.to_le_bytes());
            }
            self.pcm
                .write_all(&buffer[..chunk.len() * 2])
                .map_err(|e| e.to_string())?;
        }
        #[cfg(test)]
        inject_packet_panic("during_private_spool");
        serde_json::to_writer(&mut self.metadata,&json!({"sourceId":packet.scope.source_id().to_string(),"epoch":packet.scope.epoch(),"firstFrame":packet.frames.start,"endFrame":packet.frames.end,"byteOffset":offset,"byteLength":frames*2,"nativeFirstFrame":packet.original_native_frames.start,"nativeEndFrame":packet.original_native_frames.end,"nativeIndex":packet.native_stamp.native_index,"nativeQpcHns":packet.native_stamp.native_qpc_hns,"observedQpcHns":packet.native_stamp.observed_qpc_hns,"flags":packet.native_stamp.native_flags,"knownIdleBefore":packet.known_idle_before.as_ref().map(|span|json!({"start":span.start,"end":span.end,"provenance":"sink-certified-callback-free-idle"})),"privateOnly":true})).map_err(|e| e.to_string())?;
        self.metadata.write_all(b"\n").map_err(|e| e.to_string())?;
        self.frames += frames;
        self.packets += 1;
        Ok(())
    }
    fn sync(&mut self) -> Result<(), String> {
        self.pcm
            .flush()
            .and_then(|_| self.pcm.get_ref().sync_all())
            .map_err(|e| e.to_string())?;
        self.metadata
            .flush()
            .and_then(|_| self.metadata.get_ref().sync_all())
            .map_err(|e| e.to_string())
    }
}
fn valid_packet(
    packet: &EndpointPacket,
    reg: &CaptureSourceRegistration,
    rx: &EndpointPacketReceiver,
) -> bool {
    packet.scope.source_id().to_string() == reg.id
        && packet.scope.epoch() == reg.epoch
        && Some(packet.scope.endpoint_id()) == reg.endpoint_id.as_deref()
        && packet.scope.sample_rate() == RATE
        && packet.scope.common_origin_hns().to_string() == reg.clock_origin_qpc_hns
        && packet.frames.end.checked_sub(packet.frames.start) == Some(packet.samples.len() as u64)
        && !packet.samples.is_empty()
        && packet.samples.len() <= RATE as usize * 2
        && packet.samples.capacity() <= RATE as usize * 2
        && packet.frames.end <= MAX_SOURCE_FRAMES
        && packet
            .original_native_frames
            .end
            .checked_sub(packet.original_native_frames.start)
            == packet
                .preroll_clipped_frames
                .checked_add(packet.samples.len() as u64)
        && rx.clock().is_some_and(|clock| {
            packet
                .original_native_frames
                .start
                .checked_add(packet.preroll_clipped_frames)
                .and_then(|first| first.checked_sub(clock.cut_frame))
                == Some(packet.frames.start)
        })
}
fn poison(
    writer: &mut CaptureSourceWriter,
    shared: &Shared,
    source: &str,
    reason: impl AsRef<str>,
) {
    // Runtime admission may have recorded the source cause before this worker
    // receives its next packet. Persist that same first cause in quarantine.
    let cause = shared
        .source_failure(source)
        .unwrap_or_else(|| bounded(reason.as_ref()));
    shared.fail(Some(source), &cause);
    writer.signal_failure(&cause);
}
fn events(
    receiver: &mpsc::Receiver<CaptureEvent>,
    writer: &mut CaptureSourceWriter,
    shared: &Shared,
    source: &str,
) -> bool {
    loop {
        match receiver.try_recv() {
            Ok(CaptureEvent::Level { .. }) => {}
            Ok(CaptureEvent::StreamFailed { error, .. }) => {
                poison(writer, shared, source, format!("native capture: {error}"))
            }
            Ok(_) => poison(
                writer,
                shared,
                source,
                "unexpected legacy media event on routed source",
            ),
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}
struct NativeExit {
    events: mpsc::Receiver<CaptureEvent>,
    shared: Arc<Shared>,
}
impl Drop for NativeExit {
    fn drop(&mut self) {
        if thread::panicking() {
            self.shared
                .fail(None, "source worker panicked; waiting for native teardown");
        }
        self.shared.cancel();
        // Sender is owned only by the native closure. Draining it prevents a bounded
        // error send from blocking teardown, including during Rust unwinding.
        while !matches!(
            self.events.recv_timeout(Duration::from_millis(20)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ) {}
    }
}
fn admit_packet(
    writer: &mut CaptureSourceWriter,
    packet: SourcePcmPacket,
    shared: &Shared,
    source: &str,
) -> bool {
    match writer.try_push_recoverable(packet) {
        Ok(()) => true,
        Err(rejected) => {
            let disposition = match rejected.disposition {
                SourcePacketDisposition::Returned(_) => {
                    "returned packet; native PCM already privately spooled"
                }
                SourcePacketDisposition::RetainedTerminal { .. } => {
                    "writer retained terminal packet"
                }
            };
            poison(
                writer,
                shared,
                source,
                format!("{} ({disposition})", rejected.failure.detail),
            );
            false
        }
    }
}
#[cfg(test)]
thread_local! { static PACKET_PANIC_STAGE:std::cell::Cell<Option<&'static str>>=const {std::cell::Cell::new(None)}; }
#[cfg(test)]
fn inject_packet_panic(stage: &str) {
    PACKET_PANIC_STAGE.with(|inject| {
        if inject.get() == Some(stage) {
            inject.set(None);
            panic!("injected source packet processing panic at {stage}");
        }
    });
}
fn preserve_private_packet(
    packet: &EndpointPacket,
    reg: &CaptureSourceRegistration,
    receiver: &EndpointPacketReceiver,
    spool: &mut PrivateSpool,
    writer: &mut CaptureSourceWriter,
    shared: &Shared,
) {
    if !valid_packet(packet, reg, receiver) {
        spool.failed = true;
        poison(
            writer,
            shared,
            &reg.id,
            "panic recovery rejected packet scope/span; raw durability incomplete/unknown",
        );
    } else if let Err(error) = spool.append(packet) {
        poison(
            writer,
            shared,
            &reg.id,
            format!("panic recovery private persistence incomplete/unknown: {error}"),
        );
    }
}
struct SourceDrainState {
    packets: u64,
    summary: Option<sink::StreamSummary>,
    native_terminated: bool,
}
#[allow(clippy::too_many_arguments)]
fn drain_source(
    reg: &CaptureSourceRegistration,
    writer: &mut CaptureSourceWriter,
    receiver: &mut EndpointPacketReceiver,
    native: &NativeExit,
    spool: &mut PrivateSpool,
    shared: &Shared,
) -> SourceDrainState {
    let mut packets = 0u64;
    let mut next_frame = 0u64;
    let mut summary = None;
    let mut native_terminated = false;
    let mut stopped_at = None;
    let mut timeout_reported = false;
    // These values outlive the processing unwind boundary. A removed packet is
    // either still owned here or already durable in the private spool.
    let mut inflight: Option<EndpointPacket> = None;
    let mut inflight_spooled = false;
    let mut spool_write_in_progress = false;
    let processing = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            native_terminated |= events(&native.events, writer, shared, &reg.id);
            if receiver.clock().is_some()
                && receiver.ready_observed_hns().is_some()
                && receiver.prebarrier_witness().is_some()
            {
                if let Some(status) = lock(&shared.status)
                    .sources
                    .iter_mut()
                    .find(|source| source.source_id == reg.id)
                {
                    status.ready = true;
                }
            }
            if shared.stop.load(Ordering::SeqCst) {
                if stopped_at.is_none() {
                    stopped_at = Some(Instant::now());
                }
                if !timeout_reported
                    && stopped_at.is_some_and(|at: Instant| at.elapsed() > Duration::from_secs(15))
                {
                    poison(
                        writer,
                        shared,
                        &reg.id,
                        "native drain exceeded 15s; waiting for actual termination",
                    );
                    timeout_reported = true;
                }
            }
            if summary.is_some() && native_terminated {
                break;
            }
            if summary.is_some() {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            match receiver.recv_timeout(Duration::from_millis(5)) {
                Ok(SinkRead::Packet(packet)) => {
                    packets += 1;
                    inflight = Some(packet);
                    inflight_spooled = false;
                    #[cfg(test)]
                    inject_packet_panic("before_private_spool");
                    let packet = inflight.as_ref().expect("inflight packet installed");
                    if !valid_packet(packet, reg, receiver) {
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            "invalid accepted packet scope/span; private spool denied",
                        );
                        continue;
                    }
                    spool_write_in_progress = true;
                    let persisted = spool.append(packet);
                    spool_write_in_progress = false;
                    if let Err(error) = persisted {
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            format!("accepted PCM private persistence: {error}"),
                        );
                    } else {
                        inflight_spooled = true;
                    }
                    if !inflight_spooled {
                        continue;
                    }
                    // Transfer to a consuming writer API only after durable private
                    // ownership exists; a writer panic cannot destroy the sole PCM.
                    if let Err(error) = spool.sync() {
                        spool.failed = true;
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            format!("accepted PCM sync before writer transfer: {error}"),
                        );
                        continue;
                    }
                    #[cfg(test)]
                    inject_packet_panic("after_private_spool");
                    if shared.source_failure(&reg.id).is_some()
                        || !shared.armed.load(Ordering::SeqCst)
                    {
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            "accepted PCM retained privately after stopped/unarmed admission",
                        );
                        continue;
                    }
                    if packet.missing_before.is_some() {
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            "native packet has missing interval",
                        );
                        continue;
                    }
                    if packet.frames.start > next_frame {
                        // Only the sink's exact certified idle span can materialize silence.
                        // Startup, lost packets and generic SourceGap never receive zero fill.
                        let certified = packet.known_idle_before.as_ref().is_some_and(|idle| {
                            idle.start == next_frame && idle.end == packet.frames.start
                        }) && receiver.prebarrier_witness().is_some()
                            && receiver.ready_observed_hns().is_some();
                        if !certified {
                            poison(
                                writer,
                                shared,
                                &reg.id,
                                "uncertified common-origin gap; no silence authority",
                            );
                            continue;
                        }
                        while next_frame < packet.frames.start
                            && shared.source_failure(&reg.id).is_none()
                        {
                            let count = (packet.frames.start - next_frame).min(u64::from(RATE) * 2);
                            if !admit_packet(
                                writer,
                                SourcePcmPacket {
                                    source_id: reg.id.clone(),
                                    epoch: reg.epoch,
                                    first_frame: next_frame as i64,
                                    pcm: vec![0; count as usize],
                                },
                                shared,
                                &reg.id,
                            ) {
                                break;
                            }
                            next_frame += count;
                        }
                        if shared.source_failure(&reg.id).is_some() {
                            continue;
                        }
                    }
                    let end_frame = packet.frames.end;
                    let packet = inflight
                        .take()
                        .expect("inflight packet retained until durable transfer");
                    let packet = SourcePcmPacket {
                        source_id: reg.id.clone(),
                        epoch: reg.epoch,
                        first_frame: packet.frames.start as i64,
                        pcm: packet.samples,
                    };
                    if admit_packet(writer, packet, shared, &reg.id) {
                        next_frame = end_frame;
                    }
                }
                Ok(SinkRead::Drained(value)) => {
                    if !shared.stop.load(Ordering::SeqCst) {
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            "native source terminated before requested stop",
                        );
                    }
                    if value.quarantined
                        || value.accepted_packets != packets
                        || value.clock.is_none()
                        || value.prebarrier_witness.is_none()
                    {
                        poison(
                            writer,
                            shared,
                            &reg.id,
                            "sink drain/quarantine/packet reconciliation failed",
                        );
                    }
                    summary = Some(value);
                }
                Ok(notice) => poison(writer, shared, &reg.id, format!("native sink: {notice:?}")),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    poison(
                        writer,
                        shared,
                        &reg.id,
                        "native sink disconnected without drain receipt",
                    );
                    // Still require native sender teardown. A disconnected sink cannot confer completion.
                    while !events(&native.events, writer, shared, &reg.id) {
                        thread::sleep(Duration::from_millis(5));
                    }
                    native_terminated = true;
                    break;
                }
            }
        }
    }));
    if processing.is_err() {
        poison(
            writer,
            shared,
            &reg.id,
            "source processing panicked; quarantined before private accepted-tail recovery",
        );
        if spool_write_in_progress {
            spool.failed = true;
            shared.fail(
                Some(&reg.id),
                "private spool write interrupted; raw durability incomplete/unknown",
            );
        }
        let recovery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(packet) = inflight.take() {
                if !inflight_spooled {
                    preserve_private_packet(&packet, reg, receiver, spool, writer, shared);
                }
            }
            while summary.is_none() {
                native_terminated |= events(&native.events, writer, shared, &reg.id);
                match receiver.recv_timeout(Duration::from_millis(5)) {
                    Ok(SinkRead::Packet(packet)) => {
                        packets += 1;
                        preserve_private_packet(&packet, reg, receiver, spool, writer, shared);
                    }
                    Ok(SinkRead::Drained(value)) => {
                        if value.accepted_packets != packets {
                            spool.failed = true;
                            poison(
                                writer,
                                shared,
                                &reg.id,
                                "panic recovery accepted-packet reconciliation mismatch",
                            );
                        }
                        summary = Some(value);
                    }
                    Ok(notice) => poison(
                        writer,
                        shared,
                        &reg.id,
                        format!("panic recovery sink notice: {notice:?}"),
                    ),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        spool.failed = true;
                        break;
                    }
                }
            }
            while !native_terminated {
                native_terminated = events(&native.events, writer, shared, &reg.id);
                if !native_terminated {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }));
        if recovery.is_err() {
            spool.failed = true;
            shared.fail(
                Some(&reg.id),
                "accepted-tail recovery interrupted; raw durability incomplete/unknown",
            );
            // NativeExit still owns stop-and-termination cleanup. Missing receipt
            // and unknown counts prevent any healthy completion classification.
            summary = None;
        }
    }
    SourceDrainState {
        packets,
        summary,
        native_terminated,
    }
}

#[allow(clippy::too_many_arguments)]
fn source_worker(
    reg: CaptureSourceRegistration,
    mut writer: CaptureSourceWriter,
    mut receiver: EndpointPacketReceiver,
    producer: sink::EndpointPacketProducer,
    ticket: PreparedCapture,
    private: PathBuf,
    mut spool: PrivateSpool,
    shared: Arc<Shared>,
) -> SourceWriterReport {
    let (events_tx, events_rx) = mpsc::sync_channel(QUEUE_CAPACITY);
    let native = NativeExit {
        events: events_rx,
        shared: shared.clone(),
    };
    let launched = crate::live_system_audio::spawn_owned_endpoint_capture_thread(
        shared.stop.clone(),
        events_tx,
        private,
        2000,
        producer,
        ticket,
    );
    if let Err(error) = launched {
        poison(
            &mut writer,
            &shared,
            &reg.id,
            format!("native setup: {error}"),
        );
    }
    let drain = drain_source(
        &reg,
        &mut writer,
        &mut receiver,
        &native,
        &mut spool,
        &shared,
    );
    finish_source(&reg, writer, spool, drain, &shared)
}

fn finish_source(
    reg: &CaptureSourceRegistration,
    mut writer: CaptureSourceWriter,
    mut spool: PrivateSpool,
    drain: SourceDrainState,
    shared: &Shared,
) -> SourceWriterReport {
    let SourceDrainState {
        packets,
        summary,
        native_terminated,
    } = drain;
    if shared.source_failure(&reg.id).is_some() {
        poison(
            &mut writer,
            shared,
            &reg.id,
            "group stopped after capture failure",
        );
    }
    let raw_synced = match spool.sync() {
        Ok(()) => true,
        Err(error) => {
            poison(
                &mut writer,
                shared,
                &reg.id,
                format!("private stream sync failed: {error}"),
            );
            false
        }
    };
    let mut report = writer.finish();
    if let Some(failure) = &report.first_failure {
        shared.fail(Some(&reg.id), &failure.detail);
    }
    if let Some(error) = &report.quarantine_error {
        shared.fail(
            None,
            format!(
                "source {} quarantine persistence uncertain: {error}",
                reg.id
            ),
        );
    }
    report
        .private_orphans
        .extend([spool.pcm_path, spool.metadata_path]);
    if !raw_synced || spool.failed {
        report.durable_counts_known = false;
        if report.persistence_error.is_none() {
            report.persistence_error =
                Some("private accepted PCM durability incomplete/unknown".into());
        }
        lock(&shared.status).warnings.push(format!(
            "source {} private accepted PCM durability incomplete/unknown",
            reg.id
        ));
    }
    if summary.is_none() || !native_terminated {
        report.drained = false;
    }
    if let Some(status) = lock(&shared.status)
        .sources
        .iter_mut()
        .find(|source| source.source_id == reg.id)
    {
        status.native_terminated = native_terminated;
        status.drained = report.drained;
        status.durable_counts_known = report.durable_counts_known;
        status.durable_frames = report.durable_frames;
        status.durable_chunks = report.durable_chunks;
        status.accepted_packets = packets;
        status.private_orphans = report
            .private_orphans
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        if status.first_failure.is_none() {
            status.first_failure = report
                .first_failure
                .as_ref()
                .map(|failure| failure.detail.clone());
        }
    }
    report
}

#[cfg(test)]
#[path = "routed_capture_tests.rs"]
mod tests;
