//! Desktop native session broker.
//!
//! Refresh credentials are stored only in the OS keyring. Access tokens,
//! callback values, authorization codes, and verifiers never implement a
//! public DTO and are held in `Zeroizing` native memory for their lifetime.

use crate::{device_identity, native_auth, AppState};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use reqwest::blocking::Client as BlockingClient;
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex, OnceLock,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "FUNG";
const CALLBACK_PATH: &str = "/auth/callback";
const LOGIN_TTL: Duration = Duration::from_secs(120);
const ACCESS_SKEW_MS: u64 = 30_000;
const ACCOUNT_INDEX: &str = "desktop-session-slot-index";
const DRIVE_DOMAINS_INDEX: &str = "drive-credential-domains";

const ACCOUNT_DOMAIN: &str = "desktop-session";
const ACCOUNT_MARKER: &str = "desktop-session-commit-marker";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionLifecycleState {
    SignedOut,
    LoginPending,
    Authenticated,
    Refreshing,
    RefreshFailed,
    LogoutPending,
    CleanupFailed,
    Shutdown,
}
impl SessionLifecycleState {
    fn as_str(self) -> &'static str {
        match self {
            Self::SignedOut => "signed_out",
            Self::LoginPending => "login_pending",
            Self::Authenticated => "authenticated",
            Self::Refreshing => "refreshing",
            Self::RefreshFailed => "refresh_failed",
            Self::LogoutPending => "logout_pending",
            Self::CleanupFailed => "credential_cleanup_failed",
            Self::Shutdown => "shutdown",
        }
    }
}

pub(crate) trait KeyringPort {
    fn read(&mut self, slot: &str) -> Result<Option<Zeroizing<String>>, String>;
    fn write(&mut self, slot: &str, value: &Zeroizing<String>) -> Result<(), String>;
    fn delete(&mut self, slot: &str) -> Result<(), String>;
    fn verify_absent(&mut self, slot: &str) -> Result<(), String>;
    #[cfg(test)]
    fn inject_failure(&mut self, _stage: usize) {}
    #[cfg(test)]
    fn inject_cleanup_failure(&mut self) {}
    #[cfg(test)]
    fn event_count(&self) -> usize {
        0
    }
}

pub(crate) trait ClockPort {
    fn now_ms(&self) -> u64;
}
pub(crate) trait ListenerCallbackPort {
    fn open(&mut self) -> Result<(), String>;
    fn close(&mut self);
    fn callback_target(&self, request: &[u8], port: u16) -> Option<Zeroizing<String>>;
}

