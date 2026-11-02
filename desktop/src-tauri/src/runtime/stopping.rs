use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::exec::run_capture;
use super::manager::Manager;
use super::process::{send_signal, ProcessTable};
use super::port::listening_pids;
use super::readiness::{probe, read_supervisor_pid, Verdict};
use crate::contracts::{BenchTarget, OperationResult, Status};
use crate::error::{AppError, AppResult, ErrorCategory};

const SIGTERM: i32 = libc::SIGTERM;
const SIGKILL: i32 = libc::SIGKILL;
const PILOT_STOP_TIMEOUT: Duration = Duration::from_secs(45);

impl Manager {
    /// Stops only what this app owns, or an attached bench after explicit confirmation and a task check.
    pub fn stop(self: &Arc<Self>, name: &str, confirm_attached: bool) -> AppResult<OperationResult> {
        let _guard = self.exclusive(name)?;
        let target = self.find(name)?;
        let outcome = self.stop_checked(&target, confirm_attached);
        self.with_state(name, |state| {
            state.transition = None;
            if let Err(error) = &outcome {
                state.last_error = Some(error.clone());
            }
        });
        self.notify();
        outcome.map(|(message, notices)| {
            let mut result = OperationResult::success(message, Some(self.summarize(&target)));
            result.notices = notices;
            result
        })
    }

    pub fn restart(self: &Arc<Self>, name: &str, confirm_attached: bool) -> AppResult<OperationResult> {
        let target = self.find(name)?;
        if matches!(probe(&target, None)?, Verdict::Running { .. } | Verdict::Starting { .. }) {
            self.stop(name, confirm_attached)?;
        }
        self.start(name)
    }

    fn stop_checked(&self, target: &BenchTarget, confirm_attached: bool) -> AppResult<(String, Vec<String>)> {
        let owned = self.with_state(&target.name, |state| state.owned.clone());
        let table = ProcessTable::snapshot()?;
        let owned_pid = owned.as_ref().filter(|o| table.matches(&o.identity)).map(|o| o.identity.pid);
        let verdict = probe(target, owned_pid)?;
        let anchor = match verdict {
            Verdict::Free => return Ok((format!("'{}' is already stopped.", target.name), Vec::new())),
            Verdict::Conflict(message) => {
                return Err(AppError::new(ErrorCategory::Conflict, format!("{message} Pilot Desktop will not stop it.")).stage("identity"));
            }
            Verdict::Starting { pid } => Some(pid),
            Verdict::Running { anchor } => anchor,
        };
        if owned_pid.is_none() && !confirm_attached {
            return Err(AppError::new(
                ErrorCategory::ConfirmationRequired,
                "This bench was already running when Pilot Desktop opened. Stopping it ends its web, worker and scheduler processes for every tool using it.",
            )
            .stage("confirmation"));
        }
        self.ensure_no_active_work(target)?;
        self.with_state(&target.name, |state| state.transition = Some(Status::Stopping));
        self.notify();
        match owned_pid {
            Some(pid) => self.stop_owned(target, pid, &owned),
            None => self.stop_attached(target, anchor, &table),
        }
    }

    fn ensure_no_active_work(&self, target: &BenchTarget) -> AppResult<()> {
        let activity = self.adapter()?.activity(target).map_err(|error| {
            AppError::new(ErrorCategory::UnsafeToStop, "Pilot Desktop could not check for running tasks, so it did not stop the bench.")
                .stage("safety")
                .details(&error.message)
        })?;
        if activity.is_unsafe() {
            return Err(AppError::new(ErrorCategory::UnsafeToStop, format!("{} Stopping now could interrupt a migration, restore or setup.", activity.describe()))
                .stage("safety")
                .recovery("Wait for the work to finish in the Pilot dashboard, then stop the bench."));
        }
        Ok(())
    }

