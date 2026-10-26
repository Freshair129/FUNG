//! External evidence probe. Proposed same-crate ignored Windows test only.
//! Requires reviewed endpoint_packet_sink + recoverable writer APIs first.
use crate::{
    capture_routing as routing, capture_source_custody as custody,
    capture_source_writer::{
        CaptureSourceWriter, CaptureSourceWriterGroup, SourcePacketDisposition, SourcePcmPacket,
        SourceWriterOptions, SourceWriterReport,
    },
    endpoint_packet_sink::{self as sink, EndpointPacketReceiver, SinkRead, SourcePacketScope},
    genesis_adapter as ga,
    live_meeting::CaptureEvent,
    live_system_audio::{spawn_selected_endpoint_capture_thread, EndpointSelection},
};
use genesis_block_native::{OpenOptions, Storage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf, Prefix},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

const RATE: i64 = 48_000;
const MAX_PLAN: u64 = 128 * 1024;
type R<T> = Result<T, String>;

#[derive(Debug)]
struct Config {
    root: PathBuf,
    plan: routing::CaptureRoutePlan,
    plan_hash: String,
    duration_ms: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnedBinary {
    pid: u32,
    image_path: String,
    sha256: String,
    creation_filetime: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    plan: routing::CaptureRoutePlan,
    owned_binaries: Vec<OwnedBinary>,
}

fn bounded_read(path: &Path, limit: u64) -> R<Vec<u8>> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    if !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("not a file".into());
    }
    let mut bytes = Vec::new();
    (&mut f)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("input exceeds bound".into());
    }
    Ok(bytes)
}
fn file_hash(path: &Path) -> R<String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err("binary changed while hashing".into());
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn local_unaliased(path: &Path, must_exist: bool) -> R<PathBuf> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)))
        || !matches!(components.next(), Some(Component::RootDir))
        || components.any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("absolute ordinary local drive path required".into());
    }
    let existing = if must_exist {
        path
    } else {
        path.parent().ok_or("missing parent")?
    };
    for ancestor in existing.ancestors() {
        use std::os::windows::fs::MetadataExt;
        if fs::symlink_metadata(ancestor)
            .map_err(|e| e.to_string())?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err("reparse ancestor rejected".into());
        }
    }
    let canonical = existing.canonicalize().map_err(|e| e.to_string())?;
    let canonical = canonical
        .to_str()
        .ok_or("nonUTF8 path")?
        .strip_prefix(r"\\?\")
        .unwrap_or(canonical.to_str().unwrap());
    if !canonical.eq_ignore_ascii_case(&existing.to_string_lossy()) {
        return Err("path alias rejected".into());
    }
    Ok(path.to_owned())
}
fn validate_input(root: &Path, bytes: &[u8], duration_ms: u64) -> R<Config> {
    local_unaliased(root, false)?;
    if root.exists() || !(3000..=120000).contains(&duration_ms) {
        return Err("fresh absent root and bounded duration required".into());
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repo parent")?;
    if root.starts_with(manifest) {
        return Err("probe root cannot be inside product repository".into());
    }
    // A private verification root is mandatory; rejects ordinary profile paths.
    let authority = Path::new(r"G:\AI-Studio\verification");
    if !root.starts_with(authority) || root == authority {
        return Err("root outside explicit verification authority".into());
    }
    let input: Input = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let plan = input.plan;
    if plan.policy_version != routing::ROUTE_POLICY_VERSION || plan.routes.len() != 2 {
        return Err("exactly two policy-v1 routes required".into());
    }
    let mut sources = HashSet::new();
    let mut endpoints = HashSet::new();
    let mut owners = HashSet::new();
    for route in &plan.routes {
        let id = uuid::Uuid::parse_str(&route.source_id).map_err(|e| e.to_string())?;
        if id.is_nil()
            || id.to_string() != route.source_id
            || !sources.insert(route.source_id.clone())
            || route.endpoint_id.is_empty()
            || route.endpoint_id.contains('\0')
            || !endpoints.insert(route.endpoint_id.clone())
            || route.allowed_children.len() > 32
        {
            return Err("invalid distinct explicit source/endpoint identity".into());
        }
        for owner in std::iter::once(&route.application).chain(route.allowed_children.iter()) {
            if owner.pid == 0 || owner.image_path.is_empty() || owner.creation_filetime.is_empty() {
                return Err("incomplete owned identity".into());
            }
            owners.insert(owner.pid);
            let matches = input
                .owned_binaries
                .iter()
                .filter(|b| b.pid == owner.pid)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err("exactly one binary receipt per owned PID required".into());
            }
            let b = matches[0];
            if b.image_path != owner.image_path
                || b.creation_filetime != owner.creation_filetime
                || b.sha256.len() != 64
                || !b.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err("owned receipt disagrees with route identity".into());
            }
            let path = local_unaliased(Path::new(&b.image_path), true)?;
            if file_hash(&path)? != b.sha256.to_ascii_lowercase() {
                return Err("owned executable hash mismatch".into());
            }
        }
    }
    if input.owned_binaries.len() != owners.len() {
        return Err("extra owned binary receipt rejected".into());
    }
    Ok(Config {
        root: root.into(),
        plan,
        plan_hash: format!("{:x}", Sha256::digest(bytes)),
        duration_ms,
    })
}
fn config() -> R<Config> {
    if std::env::var("FUNG_ROUTED_PROBE_CONFIRM").as_deref() != Ok("OWNED_TWO_ENDPOINTS_ONLY") {
        return Err("explicit owned probe confirmation absent".into());
    }
    let root = PathBuf::from(std::env::var_os("FUNG_ROUTED_PROBE_ROOT").ok_or("missing root")?);
    let input = PathBuf::from(std::env::var_os("FUNG_ROUTED_PROBE_PLAN").ok_or("missing plan")?);
    local_unaliased(&input, true)?;
    let duration = std::env::var("FUNG_ROUTED_PROBE_DURATION_MS")
        .map_err(|e| e.to_string())?
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    validate_input(&root, &bounded_read(&input, MAX_PLAN)?, duration)
}
fn qpc() -> R<u64> {
    let mut f = 0;
    let mut c = 0;
    unsafe { QueryPerformanceFrequency(&mut f) }.map_err(|e| e.to_string())?;
    unsafe { QueryPerformanceCounter(&mut c) }.map_err(|e| e.to_string())?;
    if f <= 0 || c < 0 {
        return Err("invalid QPC".into());
    }
    u64::try_from(c as u128 * 10_000_000 / f as u128).map_err(|e| e.to_string())
}
fn durable_json(path: &Path, value: &Value) -> R<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut f, value).map_err(|e| e.to_string())?;
    f.write_all(b"\n")
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
fn attempt(s: &Storage) -> R<custody::SourceCommitAttempt> {
    custody::begin_source_commit(
        s,
        &uuid::Uuid::new_v4().to_string(),
        &chrono::Utc::now().to_rfc3339(),
    )
}
fn armed(plan: &routing::CaptureRoutePlan) -> R<routing::ValidatedRoutePlan> {
    let result = routing::admit_route_plan(plan).map_err(|e| format!("{e:?}"))?;
    if result.routes().len() != plan.routes.len()
        || result
            .routes()
            .iter()
            .any(|r| r.readiness() != &routing::RouteReadiness::Armed)
    {
        return Err("all native routes must be Armed".into());
    }
    Ok(result)
}
struct RuntimeAdmission {
    receipt: Value,
    error: Option<routing::RouteError>,
}
fn runtime_failure_message(error: &routing::RouteError) -> String {
    if error.code == routing::Rejection::InventoryError {
        "InventoryError: native inventory error; selected-source projection retained".into()
    } else {
        format!("{:?}: {}", error.code, error.message)
    }
}
fn runtime_preflight(plan: &routing::CaptureRoutePlan) -> Option<routing::RouteError> {
    // Mirrors admit_route_plan's pre-native limits; no admission rule is waived.
    if plan.policy_version != routing::ROUTE_POLICY_VERSION {
        return Some(routing::RouteError {
            code: routing::Rejection::PolicyVersion,
            message: "unsupported route policy".into(),
        });
    }
    if plan.routes.is_empty()
        || plan.routes.len() > 2
        || plan.routes.iter().any(|r| r.allowed_children.len() > 32)
    {
        return Some(routing::RouteError {
            code: routing::Rejection::RouteCount,
            message: "route/child count exceeds admission bounds".into(),
        });
    }
    None
}
fn runtime_context(
    plan: &routing::CaptureRoutePlan,
    guard_qpc_hns: Option<u64>,
    own_pid: u32,
) -> Value {
    json!({"policyVersion":plan.policy_version,"guardQpcHns":guard_qpc_hns,"ownCurrentPid":own_pid,"routes":plan.routes.iter().map(|r|json!({"sourceId":r.source_id,"epoch":1,"endpointId":r.endpoint_id,"application":r.application,"allowedChildren":r.allowed_children})).collect::<Vec<_>>()})
}
fn runtime_snapshot(
    plan: &routing::CaptureRoutePlan,
    snapshot: &routing::NativeInventory,
    now_ms: u64,
    guard_qpc_hns: u64,
    own_pid: u32,
) -> RuntimeAdmission {
    // Full snapshot is used once for validation. Only selected endpoints,
    // default role IDs, requested owners and this observer are exported.
    let validation = match runtime_preflight(plan) {
        Some(error) => Err(error),
        None => routing::validate_route_plan(plan, snapshot, now_ms),
    };
    let validation = validation.and_then(|valid| {
        if valid.routes().len() == 1 {
            Ok(valid)
        } else {
            Err(routing::RouteError {
                code: routing::Rejection::RouteCount,
                message: "single-source runtime receipt missing".into(),
            })
        }
    });
    let owned = plan
        .routes
        .iter()
        .flat_map(|r| {
            std::iter::once(r.application.pid).chain(r.allowed_children.iter().map(|c| c.pid))
        })
        .collect::<HashSet<_>>();
    let selected = snapshot
        .endpoints
        .iter()
        .filter(|e| plan.routes.iter().any(|r| r.endpoint_id == e.id))
        .collect::<Vec<_>>();
    let projection = json!({"capturedAtMs":snapshot.captured_at_ms,"validatedAtMs":now_ms,"processSnapshotCutoffFiletime":snapshot.process_snapshot_cutoff_filetime,"defaultRoles":snapshot.defaults,"selectedEndpoints":selected,"requestedOwnedProcesses":snapshot.processes.iter().filter(|p|owned.contains(&p.pid)).collect::<Vec<_>>(),"observerProcess":snapshot.processes.iter().find(|p|p.pid==own_pid),"nativeInventoryErrorCount":snapshot.errors.len()});
    let error = validation.as_ref().err().cloned();
    let exported_error=error.as_ref().map(|e|json!({"code":e.code,"message":if e.code==routing::Rejection::InventoryError{"native inventory error; unrelated endpoint details withheld"}else{&e.message}}));
    let receipt = json!({"context":runtime_context(plan,Some(guard_qpc_hns),own_pid),"accepted":validation.is_ok(),"sameInputSnapshotProjection":projection,"validation":validation.as_ref().ok(),"rejection":exported_error});
    RuntimeAdmission { receipt, error }
}
fn runtime_admission(plan: &routing::CaptureRoutePlan) -> RuntimeAdmission {
    let own_pid = std::process::id();
    let guard_qpc = qpc();
    let fail = |error: routing::RouteError| RuntimeAdmission {
        receipt: json!({"context":runtime_context(plan,guard_qpc.as_ref().ok().copied(),own_pid),"accepted":false,"sameInputSnapshotProjection":null,"rejection":{"code":error.code,"message":runtime_failure_message(&error)}}),
        error: Some(error),
    };
    if let Some(error) = runtime_preflight(plan) {
        return fail(error);
    }
    let guard_qpc_hns = match &guard_qpc {
        Ok(value) => *value,
        Err(reason) => {
            return fail(routing::RouteError {
                code: routing::Rejection::InventoryError,
                message: reason.clone(),
            })
        }
    };
    let mut pids = plan
        .routes
        .iter()
        .flat_map(|r| {
            std::iter::once(r.application.pid).chain(r.allowed_children.iter().map(|c| c.pid))
        })
        .collect::<Vec<_>>();
    // Observer PID is queried solely for provenance; it is never added to any
    // route/allowed-child list and cannot authorize any of its audio sessions.
    pids.push(own_pid);
    pids.sort_unstable();
    pids.dedup();
    let snapshot = match routing::read_native_inventory(&pids) {
        Ok(snapshot) => snapshot,
        Err(error) => return fail(error),
    };
    let now_ms = match std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
    {
        Some(now) => now,
        None => {
            return fail(routing::RouteError {
                code: routing::Rejection::InventoryError,
                message: "invalid system clock".into(),
            })
        }
    };
    runtime_snapshot(plan, &snapshot, now_ms, guard_qpc_hns, own_pid)
}
fn open_store(root: &Path) -> R<Arc<Storage>> {
    Storage::open(OpenOptions {
        path: root.join("db").display().to_string(),
        page_cache_mb: Some(16),
        read_only: Some(false),
        vector_dim: Some(4),
        retention: None,
    })
    .map(Arc::new)
    .map_err(|e| e.to_string())
}
fn setup(
    c: &Config,
    s: &Storage,
    initial: &routing::ValidatedRoutePlan,
) -> R<(String, String, Vec<custody::CaptureSourceRegistration>, u64)> {
    ga::install(s)?;
    let project = uuid::Uuid::new_v4().to_string();
    let recording = uuid::Uuid::new_v4().to_string();
    let project_root = c.root.join("project");
    fs::create_dir(&project_root).map_err(|e| e.to_string())?;
    let at = chrono::Utc::now().to_rfc3339();
    ga::commit_rows(
        s,
        vec![
            ga::upsert(
                "projects",
                json!({"id":project,"name":"Private native probe","storage_path":project_root,"created_at":at,"updated_at":at}),
            ),
            ga::upsert(
                "recordings",
                json!({"id":recording,"project_id":project,"source":"microphone","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":at,"updated_at":at}),
            ),
            ga::upsert(
                "mobile_recording_checkpoints",
                json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":at}),
            ),
        ],
    )?;
    // Five seconds is explicit shared preroll, after database fixture setup.
    // Late registration/activation fails instead of rebasing either source.
    let origin = qpc()?.checked_add(50_000_000).ok_or("deadline overflow")?;
    let regs = c
        .plan
        .routes
        .iter()
        .map(|r| {
          let admitted=initial.routes().iter().find(|v|v.source_id().to_string()==r.source_id && v.endpoint_id()==r.endpoint_id).ok_or("initial source admission provenance missing")?;
          if admitted.readiness()!=&routing::RouteReadiness::Armed || admitted.observed_active_session_ids().is_empty(){return Err("initial active session provenance missing".to_string());}
          Ok(custody::CaptureSourceRegistration {
            id: r.source_id.clone(),
            epoch: 1,
            kind: "endpoint".into(),
            endpoint_id: Some(r.endpoint_id.clone()),
            label: r.label.clone(),
            sample_rate: RATE,
            clock_origin_qpc_hns: origin.to_string(),
            metadata: json!({"probe":true,"policyVersion":c.plan.policy_version,"inputHash":c.plan_hash,"application":r.application,"allowedChildren":r.allowed_children,"observedActiveSessionIds":admitted.observed_active_session_ids(),"admissionValidatedAtMs":initial.validated_at_ms()}),
        })})
        .collect::<R<Vec<_>>>()?;
    for reg in &regs {
        if serde_json::to_vec(&reg.metadata)
            .map_err(|e| e.to_string())?
            .len()
            > 16384
        {
            return Err("source registration provenance exceeds 16 KiB; fail closed".into());
        }
    }
    custody::register_capture_sources(s, &attempt(s)?, &project, &recording, &regs)?;
    Ok((project, recording, regs, origin))
}
#[derive(Serialize)]
struct PacketReceipt {
    index: u64,
    frames_start: u64,
    frames_end: u64,
    pcm_sha256: String,
    file: PathBuf,
    byte_offset: u64,
    byte_length: u64,
    disposition: String,
    metadata: Value,
    metadata_sha256: String,
}
struct RawEvidence {
    pcm: BufWriter<fs::File>,
    metadata: BufWriter<fs::File>,
    pcm_path: PathBuf,
    metadata_path: PathBuf,
    offset: u64,
}
impl RawEvidence {
    fn create(private: &Path) -> R<Self> {
        let pcm_path = private.join("accepted.pcm16");
        let metadata_path = private.join("packets.jsonl");
        let create = |path: &Path| {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())
        };
        Ok(Self {
            pcm: BufWriter::with_capacity(RATE as usize * 4, create(&pcm_path)?),
            metadata: BufWriter::with_capacity(65536, create(&metadata_path)?),
            pcm_path,
            metadata_path,
            offset: 0,
        })
    }
    fn append_pcm(&mut self, samples: &[i16]) -> R<(u64, u64, String)> {
        if samples.is_empty() || samples.len() > RATE as usize * 2 {
            return Err("diagnostic PCM packet bound exceeded".into());
        }
        // One bounded packet allocation/write, at most two seconds of mono PCM16.
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let offset = self.offset;
        let length = bytes.len() as u64;
        self.pcm.write_all(&bytes).map_err(|e| e.to_string())?;
        self.offset = self
            .offset
            .checked_add(length)
            .ok_or("diagnostic offset overflow")?;
        Ok((offset, length, format!("{:x}", Sha256::digest(&bytes))))
    }
    fn append_receipt(&mut self, receipt: &PacketReceipt) -> R<()> {
        serde_json::to_writer(&mut self.metadata, receipt).map_err(|e| e.to_string())?;
        self.metadata.write_all(b"\n").map_err(|e| e.to_string())
    }
    fn sync(&mut self) -> R<()> {
        self.pcm.flush().map_err(|e| e.to_string())?;
        self.metadata.flush().map_err(|e| e.to_string())?;
        self.pcm.get_ref().sync_all().map_err(|e| e.to_string())?;
        self.metadata
            .get_ref()
            .sync_all()
            .map_err(|e| e.to_string())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinatorFault {
    source_id: Option<String>,
    class: String,
    reason: String,
}
type Faults = Arc<Mutex<Vec<CoordinatorFault>>>;
fn register_fault(faults: &Faults, source: Option<&str>, class: &str, reason: String) {
    let mut faults = faults.lock().unwrap_or_else(|p| p.into_inner());
    if !faults.iter().any(|f| f.source_id.as_deref() == source) {
        faults.push(CoordinatorFault {
            source_id: source.map(str::to_owned),
            class: class.into(),
            reason,
        });
    }
}
fn source_fault(faults: &Faults, source: &str) -> Option<CoordinatorFault> {
    let faults = faults.lock().unwrap_or_else(|p| p.into_inner());
    faults
        .iter()
        .find(|f| f.source_id.as_deref() == Some(source))
        .or_else(|| faults.iter().find(|f| f.source_id.is_none()))
        .cloned()
}
fn span(span: Option<sink::FrameSpan>) -> Value {
    span.map(|v| json!({"start":v.start,"end":v.end}))
        .unwrap_or(Value::Null)
}
fn notice_value(notice: &SinkRead) -> Value {
    match notice {
        SinkRead::StartupMissing { scope, missing } => {
            json!({"class":"startupMissing","sourceId":scope.source_id(),"elapsedHns":missing.elapsed_hns,"roundedFrameSpan":span(missing.rounded_frame_span)})
        }
        SinkRead::StartupUnverified { scope, unverified } => {
            json!({"class":"startupUnverified","sourceId":scope.source_id(),"reason":unverified.reason,"firstObservedHns":unverified.first_observed_hns,"commonOriginHns":unverified.common_origin_hns,"missingRange":null})
        }
        SinkRead::NativeGap { scope, gap } => {
            json!({"class":"nativeGap","sourceId":scope.source_id(),"nativeFrames":span(Some(gap.native_frames)),"visibleFrames":span(gap.visible_frames)})
        }
        SinkRead::Fault { scope, fault } => {
            json!({"class":"sinkFault","sourceId":scope.source_id(),"kind":match fault{sink::SinkFault::Protocol(_)=>"protocol",sink::SinkFault::QueueFull{..}=>"queueFull",sink::SinkFault::ConsumerGone{..}=>"consumerGone",sink::SinkFault::Native(_)=>"native",sink::SinkFault::AbandonedProducer=>"abandonedProducer"},"reason":match fault{sink::SinkFault::Protocol(reason)|sink::SinkFault::Native(reason)=>Some(reason),_=>None},"detail":format!("{fault:?}"),"droppedFrames":match fault{sink::SinkFault::QueueFull{dropped}|sink::SinkFault::ConsumerGone{dropped}=>span(Some(*dropped)),_=>Value::Null}})
        }
        _ => json!({"class":"unexpectedNotice"}),
    }
}
fn summary_value(summary: Option<&sink::StreamSummary>) -> Value {
    summary.map(|s|json!({"sourceId":s.scope.source_id(),"epoch":s.scope.epoch(),"endpointId":s.scope.endpoint_id(),"sampleRate":s.scope.sample_rate(),"commonOriginHns":s.scope.common_origin_hns(),"clock":s.clock.as_ref().map(|c|json!({"nativeOriginHns":c.native_origin_hns,"commonOriginHns":c.common_origin_hns,"sampleRate":c.sample_rate,"cutFrame":c.cut_frame,"phaseNumerator":c.phase_numerator})),"readyObservedHns":s.ready_observed_hns,"prebarrierWitness":s.prebarrier_witness.as_ref().map(|w|json!({"nativeFrames":span(Some(w.native_frames)),"nativeQpcHns":w.native_qpc_hns,"observedQpcHns":w.observed_qpc_hns,"nativeFlags":w.native_flags})),"acceptedPackets":s.accepted_packets,"lastAcceptedEndFrame":s.last_accepted_end_frame,"prerollPacketsDropped":s.preroll_packets_dropped,"prerollFramesDropped":s.preroll_frames_dropped,"quarantined":s.quarantined})).unwrap_or(Value::Null)
}
struct SourceResult {
    source: String,
    registration: custody::CaptureSourceRegistration,
    writer: SourceWriterReport,
    summary: Option<sink::StreamSummary>,
    packets: Vec<PacketReceipt>,
    errors: Vec<String>,
    ready: Option<Value>,
    native_terminated: bool,
    idle_spans: Vec<(u64, u64)>,
    evidence_synced: bool,
    metadata_path: PathBuf,
    notices: Vec<Value>,
    diagnostics_persisted: bool,
}
fn source_diagnostic(r: &SourceResult) -> Value {
    json!({"sourceId":r.source,"registration":r.registration,"writer":report_writer(&r.writer),"sink":summary_value(r.summary.as_ref()),"ready":r.ready,"errors":r.errors,"notices":r.notices,"packets":r.packets,"verifiedIdleSpans":r.idle_spans,"nativeTerminated":r.native_terminated,"diagnosticsPersisted":r.diagnostics_persisted,"diagnosticEvidence":{"durability":"bufferedDuringCaptureFinalSyncRequired","synced":r.evidence_synced,"metadataPath":r.metadata_path}})
}
struct CaptureCleanup {
    stop: Arc<AtomicBool>,
    root: PathBuf,
    handles: Vec<(String, thread::JoinHandle<SourceResult>)>,
    faults: Faults,
}
impl Drop for CaptureCleanup {
    fn drop(&mut self) {
        if self.handles.is_empty() {
            return;
        }
        self.stop.store(true, Ordering::SeqCst);
        let mut sources = vec![];
        let mut failures = vec![];
        for (source, handle) in std::mem::take(&mut self.handles) {
            match handle.join() {
                Ok(result) => sources.push(source_diagnostic(&result)),
                Err(_) => {
                    failures.push(json!({"sourceId":source,"class":"sourceCoordinatorPanic"}))
                }
            }
        }
        let faults = self
            .faults
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let receipt = json!({"phase":"earlyResultErrOrUnwindCleanup","healthy":false,"sources":sources,"coordinatorFaults":faults,"cleanupFailures":failures});
        if let Err(error) = durable_json(&self.root.join("early-return-diagnostics.json"), &receipt)
        {
            eprintln!("cleanup diagnostic persistence failed: {error}");
        }
    }
}
fn report_writer(w: &SourceWriterReport) -> Value {
    json!({"sourceId":w.source_id,"epoch":w.epoch,"durableChunks":w.durable_chunks,"durableFrames":w.durable_frames,"firstFailure":w.first_failure.as_ref().map(|f|json!({"kind":format!("{:?}",f.kind),"detail":f.detail,"expectedFrame":f.expected_frame,"observedFrame":f.observed_frame,"packetFrames":f.packet_frames})),"quarantinePersisted":w.quarantine_persisted,"drained":w.drained,"privateOrphans":w.private_orphans,"durableCountsKnown":w.durable_counts_known,"persistenceError":w.persistence_error,"quarantineError":w.quarantine_error})
}
fn poison(
    errors: &mut Vec<String>,
    writer: &mut CaptureSourceWriter,
    stop: &AtomicBool,
    cause: String,
) {
    if errors.is_empty() {
        writer.signal_failure(&cause);
    }
    errors.push(cause);
    stop.store(true, Ordering::SeqCst);
}
fn admit(
    writer: &mut CaptureSourceWriter,
    packet: SourcePcmPacket,
    expected: &custody::CaptureSourceRegistration,
) -> R<&'static str> {
    let original_first = packet.first_frame;
    let original_frames = packet.pcm.len();
    let mut original_hash = Sha256::new();
    for sample in &packet.pcm {
        original_hash.update(sample.to_le_bytes());
    }
    let original_hash = original_hash.finalize();
    match writer.try_push_recoverable(packet) {
        Ok(()) => Ok("writerAccepted"),
        Err(rejection) => {
            // Returned bytes are not automatically authorized to become an orphan.
            let failure = format!("{:?}", rejection.failure);
            match rejection.disposition {
                SourcePacketDisposition::Returned(packet) => {
                    if packet.source_id != expected.id
                        || packet.epoch != expected.epoch
                        || packet.pcm.is_empty()
                        || packet.pcm.len() > RATE as usize * 2
                        || packet.pcm.capacity() > RATE as usize * 2
                        || packet.first_frame < 0
                        || packet.first_frame != original_first
                        || packet.pcm.len() != original_frames
                    {
                        return Err(format!(
                            "unauthorized returned packet; no spool permission: {failure}"
                        ));
                    }
                    let mut returned_hash = Sha256::new();
                    for sample in &packet.pcm {
                        returned_hash.update(sample.to_le_bytes());
                    }
                    if returned_hash.finalize() != original_hash {
                        return Err("returned packet bytes changed; closure blocked".into());
                    }
                    // Exact PCM is retained in the bounded buffered diagnostic
                    // stream; final flush+sync remains a mandatory closure gate.
                    Err(format!("returnedToPrivateRawEvidence: {failure}"))
                }
                SourcePacketDisposition::RetainedTerminal {
                    source_id,
                    epoch,
                    first_frame,
                    frames,
                } => {
                    if source_id != expected.id
                        || epoch != expected.epoch
                        || first_frame < 0
                        || frames == 0
                        || first_frame != original_first
                        || frames != original_frames
                    {
                        return Err(format!("invalid retained receipt: {failure}"));
                    }
                    Err(format!("writerRetainedTerminal: {failure}"))
                }
            }
        }
    }
}
struct SourceWorkerInput {
    reg: custody::CaptureSourceRegistration,
    writer: CaptureSourceWriter,
    rx: EndpointPacketReceiver,
    producer: sink::EndpointPacketProducer,
    stop: Arc<AtomicBool>,
    private: PathBuf,
    evidence: RawEvidence,
    ready_tx: mpsc::Sender<Value>,
    faults: Faults,
}
fn source_worker(input: SourceWorkerInput) -> SourceResult {
    let SourceWorkerInput {
        reg,
        mut writer,
        mut rx,
        producer,
        stop,
        private,
        mut evidence,
        ready_tx,
        faults,
    } = input;
    let mut errors = vec![];
    let mut packets = vec![];
    let mut summary = None;
    let mut ready = None;
    let mut next = 0u64;
    let mut idle_spans = vec![];
    let mut notices = vec![];
    let (events_tx, events_rx) = mpsc::sync_channel(64);
    if let Err(e) = spawn_selected_endpoint_capture_thread(
        EndpointSelection::ExactId(reg.endpoint_id.clone().unwrap()),
        stop.clone(),
        events_tx,
        private.clone(),
        2000,
        Some(producer),
    ) {
        poison(
            &mut errors,
            &mut writer,
            &stop,
            format!("native setup: {e}"),
        );
    }
    let mut stopped_at = None;
    loop {
        if let Some(cause) = source_fault(&faults, &reg.id) {
            if errors.is_empty() {
                notices.push(json!({"class":"coordinatorFault","fault":cause}));
                poison(&mut errors, &mut writer, &stop, cause.reason);
            }
        }
        for event in events_rx.try_iter() {
            match event {
                CaptureEvent::StreamFailed { error, .. } => poison(
                    &mut errors,
                    &mut writer,
                    &stop,
                    format!("native event: {error}"),
                ),
                CaptureEvent::Level { .. } => {}
                _ => poison(
                    &mut errors,
                    &mut writer,
                    &stop,
                    "legacy media event on exact-frame source".into(),
                ),
            }
        }
        if ready.is_none() {
            if let (Some(clock), Some(observed), Some(witness)) =
                (rx.clock(), rx.ready_observed_hns(), rx.prebarrier_witness())
            {
                let value = json!({"sourceId":reg.id,"endpointId":reg.endpoint_id,"sampleRate":RATE,"commonOriginHns":clock.common_origin_hns,"nativeOriginHns":clock.native_origin_hns,"cutFrame":clock.cut_frame,"phaseNumerator":clock.phase_numerator,"readyObservedHns":observed,"prebarrierWitness":{"start":witness.native_frames.start,"end":witness.native_frames.end,"nativeQpcHns":witness.native_qpc_hns,"observedQpcHns":witness.observed_qpc_hns,"nativeFlags":witness.native_flags}});
                let _ = ready_tx.send(value.clone());
                ready = Some(value);
            }
        }
        if stop.load(Ordering::SeqCst) && stopped_at.is_none() {
            stopped_at = Some(Instant::now());
        }
        if stopped_at.is_some_and(|s: Instant| s.elapsed() > Duration::from_secs(15)) {
            poison(
                &mut errors,
                &mut writer,
                &stop,
                "native drain timeout; completion forbidden".into(),
            );
            break;
        }
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(SinkRead::Packet(p)) => {
                let index = packets.len() as u64;
                let valid = p.scope.source_id().to_string() == reg.id
                    && p.scope.epoch() == reg.epoch
                    && p.scope.endpoint_id() == reg.endpoint_id.as_deref().unwrap()
                    && p.scope.sample_rate() == RATE as u32
                    && p.scope.common_origin_hns().to_string() == reg.clock_origin_qpc_hns
                    && p.frames.end.checked_sub(p.frames.start) == Some(p.samples.len() as u64)
                    && !p.samples.is_empty()
                    && p.samples.len() <= RATE as usize * 2
                    && p.samples.capacity() <= RATE as usize * 2
                    && p.original_native_frames
                        .end
                        .checked_sub(p.original_native_frames.start)
                        == p.preroll_clipped_frames.checked_add(p.samples.len() as u64)
                    && rx.clock().is_some_and(|clock| {
                        p.original_native_frames
                            .start
                            .checked_add(p.preroll_clipped_frames)
                            .and_then(|start| start.checked_sub(clock.cut_frame))
                            == Some(p.frames.start)
                    })
                    && p.frames.end <= RATE as u64 * 86400;
                if !valid {
                    poison(
                        &mut errors,
                        &mut writer,
                        &stop,
                        "invalid native packet scope/span; no private spool permission".into(),
                    );
                    continue;
                }
                let meta = json!({"sourceId":reg.id,"epoch":reg.epoch,"endpointId":reg.endpoint_id,"startFrame":p.frames.start,"endFrame":p.frames.end,"nativeStartFrame":p.original_native_frames.start,"nativeEndFrame":p.original_native_frames.end,"prerollClippedFrames":p.preroll_clipped_frames,"missingBefore":span(p.missing_before),"knownIdleBefore":span(p.known_idle_before),"nativeIndex":p.native_stamp.native_index,"nativeQpcHns":p.native_stamp.native_qpc_hns,"observedQpcHns":p.native_stamp.observed_qpc_hns,"nativeFlags":p.native_stamp.native_flags});
                let metadata_hash = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&meta).expect("bounded JSON metadata"))
                );
                let write = evidence.append_pcm(&p.samples);
                let mut offset = evidence.offset;
                let mut length = 0;
                let mut pcm_hash = String::new();
                let mut disposition = "privateOrphanAfterFault".to_string();
                match write {
                    Err(e) => {
                        poison(
                            &mut errors,
                            &mut writer,
                            &stop,
                            format!("raw evidence persistence failed: {e}"),
                        );
                        disposition = "privatePersistenceFailed".into();
                    }
                    Ok((o, l, h)) => {
                        offset = o;
                        length = l;
                        pcm_hash = h;
                    }
                }
                if errors.is_empty() && p.missing_before.is_some() {
                    poison(
                        &mut errors,
                        &mut writer,
                        &stop,
                        "native missing interval".into(),
                    );
                }
                if errors.is_empty() && p.frames.start > next {
                    if let Some(idle) = p.known_idle_before.filter(|span| {
                        span.start == next
                            && span.end == p.frames.start
                            && rx.prebarrier_witness().is_some()
                    }) {
                        idle_spans.push((idle.start, idle.end));
                        // Bounded exact idle packets are separate from actual native PCM.
                        while next < idle.end && errors.is_empty() {
                            let count = (idle.end - next).min(RATE as u64 * 2);
                            let result = admit(
                                &mut writer,
                                SourcePcmPacket {
                                    source_id: reg.id.clone(),
                                    epoch: reg.epoch,
                                    first_frame: next as i64,
                                    pcm: vec![0; count as usize],
                                },
                                &reg,
                            );
                            if let Err(e) = result {
                                poison(
                                    &mut errors,
                                    &mut writer,
                                    &stop,
                                    format!("idle admission: {e}"),
                                );
                            } else {
                                next += count;
                            }
                        }
                    } else {
                        poison(
                            &mut errors,
                            &mut writer,
                            &stop,
                            "unverified native start gap; no fabricated zeros".into(),
                        );
                    }
                }
                if errors.is_empty() && p.frames.start != next {
                    poison(
                        &mut errors,
                        &mut writer,
                        &stop,
                        "native frame overlap".into(),
                    );
                }
                if errors.is_empty() {
                    let result = admit(
                        &mut writer,
                        SourcePcmPacket {
                            source_id: reg.id.clone(),
                            epoch: reg.epoch,
                            first_frame: p.frames.start as i64,
                            pcm: p.samples,
                        },
                        &reg,
                    );
                    match result {
                        Ok(d) => {
                            disposition = d.into();
                            next = p.frames.end;
                        }
                        Err(e) => {
                            disposition = e.clone();
                            poison(&mut errors, &mut writer, &stop, e);
                        }
                    }
                }
                let receipt = PacketReceipt {
                    index,
                    frames_start: p.frames.start,
                    frames_end: p.frames.end,
                    pcm_sha256: pcm_hash,
                    file: evidence.pcm_path.clone(),
                    byte_offset: offset,
                    byte_length: length,
                    disposition,
                    metadata: meta,
                    metadata_sha256: metadata_hash,
                };
                if let Err(e) = evidence.append_receipt(&receipt) {
                    poison(
                        &mut errors,
                        &mut writer,
                        &stop,
                        format!("raw metadata append failed: {e}"),
                    );
                }
                packets.push(receipt);
            }
            Ok(SinkRead::Drained(s)) => {
                if s.quarantined {
                    poison(
                        &mut errors,
                        &mut writer,
                        &stop,
                        "sink summary quarantined".into(),
                    );
                }
                summary = Some(s);
                break;
            }
            Ok(notice) => {
                notices.push(notice_value(&notice));
                poison(
                    &mut errors,
                    &mut writer,
                    &stop,
                    format!("native sink notice: {notice:?}"),
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                poison(
                    &mut errors,
                    &mut writer,
                    &stop,
                    "sink disconnected without Drained".into(),
                );
                break;
            }
        }
    }
    // Drain a terminal error send after summary publication; channel disconnect
    // is required to prove the native closure completed, not just its sink.
    let event_deadline = Instant::now() + Duration::from_secs(2);
    let mut native_terminated = false;
    while Instant::now() < event_deadline {
        match events_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(CaptureEvent::StreamFailed { error, .. }) => poison(
                &mut errors,
                &mut writer,
                &stop,
                format!("terminal event: {error}"),
            ),
            Ok(CaptureEvent::Level { .. }) => {}
            Ok(_) => poison(
                &mut errors,
                &mut writer,
                &stop,
                "legacy terminal media event".into(),
            ),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                native_terminated = true;
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() >= event_deadline {
                    poison(
                        &mut errors,
                        &mut writer,
                        &stop,
                        "native event sender termination unproven".into(),
                    );
                }
            }
        }
    }
    if !native_terminated {
        poison(
            &mut errors,
            &mut writer,
            &stop,
            "native thread termination unproven; closure blocked".into(),
        );
    }
    if let Some(s) = &summary {
        if s.accepted_packets != packets.len() as u64 {
            poison(
                &mut errors,
                &mut writer,
                &stop,
                "accepted packet reconciliation mismatch".into(),
            );
        }
    }
    // Diagnostic streams were buffered during capture, not durably published.
    // Both flush+sync barriers must succeed before reconciliation/completion.
    let evidence_synced = match evidence.sync() {
        Ok(()) => true,
        Err(e) => {
            poison(
                &mut errors,
                &mut writer,
                &stop,
                format!("raw/metadata final flush+sync failed: {e}"),
            );
            false
        }
    };
    let metadata_path = evidence.metadata_path.clone();
    drop(evidence);
    let report = writer.finish();
    let mut result = SourceResult {
        source: reg.id.clone(),
        registration: reg,
        writer: report,
        summary,
        packets,
        errors,
        ready,
        native_terminated,
        idle_spans,
        evidence_synced,
        metadata_path,
        notices,
        diagnostics_persisted: true,
    };
    // Persist the first native/writer/guard causes inside the worker, before
    // returning to any fallible coordinator DB query or qualification step.
    if let Err(error) = durable_json(
        &private.join("source-finished.json"),
        &source_diagnostic(&result),
    ) {
        result.diagnostics_persisted = false;
        result
            .errors
            .push(format!("source diagnostic persistence failed: {error}"));
    }
    result
}
const CHUNK_QUERY_PAGE: u32 = 256;
fn load_capture_chunks(storage: &Storage, recording: &str) -> R<Vec<Value>> {
    let mut rows = vec![];
    let mut offset = 0;
    loop {
        let page = ga::query_page(
            storage,
            "capture_source_chunks",
            &["source_id", "chunk_json", "payload_hash", "byte_size"],
            vec![genesis_block_native::RelationalFilter::equal(
                "capture_source_chunks.recording_id",
                json!(recording),
            )],
            CHUNK_QUERY_PAGE,
            offset,
        )?;
        let full = page.len() == CHUNK_QUERY_PAGE as usize;
        rows.extend(page);
        if rows.len() > 4096 {
            return Err("private chunk query row bound exceeded".into());
        }
        if !full {
            return Ok(rows);
        }
        offset = offset
            .checked_add(CHUNK_QUERY_PAGE)
            .ok_or("chunk query offset overflow")?;
    }
}

