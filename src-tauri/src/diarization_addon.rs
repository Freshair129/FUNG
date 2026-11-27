//! The speaker-diarization runtime add-on: installed from inside the app,
//! on the user's request, from exactly the wheels the reviewed lockfile pins.
//!
//! `pyannote.audio` and its `torch` tree are not in the installer (they would
//! add ~240 MB to every download for a feature many users never turn on), and
//! the embedded Python has no pip by design. So the app does what pip would
//! do for a `--no-deps --require-hashes` install, and nothing more:
//!
//! 1. Download each wheel listed in `scripts/diarization-runtime-manifest.json`
//!    from `files.pythonhosted.org`. The manifest is compiled in; no URL or
//!    digest is read from the network, and nothing is resolved at run time.
//! 2. Reject any file whose size or SHA-256 differs from the manifest.
//! 3. Unpack the wheels (zip archives) into a staging `site-packages`. No
//!    code from a package runs during install.
//! 4. Rename the staging directory into place and write a marker, so a
//!    half-finished install is never reported as ready.
//!
//! Downloading from PyPI rather than hosting a FUNG archive means FUNG does
//! not redistribute these binaries; see
//! `docs/verification/2026-11-26-diarization-runtime-licence-audit.md`.
//!
//! The add-on lives under the app's data root, not the install directory:
//! that may not be writable, and an app update would delete it.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST_JSON: &str = include_str!("../../scripts/diarization-runtime-manifest.json");

/// The only host a wheel may come from. Checked per file at run time as well
/// as by a unit test over the compiled manifest.
const ALLOWED_URL_PREFIX: &str = "https://files.pythonhosted.org/packages/";

/// Written last, inside the install directory, once every wheel is unpacked.
const INSTALLED_MARKER: &str = "INSTALLED";

/// Unpacked wheels take roughly 3.5x their compressed size (836 MiB from
/// 240 MiB for the current lock). The free-space check asks for the
/// downloads plus this multiple, so a nearly full disk fails up front.
const UNPACK_FACTOR: u64 = 4;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Wheel {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) filename: String,
    pub(crate) url: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Manifest {
    pub(crate) python_tag: String,
    pub(crate) platform_tag: String,
    pub(crate) wheels: Vec<Wheel>,
}

pub(crate) fn manifest() -> Manifest {
    serde_json::from_str(MANIFEST_JSON).expect("compiled diarization manifest is valid JSON")
}

/// Short, stable id of the compiled manifest. A new lock gets a new
/// directory, so an app update never loads wheels from an older lock.
pub(crate) fn lock_id() -> String {
    let digest = Sha256::digest(MANIFEST_JSON.as_bytes());
    digest[..6].iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn download_bytes() -> u64 {
    manifest().wheels.iter().map(|w| w.size).sum()
}

fn addon_root(data_root: &Path) -> PathBuf {
    data_root.join("diarization-runtime")
}

fn install_dir(data_root: &Path) -> PathBuf {
    addon_root(data_root).join(lock_id())
}

/// The `site-packages` the diarization worker adds to its `sys.path`.
pub(crate) fn site_packages(data_root: &Path) -> PathBuf {
    install_dir(data_root).join("site-packages")
}

/// Whether the add-on for the compiled lock is fully installed.
pub(crate) fn installed(data_root: &Path) -> bool {
    fs::read_to_string(install_dir(data_root).join(INSTALLED_MARKER))
        .is_ok_and(|content| content.trim() == lock_id())
}

/// Progress of one install, emitted to the UI as it changes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallProgress {
    pub(crate) phase: InstallPhase,
    pub(crate) done_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) done_files: usize,
    pub(crate) total_files: usize,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InstallPhase {
    Downloading,
    Unpacking,
    Done,
}

/// Why an install stopped. Codes are stable for logs and the UI; the Thai
/// sentence names what the user can do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstallError {
    Cancelled,
    UnsupportedRuntime(String),
    InsufficientDisk { needed: u64, free: u64 },
    Network(String),
    Integrity(String),
    Io(String),
}

