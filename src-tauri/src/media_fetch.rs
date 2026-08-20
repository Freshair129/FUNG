//! Fetching a recording FUNG did not capture: whether this installation can
//! do it, whether its owner has said it may, and what actually leaves the
//! machine when it does.
//!
//! Everything else in this tree moves in one direction. Capture writes audio,
//! transcription reads it, and the single path on which source audio leaves
//! (cloud STT) is off by default and gated by the cloud tier policy in
//! `policy.rs`. This module is the first that reaches *out* on the desktop's
//! own behalf, so it carries three gates rather than one:
//!
//! 1. **Consent.** Off by default, persisted, reversible — see
//!    [`crate::policy::media_fetch_consent`]. Staging the runtime is not
//!    consent; a build that has yt-dlp on disk still refuses until someone
//!    turns it on.
//! 2. **Scheme.** `http`/`https` only, checked here *and* in the worker. A
//!    `file://` URL would turn a text box into an arbitrary-file read, and
//!    yt-dlp accepts one happily.
//! 3. **Readiness.** The fetcher is not in the default bundle. [`probe`]
//!    answers "can this run" before a subprocess is spawned to find out, in
//!    terms of what the user would have to do about it.
//!
//! What leaves on a fetch is the URL the user typed and this machine's IP
//! address, to the host in that URL. No recording, no transcript, and no
//! FUNG-held material of any kind. What arrives is an audio stream that then
//! enters the ordinary import path — custody, digest, ledger — so a fetched
//! recording is backed up and integrity-checked exactly like a dragged-in
//! file. Declared in `docs/appendices/E-egress-register.md` §1.4.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::WhisperRuntime;

/// Longest media this will fetch. Six hours is past any meeting or interview
/// FUNG is for, and the ceiling exists so that a mistyped URL pointing at a
/// 24-hour livestream fails in a sentence rather than by filling the disk.
/// Enforced in the worker, before the transfer, from this value.
pub(crate) const MAX_FETCH_DURATION_S: u32 = 6 * 60 * 60;

/// The single missing thing that stops a URL fetch running.
///
/// Ordered by what has to be true first, so the reported blocker is always
/// the *next* step. Telling someone to install a JS runtime when they have
/// not consented to outbound fetches at all sends them to the wrong place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MediaFetchBlocker {
    /// The bundled Python runtime is absent — the build is broken or was
    /// never staged.
    RuntimeMissing,
    /// The runtime is present but `fetch_media.py` is not beside it.
    WorkerMissing,
    /// yt-dlp is not installed. It is not in the default bundle; see the
    /// module docs for why a network fetcher is opt-in.
    DependenciesMissing,
    /// Everything is installed and nobody has agreed to it being used.
    ConsentWithheld,
}

impl MediaFetchBlocker {
    /// A sentence naming the next action, not the failure.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            MediaFetchBlocker::RuntimeMissing => "ไม่พบ Python runtime ที่มากับ FUNG — ต้องติดตั้งแอปใหม่",
            MediaFetchBlocker::WorkerMissing => "ไม่พบสคริปต์ fetch_media.py ในชุดติดตั้ง — ต้องติดตั้งแอปใหม่",
            MediaFetchBlocker::DependenciesMissing => {
                "ยังไม่ได้ติดตั้ง yt-dlp — รัน scripts/stage_media_fetch_runtime.ps1 \
                 (ไม่ได้มากับตัวติดตั้งเพราะเป็นตัวเดียวในแอปที่ต่อออกอินเทอร์เน็ต)"
            }
            MediaFetchBlocker::ConsentWithheld => {
                "ยังไม่ได้อนุญาตให้ดึงสื่อจากอินเทอร์เน็ต — เปิดได้ในหน้าตั้งค่า และปิดกลับได้ทุกเมื่อ"
            }
        }
    }

    /// A stable code for logs and job events, so a failure can be grepped
    /// without matching on Thai prose.
    pub(crate) fn code(self) -> &'static str {
        match self {
            MediaFetchBlocker::RuntimeMissing => "runtime_missing",
            MediaFetchBlocker::WorkerMissing => "worker_missing",
            MediaFetchBlocker::DependenciesMissing => "dependencies_missing",
            MediaFetchBlocker::ConsentWithheld => "consent_withheld",
        }
    }
}