fn read_packet(packet: &PacketReceipt) -> R<Vec<u8>> {
    if packet.byte_length == 0 || packet.byte_length > RATE as u64 * 4 {
        return Err("raw packet byte bound exceeded".into());
    }
    let mut file = fs::File::open(&packet.file).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(packet.byte_offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![0; packet.byte_length as usize];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
fn read_receipt(reader: &mut BufReader<fs::File>) -> R<Option<Value>> {
    let mut line = Vec::new();
    let count = reader
        .take(MAX_PLAN + 1)
        .read_until(b'\n', &mut line)
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(None);
    }
    if line.len() > MAX_PLAN as usize || line.last() != Some(&b'\n') {
        return Err("packet JSONL line bound/truncation failure".into());
    }
    serde_json::from_slice(&line)
        .map(Some)
        .map_err(|e| e.to_string())
}
fn qualify_source(r: &SourceResult, rows: &[Value]) -> R<Value> {
    if !r.evidence_synced {
        return Err("raw and metadata final sync barrier failed".into());
    }
    let mut lines = BufReader::new(fs::File::open(&r.metadata_path).map_err(|e| e.to_string())?);
    let mut raw_end = 0u64;
    // Verify each retained raw payload even for partial/quarantined output.
    for packet in &r.packets {
        if format!("{:x}", Sha256::digest(read_packet(packet)?)) != packet.pcm_sha256 {
            return Err("raw packet hash mismatch".into());
        }
        if packet.byte_offset != raw_end
            || packet.byte_length != (packet.frames_end - packet.frames_start) * 2
        {
            return Err("raw packet frame/byte mismatch".into());
        }
        raw_end = raw_end
            .checked_add(packet.byte_length)
            .ok_or("raw offset overflow")?;
        if packet.metadata_sha256
            != format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&packet.metadata).map_err(|e| e.to_string())?)
            )
        {
            return Err("raw metadata hash mismatch".into());
        }
        let persisted = read_receipt(&mut lines)?.ok_or("missing packet JSONL receipt")?;
        if persisted != serde_json::to_value(packet).map_err(|e| e.to_string())? {
            return Err("raw metadata content mismatch".into());
        }
    }
    if read_receipt(&mut lines)?.is_some() {
        return Err("extra packet metadata receipt".into());
    }
    if let Some(last) = r.packets.last() {
        if fs::metadata(&last.file).map_err(|e| e.to_string())?.len() != raw_end {
            return Err("raw diagnostic stream length mismatch".into());
        }
    }
    if !r.errors.is_empty() || r.writer.first_failure.is_some() {
        return Ok(
            json!({"rawHashesVerified":true,"healthyPcmComparison":"not applicable: source quarantined"}),
        );
    }
    let mut expected = Sha256::new();
    let mut expected_frames = 0u64;
    for packet in &r.packets {
        if packet.disposition != "writerAccepted" {
            return Err("healthy source has unadmitted accepted packet".into());
        }
        if packet.frames_start > expected_frames {
            if !r
                .idle_spans
                .contains(&(expected_frames, packet.frames_start))
            {
                return Err("healthy source gap has no exact idle receipt".into());
            }
            for _ in expected_frames..packet.frames_start {
                expected.update([0u8, 0]);
            }
            expected_frames = packet.frames_start;
        }
        if expected_frames != packet.frames_start {
            return Err("raw source interval overlap".into());
        }
        expected.update(read_packet(packet)?);
        expected_frames = packet.frames_end;
    }
    let mut chunks = rows
        .iter()
        .filter(|row| row["capture_source_chunks.source_id"] == r.source)
        .map(|row| {
            serde_json::from_value::<custody::CaptureSourceChunk>(
                row["capture_source_chunks.chunk_json"].clone(),
            )
            .map_err(|e| e.to_string())
        })
        .collect::<R<Vec<_>>>()?;
    chunks.sort_by_key(|chunk| chunk.sequence);
    let mut actual = Sha256::new();
    let mut actual_frames = 0u64;
    for (index, chunk) in chunks.iter().enumerate() {
        if chunk.sequence != index as i64 + 1
            || chunk.start_frame != actual_frames as i64
            || chunk.source_id != r.source
            || chunk.epoch != 1
            || file_hash(&chunk.path)? != chunk.checksum
        {
            return Err("chunk authority/hash/sequence mismatch".into());
        }
        let mut reader = hound::WavReader::open(&chunk.path).map_err(|e| e.to_string())?;
        let spec = reader.spec();
        if spec.channels != 1
            || spec.sample_rate != RATE as u32
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
        {
            return Err("source WAV format mismatch".into());
        }
        for sample in reader.samples::<i16>() {
            actual.update(sample.map_err(|e| e.to_string())?.to_le_bytes());
            actual_frames += 1;
        }
        if actual_frames != chunk.end_frame as u64 {
            return Err("chunk exact frame count mismatch".into());
        }
    }
    let expected_hash = format!("{:x}", expected.finalize());
    let actual_hash = format!("{:x}", actual.finalize());
    if actual_frames == 0
        || actual_frames != expected_frames
        || actual_frames != r.writer.durable_frames
        || chunks.len() as u64 != r.writer.durable_chunks
        || expected_hash != actual_hash
    {
        return Err("native raw + verified idle differs from durable writer PCM".into());
    }
    Ok(
        json!({"rawHashesVerified":true,"healthyPcmComparison":"exact","frames":actual_frames,"nativeAndIdleSha256":expected_hash,"durablePcmSha256":actual_hash,"chunks":chunks.len()}),
    )
}
fn sot_snapshot(storage: &Storage, project: &str, recording: &str) -> R<Value> {
    let mut tables = serde_json::Map::new();
    for (table, key, scope) in [
        ("audio_chunks", "recording_id", recording),
        ("capture_source_chunks", "recording_id", recording),
        ("capture_sources", "recording_id", recording),
        ("mobile_recording_checkpoints", "recording_id", recording),
        ("projects", "id", project),
        ("recordings", "id", recording),
    ] {
        let columns = ga::schema()
            .tables
            .into_iter()
            .find(|t| t.name == table)
            .ok_or("snapshot table missing")?
            .columns
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>();
        let refs = columns.iter().map(String::as_str).collect::<Vec<_>>();
        let mut rows = vec![];
        let mut offset = 0;
        loop {
            let page = ga::query_page(
                storage,
                table,
                &refs,
                vec![genesis_block_native::RelationalFilter::equal(
                    &format!("{table}.{key}"),
                    json!(scope),
                )],
                256,
                offset,
            )?;
            let full = page.len() == 256;
            rows.extend(page);
            if rows.len() > 4096 {
                return Err("private snapshot row bound exceeded".into());
            }
            if !full {
                break;
            }
            offset += 256;
        }
        let pk = format!("{table}.id");
        let mut ids = HashSet::new();
        for row in &rows {
            let id = row
                .get(&pk)
                .and_then(Value::as_str)
                .ok_or("snapshot primary key missing")?;
            if !ids.insert(id.to_owned()) {
                return Err("duplicate scoped snapshot primary key".into());
            }
        }
        rows.sort_by(|a, b| a[&pk].as_str().cmp(&b[&pk].as_str()));
        if serde_json::to_vec(&rows).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            return Err("private snapshot byte bound exceeded".into());
        }
        tables.insert(table.into(), json!({"count":rows.len(),"rows":rows}));
    }
    Ok(
        json!({"projectId":project,"recordingId":recording,"scopeClaim":"exact scoped owner rows sorted by table and primary key; not full profile or internal snapshot bytes","tables":tables}),
    )
}

