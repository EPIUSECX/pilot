use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager as _};

use crate::contracts::{BenchSummary, Ownership, Status};
use crate::dashboard;
use crate::state::AppState;

const TRAY_ID: &str = "pilot-desktop";

pub fn status_line(summaries: &[BenchSummary]) -> String {
    let running = summaries.iter().filter(|summary| summary.status == Status::Running).count();
    match running {
        0 => "No benches running".to_string(),
        1 => "1 bench running".to_string(),
        count => format!("{count} benches running"),
    }
}

fn build_menu(app: &AppHandle, summaries: &[BenchSummary]) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "status", status_line(summaries), false, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "open-main", "Open Pilot Desktop", true, None::<&str>)?)?;
    let running: Vec<&BenchSummary> = summaries.iter().filter(|summary| summary.status == Status::Running).collect();
    if !running.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    for summary in running {
        let origin = if summary.ownership == Ownership::Owned { "started here" } else { "attached" };
        let label = format!("Open {} ({origin})", summary.name);
        menu.append(&MenuItem::with_id(app, format!("bench:{}", summary.id), label, true, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit Pilot Desktop", true, Some("Cmd+Q"))?)?;
    Ok(menu)
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, &[])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID).menu(&menu).tooltip("Pilot Desktop").show_menu_on_left_click(true);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            match id.as_str() {
                "open-main" => crate::show_main_window(app),
                "quit" => crate::request_quit(app, &app.state::<AppState>()),
                other => {
                    if let Some(bench) = other.strip_prefix("bench:") {
                        open_bench(app, bench.to_string());
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn open_bench(app: &AppHandle, bench: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        if let Ok(summary) = state.manager.summary_for(&bench) {
            if let (Status::Running, Some(address)) = (summary.status, summary.dashboard_url.clone()) {
                let _ = dashboard::open(&app, &summary.name, &summary.path, &address);
            }
        }
    });
}

pub fn refresh(app: &AppHandle, summaries: &[BenchSummary]) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app, summaries) {
            let _ = tray.set_menu(Some(menu));
        }
        let _ = tray.set_tooltip(Some(format!("Pilot Desktop: {}", status_line(summaries))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_counts_only_running_benches() {
        assert_eq!(status_line(&[]), "No benches running");
    }
}
