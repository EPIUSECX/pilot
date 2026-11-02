use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use super::discovery::child_environment;
use super::exec::run_capture;
use crate::contracts::{BenchTarget, TaskActivity};
use crate::error::{AppError, AppResult, ErrorCategory};

const BRIDGE_SCHEMA_VERSION: u64 = 1;
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(25);
pub const LOOPBACK: &str = "127.0.0.1";

/// Everything the lifecycle needs from Pilot. Real runs go through `InstalledPilot`.
pub trait PilotAdapter: Send + Sync {
    fn list(&self) -> AppResult<Vec<BenchTarget>>;
    fn activity(&self, bench: &BenchTarget) -> AppResult<TaskActivity>;
    fn start_command(&self, bench: &BenchTarget) -> Command;
    fn stop_command(&self, bench: &BenchTarget) -> Command;
    fn create_command(&self, name: &str, branch: &str) -> Command;
}

pub struct InstalledPilot {
    pub root: PathBuf,
    pub python: PathBuf,
    pub bridge: PathBuf,
    pub home: PathBuf,
}

impl InstalledPilot {
    fn pilot(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(&self.python);
        command.arg(self.root.join("bin").join("pilot")).args(arguments);
        command.current_dir(&self.root).env_clear().envs(child_environment(&self.home, LOOPBACK));
        command
    }

    pub fn pilot_version(&self) -> Option<String> {
        self.bridge_json(&["identity"]).ok()?["pilot_version"].as_str().map(str::to_string)
    }

    fn bridge_json(&self, arguments: &[&str]) -> AppResult<Value> {
        let mut command = Command::new(&self.python);
        command.arg("-I").arg(&self.bridge).arg("--pilot-root").arg(&self.root).args(arguments);
        command.current_dir(&self.root).env_clear().envs(child_environment(&self.home, LOOPBACK));
        let output = run_capture(&mut command, BRIDGE_TIMEOUT)?;
        let payload: Value = serde_json::from_str(output.stdout.trim()).map_err(|_| {
            AppError::new(ErrorCategory::EnvironmentMissing, "The Pilot adapter returned unreadable output.")
                .details(&output.stderr)
        })?;
        if payload["schema_version"] != BRIDGE_SCHEMA_VERSION {
            return Err(AppError::new(ErrorCategory::EnvironmentMissing, "The Pilot adapter speaks an unsupported schema version."));
        }
        if let Some(message) = payload["error"]["message"].as_str() {
            return Err(AppError::new(ErrorCategory::NotConfigured, message));
        }
        Ok(payload)
    }
}

pub fn parse_inventory(payload: &Value) -> Vec<BenchTarget> {
    let Some(records) = payload["benches"].as_array() else { return Vec::new() };
    records
        .iter()
        .filter_map(|record| {
            Some(BenchTarget {
                name: record["name"].as_str()?.to_string(),
                path: Path::new(record["path"].as_str()?).to_path_buf(),
                is_production: record["mode"] == "production",
                admin_enabled: record["admin_enabled"].as_bool().unwrap_or(false),
                admin_port: record["admin_port"].as_u64().and_then(|port| u16::try_from(port).ok()).unwrap_or(0),
                is_initialized: record["is_initialized"].as_bool().unwrap_or(false),
                frappe_branch: record["frappe_branch"].as_str().map(str::to_string),
                read_error: record["read_error"].as_str().map(str::to_string),
            })
        })
        .collect()
}

pub fn parse_activity(payload: &Value) -> TaskActivity {
    let activity = &payload["activity"];
    TaskActivity {
        active: activity["active"].as_bool().unwrap_or(true),
        uncertain: activity["uncertain"].as_bool().unwrap_or(true),
        queued_tasks: activity["queued_tasks"].as_u64().unwrap_or(0) as u32,
        running_tasks: activity["running_tasks"].as_u64().unwrap_or(0) as u32,
    }
}

impl PilotAdapter for InstalledPilot {
    fn list(&self) -> AppResult<Vec<BenchTarget>> {
        Ok(parse_inventory(&self.bridge_json(&["inventory"])?))
    }

    fn activity(&self, bench: &BenchTarget) -> AppResult<TaskActivity> {
        Ok(parse_activity(&self.bridge_json(&["--bench", &bench.name, "activity"])?))
    }

    fn start_command(&self, bench: &BenchTarget) -> Command {
        self.pilot(&["-b", &bench.name, "start"])
    }

    fn stop_command(&self, bench: &BenchTarget) -> Command {
        self.pilot(&["-b", &bench.name, "stop"])
    }

    fn create_command(&self, name: &str, branch: &str) -> Command {
        self.pilot(&["new", name, "--frappe-branch", branch])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inventory_parsing_keeps_unreadable_benches_and_defaults_unknowns_safely() {
        let payload = json!({"benches": [
            {"name": "a b", "path": "/p/a b", "mode": "development", "admin_enabled": true, "admin_port": 7000, "is_initialized": true, "frappe_branch": "version-16", "read_error": null},
            {"name": "broken", "path": "/p/broken", "read_error": "ValueError: bad"}
        ]});
        let benches = parse_inventory(&payload);
        assert_eq!(benches.len(), 2);
        assert_eq!(benches[0].path, PathBuf::from("/p/a b"));
        assert!(!benches[1].admin_enabled && benches[1].read_error.is_some());
    }

    #[test]
    fn missing_activity_fields_count_as_unsafe() {
        let activity = parse_activity(&json!({}));
        assert!(activity.is_unsafe());
        assert!(!parse_activity(&json!({"activity": {"active": false, "uncertain": false}})).is_unsafe());
    }

    #[test]
    fn user_input_is_an_argument_never_shell_text() {
        let pilot = InstalledPilot {
            root: PathBuf::from("/Users/a b/pilot"),
            python: PathBuf::from("/usr/bin/python3"),
            bridge: PathBuf::from("/x/bridge.py"),
            home: PathBuf::from("/Users/a b"),
        };
        let command = pilot.create_command("evil; rm -rf ~ $(whoami)", "version-16");
        let arguments: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(command.get_program(), "/usr/bin/python3");
        assert_eq!(arguments[0], "/Users/a b/pilot/bin/pilot");
        assert!(arguments.contains(&"evil; rm -rf ~ $(whoami)".to_string()));
        assert_eq!(command.get_current_dir(), Some(Path::new("/Users/a b/pilot")));
    }
}
