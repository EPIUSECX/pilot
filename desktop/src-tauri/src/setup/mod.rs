pub mod askpass;
pub mod executor;
pub mod failure;
pub mod folder;
pub mod machine;
pub mod progress;
pub mod runner;
pub mod steps;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{AppError, AppResult, ErrorCategory};
use progress::{Phase, SetupProgress, StepProgress, StepStatus};
use steps::Step;

type Listener = Arc<dyn Fn(&SetupProgress) + Send + Sync>;

/// One setup at a time. Owns the progress the launcher shows and the cancel flag the running step watches.
pub struct SetupSession {
    progress: Mutex<SetupProgress>,
    listener: Mutex<Option<Listener>>,
    cancel: Arc<AtomicBool>,
}

impl Default for SetupSession {
    fn default() -> Self {
        Self { progress: Mutex::new(SetupProgress::idle()), listener: Mutex::new(None), cancel: Arc::new(AtomicBool::new(false)) }
    }
}

impl SetupSession {
    pub fn set_listener(&self, listener: Listener) {
        *self.listener.lock().expect("listener") = Some(listener);
    }

    pub fn snapshot(&self) -> SetupProgress {
        self.progress.lock().expect("progress").clone()
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn begin(&self, steps: &[Step]) -> AppResult<()> {
        let mut progress = self.progress.lock().expect("progress");
        if progress.phase == Phase::Running {
            return Err(AppError::new(ErrorCategory::Busy, "Setup is already running."));
        }
        self.cancel.store(false, Ordering::SeqCst);
        let steps = steps
            .iter()
            .map(|step| StepProgress { id: step.id().into(), label: step.label().into(), status: StepStatus::Pending })
            .collect();
        *progress = SetupProgress { phase: Phase::Running, steps, activity: "Getting started".into(), failure: None };
        self.emit(&progress);
        Ok(())
    }

    pub fn update(&self, change: impl FnOnce(&mut SetupProgress)) {
        let mut progress = self.progress.lock().expect("progress");
        let before = progress.clone();
        change(&mut progress);
        if *progress != before {
            self.emit(&progress);
        }
    }

    fn emit(&self, progress: &SetupProgress) {
        if let Some(listener) = self.listener.lock().expect("listener").clone() {
            listener(progress);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_setup_cannot_start_while_one_runs_but_can_after_it_ends() {
        let session = SetupSession::default();
        session.begin(&[Step::Node]).unwrap();
        assert_eq!(session.begin(&[Step::Node]).unwrap_err().category, ErrorCategory::Busy);
        session.update(|progress| progress.phase = Phase::Failed);
        assert!(session.begin(&[Step::Node]).is_ok());
    }

    #[test]
    fn listeners_hear_only_real_changes() {
        let session = SetupSession::default();
        let heard = Arc::new(Mutex::new(0));
        let counter = heard.clone();
        session.set_listener(Arc::new(move |_| *counter.lock().unwrap() += 1));
        session.begin(&[Step::Node]).unwrap();
        session.update(|progress| progress.activity = "Installing Node.js".into());
        session.update(|progress| progress.activity = "Installing Node.js".into());
        assert_eq!(*heard.lock().unwrap(), 2);
    }

    #[test]
    fn beginning_again_clears_an_earlier_cancel() {
        let session = SetupSession::default();
        session.request_cancel();
        session.begin(&[Step::Node]).unwrap();
        assert!(!session.is_cancelled());
    }
}
