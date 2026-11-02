use std::process::Command;

use serde::Deserialize;
use tauri::{State, Webview};

use super::{authorize_webview, blocking};
use crate::environment::application_path;
use crate::error::{AppError, AppResult, ErrorCategory};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectTarget {
    Editor,
    Finder,
}

#[tauri::command]
pub async fn open_project(webview: Webview, state: State<'_, AppState>, bench_id: String, target: ProjectTarget) -> AppResult<()> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    let editor = state.preferences.lock().expect("preferences").editor;
    let home = state.resolver.home.clone();
    blocking(move || {
        let bench = manager.find(&bench_id)?;
        if !bench.path.is_dir() {
            return Err(AppError::new(ErrorCategory::NotFound, "The bench folder no longer exists."));
        }
        let mut command = Command::new("/usr/bin/open");
        match target {
            ProjectTarget::Finder => command.arg("-R").arg(&bench.path),
            ProjectTarget::Editor => {
                let name = editor.application_name();
                if application_path(name, &home).is_none() {
                    return Err(AppError::new(ErrorCategory::EnvironmentMissing, format!("{name} is not installed."))
                        .recovery("Install it, or choose another editor in Preferences."));
                }
                command.arg("-a").arg(name).arg("--").arg(&bench.path)
            }
        };
        command.spawn()?;
        Ok(())
    })
    .await
}
