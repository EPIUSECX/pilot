use super::progress::Failure;
use super::steps::Step;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Network,
    DiskFull,
    Permission,
    ReleaseMissing,
    LocalChanges,
    StaleCheckout,
    Other,
}

const NETWORK: &[&str] = &[
    "could not resolve host",
    "curl: (6)",
    "curl: (7)",
    "curl: (28)",
    "curl: (35)",
    "curl: (56)",
    "failed to connect",
    "operation timed out",
    "network is unreachable",
    "connection reset",
    "temporary failure in name resolution",
    "ssl connect error",
];
const LOCAL_CHANGES: &[&str] = &[
    "would be overwritten by merge",
    "commit your changes or stash",
    "you have unmerged files",
    "already exists and is not an empty directory",
];
const STALE_CHECKOUT: &[&str] = &[
    "no such ref was fetched",
    "your configuration specifies to merge with the ref",
    "there is no tracking information",
];
const DISK: &[&str] = &["no space left on device", "enospc"];
const PERMISSION: &[&str] = &[
    "need sudo access",
    "a password is required",
    "incorrect password",
    "not in the sudoers",
    "needs to be an administrator",
    "sudo: no password was provided",
];

pub fn classify(output: &str) -> FailureKind {
    let text = output.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    if text.contains("could not find a pilot.tar.gz") {
        FailureKind::ReleaseMissing
    } else if has(LOCAL_CHANGES) {
        FailureKind::LocalChanges
    } else if has(STALE_CHECKOUT) {
        FailureKind::StaleCheckout
    } else if has(DISK) {
        FailureKind::DiskFull
    } else if has(PERMISSION) {
        FailureKind::Permission
    } else if has(NETWORK) {
        FailureKind::Network
    } else {
        FailureKind::Other
    }
}

/// Network trouble is the only failure worth repeating on its own; every step reuses what it already did.
pub fn is_retryable(kind: FailureKind) -> bool {
    kind == FailureKind::Network
}

pub fn explain(step: Step, kind: FailureKind, details: &str) -> Failure {
    let (title, message) = match kind {
        FailureKind::Network => (
            "Couldn't reach the internet",
            "Pilot Desktop couldn't download what it needs. Check your connection and try again. Setup continues from where it stopped.".to_string(),
        ),
        FailureKind::DiskFull => (
            "Your Mac is out of space",
            "Setup needs a few gigabytes free. Empty the Trash or remove files you don't need, then try again.".to_string(),
        ),
        FailureKind::Permission => (
            "Your Mac password is needed",
            "Setup needs an administrator account to add Homebrew. Try again and enter your Mac password when asked. If this Mac isn't yours to administer, ask whoever manages it.".to_string(),
        ),
        FailureKind::LocalChanges => (
            "Your Pilot folder has changes of its own",
            "Pilot Desktop left it untouched. You can move it aside, which deletes nothing, and install a fresh copy. Or choose \"I already have Pilot\" to use a folder you already work in.".to_string(),
        ),
        FailureKind::StaleCheckout => (
            "Your Pilot folder is out of date",
            "The Pilot folder in your home directory is an older developer copy that can no longer update itself. Pilot Desktop left it untouched. You can move it aside, which deletes nothing, and install a fresh copy. Or choose \"I already have Pilot\" to use a folder you already work in.".to_string(),
        ),
        FailureKind::ReleaseMissing | FailureKind::Other => (
            "Setup stopped",
            format!("Something went wrong while installing {}. Try again. If it keeps happening, copy the details and send them to the Pilot team.", step.label()),
        ),
    };
    let offers_move = matches!(kind, FailureKind::LocalChanges | FailureKind::StaleCheckout);
    Failure { title: title.into(), message, details: details.to_string(), recovery: offers_move.then(|| "move_aside".to_string()) }
}

