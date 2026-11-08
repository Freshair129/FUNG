//! Native-only factory and revocable runtime evidence. No serde capability.
//! The private GUID is capture-only by trusted construction, not an OS sandbox:
//! metadata cannot distinguish a malicious same-process renderer joining it.
#[path = "native_capture_lifecycle.rs"]
mod lifecycle;
use crate::{
    capture_routing as routing, capture_source_custody as custody,
    endpoint_packet_sink::SourcePacketScope, live_system_audio::EndpointCaptureFlow,
    native_capture_session_audit::NativeProof,
};
use genesis_block_native::Storage;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static RESERVED: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
struct Reservation(Vec<String>);
impl Reservation {
    fn acquire(keys: Vec<String>) -> Result<Self, String> {
        let mut active = RESERVED
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| "native reservations poisoned")?;
        let unique = keys.iter().collect::<BTreeSet<_>>();
        if unique.len() != keys.len() || keys.iter().any(|key| active.contains(key)) {
            return Err("duplicate native source/endpoint activation".into());
        }
        active.extend(keys.iter().cloned());
        Ok(Self(keys))
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut active = RESERVED
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for key in &self.0 {
            active.remove(key);
        }
    }
}
struct Context {
    storage: Arc<Storage>,
    project: String,
    recording: String,
    plan: routing::CaptureRoutePlan,
    _reservation: Reservation,
}
#[derive(Clone)]
struct PlannedSource {
    source_id: String,
    endpoint_id: String,
    kind: &'static str,
    flow: EndpointCaptureFlow,
}

// Use the already-polled evidence and keep identifiers and free-form COM errors
// out of the terminal failure detail.
fn runtime_guard_failure(
    inventory: &routing::NativeInventory,
    endpoint_id: &str,
    proof: &NativeProof,
) -> (usize, String) {
    let mut endpoint_found = false;
    let mut all_sessions_known = true;
    let mut known_session_count = 0usize;
    let mut session_enumeration_error = false;
    let mut active = 0usize;
    let mut inactive = 0usize;
    let mut expired = 0usize;
    let mut unknown_state = 0usize;

    for endpoint in inventory
        .endpoints
        .iter()
        .filter(|endpoint| endpoint.id == endpoint_id)
    {
        endpoint_found = true;
        match &endpoint.sessions {
            routing::Observation::Known(sessions) => {
                known_session_count = known_session_count.saturating_add(sessions.len());
                for session in sessions {
                    let (
                        routing::Observation::Known(instance),
                        routing::Observation::Known(routing::SessionOwner::SingleProcess(owner)),
                    ) = (&session.instance_id, &session.owner)
                    else {
                        continue;
                    };
                    if !proof.matches(instance, owner) {
                        continue;
                    }
                    match &session.state {
                        routing::Observation::Known(routing::SessionState::Active) => active += 1,
                        routing::Observation::Known(routing::SessionState::Inactive) => {
                            inactive += 1
                        }
                        routing::Observation::Known(routing::SessionState::Expired) => expired += 1,
                        routing::Observation::Unavailable(_) => unknown_state += 1,
                    }
                }
            }
            routing::Observation::Unavailable(reason) => {
                all_sessions_known = false;
                if reason != "inactive endpoint; sessions not queried" {
                    session_enumeration_error = true;
                }
            }
        }
    }

    let exact_active = active;
    let sessions = if endpoint_found && all_sessions_known {
        format!("known:{known_session_count}")
    } else {
        "unavailable".to_owned()
    };
    let session_error = if session_enumeration_error {
        "; error=enum"
    } else {
        ""
    };
    let message = format!(
        "live native capture instance absent/inactive/ambiguous (endpoint={}; sessions={sessions}{session_error}; proofMatches=active:{active},inactive:{inactive},expired:{expired},unknownState:{unknown_state}; exactActive={exact_active})",
        if endpoint_found { "found" } else { "missing" },
    );
    (exact_active, message)
}

