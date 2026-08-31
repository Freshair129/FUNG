//! The job engine: one durable queue, one worker, and one place where a row
//! in `jobs` becomes work that actually runs.
//!
//! Before this module a job row was a claim, not a promise. `create_job`
//! wrote `status: "queued"` and nothing ever read it back, while the three
//! operations that *did* run real work — post-meeting summarisation, the
//! knowledge-graph build, and the transcript catch-up pass — each spawned
//! their own detached thread and hand-wrote their own job rows. Four
//! different notions of "a job" with no queue, no retry, no cancellation, and
//! no survival across a restart.
//!
//! The design decisions worth knowing:
//!
//! * **One worker, serialised.** Every kind the engine runs contends for the
//!   same scarce local resources — the Ollama endpoint, the whisper worker,
//!   and the single-writer Genesis WAL. Running two at once would not
//!   finish either sooner, and would make progress reporting a lie. Adding
//!   parallelism later is a change to [`Schedule`] and the worker loop, not
//!   to the handlers.
//! * **Handlers must be idempotent.** Retry is only safe if a second attempt
//!   converges on the same result rather than appending a second one. Every
//!   kind registered here satisfies that; see [`JobKind`] for how each does.
//!   A kind that cannot be made idempotent must not be registered.
//! * **`queued` is durable.** The queue lives in the ledger, not in memory.
//!   The in-memory [`Schedule`] is a cache of it, rebuilt at startup by
//!   [`JobEngine::adopt_pending`], so closing FUNG with work outstanding
//!   resumes rather than discards.
//! * **Cancellation is honest.** A job that has not started is cancelled
//!   immediately. A job already inside a handler is *not* interrupted — the
//!   handlers block in a 600-second HTTP call or a whisper subprocess with no
//!   cancellation point — so the request is recorded and applied when the
//!   handler returns. [`CancelOutcome`] says which of the two happened rather
//!   than reporting both as success.

use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};
use uuid::Uuid;

use crate::{genesis_adapter, now};

/// Attempts a job gets before the engine stops trying, counting the first
/// run. Three is one immediate try plus two chances for a transient
/// condition (Ollama restarting, whisper busy) to clear.
pub(crate) const MAX_ATTEMPTS: i64 = 3;

/// Delay before the second attempt; doubles for each attempt after it.
const BASE_BACKOFF: Duration = Duration::from_secs(15);

/// Ceiling on the backoff. Past this the delay stops being "wait for the
/// blip to pass" and starts being "the user has gone home", at which point
/// the failing job should be visible rather than pending.
const MAX_BACKOFF: Duration = Duration::from_secs(300);

/// How long the worker parks when the queue is empty. It is woken by the
/// condvar on every enqueue, so this is only a backstop against a missed
/// notification, not the normal wake path.
const IDLE_PARK: Duration = Duration::from_secs(60);

/// The engine's single-read page size (paging past it exists via
/// `genesis_adapter::query_all`). One page of pending jobs is far more than
/// a queue one person drives ever holds, so the single read stays.
const GENESIS_QUERY_LIMIT: u32 = crate::genesis_adapter::ROW_CAP;

/// The `jobs.status` values that mean "this job is not finished".
///
/// Shared with [`crate::recovery`], which must leave these alone for kinds
/// the engine can run: terminalising a `queued` row at startup is exactly
/// the behaviour the durable queue replaces.
pub(crate) const PENDING_STATUSES: [&str; 3] = ["queued", "running", "retrying"];

/// Event emitted to the UI whenever a job changes state.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobEvent {
    pub(crate) job_id: String,
    pub(crate) project_id: String,
    pub(crate) job_type: String,
    pub(crate) recording_id: Option<String>,
    pub(crate) status: String,
    pub(crate) progress: i64,
    pub(crate) attempt_no: i64,
    pub(crate) detail: Option<String>,
}

/// The closed set of work the engine knows how to run.
///
/// A job type outside this set is refused at enqueue time rather than
/// written as a row nothing will ever pick up. The old `create_job` accepted
/// any string, which is how nine buttons came to file rows that sat `queued`
/// until the next launch terminalised them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JobKind {
    /// Post-meeting summarisation and Markdown export.
    ///
    /// Idempotent because [`crate::meeting_intel::summarize_and_export`]
    /// derives each summary row's id from (project, recording, kind), so a
    /// second attempt replaces the first rather than adding a second recap.
    SummaryGenerate,
    /// Transcribes chunks of a recording that no transcript segment covers.
    ///
    /// Idempotent by construction: the pass recomputes which chunks are
    /// missing on every run, so anything already transcribed is skipped.
    TranscriptRetry,
    /// Structural + LLM knowledge-graph build for a recording.
    ///
    /// Idempotent because the builder derives node and edge ids from the
    /// recording and label (`graph_build::det_node_id`) and deletes stale
    /// extractions for the recording before writing.
    GraphBuild,
    /// Splits a locally captured meeting's far side into individual
    /// speakers.
    ///
    /// Idempotent because the pass removes the previous *proposed* turns
    /// before writing new ones, and re-labels segments in place by id rather
    /// than deleting and re-inserting them — so a second run converges on the
    /// same attribution instead of stacking a second proposal beside the
    /// first or orphaning the evidence refs that cite those segments.
    SpeakerDiarize,
    /// Renders the recording's transcript as `.srt` and `.vtt` beside it.
    ///
    /// Idempotent because both filenames derive from the recording id, so a
    /// retry overwrites its own previous output instead of leaving a second
    /// copy the ledger would then list twice.
    ExportRender,
}

