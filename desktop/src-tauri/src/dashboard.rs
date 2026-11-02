use std::path::PathBuf;
use std::process::Command;

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{AppHandle, Manager as _, Url, WebviewUrl, WebviewWindowBuilder};

use crate::error::{AppError, AppResult, ErrorCategory};

pub fn window_label(bench: &str) -> String {
    let readable: String = bench.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    format!("dashboard-{readable}-{:08x}", fnv(bench.as_bytes(), 0xcbf29ce484222325) as u32)
}

fn fnv(bytes: &[u8], seed: u64) -> u64 {
    bytes.iter().fold(seed, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3))
}

/// One WebKit data store per bench path, so bench A's cookies never reach bench B.
pub fn data_store_identifier(bench_path: &str) -> [u8; 16] {
    let first = fnv(bench_path.as_bytes(), 0xcbf29ce484222325).to_be_bytes();
    let second = fnv(bench_path.as_bytes(), 0x84222325cbf29ce4).to_be_bytes();
    let mut identifier = [0u8; 16];
    identifier[..8].copy_from_slice(&first);
    identifier[8..].copy_from_slice(&second);
    identifier
}

pub fn is_same_origin(candidate: &Url, expected: &Url) -> bool {
    candidate.scheme() == expected.scheme() && candidate.host_str() == expected.host_str() && candidate.port_or_known_default() == expected.port_or_known_default()
}

pub fn open_in_system_browser(url: &Url) {
    if matches!(url.scheme(), "http" | "https") {
        let _ = Command::new("/usr/bin/open").arg(url.as_str()).spawn();
    }
}

pub fn download_destination(file_name: &str) -> Option<PathBuf> {
    let safe: String = file_name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
    let safe = safe.trim_start_matches('.');
    if safe.is_empty() {
        return None;
    }
    let directory = PathBuf::from(std::env::var_os("HOME")?).join("Downloads");
    let mut candidate = directory.join(safe);
    let mut counter = 1;
    while candidate.exists() {
        candidate = directory.join(format!("{counter}-{safe}"));
        counter += 1;
    }
    Some(candidate)
}

pub fn open(app: &AppHandle, bench: &str, bench_path: &str, address: &str) -> AppResult<()> {
    let label = window_label(bench);
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let url = Url::parse(address).map_err(|_| AppError::new(ErrorCategory::InvalidInput, "The dashboard address is invalid."))?;
    let (navigation_origin, window_origin, download_origin) = (url.clone(), url.clone(), url.clone());
    WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title(format!("Pilot · {bench}"))
        .inner_size(1280.0, 820.0)
        .min_inner_size(900.0, 600.0)
        .data_store_identifier(data_store_identifier(bench_path))
        .on_navigation(move |target| {
            let allowed = is_same_origin(target, &navigation_origin);
            if !allowed {
                open_in_system_browser(target);
            }
            allowed
        })
        .on_new_window(move |target, _| {
            if is_same_origin(&target, &window_origin) {
                return NewWindowResponse::Allow;
            }
            open_in_system_browser(&target);
            NewWindowResponse::Deny
        })
        .on_download(move |_, event| match event {
            DownloadEvent::Requested { url, destination } if is_same_origin(&url, &download_origin) => {
                let name = url.path_segments().and_then(|mut parts| parts.next_back()).unwrap_or("download").to_string();
                match download_destination(&name) {
                    Some(path) => {
                        *destination = path;
                        true
                    }
                    None => false,
                }
            }
            DownloadEvent::Requested { .. } => false,
            _ => true,
        })
        .build()
        .map_err(|error| AppError::new(ErrorCategory::Io, format!("Could not open the dashboard window: {error}")))?;
    Ok(())
}

pub fn close(app: &AppHandle, bench: &str) {
    if let Some(window) = app.get_webview_window(&window_label(bench)) {
        let _ = window.close();
    }
}

pub fn reset_session(app: &AppHandle, bench: &str) -> AppResult<()> {
    let Some(window) = app.get_webview_window(&window_label(bench)) else {
        return Err(AppError::new(ErrorCategory::NotFound, "Open the dashboard first, then reset its session."));
    };
    window.clear_all_browsing_data().map_err(|error| AppError::new(ErrorCategory::Io, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn each_bench_gets_its_own_label_and_data_store() {
        assert_ne!(window_label("a"), window_label("b"));
        assert_ne!(data_store_identifier("/p/a"), data_store_identifier("/p/b"));
        assert_eq!(data_store_identifier("/p/a"), data_store_identifier("/p/a"));
        assert!(window_label("we ird/name").chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn only_the_exact_dashboard_origin_stays_in_the_window() {
        let dashboard = url("http://127.0.0.1:7000/");
        assert!(is_same_origin(&url("http://127.0.0.1:7000/sites"), &dashboard));
        for other in ["http://127.0.0.1:7001/", "http://localhost:7000/", "https://127.0.0.1:7000/", "https://frappe.io/docs"] {
            assert!(!is_same_origin(&url(other), &dashboard), "{other}");
        }
    }

    #[test]
    fn download_names_cannot_escape_the_downloads_folder() {
        for hostile in ["../../etc/passwd", "..", ".", "/", ""] {
            let destination = download_destination(hostile);
            assert!(destination.is_none() || destination.as_ref().unwrap().parent().unwrap().ends_with("Downloads"), "{hostile}");
        }
        assert!(download_destination("backup 1.sql.gz").unwrap().ends_with("backup1.sql.gz"));
    }
}
