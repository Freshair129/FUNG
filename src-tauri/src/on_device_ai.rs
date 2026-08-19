use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime, State, Wry,
};

use crate::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeviceTier {
    Core,
    AiLite,
    AiStandard,
    AiPro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AiTaskKind {
    Stt,
    Embedding,
    Llm,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeKind {
    WhisperCpp,
    OnnxRuntimeMobile,
    LlamaCpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThermalStatus {
    Nominal,
    Moderate,
    Severe,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceProfile {
    pub(crate) sdk_int: u32,
    pub(crate) arm64: bool,
    pub(crate) total_ram_mb: u64,
    pub(crate) available_ram_mb: u64,
    pub(crate) free_storage_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeStatus {
    pub(crate) probe_available: bool,
    pub(crate) tier: DeviceTier,
    pub(crate) reason: String,
    pub(crate) sdk_int: Option<u32>,
    pub(crate) arm64: Option<bool>,
    pub(crate) total_ram_mb: Option<u64>,
    pub(crate) available_ram_mb: Option<u64>,
    pub(crate) free_storage_mb: Option<u64>,
}

pub(crate) fn runtime_status(profile: Option<DeviceProfile>) -> RuntimeStatus {
    match profile {
        None => RuntimeStatus {
            probe_available: false,
            tier: DeviceTier::Core,
            reason: "android_profile_unavailable".to_string(),
            sdk_int: None,
            arm64: None,
            total_ram_mb: None,
            available_ram_mb: None,
            free_storage_mb: None,
        },
        Some(profile) => {
            let tier = evaluate_device_tier(&profile);
            let admission = admit_task(tier, &ResourceSnapshot::idle(), AiTaskKind::Stt);
            let reason = if admission.is_allowed() {
                "model_package_pending_approval"
            } else {
                admission.reason().expect("deferred admission has a reason")
            };
            RuntimeStatus {
                probe_available: true,
                tier,
                reason: reason.to_string(),
                sdk_int: Some(profile.sdk_int),
                arm64: Some(profile.arm64),
                total_ram_mb: Some(profile.total_ram_mb),
                available_ram_mb: Some(profile.available_ram_mb),
                free_storage_mb: Some(profile.free_storage_mb),
            }
        }
    }
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct ProfileRequest {}

pub(crate) struct AndroidProfileProbe<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    marker: std::marker::PhantomData<fn() -> R>,
}

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("on-device-ai-profile")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let probe: AndroidProfileProbe<R> = AndroidProfileProbe {
                handle: _api
                    .register_android_plugin("dev.fung.local.aiprofile", "AiProfilePlugin")?,
            };
            #[cfg(not(target_os = "android"))]
            let probe: AndroidProfileProbe<R> = AndroidProfileProbe {
                marker: std::marker::PhantomData,
            };
            app.manage(probe);
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
fn read_android_profile(probe: &AndroidProfileProbe<Wry>) -> AppResult<DeviceProfile> {
    probe
        .handle
        .run_mobile_plugin("profile", ProfileRequest {})
        .map_err(|error| AppError::InvalidInput(format!("android AI profile: {error}")))
}

#[cfg(not(target_os = "android"))]
fn read_android_profile(_probe: &AndroidProfileProbe<Wry>) -> AppResult<DeviceProfile> {
    Err(AppError::InvalidInput(
        "Android AI profile is unavailable on this platform".to_string(),
    ))
}

#[tauri::command]
pub(crate) fn mobile_on_device_ai_status(
    probe: State<'_, AndroidProfileProbe<Wry>>,
) -> RuntimeStatus {
    runtime_status(read_android_profile(&probe).ok())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceSnapshot {
    pub(crate) capture_active: bool,
    pub(crate) thermal_status: ThermalStatus,
    pub(crate) battery_percent: u8,
    pub(crate) charging: bool,
    pub(crate) memory_pressure: bool,
}

impl ResourceSnapshot {
    pub(crate) fn idle() -> Self {
        Self {
            capture_active: false,
            thermal_status: ThermalStatus::Nominal,
            battery_percent: 100,
            charging: false,
            memory_pressure: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Admission {
    Allowed,
    Deferred { reason: &'static str },
}

impl Admission {
    pub(crate) fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub(crate) fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::Deferred { reason } => Some(reason),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct PackageManifest {
    pub(crate) package_id: String,
    pub(crate) task_kind: AiTaskKind,
    pub(crate) runtime_kind: RuntimeKind,
    pub(crate) artifact_sha256: String,
    pub(crate) size_bytes: u64,
    pub(crate) license_ref: String,
    pub(crate) signature: String,
    pub(crate) minimum_tier: DeviceTier,
}

#[allow(dead_code)]
impl PackageManifest {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.package_id.trim().is_empty()
            || !self.package_id.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '-' | '_')
            })
        {
            return Err("package_id_invalid");
        }
        if self.artifact_sha256.len() != 64
            || !self
                .artifact_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err("artifact_sha256_invalid");
        }
        if self.size_bytes == 0 {
            return Err("size_bytes_invalid");
        }
        if self.license_ref.trim().is_empty() {
            return Err("license_ref_missing");
        }
        if self.signature.trim().is_empty() {
            return Err("signature_missing");
        }
        if self.minimum_tier == DeviceTier::Core {
            return Err("minimum_tier_invalid");
        }
        if !matches!(
            (self.task_kind, self.runtime_kind),
            (AiTaskKind::Stt, RuntimeKind::WhisperCpp)
                | (AiTaskKind::Embedding, RuntimeKind::OnnxRuntimeMobile)
                | (AiTaskKind::Llm, RuntimeKind::LlamaCpp)
        ) {
            return Err("runtime_task_mismatch");
        }
        Ok(())
    }
}

pub(crate) fn evaluate_device_tier(profile: &DeviceProfile) -> DeviceTier {
    if !profile.arm64
        || profile.sdk_int < 29
        || profile.total_ram_mb < 6 * 1024
        || profile.available_ram_mb < 3 * 1024
        || profile.free_storage_mb < 8 * 1024
    {
        return DeviceTier::Core;
    }
    if profile.sdk_int >= 33
        && profile.total_ram_mb >= 12 * 1024
        && profile.available_ram_mb >= 6 * 1024
        && profile.free_storage_mb >= 16 * 1024
    {
        return DeviceTier::AiPro;
    }
    if profile.sdk_int >= 31
        && profile.total_ram_mb >= 8 * 1024
        && profile.available_ram_mb >= 4_608
        && profile.free_storage_mb >= 12 * 1024
    {
        return DeviceTier::AiStandard;
    }
    DeviceTier::AiLite
}

pub(crate) fn admit_task(
    tier: DeviceTier,
    resources: &ResourceSnapshot,
    task: AiTaskKind,
) -> Admission {
    if tier == DeviceTier::Core {
        return Admission::Deferred {
            reason: "device_tier_core",
        };
    }
    if resources.capture_active {
        return Admission::Deferred {
            reason: "capture_active",
        };
    }
    if resources.thermal_status == ThermalStatus::Severe {
        return Admission::Deferred {
            reason: "thermal_severe",
        };
    }
    if resources.memory_pressure {
        return Admission::Deferred {
            reason: "memory_pressure",
        };
    }
    if resources.battery_percent < 20 && !resources.charging {
        return Admission::Deferred {
            reason: "battery_low",
        };
    }
    if task == AiTaskKind::Llm && tier == DeviceTier::AiLite {
        return Admission::Deferred {
            reason: "device_tier_insufficient",
        };
    }
    Admission::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_at_standard_threshold_is_eligible_for_stt_embedding_and_llm_lite() {
        let profile = DeviceProfile {
            sdk_int: 31,
            arm64: true,
            total_ram_mb: 8 * 1024,
            available_ram_mb: 4_608,
            free_storage_mb: 12 * 1024,
        };

        assert_eq!(evaluate_device_tier(&profile), DeviceTier::AiStandard);
        assert!(admit_task(
            DeviceTier::AiStandard,
            &ResourceSnapshot::idle(),
            AiTaskKind::Llm,
        )
        .is_allowed());
    }

    #[test]
    fn active_capture_or_severe_thermal_state_defers_heavy_inference() {
        let capture = ResourceSnapshot {
            capture_active: true,
            ..ResourceSnapshot::idle()
        };
        let thermal = ResourceSnapshot {
            thermal_status: ThermalStatus::Severe,
            ..ResourceSnapshot::idle()
        };

        assert_eq!(
            admit_task(DeviceTier::AiPro, &capture, AiTaskKind::Stt).reason(),
            Some("capture_active")
        );
        assert_eq!(
            admit_task(DeviceTier::AiPro, &thermal, AiTaskKind::Embedding).reason(),
            Some("thermal_severe")
        );
    }

    #[test]
    fn package_manifest_requires_hash_license_signature_and_matching_tier() {
        let valid = PackageManifest {
            package_id: "fung.stt.thai.standard".to_string(),
            task_kind: AiTaskKind::Stt,
            runtime_kind: RuntimeKind::WhisperCpp,
            artifact_sha256: "a".repeat(64),
            size_bytes: 700 * 1024 * 1024,
            license_ref: "Apache-2.0".to_string(),
            signature: "release-channel-signature".to_string(),
            minimum_tier: DeviceTier::AiStandard,
        };

        assert!(valid.validate().is_ok());
        assert!(PackageManifest {
            artifact_sha256: "bad".to_string(),
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(PackageManifest {
            license_ref: String::new(),
            ..valid
        }
        .validate()
        .is_err());
    }

    #[test]
    fn status_is_truthful_when_android_profile_or_approved_package_is_missing() {
        let unavailable = runtime_status(None);
        assert!(!unavailable.probe_available);
        assert_eq!(unavailable.reason, "android_profile_unavailable");

        let profile = DeviceProfile {
            sdk_int: 31,
            arm64: true,
            total_ram_mb: 8 * 1024,
            available_ram_mb: 4_608,
            free_storage_mb: 12 * 1024,
        };
        let pending = runtime_status(Some(profile));
        assert!(pending.probe_available);
        assert_eq!(pending.tier, DeviceTier::AiStandard);
        assert_eq!(pending.reason, "model_package_pending_approval");
    }
}
