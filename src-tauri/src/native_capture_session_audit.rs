//! Passive bounded diagnostic only. No admission/lease authority and no IPC token.
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
fn manager_value(m: &windows::core::Result<IAudioSessionManager2>, guid: Option<&GUID>) -> Value {
    match m {
        Err(e) => api_error(e),
        Ok(m) => match unsafe { m.GetAudioSessionControl(guid.map(|g| g as *const GUID), 0) } {
            Err(e) => api_error(&e),
            Ok(v) => match v.cast::<IAudioSessionControl2>() {
                Ok(v) => control(&v),
                Err(e) => api_error(&e),
            },
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
/// MTA-local diagnostic collector, NOT a lease or admission capability.
pub(crate) struct NativeCaptureSessionAudit {
    guid: GUID,
    manager: windows::core::Result<IAudioSessionManager2>,
    before: windows::core::Result<IAudioSessionControl2>,
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
        let before = service(client);
        let manager = unsafe { device.Activate(CLSCTX_ALL, None) };
        let own = service_value(&before);
        let private = manager_value(&manager, Some(&guid));
        let default = manager_value(&manager, None);
        let channels = format.nChannels;
        let rate = format.nSamplesPerSec;
        let bits = format.wBitsPerSample;
        let receipt = json!({"schemaVersion":1,"kind":"passiveNativeCaptureSessionAudit","authority":false,"scope":{"sourceId":scope.source_id().to_string(),"epoch":scope.epoch(),"endpointId":scope.endpoint_id(),"sampleRate":scope.sample_rate(),"commonOriginHns":scope.common_origin_hns().to_string()},"captureProcess":process_identity(),"initialize":{"hresult":hr(HRESULT(0)),"sessionGuidMode":"freshPrivatePerClient","privateGuidFingerprint":format!("{:x}",Sha256::digest(format!("{guid:?}").as_bytes())),"flags":flags,"durationHns":duration,"channels":channels,"sampleRate":rate,"bits":bits},"beforeStart":{"ownControl":own,"managerPrivate":private,"managerProcessDefault":default,"ownEqualsPrivate":comparison(&own,&private),"ownEqualsProcessDefault":comparison(&own,&default),"privateEqualsProcessDefault":comparison(&private,&default)},"afterStart":null,"beforeAfterInstanceEqual":null});
        Ok(Self {
            guid,
            manager,
            before,
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
}
#[cfg(test)]
mod tests {
    use super::*;
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
