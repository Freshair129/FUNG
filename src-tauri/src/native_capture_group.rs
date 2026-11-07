//! Native-only factory and revocable runtime evidence. No serde capability.
//! The private GUID is capture-only by trusted construction, not an OS sandbox:
//! metadata cannot distinguish a malicious same-process renderer joining it.
#[path = "native_capture_lifecycle.rs"]
mod lifecycle;
use crate::{
    capture_routing as routing, capture_source_custody as custody,
    endpoint_packet_sink::SourcePacketScope, native_capture_session_audit::NativeProof,
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
fn binding(context: &Context, scope: &SourcePacketScope) -> Result<(), String> {
    let row = custody::load_active_endpoint_registration(
        &context.storage,
        &context.project,
        &context.recording,
        &scope.source_id().to_string(),
        scope.epoch(),
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
    if epochs.len() != plan.routes.len() || plan.routes.is_empty() || plan.routes.len() > 2 {
        return Err("native source epoch count mismatch".into());
    }
    let keys = plan
        .routes
        .iter()
        .flat_map(|r| {
            [
                format!("source:{}", r.source_id),
                format!("endpoint:{}", r.endpoint_id),
            ]
        })
        .collect();
    let reservation = Reservation::acquire(keys)?;
    routing::admit_owned_route_plan(&plan).map_err(|e| e.message)?;
    let mut scopes = Vec::new();
    for route in &plan.routes {
        let matches = epochs
            .iter()
            .filter(|(id, _)| id == &route.source_id)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("native source epoch missing/ambiguous".into());
        }
        let row = custody::load_active_endpoint_registration(
            &storage,
            &project,
            &recording,
            &route.source_id,
            matches[0].1,
        )?;
        if row.endpoint_id.as_deref() != Some(&route.endpoint_id) || row.sample_rate != 48_000 {
            return Err("native registration route mismatch".into());
        }
        let scope = SourcePacketScope::new(
            uuid::Uuid::parse_str(&row.id).map_err(|e| e.to_string())?,
            row.epoch,
            route.endpoint_id.clone(),
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
    let (leases, observer) =
        lifecycle::prepare(plan.routes.iter().map(|r| r.source_id.clone()).collect())?;
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
    let tickets = scopes
        .into_iter()
        .zip(leases)
        .map(|(scope, lease)| PreparedCapture {
            context: context.clone(),
            scope,
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
    pub(crate) fn before_start(&self, scope: &SourcePacketScope) -> Result<(), String> {
        self.ensure_starting()?;
        if scope != &self.scope {
            return Err("native worker scope mismatch".into());
        }
        binding(&self.context, &self.scope)?;
        let route = self
            .context
            .plan
            .routes
            .iter()
            .find(|r| r.source_id == self.source_id())
            .ok_or("native route missing")?;
        routing::admit_owned_route_plan(&routing::CaptureRoutePlan {
            policy_version: self.context.plan.policy_version,
            routes: vec![route.clone()],
        })
        .map_err(|e| e.message)?;
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
        binding(&self.context, &self.scope)?;
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
        binding(&context, &snapshot.value.scope).map_err(route_error)?;
        let route = context
            .plan
            .routes
            .iter()
            .find(|r| r.source_id == source)
            .ok_or_else(|| route_error("native route missing"))?;
        let plan = routing::CaptureRoutePlan {
            policy_version: context.plan.policy_version,
            routes: vec![route.clone()],
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
        let exact_active = inventory.endpoints.iter().filter(|e| e.id == snapshot.value.scope.endpoint_id()).filter_map(|e| match &e.sessions { routing::Observation::Known(rows) => Some(rows), _ => None }).flatten().filter(|session| matches!((&session.instance_id, &session.owner, &session.state), (routing::Observation::Known(instance), routing::Observation::Known(routing::SessionOwner::SingleProcess(owner)), routing::Observation::Known(routing::SessionState::Active)) if snapshot.value.proof.matches(instance, owner))).count();
        if exact_active != 1 {
            return Err(route_error(
                "live native capture instance absent/inactive/ambiguous",
            ));
        }
        let result = routing::validate_owned_route_plan(
            &plan,
            &inventory,
            clock_ms().map_err(route_error)?,
            &snapshot.value,
        );
        binding(&context, &snapshot.value.scope).map_err(route_error)?;
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
    fn duplicate_global_reservation_is_rejected_and_drop_releases_exact_keys() {
        let key = format!("test:{}", uuid::Uuid::new_v4());
        let first = Reservation::acquire(vec![key.clone()]).unwrap();
        assert!(Reservation::acquire(vec![key.clone()]).is_err());
        drop(first);
        assert!(Reservation::acquire(vec![key.clone()]).is_ok());
        assert!(Reservation::acquire(vec![key.clone(), key]).is_err());
    }
}
