use tauri::{State, Webview};

use super::authorize_webview;
use crate::error::AppResult;
use crate::runtime::logs::LogLine;
use crate::state::AppState;

/// Backlog of already-redacted lines. New lines arrive on the `diagnostics://log` event.
#[tauri::command]
pub fn read_logs(webview: Webview, state: State<'_, AppState>, bench_id: String) -> AppResult<Vec<LogLine>> {
    authorize_webview(&webview)?;
    Ok(state.manager.logs.recent(&bench_id))
}
