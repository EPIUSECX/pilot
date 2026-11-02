use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::contracts::{CheckStatus, EnvironmentReport};
use crate::runtime::exec::run_capture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    CommandLineTools,
    Homebrew,
    Node,
    Pilot,
    Python,
}

impl Step {
    pub fn id(self) -> &'static str {
        match self {
            Step::CommandLineTools => "tools",
            Step::Homebrew => "homebrew",
            Step::Node => "node",
            Step::Pilot => "pilot",
            Step::Python => "python",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Step::CommandLineTools => "Apple developer tools",
            Step::Homebrew => "Homebrew",
            Step::Node => "Node.js 24",
            Step::Pilot => "Pilot and its tools",
            Step::Python => "Python",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Step::CommandLineTools => "Needed to install developer software. macOS shows its own install window for this.",
            Step::Homebrew => "The standard way to add software on a Mac. Asks for your Mac password once.",
            Step::Node => "Builds the Pilot dashboard and your Frappe sites.",
            Step::Pilot => "Pilot itself, with the database, Redis and web server it relies on.",
            Step::Python => "Runs Pilot's commands.",
        }
    }

    pub fn needs_password(self) -> bool {
        matches!(self, Step::CommandLineTools | Step::Homebrew)
    }
}

/// What this Mac already has, reduced to the decisions setup makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    pub has_command_line_tools: bool,
    pub has_homebrew: bool,
    pub has_node: bool,
    pub has_pilot_tools: bool,
    pub has_python: bool,
}

impl Probe {
    pub fn from_report(report: &EnvironmentReport, has_command_line_tools: bool) -> Self {
        let is_ok = |id: &str| report.checks.iter().any(|check| check.id == id && check.status == CheckStatus::Ok);
        Self {
            has_command_line_tools,
            has_homebrew: is_ok("homebrew"),
            has_node: is_ok("node"),
            has_pilot_tools: ["pilot", "uv", "redis"].iter().all(|id| is_ok(id)),
            has_python: is_ok("python"),
        }
    }

    /// Only what is missing, in dependency order. Homebrew is added only when a later step needs it.
    pub fn steps(&self) -> Vec<Step> {
        let needs_homebrew_packages = !self.has_node || !self.has_pilot_tools;
        let candidates = [
            (Step::CommandLineTools, !self.has_command_line_tools),
            (Step::Homebrew, !self.has_homebrew && needs_homebrew_packages),
            (Step::Node, !self.has_node),
            (Step::Pilot, !self.has_pilot_tools),
            (Step::Python, !self.has_python),
        ];
        candidates.into_iter().filter_map(|(step, needed)| needed.then_some(step)).collect()
    }
}

pub fn command_line_tools_installed() -> bool {
    let probe = run_capture(Command::new("/usr/bin/xcode-select").arg("-p"), Duration::from_secs(5));
    probe.is_ok_and(|output| output.status == 0 && Path::new(output.stdout.trim()).is_dir())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub id: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlan {
    pub steps: Vec<PlanStep>,
    pub needs_password: bool,
    pub estimate: String,
}

pub fn plan_for(steps: &[Step]) -> SetupPlan {
    SetupPlan {
        steps: steps
            .iter()
            .map(|step| PlanStep { id: step.id().into(), label: step.label().into(), detail: step.detail().into() })
            .collect(),
        needs_password: steps.iter().any(|step| step.needs_password()),
        estimate: "Usually 5 to 15 minutes, depending on your connection.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete() -> Probe {
        Probe { has_command_line_tools: true, has_homebrew: true, has_node: true, has_pilot_tools: true, has_python: true }
    }

    #[test]
    fn a_ready_mac_needs_no_steps() {
        assert!(complete().steps().is_empty());
    }

    #[test]
    fn a_bare_mac_gets_every_step_in_dependency_order() {
        let bare = Probe { has_command_line_tools: false, has_homebrew: false, has_node: false, has_pilot_tools: false, has_python: false };
        let steps = bare.steps();
        assert_eq!(steps, [Step::CommandLineTools, Step::Homebrew, Step::Node, Step::Pilot, Step::Python]);
        assert!(plan_for(&steps).needs_password);
    }

    #[test]
    fn only_an_old_node_adds_only_node_and_asks_for_no_password() {
        let probe = Probe { has_node: false, ..complete() };
        let plan = plan_for(&probe.steps());
        assert_eq!(plan.steps.iter().map(|step| step.id.as_str()).collect::<Vec<_>>(), ["node"]);
        assert!(!plan.needs_password);
    }

    #[test]
    fn homebrew_is_not_installed_just_to_install_python() {
        let probe = Probe { has_homebrew: false, has_python: false, ..complete() };
        assert_eq!(probe.steps(), [Step::Python]);
    }
}