/// What the probe found, component by component.
///
/// Every field is reported rather than collapsed into `available`, because
/// "yt-dlp is installed but YouTube will still fail" and "nothing is
/// installed" call for different actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaFetchReadiness {
    pub(crate) available: bool,
    pub(crate) blocker: Option<MediaFetchBlocker>,
    /// The blocker's stable code. Reported alongside `detail` so the UI can
    /// branch on which action to offer — a consent switch, or an
    /// installation instruction — without matching on Thai prose that is
    /// free to be reworded.
    pub(crate) blocker_code: Option<&'static str>,
    pub(crate) detail: Option<String>,
    pub(crate) runtime_present: bool,
    pub(crate) worker_present: bool,
    pub(crate) dependencies_present: bool,
    /// Consent is reported separately from `available` so the UI can offer
    /// the switch rather than an installation instruction when that is the
    /// only thing missing.
    pub(crate) consent_granted: bool,
    /// Whether a JavaScript runtime is staged for YouTube's signature
    /// challenges.
    ///
    /// Deliberately NOT a blocker: yt-dlp fetches from most extractors
    /// without it, so refusing every URL because one site is difficult would
    /// be wrong. It is reported so that a YouTube failure is predicted
    /// instead of discovered.
    pub(crate) js_runtime_present: bool,
    /// The advisory that goes with `js_runtime_present == false`. `None` once
    /// the runtime is staged, so a resolved warning stops being shown.
    pub(crate) js_runtime_detail: Option<&'static str>,
    pub(crate) max_duration_s: u32,
    /// Where staged packages live, so the user can see what to delete to
    /// remove the capability. Always reported, present or not.
    pub(crate) packages_dir: String,
}

/// What a completed fetch produced. Mirrors `fetch_media.py`'s stdout JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FetchedMedia {
    /// Absolute path of the downloaded audio, inside the directory the
    /// caller created. Handed straight to `audio_custody::take_custody_of_import`.
    pub(crate) path: String,
    pub(crate) title: String,
    pub(crate) duration_ms: i64,
    /// yt-dlp's extractor key (`Youtube`, `Vimeo`, ...). Recorded on the
    /// recording row so a fetched file can be told from a dragged-in one
    /// without parsing the URL again.
    pub(crate) extractor: String,
    /// The URL yt-dlp resolved to, which is not always the one pasted — a
    /// share link redirects, and the canonical one is what belongs in the
    /// ledger.
    pub(crate) webpage_url: String,
}

/// `<runtime>/Lib/media-fetch-packages`, derived from the interpreter path
/// the runtime already resolved so the two cannot point at different
/// installations.
///
/// Deliberately not the runtime's own `site-packages`: `certifi` and `idna`
/// are pinned by both sets, and installing over the top would silently change
/// the transcription runtime that `stage_whisper_runtime.ps1` hashed into its
/// manifest. Keeping them apart also means the transcription worker cannot
/// import an HTTP client even by accident.
pub(crate) fn packages_dir(runtime: &WhisperRuntime) -> Option<PathBuf> {
    Some(
        runtime
            .python
            .parent()? // Scripts
            .parent()? // .venv-whisper
            .join("Lib")
            .join("media-fetch-packages"),
    )
}

/// `<packages>/bin`, where pip's `--target` puts a wheel's console scripts —
/// which is where the `deno` wheel's `deno.exe` lands. Prepended to the
/// worker's PATH so yt-dlp's own executable lookup finds it.
fn js_runtime_dir(runtime: &WhisperRuntime) -> Option<PathBuf> {
    Some(packages_dir(runtime)?.join("bin"))
}

/// `scripts/fetch_media.py`, resolved beside the transcription worker so both
/// come from the same bundle.
pub(crate) fn worker_script(runtime: &WhisperRuntime) -> Option<PathBuf> {
    Some(runtime.script.parent()?.join("fetch_media.py"))
}

/// Whether yt-dlp is staged.
///
/// Checks for the package directory rather than importing it: a probe the UI
/// may call on every panel open cannot afford a subprocess. Staging does run
/// the real import, which is the stronger check and the right place for it.
fn dependencies_present(runtime: &WhisperRuntime) -> bool {
    packages_dir(runtime).is_some_and(|dir| dir.join("yt_dlp").is_dir())
}