#[test]
#[ignore = "explicit owned two-endpoint native capture; fresh private root and plan required"]
fn owned_two_endpoint_exact_frame_custody_probe() -> R<()> {
    let c = config()?;
    let initial = armed(&c.plan)?;
    fs::create_dir(&c.root).map_err(|e| e.to_string())?;
    durable_json(
        &c.root.join("input-receipt.json"),
        &json!({"inputSha256":c.plan_hash,"plan":c.plan,"initialAdmission":initial,"scope":"native probe only; no installed/UI/PID/release claim"}),
    )?;
    let storage = open_store(&c.root)?;
    let (project, recording, regs, origin) = setup(&c, &storage, &initial)?;
    let mut group =
        CaptureSourceWriterGroup::new(storage.clone(), project.clone(), recording.clone())?;
    let stop = Arc::new(AtomicBool::new(false));
    let faults: Faults = Arc::new(Mutex::new(vec![]));
    let (ready_tx, ready_rx) = mpsc::channel();
    let mut cleanup = CaptureCleanup {
        stop: stop.clone(),
        root: c.root.clone(),
        handles: vec![],
        faults: faults.clone(),
    };
    // Prepare every fallible writer/channel before starting any native source.
    let mut prepared = vec![];
    for reg in regs {
        let private = c.root.join(format!("raw-{}", reg.id));
        fs::create_dir(&private).map_err(|e| e.to_string())?;
        let evidence = RawEvidence::create(&private)?;
        let writer = group.spawn(SourceWriterOptions::new(reg.clone()))?;
        let scope = SourcePacketScope::new(
            uuid::Uuid::parse_str(&reg.id).map_err(|e| e.to_string())?,
            reg.epoch,
            reg.endpoint_id.clone().unwrap(),
            RATE as u32,
            origin,
        )
        .map_err(|e| format!("{e:?}"))?;
        let (producer, rx) = sink::packet_channel(scope, 64).map_err(|e| format!("{e:?}"))?;
        prepared.push((reg, writer, rx, producer, private, evidence));
    }
    let preactivation = armed(&c.plan)?;
    durable_json(
        &c.root.join("preactivation.json"),
        &serde_json::to_value(preactivation).map_err(|e| e.to_string())?,
    )?;
    for (reg, writer, rx, producer, private, evidence) in prepared {
        let stop = stop.clone();
        let ready_tx = ready_tx.clone();
        let fault = faults.clone();
        let source = reg.id.clone();
        cleanup.handles.push((
            source,
            thread::spawn(move || {
                source_worker(SourceWorkerInput {
                    reg,
                    writer,
                    rx,
                    producer,
                    stop,
                    private,
                    evidence,
                    ready_tx,
                    faults: fault,
                })
            }),
        ));
    }
    drop(ready_tx);
    let end = origin
        .checked_add(c.duration_ms * 10_000)
        .ok_or("end overflow")?;
    let mut guard_receipts = vec![];
    let mut errors = vec![];
    let mut ready = vec![];
    let mut ready_written = false;
    let mut next_guard = Instant::now();
    let mut previous_guard: Option<Instant> = None;
    loop {
        let now = match qpc() {
            Ok(t) => t,
            Err(e) => {
                register_fault(&faults, None, "recordingClock", e.clone());
                errors.push(e);
                break;
            }
        };
        ready.extend(ready_rx.try_iter());
        if !ready_written && ready.len() == 2 {
            match durable_json(
                &c.root.join("ready.json"),
                &json!({"recordingId":recording,"projectId":project,"commonOriginHns":origin,"sources":ready}),
            ) {
                Ok(()) => ready_written = true,
                Err(e) => {
                    register_fault(&faults, None, "rootEvidence", e.clone());
                    errors.push(e);
                    break;
                }
            }
        }
        if stop.load(Ordering::SeqCst) || now >= end {
            break;
        }
        if now >= origin && !ready_written {
            for route in &c.plan.routes {
                if !ready.iter().any(|v| v["sourceId"] == route.source_id) {
                    register_fault(
                        &faults,
                        Some(&route.source_id),
                        "startupBarrier",
                        "native prebarrier witness not published before deadline".into(),
                    );
                }
            }
            errors
                .push("both native prebarrier witnesses were not published before deadline".into());
            break;
        }
        if Instant::now() >= next_guard {
            let started = Instant::now();
            let since_previous = previous_guard.map(|p| started.duration_since(p).as_micros());
            previous_guard = Some(started);
            let mut source_receipts = vec![];
            // Fresh single-source clones retain exact last validated intent;
            // neither clone adds render PIDs or broadens owned children.
            for route in &c.plan.routes {
                let plan = routing::CaptureRoutePlan {
                    policy_version: c.plan.policy_version,
                    routes: vec![route.clone()],
                };
                let admission = runtime_admission(&plan);
                source_receipts.push(admission.receipt);
                if let Some(error) = admission.error {
                    let e = runtime_failure_message(&error);
                    register_fault(&faults, Some(&route.source_id), "routeAdmission", e.clone());
                    errors.push(format!("source {} admission: {e}", route.source_id));
                }
            }
            guard_receipts.push(json!({"qpcHns":now,"elapsedMicros":started.elapsed().as_micros(),"sincePreviousMicros":since_previous,"cadenceOverrun":since_previous.is_some_and(|us|us>100_000),"sources":source_receipts}));
            if !errors.is_empty() {
                break;
            }
            next_guard = started + Duration::from_millis(100);
        }
        thread::sleep(Duration::from_millis(5));
    }
    // A final fresh identity/endpoint check records target liveness even if a
    // marker is momentarily Waiting during an externally controlled Stop.
    for route in &c.plan.routes {
        let plan = routing::CaptureRoutePlan {
            policy_version: c.plan.policy_version,
            routes: vec![route.clone()],
        };
        let admission = runtime_admission(&plan);
        guard_receipts.push(json!({"final":true,"source":admission.receipt}));
        if let Some(error) = admission.error {
            let e = runtime_failure_message(&error);
            register_fault(
                &faults,
                Some(&route.source_id),
                "finalRouteAdmission",
                e.clone(),
            );
            errors.push(format!("final source {} admission: {e}", route.source_id));
        }
    }
    stop.store(true, Ordering::SeqCst);
    let mut results = vec![];
    for (source, h) in std::mem::take(&mut cleanup.handles) {
        match h.join() {
            Ok(r) => results.push(r),
            Err(_) => {
                register_fault(
                    &faults,
                    Some(&source),
                    "sourceCoordinatorPanic",
                    "source coordinator panic; completion blocked".into(),
                );
                errors.push("source coordinator panic; completion blocked".into());
            }
        }
    }
    // This collection is a durable first-cause boundary. Nothing below may
    // discard it or replace source failures with a secondary SDK error.
    let collected = results.iter().map(source_diagnostic).collect::<Vec<_>>();
    let collected_faults = faults.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let collection_receipt = durable_json(
        &c.root.join("collected-source-diagnostics.json"),
        &json!({"phase":"afterAllSourceJoinsBeforeQueries","healthy":false,"sources":collected,"errors":errors,"coordinatorFaults":collected_faults,"guards":guard_receipts}),
    );
    let mut supplemental_failures = vec![];
    if let Err(error) = &collection_receipt {
        supplemental_failures.push(
            json!({"scope":"apparatus","class":"collectedDiagnosticsPersistence","reason":error}),
        );
        errors.push(format!("collected diagnostics persistence failed: {error}"));
    }
    let chunk_query = load_capture_chunks(&storage, &recording);
    let query_succeeded = chunk_query.is_ok();
    let rows = match &chunk_query {
        Ok(rows) => rows.clone(),
        Err(error) => {
            let failure = json!({"scope":"apparatus","class":"postCaptureQuery","reason":error,"primarySourceDiagnostics":"collected-source-diagnostics.json"});
            supplemental_failures.push(failure.clone());
            errors.push(format!("supplemental postcapture query failure: {error}"));
            let receipt = durable_json(
                &c.root.join("postcapture-query-failure.json"),
                &json!({"failure":failure,"sources":collected,"coordinatorFaults":collected_faults}),
            );
            if let Err(error) = receipt {
                errors.push(format!(
                    "supplemental query diagnostic persistence failed: {error}"
                ));
            }
            vec![]
        }
    };
    let mut qualifications = vec![];
    let mut evidence_intact = query_succeeded && collection_receipt.is_ok();
    for r in &results {
        if !query_succeeded {
            qualifications.push(json!({"sourceId":r.source,"status":"notRun","reason":"postCaptureQueryFailed; source diagnostics retained separately"}));
            continue;
        }
        match qualify_source(r, &rows) {
            Ok(q) => qualifications.push(json!({"sourceId":r.source,"qualification":q})),
            Err(e) => {
                evidence_intact = false;
                register_fault(
                    &faults,
                    Some(&r.source),
                    "sourceEvidenceQualification",
                    e.clone(),
                );
                errors.push(format!("source {} evidence qualification: {e}", r.source));
            }
        }
    }
    // Quarantine exactly the failed source; a global stop is not evidence that
    // an independently healthy source was contaminated. True recording/root
    // failures use explicit sourceId=null and apply to every source.
    for r in &results {
        if let Some(fault) = source_fault(&faults, &r.source) {
            if r.writer.first_failure.is_none() {
                let result = attempt(&storage).and_then(|a| {
                    custody::quarantine_capture_source(
                        &storage,
                        &a,
                        &r.source,
                        &format!("{}:{}", fault.class, fault.reason),
                    )
                });
                if let Err(e) = result {
                    errors.push(format!("coordinator quarantine persistence failed: {e}"));
                }
            }
        }
    }
    let all_drained = evidence_intact
        && results.len() == 2
        && results.iter().all(|r| {
            r.summary.is_some()
                && r.evidence_synced
                && r.diagnostics_persisted
                && r.native_terminated
                && r.writer.drained
                && r.writer.durable_counts_known
                && r.writer.persistence_error.is_none()
                && r.writer.quarantine_error.is_none()
                && (r.writer.first_failure.is_none() || r.writer.quarantine_persisted)
                && r.summary.as_ref().unwrap().accepted_packets == r.packets.len() as u64
        });
    let fault_receipts = faults.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let sources = results.iter().map(source_diagnostic).collect::<Vec<_>>();
    durable_json(
        &c.root.join("precompletion.json"),
        &json!({"allDrained":all_drained,"errors":errors,"supplementalFailures":supplemental_failures,"chunkQuery":chunk_query,"coordinatorFaults":fault_receipts,"sources":sources,"guards":guard_receipts,"qualifications":qualifications}),
    )?;
    let closure_allowed = all_drained
        && !errors
            .iter()
            .any(|e| e.contains("quarantine persistence failed"));
    let completion = if closure_allowed {
        attempt(&storage).and_then(|a| custody::complete_capture_sources(&storage, &a, &recording))
    } else {
        Err("closure blocked; see precompletion.json".into())
    };
    let access = custody::require_capture_source_access(&storage, &recording);
    let source_transport_healthy = all_drained
        && errors.is_empty()
        && results.iter().all(|r| {
            r.errors.is_empty()
                && r.writer.first_failure.is_none()
                && r.summary.as_ref().is_some_and(|s| !s.quarantined)
        });
    // Snapshot actual scoped Genesis rows after the completion attempt. Input
    // registrations cannot substitute for this canonical state export.
    let snapshot = sot_snapshot(&storage, &project, &recording);
    let snapshot_persistence = match &snapshot {
        Ok(value) => durable_json(&c.root.join("canonicalSotSnapshot.json"), value)
            .and_then(|_| file_hash(&c.root.join("canonicalSotSnapshot.json"))),
        Err(e) => Err(e.clone()),
    };
    drop(group);
    drop(storage);
    let (reopen_install, reopen_access, reopen_snapshot) = match open_store(&c.root) {
        Ok(reopened) => {
            let installed = ga::install(&reopened);
            let access = custody::require_capture_source_access(&reopened, &recording);
            let snapshot = sot_snapshot(&reopened, &project, &recording);
            (installed, access, snapshot)
        }
        Err(e) => (Err(e.clone()), Err(e.clone()), Err(e)),
    };
    let stable_rows = match (&snapshot, &reopen_snapshot) {
        (Ok(before), Ok(after)) => before == after,
        _ => false,
    };
    let reopen_snapshot_persistence = match &reopen_snapshot {
        Ok(value) => durable_json(&c.root.join("reopenedCanonicalSotSnapshot.json"), value)
            .and_then(|_| file_hash(&c.root.join("reopenedCanonicalSotSnapshot.json"))),
        Err(e) => Err(e.clone()),
    };
    let reopen_receipt = durable_json(
        &c.root.join("reopen.json"),
        &json!({"install":reopen_install,"access":reopen_access,"stableCanonicalRows":stable_rows,"snapshot":reopen_snapshot,"snapshotSha256":reopen_snapshot_persistence}),
    );
    let final_custody_healthy = source_transport_healthy
        && completion.is_ok()
        && access.is_ok()
        && snapshot_persistence.is_ok()
        && reopen_install.is_ok()
        && reopen_access.is_ok()
        && stable_rows
        && reopen_snapshot_persistence.is_ok()
        && reopen_receipt.is_ok();
    durable_json(
        &c.root.join("report.json"),
        &json!({"recordingId":recording,"projectId":project,"commonOriginHns":origin,"healthy":final_custody_healthy,"sourceTransportHealthy":source_transport_healthy,"finalCustodyHealthy":final_custody_healthy,"completion":completion,"access":access,"canonicalSotSnapshot":snapshot,"canonicalSotSnapshotSha256":snapshot_persistence,"reopen":{"install":reopen_install,"access":reopen_access,"stableCanonicalRows":stable_rows,"snapshotSha256":reopen_snapshot_persistence,"receiptPersistence":reopen_receipt},"sources":sources,"chunks":rows,"chunkQuery":chunk_query,"supplementalFailures":supplemental_failures,"errors":errors,"coordinatorFaults":fault_receipts,"guards":guard_receipts,"qualifications":qualifications}),
    )?;
    if !final_custody_healthy {
        return Err("probe failed/partial; preserved private evidence, no release claim".into());
    }
    Ok(())
}

