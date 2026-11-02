use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepProgress {
    pub id: String,
    pub label: String,
    pub status: StepStatus,
}

/// Plain-language failure. `details` is redacted output for people who want to send it on.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub title: String,
    pub message: String,
    pub details: String,
    /// An extra action the screen may offer, such as `move_aside`.
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetupProgress {
    pub phase: Phase,
    pub steps: Vec<StepProgress>,
    pub activity: String,
    pub failure: Option<Failure>,
}

impl SetupProgress {
    pub fn idle() -> Self {
        Self { phase: Phase::Idle, steps: Vec::new(), activity: String::new(), failure: None }
    }

    pub fn set_status(&mut self, id: &str, status: StepStatus) {
        if let Some(step) = self.steps.iter_mut().find(|step| step.id == id) {
            step.status = status;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_format_uses_snake_case_phases_and_camel_case_fields() {
        let mut progress = SetupProgress::idle();
        progress.steps.push(StepProgress { id: "node".into(), label: "Node.js".into(), status: StepStatus::Pending });
        progress.set_status("node", StepStatus::Running);
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["phase"], "idle");
        assert_eq!(json["steps"][0]["status"], "running");
        assert!(json.get("failure").is_some());
    }
}
