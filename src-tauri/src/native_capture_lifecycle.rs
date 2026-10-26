//! Private lifecycle bookkeeping. This contains no native proof constructor.
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

struct Slot<T> {
    value: Option<T>,
    revoked: bool,
    generation: u64,
}
struct State<T> {
    slots: BTreeMap<String, Slot<T>>,
    ready: bool,
}
struct Core<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
}
pub(super) struct Lease<T> {
    core: Arc<Core<T>>,
    source: String,
}
pub(super) struct Observer<T> {
    core: Weak<Core<T>>,
}
pub(super) struct Snapshot<T> {
    pub(super) value: T,
    source: String,
    generation: u64,
}
pub(super) fn prepare<T>(sources: Vec<String>) -> Result<(Vec<Lease<T>>, Observer<T>), String> {
    let mut slots = BTreeMap::new();
    for source in &sources {
        if source.is_empty()
            || slots
                .insert(
                    source.clone(),
                    Slot {
                        value: None,
                        revoked: false,
                        generation: 0,
                    },
                )
                .is_some()
        {
            return Err("duplicate or empty native activation".into());
        }
    }
    if slots.is_empty() {
        return Err("empty native activation group".into());
    }
    let core = Arc::new(Core {
        state: Mutex::new(State {
            slots,
            ready: false,
        }),
        changed: Condvar::new(),
    });
    let observer = Observer {
        core: Arc::downgrade(&core),
    };
    Ok((
        sources
            .into_iter()
            .map(|source| Lease {
                core: core.clone(),
                source,
            })
            .collect(),
        observer,
    ))
}
impl<T> Lease<T> {
    pub(super) fn ensure_starting(&self) -> Result<(), String> {
        let state = self
            .core
            .state
            .lock()
            .map_err(|_| "native lifecycle poisoned")?;
        if state.slots.values().any(|slot| slot.revoked) {
            return Err("native group cancelled before Start".into());
        }
        match state.slots.get(&self.source) {
            Some(slot) if slot.value.is_none() && !slot.revoked => Ok(()),
            _ => Err("native activation no longer pending".into()),
        }
    }
    pub(super) fn register(&self, value: T) -> Result<(), String> {
        let mut state = self
            .core
            .state
            .lock()
            .map_err(|_| "native lifecycle poisoned")?;
        if !state.ready && state.slots.values().any(|slot| slot.revoked) {
            return Err("native group startup cancelled".into());
        }
        let slot = state
            .slots
            .get_mut(&self.source)
            .ok_or("native activation missing")?;
        if slot.revoked || slot.value.is_some() {
            return Err("native activation reused/revoked".into());
        }
        slot.value = Some(value);
        state.ready = state
            .slots
            .values()
            .all(|s| s.value.is_some() && !s.revoked);
        self.core.changed.notify_all();
        Ok(())
    }
    pub(super) fn revoke(&self) {
        let mut state = self.core.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = state.slots.get_mut(&self.source) {
            slot.revoked = true;
            slot.value = None;
            slot.generation = slot.generation.saturating_add(1);
        }
        self.core.changed.notify_all();
    }
    pub(super) fn wait_ready(&self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or("native group deadline overflow")?;
        let mut state = self
            .core
            .state
            .lock()
            .map_err(|_| "native lifecycle poisoned")?;
        loop {
            if state.slots.values().any(|s| s.revoked) {
                return Err("native group startup cancelled".into());
            }
            if state.ready {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("native group startup deadline".into());
            }
            state = self
                .core
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| "native lifecycle poisoned")?
                .0;
        }
    }
}
impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        self.revoke();
    }
}
impl<T: Clone> Observer<T> {
    pub(super) fn revoke_all(&self) {
        if let Some(core) = self.core.upgrade() {
            let mut state = core.state.lock().unwrap_or_else(|e| e.into_inner());
            for slot in state.slots.values_mut() {
                slot.revoked = true;
                slot.value = None;
                slot.generation = slot.generation.saturating_add(1);
            }
            core.changed.notify_all();
        }
    }
    pub(super) fn snapshot(&self, source: &str) -> Result<Snapshot<T>, String> {
        let core = self.core.upgrade().ok_or("native owners expired")?;
        let state = core.state.lock().map_err(|_| "native lifecycle poisoned")?;
        if !state.ready {
            return Err("native group not fully registered".into());
        }
        let slot = state.slots.get(source).ok_or("native source mismatch")?;
        if slot.revoked {
            return Err("native source revoked".into());
        }
        Ok(Snapshot {
            value: slot.value.clone().ok_or("native source not live")?,
            source: source.into(),
            generation: slot.generation,
        })
    }
    pub(super) fn unchanged(&self, snapshot: &Snapshot<T>) -> bool {
        let Some(core) = self.core.upgrade() else {
            return false;
        };
        let Ok(state) = core.state.lock() else {
            return false;
        };
        state.ready
            && state.slots.get(&snapshot.source).is_some_and(|slot| {
                !slot.revoked && slot.value.is_some() && slot.generation == snapshot.generation
            })
    }
    pub(super) fn ready(&self) -> bool {
        self.core
            .upgrade()
            .and_then(|core| {
                core.state
                    .lock()
                    .ok()
                    .map(|state| state.ready && state.slots.values().all(|slot| !slot.revoked))
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revoked_or_already_registered_ticket_cannot_begin_native_start() {
        let (leases, observer) = prepare(vec!["a".into(), "b".into()]).unwrap();
        assert!(leases[0].ensure_starting().is_ok());
        leases[0].register(1).unwrap();
        assert!(leases[0].ensure_starting().is_err());
        assert!(leases[1].ensure_starting().is_ok());
        observer.revoke_all();
        assert!(leases[1].ensure_starting().is_err());
        let (leases, _) = prepare::<u8>(vec!["c".into(), "d".into()]).unwrap();
        leases[0].revoke();
        assert!(leases[1].ensure_starting().is_err());
    }
    #[test]
    fn stale_snapshot_cannot_survive_revocation_even_with_live_peer() {
        let (leases, observer) = prepare(vec!["a".into(), "b".into()]).unwrap();
        assert!(observer.snapshot("a").is_err());
        leases[0].register(10).unwrap();
        assert!(observer.snapshot("a").is_err());
        leases[1].register(20).unwrap();
        let before = observer.snapshot("a").unwrap();
        assert_eq!(before.value, 10);
        assert!(observer.unchanged(&before));
        leases[0].revoke();
        assert!(!observer.unchanged(&before));
        assert!(observer.snapshot("a").is_err());
        assert!(leases[0].register(30).is_err());
    }
    #[test]
    fn duplicate_expired_partial_and_cancelled_groups_are_closed() {
        assert!(prepare::<u8>(vec!["a".into(), "a".into()]).is_err());
        let (mut leases, observer) = prepare(vec!["a".into(), "b".into()]).unwrap();
        leases[0].register(1).unwrap();
        assert!(leases[0].register(2).is_err());
        assert!(leases[0].wait_ready(Duration::from_millis(1)).is_err());
        leases.pop();
        assert!(leases[0].wait_ready(Duration::from_secs(1)).is_err());
        drop(leases);
        assert!(observer.snapshot("a").is_err());
    }
    #[test]
    fn stop_request_revokes_every_snapshot_without_extending_owner_lifetime() {
        let (leases, observer) = prepare(vec!["a".into()]).unwrap();
        leases[0].register(7).unwrap();
        assert!(observer.ready());
        let snapshot = observer.snapshot("a").unwrap();
        observer.revoke_all();
        assert!(!observer.unchanged(&snapshot));
        assert!(observer.snapshot("a").is_err());
        drop(leases);
        assert!(observer.snapshot("a").is_err());
    }
}
