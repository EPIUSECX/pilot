use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::askpass;
use super::executor::{Executor, Job};
use super::failure::{classify, describe_activity, explain, is_retryable, FailureKind};
use super::machine::Machine;
use super::progress::{Failure, Phase, StepStatus};
use super::steps::Step;
use super::SetupSession;
use crate::runtime::logs::LogBuffer;
use crate::runtime::redact::redact;

pub const INSTALLER_URL: &str = "https://raw.githubusercontent.com/frappe/pilot/develop/install.sh";
pub const HOMEBREW_URL: &str = "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh";
pub const LOG_NAME: &str = "setup";
const MAX_ATTEMPTS: u32 = 3;
const TAIL_LINES: usize = 40;
const SETUP_PYTHON: &str = "3.14";

#[derive(Debug, Clone, Copy)]
pub struct Timings {
    pub retry_pause: Duration,
    pub poll: Duration,
    pub tools_wait: Duration,
    pub step_timeout: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            retry_pause: Duration::from_secs(5),
            poll: Duration::from_secs(3),
            tools_wait: Duration::from_secs(30 * 60),
            step_timeout: Duration::from_secs(60 * 60),
        }
    }
}

enum Trouble {
    Cancelled,
    Failed { kind: FailureKind, details: String },
}

fn failed(kind: FailureKind, details: impl Into<String>) -> Trouble {
    Trouble::Failed { kind, details: details.into() }
}

/// Brings this Mac up to what Pilot needs, one missing piece at a time. Every step reuses what is already there.
pub struct Runner {
    pub session: Arc<SetupSession>,
    pub executor: Arc<dyn Executor>,
    pub machine: Arc<dyn Machine>,
    pub logs: Arc<LogBuffer>,
    pub support: PathBuf,
    pub timings: Timings,
}

impl Runner {
    pub fn execute(&self, steps: &[Step]) {
        for step in steps {
            self.session.update(|progress| {
                progress.set_status(step.id(), StepStatus::Running);
                progress.activity = format!("Installing {}", step.label());
            });
            match self.run_with_retries(*step) {
                Ok(()) => self.session.update(|progress| progress.set_status(step.id(), StepStatus::Done)),
                Err(Trouble::Cancelled) => return self.finish(Phase::Cancelled, None),
                Err(Trouble::Failed { kind, details }) => {
                    self.session.update(|progress| progress.set_status(step.id(), StepStatus::Failed));
                    return self.finish(Phase::Failed, Some(explain(*step, kind, &details)));
                }
            }
        }
        self.verify();
    }

    fn finish(&self, phase: Phase, failure: Option<Failure>) {
        self.session.update(|progress| {
            progress.phase = phase;
            progress.failure = failure;
            progress.activity.clear();
        });
    }

    fn verify(&self) {
        self.session.update(|progress| progress.activity = "Checking that everything works".into());
        let report = self.machine.report();
        if report.is_ready {
            return self.finish(Phase::Succeeded, None);
        }
        let unmet: Vec<_> = report.checks.iter().filter(|check| check.required && check.status != crate::contracts::CheckStatus::Ok).collect();
        let names: Vec<&str> = unmet.iter().map(|check| check.label.as_str()).collect();
        let details = unmet.iter().map(|check| format!("{}: {}", check.label, check.message)).collect::<Vec<_>>().join("\n");
        let failure = Failure {
            title: "Almost there".into(),
            message: format!("Setup finished, but {} still isn't working. Try again, and if it persists copy the details for the Pilot team.", names.join(", ")),
            details,
            recovery: None,
        };
        self.finish(Phase::Failed, Some(failure));
    }

    fn run_with_retries(&self, step: Step) -> Result<(), Trouble> {
        for attempt in 1..=MAX_ATTEMPTS {
            match self.attempt(step) {
                Err(Trouble::Failed { kind, .. }) if is_retryable(kind) && attempt < MAX_ATTEMPTS => {
                    self.logs.push(LOG_NAME, "launcher", &format!("Network problem. Trying again ({attempt} of {MAX_ATTEMPTS})."));
                    self.session.update(|progress| progress.activity = "Connection dropped. Trying again".into());
                    thread::sleep(self.timings.retry_pause);
                }
                outcome => return outcome,
            }
        }
        unreachable!("the last attempt always returns")
    }

