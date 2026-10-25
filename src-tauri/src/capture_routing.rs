//! Native endpoint inventory and application-route admission.
//!
//! This read-only foundation does not capture audio or mutate OS routing.
//! Runtime capture must revalidate its private plan and monitor source state.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const ROUTE_POLICY_VERSION: u32 = 1;
pub const MAX_INVENTORY_AGE_MS: u64 = 5000;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", content = "value", rename_all = "camelCase")]
pub enum Observation<T> {
    Known(T),
    Unavailable(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationIdentity {
    pub pid: u32,
    pub parent_pid: u32,
    pub creation_filetime: String,
    pub image_path: String,
    pub image_volume_serial: u32,
    pub image_file_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EndpointState {
    Active,
    Disabled,
    NotPresent,
    Unplugged,
    Unknown(u32),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionState {
    Active,
    Inactive,
    Expired,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionOwner {
    SingleProcess(ApplicationIdentity),
    Ambiguous {
        pid: u32,
        hresult: i32,
        reason: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDescriptor {
    pub instance_id: Observation<String>,
    pub state: Observation<SessionState>,
    pub owner: Observation<SessionOwner>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixFormat {
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub sample_type: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointDescriptor {
    pub id: String,
    pub name: Observation<String>,
    pub state: Observation<EndpointState>,
    pub mix_format: Observation<MixFormat>,
    pub sessions: Observation<Vec<SessionDescriptor>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultRoles {
    pub console: String,
    pub multimedia: String,
    pub communications: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessObservation {
    pub pid: u32,
    pub identity: Observation<ApplicationIdentity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInventory {
    pub captured_at_ms: u64,
    pub process_snapshot_cutoff_filetime: String,
    pub endpoints: Vec<EndpointDescriptor>,
    pub defaults: Observation<DefaultRoles>,
    pub processes: Vec<ProcessObservation>,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRoute {
    pub source_id: String,
    pub label: String,
    pub endpoint_id: String,
    pub application: ApplicationIdentity,
    pub allowed_children: Vec<ApplicationIdentity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRoutePlan {
    pub policy_version: u32,
    pub routes: Vec<CaptureRoute>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteReadiness {
    WaitingForTarget,
    Armed,
}
// Deliberately no Deserialize or public constructor/fields. UI validation claims
// cannot create native admission. This token is still not a runtime privacy lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedRoutePlan {
    request: CaptureRoutePlan,
    validated_at_ms: u64,
    routes: Vec<ValidatedRoute>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedRoute {
    source_id: uuid::Uuid,
    endpoint_id: String,
    readiness: RouteReadiness,
    observed_active_session_ids: Vec<String>,
}
impl ValidatedRoutePlan {
    pub fn routes(&self) -> &[ValidatedRoute] {
        &self.routes
    }
    pub fn validated_at_ms(&self) -> u64 {
        self.validated_at_ms
    }
}
impl ValidatedRoute {
    pub fn observed_active_session_ids(&self) -> &[String] {
        &self.observed_active_session_ids
    }
    pub fn source_id(&self) -> uuid::Uuid {
        self.source_id
    }
    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }
    pub fn readiness(&self) -> &RouteReadiness {
        &self.readiness
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Rejection {
    PolicyVersion,
    RouteCount,
    InvalidSourceId,
    DuplicateSource,
    DuplicateEndpoint,
    InvalidInput,
    InventoryError,
    StaleInventory,
    MissingEndpoint,
    AmbiguousEndpoint,
    DefaultEndpoint,
    EndpointInactive,
    StaleProcess,
    InvalidChild,
    AmbiguousSession,
    UnknownSession,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RouteError {
    pub code: Rejection,
    pub message: String,
}
fn error(code: Rejection, message: impl Into<String>) -> RouteError {
    RouteError {
        code,
        message: message.into(),
    }
}
fn known<T>(value: &Observation<T>) -> Result<&T, RouteError> {
    match value {
        Observation::Known(v) => Ok(v),
        Observation::Unavailable(e) => Err(error(Rejection::InventoryError, e)),
    }
}
fn valid_identity(a: &ApplicationIdentity) -> bool {
    a.pid > 0
        && a.creation_filetime
            .parse::<u64>()
            .is_ok_and(|v| v > 0 && v.to_string() == a.creation_filetime)
        && std::path::Path::new(&a.image_path).is_absolute()
        && !a.image_path.contains('\0')
        && a.image_file_id.len() == 16
        && u64::from_str_radix(&a.image_file_id, 16).is_ok()
}
fn current_identity(
    expected: &ApplicationIdentity,
    inventory: &NativeInventory,
) -> Result<(), RouteError> {
    if !valid_identity(expected) {
        return Err(error(Rejection::InvalidInput, "invalid process identity"));
    }
    let rows = inventory
        .processes
        .iter()
        .filter(|p| p.pid == expected.pid)
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(error(
            Rejection::StaleProcess,
            "process absent or ambiguous",
        ));
    }
    if known(&rows[0].identity)? != expected {
        return Err(error(
            Rejection::StaleProcess,
            "PID birth/image/ancestry changed",
        ));
    }
    Ok(())
}
pub fn validate_route_plan(
    plan: &CaptureRoutePlan,
    inventory: &NativeInventory,
    now_ms: u64,
) -> Result<ValidatedRoutePlan, RouteError> {
    if plan.policy_version != ROUTE_POLICY_VERSION {
        return Err(error(Rejection::PolicyVersion, "unsupported route policy"));
    }
    if plan.routes.is_empty() || plan.routes.len() > 2 {
        return Err(error(
            Rejection::RouteCount,
            "one or two independent routes required",
        ));
    }
    if now_ms < inventory.captured_at_ms || now_ms - inventory.captured_at_ms > MAX_INVENTORY_AGE_MS
    {
        return Err(error(
            Rejection::StaleInventory,
            "native snapshot expired or clock moved backwards",
        ));
    }
    if !inventory.errors.is_empty() {
        return Err(error(
            Rejection::InventoryError,
            inventory.errors.join("; "),
        ));
    }
    let defaults = known(&inventory.defaults)?;
    if [
        &defaults.console,
        &defaults.multimedia,
        &defaults.communications,
    ]
    .iter()
    .any(|id| id.is_empty())
    {
        return Err(error(Rejection::InventoryError, "default roles incomplete"));
    }
    let (mut sources, mut endpoints, mut processes) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut admitted = Vec::new();
    for route in &plan.routes {
        let source = uuid::Uuid::parse_str(&route.source_id)
            .map_err(|_| error(Rejection::InvalidSourceId, "source UUID required"))?;
        if source.is_nil() {
            return Err(error(Rejection::InvalidSourceId, "nil source UUID"));
        }
        if !sources.insert(source) {
            return Err(error(Rejection::DuplicateSource, "source reused"));
        }
        if !endpoints.insert(route.endpoint_id.clone()) {
            return Err(error(
                Rejection::DuplicateEndpoint,
                "one endpoint per source",
            ));
        }
        if route.endpoint_id.is_empty()
            || route.endpoint_id.len() > 4096
            || route.endpoint_id.contains('\0')
            || route.label.trim().is_empty()
            || route.label.chars().count() > 128
            || route.label.contains('\0')
            || route.allowed_children.len() > 32
        {
            return Err(error(Rejection::InvalidInput, "route input bounds invalid"));
        }
        if [
            &defaults.console,
            &defaults.multimedia,
            &defaults.communications,
        ]
        .contains(&&route.endpoint_id)
        {
            return Err(error(
                Rejection::DefaultEndpoint,
                "dedicated route cannot use a default-role endpoint",
            ));
        }
        let found = inventory
            .endpoints
            .iter()
            .filter(|e| e.id == route.endpoint_id)
            .collect::<Vec<_>>();
        if found.is_empty() {
            return Err(error(
                Rejection::MissingEndpoint,
                "explicit endpoint absent; no default substitution",
            ));
        }
        if found.len() != 1 {
            return Err(error(
                Rejection::AmbiguousEndpoint,
                "duplicate native endpoint ID",
            ));
        }
        let endpoint = found[0];
        if known(&endpoint.state)? != &EndpointState::Active {
            return Err(error(Rejection::EndpointInactive, "endpoint is not active"));
        }
        if known(&endpoint.name)?.trim().is_empty() {
            return Err(error(
                Rejection::InventoryError,
                "endpoint display identity unavailable",
            ));
        }
        let mix = known(&endpoint.mix_format)?;
        if mix.rate == 0 || mix.channels == 0 || mix.bits == 0 || mix.sample_type.is_empty() {
            return Err(error(
                Rejection::InventoryError,
                "endpoint format incomplete",
            ));
        }
        let mut identities = std::collections::BTreeMap::new();
        for app in std::iter::once(&route.application).chain(&route.allowed_children) {
            current_identity(app, inventory)?;
            if !processes.insert(app.pid) || identities.insert(app.pid, app).is_some() {
                return Err(error(
                    Rejection::InvalidInput,
                    "process assigned more than once",
                ));
            }
        }
        for child in &route.allowed_children {
            let (mut current, mut seen) = (child, BTreeSet::new());
            loop {
                if !seen.insert(current.pid) {
                    return Err(error(Rejection::InvalidChild, "child ancestry cycle"));
                }
                let parent = *identities.get(&current.parent_pid).ok_or_else(|| {
                    error(
                        Rejection::InvalidChild,
                        "child ancestor not explicitly allowed",
                    )
                })?;
                if current.creation_filetime.parse::<u64>().unwrap()
                    < parent.creation_filetime.parse::<u64>().unwrap()
                {
                    return Err(error(
                        Rejection::InvalidChild,
                        "child predates current parent; ancestor PID was reused",
                    ));
                }
                if parent.pid == route.application.pid {
                    break;
                }
                current = parent;
            }
        }
        let (mut active, mut instances) = (Vec::new(), BTreeSet::new());
        let sessions = known(&endpoint.sessions)?;
        if sessions.len() > 4096 {
            return Err(error(
                Rejection::InvalidInput,
                "session inventory exceeds bound",
            ));
        }
        for session in sessions {
            if known(&session.state)? != &SessionState::Active {
                continue;
            }
            if active.len() >= 64 {
                return Err(error(
                    Rejection::InvalidInput,
                    "too many active source sessions",
                ));
            }
            let instance = known(&session.instance_id)?;
            if instance.is_empty()
                || instance.len() > 8192
                || instance.contains('\0')
                || !instances.insert(instance)
            {
                return Err(error(
                    Rejection::AmbiguousSession,
                    "active session instance unavailable/duplicate",
                ));
            }
            let owner = match known(&session.owner)? {
                SessionOwner::SingleProcess(a) if a.pid != 0 => a,
                _ => {
                    return Err(error(
                        Rejection::AmbiguousSession,
                        "active system/cross-process session cannot be attributed",
                    ))
                }
            };
            if identities.get(&owner.pid).is_none_or(|a| *a != owner) {
                return Err(error(
                    Rejection::UnknownSession,
                    "active session is not an explicitly allowed current renderer",
                ));
            }
            active.push(instance.clone());
        }
        admitted.push(ValidatedRoute {
            source_id: source,
            endpoint_id: route.endpoint_id.clone(),
            readiness: if active.is_empty() {
                RouteReadiness::WaitingForTarget
            } else {
                RouteReadiness::Armed
            },
            observed_active_session_ids: active,
        });
    }
    Ok(ValidatedRoutePlan {
        request: plan.clone(),
        validated_at_ms: now_ms,
        routes: admitted,
    })
}

fn now_ms() -> Result<u64, RouteError> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| error(Rejection::InventoryError, e.to_string()))?
            .as_millis(),
    )
    .map_err(|_| error(Rejection::InventoryError, "clock overflow"))
}
fn snapshot_parent(
    pid: u32,
    creation_filetime: u64,
    snapshot_cutoff_filetime: u64,
    parents: &std::collections::BTreeMap<u32, u32>,
) -> Result<u32, String> {
    if creation_filetime == 0
        || snapshot_cutoff_filetime == 0
        || creation_filetime > snapshot_cutoff_filetime
    {
        return Err(
            "process generation postdates native ancestry snapshot; refresh inventory".into(),
        );
    }
    parents
        .get(&pid)
        .copied()
        .ok_or_else(|| "process absent from native process snapshot".into())
}
/// Always obtains a new native snapshot on a dedicated MTA; never trusts a UI
/// validation token or a previously populated dropdown.
pub fn admit_route_plan(plan: &CaptureRoutePlan) -> Result<ValidatedRoutePlan, RouteError> {
    if plan.policy_version != ROUTE_POLICY_VERSION {
        return Err(error(Rejection::PolicyVersion, "unsupported route policy"));
    }
    if plan.routes.is_empty()
        || plan.routes.len() > 2
        || plan.routes.iter().any(|r| r.allowed_children.len() > 32)
    {
        return Err(error(
            Rejection::RouteCount,
            "route/child count exceeds admission bounds",
        ));
    }
    let pids = plan
        .routes
        .iter()
        .flat_map(|r| {
            std::iter::once(r.application.pid).chain(r.allowed_children.iter().map(|a| a.pid))
        })
        .collect::<Vec<_>>();
    let snapshot = read_native_inventory(&pids)?;
    validate_route_plan(plan, &snapshot, now_ms()?)
}
fn session_owner_status(
    pid: u32,
    hresult: i32,
    lookup: impl FnOnce() -> Result<ApplicationIdentity, String>,
) -> Observation<SessionOwner> {
    if hresult < 0 {
        Observation::Unavailable(format!("GetProcessId HRESULT {hresult:#x}"))
    } else if hresult != 0 || pid == 0 {
        Observation::Known(SessionOwner::Ambiguous {
            pid,
            hresult,
            reason: "system or non-S_OK success status; not a single renderer".into(),
        })
    } else {
        match lookup() {
            Ok(v) => Observation::Known(SessionOwner::SingleProcess(v)),
            Err(e) => Observation::Unavailable(e),
        }
    }
}
pub fn revalidate_route_plan(
    previous: &ValidatedRoutePlan,
) -> Result<ValidatedRoutePlan, RouteError> {
    admit_route_plan(&previous.request)
}
pub fn read_native_inventory(pids: &[u32]) -> Result<NativeInventory, RouteError> {
    if pids.len() > 128 {
        return Err(error(
            Rejection::InvalidInput,
            "too many process identities",
        ));
    }
    #[cfg(windows)]
    {
        let pids = pids.to_vec();
        std::thread::Builder::new()
            .name("fung-routing-inventory".into())
            .spawn(move || native::read(&pids))
            .map_err(|e| error(Rejection::InventoryError, e.to_string()))?
            .join()
            .map_err(|_| error(Rejection::InventoryError, "inventory worker panicked"))?
            .map_err(|e| error(Rejection::InventoryError, e))
    }
    #[cfg(not(windows))]
    {
        let _ = pids;
        Err(error(
            Rejection::InventoryError,
            "native endpoint routing requires Windows",
        ))
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        core::{Interface, HSTRING, PWSTR},
        Win32::{
            Foundation::{CloseHandle, ERROR_NO_MORE_FILES, FILETIME, HANDLE},
            Media::Audio::*,
            Storage::FileSystem::*,
            System::{
                Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL},
                Diagnostics::ToolHelp::*,
                Threading::{
                    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                },
            },
        },
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            wasapi::deinitialize();
        }
    }
    fn observe<T, E: std::fmt::Display>(r: Result<T, E>) -> Observation<T> {
        match r {
            Ok(v) => Observation::Known(v),
            Err(e) => Observation::Unavailable(e.to_string()),
        }
    }
    fn wide(p: PWSTR) -> Result<String, String> {
        let result = unsafe { p.to_string() }.map_err(|e| e.to_string());
        unsafe { CoTaskMemFree(Some(p.0.cast())) };
        result
    }
    struct ParentSnapshot {
        parents: std::collections::BTreeMap<u32, u32>,
        cutoff_filetime: u64,
    }
    fn parents() -> Result<ParentSnapshot, String> {
        // Bind later process-handle identities to the same ancestry generation.
        // Taking the cutoff BEFORE snapshot creation conservatively rejects even
        // legitimate births during enumeration; the caller can refresh safely.
        let cutoff =
            unsafe { windows::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime() };
        let cutoff_filetime =
            (u64::from(cutoff.dwHighDateTime) << 32) | u64::from(cutoff.dwLowDateTime);
        if cutoff_filetime == 0 {
            return Err("native process snapshot clock unavailable".into());
        }
        let snapshot = Handle(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .map_err(|e| e.to_string())?,
        );
        let mut row = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        unsafe { Process32FirstW(snapshot.0, &mut row) }.map_err(|e| e.to_string())?;
        let mut map = std::collections::BTreeMap::new();
        loop {
            map.insert(row.th32ProcessID, row.th32ParentProcessID);
            match unsafe { Process32NextW(snapshot.0, &mut row) } {
                Ok(()) => (),
                Err(e) if e.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(ParentSnapshot {
            parents: map,
            cutoff_filetime,
        })
    }
    fn identity(pid: u32, parent_map: &ParentSnapshot) -> Result<ApplicationIdentity, String> {
        if pid == 0 {
            return Err("PID 0 cannot identify an application renderer".into());
        }
        if !parent_map.parents.contains_key(&pid) {
            return Err("process absent from native process snapshot".into());
        }
        let handle = Handle(
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
                .map_err(|e| e.to_string())?,
        );
        let (mut created, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe { GetProcessTimes(handle.0, &mut created, &mut exit, &mut kernel, &mut user) }
            .map_err(|e| e.to_string())?;
        if exit.dwLowDateTime != 0 || exit.dwHighDateTime != 0 {
            return Err("process already exited".into());
        }
        let creation = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        let parent_pid = snapshot_parent(
            pid,
            creation,
            parent_map.cutoff_filetime,
            &parent_map.parents,
        )?;
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle.0,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|e| e.to_string())?;
        let image_path =
            String::from_utf16(&buffer[..length as usize]).map_err(|e| e.to_string())?;
        let path = HSTRING::from(&image_path);
        let file = Handle(
            unsafe {
                CreateFileW(
                    &path,
                    FILE_READ_ATTRIBUTES.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .map_err(|e| e.to_string())?,
        );
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe { GetFileInformationByHandle(file.0, &mut info) }.map_err(|e| e.to_string())?;
        let file_id = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
        let mut created_again = FILETIME::default();
        unsafe {
            GetProcessTimes(
                handle.0,
                &mut created_again,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        }
        .map_err(|e| e.to_string())?;
        if created_again != created || exit.dwLowDateTime != 0 || exit.dwHighDateTime != 0 {
            return Err("process changed/exited during identity query".into());
        }
        Ok(ApplicationIdentity {
            pid,
            parent_pid,
            creation_filetime: creation.to_string(),
            image_path,
            image_volume_serial: info.dwVolumeSerialNumber,
            image_file_id: format!("{file_id:016x}"),
        })
    }
    fn session_rows(
        device: &IMMDevice,
        parent_map: &ParentSnapshot,
    ) -> Result<Vec<SessionDescriptor>, String> {
        let manager: IAudioSessionManager2 =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| e.to_string())?;
        let sessions = unsafe { manager.GetSessionEnumerator() }.map_err(|e| e.to_string())?;
        let count = unsafe { sessions.GetCount() }.map_err(|e| e.to_string())?;
        if !(0..=4096).contains(&count) {
            return Err("session inventory exceeds bound".into());
        }
        let mut rows = Vec::new();
        for index in 0..count {
            let session = unsafe { sessions.GetSession(index) }
                .map_err(|e| format!("session {index}: {e}"))?;
            let state = observe(
                unsafe { session.GetState() }
                    .map_err(|e| e.to_string())
                    .and_then(|s| {
                        if s == AudioSessionStateActive {
                            Ok(SessionState::Active)
                        } else if s == AudioSessionStateInactive {
                            Ok(SessionState::Inactive)
                        } else if s == AudioSessionStateExpired {
                            Ok(SessionState::Expired)
                        } else {
                            Err("unknown session state".into())
                        }
                    }),
            );
            let session: IAudioSessionControl2 = session.cast().map_err(|e| e.to_string())?;
            let instance_id = observe(
                unsafe { session.GetSessionInstanceIdentifier() }
                    .map_err(|e| e.to_string())
                    .and_then(wide),
            );
            let mut pid = 0;
            let hr = unsafe { (session.vtable().GetProcessId)(session.as_raw(), &mut pid) };
            let owner = session_owner_status(pid, hr.0, || identity(pid, parent_map));
            rows.push(SessionDescriptor {
                instance_id,
                state,
                owner,
            });
        }
        Ok(rows)
    }
    pub(super) fn read(pids: &[u32]) -> Result<NativeInventory, String> {
        wasapi::initialize_mta().ok().map_err(|e| e.to_string())?;
        let _apartment = Apartment;
        // Start timestamp bounds the oldest observation, not just completion time.
        let captured_at_ms = now_ms().map_err(|e| e.message)?;
        let parent_map = parents()?;
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|e| e.to_string())?;
        let role = |r| -> Result<String, String> {
            let d = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, r) }
                .map_err(|e| e.to_string())?;
            wide(unsafe { d.GetId() }.map_err(|e| e.to_string())?)
        };
        let defaults = observe((|| {
            Ok::<_, String>(DefaultRoles {
                console: role(eConsole)?,
                multimedia: role(eMultimedia)?,
                communications: role(eCommunications)?,
            })
        })());
        let collection =
            unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE(DEVICE_STATEMASK_ALL)) }
                .map_err(|e| e.to_string())?;
        let count = unsafe { collection.GetCount() }.map_err(|e| e.to_string())?;
        if count > 4096 {
            return Err("endpoint inventory exceeds bound".into());
        }
        let mut endpoints = Vec::new();
        let mut errors = Vec::new();
        for index in 0..count {
            let device = match unsafe { collection.Item(index) } {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("endpoint {index}: {e}"));
                    continue;
                }
            };
            let id = match unsafe { device.GetId() }
                .map_err(|e| e.to_string())
                .and_then(wide)
            {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("endpoint ID {index}: {e}"));
                    continue;
                }
            };
            let state = observe(unsafe { device.GetState() }.map(|s| match s {
                DEVICE_STATE_ACTIVE => EndpointState::Active,
                DEVICE_STATE_DISABLED => EndpointState::Disabled,
                DEVICE_STATE_NOTPRESENT => EndpointState::NotPresent,
                DEVICE_STATE_UNPLUGGED => EndpointState::Unplugged,
                _ => EndpointState::Unknown(s.0),
            }));
            let wrapper = wasapi::Device::from_immdevice(device.clone());
            let name = match &wrapper {
                Ok(w) => observe(w.get_friendlyname()),
                Err(e) => Observation::Unavailable(e.to_string()),
            };
            let mix_format = if state == Observation::Known(EndpointState::Active) {
                match &wrapper {
                    Ok(w) => observe(
                        w.get_iaudioclient()
                            .and_then(|c| c.get_mixformat())
                            .and_then(|f| {
                                Ok(MixFormat {
                                    rate: f.get_samplespersec(),
                                    channels: f.get_nchannels(),
                                    bits: f.get_bitspersample(),
                                    sample_type: format!("{:?}", f.get_subformat()?),
                                })
                            }),
                    ),
                    Err(e) => Observation::Unavailable(e.to_string()),
                }
            } else {
                Observation::Unavailable("inactive endpoint; audio client not activated".into())
            };
            let sessions = if state == Observation::Known(EndpointState::Active) {
                observe(session_rows(&device, &parent_map))
            } else {
                Observation::Unavailable("inactive endpoint; sessions not queried".into())
            };
            endpoints.push(EndpointDescriptor {
                id,
                name,
                state,
                mix_format,
                sessions,
            });
        }
        let processes = pids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|pid| ProcessObservation {
                pid,
                identity: observe(identity(pid, &parent_map)),
            })
            .collect();
        Ok(NativeInventory {
            captured_at_ms,
            process_snapshot_cutoff_filetime: parent_map.cutoff_filetime.to_string(),
            endpoints,
            defaults,
            processes,
            errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_provenance_is_bounded_and_cannot_expand_owner_identity() {
        let (p, mut i) = fixture();
        i.endpoints[0].sessions = Observation::Known(
            (0..64)
                .map(|n| session(10, &format!("stream-{n}")))
                .collect(),
        );
        assert_eq!(
            validate_route_plan(&p, &i, 101).unwrap().routes()[0]
                .observed_active_session_ids()
                .len(),
            64
        );
        if let Observation::Known(s) = &mut i.endpoints[0].sessions {
            s.push(session(10, "stream-65"));
        }
        reject(&p, &i, Rejection::InvalidInput);
        i.endpoints[0].sessions = Observation::Known(vec![session(10, "")]);
        reject(&p, &i, Rejection::AmbiguousSession);
        let mut reused = app(10, 1);
        reused.creation_filetime = "100000000000000999".into();
        i.endpoints[0].sessions = Observation::Known(vec![SessionDescriptor {
            owner: Observation::Known(SessionOwner::SingleProcess(reused)),
            ..session(10, "new-process-instance")
        }]);
        reject(&p, &i, Rejection::UnknownSession);
    }
    #[test]
    fn stale_parent_row_must_not_attach_to_a_post_snapshot_process_generation() {
        let parents = std::collections::BTreeMap::from([(11, 10)]);
        assert_eq!(snapshot_parent(11, 90, 100, &parents).unwrap(), 10);
        assert_eq!(snapshot_parent(11, 100, 100, &parents).unwrap(), 10);
        assert!(
            snapshot_parent(11, 101, 100, &parents).is_err(),
            "old child row must not be attached to newly opened replacement generation"
        );
        assert!(snapshot_parent(11, 0, 100, &parents).is_err());
    }
    #[test]
    fn raw_hresult_success_ambiguity_does_not_lookup_or_authorize_pid() {
        for (pid, hr) in [(0, 0), (10, 1), (10, 0x088900d)] {
            let owner =
                session_owner_status(pid, hr, || panic!("ambiguous owner must not be guessed"));
            assert!(matches!(
                owner,
                Observation::Known(SessionOwner::Ambiguous { .. })
            ));
        }
        assert!(matches!(
            session_owner_status(10, -1, || panic!("failed HRESULT must not lookup")),
            Observation::Unavailable(_)
        ));
        assert!(matches!(
            session_owner_status(10, 0, || Ok(app(10, 1))),
            Observation::Known(SessionOwner::SingleProcess(_))
        ));
    }
    #[test]
    fn reused_ancestor_pid_cannot_authorize_an_older_child() {
        let (mut p, mut i) = fixture();
        let mut child = app(11, 10);
        child.creation_filetime = "99999999999999999".into();
        p.routes[0].allowed_children.push(child.clone());
        i.processes.push(ProcessObservation {
            pid: 11,
            identity: Observation::Known(child.clone()),
        });
        i.endpoints[0].sessions = Observation::Known(vec![SessionDescriptor {
            owner: Observation::Known(SessionOwner::SingleProcess(child)),
            ..session(11, "session-A")
        }]);
        reject(&p, &i, Rejection::InvalidChild);
    }
    #[test]
    fn inactive_system_session_is_not_an_active_capture_authorization() {
        let (p, mut i) = fixture();
        if let Observation::Known(sessions) = &mut i.endpoints[0].sessions {
            sessions.push(SessionDescriptor {
                instance_id: Observation::Known("system".into()),
                state: Observation::Known(SessionState::Inactive),
                owner: Observation::Known(SessionOwner::Ambiguous {
                    pid: 0,
                    hresult: 0,
                    reason: "system".into(),
                }),
            })
        }
        assert!(validate_route_plan(&p, &i, 101).is_ok());
    }
    #[test]
    fn every_default_role_and_selected_field_error_fails() {
        let (p, mut i) = fixture();
        for role in 0..3 {
            let mut d = DefaultRoles {
                console: "C".into(),
                multimedia: "M".into(),
                communications: "V".into(),
            };
            match role {
                0 => d.console = "opaque-A".into(),
                1 => d.multimedia = "opaque-A".into(),
                _ => d.communications = "opaque-A".into(),
            };
            i.defaults = Observation::Known(d);
            reject(&p, &i, Rejection::DefaultEndpoint);
        }
        i = fixture().1;
        i.defaults = Observation::Unavailable("no default role".into());
        reject(&p, &i, Rejection::InventoryError);
        i = fixture().1;
        i.endpoints[0].mix_format = Observation::Unavailable("unsupported".into());
        reject(&p, &i, Rejection::InventoryError);
        i = fixture().1;
        i.processes[0].identity = Observation::Unavailable("process access denied".into());
        reject(&p, &i, Rejection::InventoryError);
    }
    fn app(pid: u32, parent: u32) -> ApplicationIdentity {
        ApplicationIdentity {
            pid,
            parent_pid: parent,
            creation_filetime: (100_000_000_000_000_000u64 + pid as u64).to_string(),
            image_path: format!("C:\\Apps\\app{pid}.exe"),
            image_volume_serial: 8,
            image_file_id: format!("{pid:016x}"),
        }
    }
    fn session(pid: u32, id: &str) -> SessionDescriptor {
        SessionDescriptor {
            instance_id: Observation::Known(id.into()),
            state: Observation::Known(SessionState::Active),
            owner: Observation::Known(SessionOwner::SingleProcess(app(pid, 1))),
        }
    }
    fn fixture() -> (CaptureRoutePlan, NativeInventory) {
        let identity = app(10, 1);
        let route = CaptureRoute {
            source_id: "ec548fff-976e-40b5-8955-f79633475690".into(),
            label: "ประชุม".into(),
            endpoint_id: "opaque-A".into(),
            application: identity.clone(),
            allowed_children: vec![],
        };
        let endpoint = EndpointDescriptor {
            id: "opaque-A".into(),
            name: Observation::Known("Speaker".into()),
            state: Observation::Known(EndpointState::Active),
            mix_format: Observation::Known(MixFormat {
                rate: 48000,
                channels: 2,
                bits: 32,
                sample_type: "Float".into(),
            }),
            sessions: Observation::Known(vec![session(10, "session-A")]),
        };
        (
            CaptureRoutePlan {
                policy_version: 1,
                routes: vec![route],
            },
            NativeInventory {
                captured_at_ms: 100,
                process_snapshot_cutoff_filetime: "100000000000001000".into(),
                endpoints: vec![endpoint],
                defaults: Observation::Known(DefaultRoles {
                    console: "default-C".into(),
                    multimedia: "default-M".into(),
                    communications: "default-V".into(),
                }),
                processes: vec![ProcessObservation {
                    pid: 10,
                    identity: Observation::Known(identity),
                }],
                errors: vec![],
            },
        )
    }
    fn reject(p: &CaptureRoutePlan, i: &NativeInventory, code: Rejection) {
        assert_eq!(validate_route_plan(p, i, 101).unwrap_err().code, code)
    }
    #[test]
    fn valid_route_is_distinct_and_waiting_is_not_armed() {
        let (p, mut i) = fixture();
        let v = validate_route_plan(&p, &i, 101).unwrap();
        assert_eq!(v.routes()[0].readiness(), &RouteReadiness::Armed);
        i.endpoints[0].sessions = Observation::Known(vec![]);
        assert_eq!(
            validate_route_plan(&p, &i, 101).unwrap().routes()[0].readiness(),
            &RouteReadiness::WaitingForTarget
        );
    }
    #[test]
    fn version_and_capacity() {
        let (mut p, i) = fixture();
        p.policy_version = 99;
        reject(&p, &i, Rejection::PolicyVersion);
        p.policy_version = 1;
        p.routes.clear();
        reject(&p, &i, Rejection::RouteCount);
        p.routes = vec![fixture().0.routes[0].clone(); 3];
        reject(&p, &i, Rejection::RouteCount);
    }
    #[test]
    fn source_uuid_and_duplicate_identity() {
        let (mut p, i) = fixture();
        p.routes[0].source_id = "not-uuid".into();
        reject(&p, &i, Rejection::InvalidSourceId);
        p.routes[0].source_id = fixture().0.routes[0].source_id.clone();
        p.routes.push(p.routes[0].clone());
        reject(&p, &i, Rejection::DuplicateSource);
        p.routes[1].source_id = uuid::Uuid::new_v4().to_string();
        reject(&p, &i, Rejection::DuplicateEndpoint);
    }
    #[test]
    fn missing_ambiguous_inactive_and_default_endpoint() {
        let (p, mut i) = fixture();
        i.endpoints.clear();
        reject(&p, &i, Rejection::MissingEndpoint);
        i = fixture().1;
        i.endpoints.push(i.endpoints[0].clone());
        reject(&p, &i, Rejection::AmbiguousEndpoint);
        i = fixture().1;
        i.endpoints[0].state = Observation::Known(EndpointState::Disabled);
        reject(&p, &i, Rejection::EndpointInactive);
        i = fixture().1;
        i.defaults = Observation::Known(DefaultRoles {
            console: "opaque-A".into(),
            multimedia: "d".into(),
            communications: "e".into(),
        });
        reject(&p, &i, Rejection::DefaultEndpoint);
    }
    #[test]
    fn errors_and_stale_inventory_fail_closed() {
        let (p, mut i) = fixture();
        i.errors.push("enumeration failed".into());
        reject(&p, &i, Rejection::InventoryError);
        i = fixture().1;
        i.endpoints[0].sessions = Observation::Unavailable("access denied".into());
        reject(&p, &i, Rejection::InventoryError);
        i = fixture().1;
        assert_eq!(
            validate_route_plan(&p, &i, 5101).unwrap_err().code,
            Rejection::StaleInventory
        );
        assert_eq!(
            validate_route_plan(&p, &i, 99).unwrap_err().code,
            Rejection::StaleInventory
        );
    }
    #[test]
    fn reused_pid_or_replaced_image_is_rejected() {
        let (p, mut i) = fixture();
        if let Observation::Known(a) = &mut i.processes[0].identity {
            a.creation_filetime = "100000000000000999".into()
        }
        reject(&p, &i, Rejection::StaleProcess);
        i = fixture().1;
        if let Observation::Known(a) = &mut i.processes[0].identity {
            a.image_file_id = "00000000000000ff".into()
        }
        reject(&p, &i, Rejection::StaleProcess);
    }
    #[test]
    fn unknown_session_and_crossprocess_pidzero_rejected() {
        let (p, mut i) = fixture();
        i.endpoints[0].sessions = Observation::Known(vec![session(11, "session-A")]);
        reject(&p, &i, Rejection::UnknownSession);
        for pid in [0, 10] {
            i.endpoints[0].sessions = Observation::Known(vec![SessionDescriptor {
                owner: Observation::Known(SessionOwner::Ambiguous {
                    pid,
                    hresult: 0x088900d,
                    reason: "cross-process/system".into(),
                }),
                ..session(10, "session-A")
            }]);
            reject(&p, &i, Rejection::AmbiguousSession);
        }
    }
    #[test]
    fn same_process_stream_restart_retains_source_and_records_new_instance() {
        let (p, mut i) = fixture();
        let original = validate_route_plan(&p, &i, 101).unwrap();
        i.endpoints[0].sessions = Observation::Known(vec![session(10, "new-instance")]);
        let restarted = validate_route_plan(&p, &i, 101)
            .expect("new stream in the same authorized process must remain eligible");
        assert_eq!(
            restarted.routes()[0].source_id(),
            original.routes()[0].source_id()
        );
        assert_eq!(restarted.routes()[0].readiness(), &RouteReadiness::Armed);
        assert_eq!(
            restarted.routes[0].observed_active_session_ids,
            vec!["new-instance"]
        );
        i.endpoints[0].sessions = Observation::Known(vec![]);
        assert_eq!(
            validate_route_plan(&p, &i, 101).unwrap().routes()[0].readiness(),
            &RouteReadiness::WaitingForTarget
        );
        i.endpoints[0].sessions = Observation::Known(vec![session(10, "third-instance")]);
        assert_eq!(
            validate_route_plan(&p, &i, 101).unwrap().routes()[0].readiness(),
            &RouteReadiness::Armed
        );
    }
    #[test]
    fn explicit_children_must_have_current_ancestry() {
        let (mut p, mut i) = fixture();
        let child = app(11, 10);
        p.routes[0].allowed_children.push(child.clone());
        i.processes.push(ProcessObservation {
            pid: 11,
            identity: Observation::Known(child.clone()),
        });
        i.endpoints[0].sessions = Observation::Known(vec![SessionDescriptor {
            owner: Observation::Known(SessionOwner::SingleProcess(child)),
            ..session(11, "session-A")
        }]);
        assert!(validate_route_plan(&p, &i, 101).is_ok());
        p.routes[0].allowed_children[0].parent_pid = 999;
        reject(&p, &i, Rejection::StaleProcess);
    }
    #[test]
    fn two_routes_remain_two_sources() {
        let (mut p, mut i) = fixture();
        let mut route = p.routes[0].clone();
        route.source_id = uuid::Uuid::new_v4().to_string();
        route.endpoint_id = "opaque-B".into();
        route.application = app(20, 1);
        p.routes.push(route);
        let mut endpoint = i.endpoints[0].clone();
        endpoint.id = "opaque-B".into();
        endpoint.sessions = Observation::Known(vec![session(20, "session-B")]);
        i.endpoints.push(endpoint);
        i.processes.push(ProcessObservation {
            pid: 20,
            identity: Observation::Known(app(20, 1)),
        });
        let v = validate_route_plan(&p, &i, 101).unwrap();
        assert_ne!(v.routes()[0].source_id(), v.routes()[1].source_id());
        assert_ne!(v.routes()[0].endpoint_id(), v.routes()[1].endpoint_id());
    }
}