#[test]
fn probe_environment_requires_two_routes_and_explicit_fresh_authority() {
    let root = Path::new(r"C:\Windows\fung-probe-do-not-create");
    assert!(validate_input(root, b"{}", 3000).is_err());
    assert!(local_unaliased(Path::new(r"\\server\share\probe"), false).is_err());
    assert!(local_unaliased(Path::new(r"G:\AI-Studio\verification\..\probe"), false).is_err());
    assert!(validate_input(
        Path::new(r"G:\AI-Studio\verification\never-created-env-test"),
        b"{}",
        0
    )
    .is_err());
}

fn synthetic_runtime_fixture() -> (routing::CaptureRoutePlan, routing::NativeInventory) {
    use routing::*;
    let app = ApplicationIdentity {
        pid: 10,
        parent_pid: 1,
        creation_filetime: "100".into(),
        image_path: r"C:\owned\marker.exe".into(),
        image_volume_serial: 7,
        image_file_id: "0000000000000010".into(),
    };
    let observer = ApplicationIdentity {
        pid: 999,
        parent_pid: 1,
        creation_filetime: "200".into(),
        image_path: r"C:\owned\probe.exe".into(),
        image_volume_serial: 7,
        image_file_id: "0000000000000999".into(),
    };
    let session = SessionDescriptor {
        instance_id: Observation::Known("owned-exact-instance".into()),
        state: Observation::Known(SessionState::Active),
        owner: Observation::Known(SessionOwner::SingleProcess(app.clone())),
    };
    let selected = EndpointDescriptor {
        id: "opaque-selected".into(),
        name: Observation::Known("selected".into()),
        state: Observation::Known(EndpointState::Active),
        mix_format: Observation::Known(MixFormat {
            rate: 48000,
            channels: 2,
            bits: 32,
            sample_type: "Float".into(),
        }),
        sessions: Observation::Known(vec![session]),
    };
    let mut unrelated = selected.clone();
    unrelated.id = "opaque-unrelated".into();
    unrelated.name = Observation::Known("DO_NOT_EXPORT_UNRELATED_NAME".into());
    unrelated.sessions = Observation::Known(vec![SessionDescriptor {
        instance_id: Observation::Known("DO_NOT_EXPORT_UNRELATED_INSTANCE".into()),
        state: Observation::Known(SessionState::Active),
        owner: Observation::Unavailable("DO_NOT_EXPORT_UNRELATED_OWNER".into()),
    }]);
    (
        CaptureRoutePlan {
            policy_version: ROUTE_POLICY_VERSION,
            routes: vec![CaptureRoute {
                source_id: "ec548fff-976e-40b5-8955-f79633475690".into(),
                label: "owned synthetic route".into(),
                endpoint_id: selected.id.clone(),
                application: app.clone(),
                allowed_children: vec![],
            }],
        },
        NativeInventory {
            captured_at_ms: 100,
            process_snapshot_cutoff_filetime: "1000".into(),
            endpoints: vec![selected, unrelated],
            defaults: Observation::Known(DefaultRoles {
                console: "default-c".into(),
                multimedia: "default-m".into(),
                communications: "default-v".into(),
            }),
            processes: vec![
                ProcessObservation {
                    pid: 10,
                    identity: Observation::Known(app),
                },
                ProcessObservation {
                    pid: 999,
                    identity: Observation::Known(observer),
                },
            ],
            errors: vec![],
        },
    )
}

