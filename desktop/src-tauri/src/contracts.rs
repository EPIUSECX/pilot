use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    Owned,
    Attached,
    None,
}

/// A bench as the Pilot adapter reports it. Never crosses into the renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchTarget {
    pub name: String,
    pub path: PathBuf,
    pub is_production: bool,
    pub admin_enabled: bool,
    pub admin_port: u16,
    pub is_initialized: bool,
    pub frappe_branch: Option<String>,
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BenchSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub status: Status,
    pub ownership: Ownership,
    pub dashboard_url: Option<String>,
    pub frappe_branch: Option<String>,
    pub is_production: bool,
    pub is_initialized: bool,
    pub admin_enabled: bool,
    pub last_error: Option<AppError>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskActivity {
    pub active: bool,
    pub uncertain: bool,
    pub queued_tasks: u32,
    pub running_tasks: u32,
}

impl TaskActivity {
    pub fn is_unsafe(&self) -> bool {
        self.active || self.uncertain
    }

    pub fn describe(&self) -> String {
        if self.uncertain {
            return "Pilot could not confirm that no task is running.".to_string();
        }
        format!("{} running and {} queued Pilot task(s).", self.running_tasks, self.queued_tasks)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub ok: bool,
    pub message: String,
    pub bench: Option<BenchSummary>,
    pub notices: Vec<String>,
    pub generated_admin_password: Option<String>,
}

impl OperationResult {
    pub fn success(message: impl Into<String>, bench: Option<BenchSummary>) -> Self {
        Self { ok: true, message: message.into(), bench, notices: Vec::new(), generated_admin_password: None }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Missing,
    Incompatible,
    Stopped,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DependencyCheck {
    pub id: String,
    pub label: String,
    pub status: CheckStatus,
    pub required: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub message: String,
    pub next_action: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReport {
    pub checks: Vec<DependencyCheck>,
    pub pilot_root: Option<String>,
    pub pilot_version: Option<String>,
    pub is_ready: bool,
    pub architecture: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateBenchRequest {
    pub name: String,
    pub frappe_branch: String,
}