pub(crate) struct LifecycleMaterial {
    pub(crate) access: Zeroizing<String>,
    pub(crate) refresh: Zeroizing<String>,
    pub(crate) user_id: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) access_expires_at_ms: Option<u64>,
}
pub(crate) trait ProviderHttpPort:
    DriveHttpPort + ArchiveJobPort + CommitObservationPort + Send + Sync + Clone + 'static
{
    fn exchange(
        &mut self,
        code: Zeroizing<String>,
        verifier: Zeroizing<String>,
    ) -> Result<LifecycleMaterial, String>;
    fn refresh(&mut self, refresh: Zeroizing<String>) -> Result<LifecycleMaterial, String>;
    fn drive_exchange(
        &self,
        client_id: String,
        redirect_uri: Zeroizing<String>,
        code: Zeroizing<String>,
        verifier: Zeroizing<String>,
    ) -> Result<DriveTokenMaterial, String>;
    fn drive_refresh(
        &self,
        client_id: String,
        refresh: Zeroizing<String>,
    ) -> Result<DriveTokenMaterial, String>;
    #[cfg(test)]
    fn inject_failure(&mut self, _code: &'static str) {}
    #[cfg(test)]
    fn call_count(&self) -> usize {
        0
    }
}
pub(crate) trait DriveHttpPort {}
pub(crate) trait ArchiveJobPort {}
pub(crate) trait CommitObservationPort {
    fn observe(&mut self, _event: &'static str) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LifecycleOutcome {
    pub(crate) state: &'static str,
    pub(crate) code: Option<&'static str>,
}

pub(crate) struct DriveTokenMaterial {
    pub(crate) access: Zeroizing<String>,
    pub(crate) refresh: Option<Zeroizing<String>>,
    pub(crate) scope: Option<Zeroizing<String>>,
}

enum RefreshAdmission<P> {
    Ready(Zeroizing<String>),
    Wait(Arc<(Mutex<bool>, Condvar)>),
    Work {
        ticket: LifecycleTicket,
        refresh: Zeroizing<String>,
        provider: P,
    },
}

struct AccountSession {
    generation: u64,
    state: SessionLifecycleState,
    startup_checked: bool,
    user_id: Option<String>,
    email: Option<String>,
    access_token: Option<Zeroizing<String>>,
    access_expires_at_ms: Option<u64>,
    pending_login: Option<PendingLogin>,
    refresh_flight: Option<Arc<(Mutex<bool>, Condvar)>>,
    pending_operations: HashSet<u64>,
    pending_login_operation: Option<u64>,
}

struct DriveCredential {
    drive_generation: u64,
    connected: bool,
    quiescing: bool,
    slot_base: Option<String>,
    pending_operations: HashSet<u64>,
}

#[derive(Default)]
pub(crate) struct OperationDrain {
    active: Mutex<usize>,
    signal: Condvar,
}

impl OperationDrain {
    fn admit(&self) {
        if let Ok(mut active) = self.active.lock() {
            *active += 1;
        }
    }

    pub(crate) fn release(&self) {
        if let Ok(mut active) = self.active.lock() {
            *active = active.saturating_sub(1);
            if *active == 0 {
                self.signal.notify_all();
            }
        }
    }

    fn wait_empty(&self) {
        if let Ok(mut active) = self.active.lock() {
            while *active != 0 {
                active = match self.signal.wait(active) {
                    Ok(value) => value,
                    Err(_) => return,
                };
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LifecycleTicket {
    pub(crate) operation_id: u64,
    pub(crate) account_epoch: u64,
    pub(crate) account_generation: u64,
    pub(crate) drive_generation: u64,
}

pub(crate) struct AccountOperationGuard {
    ticket: LifecycleTicket,
    drain: Arc<OperationDrain>,
}

impl AccountOperationGuard {
    pub(crate) fn check(&self) -> Result<(), String> {
        production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?
            .check_account_operation(self.ticket)
    }
}

impl Drop for AccountOperationGuard {
    fn drop(&mut self) {
        self.drain.release();
        if let Ok(mut lifecycle) = production_lifecycle().lock() {
            lifecycle.finish_account_operation(self.ticket);
        }
    }
}

#[derive(Default)]
struct CommitFence {
    commits: u64,
}

pub(crate) struct SessionLifecycle<K, C, L, P> {
    account: AccountSession,
    drive: DriveCredential,
    account_epoch: u64,
    next_operation_id: u64,
    quiescing: bool,
    commit_fence: CommitFence,
    keyring: K,
    clock: C,
    listener: L,
    provider: P,
    account_drain: Arc<OperationDrain>,
    drive_drain: Arc<OperationDrain>,
}

impl<K, C, L, P> SessionLifecycle<K, C, L, P>
where
    K: KeyringPort,
    C: ClockPort,
    L: ListenerCallbackPort,
    P: ProviderHttpPort + Clone,
{
    pub(crate) fn new(keyring: K, clock: C, listener: L, provider: P) -> Self {
        Self {
            account: AccountSession {
                generation: 1,
                state: SessionLifecycleState::SignedOut,
                startup_checked: false,
                user_id: None,
                email: None,
                access_token: None,
                access_expires_at_ms: None,
                pending_login: None,
                refresh_flight: None,
                pending_operations: HashSet::new(),
                pending_login_operation: None,
            },
            drive: DriveCredential {
                drive_generation: 1,
                connected: false,
                quiescing: false,
                slot_base: None,
                pending_operations: HashSet::new(),
            },
            account_epoch: 1,
            next_operation_id: 1,
            quiescing: false,
            commit_fence: CommitFence::default(),
            keyring,
            clock,
            listener,
            provider,
            account_drain: Arc::new(OperationDrain::default()),
            drive_drain: Arc::new(OperationDrain::default()),
        }
    }

    fn outcome(&self, state: &'static str, code: Option<&'static str>) -> LifecycleOutcome {
        LifecycleOutcome { state, code }
    }
    fn clear_memory(&mut self) {
        self.account.pending_login = None;
        self.account.pending_login_operation = None;
        self.account.access_token = None;
        self.account.access_expires_at_ms = None;
        self.account.user_id = None;
        self.account.email = None;
        self.listener.close();
    }
    fn mark_credential_cleanup_failed(&mut self) {
        self.clear_memory();
        self.account.state = SessionLifecycleState::CleanupFailed;
        self.quiescing = true;
        self.drive.quiescing = true;
        self.drive.connected = false;
    }
    fn next_operation(&mut self) -> u64 {
        let id = self.next_operation_id;
        self.next_operation_id = self.next_operation_id.wrapping_add(1);
        id
    }
    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.account.generation
    }
    #[cfg(test)]
    pub(crate) fn state_name(&self) -> &'static str {
        self.account.state.as_str()
    }
    #[cfg(test)]
    pub(crate) fn disposed(&self) -> bool {
        self.account.pending_login.is_none() && self.account.access_token.is_none()
    }
    pub(crate) fn begin_login(&mut self, pending: PendingLogin) -> Result<u64, String> {
        if self.quiescing
            || matches!(
                self.account.state,
                SessionLifecycleState::Shutdown
                    | SessionLifecycleState::LogoutPending
                    | SessionLifecycleState::CleanupFailed
            )
        {
            return Err(public_error("auth_transition_in_progress"));
        }
        if self.account.pending_login.is_some() {
            return Err(public_error("auth_request_in_progress"));
        }
        self.account.startup_checked = true;
        self.listener.open()?;
        self.account.state = SessionLifecycleState::LoginPending;
        let generation = self.account.generation;
        let operation_id = self.next_operation();
        self.account.pending_login_operation = Some(operation_id);
        let mut pending = pending;
        pending.generation = generation;
        self.account.pending_login = Some(pending);
        Ok(generation)
    }

    pub(crate) fn registered_login_begin(&mut self, pending: PendingLogin) -> Result<u64, String> {
        self.begin_login(pending)
    }
    pub(crate) fn take_login(&mut self, request_id: &str, generation: u64) -> Option<PendingLogin> {
        if self.account.generation == generation
            && self
                .account
                .pending_login
                .as_ref()
                .is_some_and(|pending| pending.request_id == request_id)
        {
            self.account.pending_login.take()
        } else {
            None
        }
    }

    pub(crate) fn take_login_for_exchange(
        &mut self,
        request_id: &str,
        generation: u64,
    ) -> Option<(PendingLogin, LifecycleTicket, P)> {
        if self
            .account
            .pending_login
            .as_ref()
            .is_some_and(|pending| SystemTime::now() >= pending.expires_at)
        {
            return None;
        }
        let pending = self.take_login(request_id, generation)?;
        let operation_id = self.account.pending_login_operation?;
        Some((
            pending,
            LifecycleTicket {
                operation_id,
                account_epoch: self.account_epoch,
                account_generation: self.account.generation,
                drive_generation: self.drive.drive_generation,
            },
            self.provider.clone(),
        ))
    }

    pub(crate) fn registered_login_take_for_exchange(
        &mut self,
        request_id: &str,
        generation: u64,
    ) -> Option<(PendingLogin, LifecycleTicket, P)> {
        self.take_login_for_exchange(request_id, generation)
    }

    fn ensure_account_ticket(&self, ticket: LifecycleTicket) -> Result<(), String> {
        if self.account_epoch != ticket.account_epoch
            || self.account.generation != ticket.account_generation
            || self
                .account
                .pending_operations
                .contains(&ticket.operation_id)
                == false
        {
            return Err(public_error("auth_transition_in_progress"));
        }
        Ok(())
    }

    pub(crate) fn begin_account_operation(&mut self) -> Result<AccountOperationGuard, String> {
        if self.quiescing
            || matches!(
                self.account.state,
                SessionLifecycleState::Shutdown
                    | SessionLifecycleState::LogoutPending
                    | SessionLifecycleState::CleanupFailed
            )
        {
            return Err(public_error("auth_transition_in_progress"));
        }
        let operation_id = self.next_operation();
        self.account.pending_operations.insert(operation_id);
        self.account_drain.admit();
        Ok(AccountOperationGuard {
            ticket: LifecycleTicket {
                operation_id,
                account_epoch: self.account_epoch,
                account_generation: self.account.generation,
                drive_generation: self.drive.drive_generation,
            },
            drain: self.account_drain.clone(),
        })
    }

    pub(crate) fn check_account_operation(&self, ticket: LifecycleTicket) -> Result<(), String> {
        self.ensure_account_ticket(ticket)
    }

    pub(crate) fn finish_account_operation(&mut self, ticket: LifecycleTicket) {
        self.account.pending_operations.remove(&ticket.operation_id);
    }

    pub(crate) fn complete_login(
        &mut self,
        ticket: LifecycleTicket,
        result: Result<LifecycleMaterial, String>,
    ) -> Result<LifecycleOutcome, String> {
        if self.account.pending_login_operation != Some(ticket.operation_id)
            || self.account_epoch != ticket.account_epoch
            || self.account.generation != ticket.account_generation
            || self.quiescing
        {
            self.clear_memory();
            return Err(public_error("auth_transition_in_progress"));
        }
        self.account.pending_login_operation = None;
        let material = match result {
            Ok(material) => material,
            Err(error) => {
                self.clear_memory();
                self.account.state = SessionLifecycleState::SignedOut;
                return Err(error);
            }
        };
        self.accept_material(material)?;
        self.listener.close();
        self.account.state = SessionLifecycleState::Authenticated;
        Ok(self.outcome("authenticated", None))
    }

    pub(crate) fn registered_login_complete(
        &mut self,
        ticket: LifecycleTicket,
        result: Result<LifecycleMaterial, String>,
    ) -> Result<LifecycleOutcome, String> {
        self.complete_login(ticket, result)
    }
    pub(crate) fn login_expired(&self) -> bool {
        self.account
            .pending_login
            .as_ref()
            .is_none_or(|pending| self.clock.now_ms() >= pending.expires_at_ms)
    }
    pub(crate) fn cancel_login(&mut self, request_id: &str) -> Result<(), String> {
        let pending = self
            .account
            .pending_login
            .take()
            .ok_or_else(|| public_error("auth_request_not_found"))?;
        if pending.request_id != request_id {
            self.account.pending_login = Some(pending);
            return Err(public_error("auth_request_not_found"));
        }
        pending.cancelled.store(true, Ordering::Release);
        self.account.generation = self.account.generation.wrapping_add(1);
        self.account.pending_login_operation = None;
        self.account.pending_operations.clear();
        self.account.state = SessionLifecycleState::SignedOut;
        Ok(())
    }
    pub(crate) fn accept_material(
        &mut self,
        material: LifecycleMaterial,
    ) -> Result<LifecycleOutcome, String> {
        if let Err(error) = commit_credential(
            &mut self.keyring,
            ACCOUNT_DOMAIN,
            ACCOUNT_MARKER,
            &material.refresh,
            "keyring_unavailable",
            &mut self.commit_fence,
        ) {
            if error == "cleanup_failed" {
                self.mark_credential_cleanup_failed();
            }
            return Err(error);
        }
        self.provider.observe("account-marker-verified");
        self.account.access_token = Some(material.access);
        if let Some(user_id) = material.user_id {
            self.account.user_id = Some(user_id);
        }
        if material.email.is_some() {
            self.account.email = material.email;
        }
        self.account.access_expires_at_ms = material.access_expires_at_ms;
        self.account.state = SessionLifecycleState::Authenticated;
        Ok(self.outcome("authenticated", None))
    }

    fn begin_refresh(&mut self) -> Result<RefreshAdmission<P>, String> {
        if self.quiescing
            || matches!(
                self.account.state,
                SessionLifecycleState::Shutdown
                    | SessionLifecycleState::LogoutPending
                    | SessionLifecycleState::CleanupFailed
            )
        {
            return Err(public_error("auth_transition_in_progress"));
        }
        if self.account.state == SessionLifecycleState::Authenticated
            && self
                .account
                .access_expires_at_ms
                .is_some_and(|expires| expires > self.clock.now_ms() + ACCESS_SKEW_MS)
        {
            return self
                .account
                .access_token
                .clone()
                .map(RefreshAdmission::Ready)
                .ok_or_else(|| public_error("auth_refresh_unavailable"));
        }
        if let Some(flight) = &self.account.refresh_flight {
            return Ok(RefreshAdmission::Wait(flight.clone()));
        }
        let refresh = load_committed(&mut self.keyring, ACCOUNT_DOMAIN, ACCOUNT_MARKER)?
            .ok_or_else(|| public_error("auth_required"))?;
        let operation_id = self.next_operation();
        let ticket = LifecycleTicket {
            operation_id,
            account_epoch: self.account_epoch,
            account_generation: self.account.generation,
            drive_generation: self.drive.drive_generation,
        };
        self.account.pending_operations.insert(operation_id);
        self.account_drain.admit();
        self.account.refresh_flight = Some(Arc::new((Mutex::new(false), Condvar::new())));
        self.account.state = SessionLifecycleState::Refreshing;
        Ok(RefreshAdmission::Work {
            ticket,
            refresh,
            provider: self.provider.clone(),
        })
    }

    fn finish_refresh(
        &mut self,
        ticket: LifecycleTicket,
        result: Result<LifecycleMaterial, String>,
    ) -> (
        Result<Zeroizing<String>, String>,
        Option<Arc<(Mutex<bool>, Condvar)>>,
    ) {
        let valid = self
            .account
            .pending_operations
            .contains(&ticket.operation_id)
            && self.account_epoch == ticket.account_epoch
            && self.account.generation == ticket.account_generation;
        if !valid {
            self.account.pending_operations.remove(&ticket.operation_id);
            self.account_drain.release();
            return (
                Err(public_error("auth_transition_in_progress")),
                self.account.refresh_flight.take(),
            );
        }
        self.account.pending_operations.remove(&ticket.operation_id);
        self.account_drain.release();
        let notify = self.account.refresh_flight.take();
        let outcome = match result {
            Ok(material) => {
                if let Err(error) = self.accept_material(material) {
                    if self.account.state != SessionLifecycleState::CleanupFailed {
                        self.account.state = SessionLifecycleState::RefreshFailed;
                    }
                    Err(error)
                } else {
                    self.account
                        .access_token
                        .clone()
                        .ok_or_else(|| public_error("auth_refresh_unavailable"))
                }
            }
            Err(error) => {
                self.account.access_token = None;
                self.account.access_expires_at_ms = None;
                self.account.state = if error == "auth_refresh_invalid" || error == "auth_required"
                {
                    SessionLifecycleState::SignedOut
                } else {
                    SessionLifecycleState::RefreshFailed
                };
                Err(error)
            }
        };
        (outcome, notify)
    }
    fn begin_terminal_transition(&mut self) -> (Arc<OperationDrain>, Arc<OperationDrain>) {
        self.quiescing = true;
        self.account_epoch = self.account_epoch.wrapping_add(1);
        self.account.generation = self.account.generation.wrapping_add(1);
        self.account.pending_operations.clear();
        self.account.refresh_flight = None;
        self.drive.drive_generation = self.drive.drive_generation.wrapping_add(1);
        self.drive.quiescing = true;
        self.drive.pending_operations.clear();
        self.clear_memory();
        (self.account_drain.clone(), self.drive_drain.clone())
    }

    fn finish_terminal_transition(&mut self, shutdown: bool) -> Result<LifecycleOutcome, String> {
        let cleanup = clear_credential(
            &mut self.keyring,
            ACCOUNT_DOMAIN,
            ACCOUNT_MARKER,
            &mut self.commit_fence,
        )
        .and_then(|_| {
            if let Some(base) = self.drive.slot_base.clone() {
                clear_credential(
                    &mut self.keyring,
                    &base,
                    &format!("{base}-marker"),
                    &mut self.commit_fence,
                )
            } else {
                Ok(())
            }
        });
        if let Err(error) = cleanup {
            self.account.state = SessionLifecycleState::CleanupFailed;
            return Err(error);
        }
        self.drive.connected = false;
        self.drive.quiescing = false;
        self.account.state = if shutdown {
            SessionLifecycleState::Shutdown
        } else {
            SessionLifecycleState::SignedOut
        };
        if !shutdown {
            self.quiescing = false;
        }
        Ok(self.outcome(if shutdown { "shutdown" } else { "signed_out" }, None))
    }

    pub(crate) fn logout(&mut self) -> Result<LifecycleOutcome, String> {
        let (account_drain, drive_drain) = self.begin_terminal_transition();
        account_drain.wait_empty();
        drive_drain.wait_empty();
        self.finish_terminal_transition(false)
    }
    pub(crate) fn shutdown(&mut self) -> Result<LifecycleOutcome, String> {
        let (account_drain, drive_drain) = self.begin_terminal_transition();
        account_drain.wait_empty();
        drive_drain.wait_empty();
        self.finish_terminal_transition(true)
    }
    #[cfg(test)]
    pub(crate) fn seed_active(&mut self, value: &str) -> Result<(), String> {
        commit_credential(
            &mut self.keyring,
            ACCOUNT_DOMAIN,
            ACCOUNT_MARKER,
            &Zeroizing::new(value.to_owned()),
            "keyring_unavailable",
            &mut self.commit_fence,
        )
    }
    #[cfg(test)]
    pub(crate) fn seed_drive_active(&mut self, domain: &str, value: &str) -> Result<(), String> {
        let ticket = self.begin_drive_operation(domain.to_owned())?;
        let result = self.drive_commit(ticket, &Zeroizing::new(value.to_owned()));
        self.finish_drive_operation(ticket);
        result
    }
    #[cfg(test)]
    pub(crate) fn fail_keyring_at(&mut self, stage: usize) {
        self.keyring.inject_failure(stage);
    }
    #[cfg(test)]
    pub(crate) fn fail_cleanup(&mut self) {
        self.keyring.inject_cleanup_failure();
    }
    #[cfg(test)]
    pub(crate) fn fail_provider_with(&mut self, code: &'static str) {
        self.provider.inject_failure(code);
    }
    #[cfg(test)]
    pub(crate) fn provider_calls(&self) -> usize {
        self.provider.call_count()
    }
    #[cfg(test)]
    pub(crate) fn keyring_events(&self) -> usize {
        self.keyring.event_count()
    }
    #[cfg(test)]
    pub(crate) fn invalidate_generation(&mut self) {
        self.account.generation = self.account.generation.wrapping_add(1);
    }
    pub(crate) fn begin_drive_operation(
        &mut self,
        slot_base: String,
    ) -> Result<LifecycleTicket, String> {
        if self.quiescing || self.drive.quiescing {
            return Err(public_error("auth_transition_in_progress"));
        }
        let operation_id = self.next_operation();
        register_drive_domain(&mut self.keyring, &slot_base)?;
        self.drive_drain.admit();
        self.drive.slot_base = Some(slot_base);
        self.drive.pending_operations.insert(operation_id);
        Ok(LifecycleTicket {
            operation_id,
            account_epoch: self.account_epoch,
            account_generation: self.account.generation,
            drive_generation: self.drive.drive_generation,
        })
    }
    pub(crate) fn drive_commit(
        &mut self,
        ticket: LifecycleTicket,
        token: &Zeroizing<String>,
    ) -> Result<(), String> {
        self.ensure_drive_ticket(ticket)?;
        let base = self
            .drive
            .slot_base
            .clone()
            .ok_or_else(|| public_error("drive_token_storage_failed"))?;
        if let Err(error) = commit_credential(
            &mut self.keyring,
            &base,
            &format!("{base}-marker"),
            token,
            "drive_token_storage_failed",
            &mut self.commit_fence,
        ) {
            if error == "cleanup_failed" {
                self.mark_credential_cleanup_failed();
            }
            return Err(error);
        }
        self.provider.observe("drive-marker-verified");
        self.drive.connected = true;
        Ok(())
    }
    pub(crate) fn ensure_drive_ticket(&self, ticket: LifecycleTicket) -> Result<(), String> {
        if self.quiescing
            || self.account_epoch != ticket.account_epoch
            || self.account.generation != ticket.account_generation
            || self.drive.drive_generation != ticket.drive_generation
            || !self.drive.pending_operations.contains(&ticket.operation_id)
        {
            return Err(public_error("drive_transition_in_progress"));
        }
        Ok(())
    }
    pub(crate) fn finish_drive_operation(&mut self, ticket: LifecycleTicket) {
        self.drive.pending_operations.remove(&ticket.operation_id);
        self.drive_drain.release();
    }
    pub(crate) fn begin_drive_disconnect(&mut self) -> Arc<OperationDrain> {
        self.drive.quiescing = true;
        self.drive.drive_generation = self.drive.drive_generation.wrapping_add(1);
        self.drive.pending_operations.clear();
        self.drive.connected = false;
        self.drive_drain.clone()
    }
    pub(crate) fn finish_drive_disconnect(&mut self) -> Result<(), String> {
        if let Some(base) = self.drive.slot_base.clone() {
            clear_credential(
                &mut self.keyring,
                &base,
                &format!("{base}-marker"),
                &mut self.commit_fence,
            )?;
        }
        self.drive.quiescing = false;
        Ok(())
    }
    pub(crate) fn disconnect_drive(&mut self) -> Result<(), String> {
        let drain = self.begin_drive_disconnect();
        drain.wait_empty();
        self.finish_drive_disconnect()
    }

    pub(crate) fn listener_callback_target(
        &self,
        request: &[u8],
        port: u16,
    ) -> Option<Zeroizing<String>> {
        self.listener.callback_target(request, port)
    }

    pub(crate) fn recover_startup(&mut self) -> Result<(), String> {
        let result = (|| {
            load_committed(&mut self.keyring, ACCOUNT_DOMAIN, ACCOUNT_MARKER)?;
            if let Some(domains) = self.keyring.read(DRIVE_DOMAINS_INDEX)? {
                let registry = serde_json::from_str::<DomainRegistry>(domains.as_str())
                    .map_err(|_| public_error("keyring_unavailable"))?;
                if registry.format_version != KEYRING_FORMAT_VERSION
                    || registry.integrity != registry_digest(&registry.domains)
                {
                    return Err(public_error("keyring_unavailable"));
                }
                for domain in registry.domains {
                    if domain.is_empty() || domain.chars().any(char::is_control) {
                        return Err(public_error("keyring_unavailable"));
                    }
                    load_committed(&mut self.keyring, &domain, &format!("{domain}-marker"))?;
                }
            }
            self.account.startup_checked = true;
            Ok(())
        })();
        if let Err(error) = result {
            self.account.state = SessionLifecycleState::CleanupFailed;
            self.quiescing = true;
            self.drive.quiescing = true;
            self.drive.connected = false;
            self.clear_memory();
            Err(error)
        } else {
            Ok(())
        }
    }

    pub(crate) fn drive_status(&mut self, base: String) -> Result<bool, String> {
        self.drive.slot_base = Some(base.clone());
        Ok(load_committed(&mut self.keyring, &base, &format!("{base}-marker"))?.is_some())
    }
}

pub(crate) struct PendingLogin {
    request_id: String,
    generation: u64,
    port: u16,
    state: Zeroizing<String>,
    verifier: Zeroizing<String>,
    expires_at: SystemTime,
    expires_at_ms: u64,
    callback: Arc<Mutex<Option<Zeroizing<String>>>>,
    cancelled: Arc<AtomicBool>,
}

fn public_error(code: &str) -> String {
    code.to_owned()
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
fn keyring_entry(slot: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, slot).map_err(|_| public_error("keyring_unavailable"))
}

fn read_secret(slot: &str) -> Result<Option<Zeroizing<String>>, String> {
    match keyring_entry(slot)?.get_password() {
        Ok(value) => Ok(Some(Zeroizing::new(value))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(public_error("keyring_unavailable")),
    }
}

fn delete_secret(slot: &str) -> Result<(), String> {
    match keyring_entry(slot)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(public_error("keyring_unavailable")),
    }
}

fn verify_absent(slot: &str) -> Result<(), String> {
    match read_secret(slot)? {
        Some(_) => Err(public_error("keyring_unavailable")),
        None => Ok(()),
    }
}

fn versioned_slot(domain: &str, version: u64) -> String {
    format!("{domain}-slot-v{version}")
}
fn index_slot(domain: &str) -> String {
    if domain == ACCOUNT_DOMAIN {
        ACCOUNT_INDEX.to_owned()
    } else {
        format!("{domain}-slot-index")
    }
}
const KEYRING_FORMAT_VERSION: u8 = 1;

#[derive(Serialize, Deserialize)]
struct CredentialMarker {
    format_version: u8,
    domain: String,
    version: u64,
    slot: String,
    integrity: String,
    content_sha256: String,
}

#[derive(Serialize, Deserialize)]
struct SlotIndex {
    format_version: u8,
    domain: String,
    versions: Vec<u64>,
    integrity: String,
}

#[derive(Serialize, Deserialize)]
struct DomainRegistry {
    format_version: u8,
    domains: Vec<String>,
    integrity: String,
}

fn content_sha256(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn marker_digest(domain: &str, version: u64, slot: &str, content_hash: &str) -> String {
    content_sha256(&format!(
        "{KEYRING_FORMAT_VERSION}|{domain}|{version}|{slot}|{content_hash}"
    ))
}

fn index_digest(domain: &str, versions: &[u64]) -> String {
    content_sha256(&format!(
        "{KEYRING_FORMAT_VERSION}|{domain}|{}",
        versions
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn registry_digest(domains: &[String]) -> String {
    content_sha256(&format!(
        "{KEYRING_FORMAT_VERSION}|{}",
        domains.join("\u{1f}")
    ))
}

fn encode_marker(domain: &str, version: u64, value: &str) -> Result<Zeroizing<String>, String> {
    let slot = versioned_slot(domain, version);
    let content_sha256 = content_sha256(value);
    serde_json::to_string(&CredentialMarker {
        format_version: KEYRING_FORMAT_VERSION,
        domain: domain.to_owned(),
        version,
        slot: slot.clone(),
        integrity: marker_digest(domain, version, &slot, &content_sha256),
        content_sha256,
    })
    .map(Zeroizing::new)
    .map_err(|_| public_error("keyring_unavailable"))
}

fn parse_marker(value: &Zeroizing<String>, domain: &str) -> Result<CredentialMarker, String> {
    let marker = serde_json::from_str::<CredentialMarker>(value.as_str())
        .map_err(|_| public_error("keyring_unavailable"))?;
    let expected_slot = versioned_slot(domain, marker.version);
    if marker.format_version != KEYRING_FORMAT_VERSION
        || marker.domain != domain
        || marker.version == 0
        || marker.slot != expected_slot
        || marker.integrity
            != marker_digest(domain, marker.version, &marker.slot, &marker.content_sha256)
    {
        return Err(public_error("keyring_unavailable"));
    }
    Ok(marker)
}

fn encode_index(domain: &str, versions: &[u64]) -> Result<Zeroizing<String>, String> {
    serde_json::to_string(&SlotIndex {
        format_version: KEYRING_FORMAT_VERSION,
        domain: domain.to_owned(),
        versions: versions.to_vec(),
        integrity: index_digest(domain, versions),
    })
    .map(Zeroizing::new)
    .map_err(|_| public_error("keyring_unavailable"))
}

fn parse_index(value: &Zeroizing<String>, domain: &str) -> Result<Vec<u64>, String> {
    let index = serde_json::from_str::<SlotIndex>(value.as_str())
        .map_err(|_| public_error("keyring_unavailable"))?;
    let mut versions = index.versions;
    versions.sort_unstable();
    if index.format_version != KEYRING_FORMAT_VERSION
        || index.domain != domain
        || index.integrity != index_digest(domain, &versions)
        || versions.is_empty()
        || versions.iter().any(|version| *version == 0)
        || versions.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err(public_error("keyring_unavailable"));
    }
    Ok(versions)
}

fn register_drive_domain<K: KeyringPort>(keyring: &mut K, domain: &str) -> Result<(), String> {
    let current = keyring.read(DRIVE_DOMAINS_INDEX)?;
    let mut domains = match current {
        Some(value) => {
            let registry = serde_json::from_str::<DomainRegistry>(value.as_str())
                .map_err(|_| public_error("keyring_unavailable"))?;
            if registry.format_version != KEYRING_FORMAT_VERSION
                || registry.integrity != registry_digest(&registry.domains)
            {
                return Err(public_error("keyring_unavailable"));
            }
            registry.domains
        }
        None => Vec::new(),
    };
    if domains.iter().any(|entry| entry == domain) {
        return Ok(());
    }
    domains.push(domain.to_owned());
    let encoded = Zeroizing::new(
        serde_json::to_string(&DomainRegistry {
            format_version: KEYRING_FORMAT_VERSION,
            integrity: registry_digest(&domains),
            domains,
        })
        .map_err(|_| public_error("keyring_unavailable"))?,
    );
    keyring
        .write(DRIVE_DOMAINS_INDEX, &encoded)
        .map_err(|_| public_error("keyring_unavailable"))?;
    if !keyring
        .read(DRIVE_DOMAINS_INDEX)?
        .as_ref()
        .is_some_and(|value| {
            value.as_str() == encoded.as_str()
                && serde_json::from_str::<DomainRegistry>(value.as_str())
                    .ok()
                    .is_some_and(|registry| {
                        registry.format_version == KEYRING_FORMAT_VERSION
                            && registry.integrity == registry_digest(&registry.domains)
                    })
        })
    {
        return Err(public_error("keyring_unavailable"));
    }
    Ok(())
}

fn restore_index<K: KeyringPort>(
    keyring: &mut K,
    index: &str,
    previous: Option<&Zeroizing<String>>,
) -> Result<(), String> {
    match previous {
        Some(value) => {
            keyring
                .write(index, value)
                .map_err(|_| public_error("cleanup_failed"))?;
            if !keyring
                .read(index)
                .ok()
                .flatten()
                .as_ref()
                .is_some_and(|current| current.as_str() == value.as_str())
            {
                return Err(public_error("cleanup_failed"));
            }
        }
        None => {
            keyring
                .delete(index)
                .map_err(|_| public_error("cleanup_failed"))?;
            keyring
                .verify_absent(index)
                .map_err(|_| public_error("cleanup_failed"))?;
        }
    }
    Ok(())
}

fn delete_slots<K: KeyringPort>(
    keyring: &mut K,
    domain: &str,
    versions: &[u64],
) -> Result<(), String> {
    for version in versions {
        let slot = versioned_slot(domain, *version);
        keyring
            .delete(&slot)
            .map_err(|_| public_error("cleanup_failed"))?;
        keyring
            .verify_absent(&slot)
            .map_err(|_| public_error("cleanup_failed"))?;
    }
    Ok(())
}

fn load_committed<K: KeyringPort>(
    keyring: &mut K,
    domain: &str,
    marker: &str,
) -> Result<Option<Zeroizing<String>>, String> {
    let marker_value = keyring.read(marker)?;
    let index = index_slot(domain);
    let index_value = keyring.read(&index)?;
    match (marker_value, index_value) {
        (None, None) => Ok(None),
        (None, Some(index_value)) => {
            let versions = parse_index(&index_value, domain)?;
            delete_slots(keyring, domain, &versions)?;
            keyring
                .delete(&index)
                .map_err(|_| public_error("cleanup_failed"))?;
            keyring
                .verify_absent(&index)
                .map_err(|_| public_error("cleanup_failed"))?;
            Ok(None)
        }
        (Some(_), None) => Err(public_error("keyring_unavailable")),
        (Some(marker_value), Some(index_value)) => {
            let marker_record = parse_marker(&marker_value, domain)?;
            let versions = parse_index(&index_value, domain)?;
            if !versions.contains(&marker_record.version) {
                return Err(public_error("keyring_unavailable"));
            }
            let slot = versioned_slot(domain, marker_record.version);
            let value = keyring
                .read(&slot)?
                .ok_or_else(|| public_error("keyring_unavailable"))?;
            if value.trim().is_empty()
                || content_sha256(value.as_str()) != marker_record.content_sha256
            {
                return Err(public_error("keyring_unavailable"));
            }
            let old_versions = versions
                .into_iter()
                .filter(|candidate| *candidate != marker_record.version)
                .collect::<Vec<_>>();
            delete_slots(keyring, domain, &old_versions)?;
            let compacted = encode_index(domain, &[marker_record.version])?;
            keyring
                .write(&index, &compacted)
                .map_err(|_| public_error("cleanup_failed"))?;
            if !keyring
                .read(&index)
                .map_err(|_| public_error("cleanup_failed"))?
                .as_ref()
                .is_some_and(|value| value.as_str() == compacted.as_str())
            {
                return Err(public_error("cleanup_failed"));
            }
            Ok(Some(value))
        }
    }
}

fn restore_marker<K: KeyringPort>(
    keyring: &mut K,
    marker: &str,
    previous: Option<&Zeroizing<String>>,
) -> Result<(), String> {
    match previous {
        Some(value) => {
            keyring
                .write(marker, value)
                .map_err(|_| public_error("cleanup_failed"))?;
            if !keyring
                .read(marker)
                .map_err(|_| public_error("cleanup_failed"))?
                .as_ref()
                .is_some_and(|current| current.as_str() == value.as_str())
            {
                return Err(public_error("cleanup_failed"));
            }
        }
        None => {
            keyring
                .delete(marker)
                .map_err(|_| public_error("cleanup_failed"))?;
            keyring
                .verify_absent(marker)
                .map_err(|_| public_error("cleanup_failed"))?;
        }
    }
    Ok(())
}

fn compensate_commit<K: KeyringPort>(
    keyring: &mut K,
    domain: &str,
    index: &str,
    marker: &str,
    previous_index: Option<&Zeroizing<String>>,
    previous_marker: Option<&Zeroizing<String>>,
    version: u64,
    staged_marker: &Zeroizing<String>,
) -> Result<(), String> {
    let slot = versioned_slot(domain, version);
    keyring
        .delete(&slot)
        .map_err(|_| public_error("cleanup_failed"))?;
    keyring
        .verify_absent(&slot)
        .map_err(|_| public_error("cleanup_failed"))?;
    match keyring
        .read(marker)
        .map_err(|_| public_error("cleanup_failed"))?
    {
        Some(current) if current.as_str() == staged_marker.as_str() => {
            restore_marker(keyring, marker, previous_marker)?;
        }
        Some(_) | None => {}
    }
    restore_index(keyring, index, previous_index)
}

fn commit_credential<K: KeyringPort>(
    keyring: &mut K,
    domain: &str,
    marker: &str,
    value: &Zeroizing<String>,
    error_code: &str,
    fence: &mut CommitFence,
) -> Result<(), String> {
    let marker_value = keyring.read(marker)?;
    let index = index_slot(domain);
    let previous_index = keyring.read(&index)?;
    let previous_marker = marker_value.clone();
    let current = match (&marker_value, &previous_index) {
        (Some(marker_value), Some(index_value)) => {
            let marker_record = parse_marker(marker_value, domain)?;
            let current_slot = versioned_slot(domain, marker_record.version);
            let current_value = keyring
                .read(&current_slot)?
                .ok_or_else(|| public_error(error_code))?;
            if content_sha256(current_value.as_str()) != marker_record.content_sha256
                || !parse_index(index_value, domain)?.contains(&marker_record.version)
            {
                return Err(public_error(error_code));
            }
            marker_record.version
        }
        (None, None) => 0,
        _ => return Err(public_error(error_code)),
    };
    let next = current.saturating_add(1);
    let mut versions = previous_index
        .as_ref()
        .map(|value| parse_index(value, domain))
        .transpose()?
        .unwrap_or_default();
    versions.push(next);
    let staged_index = encode_index(domain, &versions)?;
    if keyring.write(&index, &staged_index).is_err()
        || !keyring
            .read(&index)
            .ok()
            .flatten()
            .as_ref()
            .is_some_and(|value| {
                value.as_str() == staged_index.as_str() && parse_index(value, domain).is_ok()
            })
    {
        let _ = restore_index(keyring, &index, previous_index.as_ref());
        return Err(public_error(error_code));
    }
    let new_slot = versioned_slot(domain, next);
    if keyring.write(&new_slot, value).is_err() {
        let staged_marker = encode_marker(domain, next, value)?;
        let cleanup = compensate_commit(
            keyring,
            domain,
            &index,
            marker,
            previous_index.as_ref(),
            previous_marker.as_ref(),
            next,
            &staged_marker,
        );
        return Err(cleanup.err().unwrap_or_else(|| public_error(error_code)));
    }
    let staged = match keyring.read(&new_slot) {
        Ok(Some(staged)) if staged.as_str() == value.as_str() => staged,
        Ok(_) | Err(_) => {
            let staged_marker = encode_marker(domain, next, value)?;
            let cleanup = compensate_commit(
                keyring,
                domain,
                &index,
                marker,
                previous_index.as_ref(),
                previous_marker.as_ref(),
                next,
                &staged_marker,
            );
            return Err(cleanup.err().unwrap_or_else(|| public_error(error_code)));
        }
    };
    drop(staged);
    let marker_value = encode_marker(domain, next, value)?;
    if keyring.write(marker, &marker_value).is_err() {
        let cleanup = compensate_commit(
            keyring,
            domain,
            &index,
            marker,
            previous_index.as_ref(),
            previous_marker.as_ref(),
            next,
            &marker_value,
        );
        return Err(cleanup.err().unwrap_or_else(|| public_error(error_code)));
    }
    match keyring.read(marker) {
        Ok(Some(value)) if value.as_str() == marker_value.as_str() => {
            let parsed = parse_marker(&value, domain)?;
            if parsed.slot != versioned_slot(domain, next) {
                let cleanup = compensate_commit(
                    keyring,
                    domain,
                    &index,
                    marker,
                    previous_index.as_ref(),
                    previous_marker.as_ref(),
                    next,
                    &marker_value,
                );
                return Err(cleanup
                    .err()
                    .unwrap_or_else(|| public_error("cleanup_failed")));
            }
        }
        Ok(Some(_)) => {
            let cleanup = compensate_commit(
                keyring,
                domain,
                &index,
                marker,
                previous_index.as_ref(),
                previous_marker.as_ref(),
                next,
                &marker_value,
            );
            return Err(cleanup
                .err()
                .unwrap_or_else(|| public_error("cleanup_failed")));
        }
        Ok(None) | Err(_) => {
            let cleanup = compensate_commit(
                keyring,
                domain,
                &index,
                marker,
                previous_index.as_ref(),
                previous_marker.as_ref(),
                next,
                &marker_value,
            );
            return Err(cleanup.err().unwrap_or_else(|| public_error(error_code)));
        }
    }
    if current > 0 {
        let old_slot = versioned_slot(domain, current);
        if keyring.delete(&old_slot).is_err() || keyring.verify_absent(&old_slot).is_err() {
            return Err(public_error("cleanup_failed"));
        }
    }
    let compacted = encode_index(domain, &[next])?;
    if keyring.write(&index, &compacted).is_err()
        || !keyring
            .read(&index)
            .ok()
            .flatten()
            .as_ref()
            .is_some_and(|value| {
                value.as_str() == compacted.as_str() && parse_index(value, domain).is_ok()
            })
    {
        return Err(public_error("cleanup_failed"));
    }
    fence.commits = fence.commits.wrapping_add(1);
    Ok(())
}

fn clear_credential<K: KeyringPort>(
    keyring: &mut K,
    domain: &str,
    marker: &str,
    _fence: &mut CommitFence,
) -> Result<(), String> {
    let index = index_slot(domain);
    let _ = load_committed(keyring, domain, marker).map_err(|_| public_error("cleanup_failed"))?;
    let marker_value = keyring
        .read(marker)
        .map_err(|_| public_error("cleanup_failed"))?;
    let index_value = keyring
        .read(&index)
        .map_err(|_| public_error("cleanup_failed"))?;
    let versions = match (marker_value, index_value) {
        (None, None) => Vec::new(),
        (Some(marker_value), Some(index_value)) => {
            let marker_record =
                parse_marker(&marker_value, domain).map_err(|_| public_error("cleanup_failed"))?;
            let versions =
                parse_index(&index_value, domain).map_err(|_| public_error("cleanup_failed"))?;
            if !versions.contains(&marker_record.version) {
                return Err(public_error("cleanup_failed"));
            }
            versions
        }
        _ => return Err(public_error("cleanup_failed")),
    };
    delete_slots(keyring, domain, &versions)?;
    keyring
        .delete(marker)
        .map_err(|_| public_error("cleanup_failed"))?;
    keyring
        .verify_absent(marker)
        .map_err(|_| public_error("cleanup_failed"))?;
    keyring
        .delete(&index)
        .map_err(|_| public_error("cleanup_failed"))?;
    keyring
        .verify_absent(&index)
        .map_err(|_| public_error("cleanup_failed"))
}

struct NativeKeyring;
impl KeyringPort for NativeKeyring {
    fn read(&mut self, slot: &str) -> Result<Option<Zeroizing<String>>, String> {
        read_secret(slot)
    }
    fn write(&mut self, slot: &str, value: &Zeroizing<String>) -> Result<(), String> {
        keyring_entry(slot)?
            .set_password(value.as_str())
            .map_err(|_| public_error("keyring_unavailable"))
    }
    fn delete(&mut self, slot: &str) -> Result<(), String> {
        delete_secret(slot)
    }
    fn verify_absent(&mut self, slot: &str) -> Result<(), String> {
        verify_absent(slot)
    }
}

struct NativeClock;
impl ClockPort for NativeClock {
    fn now_ms(&self) -> u64 {
        now_ms()
    }
}
struct NativeListener;
impl ListenerCallbackPort for NativeListener {
    fn open(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn close(&mut self) {}
    fn callback_target(&self, request: &[u8], port: u16) -> Option<Zeroizing<String>> {
        callback_from_request(request, port)
    }
}
#[derive(Clone)]
struct NativeProvider;
impl ProviderHttpPort for NativeProvider {
    fn exchange(
        &mut self,
        code: Zeroizing<String>,
        verifier: Zeroizing<String>,
    ) -> Result<LifecycleMaterial, String> {
        let mut url = native_auth::configured_supabase_origin()?;
        url.set_path("/auth/v1/token");
        url.set_query(Some("grant_type=pkce"));
        let response = BlockingClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| public_error("auth_exchange_failed"))?
            .post(url)
            .header("apikey", native_auth::configured_supabase_anon_key()?)
            .form(&[
                ("code", code.as_str()),
                ("code_verifier", verifier.as_str()),
            ])
            .send()
            .map_err(|_| public_error("auth_exchange_failed"))?;
        if !response.status().is_success() {
            return Err(public_error("auth_exchange_failed"));
        }
        let token = response
            .json::<AuthTokenResponse>()
            .map_err(|_| public_error("auth_exchange_failed"))?;
        if token.error.is_some() || token.refresh.is_none() {
            return Err(public_error("auth_exchange_failed"));
        }
        let access = token.access;
        let refresh = token
            .refresh
            .ok_or_else(|| public_error("auth_exchange_failed"))?;
        let mut user_url = native_auth::configured_supabase_origin()?;
        user_url.set_path("/auth/v1/user");
        let user = BlockingClient::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| public_error("auth_exchange_failed"))?
            .get(user_url)
            .bearer_auth(access.as_str())
            .header("apikey", native_auth::configured_supabase_anon_key()?)
            .send()
            .map_err(|_| public_error("auth_exchange_failed"))?
            .json::<AuthUser>()
            .map_err(|_| public_error("auth_exchange_failed"))?;
        Ok(LifecycleMaterial {
            access,
            refresh,
            user_id: Some(user.id),
            email: user.email,
            access_expires_at_ms: Some(now_ms() + token.expires_in.unwrap_or(3600) * 1000),
        })
    }
    fn refresh(&mut self, refresh: Zeroizing<String>) -> Result<LifecycleMaterial, String> {
        let old = refresh;
        let mut url = native_auth::configured_supabase_origin()?;
        url.set_path("/auth/v1/token");
        url.set_query(Some("grant_type=refresh_token"));
        let response = BlockingClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| public_error("auth_refresh_unavailable"))?
            .post(url)
            .header("apikey", native_auth::configured_supabase_anon_key()?)
            .form(&[("refresh_token", old.as_str())])
            .send()
            .map_err(|_| public_error("auth_refresh_unavailable"))?;
        if response.status().as_u16() == 400 {
            return Err(public_error("auth_refresh_invalid"));
        }
        if !response.status().is_success() {
            return Err(public_error("auth_refresh_unavailable"));
        }
        let token = response
            .json::<AuthTokenResponse>()
            .map_err(|_| public_error("auth_refresh_unavailable"))?;
        if token.error.is_some() {
            return Err(public_error("auth_refresh_invalid"));
        }
        Ok(LifecycleMaterial {
            access: token.access,
            refresh: token.refresh.unwrap_or(old),
            user_id: None,
            email: None,
            access_expires_at_ms: Some(now_ms() + token.expires_in.unwrap_or(3600) * 1000),
        })
    }
    fn drive_exchange(
        &self,
        client_id: String,
        redirect_uri: Zeroizing<String>,
        code: Zeroizing<String>,
        verifier: Zeroizing<String>,
    ) -> Result<DriveTokenMaterial, String> {
        let response = BlockingClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| public_error("drive_oauth_token_exchange_failed"))?
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", client_id.as_str()),
                ("code", code.as_str()),
                ("code_verifier", verifier.as_str()),
                ("grant_type", "authorization_code"),
                ("redirect_uri", redirect_uri.as_str()),
            ])
            .send()
            .map_err(|_| public_error("drive_oauth_token_exchange_failed"))?;
        if !response.status().is_success() {
            return Err(public_error("drive_oauth_token_exchange_failed"));
        }
        let token = response
            .json::<DriveTokenResponse>()
            .map_err(|_| public_error("drive_oauth_token_exchange_failed"))?;
        if token.error.is_some() || token.access.is_none() {
            return Err(public_error("drive_oauth_token_exchange_failed"));
        }
        Ok(DriveTokenMaterial {
            access: token
                .access
                .ok_or_else(|| public_error("drive_oauth_token_exchange_failed"))?,
            refresh: token.refresh,
            scope: token.scope,
        })
    }
    fn drive_refresh(
        &self,
        client_id: String,
        refresh: Zeroizing<String>,
    ) -> Result<DriveTokenMaterial, String> {
        let response = BlockingClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| public_error("drive_token_refresh_failed"))?
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
            ])
            .send()
            .map_err(|_| public_error("drive_token_refresh_failed"))?;
        if !response.status().is_success() {
            return Err(public_error("drive_token_refresh_failed"));
        }
        let token = response
            .json::<DriveTokenResponse>()
            .map_err(|_| public_error("drive_token_refresh_failed"))?;
        if token.error.is_some() || token.access.is_none() {
            return Err(public_error("drive_token_refresh_failed"));
        }
        Ok(DriveTokenMaterial {
            access: token
                .access
                .ok_or_else(|| public_error("drive_token_refresh_failed"))?,
            refresh: token.refresh,
            scope: token.scope,
        })
    }
}
impl DriveHttpPort for NativeProvider {}
impl ArchiveJobPort for NativeProvider {}
impl CommitObservationPort for NativeProvider {}