impl JobKind {
    /// Every kind, so callers that need the whole set — `parse`, the
    /// `runnable_job_types` command the UI checks itself against — derive it
    /// from one list instead of each keeping their own copy to drift.
    pub(crate) const ALL: [JobKind; 5] = [
        JobKind::SummaryGenerate,
        JobKind::TranscriptRetry,
        JobKind::GraphBuild,
        JobKind::SpeakerDiarize,
        JobKind::ExportRender,
    ];

    /// The `jobs.type` string. These are the values already in existing
    /// ledgers, so they are a compatibility surface, not free naming.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            JobKind::SummaryGenerate => "summary.generate",
            JobKind::TranscriptRetry => "transcript.retry",
            JobKind::GraphBuild => "graph.build",
            JobKind::SpeakerDiarize => "speakers.diarize",
            JobKind::ExportRender => "export.render",
        }
    }

    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        JobKind::ALL.into_iter().find(|kind| kind.as_str() == raw)
    }

    /// Thai label for job-event messages, matching the rest of the UI.
    fn label(self) -> &'static str {
        match self {
            JobKind::SummaryGenerate => "สรุปการประชุม",
            JobKind::TranscriptRetry => "ถอดเสียงส่วนที่ขาด",
            JobKind::GraphBuild => "สร้างกราฟความรู้",
            JobKind::SpeakerDiarize => "แยกเสียงผู้พูด",
            JobKind::ExportRender => "ส่งออกซับไตเติล",
        }
    }

    /// Every kind runs against one recording; the id travels in
    /// `input_refs_json[0]`. Kept as a method rather than assumed so that a
    /// future project-scoped kind fails loudly here instead of silently
    /// reading the wrong ref.
    fn requires_recording(self) -> bool {
        true
    }
}

/// Why a job stopped, and whether trying again could plausibly help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobFailure {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) retryable: bool,
}

impl JobFailure {
    fn permanent(code: &'static str, message: impl Into<String>) -> Self {
        JobFailure {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    fn transient(code: &'static str, message: impl Into<String>) -> Self {
        JobFailure {
            code,
            message: message.into(),
            retryable: true,
        }
    }
}

/// Decides whether an opaque error string from a handler describes a
/// condition that a later attempt could survive.
///
/// The underlying operations report `Result<_, String>`, so classification
/// has to key off the message. It defaults to **not** retryable, which is
/// the safe direction: refusing to retry a transient failure leaves an
/// honest `failed` row the user can requeue, whereas retrying a permanent
/// one burns the local model and — for any handler whose idempotency the
/// engine has mis-assumed — risks duplicating output.
///
/// The transient markers are the ones `graph_build::send_error_message` and
/// `call_llm` actually produce; `cloud_executor::is_connection_error` keys
/// off the same "LLM endpoint unreachable" text, so the three must not
/// drift apart.
pub(crate) fn classify(message: &str) -> JobFailure {
    const TRANSIENT: [&str; 4] = [
        "LLM endpoint unreachable",
        "LLM endpoint timed out",
        "LLM endpoint returned 5",
        "worker failure",
    ];
    if TRANSIENT.iter().any(|marker| message.contains(marker)) {
        JobFailure::transient("provider_unavailable", message)
    } else {
        JobFailure::permanent("job_failed", message)
    }
}

/// Delay before attempt `attempt_no + 1`, doubling from [`BASE_BACKOFF`] and
/// clamped at [`MAX_BACKOFF`].
pub(crate) fn backoff_for(attempt_no: i64) -> Duration {
    let steps = attempt_no.clamp(1, 16) - 1;
    let scaled = BASE_BACKOFF.saturating_mul(1u32 << steps.min(16));
    scaled.min(MAX_BACKOFF)
}

/// What the engine should do after a handler returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NextStep {
    /// Terminal success.
    Complete,
    /// Terminal failure — out of attempts, or a failure retrying cannot fix.
    Fail(JobFailure),
    /// Try again after the given delay, as attempt `attempt_no`.
    Retry {
        attempt_no: i64,
        delay: Duration,
        failure: JobFailure,
    },
    /// The user asked for this job to stop while it was running.
    Cancelled,
}

/// The retry policy, isolated from the worker so it can be exercised without
/// a Tauri app, a ledger, or a real handler.
pub(crate) fn next_step(
    outcome: Result<(), JobFailure>,
    attempt_no: i64,
    cancel_requested: bool,
) -> NextStep {
    // Cancellation wins over the handler's own result only when the handler
    // failed. A job that finished its work before the cancel landed did the
    // work — reporting it as cancelled would claim output exists that does
    // not, or that it does not exist when it does.
    match outcome {
        Ok(()) => NextStep::Complete,
        Err(failure) => {
            if cancel_requested {
                return NextStep::Cancelled;
            }
            if failure.retryable && attempt_no < MAX_ATTEMPTS {
                NextStep::Retry {
                    attempt_no: attempt_no + 1,
                    delay: backoff_for(attempt_no),
                    failure,
                }
            } else {
                NextStep::Fail(failure)
            }
        }
    }
}

/// A job waiting to run, and the earliest moment it may.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scheduled {
    job_id: String,
    ready_at: Instant,
}