impl InstallError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            InstallError::Cancelled => "cancelled",
            InstallError::UnsupportedRuntime(_) => "unsupported_runtime",
            InstallError::InsufficientDisk { .. } => "insufficient_disk",
            InstallError::Network(_) => "network",
            InstallError::Integrity(_) => "integrity",
            InstallError::Io(_) => "io",
        }
    }

    pub(crate) fn detail(&self) -> String {
        match self {
            InstallError::Cancelled => "ยกเลิกการติดตั้งแล้ว".to_string(),
            InstallError::UnsupportedRuntime(found) => {
                format!("Python ที่มากับ FUNG ไม่ใช่รุ่นที่ add-on รองรับ ({found}) — ต้องติดตั้งแอปใหม่")
            }
            InstallError::InsufficientDisk { needed, free } => format!(
                "พื้นที่ดิสก์ไม่พอ ต้องการประมาณ {} MB แต่เหลือ {} MB",
                needed / 1_048_576,
                free / 1_048_576
            ),
            InstallError::Network(error) => {
                format!("ดาวน์โหลดไม่สำเร็จ ตรวจการเชื่อมต่อแล้วลองใหม่ ({error})")
            }
            InstallError::Integrity(file) => {
                format!("ไฟล์ {file} ไม่ตรงกับที่ตรวจสอบไว้ จึงไม่ติดตั้ง — ลองใหม่อีกครั้ง")
            }
            InstallError::Io(error) => format!("เขียนไฟล์ไม่สำเร็จ ({error})"),
        }
    }
}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> Self {
        InstallError::Io(error.to_string())
    }
}

/// The wheels target one interpreter ABI. The embedded runtime ships its
/// version as `python3XY.dll` beside `python.exe`, which is cheaper and more
/// honest to check than spawning the interpreter.
pub(crate) fn runtime_supported(python: &Path, manifest: &Manifest) -> Result<(), InstallError> {
    if !(cfg!(all(windows, target_arch = "x86_64")) && manifest.platform_tag == "win_amd64") {
        return Err(InstallError::UnsupportedRuntime(format!(
            "add-on targets {}",
            manifest.platform_tag
        )));
    }
    let Some(dir) = python.parent() else {
        return Err(InstallError::UnsupportedRuntime(
            "no runtime directory".into(),
        ));
    };
    let expected = format!("python{}.dll", manifest.python_tag.trim_start_matches("cp"));
    if dir.join(&expected).is_file() {
        Ok(())
    } else {
        Err(InstallError::UnsupportedRuntime(format!(
            "{expected} not found"
        )))
    }
}

