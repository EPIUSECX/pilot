use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::exec::run_capture;
use crate::error::{AppError, AppResult, ErrorCategory};

pub type Version = (u32, u32, u32);
pub const MINIMUM_PYTHON: Version = (3, 11, 0);
pub const REQUIRED_NODE_MAJOR: u32 = 24;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn validate_pilot_root(path: &Path) -> AppResult<PathBuf> {
    let root = fs::canonicalize(path).map_err(|_| {
        AppError::new(ErrorCategory::NotFound, format!("{} does not exist.", path.display()))
    })?;
    let has_entry_point = root.join("bin").join("pilot").is_file();
    let has_package = root.join("pilot").join("__init__.py").is_file();
    if has_entry_point && has_package {
        return Ok(root);
    }
    Err(AppError::new(ErrorCategory::NotConfigured, format!("{} is not a complete Pilot installation.", root.display()))
        .recovery("Choose the folder that contains bin/pilot, or choose Install Pilot."))
}

pub fn parse_version(text: &str) -> Option<Version> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let mut numbers = text[start..]
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()?
        .split('.')
        .map(|part| part.parse::<u32>().ok());
    let major = numbers.next()??;
    let minor = numbers.next().flatten().unwrap_or(0);
    let patch = numbers.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

pub fn command_version(executable: &Path, arguments: &[&str]) -> Option<Version> {
    let output = run_capture(Command::new(executable).args(arguments), PROBE_TIMEOUT).ok()?;
    parse_version(&format!("{}{}", output.stdout, output.stderr))
}

fn existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|path| path.is_dir()).collect()
}

fn child_directories(parent: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(parent)
        .map(|entries| entries.flatten().map(|entry| entry.path().join(suffix)).collect())
        .unwrap_or_default();
    found.sort();
    found.reverse();
    found
}

/// Node versions kept by version managers, newest first. The version comes from the folder name.
fn node_installations(parent: &Path, suffix: &str) -> Vec<(Version, PathBuf)> {
    let entries = fs::read_dir(parent).map(|entries| entries.flatten().collect::<Vec<_>>()).unwrap_or_default();
    let mut found: Vec<(Version, PathBuf)> = entries
        .iter()
        .filter_map(|entry| Some((parse_version(&entry.file_name().to_string_lossy())?, entry.path().join(suffix))))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
}