    fn attempt(&self, step: Step) -> Result<(), Trouble> {
        match step {
            Step::CommandLineTools => self.install_command_line_tools(),
            Step::Homebrew => self.install_homebrew(),
            Step::Node => self.install_node(),
            Step::Pilot => self.install_pilot(),
            Step::Python => self.install_python(),
        }
    }

    fn job(&self, program: impl Into<PathBuf>, arguments: &[&str], askpass: Option<&Path>) -> Job {
        Job {
            program: program.into(),
            arguments: arguments.iter().map(|argument| argument.to_string()).collect(),
            environment: self.machine.environment(askpass),
            timeout: self.timings.step_timeout,
        }
    }

    /// Returns the last lines of output. Output is redacted before it is stored or shown.
    fn run_job(&self, job: &Job) -> Result<String, Trouble> {
        let mut tail: VecDeque<String> = VecDeque::new();
        let outcome = self.executor.run(job, &mut |raw| {
            let Some(line) = raw.split('\r').map(str::trim).rfind(|segment| !segment.is_empty()) else { return };
            self.logs.push(LOG_NAME, "stdout", line);
            if let Some(activity) = describe_activity(line) {
                self.session.update(|progress| progress.activity = activity);
            }
            if tail.len() == TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(redact(line));
        });
        let output = tail.iter().cloned().collect::<Vec<_>>().join("\n");
        if self.session.is_cancelled() {
            return Err(Trouble::Cancelled);
        }
        match outcome {
            Ok(0) => Ok(output),
            Ok(_) => Err(failed(classify(&output), output)),
            Err(error) => Err(failed(FailureKind::Other, format!("{}\n{output}", error.message))),
        }
    }

    fn download(&self, url: &str, destination: &Path) -> Result<(), Trouble> {
        if let Some(directory) = destination.parent() {
            fs::create_dir_all(directory).map_err(|error| failed(FailureKind::Other, error.to_string()))?;
        }
        let target = destination.display().to_string();
        let arguments = ["-fsSL", "--proto", "=https", "--tlsv1.2", "-o", &target, url];
        self.run_job(&self.job("/usr/bin/curl", &arguments, None)).map(|_| ())
    }

    fn install_command_line_tools(&self) -> Result<(), Trouble> {
        if let Err(Trouble::Cancelled) = self.run_job(&self.job("/usr/bin/xcode-select", &["--install"], None)) {
            return Err(Trouble::Cancelled);
        }
        self.session.update(|progress| progress.activity = "Waiting for the Apple install window. Follow its steps".into());
        let deadline = Instant::now() + self.timings.tools_wait;
        while !self.machine.has_command_line_tools() {
            if self.session.is_cancelled() {
                return Err(Trouble::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(failed(FailureKind::Other, "The Apple developer tools were not installed in time."));
            }
            thread::sleep(self.timings.poll);
        }
        Ok(())
    }

    fn install_homebrew(&self) -> Result<(), Trouble> {
        let script = self.support.join("setup").join("homebrew-install.sh");
        self.download(HOMEBREW_URL, &script)?;
        let helper = askpass::write(&self.support).map_err(|error| failed(FailureKind::Other, error.message))?;
        let script = script.display().to_string();
        self.run_job(&self.job("/bin/bash", &[&script], Some(&helper)))?;
        match self.machine.tool("brew") {
            Some(_) => Ok(()),
            None => Err(failed(FailureKind::Other, "Homebrew finished but brew was not found.")),
        }
    }

    fn install_node(&self) -> Result<(), Trouble> {
        let brew = self.machine.tool("brew").ok_or_else(|| failed(FailureKind::Other, "Homebrew is not installed."))?;
        self.run_job(&self.job(brew, &["install", "node@24"], None))?;
        match self.machine.has_node() {
            true => Ok(()),
            false => Err(failed(FailureKind::Other, "Node.js 24 was installed but could not be found.")),
        }
    }

    fn install_pilot(&self) -> Result<(), Trouble> {
        let script = self.support.join("installer").join("install.sh");
        self.download(INSTALLER_URL, &script)?;
        let script_path = script.display().to_string();
        let checksum = self.run_job(&self.job("/usr/bin/shasum", &["-a", "256", &script_path], None))?;
        let mode = self.run_installer(&script_path)?;
        self.record_installation(&checksum, mode);
        Ok(())
    }

    /// A git checkout is only ever updated in place. A release download is unpacked only into a folder that is not one.
    fn run_installer(&self, script: &str) -> Result<&'static str, Trouble> {
        let develop = || self.run_job(&self.job("/bin/sh", &[script, "--dev"], None)).map(|_| "development");
        if self.machine.has_development_checkout() {
            return develop();
        }
        match self.run_job(&self.job("/bin/sh", &[script], None)) {
            Err(Trouble::Failed { kind: FailureKind::ReleaseMissing, .. }) => develop(),
            other => other.map(|_| "release"),
        }
    }