/// Downloads, verifies and unpacks the add-on for the compiled lock.
///
/// Idempotent: an already-installed lock returns immediately, and wheels
/// already downloaded and verified by an interrupted attempt are reused.
pub(crate) fn install(
    data_root: &Path,
    python: &Path,
    cancel: &AtomicBool,
    on_progress: &dyn Fn(InstallProgress),
) -> Result<(), InstallError> {
    let manifest = manifest();
    runtime_supported(python, &manifest)?;
    if installed(data_root) {
        return Ok(());
    }

    let root = addon_root(data_root);
    fs::create_dir_all(&root)?;
    let total_bytes: u64 = manifest.wheels.iter().map(|w| w.size).sum();
    let needed = total_bytes.saturating_mul(1 + UNPACK_FACTOR);
    if let Some(free) = crate::live_meeting::free_disk_bytes(&root) {
        if free < needed {
            return Err(InstallError::InsufficientDisk { needed, free });
        }
    }

    let downloads = root.join("downloads");
    fs::create_dir_all(&downloads)?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("FUNG/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(None)
        .build()
        .map_err(|error| InstallError::Network(error.to_string()))?;

    let total_files = manifest.wheels.len();
    let mut progress = InstallProgress {
        phase: InstallPhase::Downloading,
        done_bytes: 0,
        total_bytes,
        done_files: 0,
        total_files,
    };
    on_progress(progress.clone());

    for wheel in &manifest.wheels {
        let target = downloads.join(&wheel.filename);
        if !verified(&target, wheel) {
            download(&client, wheel, &target, cancel, &mut |bytes| {
                on_progress(InstallProgress {
                    done_bytes: progress.done_bytes + bytes,
                    ..progress.clone()
                });
            })?;
        }
        progress.done_bytes += wheel.size;
        progress.done_files += 1;
        on_progress(progress.clone());
    }

    let staging = root.join(format!(".staging-{}", lock_id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let staging_site = staging.join("site-packages");
    fs::create_dir_all(&staging_site)?;

    progress.phase = InstallPhase::Unpacking;
    progress.done_files = 0;
    on_progress(progress.clone());
    for wheel in &manifest.wheels {
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_dir_all(&staging);
            return Err(InstallError::Cancelled);
        }
        unpack_wheel(&downloads.join(&wheel.filename), &staging_site)?;
        progress.done_files += 1;
        on_progress(progress.clone());
    }
    fs::write(staging.join(INSTALLED_MARKER), lock_id())?;

    let destination = install_dir(data_root);
    if destination.exists() {
        // Only a directory without a valid marker reaches here.
        fs::remove_dir_all(&destination)?;
    }
    fs::rename(&staging, &destination)?;

    // Wheels are not needed once unpacked, and add-ons for older locks can
    // no longer be loaded by this build.
    let _ = fs::remove_dir_all(&downloads);
    remove_other_locks(&root);

    progress.phase = InstallPhase::Done;
    on_progress(progress);
    Ok(())
}

/// Deletes the add-on, every lock's copy and any partial download. The model
/// cache is separate and is not touched here.
pub(crate) fn remove(data_root: &Path) -> io::Result<()> {
    let root = addon_root(data_root);
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    Ok(())
}

fn remove_other_locks(root: &Path) {
    let keep = lock_id();
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy() != keep && entry.path().is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn verified(path: &Path, wheel: &Wheel) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() != wheel.size {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut hasher = Sha256::new();
    if io::copy(&mut file, &mut hasher).is_err() {
        return false;
    }
    hex(&hasher.finalize()) == wheel.sha256
}

fn download(
    client: &reqwest::blocking::Client,
    wheel: &Wheel,
    target: &Path,
    cancel: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<(), InstallError> {
    if !wheel.url.starts_with(ALLOWED_URL_PREFIX) {
        return Err(InstallError::Integrity(format!(
            "{} {} ({})",
            wheel.name, wheel.version, wheel.filename
        )));
    }
    let mut response = client
        .get(&wheel.url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| InstallError::Network(error.to_string()))?;

    let partial = target.with_extension("partial");
    let mut out = fs::File::create(&partial)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut received: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(out);
            let _ = fs::remove_file(&partial);
            return Err(InstallError::Cancelled);
        }
        let read = response
            .read(&mut buffer)
            .map_err(|error| InstallError::Network(error.to_string()))?;
        if read == 0 {
            break;
        }
        received += read as u64;
        if received > wheel.size {
            drop(out);
            let _ = fs::remove_file(&partial);
            return Err(InstallError::Integrity(format!(
                "{} {} ({})",
                wheel.name, wheel.version, wheel.filename
            )));
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])?;
        on_bytes(received);
    }
    out.sync_all()?;
    drop(out);

    if received != wheel.size || hex(&hasher.finalize()) != wheel.sha256 {
        let _ = fs::remove_file(&partial);
        return Err(InstallError::Integrity(format!(
            "{} {} ({})",
            wheel.name, wheel.version, wheel.filename
        )));
    }
    fs::rename(&partial, target)?;
    Ok(())
}

/// Where a wheel entry belongs inside `site-packages`, or `None` to skip it.
///
/// Wheel layout (PEP 427): everything is relative to `site-packages` except
/// `<dist>.data/<scheme>/...`. `purelib` and `platlib` also go to
/// `site-packages`; `scripts`, `headers` and `data` are not needed to import
/// the package and are skipped rather than written outside the add-on.
/// Absolute paths, drive prefixes and `..` are rejected so an archive cannot
/// write outside the staging directory.
pub(crate) fn wheel_entry_destination(name: &str) -> Result<Option<PathBuf>, String> {
    let path = Path::new(name);
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(format!("unsafe path in wheel: {name}")),
        }
    }
    if parts.iter().any(|part| part.contains(':')) {
        return Err(format!("unsafe path in wheel: {name}"));
    }
    let Some(first) = parts.first() else {
        return Ok(None);
    };
    if first.ends_with(".data") {
        return match parts.get(1).map(String::as_str) {
            Some("purelib" | "platlib") if parts.len() > 2 => Ok(Some(parts[2..].iter().collect())),
            _ => Ok(None),
        };
    }
    Ok(Some(parts.iter().collect()))
}