#[test]
fn probe_runtime_rejection_retains_exact_selected_snapshot_and_hides_unrelated_sessions() {
    use routing::*;
    let (plan, mut snapshot) = synthetic_runtime_fixture();
    let foreign = ApplicationIdentity {
        pid: 77,
        parent_pid: 1,
        creation_filetime: "300".into(),
        image_path: r"C:\foreign\foreign.exe".into(),
        image_volume_serial: 7,
        image_file_id: "0000000000000077".into(),
    };
    let Observation::Known(sessions) = &mut snapshot.endpoints[0].sessions else {
        panic!("fixture")
    };
    sessions.push(SessionDescriptor {
        instance_id: Observation::Known("unknown-exact-instance-at-rejection".into()),
        state: Observation::Known(SessionState::Active),
        owner: Observation::Known(SessionOwner::SingleProcess(foreign)),
    });
    let admission = runtime_snapshot(&plan, &snapshot, 101, 123456, 999);
    assert_eq!(
        admission.error.as_ref().unwrap().code,
        Rejection::UnknownSession
    );
    assert_eq!(admission.receipt["accepted"], false);
    let serialized = serde_json::to_string(&admission.receipt).unwrap();
    assert!(serialized.contains("unknown-exact-instance-at-rejection"));
    assert!(!serialized.contains("DO_NOT_EXPORT_UNRELATED"));
    assert_eq!(
        admission.receipt["sameInputSnapshotProjection"]["capturedAtMs"],
        100
    );
    assert_eq!(admission.receipt["context"]["guardQpcHns"], 123456);
    assert_eq!(
        admission.receipt["sameInputSnapshotProjection"]["observerProcess"]["identity"]["value"]
            ["creationFiletime"],
        "200"
    );
    // Mutating a later inventory cannot retroactively replace the rejection input.
    snapshot.endpoints[0].sessions = Observation::Known(vec![]);
    assert!(serde_json::to_string(&admission.receipt)
        .unwrap()
        .contains("unknown-exact-instance-at-rejection"));
}

