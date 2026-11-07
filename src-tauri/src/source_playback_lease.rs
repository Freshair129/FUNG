//! Private process-local source cancellation. Registration grants no media access.
//!
//! A lease must be registered BEFORE custody/media admission, then its frontier
//! and availability rechecked under its output gate before installation. Every
//! queue publication and output callback use separate gates. Revocation fences
//! both operations; worker exit/handle closure require separate owner ack.

use genesis_block_native::Storage;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard, OnceLock, Weak,
};
use uuid::Uuid;

const MAX_STORES: usize = 64;
const MAX_LEASES_PER_STORE: usize = 64;
const MAX_DENIED_RECORDINGS: usize = 256;

pub(crate) struct SourcePlaybackLease {
    // Keep the exact Storage allocation alive for the whole lease lifetime.
    storage: Arc<Storage>,
    recording: Uuid,
    available: AtomicBool,
    output_gate: Mutex<()>,
    producer_gate: Mutex<()>,
}

struct StoreLeases {
    storage: Weak<Storage>,
    leases: Vec<Weak<SourcePlaybackLease>>,
    denied: Vec<Uuid>,
    deny_all: bool,
}

impl StoreLeases {
    fn new(storage: &Arc<Storage>) -> Self {
        Self {
            storage: Arc::downgrade(storage),
            leases: Vec::with_capacity(MAX_LEASES_PER_STORE),
            denied: Vec::with_capacity(MAX_DENIED_RECORDINGS),
            deny_all: false,
        }
    }

    fn matches(&self, storage: &Arc<Storage>) -> bool {
        std::ptr::eq(self.storage.as_ptr(), Arc::as_ptr(storage))
    }

    fn prune_leases(&mut self) {
        self.leases.retain(|lease| lease.strong_count() != 0);
    }

    fn revoke(&mut self, recording: Option<Uuid>) -> Vec<Arc<SourcePlaybackLease>> {
        self.prune_leases();
        if let Some(recording) = recording {
            if !self.denied.contains(&recording) {
                if self.denied.len() == MAX_DENIED_RECORDINGS {
                    // Never evict a live denial and accidentally resurrect access.
                    self.deny_all = true;
                } else {
                    self.denied.push(recording);
                }
            }
        } else {
            self.deny_all = true;
        }
        let mut revoked = Vec::with_capacity(self.leases.len());
        for lease in self.leases.iter().filter_map(Weak::upgrade) {
            if self.deny_all || Some(lease.recording) == recording {
                lease.available.store(false, Ordering::Release);
                revoked.push(lease);
            }
        }
        revoked
    }
}

#[derive(Default)]
struct Registry {
    stores: Vec<StoreLeases>,
    registration_denied: bool,
}

impl Registry {
    fn prune_stores(&mut self) {
        // Runtime denials survive every retry while this exact Storage is alive.
        self.stores
            .retain(|store| store.storage.strong_count() != 0);
    }

    fn register(
        &mut self,
        storage: Arc<Storage>,
        recording: Uuid,
    ) -> Result<Arc<SourcePlaybackLease>, String> {
        self.prune_stores();
        if self.registration_denied {
            return Err("source playback registry fail-closed".into());
        }
        let index = match self.stores.iter().position(|store| store.matches(&storage)) {
            Some(index) => index,
            None if self.stores.len() < MAX_STORES => {
                self.stores.push(StoreLeases::new(&storage));
                self.stores.len() - 1
            }
            None => return Err("source playback store registry full".into()),
        };
        let store = &mut self.stores[index];
        store.prune_leases();
        if store.deny_all || store.denied.contains(&recording) {
            return Err("source playback recording revoked".into());
        }
        if store.leases.len() == MAX_LEASES_PER_STORE {
            return Err("source playback lease registry full".into());
        }
        let lease = Arc::new(SourcePlaybackLease {
            storage,
            recording,
            available: AtomicBool::new(true),
            output_gate: Mutex::new(()),
            producer_gate: Mutex::new(()),
        });
        store.leases.push(Arc::downgrade(&lease));
        Ok(lease)
    }

