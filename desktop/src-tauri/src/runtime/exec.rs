use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult, ErrorCategory};

#[derive(Debug)]
pub struct Captured {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a command to completion with a deadline, reading both pipes so large output cannot block it.
pub fn run_capture(command: &mut Command, timeout: Duration) -> AppResult<Captured> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::new(ErrorCategory::EnvironmentMissing, format!("Could not start command: {error}")))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            let code = status.code().unwrap_or(-1);
            return Ok(Captured { status: code, stdout: join(stdout), stderr: join(stderr) });
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::new(ErrorCategory::Timeout, "A helper command did not finish in time."));
        }
        thread::sleep(Duration::from_millis(15));
    }
}

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stream) = stream {
            let mut bytes = Vec::new();
            let _ = stream.read_to_end(&mut bytes);
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        text
    })
}

fn join(handle: thread::JoinHandle<String>) -> String {
    handle.join().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_output_and_status() {
        let result = run_capture(Command::new("/bin/sh").args(["-c", "echo out; echo err >&2; exit 3"]), Duration::from_secs(5)).unwrap();
        assert_eq!((result.status, result.stdout.trim(), result.stderr.trim()), (3, "out", "err"));
    }

    #[test]
    fn times_out_a_hung_command() {
        let error = run_capture(Command::new("/bin/sleep").arg("5"), Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.category, ErrorCategory::Timeout);
    }
}