#[test]
fn probe_runtime_own_pid_or_reused_allowed_pid_is_not_a_session_exemption() {
    use routing::*;
    for own_pid_case in [true, false] {
        let (plan, mut snapshot) = synthetic_runtime_fixture();
        let mut foreign = if own_pid_case {
            match &snapshot.processes[1].identity {
                Observation::Known(owner) => owner.clone(),
                _ => panic!("fixture"),
            }
        } else {
            plan.routes[0].application.clone()
        };
        if !own_pid_case {
            foreign.creation_filetime = "999".into();
        }
        let Observation::Known(sessions) = &mut snapshot.endpoints[0].sessions else {
            panic!("fixture")
        };
        sessions.push(SessionDescriptor {
            instance_id: Observation::Known("foreign-same-pid-instance".into()),
            state: Observation::Known(SessionState::Active),
            owner: Observation::Known(SessionOwner::SingleProcess(foreign)),
        });
        let admission = runtime_snapshot(&plan, &snapshot, 101, 123456, 999);
        assert_eq!(
            admission.error.as_ref().unwrap().code,
            Rejection::UnknownSession
        );
        assert_eq!(admission.receipt["accepted"], false);
        assert!(
            admission.receipt["validation"].is_null(),
            "rejection cannot yield a validated runtime token"
        );
    }
}

