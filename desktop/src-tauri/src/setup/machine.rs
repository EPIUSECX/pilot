use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::steps::{command_line_tools_installed, Probe, Step};
use crate::contracts::EnvironmentReport;
use crate::runtime::discovery::{controlled_path, find_executable, find_node, tool_directories};
use crate::state::Resolver;

/// What setup asks of this Mac. Tests replace it so no real software is touched.
pub trait Machine: Send + Sync {
    fn report(&self) -> EnvironmentReport;
    fn has_command_line_tools(&self) -> bool;
    fn has_node(&self) -> bool;
    /// True when the installer's folder is a git checkout, which a release download must never overwrite.
    fn has_development_checkout(&self) -> bool;
    fn tool(&self, name: &str) -> Option<PathBuf>;
    fn environment(&self, askpass: Option<&Path>) -> Vec<(String, String)>;
}

pub fn needed_steps(machine: &dyn Machine) -> Vec<Step> {
    Probe::from_report(&machine.report(), machine.has_command_line_tools()).steps()
}

pub struct Host {
    pub home: PathBuf,
    pub resolver: Arc<Resolver>,
}

impl Machine for Host {
    fn report(&self) -> EnvironmentReport {
        self.resolver.detect()
    }

    fn has_command_line_tools(&self) -> bool {
        command_line_tools_installed()
    }

    fn has_node(&self) -> bool {
        find_node(&self.home).usable.is_some()
    }

    fn has_development_checkout(&self) -> bool {
        self.home.join("pilot").join(".git").exists()
    }

    fn tool(&self, name: &str) -> Option<PathBuf> {
        find_executable(name, &tool_directories(&self.home))
    }

    /// Installers see only verified tool folders plus the settings they need to run without a terminal.
    fn environment(&self, askpass: Option<&Path>) -> Vec<(String, String)> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut environment = vec![
            ("HOME".to_string(), self.home.display().to_string()),
            ("PATH".to_string(), controlled_path(&self.home)),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
            ("SHELL".to_string(), shell),
            ("NONINTERACTIVE".to_string(), "1".to_string()),
            ("HOMEBREW_NO_ENV_HINTS".to_string(), "1".to_string()),
            ("HOMEBREW_NO_INSTALL_CLEANUP".to_string(), "1".to_string()),
        ];
        for key in ["USER", "LOGNAME", "TMPDIR"] {
            if let Ok(value) = std::env::var(key) {
                environment.push((key.to_string(), value));
            }
        }
        if let Some(path) = askpass {
            environment.push(("SUDO_ASKPASS".to_string(), path.display().to_string()));
        }
        environment
    }
}
