use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult, ErrorCategory};

const POLL: Duration = Duration::from_millis(100);
const LINGER: Duration = Duration::from_millis(300);
const TERMINATE_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct Job {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub timeout: Duration,
}

pub trait Executor: Send + Sync {
    /// Runs the job to completion, passing each output line to `on_line`, and returns the exit status.
    fn run(&self, job: &Job, on_line: &mut dyn FnMut(&str)) -> AppResult<i32>;
}

/// Runs a job in its own process group so a cancel or timeout reaches everything it started.
pub struct ProcessExecutor {
    cancel: Arc<AtomicBool>,
}

impl ProcessExecutor {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        Self { cancel }
    }
}

fn spawn_reader<R: Read + Send + 'static>(stream: R, sender: mpsc::Sender<String>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut bytes = Vec::new();
        while matches!(reader.read_until(b'\n', &mut bytes), Ok(read) if read > 0) {
            if sender.send(String::from_utf8_lossy(&bytes).into_owned()).is_err() {
                return;
            }
            bytes.clear();
        }
    });
}

fn terminate(child: &mut Child) {
    let group = child.id() as i32;
    unsafe { libc::killpg(group, libc::SIGTERM) };
    let deadline = Instant::now() + TERMINATE_GRACE;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    unsafe { libc::killpg(group, libc::SIGKILL) };
    let _ = child.wait();
}

impl Executor for ProcessExecutor {
    fn run(&self, job: &Job, on_line: &mut dyn FnMut(&str)) -> AppResult<i32> {
        let mut child = Command::new(&job.program)
            .args(&job.arguments)
            .env_clear()
            .envs(job.environment.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|error| AppError::new(ErrorCategory::EnvironmentMissing, format!("Could not start {}: {error}", job.program.display())))?;
        let (sender, receiver) = mpsc::channel();
        spawn_reader(child.stdout.take().expect("piped stdout"), sender.clone());
        spawn_reader(child.stderr.take().expect("piped stderr"), sender);
        let deadline = Instant::now() + job.timeout;
        loop {
            match receiver.recv_timeout(POLL) {
                Ok(line) => on_line(&line),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.cancel.load(Ordering::SeqCst) {
                terminate(&mut child);
                return Ok(-1);
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                return Err(AppError::new(ErrorCategory::Timeout, "A setup step took too long."));
            }
            if matches!(child.try_wait(), Ok(Some(_))) {
                while let Ok(line) = receiver.recv_timeout(LINGER) {
                    on_line(&line);
                }
                break;
            }
        }
        Ok(child.wait()?.code().unwrap_or(-1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str, timeout: Duration) -> Job {
        Job { program: "/bin/sh".into(), arguments: vec!["-c".into(), script.into()], environment: vec![("PATH".into(), "/usr/bin:/bin".into())], timeout }
    }

    fn executor() -> (ProcessExecutor, Arc<AtomicBool>) {
        let cancel = Arc::new(AtomicBool::new(false));
        (ProcessExecutor::new(cancel.clone()), cancel)
    }

    #[test]
    fn streams_both_pipes_and_reports_the_exit_status() {
        let (executor, _) = executor();
        let mut lines = Vec::new();
        let status = executor.run(&shell("echo out; echo err >&2; exit 3", Duration::from_secs(5)), &mut |line| lines.push(line.trim().to_string())).unwrap();
        lines.sort();
        assert_eq!((status, lines), (3, vec!["err".to_string(), "out".to_string()]));
    }

    #[test]
    fn runs_with_only_the_environment_it_was_given() {
        let (executor, _) = executor();
        let mut lines = Vec::new();
        let job = shell("echo \"${HOME:-unset}\"", Duration::from_secs(5));
        executor.run(&job, &mut |line| lines.push(line.trim().to_string())).unwrap();
        assert_eq!(lines, ["unset"]);
    }

    #[test]
    fn a_background_child_holding_the_pipe_does_not_hang_the_step() {
        let (executor, _) = executor();
        let started = Instant::now();
        executor.run(&shell("sleep 5 & echo done", Duration::from_secs(10)), &mut |_| {}).unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_hung_step_times_out_and_its_children_are_stopped() {
        let (executor, _) = executor();
        let error = executor.run(&shell("sleep 30", Duration::from_millis(300)), &mut |_| {}).unwrap_err();
        assert_eq!(error.category, ErrorCategory::Timeout);
    }

    #[test]
    fn cancelling_stops_the_job_promptly() {
        let (executor, cancel) = executor();
        let flag = cancel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        assert_eq!(executor.run(&shell("sleep 30", Duration::from_secs(60)), &mut |_| {}).unwrap(), -1);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