type NativeSessionLifecycle =
    SessionLifecycle<NativeKeyring, NativeClock, NativeListener, NativeProvider>;
fn production_lifecycle() -> &'static Mutex<NativeSessionLifecycle> {
    static LIFECYCLE: OnceLock<Mutex<NativeSessionLifecycle>> = OnceLock::new();
    LIFECYCLE.get_or_init(|| {
        Mutex::new(SessionLifecycle::new(
            NativeKeyring,
            NativeClock,
            NativeListener,
            NativeProvider,
        ))
    })
}

fn registered_listener_callback(request: &[u8], port: u16) -> Option<Zeroizing<String>> {
    production_lifecycle()
        .lock()
        .ok()
        .and_then(|lifecycle| lifecycle.listener_callback_target(request, port))
}
pub(crate) fn drive_begin(
    slot_base: String,
) -> Result<(LifecycleTicket, Arc<OperationDrain>), String> {
    let mut lifecycle = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?;
    let ticket = lifecycle.begin_drive_operation(slot_base)?;
    Ok((ticket, lifecycle.drive_drain.clone()))
}
pub(crate) fn account_begin_operation() -> Result<AccountOperationGuard, String> {
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .begin_account_operation()
}
pub(crate) fn drive_check(ticket: LifecycleTicket) -> Result<(), String> {
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .ensure_drive_ticket(ticket)
}
pub(crate) fn drive_commit(
    ticket: LifecycleTicket,
    token: &Zeroizing<String>,
) -> Result<(), String> {
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .drive_commit(ticket, token)
}
pub(crate) fn drive_finish(ticket: LifecycleTicket) {
    if let Ok(mut lifecycle) = production_lifecycle().lock() {
        lifecycle.finish_drive_operation(ticket);
    }
}
pub(crate) fn drive_status(slot_base: String) -> Result<bool, String> {
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .drive_status(slot_base)
}
pub(crate) fn drive_load(slot_base: String) -> Result<Option<Zeroizing<String>>, String> {
    let mut lifecycle = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?;
    lifecycle.drive.slot_base = Some(slot_base.clone());
    load_committed(
        &mut lifecycle.keyring,
        &slot_base,
        &format!("{slot_base}-marker"),
    )
}
pub(crate) fn drive_disconnect() -> Result<(), String> {
    let drain = {
        let mut lifecycle = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?;
        lifecycle.begin_drive_disconnect()
    };
    drain.wait_empty();
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .finish_drive_disconnect()
}