fn js_runtime_present(runtime: &WhisperRuntime) -> bool {
    js_runtime_dir(runtime).is_some_and(|dir| {
        dir.join(format!("deno{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    })
}

const JS_RUNTIME_ADVISORY: &str = "ยังไม่ได้ติดตั้ง JS runtime — YouTube ส่วนใหญ่จะดึงไม่สำเร็จ \
     รัน scripts/stage_media_fetch_runtime.ps1 -WithJsRuntime (เพิ่ม 41 MB) \
     เว็บอื่นใช้ได้ตามปกติ";

/// Decides the single blocker from the four facts. Pure, so the ordering is
/// testable without a filesystem.
fn first_blocker(
    runtime_present: bool,
    worker_present: bool,
    dependencies_present: bool,
    consent_granted: bool,
) -> Option<MediaFetchBlocker> {
    if !runtime_present {
        return Some(MediaFetchBlocker::RuntimeMissing);
    }
    if !worker_present {
        return Some(MediaFetchBlocker::WorkerMissing);
    }
    if !dependencies_present {
        return Some(MediaFetchBlocker::DependenciesMissing);
    }
    if !consent_granted {
        return Some(MediaFetchBlocker::ConsentWithheld);
    }
    None
}

/// Inspects the installation. Filesystem checks only — no subprocess, no
/// network — so this is cheap enough to call whenever a panel opens.
pub(crate) fn probe(runtime: &WhisperRuntime, consent_granted: bool) -> MediaFetchReadiness {
    let runtime_present = runtime.python.is_file();
    let worker_present = worker_script(runtime).is_some_and(|script| script.is_file());
    let dependencies_present = dependencies_present(runtime);
    let js_runtime_present = js_runtime_present(runtime);

    let blocker = first_blocker(
        runtime_present,
        worker_present,
        dependencies_present,
        consent_granted,
    );
    MediaFetchReadiness {
        available: blocker.is_none(),
        detail: blocker.map(|blocker| blocker.detail().to_string()),
        blocker_code: blocker.map(MediaFetchBlocker::code),
        blocker,
        runtime_present,
        worker_present,
        dependencies_present,
        consent_granted,
        js_runtime_present,
        js_runtime_detail: (!js_runtime_present).then_some(JS_RUNTIME_ADVISORY),
        max_duration_s: MAX_FETCH_DURATION_S,
        packages_dir: packages_dir(runtime)
            .map(|dir| dir.display().to_string())
            .unwrap_or_default(),
    }
}

/// Rejects anything that is not an `http(s)` URL with a host.
///
/// The worker checks this too. Both, deliberately: this one keeps the refusal
/// legible and subprocess-free for the common typo, and the worker's keeps it
/// true no matter who calls the script. A scheme allowlist rather than a
/// blocklist — `file`, `data` and yt-dlp's own pseudo-schemes are all things
/// nobody enumerated in advance.
pub(crate) fn require_http_url(raw: &str) -> Result<&str, String> {
    let trimmed = raw.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err("ต้องเป็นลิงก์ที่ขึ้นต้นด้วย http:// หรือ https://".to_string());
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(format!(
            "ไม่รองรับลิงก์แบบ {scheme}:// — รับเฉพาะ http และ https"
        ));
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        // Strip any userinfo: `https://evil@host/` reads as `host` to a
        // fetcher and as `evil` to a careless split.
        .rsplit('@')
        .next()
        .unwrap_or_default();
    if host.is_empty() {
        return Err("ลิงก์นี้ไม่มีชื่อโฮสต์".to_string());
    }
    Ok(trimmed)
}

/// Runs `fetch_media.py` and blocks until it exits, reporting its
/// `PROGRESS <pct>` lines through `on_progress`. Intended to run off the main
/// thread, like `run_transcription`.
///
/// `dest_dir` must already exist and belong to the caller: the worker writes
/// only inside it, and the caller is what moves the result into custody.
pub(crate) fn fetch(
    runtime: &WhisperRuntime,
    url: &str,
    dest_dir: &Path,
    on_progress: impl Fn(i64) + Send + 'static,
) -> Result<FetchedMedia, String> {
    let url = require_http_url(url)?;
    let script = worker_script(runtime).ok_or("could not resolve the fetch worker script")?;
    let packages = packages_dir(runtime).ok_or("could not resolve the staged fetch packages")?;

    let max_duration = MAX_FETCH_DURATION_S.to_string();
    let raw_output = crate::run_python_worker(
        runtime,
        &script,
        &[
            url,
            "--dest-dir",
            &dest_dir.display().to_string(),
            "--packages-dir",
            &packages.display().to_string(),
            "--max-duration-s",
            &max_duration,
        ],
        // yt-dlp finds `deno` by executable lookup, so the staged copy has to
        // be on the child's PATH. Absent, this is `None` and YouTube fails
        // with yt-dlp's own message — which the probe has already predicted.
        js_runtime_present(runtime)
            .then(|| js_runtime_dir(runtime))
            .flatten()
            .as_deref(),
        // No Hugging Face cache: this worker has no business reaching the hub
        // even though it is the one worker allowed to reach the network at
        // all. `run_python_worker` turns `None` into `HF_HUB_OFFLINE=1`.
        None,
        on_progress,
    )?;

    serde_json::from_str::<FetchedMedia>(raw_output.trim())
        .map_err(|err| format!("failed to parse fetch output: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reported_blocker_is_the_next_step_not_the_last_check() {
        // Offering a consent switch to someone whose Python runtime is
        // missing sends them somewhere that cannot help.
        assert_eq!(
            first_blocker(false, false, false, false),
            Some(MediaFetchBlocker::RuntimeMissing)
        );
        assert_eq!(
            first_blocker(true, false, false, false),
            Some(MediaFetchBlocker::WorkerMissing)
        );
        assert_eq!(
            first_blocker(true, true, false, false),
            Some(MediaFetchBlocker::DependenciesMissing)
        );
        assert_eq!(
            first_blocker(true, true, true, false),
            Some(MediaFetchBlocker::ConsentWithheld)
        );
        assert_eq!(first_blocker(true, true, true, true), None);
    }

    #[test]
    fn a_staged_runtime_without_consent_is_still_refused() {
        // Installing the fetcher is not agreeing to use it. This is the
        // whole reason consent is a separate fact from readiness: someone
        // who ran the staging script once should not thereby have granted
        // every future fetch.
        assert_eq!(
            first_blocker(true, true, true, false),
            Some(MediaFetchBlocker::ConsentWithheld)
        );
    }

    #[test]
    fn only_http_and_https_are_accepted() {
        assert!(require_http_url("https://example.com/watch?v=abc").is_ok());
        assert!(require_http_url("http://example.com/a").is_ok());
        // The one that matters: yt-dlp accepts this and would read the disk.
        assert!(require_http_url("file:///C:/Users/me/secrets.txt").is_err());
        assert!(require_http_url("data:text/plain,hello").is_err());
        assert!(require_http_url("example.com/watch").is_err());
    }

    #[test]
    fn a_url_with_no_host_is_refused() {
        assert!(require_http_url("https:///path-only").is_err());
        // Userinfo must not be mistaken for the host.
        assert!(require_http_url("https://user@/path").is_err());
    }

    #[test]
    fn surrounding_whitespace_is_tolerated_because_pasted_urls_carry_it() {
        assert_eq!(
            require_http_url("  https://example.com/a  ").unwrap(),
            "https://example.com/a"
        );
    }

    #[test]
    fn the_js_advisory_disappears_once_the_runtime_is_staged() {
        // A warning that stays after it has been acted on trains people to
        // ignore warnings.
        let runtime = WhisperRuntime {
            python: PathBuf::from("nonexistent/Scripts/python.exe"),
            script: PathBuf::from("nonexistent/scripts/transcribe.py"),
            cuda_bin: PathBuf::new(),
        };
        let readiness = probe(&runtime, false);
        assert!(!readiness.js_runtime_present);
        assert!(readiness.js_runtime_detail.is_some());
    }

    #[test]
    fn a_missing_installation_never_reports_available() {
        let runtime = WhisperRuntime {
            python: PathBuf::from("nonexistent/Scripts/python.exe"),
            script: PathBuf::from("nonexistent/scripts/transcribe.py"),
            cuda_bin: PathBuf::new(),
        };
        // Even with consent granted: consent is permission, not capability.
        let readiness = probe(&runtime, true);
        assert!(!readiness.available);
        assert_eq!(readiness.blocker, Some(MediaFetchBlocker::RuntimeMissing));
        assert!(readiness.consent_granted);
    }
}
