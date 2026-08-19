//! Whether this installation can actually diarize, and why not when it
//! cannot.
//!
//! `scripts/diarize.py` and the merge/persist path have existed and been
//! tested since the Zoom work, but no installed build could run either: the
//! worker script was not in `bundle.resources`, and `pyannote.audio` and its
//! `torch` tree were in no pinned requirements file. The only signal a user
//! ever got was a job event reading `diarization unavailable: MODEL_ACCESS
//! pyannote-audio is not installed`, produced by a subprocess that had
//! already been spawned to find that out.
//!
//! This module answers the question before the subprocess, and in terms of
//! what the user would have to do about it.
//!
//! # Why the model is not bundled
//!
//! `pyannote/speaker-diarization-3.1` is gated on Hugging Face: every user
//! must accept its licence under their own account. Weights obtained that way
//! cannot be redistributed inside an installer, which is a licence
//! constraint, not an oversight — so unlike the Whisper model, which
//! `stage_whisper_runtime.ps1` pins and ships, the diarization model is
//! fetched once by the user with their own token. `HF_HOME` points at a
//! FUNG-owned directory so that download lands under the app rather than in
//! the user's global cache, and every run after the first is offline.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::WhisperRuntime;

/// Hugging Face repository the worker loads. Kept here as well as in
/// `diarize.py`'s default because the readiness probe has to look for this
/// exact repo in the cache, and a silent disagreement between the two would
/// report a model that is not the one that runs.
pub(crate) const DIARIZATION_MODEL: &str = "pyannote/speaker-diarization-3.1";

/// Directory name `huggingface_hub` gives a cached repo: `models--{org}--{name}`.
fn cache_dir_name(model: &str) -> String {
    format!("models--{}", model.replace('/', "--"))
}

/// The single missing thing that stops diarization running.
///
/// Ordered by what has to be true first, so the reported blocker is always
/// the *next* step rather than the last one checked. Telling someone their
/// Hugging Face token is missing when the Python runtime is absent would send
/// them to the wrong place entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DiarizationBlocker {
    /// The bundled Python runtime is absent — the build is broken or was
    /// never staged.
    RuntimeMissing,
    /// The runtime is present but `diarize.py` is not beside it. This was the
    /// state of every packaged build before the script joined
    /// `bundle.resources`.
    WorkerMissing,
    /// `pyannote.audio` or `torch` is not installed in the staged runtime.
    /// They are not in the default bundle; see the module docs.
    DependenciesMissing,
    /// Everything is installed but the gated model has never been fetched,
    /// and no token is configured to fetch it with.
    ModelNotFetched,
}

impl DiarizationBlocker {
    /// A sentence naming the next action, not the failure.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            DiarizationBlocker::RuntimeMissing => "ไม่พบ Python runtime ที่มากับ FUNG — ต้องติดตั้งแอปใหม่",
            DiarizationBlocker::WorkerMissing => "ไม่พบสคริปต์ diarize.py ในชุดติดตั้ง — ต้องติดตั้งแอปใหม่",
            DiarizationBlocker::DependenciesMissing => {
                "ยังไม่ได้ติดตั้ง pyannote.audio — รัน scripts/stage_diarization_runtime.ps1 \
                 (ไม่ได้มากับตัวติดตั้งเพราะ torch มีขนาดใหญ่มาก)"
            }
            DiarizationBlocker::ModelNotFetched => {
                "ยังไม่ได้ดาวน์โหลดโมเดล — ต้องยอมรับสัญญาอนุญาตบน Hugging Face \
                 แล้วตั้งค่า FUNG_HF_TOKEN ครั้งแรกครั้งเดียว"
            }
        }
    }

    /// A stable code for logs and job events, so a failure can be grepped
    /// without matching on Thai prose.
    pub(crate) fn code(self) -> &'static str {
        match self {
            DiarizationBlocker::RuntimeMissing => "runtime_missing",
            DiarizationBlocker::WorkerMissing => "worker_missing",
            DiarizationBlocker::DependenciesMissing => "dependencies_missing",
            DiarizationBlocker::ModelNotFetched => "model_not_fetched",
        }
    }
}