fn friendly_package(name: &str) -> String {
    let base = name.split('@').next().unwrap_or(name);
    match base {
        "mariadb" => "MariaDB",
        "postgresql" => "PostgreSQL",
        "redis" => "Redis",
        "nginx" => "nginx",
        "certbot" => "certificate tools",
        "node" => "Node.js",
        "uv" => "Python tooling",
        _ => return name.to_string(),
    }
    .to_string()
}

/// A short, non-technical description of what an installer line is doing, if it says anything useful.
pub fn describe_activity(line: &str) -> Option<String> {
    let text = line.trim();
    if let Some(rest) = text.strip_prefix("==> ") {
        let mut words = rest.split_whitespace();
        return match (words.next(), words.next()) {
            (Some("Installing" | "Pouring" | "Fetching" | "Downloading"), Some(name)) if !name.starts_with("http") => {
                Some(format!("Installing {}", friendly_package(name.split("--").next().unwrap_or(name))))
            }
            _ => None,
        };
    }
    const KNOWN: &[(&str, &str)] = &[
        ("Installing Homebrew", "Installing Homebrew"),
        ("Fetching the latest pilot release", "Downloading Pilot"),
        ("Cloning pilot", "Downloading Pilot"),
        ("Updating pilot", "Updating Pilot"),
        ("Installing uv", "Installing Python tooling"),
        ("Setting up admin environment", "Preparing Pilot's dashboard"),
        ("Setting up your environment", "Setting up Pilot"),
        ("Installing Node.js", "Installing Node.js"),
    ];
    KNOWN.iter().find(|(prefix, _)| text.starts_with(prefix)).map(|(_, activity)| activity.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_failures_people_can_fix_themselves() {
        assert_eq!(classify("curl: (6) Could not resolve host: github.com"), FailureKind::Network);
        assert_eq!(classify("tar: Error: No space left on device"), FailureKind::DiskFull);
        assert_eq!(classify("Need sudo access on macOS (e.g. the user bob needs to be an Administrator)!"), FailureKind::Permission);
        assert_eq!(classify("Could not find a pilot.tar.gz release asset."), FailureKind::ReleaseMissing);
        assert_eq!(classify("error: Your local changes to the following files would be overwritten by merge:"), FailureKind::LocalChanges);
        assert_eq!(classify("Your configuration specifies to merge with the ref 'refs/heads/main'\nfrom the remote, but no such ref was fetched."), FailureKind::StaleCheckout);
        assert_eq!(classify("something unexpected"), FailureKind::Other);
    }

    #[test]
    fn only_network_failures_are_retried_automatically() {
        assert!(is_retryable(FailureKind::Network));
        for kind in [FailureKind::DiskFull, FailureKind::Permission, FailureKind::ReleaseMissing, FailureKind::LocalChanges, FailureKind::StaleCheckout, FailureKind::Other] {
            assert!(!is_retryable(kind));
        }
    }

    #[test]
    fn explanations_avoid_command_names_and_exit_codes() {
        for kind in [FailureKind::Network, FailureKind::DiskFull, FailureKind::Permission, FailureKind::LocalChanges, FailureKind::StaleCheckout, FailureKind::Other] {
            let text = explain(Step::Pilot, kind, "raw").message;
            assert!(!text.contains("brew ") && !text.contains("exit code") && !text.contains("sudo"), "{text}");
        }
    }

    #[test]
    fn describes_installer_activity_in_plain_words() {
        assert_eq!(describe_activity("==> Installing mariadb@11.8").as_deref(), Some("Installing MariaDB"));
        assert_eq!(describe_activity("==> Pouring redis--8.0.zip").as_deref(), Some("Installing Redis"));
        assert_eq!(describe_activity("Fetching the latest pilot release...").as_deref(), Some("Downloading Pilot"));
        assert_eq!(describe_activity("==> Downloading https://ghcr.io/x").as_deref(), None);
        assert_eq!(describe_activity("random noise"), None);
    }
}
