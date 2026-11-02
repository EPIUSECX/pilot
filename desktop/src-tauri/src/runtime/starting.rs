use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::os::unix::process::CommandExt;

use super::diagnosis::diagnose;
use super::manager::{base_summary, unsupported_reason, Manager, OwnedProcess};
use super::output::{extract_setup_url, pump};
use super::process::ProcessTable;
use super::readiness::{probe, Verdict};
use crate::contracts::{BenchTarget, OperationResult, Ownership, Status};
use crate::error::{AppError, AppResult, ErrorCategory};

impl Manager {
    /// Attach to a matching running bench, otherwise start one and wait for verified readiness.
    pub fn start(self: &Arc<Self>, name: &str) -> AppResult<OperationResult> {
        let _guard = self.exclusive(name)?;
        let target = self.find(name)?;
        if let Some(reason) = unsupported_reason(&target) {
            return Err(AppError::new(ErrorCategory::NotConfigured, reason).stage("preflight"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.with_state(name, |state| {
            state.transition = Some(Status::Starting);
            state.last_error = None;
            state.cancel = cancel.clone();
        });
        self.notify();
        let outcome = self.start_verified(&target, &cancel);
        self.with_state(name, |state| {
            state.transition = None;
            if let Err(error) = &outcome {
                state.last_error = Some(error.clone());
            }
        });
        self.notify();
        outcome.map(|message| OperationResult::success(message, Some(self.summarize(&target))))
    }

    pub fn cancel_start(&self, name: &str) {
        self.with_state(name, |state| state.cancel.store(true, Ordering::SeqCst));
    }

    fn start_verified(self: &Arc<Self>, target: &BenchTarget, cancel: &Arc<AtomicBool>) -> AppResult<String> {
        match probe(target, None)? {
            Verdict::Conflict(message) => {
                return Err(AppError::new(ErrorCategory::Conflict, message)
                    .stage("identity")
                    .recovery("Stop the other program or change this bench's admin port, then try again."));
            }
            Verdict::Running { .. } => return Ok(format!("Attached to the running bench '{}'.", target.name)),
            Verdict::Starting { .. } => {
                self.wait_until_ready(target, None, cancel)?;
                return Ok(format!("Attached to '{}' once it finished starting.", target.name));
            }
            Verdict::Free => {}
        }
        let child = self.spawn(target)?;
        let pid = child.id() as i32;
        let exit = Arc::new(Exit::default());
        self.watch(target, child, exit.clone());
        match self.wait_until_ready(target, Some((pid, exit)), cancel) {
            Ok(()) => Ok(format!("Started '{}'.", target.name)),
            Err(error) => {
                self.terminate_owned(&target.name);
                Err(error)
            }
        }
    }

    fn spawn(self: &Arc<Self>, target: &BenchTarget) -> AppResult<Child> {
        let mut command = self.adapter()?.start_command(target);
        let arguments = command.get_args().map(|argument| argument.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ");
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
        let child = command.spawn().map_err(|error| {
            AppError::new(ErrorCategory::EnvironmentMissing, format!("Could not launch Pilot: {error}"))
                .stage("launch")
                .recovery("Check the Pilot installation in Preferences.")
        })?;
        let pid = child.id() as i32;
        let identity = (0..20).find_map(|_| {
            ProcessTable::snapshot().ok().and_then(|table| table.identity_of(pid, &arguments)).or_else(|| {
                thread::sleep(Duration::from_millis(25));
                None
            })
        });
        let Some(identity) = identity else {
            return Err(AppError::new(ErrorCategory::StartupFailed, "Pilot exited before it could be registered.").stage("launch"));
        };
        self.with_state(&target.name, |state| {
            state.owned = Some(OwnedProcess { identity, bench: target.name.clone() });
        });
        self.persist_owned();
        self.logs.push(&target.name, "launcher", &format!("Started Pilot (pid {pid})."));
        Ok(child)
    }

    fn watch(self: &Arc<Self>, target: &BenchTarget, mut child: Child, exit: Arc<Exit>) {
        let name = target.name.clone();
        let path = target.path.clone();
        let port = target.admin_port;
        for (stream, reader) in [("stdout", child.stdout.take().map(Reader::Out)), ("stderr", child.stderr.take().map(Reader::Err))] {
            let Some(reader) = reader else { continue };
            let (manager, name) = (self.clone(), name.clone());
            thread::spawn(move || {
                let hook = |raw: &str| {
                    if let Some(url) = extract_setup_url(raw, port) {
                        manager.with_state(&name, |state| state.setup_url = Some(url));
                    }
                };
                match reader {
                    Reader::Out(out) => pump(out, &name, stream, &manager.logs, hook),
                    Reader::Err(err) => pump(err, &name, stream, &manager.logs, hook),
                }
            });
        }
        let manager = self.clone();
        thread::spawn(move || {
            let code = child.wait().ok().and_then(|status| status.code()).unwrap_or(-1);
            exit.code.store(code as i64, Ordering::SeqCst);
            exit.done.store(true, Ordering::SeqCst);
            manager.record_exit(&name, &path, code);
        });
    }

    fn record_exit(&self, name: &str, path: &Path, code: i32) {
        let recent: Vec<String> = self.logs.recent(name).into_iter().rev().take(200).map(|line| line.text).collect();
        let recent: Vec<&str> = recent.iter().map(String::as_str).collect();
        let diagnosis = diagnose(&recent, path);
        let unexpected = self.with_state(name, |state| {
            let was_running = state.transition.is_none() && state.owned.is_some();
            if state.transition != Some(Status::Stopping) {
                state.owned = None;
            }
            if was_running {
                let (message, recovery) = match &diagnosis {
                    Some(found) => (found.message.clone(), found.recovery.clone()),
                    None => (format!("Pilot exited unexpectedly (code {code})."), "Open the logs, fix the cause, then start the bench again.".to_string()),
                };
                state.last_error = Some(AppError::new(ErrorCategory::StartupFailed, message).stage("runtime").recovery(&recovery));
            }
            was_running
        });
        self.persist_owned();
        self.logs.push(name, "launcher", &format!("Pilot exited with code {code}."));
        if unexpected {
            let reason = diagnosis.map_or_else(|| format!("Pilot exited unexpectedly (code {code})."), |found| found.message);
            self.report_failure(name, &reason);
            self.notify();
        }
    }

    fn wait_until_ready(&self, target: &BenchTarget, owned: Option<(i32, Arc<Exit>)>, cancel: &Arc<AtomicBool>) -> AppResult<()> {
        let deadline = Instant::now() + self.timeouts.start;
        let spawned = owned.as_ref().map(|(pid, _)| *pid);
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::new(ErrorCategory::StartupFailed, "Start was cancelled.").stage("readiness"));
            }
            if let Some((_, exit)) = &owned {
                if exit.done.load(Ordering::SeqCst) {
                    return Err(self.startup_failure(target, exit.code.load(Ordering::SeqCst)));
                }
            }
            match probe(target, spawned)? {
                Verdict::Running { .. } => return Ok(()),
                Verdict::Conflict(message) => {
                    return Err(AppError::new(ErrorCategory::Conflict, message).stage("identity"));
                }
                Verdict::Free | Verdict::Starting { .. } => {}
            }
            if Instant::now() >= deadline {
                return Err(AppError::new(ErrorCategory::Timeout, format!("'{}' did not become ready in time.", target.name))
                    .stage("readiness")
                    .recovery("Open the logs to see where startup stalled, then start again.")
                    .details(&self.tail(&target.name)));
            }
            thread::sleep(self.timeouts.poll);
        }
    }

    fn startup_failure(&self, target: &BenchTarget, code: i64) -> AppError {
        AppError::new(ErrorCategory::StartupFailed, format!("Pilot stopped while starting '{}' (code {code}).", target.name))
            .stage("startup")
            .recovery("Open the logs for the error, fix it, then try again.")
            .details(&self.tail(&target.name))
    }

    pub(super) fn tail(&self, name: &str) -> String {
        let lines = self.logs.recent(name);
        let start = lines.len().saturating_sub(8);
        lines[start..].iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n")
    }

    pub fn ownership_of(&self, summary_name: &str) -> Ownership {
        let owned = self.with_state(summary_name, |state| state.owned.clone());
        match (owned, ProcessTable::snapshot()) {
            (Some(owned), Ok(table)) if table.matches(&owned.identity) => Ownership::Owned,
            _ => Ownership::None,
        }
    }

    pub fn summary_for(&self, name: &str) -> AppResult<crate::contracts::BenchSummary> {
        let target = self.find(name)?;
        Ok(self.summarize(&target))
    }

    pub fn placeholder(target: &BenchTarget) -> crate::contracts::BenchSummary {
        base_summary(target, Status::Unknown, Ownership::None, None)
    }
}

#[derive(Default)]
pub struct Exit {
    pub done: AtomicBool,
    pub code: std::sync::atomic::AtomicI64,
}

enum Reader {
    Out(std::process::ChildStdout),
    Err(std::process::ChildStderr),
}
