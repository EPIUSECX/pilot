pub mod access;
pub mod commands;
pub mod contracts;
pub mod dashboard;
pub mod environment;
pub mod error;
pub mod preferences;
pub mod runtime;
pub mod setup;
pub mod state;
pub mod tray;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager as _, RunEvent, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use access::LAUNCHER_LABEL;
use runtime::logs::LogBuffer;
use runtime::manager::{Manager, Timeouts};
use state::{AppPaths, AppState, Resolver};

const REFRESH_INTERVAL: Duration = Duration::from_secs(10);

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LAUNCHER_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn bridge_path(app: &AppHandle) -> PathBuf {
    let bundled = app.path().resource_dir().ok().map(|dir| dir.join("bridge").join("pilot_desktop_bridge.py"));
    match bundled {
        Some(path) if path.is_file() => path,
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("bridge").join("pilot_desktop_bridge.py"),
    }
}

fn build_state(app: &AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let paths = AppPaths::new(app.path().app_data_dir()?, app.path().app_log_dir()?);
    let preferences = Arc::new(Mutex::new(preferences::load(&paths.preferences)));
    let home = app.path().home_dir()?;
    let resolver = Arc::new(Resolver::new(home, bridge_path(app), preferences.clone()));
    let logs = Arc::new(LogBuffer::new(paths.logs.clone()));
    let manager = Manager::new(resolver.clone(), logs, paths.owned.clone(), Timeouts::default());
    Ok(AppState { manager, preferences, paths, resolver, setup: Default::default(), quit_confirmed: Default::default() })
}

fn connect_events(app: &AppHandle, state: &AppState) {
    let handle = app.clone();
    state.manager.logs.set_listener(Arc::new(move |line| {
        let _ = handle.emit_to(LAUNCHER_LABEL, "diagnostics://log", line);
    }));
    let handle = app.clone();
    state.setup.set_listener(Arc::new(move |progress| {
        let _ = handle.emit_to(LAUNCHER_LABEL, "setup://progress", progress);
    }));
    let (handle, manager) = (app.clone(), state.manager.clone());
    state.manager.set_on_change(Arc::new(move || {
        if let Ok(summaries) = manager.summaries() {
            tray::refresh(&handle, &summaries);
            let _ = handle.emit_to(LAUNCHER_LABEL, "benches://changed", &summaries);
        }
    }));
    let (handle, preferences) = (app.clone(), state.preferences.clone());
    state.manager.set_on_failure(Arc::new(move |bench, message| {
        if preferences.lock().expect("preferences").notifications {
            let _ = handle.notification().builder().title(format!("{bench} stopped")).body(message).show();
        }
    }));
}

/// Re-probes every few seconds. This also covers wake from sleep: stale status is corrected, nothing restarts.
fn spawn_refresh_loop(manager: Arc<Manager>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(REFRESH_INTERVAL);
        manager.notify_changed();
    });
}

pub fn request_quit(app: &AppHandle, state: &AppState) {
    let owned = state.manager.owned_benches();
    if owned.is_empty() {
        state.quit_confirmed.store(true, Ordering::SeqCst);
        app.exit(0);
        return;
    }
    show_main_window(app);
    let _ = app.emit_to(LAUNCHER_LABEL, "app://quit-requested", owned);
}

pub fn run() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_main_window(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = build_state(&handle)?;
            let leftovers = state.manager.reconcile_on_launch();
            for bench in leftovers {
                state.manager.logs.push(&bench, "launcher", "A process from a previous run is still alive. It is shown as attached and was not touched.");
            }
            connect_events(&handle, &state);
            spawn_refresh_loop(state.manager.clone());
            app.manage(state);
            tray::install(&handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == LAUNCHER_LABEL {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::environment::detect_environment,
            commands::environment::choose_pilot_installation,
            commands::environment::get_runtime_info,
            commands::benches::list_benches,
            commands::benches::create_bench,
            commands::benches::start_bench,
            commands::benches::cancel_start,
            commands::benches::stop_bench,
            commands::benches::restart_bench,
            commands::benches::open_dashboard,
            commands::benches::reset_dashboard_session,
            commands::project::open_project,
            commands::diagnostics::read_logs,
            commands::setup::plan_setup,
            commands::setup::start_setup,
            commands::setup::cancel_setup,
            commands::setup::get_setup_progress,
            commands::setup::move_pilot_aside,
            commands::settings::get_preferences,
            commands::settings::save_preferences,
            commands::app::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Pilot Desktop");

    application.run(|app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            let state = app.state::<AppState>();
            if !state.quit_confirmed.load(Ordering::SeqCst) {
                api.prevent_exit();
                request_quit(app, &state);
            }
        }
        RunEvent::Reopen { .. } => show_main_window(app),
        _ => {}
    });
}