    fn revoke(
        &mut self,
        storage: &Arc<Storage>,
        recording: Option<Uuid>,
    ) -> Vec<Arc<SourcePlaybackLease>> {
        self.prune_stores();
        let index = match self.stores.iter().position(|store| store.matches(storage)) {
            Some(index) => index,
            None if self.stores.len() < MAX_STORES => {
                self.stores.push(StoreLeases::new(storage));
                self.stores.len() - 1
            }
            None => {
                // This store cannot have existing leases (registration creates an
                // entry). Close FUTURE admission globally rather than lose its
                // denial. Existing unrelated output is not revoked by overflow.
                self.registration_denied = true;
                return Vec::new();
            }
        };
        self.stores[index].revoke(recording)
    }
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

fn recording_uuid(recording: &str) -> Result<Uuid, String> {
    let id = Uuid::parse_str(recording).map_err(|_| "invalid playback recording UUID")?;
    if id.to_string() != recording {
        return Err("noncanonical playback recording UUID".into());
    }
    Ok(id)
}

impl SourcePlaybackLease {
    /// Fence callbacks and producers even after revocation. Never hold the two
    /// gates simultaneously and never call while holding registry or SDK locks.
    pub(crate) fn quiesce(&self) {
        {
            let _output = self.output_gate.lock().unwrap_or_else(|e| e.into_inner());
        }
        {
            let _producer = self.producer_gate.lock().unwrap_or_else(|e| e.into_inner());
        }
    }

    pub(crate) fn register(storage: Arc<Storage>, recording: &str) -> Result<Arc<Self>, String> {
        let recording = recording_uuid(recording)?;
        registry()
            .lock()
            .map_err(|_| "source playback registry poisoned")?
            .register(storage, recording)
    }

    pub(crate) fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    pub(crate) fn try_output(&self) -> Option<MutexGuard<'_, ()>> {
        if !self.is_available() {
            return None;
        }
        let guard = self.output_gate.try_lock().ok()?;
        if !self.is_available() {
            return None;
        }
        Some(guard)
    }

    /// Non-realtime control path only. Wait for any in-flight output callback,
    /// then recheck revocation while holding its fence.
    pub(crate) fn output_fence(&self) -> Option<MutexGuard<'_, ()>> {
        if !self.is_available() {
            return None;
        }
        let guard = self.output_gate.lock().ok()?;
        if !self.is_available() {
            return None;
        }
        Some(guard)
    }

    /// Worker-only: wait for a short callback/queue critical section, then
    /// recheck revocation. Never use this blocking method on the audio thread.
    pub(crate) fn producer_output(&self) -> Option<MutexGuard<'_, ()>> {
        if !self.is_available() {
            return None;
        }
        let guard = self.producer_gate.lock().ok()?;
        if !self.is_available() {
            return None;
        }
        Some(guard)
    }

    /// Preparation/install check only. Never query Storage in the audio callback.
    /// Hold try_output's guard across this check and the final installation.
    pub(crate) fn check_frontier(&self, frontier: u64) -> Result<(), String> {
        if !self.is_available() {
            return Err("source playback recording revoked".into());
        }
        if self.storage.txn_frontier() != frontier {
            return Err("custody changed during source playback preparation".into());
        }
        if !self.is_available() {
            return Err("source playback recording revoked".into());
        }
        Ok(())
    }
}

/// Call only after the SDK transaction has returned and released its commit locks.
/// Returns after output fencing, not after worker cleanup or hardware drain.
pub(crate) fn revoke_recording(storage: &Arc<Storage>, recording: &str) {
    quiesce_revoked(latch_revoke_recording(storage, recording));
}

/// Latch denial while the caller holds the custody gate. Does not wait for
/// callbacks or producers; the returned leases must be quiesced after the gate
/// and all SDK locks have been released.
pub(crate) fn latch_revoke_recording(
    storage: &Arc<Storage>,
    recording: &str,
) -> Vec<Arc<SourcePlaybackLease>> {
    {
        let mut registry = registry().lock().unwrap_or_else(|poisoned| {
            let mut registry = poisoned.into_inner();
            registry.registration_denied = true;
            registry
        });
        registry.revoke(storage, recording_uuid(recording).ok())
    }
}

pub(crate) fn quiesce_revoked(revoked: Vec<Arc<SourcePlaybackLease>>) {
    for lease in revoked {
        lease.quiesce();
    }
}
