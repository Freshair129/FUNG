use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::State;
use uuid::Uuid;

use crate::{AppError, AppResult, AppState};

const CONFIG_FILE: &str = "recording-output.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingOutputStatus {
    pub(crate) current_path: String,
    pub(crate) default_path: String,
    pub(crate) is_default: bool,
    pub(crate) writable: bool,
    pub(crate) issue: Option<String>,
    pub(crate) capture_active: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingOutputConfig {
    current_path: Option<String>,
    default_path: Option<String>,
    #[serde(default)]
    known_paths: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct RecordingOutputManager {
    data_root: PathBuf,
    default_root: PathBuf,
    current_root: PathBuf,
    known_roots: Vec<PathBuf>,
}

impl RecordingOutputManager {
    pub(crate) fn load(data_root: PathBuf, default_root: PathBuf) -> Result<Self, String> {
        let config_path = data_root.join(CONFIG_FILE);
        let config = if config_path.is_file() {
            let raw = fs::read_to_string(&config_path)
                .map_err(|error| format!("อ่านการตั้งค่า output ไม่สำเร็จ: {error}"))?;
            serde_json::from_str::<RecordingOutputConfig>(&raw)
                .map_err(|error| format!("การตั้งค่า output ไม่ถูกต้อง: {error}"))?
        } else {
            RecordingOutputConfig::default()
        };

        let current_path_missing = config.current_path.is_none();
        let current_root = config
            .current_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| default_root.clone());
        let mut known_roots = Vec::new();
        push_unique_path(&mut known_roots, data_root.clone());
        push_unique_path(&mut known_roots, default_root.clone());
        push_unique_path(&mut known_roots, current_root.clone());
        if let Some(path) = config.default_path.as_deref() {
            push_unique_path(&mut known_roots, PathBuf::from(path));
        }
        for path in config.known_paths {
            push_unique_path(&mut known_roots, PathBuf::from(path));
        }

        let manager = Self {
            data_root,
            default_root,
            current_root,
            known_roots,
        };
        if current_path_missing || same_path(&manager.current_root, &manager.default_root) {
            // The first-run default is allowed to be created. A previously
            // selected custom path is deliberately not recreated when it was
            // deleted, so the UI can report the loss instead of hiding it.
            let _ = validate_writable(&manager.default_root, true);
        }
        manager.persist().map_err(|error| {
            format!(
                "บันทึกการตั้งค่า output ไปยัง {} ไม่สำเร็จ: {error}",
                manager.config_path().display()
            )
        })?;
        Ok(manager)
    }

    fn config_path(&self) -> PathBuf {
        self.data_root.join(CONFIG_FILE)
    }

    fn persist(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.data_root)?;
        let config = RecordingOutputConfig {
            current_path: Some(self.current_root.display().to_string()),
            default_path: Some(self.default_root.display().to_string()),
            known_paths: self
                .known_roots
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        };
        let encoded = serde_json::to_vec_pretty(&config)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        fs::write(self.config_path(), encoded)
    }

    pub(crate) fn current_root(&self) -> PathBuf {
        self.current_root.clone()
    }

    #[cfg(test)]
    pub(crate) fn known_roots(&self) -> Vec<PathBuf> {
        self.known_roots.clone()
    }

    pub(crate) fn status(&self, capture_active: bool) -> RecordingOutputStatus {
        let issue = validate_writable(&self.current_root, false).err();
        RecordingOutputStatus {
            current_path: self.current_root.display().to_string(),
            default_path: self.default_root.display().to_string(),
            is_default: same_path(&self.current_root, &self.default_root),
            writable: issue.is_none(),
            issue,
            capture_active,
        }
    }

    pub(crate) fn set_root(&mut self, path: PathBuf) -> Result<(), String> {
        validate_writable(&path, true)?;
        let previous = self.current_root.clone();
        self.current_root = path;
        push_unique_path(&mut self.known_roots, self.current_root.clone());
        if let Err(error) = self.persist() {
            self.current_root = previous;
            return Err(format!("บันทึกการตั้งค่า output ไม่สำเร็จ: {error}"));
        }
        Ok(())
    }

    pub(crate) fn reset(&mut self) -> Result<(), String> {
        self.set_root(self.default_root.clone())
    }

    pub(crate) fn ensure_current_writable(&self) -> Result<PathBuf, String> {
        validate_writable(&self.current_root, false)?;
        Ok(self.current_root())
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| same_path(existing, &path)) {
        paths.push(path);
    }
}