#[test]
fn probe_runtime_diagnostics_preserve_native_wrapper_preflight_limits() {
    let (mut plan, snapshot) = synthetic_runtime_fixture();
    plan.policy_version = 99;
    assert_eq!(
        runtime_snapshot(&plan, &snapshot, 101, 123456, 999)
            .error
            .unwrap()
            .code,
        routing::Rejection::PolicyVersion
    );
    plan.policy_version = routing::ROUTE_POLICY_VERSION;
    plan.routes[0].allowed_children = vec![plan.routes[0].application.clone(); 33];
    assert_eq!(
        runtime_snapshot(&plan, &snapshot, 101, 123456, 999)
            .error
            .unwrap()
            .code,
        routing::Rejection::RouteCount
    );
}

#[test]
fn probe_sdk_chunk_query_respects_actual_genesis_limit_without_losing_prior_diagnostics() {
    // Pure SDK regression: no native inventory, audio activation or rendering.
    let temp = tempfile::tempdir().unwrap();
    let storage = open_store(temp.path()).unwrap();
    ga::install(&storage).unwrap();
    let recording = uuid::Uuid::new_v4().to_string();
    let first = json!({"scope":"syntheticRegressionFixture","errors":["primary startup failure retained"],"writer":{"firstFailure":{"kind":"NativeSourceFailure","detail":"primary startup failure retained"}}});
    let path = temp.path().join("collected-source-diagnostics.json");
    durable_json(&path, &first).unwrap();
    // Retains actual historical RED behavior from the real SDK, not a fake
    // query validator. The positive loader below uses bounded scoped pages.
    let historical = ga::query(
        &storage,
        "capture_source_chunks",
        &["source_id"],
        vec![],
        10000,
    )
    .unwrap_err();
    assert!(
        historical.contains("REL_QUERY_LIMIT_EXCEEDED"),
        "{historical}"
    );
    let rows =
        load_capture_chunks(&storage, &recording).expect("bounded SDK chunk query must succeed");
    assert!(rows.is_empty());
    let persisted: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        persisted, first,
        "secondary query errors must not overwrite prior first-cause diagnostics"
    );
}

