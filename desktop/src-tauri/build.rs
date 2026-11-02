const COMMANDS: &[&str] = &[
    "detect_environment",
    "choose_pilot_installation",
    "plan_installation",
    "begin_installation",
    "get_runtime_info",
    "list_benches",
    "create_bench",
    "start_bench",
    "cancel_start",
    "stop_bench",
    "restart_bench",
    "open_dashboard",
    "reset_dashboard_session",
    "open_project",
    "read_logs",
    "get_preferences",
    "save_preferences",
    "quit_app",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("failed to run tauri-build");
}