    fn record_installation(&self, checksum: &str, mode: &str) {
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
        let sha256 = checksum.split_whitespace().next().unwrap_or("unknown");
        let record = serde_json::json!({ "source": INSTALLER_URL, "sha256": sha256, "mode": mode, "installedAtEpochSeconds": seconds });
        let _ = fs::write(self.support.join("installer").join("installation.json"), record.to_string());
    }

    fn install_python(&self) -> Result<(), Trouble> {
        let uv = self.machine.tool("uv").ok_or_else(|| failed(FailureKind::Other, "uv is not installed."))?;
        self.run_job(&self.job(uv, &["python", "install", SETUP_PYTHON], None)).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use super::*;
    use crate::contracts::{CheckStatus, DependencyCheck, EnvironmentReport};
    use crate::error::AppResult;
    use crate::setup::progress::SetupProgress;

    type Reply = Box<dyn Fn(&Job, usize) -> (i32, Vec<&'static str>) + Send + Sync>;

    struct Scripted {
        jobs: Mutex<Vec<Job>>,
        reply: Reply,
    }

    impl Executor for Scripted {
        fn run(&self, job: &Job, on_line: &mut dyn FnMut(&str)) -> AppResult<i32> {
            let index = {
                let mut jobs = self.jobs.lock().unwrap();
                jobs.push(job.clone());
                jobs.len()
            };
            let (status, lines) = (self.reply)(job, index);
            lines.into_iter().for_each(|line| on_line(line));
            Ok(status)
        }
    }

    struct FakeMachine {
        is_ready: AtomicBool,
        has_checkout: bool,
    }

    impl Machine for FakeMachine {
        fn report(&self) -> EnvironmentReport {
            let is_ready = self.is_ready.load(Ordering::SeqCst);
            let missing = DependencyCheck {
                id: "node".into(),
                label: "Node.js".into(),
                status: CheckStatus::Missing,
                required: true,
                path: None,
                version: None,
                message: "node was not found.".into(),
                next_action: None,
                details: None,
            };
            EnvironmentReport {
                checks: if is_ready { Vec::new() } else { vec![missing] },
                pilot_root: None,
                pilot_version: None,
                is_ready,
                architecture: "arm64".into(),
            }
        }
        fn has_command_line_tools(&self) -> bool {
            true
        }
        fn has_node(&self) -> bool {
            true
        }
        fn has_development_checkout(&self) -> bool {
            self.has_checkout
        }
        fn tool(&self, name: &str) -> Option<PathBuf> {
            Some(PathBuf::from("/fake").join(name))
        }
        fn environment(&self, askpass: Option<&Path>) -> Vec<(String, String)> {
            askpass.map(|path| ("SUDO_ASKPASS".to_string(), path.display().to_string())).into_iter().collect()
        }
    }

    struct Harness {
        runner: Runner,
        executor: Arc<Scripted>,
        _support: tempfile::TempDir,
    }

    fn harness(is_ready: bool, reply: impl Fn(&Job, usize) -> (i32, Vec<&'static str>) + Send + Sync + 'static) -> Harness {
        harness_with(is_ready, false, reply)
    }

    fn harness_with(is_ready: bool, has_checkout: bool, reply: impl Fn(&Job, usize) -> (i32, Vec<&'static str>) + Send + Sync + 'static) -> Harness {
        let support = tempfile::tempdir().unwrap();
        let executor = Arc::new(Scripted { jobs: Mutex::new(Vec::new()), reply: Box::new(reply) });
        let runner = Runner {
            session: Arc::new(SetupSession::default()),
            executor: executor.clone(),
            machine: Arc::new(FakeMachine { is_ready: AtomicBool::new(is_ready), has_checkout }),
            logs: Arc::new(LogBuffer::new(support.path().join("logs"))),
            support: support.path().to_path_buf(),
            timings: Timings { retry_pause: Duration::ZERO, poll: Duration::from_millis(5), tools_wait: Duration::from_millis(50), step_timeout: Duration::from_secs(5) },
        };
        Harness { runner, executor, _support: support }
    }

    fn run(harness: &Harness, steps: &[Step]) -> SetupProgress {
        harness.runner.session.begin(steps).unwrap();
        harness.runner.execute(steps);
        harness.runner.session.snapshot()
    }

    fn programs(harness: &Harness) -> Vec<String> {
        harness.executor.jobs.lock().unwrap().iter().map(|job| job.program.display().to_string()).collect()
    }

    #[test]
    fn a_missing_node_and_pilot_are_installed_in_order_and_verified() {
        let harness = harness(true, |_, _| (0, vec!["==> Installing mariadb@11.8"]));
        let progress = run(&harness, &[Step::Node, Step::Pilot]);
        assert_eq!(progress.phase, Phase::Succeeded);
        assert!(progress.steps.iter().all(|step| step.status == StepStatus::Done));
        assert_eq!(programs(&harness), ["/fake/brew", "/usr/bin/curl", "/usr/bin/shasum", "/bin/sh"]);
    }

    #[test]
    fn a_dropped_connection_is_retried_without_bothering_the_user() {
        let harness = harness(true, |job, index| match (job.program.ends_with("curl"), index) {
            (true, 1) => (6, vec!["curl: (6) Could not resolve host: raw.githubusercontent.com"]),
            _ => (0, vec![]),
        });
        let progress = run(&harness, &[Step::Pilot]);
        assert_eq!(progress.phase, Phase::Succeeded);
        assert_eq!(programs(&harness).iter().filter(|program| program.ends_with("curl")).count(), 2);
    }

    #[test]
    fn a_connection_that_stays_down_ends_in_a_plain_explanation_with_the_raw_output_kept_aside() {
        let harness = harness(true, |_, _| (6, vec!["curl: (6) Could not resolve host: raw.githubusercontent.com"]));
        let progress = run(&harness, &[Step::Pilot]);
        let failure = progress.failure.unwrap();
        assert_eq!((progress.phase, failure.title.as_str()), (Phase::Failed, "Couldn't reach the internet"));
        assert!(!failure.message.contains("curl") && failure.details.contains("curl: (6)"));
        assert_eq!(programs(&harness).len(), 3);
        assert_eq!(progress.steps[0].status, StepStatus::Failed);
    }

    #[test]
    fn a_missing_release_falls_back_to_the_development_install() {
        let harness = harness(true, |job, _| {
            let is_release_run = job.program.ends_with("sh") && job.arguments.len() == 1;
            if is_release_run { (1, vec!["Could not find a pilot.tar.gz release asset."]) } else { (0, vec![]) }
        });
        let progress = run(&harness, &[Step::Pilot]);
        assert_eq!(progress.phase, Phase::Succeeded);
        let jobs = harness.executor.jobs.lock().unwrap();
        assert_eq!(jobs.last().unwrap().arguments.last().map(String::as_str), Some("--dev"));
        let record = fs::read_to_string(harness.runner.support.join("installer/installation.json")).unwrap();
        assert!(record.contains("development"));
    }

    #[test]
    fn an_existing_git_checkout_is_updated_in_place_and_never_overwritten_by_a_release() {
        let harness = harness_with(true, true, |_, _| (0, vec![]));
        assert_eq!(run(&harness, &[Step::Pilot]).phase, Phase::Succeeded);
        let jobs = harness.executor.jobs.lock().unwrap();
        let installs: Vec<_> = jobs.iter().filter(|job| job.program.ends_with("sh")).collect();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].arguments.last().map(String::as_str), Some("--dev"));
    }

    #[test]
    fn a_checkout_that_cannot_update_offers_to_be_moved_aside() {
        let harness = harness_with(true, true, |job, _| {
            if job.arguments.last().is_some_and(|argument| argument == "--dev") { (1, vec!["Your configuration specifies to merge with the ref 'refs/heads/main'", "from the remote, but no such ref was fetched."]) } else { (0, vec![]) }
        });
        let failure = run(&harness, &[Step::Pilot]).failure.unwrap();
        assert_eq!((failure.title.as_str(), failure.recovery.as_deref()), ("Your Pilot folder is out of date", Some("move_aside")));
    }

    #[test]
    fn local_changes_in_the_pilot_folder_stop_setup_with_an_explanation_and_no_retry() {
        let harness = harness_with(true, true, |job, _| {
            if job.arguments.last().is_some_and(|argument| argument == "--dev") { (1, vec!["error: Your local changes to the following files would be overwritten by merge:"]) } else { (0, vec![]) }
        });
        let failure = run(&harness, &[Step::Pilot]).failure.unwrap();
        assert_eq!(failure.title, "Your Pilot folder has changes of its own");
        assert_eq!(harness.executor.jobs.lock().unwrap().iter().filter(|job| job.program.ends_with("sh")).count(), 1);
    }

    #[test]
    fn something_still_missing_afterwards_is_reported_by_name() {
        let harness = harness(false, |_, _| (0, vec![]));
        let progress = run(&harness, &[Step::Node]);
        let failure = progress.failure.unwrap();
        assert_eq!(failure.title, "Almost there");
        assert!(failure.message.contains("Node.js"));
    }

    #[test]
    fn cancelling_stops_before_the_next_step_and_leaves_it_pending() {
        let harness = harness(true, |_, _| (0, vec![]));
        let session = harness.runner.session.clone();
        let cancelling = Arc::new(Scripted { jobs: Mutex::new(Vec::new()), reply: Box::new(move |_, _| { session.request_cancel(); (-1, vec![]) }) });
        let runner = Runner { executor: cancelling, ..harness.runner };
        runner.session.begin(&[Step::Node, Step::Pilot]).unwrap();
        runner.execute(&[Step::Node, Step::Pilot]);
        let progress = runner.session.snapshot();
        assert_eq!(progress.phase, Phase::Cancelled);
        assert_eq!(progress.steps[1].status, StepStatus::Pending);
    }

    #[test]
    fn secrets_in_installer_output_never_reach_the_failure_details() {
        let harness = harness(true, |_, _| (1, vec!["Generated admin password: Zx9!secretValue"]));
        let failure = run(&harness, &[Step::Node]).failure.unwrap();
        assert!(!failure.details.contains("secretValue"));
    }

    #[test]
    fn only_the_homebrew_download_script_is_given_the_password_helper() {
        let harness = harness(true, |_, _| (0, vec![]));
        run(&harness, &[Step::Homebrew]);
        let jobs = harness.executor.jobs.lock().unwrap();
        let with_helper: Vec<_> = jobs.iter().filter(|job| job.environment.iter().any(|(key, _)| key == "SUDO_ASKPASS")).collect();
        assert_eq!(with_helper.len(), 1);
        assert!(with_helper[0].program.ends_with("bash"));
    }
}
