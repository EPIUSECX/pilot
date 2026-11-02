use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult, ErrorCategory};

fn has_benches(folder: &Path) -> bool {
    let Ok(entries) = fs::read_dir(folder.join("benches")) else { return false };
    entries.flatten().any(|entry| entry.path().join("bench.toml").is_file())
}

fn free_name(home: &Path) -> PathBuf {
    let first = home.join("pilot (previous)");
    if !first.exists() {
        return first;
    }
    (2..).map(|number| home.join(format!("pilot (previous {number})"))).find(|candidate| !candidate.exists()).expect("endless range")
}

/// Renames the Pilot folder in the home directory. Nothing is deleted, and a folder that holds benches is never moved.
pub fn move_aside(home: &Path) -> AppResult<PathBuf> {
    let folder = home.join("pilot");
    if !folder.is_dir() {
        return Err(AppError::new(ErrorCategory::NotFound, "There is no Pilot folder in your home directory."));
    }
    if has_benches(&folder) {
        return Err(AppError::new(ErrorCategory::Conflict, "That Pilot folder contains benches, so Pilot Desktop won't move it.")
            .recovery("Choose I already have Pilot and pick that folder instead."));
    }
    let target = free_name(home);
    fs::rename(&folder, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_with_pilot() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("pilot")).unwrap();
        fs::write(home.path().join("pilot/keep.txt"), "mine").unwrap();
        home
    }

    #[test]
    fn the_folder_is_renamed_with_its_contents_intact() {
        let home = home_with_pilot();
        let moved = move_aside(home.path()).unwrap();
        assert_eq!(moved, home.path().join("pilot (previous)"));
        assert_eq!(fs::read_to_string(moved.join("keep.txt")).unwrap(), "mine");
        assert!(!home.path().join("pilot").exists());
    }

    #[test]
    fn an_earlier_moved_folder_is_never_replaced() {
        let home = home_with_pilot();
        fs::create_dir_all(home.path().join("pilot (previous)")).unwrap();
        assert_eq!(move_aside(home.path()).unwrap(), home.path().join("pilot (previous 2)"));
    }

    #[test]
    fn a_folder_with_benches_is_left_where_it_is() {
        let home = home_with_pilot();
        fs::create_dir_all(home.path().join("pilot/benches/main")).unwrap();
        fs::write(home.path().join("pilot/benches/main/bench.toml"), "").unwrap();
        assert_eq!(move_aside(home.path()).unwrap_err().category, ErrorCategory::Conflict);
        assert!(home.path().join("pilot/keep.txt").exists());
    }

    #[test]
    fn a_missing_folder_is_reported_not_created() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(move_aside(home.path()).unwrap_err().category, ErrorCategory::NotFound);
    }
}
