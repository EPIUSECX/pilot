use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::redact::redact;

const LINES_PER_BENCH: usize = 2000;
const FILE_LIMIT_BYTES: u64 = 1_000_000;
const RETAINED_FILES: usize = 3;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub bench: String,
    pub stream: String,
    pub timestamp_ms: u64,
    pub text: String,
}

type Listener = Arc<dyn Fn(&LogLine) + Send + Sync>;

/// Sanitizing choke point: raw process output enters here and only redacted text leaves.
pub struct LogBuffer {
    lines: Mutex<HashMap<String, VecDeque<LogLine>>>,
    directory: PathBuf,
    listener: Mutex<Option<Listener>>,
}

impl LogBuffer {
    pub fn new(directory: PathBuf) -> Self {
        Self { lines: Mutex::new(HashMap::new()), directory, listener: Mutex::new(None) }
    }

    pub fn set_listener(&self, listener: Listener) {
        *self.listener.lock().expect("listener lock") = Some(listener);
    }

    pub fn push(&self, bench: &str, stream: &str, raw: &str) {
        let line = LogLine {
            bench: bench.to_string(),
            stream: stream.to_string(),
            timestamp_ms: now_milliseconds(),
            text: redact(raw.trim_end_matches(['\r', '\n'])),
        };
        self.store(&line);
        self.append_to_file(&line);
        if let Some(listener) = self.listener.lock().expect("listener lock").clone() {
            listener(&line);
        }
    }

    pub fn recent(&self, bench: &str) -> Vec<LogLine> {
        let lines = self.lines.lock().expect("lines lock");
        lines.get(bench).map(|queue| queue.iter().cloned().collect()).unwrap_or_default()
    }

    fn store(&self, line: &LogLine) {
        let mut lines = self.lines.lock().expect("lines lock");
        let queue = lines.entry(line.bench.clone()).or_default();
        if queue.len() == LINES_PER_BENCH {
            queue.pop_front();
        }
        queue.push_back(line.clone());
    }

    fn append_to_file(&self, line: &LogLine) {
        if fs::create_dir_all(&self.directory).is_err() {
            return;
        }
        let path = self.directory.join(format!("{}.log", file_stem(&line.bench)));
        self.rotate(&path);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{} [{}] {}", line.timestamp_ms, line.stream, line.text);
        }
    }

    fn rotate(&self, path: &PathBuf) {
        let size = fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
        if size < FILE_LIMIT_BYTES {
            return;
        }
        for index in (1..RETAINED_FILES).rev() {
            let from = path.with_extension(format!("log.{index}"));
            let to = path.with_extension(format!("log.{}", index + 1));
            let _ = fs::rename(from, to);
        }
        let _ = fs::rename(path, path.with_extension("log.1"));
    }
}

fn file_stem(bench: &str) -> String {
    bench.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn now_milliseconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_redacted_before_buffer_listener_and_file() {
        let directory = tempfile::tempdir().unwrap();
        let buffer = LogBuffer::new(directory.path().join("logs dir"));
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        buffer.set_listener(Arc::new(move |line| sink.lock().unwrap().push(line.text.clone())));

        buffer.push("main", "stdout", "Open http://localhost:7000/?sid=SUPERSECRET in your browser\n");
        buffer.push("main", "stdout", "Generated admin password: Hunter2!\n");

        let everything = format!(
            "{:?}{:?}{}",
            buffer.recent("main"),
            seen.lock().unwrap(),
            fs::read_to_string(directory.path().join("logs dir/main.log")).unwrap()
        );
        assert!(!everything.contains("SUPERSECRET") && !everything.contains("Hunter2"));
        assert!(everything.contains("[redacted]"));
    }

    #[test]
    fn buffer_is_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let buffer = LogBuffer::new(directory.path().to_path_buf());
        for index in 0..(LINES_PER_BENCH + 50) {
            buffer.push("b", "stdout", &format!("line {index}"));
        }
        let lines = buffer.recent("b");
        assert_eq!(lines.len(), LINES_PER_BENCH);
        assert_eq!(lines[0].text, "line 50");
    }
}
