use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::runtime::persistence::atomic_write;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Editor {
    #[default]
    Cursor,
    Vscode,
    Zed,
}

impl Editor {
    pub fn application_name(self) -> &'static str {
        match self {
            Editor::Cursor => "Cursor",
            Editor::Vscode => "Visual Studio Code",
            Editor::Zed => "Zed",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct UiState {
    pub last_page: Option<String>,
    pub last_bench: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub schema_version: u32,
    pub pilot_root: Option<String>,
    pub editor: Editor,
    pub notifications: bool,
    pub ui: UiState,
}

impl Default for Preferences {
    fn default() -> Self {
        Self { schema_version: SCHEMA_VERSION, pilot_root: None, editor: Editor::default(), notifications: true, ui: UiState::default() }
    }
}

/// The only fields the launcher may write. The Pilot location is set through a native chooser.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceUpdate {
    pub editor: Editor,
    pub notifications: bool,
    pub ui: UiState,
}

impl Preferences {
    pub fn apply(&mut self, update: PreferenceUpdate) {
        self.editor = update.editor;
        self.notifications = update.notifications;
        self.ui = update.ui;
    }
}

/// Loads tolerantly so an older file keeps its values. A corrupt file is kept aside, never silently overwritten.
pub fn load(path: &Path) -> Preferences {
    let Ok(bytes) = fs::read(path) else { return Preferences::default() };
    match serde_json::from_slice::<Preferences>(&bytes) {
        Ok(mut preferences) => {
            preferences.schema_version = SCHEMA_VERSION;
            preferences
        }
        Err(_) => {
            let _ = fs::rename(path, path.with_extension("json.corrupt"));
            Preferences::default()
        }
    }
}

pub fn save(path: &Path, preferences: &Preferences) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|e| crate::error::AppError::new(crate::error::ErrorCategory::Io, e.to_string()))?;
    atomic_write(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_file_keeps_its_values_and_gains_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        fs::write(&path, r#"{"pilotRoot": "/Users/a b/pilot", "editor": "vscode", "retiredSetting": true}"#).unwrap();
        let bench_file = directory.path().join("benches").join("main").join("bench.toml");
        fs::create_dir_all(bench_file.parent().unwrap()).unwrap();
        fs::write(&bench_file, "name = \"main\"").unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.pilot_root.as_deref(), Some("/Users/a b/pilot"));
        assert_eq!((loaded.editor, loaded.notifications, loaded.schema_version), (Editor::Vscode, true, SCHEMA_VERSION));
        save(&path, &loaded).unwrap();
        assert_eq!(load(&path), loaded);
        assert_eq!(fs::read_to_string(&bench_file).unwrap(), "name = \"main\"");
    }

    #[test]
    fn a_corrupt_file_is_preserved_for_inspection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        fs::write(&path, "{not json").unwrap();
        assert_eq!(load(&path), Preferences::default());
        assert_eq!(fs::read_to_string(directory.path().join("preferences.json.corrupt")).unwrap(), "{not json");
    }

    #[test]
    fn the_renderer_cannot_set_the_pilot_location() {
        let update: Result<PreferenceUpdate, _> = serde_json::from_str(r#"{"editor":"zed","notifications":false,"ui":{},"pilotRoot":"/etc"}"#);
        let mut preferences = Preferences { pilot_root: Some("/safe".into()), ..Preferences::default() };
        preferences.apply(update.unwrap());
        assert_eq!(preferences.pilot_root.as_deref(), Some("/safe"));
        assert_eq!(preferences.editor, Editor::Zed);
    }
}