/// Directories a Pilot child may use. A Node that meets the minimum always comes first.
pub fn tool_directories(home: &Path) -> Vec<PathBuf> {
    let mut managed = node_installations(&home.join(".nvm/versions/node"), "bin");
    managed.extend(node_installations(&home.join(".local/share/fnm/node-versions"), "installation/bin"));
    managed.sort_by(|a, b| b.0.cmp(&a.0));
    let (current, older): (Vec<_>, Vec<_>) = managed.into_iter().partition(|(version, _)| version.0 >= REQUIRED_NODE_MAJOR);
    let mut directories = vec![PathBuf::from("/opt/homebrew/opt/node@24/bin")];
    directories.extend(current.into_iter().map(|(_, directory)| directory));
    directories.extend(["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"].map(PathBuf::from));
    directories.extend([home.join(".local/bin"), home.join(".volta/bin")]);
    directories.extend(older.into_iter().map(|(_, directory)| directory));
    directories.extend(child_directories(&home.join(".local/share/uv/python"), "bin"));
    directories.extend(["/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(PathBuf::from));
    existing(directories)
}

pub fn controlled_path(home: &Path) -> String {
    let joined: Vec<String> = tool_directories(home).iter().map(|dir| dir.display().to_string()).collect();
    joined.join(":")
}

pub fn find_executable(name: &str, directories: &[PathBuf]) -> Option<PathBuf> {
    directories.iter().map(|dir| dir.join(name)).find(|path| path.is_file())
}

/// Every Node found on the tool path, and the best one that meets the minimum.
pub struct NodeSearch {
    pub usable: Option<(PathBuf, Version)>,
    pub highest: Option<(PathBuf, Version)>,
}

pub fn find_node(home: &Path) -> NodeSearch {
    let mut found: Vec<(PathBuf, Version)> = tool_directories(home)
        .iter()
        .map(|directory| directory.join("node"))
        .filter(|path| path.is_file())
        .filter_map(|path| command_version(&path, &["--version"]).map(|version| (path, version)))
        .collect();
    found.sort_by(|a, b| b.1.cmp(&a.1));
    let usable = found.iter().find(|(_, version)| version.0 >= REQUIRED_NODE_MAJOR).cloned();
    NodeSearch { usable, highest: found.into_iter().next() }
}

pub fn python_candidates(home: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for directory in tool_directories(home) {
        for name in ["python3.14", "python3.13", "python3.12", "python3.11", "python3"] {
            let path = directory.join(name);
            if path.is_file() && !candidates.contains(&path) {
                candidates.push(path);
            }
        }
    }
    candidates
}

/// First interpreter at or above the minimum, plus every probed candidate for diagnostics.
pub fn find_python(home: &Path) -> (Option<(PathBuf, Version)>, Vec<(PathBuf, Option<Version>)>) {
    let probed: Vec<(PathBuf, Option<Version>)> = python_candidates(home)
        .into_iter()
        .map(|path| {
            let version = command_version(&path, &["--version"]);
            (path, version)
        })
        .collect();
    let chosen = probed
        .iter()
        .filter_map(|(path, version)| version.filter(|v| *v >= MINIMUM_PYTHON).map(|v| (path.clone(), v)))
        .next();
    (chosen, probed)
}

/// The only environment a Pilot child sees: verified directories and the loopback bind request.
pub fn child_environment(home: &Path, admin_host: &str) -> Vec<(String, String)> {
    let mut environment = vec![
        ("HOME".to_string(), home.display().to_string()),
        ("PATH".to_string(), controlled_path(home)),
        ("LANG".to_string(), "en_US.UTF-8".to_string()),
        ("PILOT_ADMIN_HOST".to_string(), admin_host.to_string()),
    ];
    for key in ["USER", "LOGNAME", "TMPDIR"] {
        if let Ok(value) = std::env::var(key) {
            environment.push((key.to_string(), value));
        }
    }
    environment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("Python 3.12.13"), Some((3, 12, 13)));
        assert_eq!(parse_version("v24.1.0\n"), Some((24, 1, 0)));
        assert_eq!(parse_version("git version 2.50.1 (Apple Git-155)"), Some((2, 50, 1)));
        assert_eq!(parse_version("no digits"), None);
        assert!(parse_version("Python 3.9.6").unwrap() < MINIMUM_PYTHON);
    }

    #[test]
    fn accepts_a_pilot_root_with_spaces_and_rejects_partial_installs() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("My Pilot");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("pilot")).unwrap();
        assert!(validate_pilot_root(&root).is_err());
        fs::write(root.join("bin/pilot"), "").unwrap();
        fs::write(root.join("pilot/__init__.py"), "").unwrap();
        assert!(validate_pilot_root(&root).is_ok());
        let missing = validate_pilot_root(&directory.path().join("absent")).unwrap_err();
        assert_eq!(missing.category, ErrorCategory::NotFound);
    }

    #[test]
    fn a_current_node_comes_before_older_ones_whatever_the_folder_names_sort_as() {
        let home = tempfile::tempdir().unwrap();
        for version in ["v9.11.2", "v22.23.1", "v24.1.0", "v25.0.0"] {
            fs::create_dir_all(home.path().join(".nvm/versions/node").join(version).join("bin")).unwrap();
        }
        let names: Vec<String> = tool_directories(home.path())
            .iter()
            .filter_map(|path| path.parent()?.file_name()?.to_str().map(String::from))
            .filter(|name| name.starts_with('v'))
            .collect();
        assert_eq!(names, ["v25.0.0", "v24.1.0", "v22.23.1", "v9.11.2"]);
    }

    #[test]
    fn child_environment_is_explicit() {
        let home = tempfile::tempdir().unwrap();
        let environment = child_environment(home.path(), "127.0.0.1");
        let keys: Vec<&str> = environment.iter().map(|(key, _)| key.as_str()).collect();
        assert!(keys.contains(&"PILOT_ADMIN_HOST") && keys.contains(&"PATH"));
        assert!(!keys.contains(&"SSH_AUTH_SOCK"));
    }
}