    fn stop_owned(&self, target: &BenchTarget, pid: i32, owned: &Option<super::manager::OwnedProcess>) -> AppResult<(String, Vec<String>)> {
        let identity = owned.as_ref().expect("owned process").identity.clone();
        send_signal(pid, SIGTERM);
        let deadline = Instant::now() + self.timeouts.stop_grace;
        while Instant::now() < deadline {
            if !ProcessTable::snapshot()?.matches(&identity) {
                self.wait_for_port_release(target, deadline);
                return Ok(self.finish_stop(target, Vec::new()));
            }
            thread::sleep(Duration::from_millis(100));
        }
        let table = ProcessTable::snapshot()?;
        let mut victims = table.descendants(pid);
        victims.push(pid);
        let affected = victims.len();
        for victim in &victims {
            send_signal(*victim, SIGKILL);
        }
        self.wait_for_port_release(target, Instant::now() + Duration::from_secs(2));
        let notice = format!(
            "Graceful shutdown timed out, so {affected} process(es) started by '{}' were force-stopped. In-flight requests and background jobs may have been cut short.",
            target.name
        );
        Ok(self.finish_stop(target, vec![notice]))
    }

    fn stop_attached(&self, target: &BenchTarget, anchor: Option<i32>, table: &ProcessTable) -> AppResult<(String, Vec<String>)> {
        let supervisor = read_supervisor_pid(&target.path, table);
        if supervisor.is_some() {
            let mut command = self.adapter()?.stop_command(target);
            let result = run_capture(&mut command, PILOT_STOP_TIMEOUT)?;
            self.logs.push(&target.name, "launcher", &format!("pilot stop exited with code {}.", result.status));
        } else if let Some(pid) = anchor {
            send_signal(pid, SIGTERM);
        }
        let deadline = Instant::now() + self.timeouts.stop_grace;
        while Instant::now() < deadline {
            if matches!(probe(target, None)?, Verdict::Free) {
                return Ok(self.finish_stop(target, Vec::new()));
            }
            thread::sleep(Duration::from_millis(150));
        }
        Err(AppError::new(ErrorCategory::StartupFailed, "The attached bench did not stop within the grace period.")
            .stage("stop")
            .recovery("Pilot Desktop never force-stops a bench it did not start. Stop it from its own terminal."))
    }

    /// A draining child can hold the admin port briefly after its supervisor is gone.
    fn wait_for_port_release(&self, target: &BenchTarget, deadline: Instant) {
        while Instant::now() < deadline && listening_pids(target.admin_port).is_ok_and(|pids| !pids.is_empty()) {
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn finish_stop(&self, target: &BenchTarget, notices: Vec<String>) -> (String, Vec<String>) {
        self.with_state(&target.name, |state| {
            state.owned = None;
            state.last_error = None;
        });
        self.persist_owned();
        (format!("Stopped '{}'.", target.name), notices)
    }

    /// Graceful stop of an owned process after a failed or timed-out start.
    pub(super) fn terminate_owned(&self, name: &str) {
        let owned = self.with_state(name, |state| state.owned.clone());
        let Some(owned) = owned else { return };
        let matches = |table: &ProcessTable| table.matches(&owned.identity);
        if !ProcessTable::snapshot().is_ok_and(|table| matches(&table)) {
            return;
        }
        send_signal(owned.identity.pid, SIGTERM);
        let deadline = Instant::now() + self.timeouts.stop_grace;
        while Instant::now() < deadline && ProcessTable::snapshot().is_ok_and(|table| matches(&table)) {
            thread::sleep(Duration::from_millis(100));
        }
        if let Ok(table) = ProcessTable::snapshot() {
            if matches(&table) {
                for victim in table.descendants(owned.identity.pid).into_iter().chain([owned.identity.pid]) {
                    send_signal(victim, SIGKILL);
                }
            }
        }
        self.with_state(name, |state| state.owned = None);
        self.persist_owned();
    }

    /// Quit path: stop every owned bench, never an attached one. Any unsafe bench aborts the quit.
    pub fn shutdown_owned(self: &Arc<Self>) -> AppResult<Vec<String>> {
        let mut notices = Vec::new();
        for name in self.owned_benches() {
            let result = self.stop(&name, false)?;
            notices.extend(result.notices);
        }
        Ok(notices)
    }
}
