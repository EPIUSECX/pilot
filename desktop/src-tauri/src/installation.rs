use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::error::{AppError, AppResult, ErrorCategory};

pub const INSTALLER_URL: &str = "https://raw.githubusercontent.com/frappe/pilot/develop/install.sh";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallationPlan {
    pub source_url: String,
    pub steps: Vec<String>,
    pub notes: Vec<String>,
}

/// Derived from the macOS branch of Pilot's install.sh, which reuses whatever is already installed.
pub fn plan() -> InstallationPlan {
    let steps = [
        "Download install.sh from the Pilot repository to Pilot Desktop's support folder and show its SHA-256.",
        "Install Homebrew only if `brew` is missing. Homebrew may ask for your password in Terminal.",
        "Install any of mariadb@11.8, postgresql@16, redis, nginx, certbot and Node.js that are missing. Installed packages are kept as they are.",
        "Install uv if missing.",
        "Fetch the latest Pilot release into ~/pilot, or update the git checkout there when it is a checkout.",
        "Add ~/pilot/bin to your shell PATH and create the Admin environment in ~/pilot/.admin-venv.",
    ];
    let notes = [
        "Nothing runs until you type yes in Terminal. Closing Terminal cancels it.",
        "The script is fetched from a branch, so its content can change. Review the downloaded file first.",
        "Existing benches, databases and credentials are not reset. Re-running Refresh afterwards continues from where it stopped.",
    ];
    InstallationPlan {
        source_url: INSTALLER_URL.into(),
        steps: steps.map(String::from).to_vec(),
        notes: notes.map(String::from).to_vec(),
    }
}

pub fn script(support_directory: &Path) -> String {
    let directory = support_directory.join("installer").display().to_string();
    let quoted = format!("'{}'", directory.replace('\'', "'\\''"));
    let plan = plan();
    let steps: String = plan.steps.iter().map(|step| format!("echo \"  - {step}\"\n")).collect();
    format!(
        r#"#!/bin/bash
set -u
URL="{url}"
DIR={quoted}
mkdir -p "$DIR"
FILE="$DIR/install.sh"
echo "Pilot Desktop guided installation"
echo "Source: $URL"
curl -fsSL --proto '=https' --tlsv1.2 "$URL" -o "$FILE" || {{ echo "Download failed."; read -r -p "Press Return to close." _; exit 1; }}
SUM=$(shasum -a 256 "$FILE" | cut -d' ' -f1)
echo "Saved to: $FILE"
echo "SHA-256:  $SUM"
echo
echo "This will:"
{steps}echo
read -r -p "Type yes to run the installer, anything else to cancel: " ANSWER
if [ "$ANSWER" != "yes" ]; then echo "Cancelled. Nothing was changed."; read -r -p "Press Return to close." _; exit 0; fi
sh "$FILE"
STATUS=$?
if [ $STATUS -eq 0 ]; then
  printf '{{"source":"%s","sha256":"%s","installedAt":"%s"}}\n' "$URL" "$SUM" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$DIR/installation.json"
  echo "Done. Return to Pilot Desktop and choose Refresh."
else
  echo "The installer stopped with code $STATUS. Fix the message above, then choose Refresh in Pilot Desktop to continue."
fi
read -r -p "Press Return to close." _
"#,
        url = INSTALLER_URL,
        quoted = quoted,
        steps = steps
    )
}

/// Hands the installation to Terminal so macOS handles any authorization prompt natively.
pub fn begin(support_directory: &Path) -> AppResult<()> {
    fs::create_dir_all(support_directory)?;
    let path = support_directory.join("install-pilot.command");
    fs::write(&path, script(support_directory))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    let status = Command::new("/usr/bin/open").args(["-a", "Terminal"]).arg(&path).status()?;
    if status.success() {
        return Ok(());
    }
    Err(AppError::new(ErrorCategory::EnvironmentMissing, "Could not open Terminal.")
        .recovery(&format!("Run {} yourself in Terminal.", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_downloads_to_a_file_and_waits_for_confirmation_before_running() {
        let text = script(Path::new("/Users/a b/Library/Application Support/Pilot Desktop"));
        assert!(text.contains("-o \"$FILE\"") && text.contains("shasum -a 256"));
        let confirm = text.find("Type yes").unwrap();
        let run = text.find("sh \"$FILE\"").unwrap();
        assert!(confirm < run);
        assert!(!text.contains("| sh") && !text.contains("| bash"));
        assert!(text.contains("DIR='/Users/a b/Library/Application Support/Pilot Desktop/installer'"));
    }

    #[test]
    fn a_quote_in_the_support_path_cannot_break_out_of_the_script() {
        let text = script(Path::new("/tmp/it's"));
        assert!(text.contains(r#"DIR='/tmp/it'\''s/installer'"#));
    }

    #[test]
    fn the_plan_states_what_is_reused() {
        let plan = plan();
        assert!(plan.steps.iter().any(|step| step.contains("kept as they are")));
        assert_eq!(plan.source_url, INSTALLER_URL);
    }
}