pub(crate) fn drive_provider_exchange(
    ticket: LifecycleTicket,
    client_id: String,
    redirect_uri: Zeroizing<String>,
    code: Zeroizing<String>,
    verifier: Zeroizing<String>,
) -> Result<DriveTokenMaterial, String> {
    let provider = {
        let lifecycle = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?;
        lifecycle.ensure_drive_ticket(ticket)?;
        lifecycle.provider.clone()
    };
    let material = provider.drive_exchange(client_id, redirect_uri, code, verifier)?;
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .ensure_drive_ticket(ticket)?;
    Ok(material)
}

pub(crate) fn drive_provider_refresh(
    ticket: LifecycleTicket,
    client_id: String,
    refresh: Zeroizing<String>,
) -> Result<DriveTokenMaterial, String> {
    let provider = {
        let lifecycle = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?;
        lifecycle.ensure_drive_ticket(ticket)?;
        lifecycle.provider.clone()
    };
    let material = provider.drive_refresh(client_id, refresh)?;
    let mut lifecycle = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?;
    lifecycle.ensure_drive_ticket(ticket)?;
    if let Some(refresh) = material.refresh.as_ref() {
        lifecycle.drive_commit(ticket, refresh)?;
    }
    Ok(material)
}
pub(crate) fn startup_recover() -> Result<(), String> {
    let mut lifecycle = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?;
    lifecycle.recover_startup()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStatus {
    pub(crate) state: &'static str,
    pub(crate) user_id: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) access_expires_at_ms: Option<u64>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginStarted {
    pub(crate) request_id: String,
    pub(crate) expires_at_ms: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Cancelled {
    pub(crate) request_id: String,
    pub(crate) status: &'static str,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrollmentResult {
    pub(crate) request_id: String,
    pub(crate) status: &'static str,
    pub(crate) authority_state: &'static str,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrollmentStatus {
    pub(crate) status: String,
    pub(crate) device_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingResult {
    pub(crate) pairing_id: String,
    pub(crate) display_code: String,
    pub(crate) expires_at_ms: u64,
    pub(crate) status: &'static str,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingPeer {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) platform: String,
    pub(crate) fingerprint: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingPoll {
    pub(crate) status: String,
    pub(crate) peer: Option<PairingPeer>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReconcileStatus {
    pub(crate) reconciled: bool,
    pub(crate) device_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RevokeResult {
    pub(crate) device_id: String,
    pub(crate) status: &'static str,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditRow {
    pub(crate) event_id: String,
    pub(crate) event_type: String,
    pub(crate) created_at: String,
    pub(crate) device_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceRow {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) platform: String,
    pub(crate) authority_state: String,
    pub(crate) paired_at: Option<String>,
    pub(crate) revoked_at: Option<String>,
    pub(crate) endpoint_state: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EndpointStatus {
    pub(crate) status: &'static str,
    pub(crate) updated_at: Option<String>,
}

fn deserialize_zeroizing<'de, D>(deserializer: D) -> Result<Zeroizing<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Zeroizing::new)
}
fn deserialize_optional_zeroizing<'de, D>(
    deserializer: D,
) -> Result<Option<Zeroizing<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|value| value.map(Zeroizing::new))
}

#[derive(Deserialize)]
struct AuthTokenResponse {
    #[serde(rename = "access_token", deserialize_with = "deserialize_zeroizing")]
    access: Zeroizing<String>,
    #[serde(
        rename = "refresh_token",
        default,
        deserialize_with = "deserialize_optional_zeroizing"
    )]
    refresh: Option<Zeroizing<String>>,
    expires_in: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_zeroizing")]
    error: Option<Zeroizing<String>>,
}
#[derive(Deserialize)]
struct DriveTokenResponse {
    #[serde(
        rename = "access_token",
        default,
        deserialize_with = "deserialize_optional_zeroizing"
    )]
    access: Option<Zeroizing<String>>,
    #[serde(
        rename = "refresh_token",
        default,
        deserialize_with = "deserialize_optional_zeroizing"
    )]
    refresh: Option<Zeroizing<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_zeroizing")]
    scope: Option<Zeroizing<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_zeroizing")]
    error: Option<Zeroizing<String>>,
}
#[derive(Deserialize)]
struct AuthUser {
    id: String,
    email: Option<String>,
}
fn callback_pair() -> (Zeroizing<String>, String) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let verifier = Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes));
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}
fn safe_callback_value(value: &str) -> bool {
    !value.is_empty() && value.len() <= 8192 && !value.chars().any(char::is_control)
}