/// The in-memory view of the durable queue.
///
/// A `Vec` rather than a heap: the queue holds the jobs one user has asked
/// for, which is a handful, and keeping it a flat list makes "is this job
/// already queued?" — the dedupe check every enqueue does — a linear scan
/// instead of a second index that could disagree with the first.
#[derive(Debug, Default)]
pub(crate) struct Schedule {
    entries: Vec<Scheduled>,
}

impl Schedule {
    /// Adds a job unless it is already scheduled. Returns whether it was
    /// added, so a duplicate enqueue is visible rather than silent.
    pub(crate) fn insert(&mut self, job_id: String, ready_at: Instant) -> bool {
        if self.contains(&job_id) {
            return false;
        }
        self.entries.push(Scheduled { job_id, ready_at });
        true
    }

    pub(crate) fn remove(&mut self, job_id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.job_id != job_id);
        before != self.entries.len()
    }

    pub(crate) fn contains(&self, job_id: &str) -> bool {
        self.entries.iter().any(|entry| entry.job_id == job_id)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Removes and returns the job that has been ready longest, or `None` if
    /// nothing is ready yet. Oldest-ready-first rather than
    /// insertion-order-first so a job whose backoff has expired is not
    /// starved behind one that is still waiting.
    pub(crate) fn take_ready(&mut self, at: Instant) -> Option<String> {
        let (index, _) = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.ready_at <= at)
            .min_by_key(|(_, entry)| entry.ready_at)?;
        Some(self.entries.remove(index).job_id)
    }

    /// How long to park before something becomes ready. `None` means the
    /// queue is empty; `Some(ZERO)` means something is ready now.
    pub(crate) fn next_wait(&self, at: Instant) -> Option<Duration> {
        self.entries
            .iter()
            .map(|entry| entry.ready_at.saturating_duration_since(at))
            .min()
    }
}

/// What a cancel request actually achieved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CancelOutcome {
    /// The job had not started; it is `cancelled` now and will not run.
    Cancelled,
    /// The job is inside a handler that has no cancellation point. The
    /// request is recorded and applied when the handler returns, so the work
    /// currently in flight still completes or fails on its own terms.
    RequestedWhileRunning,
    /// Nothing pending under that id — already finished, or never existed.
    NotPending,
}

#[derive(Default)]
struct EngineState {
    schedule: Schedule,
    /// Jobs the user asked to stop. Entries are removed as they are applied.
    cancelling: HashSet<String>,
    running: Option<String>,
    shutdown: bool,
}

struct Inner {
    storage: Arc<genesis_block_native::Storage>,
    state: Mutex<EngineState>,
    wake: Condvar,
}

/// Handle to the running engine, held in `AppState`.
#[derive(Clone)]
pub(crate) struct JobEngine {
    inner: Arc<Inner>,
}

/// One job as the worker needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobRow {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) kind: JobKind,
    pub(crate) status: String,
    pub(crate) attempt_no: i64,
    pub(crate) recording_id: Option<String>,
    /// When the job was first queued. Carried through every rewrite because
    /// a retry is the same job, not a new one: `list_jobs` orders by this,
    /// so restamping it on attempt two would jump a week-old failure to the
    /// top of the list and hide how long it had been failing.
    pub(crate) created_at: String,
}

/// Reads the first element of a `input_refs_json` value, which may arrive as
/// a JSON array or as a string holding one.
pub(crate) fn first_input_ref(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(array) = value.as_array() {
        return array
            .first()
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
    }
    let raw = value.as_str()?;
    serde_json::from_str::<Vec<String>>(raw)
        .ok()
        .and_then(|refs| refs.into_iter().next())
}

impl JobEngine {
    /// Builds the engine without starting its worker.
    ///
    /// Construction and start are separate because the handlers reach back
    /// into `AppState` (the transcript pass needs the whisper runtime), so
    /// the worker must not run until `app.manage(state)` has happened — and
    /// the engine has to exist before that, because it lives inside the very
    /// state being managed. Enqueueing before the worker starts is safe: the
    /// row is committed and the schedule holds it until there is a worker.
    pub(crate) fn new(storage: Arc<genesis_block_native::Storage>) -> JobEngine {
        JobEngine {
            inner: Arc::new(Inner {
                storage,
                state: Mutex::new(EngineState::default()),
                wake: Condvar::new(),
            }),
        }
    }

    /// Starts the worker thread. Call once, after the app state is managed.
    pub(crate) fn start_worker(&self, app: tauri::AppHandle) {
        let worker = self.clone();
        std::thread::Builder::new()
            .name("fung-job-engine".into())
            .spawn(move || worker_loop(worker, app))
            .expect("job engine worker thread must spawn");
    }

