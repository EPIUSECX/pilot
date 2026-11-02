use tauri::{State, Webview};

use super::authorize_webview;
use crate::error::AppResult;
use crate::preferences::{save, PreferenceUpdate, Preferences};
use crate::state::AppState;

#[tauri::command]
pub fn get_preferences(webview: Webview, state: State<'_, AppState>) -> AppResult<Preferences> {
    authorize_webview(&webview)?;
    Ok(state.preferences.lock().expect("preferences").clone())
}

#[tauri::command]
pub fn save_preferences(webview: Webview, state: State<'_, AppState>, update: PreferenceUpdate) -> AppResult<Preferences> {
    authorize_webview(&webview)?;
    let mut preferences = state.preferences.lock().expect("preferences");
    let mut candidate = preferences.clone();
    candidate.apply(update);
    save(&state.paths.preferences, &candidate)?;
    *preferences = candidate.clone();
    Ok(candidate)
}