fn callback_from_request(request: &[u8], port: u16) -> Option<Zeroizing<String>> {
    let first = request
        .split(|byte| *byte == b'\n')
        .next()?
        .strip_suffix(b"\r")
        .unwrap_or(request);
    let mut fields = first.split(|byte| *byte == b' ');
    if fields.next()? != b"GET" {
        return None;
    }
    let target = fields.next()?;
    if !target.starts_with(CALLBACK_PATH.as_bytes()) {
        return None;
    }
    let target = std::str::from_utf8(target).ok()?;
    let mut callback = Zeroizing::new(String::with_capacity(target.len() + 32));
    callback.push_str("http://127.0.0.1:");
    callback.push_str(&port.to_string());
    callback.push_str(target);
    Some(callback)
}

fn parse_callback(raw: &str, pending: &PendingLogin) -> Result<Zeroizing<String>, String> {
    let prefix = format!("http://127.0.0.1:{}", pending.port);
    let target = raw
        .strip_prefix(&prefix)
        .ok_or_else(|| public_error("auth_callback_invalid"))?;
    let (path, query) = target
        .split_once('?')
        .ok_or_else(|| public_error("auth_callback_invalid"))?;
    if path != CALLBACK_PATH || query.contains('#') {
        return Err(public_error("auth_callback_invalid"));
    }
    let mut names = HashSet::new();
    let mut count = 0usize;
    let mut state: Option<Zeroizing<String>> = None;
    let mut code = None;
    let mut error = false;
    let mut error_description = false;
    for pair in query.split('&') {
        let (raw_name, raw_value) = pair
            .split_once('=')
            .ok_or_else(|| public_error("auth_callback_invalid"))?;
        let name = decode_callback_component(raw_name)?;
        let value = decode_callback_component(raw_value)?;
        count += 1;
        let name_text = name.to_string();
        if !matches!(
            name.as_str(),
            "code" | "error" | "error_description" | "state"
        ) || !names.insert(name_text)
            || !safe_callback_value(value.as_str())
        {
            return Err(public_error("auth_callback_invalid"));
        }
        match name.as_str() {
            "state" => state = Some(value),
            "code" => code = Some(value),
            "error" => error = true,
            "error_description" => error_description = true,
            _ => {}
        }
    }
    if count == 0 || state.as_ref().map(|value| value.as_str()) != Some(pending.state.as_str()) {
        return Err(public_error("auth_state_mismatch"));
    }
    if code.is_some() == error || (error_description && !error) {
        return Err(public_error("auth_callback_invalid"));
    }
    if let Some(code) = code {
        if count != 2 {
            return Err(public_error("auth_callback_invalid"));
        }
        return Ok(code);
    }
    Err(public_error("authorization_denied"))
}

fn decode_callback_component(value: &str) -> Result<Zeroizing<String>, String> {
    let mut decoded = Zeroizing::new(String::with_capacity(value.len()));
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = |byte: u8| -> Option<u8> {
                    match byte {
                        b'0'..=b'9' => Some(byte - b'0'),
                        b'a'..=b'f' => Some(byte - b'a' + 10),
                        b'A'..=b'F' => Some(byte - b'A' + 10),
                        _ => None,
                    }
                };
                let high =
                    hex(bytes[index + 1]).ok_or_else(|| public_error("auth_callback_invalid"))?;
                let low =
                    hex(bytes[index + 2]).ok_or_else(|| public_error("auth_callback_invalid"))?;
                decoded.push((high << 4 | low) as char);
                index += 3;
            }
            b'+' => {
                decoded.push(' ');
                index += 1;
            }
            byte if byte.is_ascii() => {
                decoded.push(byte as char);
                index += 1;
            }
            _ => return Err(public_error("auth_callback_invalid")),
        }
    }
    Ok(decoded)
}

fn spawn_listener(
    listener: TcpListener,
    port: u16,
    callback: Arc<Mutex<Option<Zeroizing<String>>>>,
    cancelled: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let deadline = SystemTime::now() + LOGIN_TTL;
        let _ = listener.set_nonblocking(true);
        loop {
            if cancelled.load(Ordering::Acquire) || SystemTime::now() >= deadline {
                return;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut bytes = Zeroizing::new([0u8; 8192]);
                    let count = stream.read(&mut bytes[..]).unwrap_or(0);
                    let callback_url = registered_listener_callback(&bytes[..count], port);
                    let body = if callback_url.is_some() {
                        "Authentication received. You may close this window."
                    } else {
                        "Authentication rejected."
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    if let Some(value) = callback_url {
                        if let Ok(mut slot) = callback.lock() {
                            *slot = Some(value);
                        }
                    }
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(_) => return,
            }
        }
    });
}

async fn ensure_startup() -> Result<(), String> {
    let checked = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .account
        .startup_checked;
    if !checked {
        startup_recover()?;
    }
    Ok(())
}

pub(crate) async fn ensure_access_token() -> Result<Zeroizing<String>, String> {
    ensure_startup().await?;
    loop {
        let admission = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?
            .begin_refresh()?;
        let (ticket, refresh, provider) = match admission {
            RefreshAdmission::Ready(token) => return Ok(token),
            RefreshAdmission::Wait(wait) => {
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    let (lock, signal) = &*wait;
                    let mut done = lock.lock().map_err(|_| ())?;
                    while !*done {
                        done = signal.wait(done).map_err(|_| ())?;
                    }
                    Ok::<(), ()>(())
                })
                .await;
                continue;
            }
            RefreshAdmission::Work {
                ticket,
                refresh,
                provider,
            } => (ticket, refresh, provider),
        };
        let result = tauri::async_runtime::spawn_blocking(move || {
            let mut provider = provider;
            provider.refresh(refresh)
        })
        .await
        .map_err(|_| public_error("auth_refresh_unavailable"))?;
        let (result, notify) = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?
            .finish_refresh(ticket, result);
        if let Some(notify) = notify {
            let (lock, signal) = &*notify;
            if let Ok(mut done) = lock.lock() {
                *done = true;
                signal.notify_all();
            }
        }
        return result;
    }
}

pub(crate) fn native_user_id() -> Option<String> {
    production_lifecycle()
        .lock()
        .ok()
        .and_then(|state| state.account.user_id.clone())
}

async fn finish_login(_app: AppHandle, request_id: String, generation: u64) {
    loop {
        let maybe = {
            let state = match production_lifecycle().lock() {
                Ok(value) => value,
                Err(_) => return,
            };
            if state.account.generation != generation
                || state
                    .account
                    .pending_login
                    .as_ref()
                    .is_none_or(|p| p.request_id != request_id)
            {
                return;
            }
            state
                .account
                .pending_login
                .as_ref()
                .and_then(|p| p.callback.lock().ok().and_then(|mut slot| slot.take()))
        };
        if let Some(raw) = maybe {
            let work = production_lifecycle().lock().ok().and_then(|mut state| {
                state.registered_login_take_for_exchange(&request_id, generation)
            });
            if let Some((pending, ticket, provider)) = work {
                let result = match parse_callback(raw.as_str(), &pending) {
                    Ok(code) => match tauri::async_runtime::spawn_blocking(move || {
                        let mut provider = provider;
                        provider.exchange(code, pending.verifier)
                    })
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => Err(public_error("auth_exchange_failed")),
                    },
                    Err(error) => Err(error),
                };
                if let Ok(mut state) = production_lifecycle().lock() {
                    let _ = state.registered_login_complete(ticket, result);
                }
            }
            return;
        }
        let expired = production_lifecycle()
            .lock()
            .ok()
            .map(|state| state.login_expired())
            .unwrap_or(true);
        if expired {
            if let Ok(mut state) = production_lifecycle().lock() {
                state.account.pending_login = None;
                if state.account.generation == generation {
                    state.account.state = SessionLifecycleState::SignedOut;
                }
            }
            return;
        }
        let _ =
            tauri::async_runtime::spawn_blocking(|| thread::sleep(Duration::from_millis(50))).await;
    }
}