#[test]
fn probe_environment_validates_explicit_binary_receipts_without_native_admission() {
    // Reading the running test image is a pure input-validation fixture. This
    // test neither creates the run root nor authorizes any capture process.
    let exe = std::env::current_exe().unwrap();
    let text = exe
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(exe.to_str().unwrap());
    let exe = PathBuf::from(text);
    let pid = std::process::id();
    let owner = json!({"pid":pid,"parentPid":1,"creationFiletime":"1","imagePath":exe,"imageVolumeSerial":1,"imageFileId":"explicit-fixture-only"});
    let route = |endpoint: &str| json!({"sourceId":uuid::Uuid::new_v4().to_string(),"label":"validator only","endpointId":endpoint,"application":owner,"allowedChildren":[]});
    let input = json!({"plan":{"policyVersion":1,"routes":[route("opaque-A"),route("opaque-B")]},"ownedBinaries":[{"pid":pid,"imagePath":exe,"sha256":file_hash(&exe).unwrap(),"creationFiletime":"1"}]});
    let root = PathBuf::from(r"G:\AI-Studio\verification")
        .join(format!("env-only-{}", uuid::Uuid::new_v4()));
    assert!(validate_input(&root, &serde_json::to_vec(&input).unwrap(), 3000).is_ok());
    assert!(!root.exists());
    let mut bad = input.clone();
    bad["plan"]["routes"][1]["endpointId"] = json!("opaque-A");
    assert!(validate_input(&root, &serde_json::to_vec(&bad).unwrap(), 3000).is_err());
    let mut bad = input.clone();
    bad["ownedBinaries"][0]["sha256"] = json!("0".repeat(64));
    assert!(validate_input(&root, &serde_json::to_vec(&bad).unwrap(), 3000).is_err());
    let mut bad = input;
    bad["unexpected"] = json!(true);
    assert!(validate_input(&root, &serde_json::to_vec(&bad).unwrap(), 3000).is_err());
    assert!(!root.exists());
}

#[test]
fn probe_guard_failure_is_scoped_while_recording_failure_is_explicitly_global() {
    let faults: Faults = Arc::new(Mutex::new(vec![]));
    register_fault(
        &faults,
        Some("source-a"),
        "routeAdmission",
        "owned target disappeared".into(),
    );
    assert!(source_fault(&faults, "source-a").is_some());
    assert!(
        source_fault(&faults, "source-b").is_none(),
        "stopping B does not authorize quarantining B"
    );
    register_fault(
        &faults,
        Some("source-a"),
        "later",
        "must preserve first cause".into(),
    );
    assert_eq!(
        source_fault(&faults, "source-a").unwrap().class,
        "routeAdmission"
    );
    register_fault(
        &faults,
        None,
        "rootEvidence",
        "recording root write failed".into(),
    );
    assert_eq!(
        source_fault(&faults, "source-b").unwrap().class,
        "rootEvidence"
    );
}

#[test]
fn probe_buffered_stream_retains_exact_packet_offsets_and_final_tail_after_sync() {
    let temp = tempfile::tempdir().unwrap();
    let mut stream = RawEvidence::create(temp.path()).unwrap();
    let first = stream.append_pcm(&[i16::MIN, -1, 0, i16::MAX]).unwrap();
    let second = stream.append_pcm(&[17]).unwrap();
    assert_eq!((first.0, first.1, second.0, second.1), (0, 8, 8, 2));
    let meta = json!({"missingBefore":null,"knownIdleBefore":{"start":4,"end":5}});
    let receipt = PacketReceipt {
        index: 1,
        frames_start: 4,
        frames_end: 5,
        pcm_sha256: second.2,
        file: stream.pcm_path.clone(),
        byte_offset: second.0,
        byte_length: second.1,
        disposition: "privateOrphanAfterFault".into(),
        metadata_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&meta).unwrap())),
        metadata: meta,
    };
    stream.append_receipt(&receipt).unwrap();
    stream.sync().unwrap();
    assert_eq!(read_packet(&receipt).unwrap(), 17i16.to_le_bytes());
    let pcm = fs::read(&stream.pcm_path).unwrap();
    assert_eq!(pcm.len(), 10);
    assert_eq!(format!("{:x}", Sha256::digest(&pcm[..8])), first.2);
    let persisted: Value =
        serde_json::from_str(fs::read_to_string(&stream.metadata_path).unwrap().trim()).unwrap();
    assert_eq!(persisted, serde_json::to_value(&receipt).unwrap());
}

#[test]
fn probe_startup_evidence_distinguishes_unknown_coverage_from_known_native_lateness() {
    let scope = Arc::new(
        SourcePacketScope::new(uuid::Uuid::new_v4(), 1, "opaque-only".into(), 48000, 10000)
            .unwrap(),
    );
    let unverified = notice_value(&SinkRead::StartupUnverified {
        scope: scope.clone(),
        unverified: sink::StartupUnverified {
            reason: "negative first packet anchor".into(),
            first_observed_hns: Some(10001),
            common_origin_hns: 10000,
        },
    });
    assert_eq!(unverified["class"], "startupUnverified");
    assert!(unverified["missingRange"].is_null());
    let missing = notice_value(&SinkRead::StartupMissing {
        scope,
        missing: sink::StartupMissing {
            elapsed_hns: 1000,
            rounded_frame_span: Some(sink::FrameSpan { start: 0, end: 5 }),
        },
    });
    assert_eq!(missing["class"], "startupMissing");
    assert_eq!(missing["roundedFrameSpan"], json!({"start":0,"end":5}));
}