/// What the probe found, component by component.
///
/// Every field is reported rather than collapsed into `available`, because
/// "torch is there but the model is not" and "nothing is installed" call for
/// different actions and take different amounts of the user's time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiarizationReadiness {
    pub(crate) available: bool,
    pub(crate) blocker: Option<DiarizationBlocker>,
    pub(crate) detail: Option<String>,
    pub(crate) runtime_present: bool,
    pub(crate) worker_present: bool,
    pub(crate) dependencies_present: bool,
    pub(crate) model_present: bool,
    /// A token only matters until the model is cached. Reported separately so
    /// the first-run instruction can stop being shown once it is moot.
    pub(crate) token_configured: bool,
    pub(crate) model: &'static str,
    /// Where a fetched model lives, so the user can see what to back up or
    /// delete. Always reported, present or not.
    pub(crate) cache_root: String,
}

/// Decides the single blocker from the four facts.
///
/// Pure and ordered: the first unmet prerequisite wins. Separated from the
/// filesystem so the ordering — the part that decides which instruction a
/// user is given — is testable without staging a Python runtime.
pub(crate) fn first_blocker(
    runtime_present: bool,
    worker_present: bool,
    dependencies_present: bool,
    model_present: bool,
    token_configured: bool,
) -> Option<DiarizationBlocker> {
    if !runtime_present {
        return Some(DiarizationBlocker::RuntimeMissing);
    }
    if !worker_present {
        return Some(DiarizationBlocker::WorkerMissing);
    }
    if !dependencies_present {
        return Some(DiarizationBlocker::DependenciesMissing);
    }
    // A token with no cached model is not a blocker: the first run downloads
    // it. No token *and* no model is, because that run would fail.
    if !model_present && !token_configured {
        return Some(DiarizationBlocker::ModelNotFetched);
    }
    None
}

