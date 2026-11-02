use std::path::{Path, PathBuf};

use crate::contracts::{CheckStatus, DependencyCheck, EnvironmentReport};
use crate::preferences::Editor;
use crate::runtime::discovery::{
    command_version, find_executable, find_node, find_python, tool_directories, validate_pilot_root, Version,
    MINIMUM_PYTHON, REQUIRED_NODE_MAJOR,
};

fn show(version: Version) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

fn check(id: &str, label: &str, required: bool) -> DependencyCheck {
    DependencyCheck {
        id: id.into(),
        label: label.into(),
        status: CheckStatus::Missing,
        required,
        path: None,
        version: None,
        message: String::new(),
        next_action: None,
        details: None,
    }
}

fn found(mut item: DependencyCheck, path: &Path, version: Option<Version>, message: &str) -> DependencyCheck {
    item.status = CheckStatus::Ok;
    item.path = Some(path.display().to_string());
    item.version = version.map(show);
    item.message = message.into();
    item
}

fn missing(mut item: DependencyCheck, message: &str, action: &str) -> DependencyCheck {
    item.message = message.into();
    item.next_action = Some(action.into());
    item
}

pub fn pilot_check(preferred: Option<&Path>, home: &Path) -> (DependencyCheck, Option<PathBuf>) {
    let item = check("pilot", "Pilot installation", true);
    let mut candidates: Vec<PathBuf> = preferred.map(Path::to_path_buf).into_iter().collect();
    candidates.push(home.join("pilot"));
    let mut last_problem = None;
    for candidate in candidates.iter().filter(|path| path.exists()) {
        match validate_pilot_root(candidate) {
            Ok(root) => return (found(item, &root, None, "Pilot is installed."), Some(root)),
            Err(error) => last_problem = Some(error.message),
        }
    }
    let message = last_problem.unwrap_or_else(|| "No Pilot installation was found in your home folder.".into());
    let action = "Choose Install Pilot, or choose an existing Pilot folder.";
    (missing(item, &message, action), None)
}

pub fn python_check(home: &Path) -> (DependencyCheck, Option<PathBuf>) {
    let item = check("python", "Python for the Pilot command (host)", true);
    let (chosen, probed) = find_python(home);
    if let Some((path, version)) = chosen {
        let message = "Runs the pilot command. Each bench gets its own Python separately.";
        return (found(item, &path, Some(version), message), Some(path));
    }
    let action = "Install Python 3.12 or later, for example with `uv python install 3.12`, then choose Refresh.";
    match probed.iter().find_map(|(path, version)| version.map(|v| (path, v))) {
        Some((path, version)) => {
            let mut item = missing(item, &format!("Found Python {} but Pilot needs {} or later.", show(version), show(MINIMUM_PYTHON)), action);
            item.status = CheckStatus::Incompatible;
            item.path = Some(path.display().to_string());
            item.version = Some(show(version));
            (item, None)
        }
        None => (missing(item, "No Python interpreter was found.", action), None),
    }
}

fn tool(home: &Path, id: &str, label: &str, executable: &str, required: bool, version_arguments: &[&str], action: &str) -> DependencyCheck {
    let item = check(id, label, required);
    match find_executable(executable, &tool_directories(home)) {
        Some(path) => {
            let version = command_version(&path, version_arguments);
            found(item, &path, version, "Found.")
        }
        None => missing(item, &format!("{executable} was not found."), action),
    }
}

pub fn node_check(home: &Path) -> DependencyCheck {
    let item = check("node", "Node.js", true);
    let search = find_node(home);
    if let Some((path, version)) = search.usable {
        return found(item, &path, Some(version), "Found.");
    }
    let action = "Install Pilot adds Node.js 24 for you.";
    match search.highest {
        Some((path, version)) => {
            let mut item = missing(item, &format!("Found Node.js {}; Pilot needs {REQUIRED_NODE_MAJOR} or later to build benches.", show(version)), action);
            item.status = CheckStatus::Incompatible;
            item.path = Some(path.display().to_string());
            item.version = Some(show(version));
            item
        }
        None => missing(item, "node was not found.", action),
    }
}

fn database_service(home: &Path) -> DependencyCheck {
    let mut item = tool(home, "mariadb", "MariaDB", "mariadbd", false, &["--version"], "Pilot's installer adds MariaDB 11.8. Sites can also use SQLite.");
    if item.status == CheckStatus::Ok && !crate::runtime::port::is_listening_locally(3310) {
        item.status = CheckStatus::Stopped;
        item.message = "Installed. Pilot starts its own database server when a bench is initialized.".into();
    }
    item
}

pub fn editor_check(editor: Editor, home: &Path) -> DependencyCheck {
    let name = editor.application_name();
    let mut item = check("editor", &format!("{name} (editor shortcut)"), false);
    match application_path(name, home) {
        Some(path) => found(item, &path, None, "Open in editor is available."),
        None => {
            item.message = format!("{name} is not installed, so Open in editor is unavailable.");
            item.next_action = Some("Install it or pick another editor in Preferences.".into());
            item
        }
    }
}

pub fn application_path(name: &str, home: &Path) -> Option<PathBuf> {
    [PathBuf::from("/Applications"), home.join("Applications")]
        .into_iter()
        .map(|directory| directory.join(format!("{name}.app")))
        .find(|path| path.is_dir())
}

pub fn detect(preferred: Option<&Path>, editor: Editor, home: &Path, pilot_version: impl Fn(&Path, &Path) -> Option<String>) -> EnvironmentReport {
    let (pilot, root) = pilot_check(preferred, home);
    let (python, interpreter) = python_check(home);
    let version = root.as_ref().zip(interpreter.as_ref()).and_then(|(root, python)| pilot_version(root, python));
    let checks = vec![
        pilot,
        python,
        node_check(home),
        tool(home, "git", "Git", "git", true, &["--version"], "Install the Xcode command line tools with `xcode-select --install`."),
        tool(home, "uv", "uv", "uv", true, &["--version"], "Install Pilot adds uv for you."),
        tool(home, "redis", "Redis", "redis-server", true, &["--version"], "Install Pilot adds Redis for you."),
        database_service(home),
        tool(home, "homebrew", "Homebrew", "brew", false, &["--version"], "Install Pilot adds Homebrew when it is needed."),
        editor_check(editor, home),
    ];
    let is_ready = checks.iter().filter(|item| item.required).all(|item| item.status == CheckStatus::Ok);
    EnvironmentReport {
        checks,
        pilot_root: root.map(|path| path.display().to_string()),
        pilot_version: version,
        is_ready,
        architecture: std::env::consts::ARCH.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_missing_pilot_gets_a_next_action() {
        let home = tempfile::tempdir().unwrap();
        let (item, root) = pilot_check(None, home.path());
        assert_eq!((item.status, root), (CheckStatus::Missing, None));
        assert!(item.next_action.unwrap().contains("Install Pilot"));
    }

    #[test]
    fn an_incomplete_pilot_folder_is_distinguished_from_a_missing_one() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("pilot/pilot")).unwrap();
        let (item, root) = pilot_check(None, home.path());
        assert!(root.is_none() && item.message.contains("not a complete Pilot installation"));
    }

    #[test]
    fn a_complete_pilot_is_found_through_the_preferred_path_with_spaces() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("My Pilot");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("pilot")).unwrap();
        fs::write(root.join("bin/pilot"), "").unwrap();
        fs::write(root.join("pilot/__init__.py"), "").unwrap();
        let (item, found_root) = pilot_check(Some(&root), home.path());
        assert_eq!(item.status, CheckStatus::Ok);
        assert!(found_root.is_some());
    }
}