    /// Rebuilds the in-memory queue from the ledger.
    ///
    /// A row left `running` belongs to a process that no longer exists, so
    /// it is re-queued as a fresh attempt if it has attempts left and failed
    /// otherwise — the same judgement `recovery` makes, except that here
    /// something exists to pick the work back up.
    pub(crate) fn adopt_pending(&self) -> Result<usize, String> {
        let mut adopted = 0usize;
        for status in PENDING_STATUSES {
            for job in self.pending_rows(status)? {
                if job.status == "running" {
                    if job.attempt_no >= MAX_ATTEMPTS {
                        let _ = crate::set_job_status(
                            &self.inner.storage,
                            &job.id,
                            "failed",
                            None,
                            Some(&format!(
                                "interrupted: {} was still running when FUNG last exited, and had no attempts left",
                                job.kind.as_str()
                            )),
                        );
                        continue;
                    }
                    let next = job.attempt_no + 1;
                    if let Err(error) = self.write_attempt(&job, "queued", next, Some(&format!(
                        "interrupted: {} was still running when FUNG last exited — retrying as attempt {next}",
                        job.kind.as_str()
                    ))) {
                        eprintln!("[jobs] could not re-queue {}: {error}", job.id);
                        continue;
                    }
                }
                let mut state = self.inner.state.lock().expect("job engine state poisoned");
                if state.schedule.insert(job.id.clone(), Instant::now()) {
                    adopted += 1;
                }
            }
        }
        self.inner.wake.notify_all();
        Ok(adopted)
    }

    /// Queues a job, or returns the id of the equivalent one already
    /// pending.
    ///
    /// Deduplication is not a nicety: pressing "Generate recap" three times
    /// must not run three summarisations against the same recording, both
    /// because each occupies the local model for minutes and because the
    /// third would overwrite the second's output while the second was still
    /// producing it.
    pub(crate) fn enqueue(
        &self,
        kind: JobKind,
        project_id: &str,
        recording_id: Option<&str>,
    ) -> Result<String, String> {
        if kind.requires_recording() && recording_id.is_none() {
            return Err(format!("{} needs a recording", kind.as_str()));
        }
        if let Some(existing) = self.find_pending(kind, project_id, recording_id)? {
            return Ok(existing);
        }

        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let refs: Vec<String> = recording_id.map(str::to_owned).into_iter().collect();
        genesis_adapter::commit_rows(
            &self.inner.storage,
            vec![
                genesis_adapter::upsert(
                    "jobs",
                    serde_json::json!({
                        "id": id, "project_id": project_id, "type": kind.as_str(),
                        "status": "queued", "progress": 0,
                        "input_refs_json": refs, "output_refs_json": [],
                        "provider_id": null, "error_code": null, "error_message": null,
                        "attempt_no": 1, "started_at": null, "finished_at": null,
                        "created_at": timestamp, "updated_at": timestamp,
                    }),
                ),
                genesis_adapter::upsert(
                    "job_events",
                    serde_json::json!({
                        "id": Uuid::new_v4().to_string(), "job_id": id, "status": "queued",
                        "message": format!("{} เข้าคิวแล้ว", kind.label()),
                        "created_at": timestamp,
                    }),
                ),
            ],
        )?;

        let mut state = self.inner.state.lock().expect("job engine state poisoned");
        state.schedule.insert(id.clone(), Instant::now());
        drop(state);
        self.inner.wake.notify_all();
        Ok(id)
    }

    /// Asks for a job to stop. See [`CancelOutcome`] for what that means in
    /// each case — a running handler is not interrupted.
    pub(crate) fn cancel(&self, job_id: &str) -> CancelOutcome {
        let mut state = self.inner.state.lock().expect("job engine state poisoned");
        if state.schedule.remove(job_id) {
            drop(state);
            let _ = crate::set_job_status(
                &self.inner.storage,
                job_id,
                "cancelled",
                None,
                Some("ยกเลิกโดยผู้ใช้ก่อนเริ่มทำงาน"),
            );
            return CancelOutcome::Cancelled;
        }
        if state.running.as_deref() == Some(job_id) {
            state.cancelling.insert(job_id.to_string());
            return CancelOutcome::RequestedWhileRunning;
        }
        CancelOutcome::NotPending
    }

    /// How many jobs are waiting. Used by the health surface and tests.
    pub(crate) fn queue_depth(&self) -> usize {
        self.inner
            .state
            .lock()
            .expect("job engine state poisoned")
            .schedule
            .len()
    }

    /// Stops the worker after the job in flight finishes. Queued work stays
    /// queued in the ledger and is adopted on the next launch.
    pub(crate) fn shutdown(&self) {
        let mut state = self.inner.state.lock().expect("job engine state poisoned");
        state.shutdown = true;
        drop(state);
        self.inner.wake.notify_all();
    }

    fn find_pending(
        &self,
        kind: JobKind,
        project_id: &str,
        recording_id: Option<&str>,
    ) -> Result<Option<String>, String> {
        for status in PENDING_STATUSES {
            for job in self.pending_rows(status)? {
                if job.kind == kind
                    && job.project_id == project_id
                    && job.recording_id.as_deref() == recording_id
                {
                    return Ok(Some(job.id));
                }
            }
        }
        Ok(None)
    }

