use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use tauri::{State, Webview};

use super::{authorize_webview, blocking};
use crate::error::AppResult;
use crate::setup::executor::ProcessExecutor;
use crate::setup::folder;
use crate::setup::machine::{needed_steps, Host};
use crate::setup::progress::{Failure, Phase, SetupProgress};
use crate::setup::runner::{Runner, Timings};
use crate::setup::steps::{plan_for, SetupPlan};
use crate::state::AppState;

fn host(state: &AppState) -> Arc<Host> {
    Arc::new(Host { home: state.resolver.home.clone(), resolver: state.resolver.clone() })
}

#[tauri::command]
pub async fn plan_setup(webview: Webview, state: State<'_, AppState>) -> AppResult<SetupPlan> {
    authorize_webview(&webview)?;
    let machine = host(&state);
    blocking(move || Ok(plan_for(&needed_steps(machine.as_ref())))).await
}

#[tauri::command]
pub async fn start_setup(webview: Webview, state: State<'_, AppState>) -> AppResult<()> {
    authorize_webview(&webview)?;
    let machine = host(&state);
    let probe = machine.clone();
    let steps = blocking(move || Ok(needed_steps(probe.as_ref()))).await?;
    let session = state.setup.clone();
    session.begin(&steps)?;
    let runner = Runner {
        session: session.clone(),
        executor: Arc::new(ProcessExecutor::new(session.cancel_flag())),
        machine,
        logs: state.manager.logs.clone(),
        support: state.paths.support.clone(),
        timings: Timings::default(),
    };
    let (resolver, manager) = (state.resolver.clone(), state.manager.clone());
    std::thread::spawn(move || {
        if catch_unwind(AssertUnwindSafe(|| runner.execute(&steps))).is_err() {
            session.update(|progress| {
                progress.phase = Phase::Failed;
                progress.failure = Some(Failure {
                    title: "Setup stopped".into(),
                    message: "Something unexpected went wrong. Try again.".into(),
                    details: String::new(),
                    recovery: None,
                });
            });
        }
        resolver.invalidate();
        manager.notify_changed();
    });
    Ok(())
}

#[tauri::command]
pub fn cancel_setup(webview: Webview, state: State<'_, AppState>) -> AppResult<()> {
    authorize_webview(&webview)?;
    state.setup.request_cancel();
    Ok(())
}

#[tauri::command]
pub fn get_setup_progress(webview: Webview, state: State<'_, AppState>) -> AppResult<SetupProgress> {
    authorize_webview(&webview)?;
    Ok(state.setup.snapshot())
}

/// Renames the Pilot folder in the home directory, after the launcher has asked the user to confirm.
#[tauri::command]
pub async fn move_pilot_aside(webview: Webview, state: State<'_, AppState>) -> AppResult<String> {
    authorize_webview(&webview)?;
    let home = state.resolver.home.clone();
    let moved = blocking(move || folder::move_aside(&home)).await?;
    state.resolver.invalidate();
    Ok(moved.display().to_string())
}
