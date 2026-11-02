use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// `sudo -A` runs this to ask for the password in a native dialog. The password goes from the
/// dialog to sudo and never passes through Pilot Desktop.
const SCRIPT: &str = r#"#!/bin/sh
exec /usr/bin/osascript -e 'text returned of (display dialog "Pilot Desktop needs your Mac password to add Homebrew, the tool that installs software for Pilot." default answer "" with hidden answer with title "Pilot Desktop" buttons {"Cancel", "Continue"} default button "Continue" cancel button "Cancel" giving up after 300)'
"#;

pub fn write(support_directory: &Path) -> AppResult<PathBuf> {
    let directory = support_directory.join("setup");
    fs::create_dir_all(&directory)?;
    let path = directory.join("askpass.sh");
    fs::write(&path, SCRIPT)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_is_private_to_the_user_and_hides_what_is_typed() {
        let support = tempfile::tempdir().unwrap();
        let path = write(support.path()).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o700);
        assert!(fs::read_to_string(&path).unwrap().contains("with hidden answer"));
    }
}
