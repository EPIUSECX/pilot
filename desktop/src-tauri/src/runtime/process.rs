use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, ErrorCategory};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: i32,
    pub parent: i32,
    pub started: String,
    pub command: String,
}

/// Recorded at spawn. `command` is the argument string Desktop passed (not the executable, which a
/// shim may replace by exec). Matching needs pid, start time and that argument string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessIdentity {
    pub pid: i32,
    pub started: String,
    pub command: String,
}

#[derive(Debug, Default)]
pub struct ProcessTable {
    entries: Vec<ProcessEntry>,
}

impl ProcessTable {
    pub fn snapshot() -> AppResult<Self> {
        let output = Command::new("/bin/ps")
            .args(["-axo", "pid=,ppid=,lstart=,command="])
            .output()
            .map_err(|error| AppError::new(ErrorCategory::Io, format!("Could not run ps: {error}")))?;
        Ok(Self::parse(&String::from_utf8_lossy(&output.stdout)))
    }

    pub fn parse(text: &str) -> Self {
        Self { entries: text.lines().filter_map(parse_line).collect() }
    }

    pub fn from_entries(entries: Vec<ProcessEntry>) -> Self {
        Self { entries }
    }

    pub fn get(&self, pid: i32) -> Option<&ProcessEntry> {
        self.entries.iter().find(|entry| entry.pid == pid)
    }

    pub fn identity_of(&self, pid: i32, arguments: &str) -> Option<ProcessIdentity> {
        self.get(pid).map(|entry| ProcessIdentity { pid, started: entry.started.clone(), command: arguments.to_string() })
    }

    pub fn matches(&self, identity: &ProcessIdentity) -> bool {
        self.get(identity.pid).is_some_and(|entry| {
            entry.started == identity.started && entry.command.contains(&identity.command)
        })
    }

    pub fn descendants(&self, pid: i32) -> Vec<i32> {
        let mut found = Vec::new();
        let mut frontier = vec![pid];
        while let Some(current) = frontier.pop() {
            for entry in self.entries.iter().filter(|entry| entry.parent == current) {
                if !found.contains(&entry.pid) && entry.pid != pid {
                    found.push(entry.pid);
                    frontier.push(entry.pid);
                }
            }
        }
        found
    }

    pub fn is_self_or_descendant(&self, pid: i32, ancestor: i32) -> bool {
        pid == ancestor || self.descendants(ancestor).contains(&pid)
    }
}

fn parse_line(line: &str) -> Option<ProcessEntry> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let parent = parts.next()?.parse().ok()?;
    let started = (0..5).map(|_| parts.next()).collect::<Option<Vec<_>>>()?.join(" ");
    let command = parts.collect::<Vec<_>>().join(" ");
    Some(ProcessEntry { pid, parent, started, command })
}

pub fn send_signal(pid: i32, signal: i32) -> bool {
    // SAFETY: kill(2) with a pid previously matched against a recorded identity.
    unsafe { libc::kill(pid, signal) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "  100     1 Sun Nov  1 21:00:00 2026 /usr/bin/python3 /Users/a b/pilot/bin/pilot -b main start\n\
                          101   100 Sun Nov  1 21:00:01 2026 redis-server *:13000\n\
                          102   101 Sun Nov  1 21:00:02 2026 sleep 1\n\
                          200     1 Sun Nov  1 21:00:03 2026 unrelated";

    #[test]
    fn parses_ps_with_spaces_in_the_command() {
        let table = ProcessTable::parse(SAMPLE);
        assert_eq!(table.get(100).unwrap().command, "/usr/bin/python3 /Users/a b/pilot/bin/pilot -b main start");
        assert_eq!(table.get(100).unwrap().started, "Sun Nov 1 21:00:00 2026");
    }

    #[test]
    fn finds_descendants_only() {
        let table = ProcessTable::parse(SAMPLE);
        let mut found = table.descendants(100);
        found.sort();
        assert_eq!(found, vec![101, 102]);
        assert!(table.is_self_or_descendant(102, 100));
        assert!(!table.is_self_or_descendant(200, 100));
    }

    #[test]
    fn a_reused_pid_does_not_match_the_recorded_identity() {
        let before = ProcessTable::parse(SAMPLE);
        let identity = before.identity_of(100, "/Users/a b/pilot/bin/pilot -b main start").unwrap();
        assert!(before.matches(&identity));

        let reexeced = ProcessTable::parse("  100     1 Sun Nov  1 21:00:00 2026 /real/Python /Users/a b/pilot/bin/pilot -b main start");
        assert!(reexeced.matches(&identity), "an interpreter shim re-exec keeps the same process");

        let reused = ProcessTable::parse("  100     1 Sun Nov  1 22:30:00 2026 /usr/bin/python3 /Users/a b/pilot/bin/pilot -b main start");
        assert!(!reused.matches(&identity));
        let other_program = ProcessTable::parse("  100     1 Sun Nov  1 21:00:00 2026 /Applications/Safari.app/Safari");
        assert!(!other_program.matches(&identity));
    }
}