#[tauri::command]
pub(crate) async fn broker_session_login_begin(app: AppHandle) -> Result<LoginStarted, String> {
    let origin = native_auth::configured_supabase_origin()?;
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|_| public_error("auth_listener_unavailable"))?;
    let port = listener
        .local_addr()
        .map_err(|_| public_error("auth_listener_unavailable"))?
        .port();
    let request_id = Uuid::new_v4().to_string();
    let state_value = Zeroizing::new(Uuid::new_v4().to_string());
    let (verifier, challenge) = callback_pair();
    let callback = Arc::new(Mutex::new(None));
    let cancelled = Arc::new(AtomicBool::new(false));
    let generation = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .registered_login_begin(PendingLogin {
            request_id: request_id.clone(),
            generation: 0,
            port,
            state: state_value.clone(),
            verifier,
            expires_at: SystemTime::now() + LOGIN_TTL,
            expires_at_ms: now_ms() + LOGIN_TTL.as_millis() as u64,
            callback: callback.clone(),
            cancelled: cancelled.clone(),
        })?;
    if let Ok(mut state) = production_lifecycle().lock() {
        if let Some(pending) = state.account.pending_login.as_mut() {
            pending.generation = generation;
        }
    }
    spawn_listener(listener, port, callback, cancelled);
    let mut redirect_to = Zeroizing::new(String::with_capacity(64));
    redirect_to.push_str("http://127.0.0.1:");
    redirect_to.push_str(&port.to_string());
    redirect_to.push_str(CALLBACK_PATH);
    let base = origin.to_string();
    let mut url = Zeroizing::new(String::with_capacity(base.len() + 256));
    url.push_str(base.trim_end_matches('/'));
    url.push_str("/auth/v1/authorize?provider=google&redirect_to=");
    url.push_str(&redirect_to.replace(':', "%3A").replace('/', "%2F"));
    url.push_str("&state=");
    url.push_str(state_value.as_str());
    url.push_str("&code_challenge=");
    url.push_str(&challenge);
    let pkce_method = ("code_challenge_method", "S256");
    url.push('&');
    url.push_str(pkce_method.0);
    url.push('=');
    url.push_str(pkce_method.1);
    if app.opener().open_url(url.as_str(), None::<&str>).is_err() {
        if let Ok(mut state) = production_lifecycle().lock() {
            state.account.pending_login = None;
            state.account.state = SessionLifecycleState::SignedOut;
        }
        return Err(public_error("auth_url_open_failed"));
    }
    tauri::async_runtime::spawn(finish_login(app, request_id.clone(), generation));
    Ok(LoginStarted {
        request_id,
        expires_at_ms: now_ms() + LOGIN_TTL.as_millis() as u64,
    })
}

#[tauri::command]
pub(crate) async fn broker_session_login_cancel(request_id: String) -> Result<Cancelled, String> {
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .cancel_login(&request_id)?;
    Ok(Cancelled {
        request_id,
        status: "cancelled",
    })
}

#[tauri::command]
pub(crate) async fn broker_session_status() -> Result<SessionStatus, String> {
    let has_keyring = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .account
        .startup_checked
        || load_committed(
            &mut production_lifecycle()
                .lock()
                .map_err(|_| public_error("auth_unavailable"))?
                .keyring,
            ACCOUNT_DOMAIN,
            ACCOUNT_MARKER,
        )?
        .is_some();
    let _ = ensure_startup().await;
    if has_keyring {
        let _ = ensure_access_token().await;
    }
    let state = production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?;
    Ok(SessionStatus {
        state: state.account.state.as_str(),
        user_id: state.account.user_id.clone(),
        email: state.account.email.clone(),
        access_expires_at_ms: state.account.access_expires_at_ms,
    })
}

#[tauri::command]
pub(crate) async fn broker_session_logout() -> Result<SessionStatus, String> {
    let (account_drain, drive_drain) = {
        let mut state = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?;
        if state.account.state == SessionLifecycleState::Shutdown {
            return Ok(SessionStatus {
                state: "shutdown",
                user_id: None,
                email: None,
                access_expires_at_ms: None,
            });
        }
        state.account.state = SessionLifecycleState::LogoutPending;
        state.begin_terminal_transition()
    };
    account_drain.wait_empty();
    drive_drain.wait_empty();
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .finish_terminal_transition(false)
        .map_err(|_| public_error("auth_logout_incomplete"))?;
    Ok(SessionStatus {
        state: "signed_out",
        user_id: None,
        email: None,
        access_expires_at_ms: None,
    })
}

pub(crate) fn shutdown() -> Result<(), String> {
    let (account_drain, drive_drain) = {
        let mut lifecycle = production_lifecycle()
            .lock()
            .map_err(|_| public_error("auth_unavailable"))?;
        lifecycle.begin_terminal_transition()
    };
    account_drain.wait_empty();
    drive_drain.wait_empty();
    production_lifecycle()
        .lock()
        .map_err(|_| public_error("auth_unavailable"))?
        .finish_terminal_transition(true)
        .map(|_| ())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EnrollmentInput {
    pub(crate) device_label: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentRequest<'a> {
    native_proof: &'a native_auth::NativeEnrollmentProof,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentResponse {
    request_id: String,
    status: String,
}

#[derive(Deserialize)]
struct NativeErrorBody {
    code: Option<String>,
}

fn allowed_native_error(code: &str) -> bool {
    matches!(
        code,
        "authorization_denied"
            | "device_not_found"
            | "enrollment_unavailable"
            | "invalid_native_proof"
            | "proof_replayed"
            | "auth_required"
    )
}

async fn native_post<T: for<'de> Deserialize<'de>>(
    path: &str,
    body: impl Serialize,
) -> Result<T, String> {
    let operation = account_begin_operation()?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?;
    url.set_path(path);
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| public_error("authorization_unavailable"))?
        .post(url)
        .bearer_auth(access.as_str())
        .header("apikey", native_auth::configured_supabase_anon_key()?)
        .json(&body)
        .send()
        .await
        .map_err(|_| public_error("authorization_unavailable"))?;
    if !response.status().is_success() {
        let code = response
            .json::<NativeErrorBody>()
            .await
            .ok()
            .and_then(|body| body.code)
            .filter(|code| allowed_native_error(code));
        operation.check()?;
        return Err(public_error(
            code.as_deref().unwrap_or("authorization_denied"),
        ));
    }
    let value = response
        .json::<T>()
        .await
        .map_err(|_| public_error("authorization_unavailable"))?;
    operation.check()?;
    Ok(value)
}

#[tauri::command]
pub(crate) async fn broker_enrollment_request(
    app: AppHandle,
    input: EnrollmentInput,
) -> Result<EnrollmentResult, String> {
    let proof = native_auth::native_device_enrollment_proof(&app, &input.device_label).await?;
    let response: EnrollmentResponse = native_post(
        "/functions/v1/device-enrollment",
        EnrollmentRequest {
            native_proof: &proof,
        },
    )
    .await?;
    if response.status != "pending" {
        return Err(public_error("enrollment_unavailable"));
    }
    Ok(EnrollmentResult {
        request_id: response.request_id,
        status: "pending",
        authority_state: "pending",
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct DeviceWire {
    id: String,
    device_label: Option<String>,
    platform: String,
    authority_state: String,
    registered_at: Option<String>,
    revoked_at: Option<String>,
    lan_endpoint: Option<String>,
    public_key_fingerprint: Option<String>,
}
async fn device_rows() -> Result<Vec<DeviceWire>, String> {
    let operation = account_begin_operation()?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?;
    url.set_path("/rest/v1/devices");
    url.set_query(Some("select=id,device_label,platform,authority_state,registered_at,revoked_at,lan_endpoint,public_key_fingerprint&order=registered_at.desc"));
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| public_error("authorization_unavailable"))?
        .get(url)
        .bearer_auth(access.as_str())
        .header("apikey", native_auth::configured_supabase_anon_key()?)
        .send()
        .await
        .map_err(|_| public_error("authorization_unavailable"))?;
    if !response.status().is_success() {
        return Err(public_error("authorization_unavailable"));
    }
    let rows = response
        .json::<Vec<DeviceWire>>()
        .await
        .map_err(|_| public_error("authorization_unavailable"))?;
    operation.check()?;
    Ok(rows)
}

#[tauri::command]
pub(crate) async fn broker_enrollment_status(app: AppHandle) -> Result<EnrollmentStatus, String> {
    let (_, fingerprint) = current_identity(&app)?;
    let row = device_rows().await?.into_iter().find(|row| {
        row.public_key_fingerprint
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))
    });
    Ok(match row {
        Some(row) => EnrollmentStatus {
            status: row.authority_state,
            device_id: Some(row.id),
        },
        None => EnrollmentStatus {
            status: "legacy".to_owned(),
            device_id: None,
        },
    })
}
#[tauri::command]
pub(crate) async fn broker_device_list() -> Result<Vec<DeviceRow>, String> {
    Ok(device_rows()
        .await?
        .into_iter()
        .map(|row| DeviceRow {
            id: row.id,
            label: row.device_label.unwrap_or_default(),
            platform: row.platform,
            authority_state: row.authority_state,
            paired_at: row.registered_at,
            revoked_at: row.revoked_at,
            endpoint_state: row.lan_endpoint.map(|_| "published".to_owned()),
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EndpointUpdate<'a> {
    lan_endpoint: &'a str,
    lan_endpoint_updated_at: &'a str,
}

#[tauri::command]
pub(crate) async fn broker_device_endpoint_publish(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EndpointStatus, String> {
    let endpoint = crate::fungwire_local_endpoint_native(state)
        .map_err(|_| public_error("fungwire_unavailable"))?;
    let Some(endpoint) = endpoint else {
        return Ok(EndpointStatus {
            status: "unavailable",
            updated_at: None,
        });
    };
    let (_, fingerprint) = current_identity(&app)?;
    let device = device_rows()
        .await?
        .into_iter()
        .find(|row| {
            row.public_key_fingerprint
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))
        })
        .ok_or_else(|| public_error("device_not_enrolled"))?;
    let operation = account_begin_operation()?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?;
    url.set_path("/rest/v1/devices");
    url.set_query(Some(&format!("id=eq.{}", device.id)));
    let updated_at = chrono::Utc::now().to_rfc3339();
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| public_error("authorization_unavailable"))?
        .patch(url)
        .bearer_auth(access.as_str())
        .header("apikey", native_auth::configured_supabase_anon_key()?)
        .header("Prefer", "return=representation")
        .json(&EndpointUpdate {
            lan_endpoint: &endpoint,
            lan_endpoint_updated_at: &updated_at,
        })
        .send()
        .await
        .map_err(|_| public_error("authorization_unavailable"))?;
    if !response.status().is_success() {
        operation.check()?;
        return Err(public_error("authorization_denied"));
    }
    let updated: Vec<DeviceWire> = response
        .json()
        .await
        .map_err(|_| public_error("authorization_unavailable"))?;
    if updated.len() != 1 {
        operation.check()?;
        return Err(public_error("authorization_denied"));
    }
    operation.check()?;
    Ok(EndpointStatus {
        status: "published",
        updated_at: Some(updated_at),
    })
}

fn current_identity(app: &AppHandle) -> Result<(String, String), String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|_| public_error("device_identity_unavailable"))?;
    device_identity::authorization_identity_in_dir(&app_data)
        .map_err(|_| public_error("device_identity_unavailable"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairingCreateInput {
    pub(crate) label: Option<String>,
}
#[derive(Serialize)]
struct PairingRpc<'a> {
    p_session_id: &'a str,
    p_code_hash: &'a str,
    p_initiator_device_id: &'a str,
}
#[derive(Deserialize)]
struct PairingSessionWire {
    status: String,
    responder_device_id: Option<String>,
}

async fn rpc<T: for<'de> Deserialize<'de>>(name: &str, body: impl Serialize) -> Result<T, String> {
    let operation = account_begin_operation()?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?;
    url.set_path(&format!("/rest/v1/rpc/{name}"));
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| public_error("pairing_unavailable"))?
        .post(url)
        .bearer_auth(access.as_str())
        .header("apikey", native_auth::configured_supabase_anon_key()?)
        .json(&body)
        .send()
        .await
        .map_err(|_| public_error("pairing_unavailable"))?;
    if !response.status().is_success() {
        operation.check()?;
        return Err(public_error("pairing_unavailable"));
    }
    let value = response
        .json::<T>()
        .await
        .map_err(|_| public_error("pairing_unavailable"))?;
    operation.check()?;
    Ok(value)
}

#[tauri::command]
pub(crate) async fn broker_pairing_create(
    app: AppHandle,
    input: Option<PairingCreateInput>,
) -> Result<PairingResult, String> {
    if input
        .as_ref()
        .and_then(|value| value.label.as_deref())
        .is_some_and(|label| label.len() > 80 || label.chars().any(char::is_control))
    {
        return Err(public_error("invalid_input"));
    }
    let devices = device_rows().await?;
    let (_, fingerprint) = current_identity(&app)?;
    let device = devices
        .into_iter()
        .find(|row| {
            row.public_key_fingerprint
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))
        })
        .ok_or_else(|| public_error("device_not_enrolled"))?;
    let session_id = Uuid::new_v4().to_string();
    let mut code_bytes = [0u8; 4];
    OsRng.fill_bytes(&mut code_bytes);
    let code = format!("{:06}", u32::from_be_bytes(code_bytes) % 1_000_000);
    let digest = Sha256::digest(format!("{session_id}:{code}").as_bytes());
    let hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let _: Option<String> = rpc(
        "create_pairing_session",
        PairingRpc {
            p_session_id: &session_id,
            p_code_hash: &hash,
            p_initiator_device_id: &device.id,
        },
    )
    .await?;
    Ok(PairingResult {
        pairing_id: session_id,
        display_code: code,
        expires_at_ms: now_ms() + 300_000,
        status: "waiting",
    })
}