    /// Rows in one non-terminal status whose type the engine can run.
    ///
    /// The engine deliberately ignores pending rows of any other type: they
    /// belong to no handler, and [`crate::recovery`] terminalises them so
    /// they do not spin forever.
    fn pending_rows(&self, status: &str) -> Result<Vec<JobRow>, String> {
        let rows = genesis_adapter::query(
            &self.inner.storage,
            "jobs",
            &[
                "id",
                "project_id",
                "type",
                "status",
                "attempt_no",
                "input_refs_json",
                "created_at",
            ],
            vec![genesis_adapter::eq(
                "jobs",
                "status",
                serde_json::json!(status),
            )],
            GENESIS_QUERY_LIMIT,
        )?;
        Ok(rows.iter().filter_map(job_row_from).collect())
    }

    fn load(&self, job_id: &str) -> Result<Option<JobRow>, String> {
        let rows = genesis_adapter::query(
            &self.inner.storage,
            "jobs",
            &[
                "id",
                "project_id",
                "type",
                "status",
                "attempt_no",
                "input_refs_json",
                "created_at",
            ],
            vec![genesis_adapter::eq("jobs", "id", serde_json::json!(job_id))],
            1,
        )?;
        Ok(rows.first().and_then(job_row_from))
    }

    /// Writes a status change that also moves `attempt_no`.
    ///
    /// `set_job_status` deliberately preserves `attempt_no`, so the engine
    /// needs its own path for the one thing that changes it: starting a new
    /// attempt.
    fn write_attempt(
        &self,
        job: &JobRow,
        status: &str,
        attempt_no: i64,
        message: Option<&str>,
    ) -> Result<(), String> {
        let timestamp = now();
        let refs: Vec<String> = job.recording_id.clone().into_iter().collect();
        genesis_adapter::commit_rows(
            &self.inner.storage,
            vec![
                genesis_adapter::upsert(
                    "jobs",
                    serde_json::json!({
                        "id": job.id, "project_id": job.project_id, "type": job.kind.as_str(),
                        "status": status, "progress": 0,
                        "input_refs_json": refs, "output_refs_json": [],
                        "provider_id": null, "error_code": null,
                        "error_message": message, "attempt_no": attempt_no,
                        "started_at": null, "finished_at": null,
                        "created_at": job.created_at, "updated_at": timestamp,
                    }),
                ),
                genesis_adapter::upsert(
                    "job_events",
                    serde_json::json!({
                        "id": Uuid::new_v4().to_string(), "job_id": job.id, "status": status,
                        "message": message.unwrap_or(status), "created_at": timestamp,
                    }),
                ),
            ],
        )
    }
}

