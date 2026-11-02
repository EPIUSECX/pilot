use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State, Webview};
use tauri_plugin_dialog::DialogExt;

use super::{authorize_webview, blocking};
use crate::contracts::EnvironmentReport;
use crate::environment::detect;
use crate::error::{AppError, AppResult, ErrorCategory};
use crate::installation::{self, InstallationPlan};
use crate::preferences::save;
use crate::runtime::adapter::InstalledPilot;
use crate::runtime::discovery::validate_pilot_root;
use crate::state::AppState;

async fn detect_with(state: &AppState) -> AppResult<EnvironmentReport> {
    let preferences = state.preferences.lock().expect("preferences").clone();
    let (home, bridge) = (state.resolver.home.clone(), state.resolver.bridge.clone());
    blocking(move || {
        let preferred = preferences.pilot_root.map(PathBuf::from);
        let pilot_home = home.clone();
        Ok(detect(preferred.as_deref(), preferences.editor, &home, |root, python| {
            InstalledPilot { root: root.to_path_buf(), python: python.to_path_buf(), bridge: bridge.clone(), home: pilot_home.clone() }
                .pilot_version()
        }))
    })
    .await
}

#[tauri::command]
pub async fn detect_environment(webview: Webview, state: State<'_, AppState>) -> AppResult<EnvironmentReport> {
    authorize_webview(&webview)?;
    detect_with(&state).await
}

/// The folder comes from a native chooser and is validated here; the launcher never supplies a path.
#[tauri::command]
pub async fn choose_pilot_installation(webview: Webview, app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<EnvironmentReport>> {
    authorize_webview(&webview)?;
    let picked = blocking(move || Ok(app.dialog().file().set_title("Choose the Pilot folder (contains bin/pilot)").blocking_pick_folder())).await?;
    let Some(folder) = picked else { return Ok(None) };
    let path = folder.into_path().map_err(|_| AppError::new(ErrorCategory::InvalidInput, "That location is not a folder."))?;
    let root = validate_pilot_root(&path)?;
    {
        let mut preferences = state.preferences.lock().expect("preferences");
        preferences.pilot_root = Some(root.display().to_string());
        save(&state.paths.preferences, &preferences)?;
    }
    state.resolver.invalidate();
    state.manager.notify_changed();
    detect_with(&state).await.map(Some)
}

#[tauri::command]
pub fn plan_installation(webview: Webview) -> AppResult<InstallationPlan> {
    authorize_webview(&webview)?;
    Ok(installation::plan())
}

#[tauri::command]
pub async fn begin_installation(webview: Webview, state: State<'_, AppState>) -> AppResult<()> {
    authorize_webview(&webview)?;
    let support = state.paths.support.clone();
    blocking(move || installation::begin(&support)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub version: String,
    pub architecture: String,
    pub minimum_macos: String,
}

#[tauri::command]
pub fn get_runtime_info(webview: Webview) -> AppResult<RuntimeInfo> {
    authorize_webview(&webview)?;
    Ok(RuntimeInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        architecture: std::env::consts::ARCH.into(),
        minimum_macos: "14.0".into(),
    })
}
