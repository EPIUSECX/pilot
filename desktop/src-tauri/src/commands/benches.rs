use tauri::{AppHandle, State, Webview};

use super::{authorize_webview, blocking};
use crate::contracts::{BenchSummary, CreateBenchRequest, OperationResult, Status};
use crate::dashboard;
use crate::error::{AppError, AppResult, ErrorCategory};
use crate::state::AppState;

#[tauri::command]
pub async fn list_benches(webview: Webview, state: State<'_, AppState>) -> AppResult<Vec<BenchSummary>> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    blocking(move || manager.summaries()).await
}

#[tauri::command]
pub async fn create_bench(webview: Webview, state: State<'_, AppState>, request: CreateBenchRequest) -> AppResult<OperationResult> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    let result = blocking({
        let manager = manager.clone();
        move || manager.create(&request)
    })
    .await?;
    manager.notify_changed();
    Ok(result)
}

#[tauri::command]
pub async fn start_bench(webview: Webview, state: State<'_, AppState>, bench_id: String) -> AppResult<OperationResult> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    blocking(move || manager.start(&bench_id)).await
}

#[tauri::command]
pub fn cancel_start(webview: Webview, state: State<'_, AppState>, bench_id: String) -> AppResult<()> {
    authorize_webview(&webview)?;
    state.manager.cancel_start(&bench_id);
    Ok(())
}

#[tauri::command]
pub async fn stop_bench(webview: Webview, app: AppHandle, state: State<'_, AppState>, bench_id: String, confirm_attached: bool) -> AppResult<OperationResult> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    let name = bench_id.clone();
    let result = blocking(move || manager.stop(&bench_id, confirm_attached)).await?;
    dashboard::close(&app, &name);
    Ok(result)
}

#[tauri::command]
pub async fn restart_bench(webview: Webview, state: State<'_, AppState>, bench_id: String, confirm_attached: bool) -> AppResult<OperationResult> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    blocking(move || manager.restart(&bench_id, confirm_attached)).await
}

/// Opens the dashboard only for a bench whose identity was just verified as running.
#[tauri::command]
pub async fn open_dashboard(webview: Webview, app: AppHandle, state: State<'_, AppState>, bench_id: String) -> AppResult<()> {
    authorize_webview(&webview)?;
    let manager = state.manager.clone();
    let summary = blocking({
        let (manager, bench_id) = (manager.clone(), bench_id.clone());
        move || manager.summary_for(&bench_id)
    })
    .await?;
    let address = match (summary.status, summary.dashboard_url.clone()) {
        (Status::Running, Some(address)) => manager.take_setup_url(&bench_id).unwrap_or(address),
        _ => {
            return Err(AppError::new(ErrorCategory::Conflict, "The bench is not ready, so its dashboard was not opened.")
                .recovery("Start the bench and wait until it shows Running."))
        }
    };
    dashboard::open(&app, &summary.name, &summary.path, &address)
}

#[tauri::command]
pub fn reset_dashboard_session(webview: Webview, app: AppHandle, bench_id: String) -> AppResult<()> {
    authorize_webview(&webview)?;
    dashboard::reset_session(&app, &bench_id)
}
