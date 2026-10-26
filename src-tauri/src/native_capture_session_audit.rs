//! Passive bounded receipts plus sealed proof from direct native COM observations.
//! Serialized audit data has no admission/lease authority and is never an IPC token.
use serde_json::{json, Value};
const ID_LIMIT: usize = 8192;
fn opaque(value: &str) -> Result<String, String> {
    if value.is_empty() || value.len() > ID_LIMIT || value.contains('\0') {
        Err("invalid opaque native session ID".into())
    } else {
        Ok(value.into())
    }
}
fn instance(v: &Value) -> Option<&str> {
    v.pointer("/instanceId/value")
        .and_then(Value::as_str)
        .filter(|v| opaque(v).is_ok())
}
fn comparison(a: &Value, b: &Value) -> Value {
    match (instance(a), instance(b)) {
        (Some(a), Some(b)) => json!(a == b),
        _ => Value::Null,
    }
}

use crate::capture_routing::{current_native_process_identity, ApplicationIdentity};
use crate::endpoint_packet_sink::SourcePacketScope;
use sha2::{Digest, Sha256};
use std::{marker::PhantomData, rc::Rc};
use windows::{
    core::{Interface, GUID, HRESULT, PWSTR},
    Win32::{
        Foundation::FILETIME,
        Media::Audio::{
            IAudioClient, IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
            IMMDevice, AUDCLNT_SHAREMODE_SHARED, WAVEFORMATEX,
        },
        System::{
            Com::{CoCreateGuid, CoTaskMemFree, CLSCTX_ALL},
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    },
};
fn bounded(v: &str) -> String {
    let mut end = v.len().min(1024);
    while !v.is_char_boundary(end) {
        end -= 1
    }
    v[..end].into()
}
fn hr(h: HRESULT) -> Value {
    json!({"signed":h.0,"hex":format!("0x{:08X}",h.0 as u32),"succeeded":h.is_ok()})
}
fn api_error(e: &windows::core::Error) -> Value {
    json!({"hresult":hr(e.code()),"error":bounded(&e.to_string())})
}
fn native_string(value: windows::core::Result<PWSTR>) -> Value {
    match value {
        Err(e) => api_error(&e),
        Ok(ptr) => {
            // OS API promises NUL-terminated owned UTF16. Bound the scan before allocation.
            let mut chars = Vec::new();
            let mut terminated = false;
            if !ptr.is_null() {
                for i in 0..=ID_LIMIT {
                    let c = unsafe { *ptr.0.add(i) };
                    if c == 0 {
                        terminated = true;
                        break;
                    }
                    chars.push(c);
                }
            }
            unsafe { CoTaskMemFree(Some(ptr.0.cast())) };
            let decoded = if terminated {
                String::from_utf16(&chars)
                    .map_err(|e| e.to_string())
                    .and_then(|s| opaque(&s))
            } else {
                Err("native opaque ID missing/oversize".into())
            };
            match decoded {
                Ok(s) => json!({"hresult":hr(HRESULT(0)),"value":s}),
                Err(e) => json!({"hresult":hr(HRESULT(0)),"observationError":bounded(&e)}),
            }
        }
    }
}
fn service(client: &IAudioClient) -> windows::core::Result<IAudioSessionControl2> {
    let c: IAudioSessionControl = unsafe { client.GetService() }?;
    c.cast()
}
#[derive(Clone)]
pub(crate) struct NativeProof {
    instance: String,
    process: ApplicationIdentity,
    scope: SourcePacketScope,
}
impl NativeProof {
    pub(crate) fn matches(&self, instance: &str, process: &ApplicationIdentity) -> bool {
        self.instance == instance && self.process == *process
    }
    pub(crate) fn process_matches(&self, process: &ApplicationIdentity) -> bool {
        self.process == *process
    }
    pub(crate) fn bound_to(&self, scope: &SourcePacketScope) -> bool {
        self.scope == *scope
    }
}
struct DirectProof {
    instance: String,
    pid: u32,
    state: i32,
}
fn direct_instance(control: &IAudioSessionControl2) -> Result<String, String> {
    let ptr = unsafe { control.GetSessionInstanceIdentifier() }.map_err(|e| e.to_string())?;
    let mut chars = Vec::new();
    let mut terminated = false;
    if !ptr.is_null() {
        for i in 0..=ID_LIMIT {
            let c = unsafe { *ptr.0.add(i) };
            if c == 0 {
                terminated = true;
                break;
            }
            chars.push(c);
        }
    }
    unsafe { CoTaskMemFree(Some(ptr.0.cast())) };
    if !terminated {
        return Err("native proof instance unavailable/oversize".into());
    }
    opaque(&String::from_utf16(&chars).map_err(|e| e.to_string())?)
}
fn direct(control: &IAudioSessionControl2) -> Result<DirectProof, String> {
    let mut pid = 0;
    let status =
        unsafe { (Interface::vtable(control).GetProcessId)(Interface::as_raw(control), &mut pid) };
    if status.0 != 0 || pid != std::process::id() {
        return Err("native proof PID is not S_OK current process".into());
    }
    let state = unsafe { control.GetState() }.map_err(|e| e.to_string())?.0;
    if !(0..=2).contains(&state) {
        return Err("native proof state unknown".into());
    }
    Ok(DirectProof {
        instance: direct_instance(control)?,
        pid,
        state,
    })
}
fn native_proof(
    client: &IAudioClient,
    manager: &IAudioSessionManager2,
    guid: &GUID,
    expected_state: i32,
) -> Result<DirectProof, String> {
    let actual = direct(&service(client).map_err(|e| e.to_string())?)?;
    let private: IAudioSessionControl2 =
        unsafe { manager.GetAudioSessionControl(Some(guid as *const GUID), 0) }
            .and_then(|v| v.cast())
            .map_err(|e| e.to_string())?;
    let private = direct(&private)?;
    let default: IAudioSessionControl2 = unsafe { manager.GetAudioSessionControl(None, 0) }
        .and_then(|v| v.cast())
        .map_err(|e| e.to_string())?;
    let default_state = unsafe { default.GetState() }.map_err(|e| e.to_string())?.0;
    verify_control_relationship(
        &actual,
        &private,
        &direct_instance(&default)?,
        default_state,
        expected_state,
    )?;
    Ok(actual)
}
fn verify_control_relationship(
    actual: &DirectProof,
    private: &DirectProof,
    default_instance: &str,
    default_state: i32,
    expected_state: i32,
) -> Result<(), String> {
    if !(0..=2).contains(&default_state)
        || default_instance == actual.instance
        || private.instance != actual.instance
        || actual.state != expected_state
        || private.state != expected_state
        || private.pid != actual.pid
    {
        return Err("native private/default/direct capture proof mismatch".into());
    }
    Ok(())
}
fn control(c: &IAudioSessionControl2) -> Value {
    let mut pid = 0;
    let status = unsafe { (Interface::vtable(c).GetProcessId)(Interface::as_raw(c), &mut pid) };
    let state = match unsafe { c.GetState() } {
        Ok(v) => json!({"hresult":hr(HRESULT(0)),"value":v.0}),
        Err(e) => api_error(&e),
    };
    json!({"state":state,"sessionId":native_string(unsafe{c.GetSessionIdentifier()}),"instanceId":native_string(unsafe{c.GetSessionInstanceIdentifier()}),"processId":{"hresult":hr(status),"pid":pid}})
}
fn service_value(c: &windows::core::Result<IAudioSessionControl2>) -> Value {
    match c {
        Ok(v) => control(v),
        Err(e) => api_error(e),
    }
}
fn manager_value(m: &IAudioSessionManager2, guid: Option<&GUID>) -> Value {
    match unsafe { m.GetAudioSessionControl(guid.map(|g| g as *const GUID), 0) } {
        Err(e) => api_error(&e),
        Ok(v) => match v.cast::<IAudioSessionControl2>() {
            Ok(v) => control(&v),
            Err(e) => api_error(&e),
        },
    }
}
fn process_identity() -> Value {
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    let creation = match unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u)
    } {
        Ok(()) => {
            json!({"hresult":hr(HRESULT(0)),"value":((u64::from(c.dwHighDateTime)<<32)|u64::from(c.dwLowDateTime)).to_string()})
        }
        Err(e) => api_error(&e),
    };
    json!({"pid":std::process::id(),"creationFiletime":creation,"imagePath":std::env::current_exe().ok()})
}
/// MTA-local private session construction and diagnostics. Only `verify_live`
/// reads typed direct COM proof; the exported receipt remains passive data.
pub(crate) struct NativeCaptureSessionAudit {
    guid: GUID,
    scope: SourcePacketScope,
    manager: IAudioSessionManager2,
    before: IAudioSessionControl2,
    proof_before: (DirectProof, ApplicationIdentity),
    receipt: Value,
    _mta: PhantomData<Rc<()>>,
}
impl NativeCaptureSessionAudit {
    pub(crate) fn initialize(
        client: &IAudioClient,
        device: &IMMDevice,
        scope: &SourcePacketScope,
        format: &WAVEFORMATEX,
        flags: u32,
        duration: i64,
    ) -> Result<Self, String> {
        if flags & windows::Win32::Media::Audio::AUDCLNT_STREAMFLAGS_LOOPBACK == 0 {
            return Err("private native session construction is capture-only".into());
        }
        let actual_endpoint = wasapi::Device::from_immdevice(device.clone())
            .map_err(|e| e.to_string())?
            .get_id()
            .map_err(|e| e.to_string())?;
        if actual_endpoint != scope.endpoint_id() {
            return Err("private native session endpoint binding mismatch".into());
        }
        let guid = unsafe { CoCreateGuid() }
            .map_err(|e| format!("private capture GUID: {e} ({:#010X})", e.code().0 as u32))?;
        unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                flags,
                duration,
                0,
                format,
                Some(&guid),
            )
        }
        .map_err(|e| {
            format!(
                "private raw capture Initialize: {e} ({:#010X})",
                e.code().0 as u32
            )
        })?;
        let before = service(client).map_err(|e| e.to_string())?;
        let manager = unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| e.to_string())?;
        let own = control(&before);
        let private = manager_value(&manager, Some(&guid));
        let default = manager_value(&manager, None);
        let proof_before = (
            native_proof(client, &manager, &guid, 0)?,
            current_native_process_identity()?,
        );
        let channels = format.nChannels;
        let rate = format.nSamplesPerSec;
        let bits = format.wBitsPerSample;
        let receipt = json!({"schemaVersion":1,"kind":"passiveNativeCaptureSessionAudit","authority":false,"scope":{"sourceId":scope.source_id().to_string(),"epoch":scope.epoch(),"endpointId":scope.endpoint_id(),"sampleRate":scope.sample_rate(),"commonOriginHns":scope.common_origin_hns().to_string()},"captureProcess":process_identity(),"initialize":{"hresult":hr(HRESULT(0)),"sessionGuidMode":"freshPrivatePerClient","privateGuidFingerprint":format!("{:x}",Sha256::digest(format!("{guid:?}").as_bytes())),"flags":flags,"durationHns":duration,"channels":channels,"sampleRate":rate,"bits":bits},"beforeStart":{"ownControl":own,"managerPrivate":private,"managerProcessDefault":default,"ownEqualsPrivate":comparison(&own,&private),"ownEqualsProcessDefault":comparison(&own,&default),"privateEqualsProcessDefault":comparison(&private,&default)},"afterStart":null,"beforeAfterInstanceEqual":null});
        Ok(Self {
            guid,
            scope: scope.clone(),
            manager,
            before,
            proof_before,
            receipt,
            _mta: PhantomData,
        })
    }
    pub(crate) fn observe_after_start(&mut self, client: &IAudioClient) {
        let after = service(client);
        let own = service_value(&after);
        let private = manager_value(&self.manager, Some(&self.guid));
        let default = manager_value(&self.manager, None);
        self.receipt["afterStart"] = json!({"ownControl":own,"managerPrivate":private,"managerProcessDefault":default,"ownEqualsPrivate":comparison(&own,&private),"ownEqualsProcessDefault":comparison(&own,&default),"privateEqualsProcessDefault":comparison(&private,&default)});
        self.receipt["beforeAfterInstanceEqual"] =
            comparison(&self.receipt["beforeStart"]["ownControl"], &own);
        // Keep the directly client-bound preStart COM reference alive through postStart.
        let _ = &self.before;
    }
    pub(crate) fn receipt(&self) -> Result<Value, String> {
        if serde_json::to_vec(&self.receipt)
            .map_err(|e| e.to_string())?
            .len()
            > 131072
        {
            return Err("native session audit receipt exceeded128KiB".into());
        }
        Ok(self.receipt.clone())
    }
    /// Only direct live COM observations can mint this sealed non-Serde value.
    /// Saved receipts, scopes, GUID text and policy tokens are never inputs.
    pub(crate) fn verify_live(&self, client: &IAudioClient) -> Result<NativeProof, String> {
        let (before, process) = &self.proof_before;
        let after = native_proof(client, &self.manager, &self.guid, 1)?;
        let current = current_native_process_identity()?;
        if after.instance != before.instance || after.pid != before.pid || current != *process {
            return Err("native capture identity changed across Start".into());
        }
        Ok(NativeProof {
            instance: after.instance,
            process: current,
            scope: self.scope.clone(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_pre_start_proof_accepts_only_inactive_private_client_relationship() {
        let mut actual = DirectProof {
            instance: "private".into(),
            pid: 23,
            state: 0,
        };
        let mut private = DirectProof {
            instance: "private".into(),
            pid: 23,
            state: 0,
        };
        assert!(verify_control_relationship(&actual, &private, "default", 0, 0).is_ok());
        actual.state = 1;
        assert!(verify_control_relationship(&actual, &private, "default", 0, 0).is_err());
        actual.state = 0;
        private.state = 1;
        assert!(verify_control_relationship(&actual, &private, "default", 0, 0).is_err());
        private.state = 0;
        assert!(verify_control_relationship(&actual, &private, "default", 99, 0).is_err());
        assert!(verify_control_relationship(&actual, &private, "private", 0, 0).is_err());
    }
    #[test]
    fn typed_private_proof_rejects_wrong_instance_default_alias_and_unknown_state() {
        let actual = DirectProof {
            instance: "private-A".into(),
            pid: 23,
            state: 1,
        };
        let mut private = DirectProof {
            instance: "private-A".into(),
            pid: 23,
            state: 1,
        };
        assert!(verify_control_relationship(&actual, &private, "default", 0, 1).is_ok());
        assert!(verify_control_relationship(&actual, &private, "private-A", 0, 1).is_err());
        assert!(verify_control_relationship(&actual, &private, "default", 99, 1).is_err());
        assert!(verify_control_relationship(&actual, &private, "default", 0, 0).is_err());
        private.instance = "different".into();
        assert!(verify_control_relationship(&actual, &private, "default", 0, 1).is_err());
        private.instance = "private-A".into();
        private.pid = 24;
        assert!(verify_control_relationship(&actual, &private, "default", 0, 1).is_err());
    }
    #[test]
    fn sealed_exact_tuple_does_not_admit_same_pid_foreign_instance_or_generation() {
        let identity = ApplicationIdentity {
            pid: 23,
            parent_pid: 1,
            creation_filetime: "1000".into(),
            image_path: "C:\\fung.exe".into(),
            image_volume_serial: 1,
            image_file_id: "0000000000000001".into(),
        };
        // Local unit fixture only: no factory ticket or COM owner can be minted.
        let proof = NativeProof {
            instance: "private-A".into(),
            process: identity.clone(),
            scope: SourcePacketScope::new(
                uuid::Uuid::new_v4(),
                1,
                "endpoint-A".into(),
                48_000,
                1000,
            )
            .unwrap(),
        };
        assert!(proof.bound_to(&proof.scope));
        let other_epoch = SourcePacketScope::new(
            proof.scope.source_id(),
            2,
            "endpoint-A".into(),
            48_000,
            1000,
        )
        .unwrap();
        assert!(!proof.bound_to(&other_epoch));
        let other_endpoint = SourcePacketScope::new(
            proof.scope.source_id(),
            1,
            "endpoint-B".into(),
            48_000,
            1000,
        )
        .unwrap();
        assert!(!proof.bound_to(&other_endpoint));
        let other_source =
            SourcePacketScope::new(uuid::Uuid::new_v4(), 1, "endpoint-A".into(), 48_000, 1000)
                .unwrap();
        assert!(!proof.bound_to(&other_source));
        assert!(proof.matches("private-A", &identity));
        assert!(!proof.matches("default", &identity));
        let mut changed = identity.clone();
        changed.creation_filetime = "1001".into();
        assert!(!proof.matches("private-A", &changed));
        changed = identity.clone();
        changed.image_file_id = "0000000000000002".into();
        assert!(!proof.matches("private-A", &changed));
        changed = identity;
        changed.pid = 24;
        assert!(!proof.matches("private-A", &changed));
    }
    #[test]
    fn opaque_equal_different_and_unknown_are_distinct() {
        let a = json!({"instanceId":{"value":"opaque-a"}});
        let b = json!({"instanceId":{"value":"opaque-b"}});
        assert_eq!(comparison(&a, &a), true);
        assert_eq!(comparison(&a, &b), false);
        assert!(comparison(&a, &json!({"error":"unsupported"})).is_null());
    }
    #[test]
    fn native_opaque_ids_are_bounded_and_not_truncated() {
        assert!(opaque("").is_err());
        assert!(opaque(&"x".repeat(8193)).is_err());
        assert!(opaque("a\0b").is_err());
        assert_eq!(
            opaque("opaque-any-structure").unwrap(),
            "opaque-any-structure"
        );
    }
}
