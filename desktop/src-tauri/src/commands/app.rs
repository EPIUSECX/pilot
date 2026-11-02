use std::sync::atomic::Ordering;

use tauri::{AppHandle, State, Webview};

use super::{authorize_webview, blocking};
use crate::error::AppResult;
use crate::state::AppState;

/// Leaves attached benches running. Fails, and keeps the app open, when an owned bench has active work.
#[tauri::command]
pub async fn quit_app(webview: Webview, app: AppHandle, state: State<'_, AppState>, stop_owned: bool) -> AppResult<Vec<String>> {
    authorize_webview(&webview)?;
    let mut notices = Vec::new();
    if stop_owned {
        let manager = state.manager.clone();
        notices = blocking(move || manager.shutdown_owned()).await?;
    }
    state.quit_confirmed.store(true, Ordering::SeqCst);
    app.exit(0);
    Ok(notices)
}