fn job_row_from(row: &serde_json::Value) -> Option<JobRow> {
    let id = row.get("jobs.id")?.as_str()?.to_string();
    let kind = JobKind::parse(row.get("jobs.type")?.as_str()?)?;
    Some(JobRow {
        id,
        project_id: row.get("jobs.project_id")?.as_str()?.to_string(),
        kind,
        status: row
            .get("jobs.status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        attempt_no: row
            .get("jobs.attempt_no")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(1),
        recording_id: first_input_ref(row.get("jobs.input_refs_json")),
        created_at: row
            .get("jobs.created_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn worker_loop(engine: JobEngine, app: tauri::AppHandle) {
    loop {
        let job_id = {
            let inner = &engine.inner;
            let mut state = inner.state.lock().expect("job engine state poisoned");
            loop {
                if state.shutdown {
                    return;
                }
                if let Some(id) = state.schedule.take_ready(Instant::now()) {
                    state.running = Some(id.clone());
                    break id;
                }
                let wait = state
                    .next_wait_or_park(Instant::now())
                    .unwrap_or(IDLE_PARK)
                    .max(Duration::from_millis(50));
                let (guard, _) = inner
                    .wake
                    .wait_timeout(state, wait)
                    .expect("job engine state poisoned");
                state = guard;
            }
        };

        run_one(&engine, &app, &job_id);

        let mut state = engine
            .inner
            .state
            .lock()
            .expect("job engine state poisoned");
        state.running = None;
        state.cancelling.remove(&job_id);
    }
}

impl EngineState {
    fn next_wait_or_park(&self, at: Instant) -> Option<Duration> {
        self.schedule.next_wait(at)
    }
}

/// Runs one job end to end: claim, dispatch, record.
fn run_one(engine: &JobEngine, app: &tauri::AppHandle, job_id: &str) {
    let job = match engine.load(job_id) {
        Ok(Some(job)) => job,
        // The row vanished or its type is no longer one the engine handles.
        // Dropping it is correct — there is nothing to run and nothing to
        // report against.
        Ok(None) => return,
        Err(error) => {
            eprintln!("[jobs] could not load {job_id}: {error}");
            return;
        }
    };

    if let Err(error) =
        crate::set_job_status(&engine.inner.storage, &job.id, "running", Some(0), None)
    {
        eprintln!("[jobs] could not claim {job_id}: {error}");
        return;
    }
    emit(app, &job, "running", 0, None);

    let outcome = dispatch(app, &engine.inner.storage, &job);

    let cancel_requested = {
        let state = engine
            .inner
            .state
            .lock()
            .expect("job engine state poisoned");
        state.cancelling.contains(&job.id)
    };

    match next_step(outcome, job.attempt_no, cancel_requested) {
        NextStep::Complete => {
            let _ =
                crate::set_job_status(&engine.inner.storage, &job.id, "completed", Some(100), None);
            emit(app, &job, "completed", 100, None);
        }
        NextStep::Cancelled => {
            let _ = crate::set_job_status(
                &engine.inner.storage,
                &job.id,
                "cancelled",
                None,
                Some("ยกเลิกโดยผู้ใช้ระหว่างทำงาน"),
            );
            emit(app, &job, "cancelled", 0, None);
        }
        NextStep::Fail(failure) => {
            let _ = crate::set_job_status(
                &engine.inner.storage,
                &job.id,
                "failed",
                None,
                Some(&failure.message),
            );
            emit(app, &job, "failed", 0, Some(failure.message));
        }
        NextStep::Retry {
            attempt_no,
            delay,
            failure,
        } => {
            let detail = format!(
                "{} — ลองใหม่ครั้งที่ {attempt_no} ใน {} วินาที",
                failure.message,
                delay.as_secs()
            );
            if let Err(error) = engine.write_attempt(&job, "retrying", attempt_no, Some(&detail)) {
                eprintln!("[jobs] could not schedule retry for {}: {error}", job.id);
                let _ = crate::set_job_status(
                    &engine.inner.storage,
                    &job.id,
                    "failed",
                    None,
                    Some(&failure.message),
                );
                emit(app, &job, "failed", 0, Some(failure.message));
                return;
            }
            let mut state = engine
                .inner
                .state
                .lock()
                .expect("job engine state poisoned");
            state
                .schedule
                .insert(job.id.clone(), Instant::now() + delay);
            drop(state);
            engine.inner.wake.notify_all();
            emit(app, &job, "retrying", 0, Some(detail));
        }
    }
}

/// Maps a job to the operation that performs it.
///
/// Everything below is a call into code that already existed and already
/// ran; what the engine adds is that it now runs from a queue, under a retry
/// policy, with its status told truthfully.
fn dispatch(
    app: &tauri::AppHandle,
    storage: &Arc<genesis_block_native::Storage>,
    job: &JobRow,
) -> Result<(), JobFailure> {
    let Some(recording_id) = job.recording_id.as_deref() else {
        return Err(JobFailure::permanent(
            "missing_recording",
            format!("{} has no recording in its input refs", job.kind.as_str()),
        ));
    };

    match job.kind {
        JobKind::SummaryGenerate => {
            crate::meeting_intel::summarize_and_export(storage, &job.project_id, recording_id)
                .map(|export_path| {
                    // The live panel listens on `live-summary`, and that
                    // contract predates the engine. Keeping it means ending a
                    // meeting still lights up the same UI, whether the
                    // summary ran on the first attempt or the third.
                    let _ = app.emit(
                        "live-summary",
                        serde_json::json!({
                            "recordingId": recording_id,
                            "state": "ready",
                            "detail": serde_json::Value::Null,
                            "exportPath": export_path,
                        }),
                    );
                })
                .map_err(|error| {
                    let _ = app.emit(
                        "live-summary",
                        serde_json::json!({
                            "recordingId": recording_id,
                            "state": "failed",
                            "detail": error.clone(),
                            "exportPath": serde_json::Value::Null,
                        }),
                    );
                    classify(&error)
                })
        }
        JobKind::GraphBuild => {
            let label = crate::graph_build::meeting_label(storage, &job.project_id, recording_id);
            crate::graph_build::run_graph_build(
                storage,
                &job.project_id,
                recording_id,
                &label,
                &job.id,
            )
            .map_err(|error| classify(&error))
        }
        JobKind::SpeakerDiarize => {
            let state = app.state::<crate::AppState>();
            let runtime = state.whisper_runtime_clone();
            let data_root = state.data_root.clone();
            let outcome = crate::local_diarization::diarize_recording(
                app,
                storage,
                &runtime,
                &data_root,
                &job.project_id,
                recording_id,
                |_| {},
            );
            if let Some(reason) = outcome.skipped_reason {
                // Every decline here is a state the user has to change —
                // install the dependencies, fetch the model, record some
                // far-side audio. None of them clear by waiting.
                return Err(JobFailure::permanent("diarization_skipped", reason));
            }
            if outcome.turns == 0 {
                // The model ran and heard nobody. Reported as a failure
                // rather than a silent success, because a transcript that
                // still says "อีกฝ่าย" everywhere looks identical to one
                // that was never diarized.
                return Err(JobFailure::permanent(
                    "no_speakers_found",
                    "แยกเสียงผู้พูดแล้วแต่ไม่พบผู้พูดในเสียงฝั่งอีกฝ่าย".to_string(),
                ));
            }
            Ok(())
        }
        JobKind::ExportRender => {
            crate::transcript_export::render_subtitles(storage, &job.project_id, recording_id)
                .map(|export| {
                    // `write_attempt` rewrites `output_refs_json` to `[]` on
                    // every status change, so the paths are recorded where
                    // they survive: `export_artifacts` (written by the
                    // renderer) and this event, which is the job's own trail.
                    let _ = genesis_adapter::commit_rows(
                        storage,
                        vec![genesis_adapter::upsert(
                            "job_events",
                            serde_json::json!({
                                "id": Uuid::new_v4().to_string(), "job_id": job.id,
                                "status": "running",
                                "message": format!(
                                    "เขียนซับไตเติล {} ท่อน: {} และ {}",
                                    export.cue_count, export.srt_path, export.vtt_path
                                ),
                                "created_at": now(),
                            }),
                        )],
                    );
                })
                // Every way this declines is a state the user changes, not
                // one that clears by retrying: no transcript yet, or a
                // recording past the engine's single-read ceiling. Retrying
                // would just rewrite the same refusal onto the job.
                .map_err(|error| JobFailure::permanent("export_failed", error))
        }
        JobKind::TranscriptRetry => {
            let runtime = app.state::<crate::AppState>().whisper_runtime_clone();
            // The pass reads the recording's own language from the ledger, so
            // a queued re-run transcribes with the setting the session was
            // captured under rather than re-detecting per chunk.
            let outcome = crate::live_meeting::fill_transcript_gaps(
                app,
                storage,
                &runtime,
                &job.project_id,
                recording_id,
            );
            if let Some(reason) = outcome.skipped_reason {
                // The pass declined to run at all — a saturated segment
                // table, a missing recording. None of those clear on their
                // own, so this is terminal.
                return Err(JobFailure::permanent("gap_fill_skipped", reason));
            }
            if outcome.still_missing > 0 {
                // Safe to retry precisely because the pass recomputes which
                // chunks are missing, so the chunks it already transcribed
                // are not transcribed twice.
                return Err(JobFailure::transient(
                    "gap_fill_incomplete",
                    format!(
                        "ถอดเสียงได้ {} จาก {} ส่วน — ยังขาดอีก {}",
                        outcome.chunks_transcribed,
                        outcome.chunks_missing_transcript,
                        outcome.still_missing
                    ),
                ));
            }
            Ok(())
        }
    }
}

fn emit(app: &tauri::AppHandle, job: &JobRow, status: &str, progress: i64, detail: Option<String>) {
    let _ = app.emit(
        "job-update",
        JobEvent {
            job_id: job.id.clone(),
            project_id: job.project_id.clone(),
            job_type: job.kind.as_str().to_string(),
            recording_id: job.recording_id.clone(),
            status: status.to_string(),
            progress,
            attempt_no: job.attempt_no,
            detail,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kinds_with_a_handler_parse() {
        assert_eq!(
            JobKind::parse("summary.generate"),
            Some(JobKind::SummaryGenerate)
        );
        assert_eq!(JobKind::parse(" graph.build "), Some(JobKind::GraphBuild));
        // Every one of these was a button that filed a row nothing ran.
        for inert in [
            "capture.marker",
            "speakers.lock",
            "review.evidence",
            "summary.recap",
            "summary.compare",
            "summary.intent",
            "summary.actions",
            "export.queue",
            "archive.project",
        ] {
            assert_eq!(JobKind::parse(inert), None, "{inert} must not be runnable");
        }
    }

    #[test]
    fn job_types_that_still_run_their_own_threads_stay_with_recovery() {
        // The partition matters in both directions. `recovery` terminalises
        // every non-terminal job whose type the engine does not claim; if
        // one of these ever parsed here, the engine would adopt a job with
        // no handler and it would spin forever. If an engine type were
        // *missing* from `JobKind`, recovery would kill queued work at
        // startup — the exact behaviour the durable queue replaces.
        for hand_rolled in ["zoom.import", "recording.capture", "transcript.transcribe"] {
            assert_eq!(
                JobKind::parse(hand_rolled),
                None,
                "{hand_rolled} is still driven by its own thread"
            );
        }
    }

    #[test]
    fn kind_strings_round_trip() {
        for kind in [
            JobKind::SummaryGenerate,
            JobKind::TranscriptRetry,
            JobKind::GraphBuild,
        ] {
            assert_eq!(JobKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn unknown_failures_are_not_retried() {
        let failure = classify("ยังไม่มี transcript ของเซสชันนี้");
        assert!(
            !failure.retryable,
            "an unrecognised failure must not be retried"
        );
    }

    #[test]
    fn provider_outages_are_retried() {
        for message in [
            "LLM endpoint unreachable at http://127.0.0.1:11434: connection refused",
            "LLM endpoint timed out at http://127.0.0.1:11434",
            "LLM endpoint returned 503 Service Unavailable",
        ] {
            assert!(classify(message).retryable, "{message} must be retryable");
        }
    }

    #[test]
    fn backoff_doubles_then_stops_growing() {
        assert_eq!(backoff_for(1), BASE_BACKOFF);
        assert_eq!(backoff_for(2), BASE_BACKOFF * 2);
        assert_eq!(backoff_for(3), BASE_BACKOFF * 4);
        assert_eq!(backoff_for(99), MAX_BACKOFF);
    }

    #[test]
    fn a_retryable_failure_retries_until_attempts_run_out() {
        let failure = JobFailure::transient("provider_unavailable", "endpoint down");
        assert!(matches!(
            next_step(Err(failure.clone()), 1, false),
            NextStep::Retry { attempt_no: 2, .. }
        ));
        assert!(matches!(
            next_step(Err(failure.clone()), MAX_ATTEMPTS, false),
            NextStep::Fail(_)
        ));
    }

    #[test]
    fn a_permanent_failure_is_not_retried_even_on_the_first_attempt() {
        let failure = JobFailure::permanent("job_failed", "no transcript");
        assert!(matches!(
            next_step(Err(failure), 1, false),
            NextStep::Fail(_)
        ));
    }

    #[test]
    fn work_that_finished_is_reported_as_done_even_if_cancel_arrived_late() {
        // The output exists. Calling it cancelled would tell the user their
        // summary was not written when it was.
        assert_eq!(next_step(Ok(()), 1, true), NextStep::Complete);
    }

    #[test]
    fn a_cancelled_job_that_failed_is_cancelled_not_retried() {
        let failure = JobFailure::transient("provider_unavailable", "endpoint down");
        assert_eq!(next_step(Err(failure), 1, true), NextStep::Cancelled);
    }

    #[test]
    fn schedule_refuses_to_hold_the_same_job_twice() {
        let mut schedule = Schedule::default();
        let at = Instant::now();
        assert!(schedule.insert("job-a".into(), at));
        assert!(!schedule.insert("job-a".into(), at));
        assert_eq!(schedule.len(), 1);
    }

    #[test]
    fn schedule_withholds_a_job_until_its_backoff_expires() {
        let mut schedule = Schedule::default();
        let at = Instant::now();
        schedule.insert("job-a".into(), at + Duration::from_secs(30));
        assert_eq!(schedule.take_ready(at), None);
        assert_eq!(
            schedule.next_wait(at),
            Some(Duration::from_secs(30)),
            "the worker must park until the retry is due, not spin"
        );
        assert_eq!(
            schedule.take_ready(at + Duration::from_secs(30)),
            Some("job-a".to_string())
        );
    }

    #[test]
    fn the_job_ready_longest_runs_first() {
        let mut schedule = Schedule::default();
        let at = Instant::now();
        // Inserted last, but ready first: a job whose backoff already
        // expired must not queue behind one that is still waiting.
        schedule.insert("waiting".into(), at + Duration::from_secs(60));
        schedule.insert("ready-recently".into(), at - Duration::from_secs(1));
        schedule.insert("ready-longest".into(), at - Duration::from_secs(90));
        assert_eq!(schedule.take_ready(at), Some("ready-longest".to_string()));
        assert_eq!(schedule.take_ready(at), Some("ready-recently".to_string()));
        assert_eq!(schedule.take_ready(at), None);
    }

    #[test]
    fn cancelling_removes_a_queued_job_from_the_schedule() {
        let mut schedule = Schedule::default();
        let at = Instant::now();
        schedule.insert("job-a".into(), at);
        assert!(schedule.remove("job-a"));
        assert!(!schedule.contains("job-a"));
        assert!(!schedule.remove("job-a"), "a second cancel is a no-op");
    }

    #[test]
    fn an_empty_schedule_asks_the_worker_to_park() {
        let schedule = Schedule::default();
        assert_eq!(schedule.next_wait(Instant::now()), None);
    }

    #[test]
    fn input_refs_are_read_from_arrays_and_from_encoded_strings() {
        assert_eq!(
            first_input_ref(Some(&serde_json::json!(["rec-1"]))),
            Some("rec-1".to_string())
        );
        assert_eq!(
            first_input_ref(Some(&serde_json::json!("[\"rec-2\"]"))),
            Some("rec-2".to_string())
        );
        assert_eq!(first_input_ref(Some(&serde_json::json!([]))), None);
        assert_eq!(first_input_ref(None), None);
    }

    #[test]
    fn a_row_of_an_unhandled_type_is_not_adopted() {
        // archive.project rows exist in ledgers written before the engine.
        // Adopting one would put a job in the queue with nothing to run it.
        let row = serde_json::json!({
            "jobs.id": "job-1",
            "jobs.project_id": "proj-1",
            "jobs.type": "archive.project",
            "jobs.status": "queued",
            "jobs.attempt_no": 1,
            "jobs.input_refs_json": ["rec-1"],
            "jobs.created_at": "2026-01-01T00:00:00Z",
        });
        assert_eq!(job_row_from(&row), None);
    }

    #[test]
    fn a_runnable_row_carries_its_recording_through() {
        let row = serde_json::json!({
            "jobs.id": "job-1",
            "jobs.project_id": "proj-1",
            "jobs.type": "summary.generate",
            "jobs.status": "running",
            "jobs.attempt_no": 2,
            "jobs.input_refs_json": ["rec-1"],
            "jobs.created_at": "2026-01-01T00:00:00Z",
        });
        let job = job_row_from(&row).expect("summary.generate must be runnable");
        assert_eq!(job.kind, JobKind::SummaryGenerate);
        assert_eq!(job.attempt_no, 2);
        assert_eq!(job.recording_id.as_deref(), Some("rec-1"));
        assert_eq!(job.created_at, "2026-01-01T00:00:00Z");
    }
}