async fn pairing_row(pairing_id: &str) -> Result<PairingSessionWire, String> {
    if Uuid::parse_str(pairing_id).is_err() {
        return Err(public_error("pairing_not_found"));
    }
    let operation = account_begin_operation()?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?;
    url.set_path("/rest/v1/pairing_sessions");
    url.set_query(Some(&format!(
        "id=eq.{pairing_id}&select=id,status,responder_device_id"
    )));
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| public_error("pairing_unavailable"))?
        .get(url)
        .bearer_auth(access.as_str())
        .header("apikey", native_auth::configured_supabase_anon_key()?)
        .send()
        .await
        .map_err(|_| public_error("pairing_unavailable"))?;
    if !response.status().is_success() {
        operation.check()?;
        return Err(public_error("pairing_unavailable"));
    }
    let row = response
        .json::<Vec<PairingSessionWire>>()
        .await
        .map_err(|_| public_error("pairing_unavailable"))?
        .into_iter()
        .next()
        .ok_or_else(|| public_error("pairing_not_found"))?;
    operation.check()?;
    Ok(row)
}

#[tauri::command]
pub(crate) async fn broker_pairing_poll(pairing_id: String) -> Result<PairingPoll, String> {
    let row = pairing_row(&pairing_id).await?;
    let peer = if let Some(device_id) = row.responder_device_id {
        device_rows()
            .await?
            .into_iter()
            .find(|device| device.id == device_id)
            .map(|device| PairingPeer {
                id: device.id,
                label: device.device_label.unwrap_or_default(),
                platform: device.platform,
                fingerprint: device.public_key_fingerprint.unwrap_or_default(),
            })
    } else {
        None
    };
    Ok(PairingPoll {
        status: row.status,
        peer,
    })
}
#[tauri::command]
pub(crate) async fn broker_pairing_reconcile(app: AppHandle) -> Result<ReconcileStatus, String> {
    let devices = device_rows().await?;
    let (_, fingerprint) = current_identity(&app)?;
    let device_id = devices
        .into_iter()
        .find(|row| {
            row.public_key_fingerprint
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(&fingerprint))
        })
        .map(|row| row.id);
    Ok(ReconcileStatus {
        reconciled: device_id.is_some(),
        device_id,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RevokeRequest<'a> {
    action: &'static str,
    device_id: &'a str,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevokeWire {
    device_id: String,
    authority_state: String,
}
#[tauri::command]
pub(crate) async fn broker_device_revoke(device_id: String) -> Result<RevokeResult, String> {
    if Uuid::parse_str(&device_id).is_err() {
        return Err(public_error("device_not_found"));
    }
    let response: RevokeWire = native_post(
        "/functions/v1/device-enrollment",
        RevokeRequest {
            action: "revoke",
            device_id: &device_id,
        },
    )
    .await?;
    if response.authority_state != "revoked" {
        return Err(public_error("authorization_denied"));
    }
    Ok(RevokeResult {
        device_id: response.device_id,
        status: "revoked",
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct AuditWire {
    id: String,
    event_type: String,
    created_at: String,
    device_id: Option<String>,
}
#[tauri::command]
pub(crate) async fn broker_device_audit_list() -> Result<Vec<AuditRow>, String> {
    let operation = account_begin_operation()?;
    let access = ensure_access_token().await?;
    let mut url = native_auth::configured_supabase_origin()?;
    url.set_path("/rest/v1/device_audit_events");
    url.set_query(Some(
        "select=id,event_type,created_at,device_id&order=created_at.desc&limit=100",
    ));
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| public_error("audit_unavailable"))?
        .get(url)
        .bearer_auth(access.as_str())
        .header("apikey", native_auth::configured_supabase_anon_key()?)
        .send()
        .await
        .map_err(|_| public_error("audit_unavailable"))?;
    if !response.status().is_success() {
        operation.check()?;
        return Err(public_error("audit_unavailable"));
    }
    let rows = response
        .json::<Vec<AuditWire>>()
        .await
        .map_err(|_| public_error("audit_unavailable"))?
        .into_iter()
        .map(|row| AuditRow {
            event_id: row.id,
            event_type: row.event_type,
            created_at: row.created_at,
            device_id: row.device_id,
        })
        .collect::<Vec<_>>();
    operation.check()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn callback_rejects_duplicate_or_extra_parameters() {
        let pending = PendingLogin {
            request_id: "r".into(),
            generation: 1,
            port: 43123,
            state: Zeroizing::new("s".into()),
            verifier: Zeroizing::new("v".into()),
            expires_at: SystemTime::now() + LOGIN_TTL,
            expires_at_ms: u64::MAX,
            callback: Arc::new(Mutex::new(None)),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        assert!(parse_callback(
            "http://127.0.0.1:43123/auth/callback?code=c&state=s&code=d",
            &pending
        )
        .is_err());
        assert!(parse_callback(
            "http://127.0.0.1:43123/auth/callback?code=c&state=s",
            &pending
        )
        .is_ok());
    }

    #[derive(Default)]
    struct FakeKeyring {
        slots: HashMap<String, Zeroizing<String>>,
        failure: Option<usize>,
        cleanup_failure: bool,
        step: usize,
        events: usize,
    }
    impl KeyringPort for FakeKeyring {
        fn read(&mut self, slot: &str) -> Result<Option<Zeroizing<String>>, String> {
            let failed = self.failure == Some(self.step);
            self.step += 1;
            if failed {
                Err(public_error("keyring_unavailable"))
            } else {
                Ok(self.slots.get(slot).cloned())
            }
        }
        fn write(&mut self, slot: &str, value: &Zeroizing<String>) -> Result<(), String> {
            let failed = self.failure == Some(self.step);
            self.step += 1;
            self.events += 1;
            if failed {
                Err(public_error("keyring_unavailable"))
            } else {
                self.slots.insert(slot.to_owned(), value.clone());
                Ok(())
            }
        }
        fn delete(&mut self, slot: &str) -> Result<(), String> {
            let failed = self.failure == Some(self.step);
            self.step += 1;
            self.events += 1;
            if self.cleanup_failure || failed {
                Err(if self.cleanup_failure {
                    public_error("cleanup_failed")
                } else {
                    public_error("keyring_unavailable")
                })
            } else {
                self.slots.remove(slot);
                Ok(())
            }
        }
        fn verify_absent(&mut self, slot: &str) -> Result<(), String> {
            let failed = self.failure == Some(self.step);
            self.step += 1;
            if self.cleanup_failure || failed {
                Err(if self.cleanup_failure {
                    public_error("cleanup_failed")
                } else {
                    public_error("keyring_unavailable")
                })
            } else if self.slots.contains_key(slot) {
                Err(public_error("keyring_unavailable"))
            } else {
                Ok(())
            }
        }
        fn inject_failure(&mut self, stage: usize) {
            self.failure = Some(stage);
            self.step = 0;
        }
        fn inject_cleanup_failure(&mut self) {
            self.cleanup_failure = true;
        }
        fn event_count(&self) -> usize {
            self.events
        }
    }
    #[derive(Default)]
    struct FakeClock {
        now: u64,
    }
    impl ClockPort for FakeClock {
        fn now_ms(&self) -> u64 {
            self.now
        }
    }
    #[derive(Default)]
    struct FakeListener {
        opened: bool,
    }
    impl ListenerCallbackPort for FakeListener {
        fn open(&mut self) -> Result<(), String> {
            self.opened = true;
            Ok(())
        }
        fn close(&mut self) {
            self.opened = false;
        }
        fn callback_target(&self, _request: &[u8], _port: u16) -> Option<Zeroizing<String>> {
            callback_from_request(_request, _port)
        }
    }
    #[derive(Default, Clone)]
    struct FakeProvider {
        failure: Option<&'static str>,
        calls: usize,
    }
    impl ProviderHttpPort for FakeProvider {
        fn exchange(
            &mut self,
            _code: Zeroizing<String>,
            _verifier: Zeroizing<String>,
        ) -> Result<LifecycleMaterial, String> {
            self.calls += 1;
            if let Some(error) = self.failure {
                Err(error.to_owned())
            } else {
                Ok(LifecycleMaterial {
                    access: Zeroizing::new("access".to_owned()),
                    refresh: Zeroizing::new("refresh".to_owned()),
                    user_id: None,
                    email: None,
                    access_expires_at_ms: Some(3_600_000),
                })
            }
        }
        fn refresh(&mut self, _refresh: Zeroizing<String>) -> Result<LifecycleMaterial, String> {
            self.calls += 1;
            if let Some(error) = self.failure {
                Err(error.to_owned())
            } else {
                Ok(LifecycleMaterial {
                    access: Zeroizing::new("access".to_owned()),
                    refresh: Zeroizing::new("refresh".to_owned()),
                    user_id: None,
                    email: None,
                    access_expires_at_ms: Some(3_600_000),
                })
            }
        }
        fn inject_failure(&mut self, code: &'static str) {
            self.failure = Some(code);
        }
        fn call_count(&self) -> usize {
            self.calls
        }
        fn drive_exchange(
            &self,
            _client_id: String,
            _redirect_uri: Zeroizing<String>,
            _code: Zeroizing<String>,
            _verifier: Zeroizing<String>,
        ) -> Result<DriveTokenMaterial, String> {
            Ok(DriveTokenMaterial {
                access: Zeroizing::new("drive-access".to_owned()),
                refresh: Some(Zeroizing::new("drive-refresh".to_owned())),
                scope: Some(Zeroizing::new(
                    "https://www.googleapis.com/auth/drive.appdata".to_owned(),
                )),
            })
        }
        fn drive_refresh(
            &self,
            _client_id: String,
            _refresh: Zeroizing<String>,
        ) -> Result<DriveTokenMaterial, String> {
            self.drive_exchange(
                String::new(),
                Zeroizing::new(String::new()),
                Zeroizing::new(String::new()),
                Zeroizing::new(String::new()),
            )
        }
    }
    impl DriveHttpPort for FakeProvider {}
    impl ArchiveJobPort for FakeProvider {}
    impl CommitObservationPort for FakeProvider {}
    fn make_broker() -> SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider> {
        SessionLifecycle::new(
            FakeKeyring::default(),
            FakeClock::default(),
            FakeListener::default(),
            FakeProvider::default(),
        )
    }

    fn pending_login(expired: bool) -> PendingLogin {
        PendingLogin {
            request_id: "behavioral".to_owned(),
            generation: 0,
            port: 0,
            state: Zeroizing::new("callback-state".to_owned()),
            verifier: Zeroizing::new("pkce-verifier".to_owned()),
            expires_at: if expired {
                SystemTime::UNIX_EPOCH
            } else {
                SystemTime::now() + LOGIN_TTL
            },
            expires_at_ms: if expired { 0 } else { u64::MAX },
            callback: Arc::new(Mutex::new(None)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn take_login(
        broker: &mut SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider>,
        expired: bool,
    ) -> Option<(PendingLogin, LifecycleTicket, FakeProvider)> {
        let generation = broker.generation();
        broker
            .registered_login_begin(pending_login(expired))
            .unwrap();
        broker.registered_login_take_for_exchange("behavioral", generation)
    }

    fn complete_login(
        broker: &mut SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider>,
        pending: PendingLogin,
        ticket: LifecycleTicket,
        mut provider: FakeProvider,
        callback: Result<Zeroizing<String>, &'static str>,
    ) -> Result<LifecycleOutcome, String> {
        let result = match callback {
            Ok(code) => provider.exchange(code, pending.verifier),
            Err(error) => Err(error.to_owned()),
        };
        broker.registered_login_complete(ticket, result)
    }

    fn refresh_once(
        broker: &mut SessionLifecycle<FakeKeyring, FakeClock, FakeListener, FakeProvider>,
    ) -> Result<Zeroizing<String>, String> {
        let admission = broker.begin_refresh()?;
        match admission {
            RefreshAdmission::Work {
                ticket,
                refresh,
                mut provider,
            } => {
                let result = provider.refresh(refresh);
                let (result, notify) = broker.finish_refresh(ticket, result);
                if let Some(notify) = notify {
                    let (lock, signal) = &*notify;
                    *lock.lock().unwrap() = true;
                    signal.notify_all();
                }
                result
            }
            RefreshAdmission::Ready(token) => Ok(token),
            RefreshAdmission::Wait(_) => Err(public_error("auth_refresh_in_progress")),
        }
    }

    #[test]
    fn native_behavioral_success_redacted_and_disposed() {
        let mut broker = make_broker();
        let (pending, ticket, provider) = take_login(&mut broker, false).unwrap();
        assert_eq!(
            complete_login(
                &mut broker,
                pending,
                ticket,
                provider,
                Ok(Zeroizing::new("code".to_owned()))
            )
            .unwrap()
            .state,
            "authenticated"
        );
        assert_eq!(broker.logout().unwrap().state, "signed_out");
        assert!(broker.disposed());
    }

    #[test]
    fn native_behavioral_startup_missing_and_restart() {
        let mut broker = make_broker();
        assert!(matches!(broker.begin_refresh(), Err(error) if error == "auth_required"));
        broker.seed_active("restart").unwrap();
        assert_eq!(refresh_once(&mut broker).unwrap().as_str(), "access");
        assert_eq!(broker.logout().unwrap().state, "signed_out");
    }

    #[test]
    fn native_behavioral_registered_listener_clock_and_lifecycle_entrypoints() {
        let mut broker = make_broker();
        let mut pending = pending_login(false);
        pending.port = 43123;
        let generation = broker.registered_login_begin(pending).unwrap();
        assert_eq!(generation, broker.generation());
        assert!(!broker.login_expired());
        let callback = broker.listener_callback_target(
            b"GET /auth/callback?code=code&state=callback-state HTTP/1.1\r\n\r\n",
            43123,
        );
        assert!(callback.is_some());
        assert_eq!(
            callback.unwrap().as_str(),
            "http://127.0.0.1:43123/auth/callback?code=code&state=callback-state"
        );
    }

    #[test]
    fn native_behavioral_registered_startup_recovery_both_domains_fault_matrix() {
        let cases = [
            ("staged_index", 1usize),
            ("slot", 8usize),
            ("marker", 6usize),
            ("old_slot_deletion", 9usize),
            ("compact_index", 11usize),
        ];
        for (ordering, stage) in cases {
            let mut broker = make_broker();
            broker.seed_active("account-restart").unwrap();
            broker
                .seed_drive_active("drive-restart", "drive-restart-token")
                .unwrap();
            if ordering == "old_slot_deletion" || ordering == "compact_index" {
                broker.keyring.slots.insert(
                    versioned_slot("drive-restart", 1),
                    Zeroizing::new("old-drive-token".to_owned()),
                );
                broker.keyring.slots.insert(
                    versioned_slot("drive-restart", 2),
                    Zeroizing::new("drive-restart-token".to_owned()),
                );
                broker.keyring.slots.insert(
                    index_slot("drive-restart"),
                    encode_index("drive-restart", &[1, 2]).unwrap(),
                );
                broker.keyring.slots.insert(
                    "drive-restart-marker".to_owned(),
                    encode_marker("drive-restart", 2, "drive-restart-token").unwrap(),
                );
            }
            broker.fail_keyring_at(stage);
            let result = broker.recover_startup();
            assert!(
                result.is_err(),
                "recovery ordering {ordering} unexpectedly succeeded"
            );
            assert_eq!(broker.state_name(), "credential_cleanup_failed");
            assert!(broker.begin_refresh().is_err());
            assert!(broker
                .begin_drive_operation("drive-restart".to_owned())
                .is_err());
        }

        let mut recovered = make_broker();
        recovered.seed_active("account-restart").unwrap();
        recovered
            .seed_drive_active("drive-restart", "drive-restart-token")
            .unwrap();
        recovered.recover_startup().unwrap();
        assert_eq!(recovered.state_name(), "signed_out");
    }

    #[test]
    fn native_behavioral_rotation_order_and_staged_failures() {
        let mut broker = make_broker();
        assert_eq!(
            broker
                .accept_material(LifecycleMaterial {
                    access: Zeroizing::new("access".to_owned()),
                    refresh: Zeroizing::new("new".to_owned()),
                    user_id: None,
                    email: None,
                    access_expires_at_ms: Some(3_600_000),
                })
                .unwrap()
                .state,
            "authenticated"
        );
        for failure in 0..10 {
            let mut failing = make_broker();
            failing.fail_keyring_at(failure);
            assert!(failing
                .accept_material(LifecycleMaterial {
                    access: Zeroizing::new("access".to_owned()),
                    refresh: Zeroizing::new("new".to_owned()),
                    user_id: None,
                    email: None,
                    access_expires_at_ms: Some(3_600_000),
                })
                .is_err());
        }
    }

    #[test]
    fn native_behavioral_refresh_single_flight() {
        let mut broker = make_broker();
        broker.seed_active("refresh").unwrap();
        let admission = broker.begin_refresh().unwrap();
        assert!(matches!(
            broker.begin_refresh().unwrap(),
            RefreshAdmission::Wait(_)
        ));
        if let RefreshAdmission::Work {
            ticket,
            refresh,
            mut provider,
        } = admission
        {
            let (result, notify) = broker.finish_refresh(ticket, provider.refresh(refresh));
            assert_eq!(result.unwrap().as_str(), "access");
            if let Some(notify) = notify {
                let (lock, signal) = &*notify;
                *lock.lock().unwrap() = true;
                signal.notify_all();
            }
        } else {
            panic!("first refresh did not own the flight");
        }
    }

    #[test]
    fn native_behavioral_denial_before_provider_effect() {
        let mut broker = make_broker();
        broker.quiescing = true;
        assert!(matches!(
            broker.begin_account_operation(),
            Err(error) if error == "auth_transition_in_progress"
        ));
        assert_eq!(broker.provider_calls(), 0);
        assert_eq!(broker.keyring_events(), 0);
    }

    #[test]
    fn native_behavioral_malformed_callback() {
        let mut broker = make_broker();
        let (pending, ticket, provider) = take_login(&mut broker, false).unwrap();
        assert_eq!(
            complete_login(
                &mut broker,
                pending,
                ticket,
                provider,
                Err("malformed_callback")
            )
            .unwrap_err(),
            "malformed_callback"
        );
    }

    #[test]
    fn native_behavioral_timeout() {
        let mut broker = make_broker();
        assert!(take_login(&mut broker, true).is_none());
    }

    #[test]
    fn native_behavioral_cancel() {
        let mut broker = make_broker();
        broker.begin_login(pending_login(false)).unwrap();
        broker.cancel_login("behavioral").unwrap();
        assert!(broker.disposed());
    }

    #[test]
    fn native_behavioral_exchange_failure() {
        let mut broker = make_broker();
        broker.fail_provider_with("exchange_failed");
        let (pending, ticket, provider) = take_login(&mut broker, false).unwrap();
        assert_eq!(
            complete_login(
                &mut broker,
                pending,
                ticket,
                provider,
                Ok(Zeroizing::new("code".to_owned()))
            )
            .unwrap_err(),
            "exchange_failed"
        );
    }

    #[test]
    fn native_behavioral_logout() {
        let mut broker = make_broker();
        broker.seed_active("logout").unwrap();
        broker.begin_login(pending_login(false)).unwrap();
        assert_eq!(broker.logout().unwrap().state, "signed_out");
    }

    #[test]
    fn native_behavioral_shutdown() {
        let mut broker = make_broker();
        broker.begin_login(pending_login(false)).unwrap();
        assert_eq!(broker.shutdown().unwrap().state, "shutdown");
    }

    #[test]
    fn native_behavioral_cleanup_failure() {
        let mut broker = make_broker();
        broker.seed_active("cleanup").unwrap();
        broker.fail_cleanup();
        assert_eq!(broker.shutdown().unwrap_err(), "cleanup_failed");
        assert_eq!(broker.state_name(), "credential_cleanup_failed");
    }

    #[test]
    fn native_behavioral_stale_generation() {
        let mut broker = make_broker();
        let (pending, ticket, provider) = take_login(&mut broker, false).unwrap();
        broker.invalidate_generation();
        assert_eq!(
            complete_login(
                &mut broker,
                pending,
                ticket,
                provider,
                Ok(Zeroizing::new("code".to_owned()))
            )
            .unwrap_err(),
            "auth_transition_in_progress"
        );
    }

    #[test]
    fn native_behavioral_drive_disconnect_wins_against_stale_commit() {
        let mut broker = make_broker();
        let ticket = broker
            .begin_drive_operation("drive-test".to_owned())
            .unwrap();
        broker.drive.quiescing = true;
        assert_eq!(
            broker
                .begin_drive_operation("drive-test-2".to_owned())
                .unwrap_err(),
            "auth_transition_in_progress"
        );
        broker.drive_drain.release();
        broker.finish_drive_operation(ticket);
        broker.disconnect_drive().unwrap();
        assert_eq!(
            broker.ensure_drive_ticket(ticket).unwrap_err(),
            "drive_transition_in_progress"
        );
    }

    #[test]
    fn native_behavioral_drive_transition_drains_without_deadlock() {
        let broker = Arc::new(Mutex::new(make_broker()));
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let worker_broker = broker.clone();
        let worker = std::thread::spawn(move || {
            let (ticket, drain) = {
                let mut broker = worker_broker.lock().unwrap();
                let ticket = broker
                    .begin_drive_operation("drive-race".to_owned())
                    .unwrap();
                (ticket, broker.drive_drain.clone())
            };
            ready_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            drain.release();
            worker_broker.lock().unwrap().finish_drive_operation(ticket);
        });
        ready_rx.recv().unwrap();
        let transition_broker = broker.clone();
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(0);
        let transition = std::thread::spawn(move || {
            let drain = transition_broker.lock().unwrap().begin_drive_disconnect();
            drain.wait_empty();
            transition_broker
                .lock()
                .unwrap()
                .finish_drive_disconnect()
                .unwrap();
            done_tx.send(()).unwrap();
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        release_tx.send(()).unwrap();
        done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        transition.join().unwrap();
        worker.join().unwrap();
        let mut broker = broker.lock().unwrap();
        assert!(matches!(
            broker.begin_drive_operation("after-quiesce".to_owned()),
            Ok(_)
        ));
    }

    #[test]
    fn native_behavioral_drive_marker_failure_compensates_before_publish() {
        let mut broker = make_broker();
        let ticket = broker
            .begin_drive_operation("drive-fault".to_owned())
            .unwrap();
        broker.fail_keyring_at(7);
        assert_eq!(
            broker
                .drive_commit(ticket, &Zeroizing::new("drive-refresh".to_owned()))
                .unwrap_err(),
            "drive_token_storage_failed"
        );
        broker.drive_drain.release();
        broker.finish_drive_operation(ticket);
        assert!(
            load_committed(&mut broker.keyring, "drive-fault", "drive-fault-marker")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn native_behavioral_no_entry_is_absence_but_corrupt_marker_is_not() {
        let mut broker = make_broker();
        assert!(matches!(broker.begin_refresh(), Err(error) if error == "auth_required"));
        broker
            .keyring
            .write(ACCOUNT_MARKER, &Zeroizing::new("broken".to_owned()))
            .unwrap();
        assert!(matches!(broker.begin_refresh(), Err(error) if error == "keyring_unavailable"));
    }

    #[test]
    fn native_behavioral_marker_content_integrity_is_verified() {
        let mut broker = make_broker();
        broker.seed_active("old").unwrap();
        broker.keyring.slots.insert(
            versioned_slot(ACCOUNT_DOMAIN, 1),
            Zeroizing::new("tampered".to_owned()),
        );
        assert!(matches!(broker.begin_refresh(), Err(error) if error == "keyring_unavailable"));
    }

    #[test]
    fn native_behavioral_registry_integrity_is_verified() {
        let mut broker = make_broker();
        broker.seed_active("old").unwrap();
        broker.keyring.slots.insert(
            ACCOUNT_INDEX.to_owned(),
            Zeroizing::new("{\"versions\":[1]}".to_owned()),
        );
        assert!(matches!(broker.begin_refresh(), Err(error) if error == "keyring_unavailable"));
    }

    #[test]
    fn native_behavioral_pre_marker_failure_preserves_previous_authority() {
        let mut broker = make_broker();
        broker.seed_active("old").unwrap();
        broker.fail_keyring_at(6);
        assert!(broker
            .accept_material(LifecycleMaterial {
                access: Zeroizing::new("access".to_owned()),
                refresh: Zeroizing::new("new".to_owned()),
                user_id: None,
                email: None,
                access_expires_at_ms: Some(1),
            })
            .is_err());
        assert_eq!(
            load_committed(&mut broker.keyring, ACCOUNT_DOMAIN, ACCOUNT_MARKER)
                .unwrap()
                .unwrap()
                .as_str(),
            "old"
        );
    }

    #[test]
    fn native_behavioral_registry_enumerates_orphans_before_access() {
        let mut broker = make_broker();
        broker.seed_active("old").unwrap();
        broker
            .keyring
            .write(
                &index_slot(ACCOUNT_DOMAIN),
                &encode_index(ACCOUNT_DOMAIN, &[1, 99]).unwrap(),
            )
            .unwrap();
        broker
            .keyring
            .write(
                &versioned_slot(ACCOUNT_DOMAIN, 99),
                &Zeroizing::new("orphan".to_owned()),
            )
            .unwrap();
        assert_eq!(refresh_once(&mut broker).unwrap().as_str(), "access");
        assert!(!broker
            .keyring
            .slots
            .contains_key(&versioned_slot(ACCOUNT_DOMAIN, 99)));
    }

    #[test]
    fn native_behavioral_post_marker_cleanup_failure_is_terminal() {
        let mut broker = make_broker();
        broker.fail_keyring_at(9);
        let result = broker.accept_material(LifecycleMaterial {
            access: Zeroizing::new("access".to_owned()),
            refresh: Zeroizing::new("refresh".to_owned()),
            user_id: None,
            email: None,
            access_expires_at_ms: Some(1),
        });
        assert_eq!(result.unwrap_err(), "cleanup_failed");
        assert_eq!(broker.state_name(), "credential_cleanup_failed");
        assert!(broker.account.access_token.is_none());
    }

    #[test]
    fn native_behavioral_login_completion_loses_to_logout() {
        let mut broker = make_broker();
        let (pending, ticket, provider) = take_login(&mut broker, false).unwrap();
        broker.logout().unwrap();
        assert_eq!(
            complete_login(
                &mut broker,
                pending,
                ticket,
                provider,
                Ok(Zeroizing::new("code".to_owned()))
            )
            .unwrap_err(),
            "auth_transition_in_progress"
        );
        assert!(broker.disposed());
    }
}
