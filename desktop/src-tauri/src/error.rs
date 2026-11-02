use serde::Serialize;

use crate::runtime::redact::redact;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    InvalidInput,
    NotConfigured,
    EnvironmentMissing,
    Conflict,
    Busy,
    UnsafeToStop,
    ConfirmationRequired,
    StartupFailed,
    Timeout,
    Unauthorized,
    NotFound,
    Io,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub category: ErrorCategory,
    pub message: String,
    pub stage: Option<String>,
    pub recovery: Option<String>,
    pub details: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self { category, message: message.into(), stage: None, recovery: None, details: None }
    }

    pub fn stage(mut self, stage: &str) -> Self {
        self.stage = Some(stage.to_string());
        self
    }

    pub fn recovery(mut self, recovery: &str) -> Self {
        self.recovery = Some(recovery.to_string());
        self
    }

    pub fn details(mut self, details: &str) -> Self {
        self.details = Some(redact(details));
        self
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new(ErrorCategory::Io, error.to_string())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}
