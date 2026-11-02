use std::future::Future;

use tauri::{Runtime, Webview};

use crate::access::authorize;
use crate::error::{AppError, AppResult, ErrorCategory};

pub mod app;
pub mod benches;
pub mod diagnostics;
pub mod environment;
pub mod project;
pub mod settings;

pub fn authorize_webview<R: Runtime>(webview: &Webview<R>) -> AppResult<()> {
    let url = webview.url().map_err(|_| AppError::new(ErrorCategory::Unauthorized, "The calling page is unknown."))?;
    authorize(webview.label(), &url)
}

/// Runs blocking work (process and network probes) off the UI thread.
pub fn blocking<T: Send + 'static>(work: impl FnOnce() -> AppResult<T> + Send + 'static) -> impl Future<Output = AppResult<T>> {
    async move {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|error| AppError::new(ErrorCategory::Io, error.to_string()))?
    }
}