fn planned_sources(plan: &routing::CaptureRoutePlan) -> Vec<PlannedSource> {
    let mut sources = plan
        .routes
        .iter()
        .map(|route| PlannedSource {
            source_id: route.source_id.clone(),
            endpoint_id: route.endpoint_id.clone(),
            kind: "endpoint",
            flow: EndpointCaptureFlow::RenderLoopback,
        })
        .collect::<Vec<_>>();
    if let Some(microphone) = &plan.microphone {
        sources.push(PlannedSource {
            source_id: microphone.source_id.clone(),
            endpoint_id: microphone.endpoint_id.clone(),
            kind: "microphone",
            flow: EndpointCaptureFlow::Capture,
        });
    }
    sources
}
fn load_source_registration(
    storage: &Storage,
    project: &str,
    recording: &str,
    source: &str,
    epoch: i64,
    expected_kind: &str,
) -> Result<custody::CaptureSourceRegistration, String> {
    match expected_kind {
        "endpoint" => {
            custody::load_active_endpoint_registration(storage, project, recording, source, epoch)
        }
        "microphone" => {
            custody::load_active_microphone_registration(storage, project, recording, source, epoch)
        }
        _ => Err("unsupported native capture source kind".into()),
    }
}
#[derive(Clone)]
pub(crate) struct RuntimeEvidence {
    scope: SourcePacketScope,
    proof: NativeProof,
}
impl RuntimeEvidence {
    pub(crate) fn matches(
        &self,
        route: &routing::CaptureRoute,
        instance: &str,
        owner: &routing::ApplicationIdentity,
    ) -> bool {
        self.scope.source_id().to_string() == route.source_id
            && self.scope.endpoint_id() == route.endpoint_id
            && self.proof.matches(instance, owner)
    }
}
pub(crate) struct PreparedCapture {
    context: Arc<Context>,
    scope: SourcePacketScope,
    kind: &'static str,
    flow: EndpointCaptureFlow,
    lease: lifecycle::Lease<RuntimeEvidence>,
}
pub(crate) struct NativeGroupObserver {
    context: Weak<Context>,
    observer: lifecycle::Observer<RuntimeEvidence>,
}
fn clock_ms() -> Result<u64, String> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
    )
    .map_err(|e| e.to_string())
}
fn route_error(message: impl Into<String>) -> routing::RouteError {
    routing::RouteError {
        code: routing::Rejection::InventoryError,
        message: message.into(),
    }
}
fn binding(
    context: &Context,
    scope: &SourcePacketScope,
    expected_kind: &str,
) -> Result<(), String> {
    let row = load_source_registration(
        &context.storage,
        &context.project,
        &context.recording,
        &scope.source_id().to_string(),
        scope.epoch(),
        expected_kind,
    )?;
    if row.endpoint_id.as_deref() != Some(scope.endpoint_id())
        || row.sample_rate != i64::from(scope.sample_rate())
        || row.clock_origin_qpc_hns != scope.common_origin_hns().to_string()
    {
        return Err("native source registration binding changed".into());
    }
    Ok(())
}
/// Sole production factory: strict fresh whole-group policy and all persisted
/// bindings are checked before returning any movable, single-use worker ticket.
pub(crate) fn prepare_group(
    storage: Arc<Storage>,
    project: String,
    recording: String,
    plan: routing::CaptureRoutePlan,
    epochs: &[(String, i64)],
) -> Result<(Vec<PreparedCapture>, NativeGroupObserver), String> {
    let sources = planned_sources(&plan);
    if epochs.len() != sources.len()
        || plan.routes.is_empty()
        || plan.routes.len() > 2
        || sources.len() > 3
    {
        return Err("native source epoch count mismatch".into());
    }
    let keys = sources
        .iter()
        .flat_map(|source| {
            [
                format!("source:{}", source.source_id),
                format!("endpoint:{}", source.endpoint_id),
            ]
        })
        .collect();
    let reservation = Reservation::acquire(keys)?;
    routing::admit_owned_route_plan(&plan).map_err(|e| e.message)?;
    let mut scopes = Vec::new();
    for source in &sources {
        let matches = epochs
            .iter()
            .filter(|(id, _)| id == &source.source_id)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("native source epoch missing/ambiguous".into());
        }
        let row = load_source_registration(
            &storage,
            &project,
            &recording,
            &source.source_id,
            matches[0].1,
            source.kind,
        )?;
        if row.endpoint_id.as_deref() != Some(&source.endpoint_id)
            || row.sample_rate != 48_000
            || row.kind != source.kind
        {
            return Err("native registration route mismatch".into());
        }
        let scope = SourcePacketScope::new(
            uuid::Uuid::parse_str(&row.id).map_err(|e| e.to_string())?,
            row.epoch,
            source.endpoint_id.clone(),
            row.sample_rate as u32,
            row.clock_origin_qpc_hns
                .parse()
                .map_err(|_| "invalid native common origin")?,
        )
        .map_err(|e| format!("{e:?}"))?;
        scopes.push(scope);
    }
    if scopes
        .iter()
        .any(|s| s.common_origin_hns() != scopes[0].common_origin_hns())
    {
        return Err("native group common origins differ".into());
    }
    let (leases, observer) = lifecycle::prepare(
        sources
            .iter()
            .map(|source| source.source_id.clone())
            .collect(),
    )?;
    let context = Arc::new(Context {
        storage,
        project,
        recording,
        plan,
        _reservation: reservation,
    });
    let observed = NativeGroupObserver {
        context: Arc::downgrade(&context),
        observer,
    };
    let tickets = sources
        .into_iter()
        .zip(scopes)
        .zip(leases)
        .map(|((source, scope), lease)| PreparedCapture {
            context: context.clone(),
            scope,
            kind: source.kind,
            flow: source.flow,
            lease,
        })
        .collect();
    Ok((tickets, observed))
}
impl PreparedCapture {
    pub(crate) fn source_id(&self) -> String {
        self.scope.source_id().to_string()
    }
    pub(crate) fn scope(&self) -> &SourcePacketScope {
        &self.scope
    }
    pub(crate) fn flow(&self) -> EndpointCaptureFlow {
        self.flow
    }
    pub(crate) fn before_start(&self, scope: &SourcePacketScope) -> Result<(), String> {
        self.ensure_starting()?;
        if scope != &self.scope {
            return Err("native worker scope mismatch".into());
        }
        binding(&self.context, &self.scope, self.kind)?;
        match self.flow {
            EndpointCaptureFlow::RenderLoopback => {
                let route = self
                    .context
                    .plan
                    .routes
                    .iter()
                    .find(|route| route.source_id == self.source_id())
                    .ok_or("native route missing")?;
                routing::admit_owned_route_plan(&routing::CaptureRoutePlan {
                    policy_version: self.context.plan.policy_version,
                    routes: vec![route.clone()],
                    microphone: None,
                })
                .map_err(|error| error.message)?;
            }
            EndpointCaptureFlow::Capture => {
                let microphone = self
                    .context
                    .plan
                    .microphone
                    .as_ref()
                    .filter(|microphone| microphone.source_id == self.source_id())
                    .ok_or("native microphone missing")?;
                routing::admit_owned_microphone(microphone).map_err(|error| error.message)?;
            }
        }
        self.ensure_starting()?;
        Ok(())
    }
    pub(crate) fn ensure_starting(&self) -> Result<(), String> {
        self.lease.ensure_starting()
    }
    pub(crate) fn activate(&self, proof: NativeProof) -> Result<(), String> {
        if !proof.bound_to(&self.scope) {
            return Err("native proof source/epoch/endpoint binding mismatch".into());
        }
        binding(&self.context, &self.scope, self.kind)?;
        self.lease.register(RuntimeEvidence {
            scope: self.scope.clone(),
            proof,
        })
    }
    pub(crate) fn wait_ready(&self, remaining: Duration) -> Result<(), String> {
        self.lease.wait_ready(remaining)
    }
    pub(crate) fn revoke(&self) {
        self.lease.revoke();
    }
}
impl NativeGroupObserver {
    pub(crate) fn ready(&self) -> bool {
        self.observer.ready()
    }
    pub(crate) fn revoke_all(&self) {
        self.observer.revoke_all();
    }
    pub(crate) fn validate_source(
        &self,
        source: &str,
    ) -> Result<routing::ValidatedRoutePlan, routing::RouteError> {
        self.validate_source_inner(source, None)
    }
    #[cfg(test)]
    pub(crate) fn validate_source_with_receipt(
        &self,
        source: &str,
    ) -> (
        Result<routing::ValidatedRoutePlan, routing::RouteError>,
        serde_json::Value,
    ) {
        let mut receipt = serde_json::Value::Null;
        let decision = self.validate_source_inner(source, Some(&mut receipt));
        (decision, receipt)
    }
    fn validate_source_inner(
        &self,
        source: &str,
        mut receipt: Option<&mut serde_json::Value>,
    ) -> Result<routing::ValidatedRoutePlan, routing::RouteError> {
        let snapshot = self.observer.snapshot(source).map_err(route_error)?;
        let context = self
            .context
            .upgrade()
            .ok_or_else(|| route_error("native group expired"))?;
        binding(&context, &snapshot.value.scope, "endpoint").map_err(route_error)?;
        let route = context
            .plan
            .routes
            .iter()
            .find(|r| r.source_id == source)
            .ok_or_else(|| route_error("native route missing"))?;
        let plan = routing::CaptureRoutePlan {
            policy_version: context.plan.policy_version,
            routes: vec![route.clone()],
            microphone: None,
        };
        let pids = std::iter::once(route.application.pid)
            .chain(route.allowed_children.iter().map(|a| a.pid))
            .chain(std::iter::once(std::process::id()))
            .collect::<Vec<_>>();
        let inventory = routing::read_owned_native_inventory(&pids)?;
        if let Some(receipt) = receipt.as_deref_mut() {
            *receipt = serde_json::json!({"kind":"passiveOwnedNativeDecisionSnapshot","authority":false,"sourceId":source,"epoch":snapshot.value.scope.epoch(),"endpointId":snapshot.value.scope.endpoint_id(),"commonOriginHns":snapshot.value.scope.common_origin_hns().to_string(),"capturedAtMs":inventory.captured_at_ms,"processSnapshotCutoffFiletime":inventory.process_snapshot_cutoff_filetime,"defaultRoles":inventory.defaults,"selectedEndpoints":inventory.endpoints.iter().filter(|e|e.id==snapshot.value.scope.endpoint_id()).collect::<Vec<_>>(),"processes":inventory.processes,"inventoryErrorCount":inventory.errors.len(),"generationConfirmedAtDecision":false});
        }
        let own = inventory
            .processes
            .iter()
            .filter(|p| p.pid == std::process::id())
            .collect::<Vec<_>>();
        if own.len() != 1
            || !matches!(&own[0].identity, routing::Observation::Known(identity) if snapshot.value.proof.process_matches(identity))
        {
            return Err(route_error(
                "native capture process generation changed/unknown",
            ));
        }
        let (exact_active, failure) = runtime_guard_failure(
            &inventory,
            snapshot.value.scope.endpoint_id(),
            &snapshot.value.proof,
        );
        if exact_active != 1 {
            return Err(route_error(failure));
        }
        let result = routing::validate_owned_route_plan(
            &plan,
            &inventory,
            clock_ms().map_err(route_error)?,
            &snapshot.value,
        );
        binding(&context, &snapshot.value.scope, "endpoint").map_err(route_error)?;
        if !self.observer.unchanged(&snapshot) {
            return Err(route_error("native owner revoked during decision"));
        }
        if let Some(receipt) = receipt {
            receipt["generationConfirmedAtDecision"] = serde_json::json!(true);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planned_sources_assign_capture_flow_to_microphone_only() {
        let identity = routing::ApplicationIdentity {
            pid: 10,
            parent_pid: 1,
            creation_filetime: "123".into(),
            image_path: "app".into(),
            image_volume_serial: 1,
            image_file_id: "0000000000000001".into(),
        };
        let route = routing::CaptureRoute {
            source_id: uuid::Uuid::new_v4().to_string(),
            label: "route".into(),
            endpoint_id: "render-1".into(),
            application: identity,
            allowed_children: vec![],
        };
        let mut second = route.clone();
        second.source_id = uuid::Uuid::new_v4().to_string();
        second.endpoint_id = "render-2".into();
        let mic = routing::CaptureMicrophone {
            source_id: uuid::Uuid::new_v4().to_string(),
            label: "mic".into(),
            endpoint_id: "capture".into(),
        };
        let plan = routing::CaptureRoutePlan {
            policy_version: 1,
            routes: vec![route, second],
            microphone: Some(mic),
        };
        let sources = planned_sources(&plan);
        assert_eq!(sources.len(), 3);
        assert!(sources[..2]
            .iter()
            .all(|source| source.flow == EndpointCaptureFlow::RenderLoopback
                && source.kind == "endpoint"));
        assert_eq!(sources[2].flow, EndpointCaptureFlow::Capture);
        assert_eq!(sources[2].kind, "microphone");
    }
    #[test]
    fn duplicate_global_reservation_is_rejected_and_drop_releases_exact_keys() {
        let key = format!("test:{}", uuid::Uuid::new_v4());
        let first = Reservation::acquire(vec![key.clone()]).unwrap();
        assert!(Reservation::acquire(vec![key.clone()]).is_err());
        drop(first);
        assert!(Reservation::acquire(vec![key.clone()]).is_ok());
        assert!(Reservation::acquire(vec![key.clone(), key]).is_err());
    }
}