/// The Hugging Face cache FUNG owns.
///
/// Placed under the app's data root rather than the user's global
/// `~/.cache/huggingface` so a local-first install keeps its model weights
/// where the rest of its data lives, and uninstalling takes them with it.
/// `FUNG_HF_HOME` overrides it for anyone who wants to share an existing
/// cache instead of downloading several hundred megabytes twice.
pub(crate) fn hf_home(data_root: &Path) -> PathBuf {
    std::env::var_os("FUNG_HF_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_root.join("huggingface"))
}

/// Whether a Hugging Face token is available to fetch the gated model.
///
/// Reads only whether one is set — never its value, which stays in the
/// environment and is passed to the worker process without ever entering the
/// ledger, a log line, or this struct.
pub(crate) fn token_configured() -> bool {
    ["FUNG_HF_TOKEN", "HF_TOKEN"]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
}

/// Whether the staged runtime has the diarization dependency tree.
///
/// Checks for the installed package directories rather than importing them:
/// importing `torch` costs seconds and allocates hundreds of megabytes, which
/// is far too expensive for a probe the UI may call on every panel open.
fn dependencies_present(runtime: &WhisperRuntime) -> bool {
    let Some(site_packages) = site_packages(runtime) else {
        return false;
    };
    ["pyannote", "torch"]
        .iter()
        .all(|package| site_packages.join(package).is_dir())
}

/// `<venv>/Lib/site-packages`, derived from the interpreter path the runtime
/// already resolved so the two cannot point at different installations.
fn site_packages(runtime: &WhisperRuntime) -> Option<PathBuf> {
    Some(
        runtime
            .python
            .parent()? // Scripts
            .parent()? // .venv-whisper
            .join("Lib")
            .join("site-packages"),
    )
}

/// Whether the gated model has already been fetched into the cache.
fn model_present(cache_root: &Path) -> bool {
    cache_root
        .join("hub")
        .join(cache_dir_name(DIARIZATION_MODEL))
        .is_dir()
}

/// Inspects the installation. Filesystem checks only — no subprocess, no
/// network — so this is cheap enough to call whenever a panel opens.
pub(crate) fn probe(runtime: &WhisperRuntime, data_root: &Path) -> DiarizationReadiness {
    let cache_root = hf_home(data_root);
    let runtime_present = runtime.python.is_file();
    let worker_present = worker_script(runtime).is_some_and(|script| script.is_file());
    let dependencies_present = dependencies_present(runtime);
    let model_present = model_present(&cache_root);
    let token_configured = token_configured();

    let blocker = first_blocker(
        runtime_present,
        worker_present,
        dependencies_present,
        model_present,
        token_configured,
    );
    DiarizationReadiness {
        available: blocker.is_none(),
        detail: blocker.map(|blocker| blocker.detail().to_string()),
        blocker,
        runtime_present,
        worker_present,
        dependencies_present,
        model_present,
        token_configured,
        model: DIARIZATION_MODEL,
        cache_root: cache_root.display().to_string(),
    }
}

/// `scripts/diarize.py`, resolved beside the transcription worker so both
/// come from the same bundle.
pub(crate) fn worker_script(runtime: &WhisperRuntime) -> Option<PathBuf> {
    Some(runtime.script.parent()?.join("diarize.py"))
}

/// Reports whether this installation can diarize.
#[tauri::command]
pub(crate) fn diarization_status(
    state: tauri::State<'_, crate::AppState>,
) -> crate::AppResult<DiarizationReadiness> {
    Ok(probe(&state.whisper_runtime_clone(), &state.data_root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reported_blocker_is_the_next_step_not_the_last_check() {
        // Telling someone to configure a Hugging Face token when the Python
        // runtime is missing sends them somewhere that cannot help.
        assert_eq!(
            first_blocker(false, false, false, false, false),
            Some(DiarizationBlocker::RuntimeMissing)
        );
        assert_eq!(
            first_blocker(true, false, false, false, false),
            Some(DiarizationBlocker::WorkerMissing)
        );
        assert_eq!(
            first_blocker(true, true, false, false, false),
            Some(DiarizationBlocker::DependenciesMissing)
        );
        assert_eq!(
            first_blocker(true, true, true, false, false),
            Some(DiarizationBlocker::ModelNotFetched)
        );
    }

    #[test]
    fn a_token_with_no_cached_model_is_ready_because_the_first_run_fetches_it() {
        // The gated model is downloaded on first use. Refusing to start would
        // make the one run that can populate the cache impossible.
        assert_eq!(first_blocker(true, true, true, false, true), None);
    }

    #[test]
    fn a_cached_model_needs_no_token() {
        // The token matters only for the first download; after that the
        // pipeline is offline, and demanding one would strand a user who
        // rotated or removed it.
        assert_eq!(first_blocker(true, true, true, true, false), None);
    }

    #[test]
    fn every_blocker_names_an_action_and_carries_a_stable_code() {
        let mut codes = std::collections::HashSet::new();
        for blocker in [
            DiarizationBlocker::RuntimeMissing,
            DiarizationBlocker::WorkerMissing,
            DiarizationBlocker::DependenciesMissing,
            DiarizationBlocker::ModelNotFetched,
        ] {
            assert!(!blocker.detail().is_empty());
            assert!(
                codes.insert(blocker.code()),
                "{} reuses another blocker's code",
                blocker.code()
            );
            // Codes are matched by logs and tests; Thai prose is not.
            assert!(blocker
                .code()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }

    #[test]
    fn the_cache_directory_matches_what_huggingface_hub_writes() {
        // The probe looks for this exact directory. If the naming drifts, a
        // fetched model reports as missing and the user is told to download
        // something they already have.
        assert_eq!(
            cache_dir_name("pyannote/speaker-diarization-3.1"),
            "models--pyannote--speaker-diarization-3.1"
        );
    }

    #[test]
    fn the_model_the_probe_looks_for_is_the_one_the_worker_loads() {
        // `diarize.py` defaults to this repo. A disagreement would have the
        // probe report readiness for a model that never runs.
        let worker = include_str!("../../scripts/diarize.py");
        assert!(
            worker.contains(DIARIZATION_MODEL),
            "diarize.py no longer defaults to {DIARIZATION_MODEL}"
        );
    }

    #[test]
    fn the_cache_lives_under_the_app_data_root_by_default() {
        // Local-first: weights belong with the rest of the app's data, not in
        // the user's global cache where an uninstall would leave them behind.
        let data_root = Path::new("C:/data/fung");
        assert_eq!(hf_home(data_root), data_root.join("huggingface"));
    }
}