fn validate_writable(path: &Path, create_if_missing: bool) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("โฟลเดอร์ output ต้องเป็น absolute path".to_string());
    }
    if create_if_missing {
        fs::create_dir_all(path).map_err(|error| format!("สร้างโฟลเดอร์ output ไม่สำเร็จ: {error}"))?;
    } else if !path.exists() {
        return Err("ไม่พบโฟลเดอร์ output ที่เลือก".to_string());
    }
    if !path.is_dir() {
        return Err("ปลายทาง output ไม่ใช่โฟลเดอร์".to_string());
    }

    let probe = path.join(format!(".fung-output-probe-{}", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|error| format!("เขียนโฟลเดอร์ output ไม่ได้: {error}"))?;
    file.write_all(b"fung output probe")
        .map_err(|error| format!("ตรวจสอบการเขียน output ไม่ได้: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("ยืนยันการเขียน output ไม่ได้: {error}"))?;
    drop(file);
    fs::remove_file(&probe).map_err(|error| format!("ลบไฟล์ตรวจสอบ output ไม่ได้: {error}"))?;
    Ok(())
}

pub(crate) fn known_roots_from_config(data_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    push_unique_path(&mut roots, data_root.to_path_buf());
    let path = data_root.join(CONFIG_FILE);
    let Ok(raw) = fs::read_to_string(path) else {
        return roots;
    };
    let Ok(config) = serde_json::from_str::<RecordingOutputConfig>(&raw) else {
        return roots;
    };
    if let Some(path) = config.default_path {
        push_unique_path(&mut roots, PathBuf::from(path));
    }
    if let Some(path) = config.current_path {
        push_unique_path(&mut roots, PathBuf::from(path));
    }
    for path in config.known_paths {
        push_unique_path(&mut roots, PathBuf::from(path));
    }
    roots
}

#[tauri::command]
pub(crate) fn recording_output_get(state: State<'_, AppState>) -> AppResult<RecordingOutputStatus> {
    let capture_active = state.native_capture.capture_is_active();
    let manager = state
        .recording_output
        .lock()
        .expect("recording output mutex poisoned");
    Ok(manager.status(capture_active))
}

#[tauri::command]
pub(crate) fn recording_output_set(
    path: String,
    state: State<'_, AppState>,
) -> AppResult<RecordingOutputStatus> {
    if state.native_capture.capture_is_active() {
        return Err(AppError::InvalidInput(
            "เปลี่ยนโฟลเดอร์ไม่ได้ขณะกำลังบันทึก".to_string(),
        ));
    }
    let mut manager = state
        .recording_output
        .lock()
        .expect("recording output mutex poisoned");
    manager
        .set_root(PathBuf::from(path))
        .map_err(AppError::InvalidInput)?;
    Ok(manager.status(false))
}

#[tauri::command]
pub(crate) fn recording_output_reset(
    state: State<'_, AppState>,
) -> AppResult<RecordingOutputStatus> {
    if state.native_capture.capture_is_active() {
        return Err(AppError::InvalidInput(
            "คืนค่าโฟลเดอร์ไม่ได้ขณะกำลังบันทึก".to_string(),
        ));
    }
    let mut manager = state
        .recording_output
        .lock()
        .expect("recording output mutex poisoned");
    manager.reset().map_err(AppError::InvalidInput)?;
    Ok(manager.status(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_and_custom_roots_are_persisted_and_known() {
        let data = tempdir().unwrap();
        let default_root = data.path().join("documents").join("fung");
        let mut manager =
            RecordingOutputManager::load(data.path().to_path_buf(), default_root.clone()).unwrap();
        assert!(manager.status(false).is_default);
        let custom = data.path().join("custom");
        manager.set_root(custom.clone()).unwrap();
        assert_eq!(manager.current_root(), custom);

        let reloaded =
            RecordingOutputManager::load(data.path().to_path_buf(), default_root).unwrap();
        assert_eq!(reloaded.current_root(), custom);
        assert!(reloaded.known_roots().iter().any(|path| path == &custom));
    }

    #[test]
    fn unavailable_root_is_reported_without_fallback() {
        let data = tempdir().unwrap();
        let missing = data.path().join("missing").join("output");
        let mut manager =
            RecordingOutputManager::load(data.path().to_path_buf(), missing.clone()).unwrap();
        fs::remove_dir_all(&missing).unwrap();
        manager.current_root = missing.clone();
        let status = manager.status(false);
        assert!(!status.writable);
        assert_eq!(status.current_path, missing.display().to_string());
        assert!(status.is_default);
        assert!(status.issue.is_some());
    }

    #[test]
    fn relative_root_is_rejected() {
        let data = tempdir().unwrap();
        let mut manager =
            RecordingOutputManager::load(data.path().to_path_buf(), data.path().join("documents"))
                .unwrap();
        assert!(manager.set_root(PathBuf::from("relative-output")).is_err());
    }

    #[test]
    fn config_reader_keeps_internal_and_previous_roots() {
        let data = tempdir().unwrap();
        let default_root = data.path().join("documents");
        let previous = data.path().join("previous");
        let config = RecordingOutputConfig {
            current_path: Some(default_root.display().to_string()),
            default_path: Some(default_root.display().to_string()),
            known_paths: vec![previous.display().to_string()],
        };
        fs::write(
            data.path().join(CONFIG_FILE),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let roots = known_roots_from_config(data.path());
        assert!(roots.iter().any(|path| path == data.path()));
        assert!(roots.iter().any(|path| path == &default_root));
        assert!(roots.iter().any(|path| path == &previous));
    }
}
