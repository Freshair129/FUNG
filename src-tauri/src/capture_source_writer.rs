//! Trusted native exact-frame writer; no capture executor or media publisher.
use crate::capture_source_custody::CaptureSourceRegistration;
use genesis_block_native::Storage;
use std::sync::Arc;
#[derive(Clone, Debug)]
pub struct SourcePcmPacket {
    pub source_id: String,
    pub epoch: i64,
    pub first_frame: i64,
    pub pcm: Vec<i16>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceWriterFailureKind {
    Gap,
    Overlap,
    WrongIdentity,
    PacketTooLarge,
    QueueOverflow,
    Persistence,
    EmptySource,
    WorkerPanic,
    NativeSourceFailure,
}
#[derive(Clone, Debug)]
pub struct SourceWriterFailure {
    pub kind: SourceWriterFailureKind,
    pub detail: String,
    pub expected_frame: Option<i64>,
    pub observed_frame: Option<i64>,
    pub packet_frames: Option<usize>,
}
#[derive(Debug)]
pub struct SourceWriterReport {
    pub source_id: String,
    pub epoch: i64,
    pub durable_chunks: u64,
    pub durable_frames: u64,
    pub first_failure: Option<SourceWriterFailure>,
    pub quarantine_persisted: bool,
    pub drained: bool,
    pub private_orphans: Vec<std::path::PathBuf>,
    pub durable_counts_known: bool,
    pub persistence_error: Option<String>,
    pub quarantine_error: Option<String>,
}
pub struct SourceWriterOptions {
    pub registration: CaptureSourceRegistration,
    pub queue_capacity: usize,
    #[cfg(test)]
    gate: Option<Arc<TestGate>>,
}
impl SourceWriterOptions {
    pub fn new(registration: CaptureSourceRegistration) -> Self {
        Self {
            registration,
            queue_capacity: 64,
            #[cfg(test)]
            gate: None,
        }
    }
}

use crate::capture_source_custody as custody;
use genesis_block_native::{RelationalFilter, RelationalQuery};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const MAX_SLOTS: usize = 64;
const MAX_MS: i64 = 24 * 60 * 60 * 1000;
/// One native coordinator owns this group for a recording; all source commits share its fence.
pub struct CaptureSourceWriterGroup {
    storage: Arc<Storage>,
    project: String,
    recording: String,
    root: PathBuf,
    fence: Arc<Mutex<()>>,
    started: HashSet<String>,
}
#[derive(Default)]
struct FaultState {
    first: Option<SourceWriterFailure>,
    terminal_packet: Option<SourcePcmPacket>,
}
#[derive(Clone)]
struct Context {
    storage: Arc<Storage>,
    project: String,
    recording: String,
    registration: CaptureSourceRegistration,
    dir: PathBuf,
    fence: Arc<Mutex<()>>,
}
pub struct CaptureSourceWriter {
    tx: Option<SyncSender<SourcePcmPacket>>,
    shared: Arc<Mutex<FaultState>>,
    join: Option<JoinHandle<SourceWriterReport>>,
    context: Context,
    next_frame: i64,
    #[cfg(test)]
    gate: Option<Arc<TestGate>>,
}
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}
fn bounded(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1
    }
    value[..end].to_owned()
}
fn fail(kind: SourceWriterFailureKind, detail: impl Into<String>) -> SourceWriterFailure {
    SourceWriterFailure {
        kind,
        detail: bounded(detail.into()),
        expected_frame: None,
        observed_frame: None,
        packet_frames: None,
    }
}
fn first_fault(shared: &Mutex<FaultState>, fault: SourceWriterFailure) {
    let mut s = lock(shared);
    if s.first.is_none() {
        s.first = Some(fault);
    }
}
fn local_ledger_path(path: PathBuf) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        match path.components().next() {
            Some(Component::Prefix(p)) => match p.kind() {
                Prefix::VerbatimDisk(_) => Ok(PathBuf::from(
                    path.to_str()
                        .ok_or("nonUTF8 source path")?
                        .strip_prefix("\\\\?\\")
                        .ok_or("invalid verbatim drive")?,
                )),
                Prefix::Disk(_) => Ok(path),
                _ => Err("source storage must be local drive, not UNC/device namespace".into()),
            },
            _ => Err("absolute local drive required".into()),
        }
    }
    #[cfg(not(windows))]
    {
        Ok(path)
    }
}
fn row(s: &Storage, t: &str, id: &str, columns: &[&str]) -> Result<Value, String> {
    let mut rows = s
        .query_relational(RelationalQuery {
            namespace: crate::genesis_adapter::NAMESPACE.into(),
            table: t.into(),
            columns: columns.iter().map(|c| format!("{t}.{c}")).collect(),
            joins: vec![],
            filters: vec![RelationalFilter::equal(&format!("{t}.id"), json!(id))],
            limit: Some(1),
            offset: None,
        })
        .map_err(|e| e.to_string())?;
    let v = rows.pop().ok_or_else(|| format!("missing {t}"))?;
    Ok(Value::Object(
        v.as_object()
            .ok_or("nonobject native row")?
            .iter()
            .map(|(k, v)| {
                (
                    k.strip_prefix(&format!("{t}.")).unwrap_or(k).into(),
                    v.clone(),
                )
            })
            .collect(),
    ))
}
impl CaptureSourceWriterGroup {
    pub fn new(storage: Arc<Storage>, project: String, recording: String) -> Result<Self, String> {
        let p = row(&storage, "projects", &project, &["storage_path"])?;
        let rec = row(
            &storage,
            "recordings",
            &recording,
            &["project_id", "source", "status"],
        )?;
        if rec["project_id"] != project
            || rec["source"] != "routed_endpoint"
            || rec["status"] != "recording"
        {
            return Err("writer group requires active registered scoped recording".into());
        }
        let root = PathBuf::from(p["storage_path"].as_str().ok_or("missing project path")?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            storage,
            project,
            recording,
            root,
            fence: Arc::new(Mutex::new(())),
            started: HashSet::new(),
        })
    }
    pub fn spawn(&mut self, options: SourceWriterOptions) -> Result<CaptureSourceWriter, String> {
        if options.queue_capacity == 0 || options.queue_capacity > MAX_SLOTS {
            return Err("source queue capacity must be1..64".into());
        }
        let reg = options.registration;
        let id = uuid::Uuid::parse_str(&reg.id).map_err(|e| e.to_string())?;
        if id.to_string() != reg.id
            || reg.epoch <= 0
            || !(8000..=192000).contains(&reg.sample_rate)
            || self.started.contains(&reg.id)
        {
            return Err("invalid or duplicate writer source".into());
        }
        let source = row(
            &self.storage,
            "capture_sources",
            &reg.id,
            &[
                "project_id",
                "recording_id",
                "registration_json",
                "state",
                "closed_at",
                "last_sequence",
            ],
        )?;
        if source["project_id"] != self.project
            || source["recording_id"] != self.recording
            || source["registration_json"] != json!(reg)
            || source["state"] != "registered"
            || !source["closed_at"].is_null()
            || source["last_sequence"] != 0
        {
            return Err("writer source is not a fresh exact registered identity".into());
        }
        let dir = self
            .root
            .join("live")
            .join(&self.recording)
            .join("sources")
            .join(&reg.id)
            .join(format!("epoch-{}", reg.epoch));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        if dir.canonicalize().map_err(|e| e.to_string())? != dir {
            return Err("source directory alias escapes authority".into());
        }
        let context = Context {
            storage: self.storage.clone(),
            project: self.project.clone(),
            recording: self.recording.clone(),
            registration: reg.clone(),
            dir: local_ledger_path(dir)?,
            fence: self.fence.clone(),
        };
        let shared = Arc::new(Mutex::new(FaultState::default()));
        let (tx, rx) = mpsc::sync_channel(options.queue_capacity);
        let c = context.clone();
        let sh = shared.clone();
        #[cfg(test)]
        let gate = options.gate.clone();
        let join = thread::Builder::new()
            .name(format!("source-writer-{}", reg.id))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_worker(
                        c.clone(),
                        rx,
                        sh.clone(),
                        #[cfg(test)]
                        gate,
                    )
                }));
                match result {
                    Ok(report) => report,
                    Err(_) => {
                        first_fault(
                            &sh,
                            fail(
                                SourceWriterFailureKind::WorkerPanic,
                                "source writer panicked; durability counts unknown",
                            ),
                        );
                        let mut report = empty_report(&c);
                        report.durable_counts_known = false;
                        report.drained = false;
                        report.first_failure = lock(&sh).first.clone();
                        let _ = ensure_quarantine(&c, &sh, &mut report);
                        report
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        self.started.insert(reg.id);
        Ok(CaptureSourceWriter {
            tx: Some(tx),
            shared,
            join: Some(join),
            context,
            next_frame: 0,
            #[cfg(test)]
            gate: options.gate,
        })
    }
}
impl CaptureSourceWriter {
    /// Native failure signal only; coordinator must stop producer. Worker persists quarantine.
    pub fn signal_failure(&mut self, detail: &str) {
        let mut f = fail(SourceWriterFailureKind::NativeSourceFailure, detail);
        f.expected_frame = Some(self.next_frame);
        first_fault(&self.shared, f);
        self.tx.take();
    }
    /// Bounded copy admission only: no filesystem or Genesis calls run on this producer method.
    pub fn try_push(&mut self, packet: SourcePcmPacket) -> Result<(), SourceWriterFailure> {
        let mut state = lock(&self.shared);
        if let Some(f) = state.first.clone() {
            self.tx.take();
            return Err(f);
        }
        let reg = &self.context.registration;
        let size = packet.pcm.len();
        let valid_id = packet.source_id == reg.id && packet.epoch == reg.epoch;
        let reason = if !valid_id {
            Some(fail(
                SourceWriterFailureKind::WrongIdentity,
                "packet does not belong to bound source/epoch",
            ))
        } else if size == 0
            || size > reg.sample_rate as usize * 2
            || packet.pcm.capacity() > reg.sample_rate as usize * 2
        {
            Some(fail(
                SourceWriterFailureKind::PacketTooLarge,
                "packet must contain1frame..2seconds of registered monoPCM16",
            ))
        } else if packet.first_frame < 0
            || packet
                .first_frame
                .checked_add(size as i64)
                .filter(|end| *end <= MAX_MS * reg.sample_rate / 1000)
                .is_none()
        {
            Some(fail(
                SourceWriterFailureKind::PacketTooLarge,
                "packet frame/duration bound exceeded",
            ))
        } else if packet.first_frame < self.next_frame {
            Some(fail(
                SourceWriterFailureKind::Overlap,
                format!(
                    "overlap expected{} got{}",
                    self.next_frame, packet.first_frame
                ),
            ))
        } else if packet.first_frame > self.next_frame {
            Some(fail(
                SourceWriterFailureKind::Gap,
                format!(
                    "missing common-origin frames{}..{}",
                    self.next_frame, packet.first_frame
                ),
            ))
        } else {
            None
        };
        if let Some(mut f) = reason {
            f.expected_frame = Some(self.next_frame);
            f.observed_frame = Some(packet.first_frame);
            f.packet_frames = Some(size);
            if valid_id
                && size > 0
                && size <= reg.sample_rate as usize * 2
                && packet.pcm.capacity() <= reg.sample_rate as usize * 2
                && packet.first_frame >= self.next_frame
                && packet
                    .first_frame
                    .checked_add(size as i64)
                    .filter(|end| *end <= MAX_MS * reg.sample_rate / 1000)
                    .is_some()
            {
                state.terminal_packet = Some(packet)
            }
            state.first = Some(f.clone());
            self.tx.take();
            return Err(f);
        }
        let end = packet.first_frame + size as i64;
        match self
            .tx
            .as_ref()
            .expect("sender present without terminal state")
            .try_send(packet)
        {
            Ok(()) => {
                self.next_frame = end;
                Ok(())
            }
            Err(TrySendError::Full(packet)) => {
                let mut f = fail(
                    SourceWriterFailureKind::QueueOverflow,
                    format!(
                        "bounded source queue full at frame{}; native source must stop",
                        packet.first_frame
                    ),
                );
                f.expected_frame = Some(self.next_frame);
                f.observed_frame = Some(packet.first_frame);
                f.packet_frames = Some(size);
                state.terminal_packet = Some(packet);
                state.first = Some(f.clone());
                self.tx.take();
                Err(f)
            }
            Err(TrySendError::Disconnected(_)) => {
                let f = fail(
                    SourceWriterFailureKind::Persistence,
                    "source writer disconnected",
                );
                state.first = Some(f.clone());
                self.tx.take();
                Err(f)
            }
        }
    }
    /// Call only after native producer stop; drains accepted PCM. Coordinator closes ALL sources later.
    pub fn finish(mut self) -> SourceWriterReport {
        self.tx.take();
        #[cfg(test)]
        if let Some(g) = &self.gate {
            g.release();
        }
        self.join
            .take()
            .expect("writer join available")
            .join()
            .unwrap_or_else(|_| {
                let mut r = empty_report(&self.context);
                r.drained = false;
                r.durable_counts_known = false;
                r.first_failure = Some(fail(
                    SourceWriterFailureKind::WorkerPanic,
                    "writer join panic",
                ));
                r
            })
    }
}
impl Drop for CaptureSourceWriter {
    fn drop(&mut self) {
        self.tx.take();
        #[cfg(test)]
        if let Some(g) = &self.gate {
            g.release();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
fn empty_report(c: &Context) -> SourceWriterReport {
    SourceWriterReport {
        source_id: c.registration.id.clone(),
        epoch: c.registration.epoch,
        durable_chunks: 0,
        durable_frames: 0,
        first_failure: None,
        quarantine_persisted: false,
        drained: true,
        private_orphans: vec![],
        durable_counts_known: true,
        persistence_error: None,
        quarantine_error: None,
    }
}
fn attempt(c: &Context) -> Result<custody::SourceCommitAttempt, String> {
    custody::begin_source_commit(
        &c.storage,
        &uuid::Uuid::new_v4().to_string(),
        &chrono::Utc::now().to_rfc3339(),
    )
}
fn ensure_quarantine(
    c: &Context,
    shared: &Mutex<FaultState>,
    report: &mut SourceWriterReport,
) -> bool {
    if report.quarantine_persisted {
        return true;
    }
    if report.quarantine_error.is_some() {
        return false;
    }
    let f = lock(shared).first.clone();
    let Some(f) = f else { return true };
    let reason = bounded(format!("writer_{:?}:{}", f.kind, f.detail));
    let result = (|| {
        let _guard = lock(&c.fence);
        let a = attempt(c)?;
        custody::quarantine_capture_source(&c.storage, &a, &c.registration.id, &reason).or_else(
            |_| custody::quarantine_capture_source(&c.storage, &a, &c.registration.id, &reason),
        )
    })();
    match result {
        Ok(()) => {
            report.quarantine_persisted = true;
            true
        }
        Err(error) => {
            report.quarantine_error = Some(bounded(&error));
            false
        }
    }
}
struct Assembler {
    first: Option<i64>,
    pcm: Vec<i16>,
    next_frame: i64,
    sequence: i64,
    ledger_disabled: bool,
}
fn write_finalized(
    c: &Context,
    sequence: i64,
    start: i64,
    pcm: &[i16],
) -> Result<custody::CaptureSourceChunk, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let path = c.dir.join(format!("chunk-{sequence:08}-{id}.wav"));
    let temporary = path.with_extension("wav.part");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let mut wav = hound::WavWriter::new(
        std::io::BufWriter::with_capacity(65536, file),
        hound::WavSpec {
            channels: 1,
            sample_rate: c.registration.sample_rate as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|e| e.to_string())?;
    for sample in pcm {
        wav.write_sample(*sample).map_err(|e| e.to_string())?
    }
    wav.finalize().map_err(|e| e.to_string())?;
    fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    if path.exists() {
        return Err("unexpected final source filename collision".into());
    }
    fs::rename(&temporary, &path).map_err(|e| e.to_string())?;
    let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    let end = start + pcm.len() as i64;
    Ok(custody::CaptureSourceChunk {
        id,
        source_id: c.registration.id.clone(),
        epoch: c.registration.epoch,
        sequence,
        path,
        start_frame: start,
        end_frame: end,
        start_ms: start * 1000 / c.registration.sample_rate,
        end_ms: end * 1000 / c.registration.sample_rate,
        checksum: format!("{:x}", hash.finalize()),
    })
}
fn flush(
    c: &Context,
    shared: &Mutex<FaultState>,
    a: &mut Assembler,
    report: &mut SourceWriterReport,
) {
    if a.pcm.is_empty() {
        return;
    }
    let start = a.first.take().expect("nonempty source assembly has origin");
    let pcm = std::mem::replace(
        &mut a.pcm,
        Vec::with_capacity(c.registration.sample_rate as usize * 2),
    );
    let next = a.sequence + 1;
    if !ensure_quarantine(c, shared, report) {
        a.ledger_disabled = true
    }
    match write_finalized(c, next, start, &pcm) {
        Err(error) => {
            report
                .persistence_error
                .get_or_insert_with(|| bounded(&error));
            first_fault(shared, fail(SourceWriterFailureKind::Persistence, error));
            a.ledger_disabled = true;
            let _ = ensure_quarantine(c, shared, report);
        }
        Ok(chunk) => {
            if a.ledger_disabled {
                report.private_orphans.push(chunk.path);
                return;
            }
            let result = (|| {
                let _guard = lock(&c.fence);
                let attempt = attempt(c)?;
                custody::append_capture_source_chunk(
                    &c.storage,
                    &attempt,
                    &c.project,
                    &c.recording,
                    &chunk,
                )
                .or_else(|_| {
                    custody::append_capture_source_chunk(
                        &c.storage,
                        &attempt,
                        &c.project,
                        &c.recording,
                        &chunk,
                    )
                })
            })();
            match result {
                Ok(_) => {
                    a.sequence = next;
                    report.durable_chunks += 1;
                    report.durable_frames += pcm.len() as u64;
                }
                Err(error) => {
                    // An opaque failed commit AND failed same-attempt reconciliation leave
                    // total durable state uncertain. Counters remain confirmed lower bounds.
                    report.durable_counts_known = false;
                    report.persistence_error = Some(bounded(&error));
                    first_fault(shared, fail(SourceWriterFailureKind::Persistence, error));
                    a.ledger_disabled = true;
                    report.private_orphans.push(chunk.path);
                    let _ = ensure_quarantine(c, shared, report);
                }
            }
        }
    }
}
fn consume(
    c: &Context,
    shared: &Mutex<FaultState>,
    a: &mut Assembler,
    report: &mut SourceWriterReport,
    packet: SourcePcmPacket,
) {
    if packet.first_frame != a.next_frame {
        flush(c, shared, a, report);
        if packet.first_frame < a.next_frame {
            return;
        }
        a.next_frame = packet.first_frame;
    }
    let width = c.registration.sample_rate as usize * 2;
    let mut offset = 0;
    while offset < packet.pcm.len() {
        if a.first.is_none() {
            a.first = Some(packet.first_frame + offset as i64)
        }
        let count = (width - a.pcm.len()).min(packet.pcm.len() - offset);
        a.pcm.extend_from_slice(&packet.pcm[offset..offset + count]);
        offset += count;
        a.next_frame += count as i64;
        if a.pcm.len() == width {
            flush(c, shared, a, report);
        }
    }
}
fn run_worker(
    c: Context,
    rx: Receiver<SourcePcmPacket>,
    shared: Arc<Mutex<FaultState>>,
    #[cfg(test)] gate: Option<Arc<TestGate>>,
) -> SourceWriterReport {
    #[cfg(test)]
    if let Some(g) = gate {
        let mut open = lock(&g.open);
        while !*open {
            open = g.cv.wait(open).unwrap_or_else(|p| p.into_inner());
        }
    }
    let mut report = empty_report(&c);
    let mut a = Assembler {
        first: None,
        pcm: Vec::with_capacity(c.registration.sample_rate as usize * 2),
        next_frame: 0,
        sequence: 0,
        ledger_disabled: false,
    };
    loop {
        if lock(&shared).first.is_some() {
            let _ = ensure_quarantine(&c, &shared, &mut report);
            while let Ok(packet) = rx.try_recv() {
                consume(&c, &shared, &mut a, &mut report, packet)
            }
            break;
        }
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(packet) => consume(&c, &shared, &mut a, &mut report, packet),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let terminal = lock(&shared).terminal_packet.take();
    if let Some(packet) = terminal {
        let _ = ensure_quarantine(&c, &shared, &mut report);
        consume(&c, &shared, &mut a, &mut report, packet)
    }
    if a.next_frame == 0 && lock(&shared).first.is_none() {
        first_fault(
            &shared,
            fail(
                SourceWriterFailureKind::EmptySource,
                "source closed without any PCM",
            ),
        );
    }
    flush(&c, &shared, &mut a, &mut report);
    let _ = ensure_quarantine(&c, &shared, &mut report);
    report.first_failure = lock(&shared).first.clone();
    report
}
#[cfg(test)]
#[derive(Default)]
struct TestGate {
    open: std::sync::Mutex<bool>,
    cv: std::sync::Condvar,
}
#[cfg(test)]
impl TestGate {
    fn release(&self) {
        *lock(&self.open) = true;
        self.cv.notify_all();
    }
}
#[cfg(test)]
#[path = "capture_source_writer_tests.rs"]
mod tests;
