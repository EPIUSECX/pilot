use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::adapter::PilotAdapter;
use super::logs::LogBuffer;
use super::persistence::atomic_write;
use super::port::is_exposed_to_network;
use super::process::{ProcessIdentity, ProcessTable};
use super::readiness::{probe_in, Verdict};
use crate::contracts::{BenchSummary, BenchTarget, Ownership, Status};
use crate::error::{AppError, AppResult, ErrorCategory};

const NETWORK_EXPOSED: &str = "This bench's sign-in page can be reached from other devices on your network. Update Pilot so it listens on this Mac only, and keep the admin password private.";

pub trait AdapterSource: Send + Sync {
    fn current(&self) -> AppResult<Arc<dyn PilotAdapter>>;
}

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    pub start: Duration,
    pub stop_grace: Duration,
    pub poll: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self { start: Duration::from_secs(180), stop_grace: Duration::from_secs(25), poll: Duration::from_millis(400) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OwnedProcess {
    pub identity: ProcessIdentity,
    #[serde(default)]
    pub bench: String,
}

#[derive(Default)]
pub struct BenchState {
    pub transition: Option<Status>,
    pub owned: Option<OwnedProcess>,
    pub last_error: Option<AppError>,
    pub setup_url: Option<String>,
    pub cancel: Arc<AtomicBool>,
}

pub struct Manager {
    pub(super) source: Arc<dyn AdapterSource>,
    pub logs: Arc<LogBuffer>,
    pub(super) states: Mutex<HashMap<String, BenchState>>,
    busy: Arc<Mutex<HashSet<String>>>,
    store: PathBuf,
    pub timeouts: Timeouts,
    on_change: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    on_failure: Mutex<Option<Arc<dyn Fn(&str, &str) + Send + Sync>>>,
}

impl Manager {
    pub fn new(source: Arc<dyn AdapterSource>, logs: Arc<LogBuffer>, store: PathBuf, timeouts: Timeouts) -> Arc<Self> {
        Arc::new(Self {
            source,
            logs,
            states: Mutex::new(HashMap::new()),
            busy: Arc::new(Mutex::new(HashSet::new())),
            store,
            timeouts,
            on_change: Mutex::new(None),
            on_failure: Mutex::new(None),
        })
    }

    pub fn set_on_change(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        *self.on_change.lock().expect("callback lock") = Some(callback);
    }

    pub fn set_on_failure(&self, callback: Arc<dyn Fn(&str, &str) + Send + Sync>) {
        *self.on_failure.lock().expect("callback lock") = Some(callback);
    }

    pub(super) fn report_failure(&self, name: &str, message: &str) {
        let callback = self.on_failure.lock().expect("callback lock").clone();
        if let Some(callback) = callback {
            callback(name, message);
        }
    }

    pub fn notify_changed(&self) {
        self.notify();
    }

    pub(super) fn notify(&self) {
        let callback = self.on_change.lock().expect("callback lock").clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub(super) fn adapter(&self) -> AppResult<Arc<dyn PilotAdapter>> {
        self.source.current()
    }

    pub fn find(&self, name: &str) -> AppResult<BenchTarget> {
        self.adapter()?
            .list()?
            .into_iter()
            .find(|target| target.name == name)
            .ok_or_else(|| AppError::new(ErrorCategory::NotFound, format!("No bench named '{name}'.")))
    }

    /// Serializes lifecycle operations per key. A second request fails fast instead of queueing.
    pub(super) fn exclusive(&self, key: &str) -> AppResult<OperationGuard> {
        if !self.busy.lock().expect("busy").insert(key.to_string()) {
            return Err(AppError::new(ErrorCategory::Busy, "Another operation is already running for this bench."));
        }
        Ok(OperationGuard { key: key.to_string(), busy: self.busy.clone() })
    }

    pub(super) fn with_state<T>(&self, name: &str, action: impl FnOnce(&mut BenchState) -> T) -> T {
        let mut states = self.states.lock().expect("states");
        action(states.entry(name.to_string()).or_default())
    }

    pub fn take_setup_url(&self, name: &str) -> Option<String> {
        self.with_state(name, |state| state.setup_url.take())
    }

    pub fn summaries(&self) -> AppResult<Vec<BenchSummary>> {
        let targets = self.adapter()?.list()?;
        let table = ProcessTable::snapshot()?;
        Ok(targets.iter().map(|target| self.summarize_in(target, &table)).collect())
    }

    pub fn summarize(&self, target: &BenchTarget) -> BenchSummary {
        match ProcessTable::snapshot() {
            Ok(table) => self.summarize_in(target, &table),
            Err(error) => base_summary(target, Status::Unknown, Ownership::None, Some(error)),
        }
    }

    fn summarize_in(&self, target: &BenchTarget, table: &ProcessTable) -> BenchSummary {
        let (transition, owned, last_error) =
            self.with_state(&target.name, |state| (state.transition, state.owned.clone(), state.last_error.clone()));
        let mut summary = base_summary(target, Status::Unknown, Ownership::None, last_error.clone());
        if let Some(reason) = unsupported_reason(target) {
            summary.notes.push(reason);
            summary.status = if target.read_error.is_some() { Status::Unknown } else { Status::Stopped };
            return summary;
        }
        if let Some(status) = transition {
            summary.status = status;
            summary.ownership = if owned.is_some() { Ownership::Owned } else { Ownership::None };
            return summary;
        }
        let owned_pid = owned.as_ref().filter(|o| table.matches(&o.identity)).map(|o| o.identity.pid);
        match probe_in(target, owned_pid, table) {
            Ok(Verdict::Free) => {
                summary.status = if last_error.is_some() { Status::Failed } else { Status::Stopped };
            }
            Ok(Verdict::Starting { .. }) => {
                summary.status = Status::Starting;
                summary.ownership = if owned_pid.is_some() { Ownership::Owned } else { Ownership::Attached };
            }
            Ok(Verdict::Running { .. }) => {
                summary.status = Status::Running;
                summary.ownership = if owned_pid.is_some() { Ownership::Owned } else { Ownership::Attached };
                summary.dashboard_url = Some(format!("http://127.0.0.1:{}", target.admin_port));
                summary.last_error = None;
                if is_exposed_to_network(target.admin_port) {
                    summary.notes.push(NETWORK_EXPOSED.to_string());
                }
            }
            Ok(Verdict::Conflict(message)) => {
                summary.status = Status::Failed;
                summary.last_error = Some(AppError::new(ErrorCategory::Conflict, message).stage("identity"));
            }
            Err(error) => summary.last_error = Some(error),
        }
        summary
    }

    pub fn owned_benches(&self) -> Vec<String> {
        let Ok(table) = ProcessTable::snapshot() else { return Vec::new() };
        let states = self.states.lock().expect("states");
        let mut names: Vec<String> = states
            .iter()
            .filter(|(_, state)| state.owned.as_ref().is_some_and(|owned| table.matches(&owned.identity)))
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        names
    }

    pub(super) fn persist_owned(&self) {
        let records: Vec<OwnedProcess> = {
            let states = self.states.lock().expect("states");
            states
                .iter()
                .filter_map(|(name, state)| state.owned.clone().map(|owned| OwnedProcess { bench: name.clone(), ..owned }))
                .collect()
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&records) {
            let _ = atomic_write(&self.store, &bytes);
        }
    }

    /// Saved records name processes of a previous run. They are never claimed or killed: a survivor
    /// is reported as an attached runtime by the live probe.
    pub fn reconcile_on_launch(&self) -> Vec<String> {
        let leftovers: Vec<OwnedProcess> = std::fs::read(&self.store)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let _ = std::fs::remove_file(&self.store);
        let table = ProcessTable::snapshot().unwrap_or_default();
        leftovers.into_iter().filter(|record| table.matches(&record.identity)).map(|record| record.bench).collect()
    }
}

pub struct OperationGuard {
    key: String,
    busy: Arc<Mutex<HashSet<String>>>,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        self.busy.lock().expect("busy").remove(&self.key);
    }
}

pub fn unsupported_reason(target: &BenchTarget) -> Option<String> {
    if let Some(error) = &target.read_error {
        return Some(format!("bench.toml could not be read: {error}"));
    }
    if target.is_production {
        return Some("Production benches run under system services. Pilot Desktop does not start or stop them.".into());
    }
    if !target.admin_enabled {
        return Some("The Pilot admin is disabled for this bench, so there is no dashboard to open.".into());
    }
    None
}

pub fn base_summary(target: &BenchTarget, status: Status, ownership: Ownership, error: Option<AppError>) -> BenchSummary {
    BenchSummary {
        id: target.name.clone(),
        name: target.name.clone(),
        path: target.path.display().to_string(),
        status,
        ownership,
        dashboard_url: None,
        frappe_branch: target.frappe_branch.clone(),
        is_production: target.is_production,
        is_initialized: target.is_initialized,
        admin_enabled: target.admin_enabled,
        last_error: error,
        notes: Vec::new(),
    }
}
