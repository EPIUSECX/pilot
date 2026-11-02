use tauri::Url;

use crate::error::{AppError, AppResult, ErrorCategory};

pub const LAUNCHER_LABEL: &str = "main";

fn is_bundled_origin(url: &Url) -> bool {
    let host = url.host_str().unwrap_or_default();
    let bundled = (url.scheme() == "tauri" && host == "localhost") || (url.scheme() == "http" && host == "tauri.localhost");
    let development = cfg!(debug_assertions) && url.scheme() == "http" && host == "127.0.0.1" && url.port() == Some(1420);
    bundled || development
}

/// Native commands answer only the launcher window while it shows the bundled launcher.
pub fn authorize(label: &str, url: &Url) -> AppResult<()> {
    if label == LAUNCHER_LABEL && is_bundled_origin(url) {
        return Ok(());
    }
    Err(AppError::new(ErrorCategory::Unauthorized, "This window may not use desktop commands."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn the_bundled_launcher_is_allowed() {
        assert!(authorize("main", &url("tauri://localhost/")).is_ok());
        assert!(authorize("main", &url("http://tauri.localhost/index.html")).is_ok());
    }

    #[test]
    fn dashboards_and_other_windows_are_refused() {
        for label in ["dashboard-main-1a2b3c4d", "dashboard", "", "main2"] {
            assert!(authorize(label, &url("tauri://localhost/")).is_err(), "{label}");
        }
    }

    #[test]
    fn a_launcher_that_navigated_to_the_web_or_a_pilot_page_is_refused() {
        for address in ["http://127.0.0.1:7000/", "https://example.com/", "http://localhost/", "tauri://evil/", "file:///etc/passwd", "http://tauri.localhost.evil.com/"] {
            let error = authorize("main", &url(address)).unwrap_err();
            assert_eq!(error.category, ErrorCategory::Unauthorized, "{address}");
        }
    }
}