fn unpack_wheel(wheel: &Path, site_packages: &Path) -> Result<(), InstallError> {
    let file = fs::File::open(wheel)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| InstallError::Integrity(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| InstallError::Integrity(error.to_string()))?;
        let name = entry
            .name()
            .map_err(|error| InstallError::Integrity(error.to_string()))?
            .into_owned();
        let Some(relative) = wheel_entry_destination(&name).map_err(InstallError::Integrity)?
        else {
            continue;
        };
        let destination = site_packages.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&destination)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_entries() -> Vec<(String, String, String)> {
        include_str!("../../scripts/diarization-runtime-requirements.txt")
            .lines()
            .filter_map(|line| {
                let (spec, hash) = line.split_once(" --hash=sha256:")?;
                let (name, version) = spec.split_once("==")?;
                Some((
                    name.to_string(),
                    version.to_string(),
                    hash.trim().to_string(),
                ))
            })
            .collect()
    }

    #[test]
    fn every_wheel_in_the_manifest_is_pinned_by_the_reviewed_lockfile() {
        // The manifest is generated, the lockfile is reviewed. A wheel the
        // lockfile does not pin, or pins with another digest, must not ship.
        let lock = lock_entries();
        for wheel in manifest().wheels {
            assert!(
                lock.iter().any(|(name, version, hash)| *name == wheel.name
                    && *version == wheel.version
                    && *hash == wheel.sha256),
                "{} {} is not pinned by the lockfile",
                wheel.name,
                wheel.version
            );
        }
    }

    #[test]
    fn every_locked_distribution_is_either_installed_or_explicitly_skipped() {
        // Regenerating the lock without the manifest would silently drop a
        // dependency. Skips are allowed only for the two source-only
        // distributions no locked package needs at run time.
        let manifest = manifest();
        let skipped: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        let skipped: Vec<&str> = skipped["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["name"].as_str().unwrap())
            .collect();
        assert_eq!(skipped, ["antlr4-python3-runtime", "docopt"]);
        for (name, _, _) in lock_entries() {
            assert!(
                manifest.wheels.iter().any(|wheel| wheel.name == name)
                    || skipped.contains(&name.as_str()),
                "{name} is locked but neither installed nor skipped"
            );
        }
    }

    #[test]
    fn wheels_come_only_from_pypi_and_target_the_bundled_runtime() {
        let manifest = manifest();
        assert_eq!(manifest.python_tag, "cp311");
        assert_eq!(manifest.platform_tag, "win_amd64");
        for wheel in &manifest.wheels {
            assert!(wheel.url.starts_with(ALLOWED_URL_PREFIX), "{}", wheel.url);
            assert!(wheel.url.ends_with(&wheel.filename), "{}", wheel.url);
            assert!(wheel.filename.ends_with(".whl"));
            assert_eq!(wheel.sha256.len(), 64);
            assert!(
                wheel.filename.contains("-none-any") || wheel.filename.contains("win_amd64"),
                "{} is not loadable by the bundled runtime",
                wheel.filename
            );
        }
    }

    #[test]
    fn the_install_directory_changes_when_the_manifest_does() {
        // A stable id that is a pure function of the compiled manifest.
        assert_eq!(lock_id(), lock_id());
        assert_eq!(lock_id().len(), 12);
        assert!(site_packages(Path::new("C:/d")).starts_with(Path::new("C:/d/diarization-runtime")));
    }

    #[test]
    fn wheel_entries_map_into_site_packages_and_never_out_of_it() {
        assert_eq!(
            wheel_entry_destination("torch/lib/c10.dll").unwrap(),
            Some(PathBuf::from("torch/lib/c10.dll"))
        );
        assert_eq!(
            wheel_entry_destination("pkg-1.0.data/purelib/pkg/x.py").unwrap(),
            Some(PathBuf::from("pkg/x.py"))
        );
        assert_eq!(
            wheel_entry_destination("pkg-1.0.data/platlib/pkg/x.pyd").unwrap(),
            Some(PathBuf::from("pkg/x.pyd"))
        );
        // sympy and fonttools ship man pages under `.data/data`.
        assert_eq!(
            wheel_entry_destination("sympy-1.14.0.data/data/share/man/isympy.1").unwrap(),
            None
        );
        assert_eq!(
            wheel_entry_destination("pkg-1.0.data/scripts/tool").unwrap(),
            None
        );
        for unsafe_name in [
            "../evil.py",
            "/etc/passwd",
            "C:/Windows/x.dll",
            "pkg/../../x",
            "a/b:c",
        ] {
            assert!(
                wheel_entry_destination(unsafe_name).is_err(),
                "{unsafe_name}"
            );
        }
    }

    #[test]
    fn the_runtime_check_reads_the_interpreter_dll_beside_python() {
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join("python.exe");
        let manifest = manifest();
        assert!(matches!(
            runtime_supported(&python, &manifest),
            Err(InstallError::UnsupportedRuntime(_))
        ));
        fs::write(dir.path().join("python311.dll"), b"").unwrap();
        assert_eq!(runtime_supported(&python, &manifest), Ok(()));
    }

    #[test]
    fn an_install_without_the_marker_is_not_reported_as_installed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(site_packages(dir.path())).unwrap();
        assert!(!installed(dir.path()));
        fs::write(install_dir(dir.path()).join(INSTALLED_MARKER), "stale").unwrap();
        assert!(!installed(dir.path()));
        fs::write(install_dir(dir.path()).join(INSTALLED_MARKER), lock_id()).unwrap();
        assert!(installed(dir.path()));
        remove(dir.path()).unwrap();
        assert!(!installed(dir.path()));
    }

    #[test]
    fn a_wheel_unpacks_into_site_packages_and_skips_data_schemes() {
        let dir = tempfile::tempdir().unwrap();
        let wheel_path = dir.path().join("demo-1.0-py3-none-any.whl");
        {
            let mut writer = zip::ZipWriter::new(fs::File::create(&wheel_path).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("demo/__init__.py", options).unwrap();
            writer.write_all(b"VALUE = 1\n").unwrap();
            writer
                .start_file("demo-1.0.data/purelib/demo_extra.py", options)
                .unwrap();
            writer.write_all(b"").unwrap();
            writer
                .start_file("demo-1.0.data/data/share/doc.txt", options)
                .unwrap();
            writer.write_all(b"").unwrap();
            writer.finish().unwrap();
        }
        let site = dir.path().join("site");
        unpack_wheel(&wheel_path, &site).unwrap();
        assert_eq!(
            fs::read(site.join("demo/__init__.py")).unwrap(),
            b"VALUE = 1\n"
        );
        assert!(site.join("demo_extra.py").is_file());
        assert!(!site.join("share").exists());
        assert!(!site.join("demo-1.0.data").exists());
    }

    /// Real install from PyPI into `FUNG_ADDON_E2E_DIR`, then imports the
    /// tree with the bundled interpreter. ~240 MB of network; run by hand:
    /// `FUNG_ADDON_E2E_DIR=... cargo test --lib real_install -- --ignored`
    #[test]
    #[ignore]
    fn real_install_from_pypi_imports_with_the_bundled_runtime() {
        let data_root =
            PathBuf::from(std::env::var("FUNG_ADDON_E2E_DIR").expect("FUNG_ADDON_E2E_DIR"));
        let python =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.venv-whisper/Scripts/python.exe");
        let cancel = AtomicBool::new(false);
        let last = std::sync::Mutex::new(None);
        let started = std::time::Instant::now();
        install(&data_root, &python, &cancel, &|progress| {
            *last.lock().unwrap() = Some(progress);
        })
        .unwrap();
        eprintln!(
            "installed in {:?}: {:?}",
            started.elapsed(),
            last.lock().unwrap()
        );
        assert!(installed(&data_root));

        let site = site_packages(&data_root);
        let output = std::process::Command::new(&python)
            .env("FUNG_DIARIZATION_SITE", &site)
            .arg("-c")
            .arg(
                "import os, site, sys\n\
                 before = list(sys.path); site.addsitedir(os.environ['FUNG_DIARIZATION_SITE'])\n\
                 sys.path[:] = [p for p in sys.path if p not in before] + before\n\
                 import torch, pyannote.audio, numpy\n\
                 from pyannote.audio import Pipeline\n\
                 print('ok', torch.__version__, pyannote.audio.__version__, numpy.__version__, torch.__file__)",
            )
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!(
            "stdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success());
        assert!(stdout.starts_with("ok 2.14.0"));
        assert!(
            stdout.contains(&*site.to_string_lossy()) || stdout.contains("diarization-runtime")
        );
    }

    #[test]
    fn a_corrupt_cached_wheel_is_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.whl");
        fs::write(&path, b"abc").unwrap();
        let wheel = Wheel {
            name: "x".into(),
            version: "1".into(),
            filename: "x.whl".into(),
            url: String::new(),
            sha256: hex(&Sha256::digest(b"abc")),
            size: 3,
        };
        assert!(verified(&path, &wheel));
        fs::write(&path, b"abd").unwrap();
        assert!(!verified(&path, &wheel));
    }
}
