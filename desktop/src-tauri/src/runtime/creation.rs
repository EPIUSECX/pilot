use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread;

use super::manager::Manager;
use super::output::{extract_generated_password, pump};
use crate::contracts::{CreateBenchRequest, OperationResult};
use crate::error::{AppError, AppResult, ErrorCategory};

pub const SUPPORTED_BRANCHES: [&str; 2] = ["version-16", "develop"];

pub fn validate_request(request: &CreateBenchRequest) -> AppResult<()> {
    let name = request.name.as_str();
    let valid_name = !name.is_empty()
        && name.len() <= 48
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid_name {
        return Err(AppError::new(
            ErrorCategory::InvalidInput,
            "Use 1 to 48 lowercase letters, digits, hyphens or underscores, starting with a letter or digit.",
        ));
    }
    if !SUPPORTED_BRANCHES.contains(&request.frappe_branch.as_str()) {
        return Err(AppError::new(ErrorCategory::InvalidInput, "Choose a supported Frappe branch."));
    }
    Ok(())
}

impl Manager {
    /// Runs `pilot new`. Existing benches are never touched; a duplicate name fails before any command runs.
    pub fn create(self: &Arc<Self>, request: &CreateBenchRequest) -> AppResult<OperationResult> {
        validate_request(request)?;
        let _guard = self.exclusive(&format!("create:{}", request.name))?;
        let adapter = self.adapter()?;
        if adapter.list()?.iter().any(|target| target.name == request.name) {
            return Err(AppError::new(ErrorCategory::Conflict, format!("A bench named '{}' already exists.", request.name))
                .stage("validation")
                .recovery("Pick another name. The existing bench was not changed."));
        }
        let mut command = adapter.create_command(&request.name, &request.frappe_branch);
        let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|error| {
            AppError::new(ErrorCategory::EnvironmentMissing, format!("Could not launch Pilot: {error}")).stage("launch")
        })?;
        let password = Arc::new(Mutex::new(None::<String>));
        let readers: Vec<_> = [
            child.stdout.take().map(|out| (Box::new(out) as Box<dyn std::io::Read + Send>, "stdout")),
            child.stderr.take().map(|err| (Box::new(err) as Box<dyn std::io::Read + Send>, "stderr")),
        ]
        .into_iter()
        .flatten()
        .map(|(reader, stream)| {
            let (logs, name, password) = (self.logs.clone(), request.name.clone(), password.clone());
            thread::spawn(move || {
                pump(reader, &name, stream, &logs, |raw| {
                    if let Some(found) = extract_generated_password(raw) {
                        *password.lock().expect("password") = Some(found);
                    }
                })
            })
        })
        .collect();
        let status = child.wait()?;
        readers.into_iter().for_each(|reader| {
            let _ = reader.join();
        });
        if !status.success() {
            return Err(AppError::new(ErrorCategory::StartupFailed, format!("Pilot could not create '{}'.", request.name))
                .stage("create")
                .recovery("Read the details, fix the cause, then try again. Nothing existing was removed.")
                .details(&self.tail(&request.name)));
        }
        let mut result = OperationResult::success(format!("Created '{}'.", request.name), None);
        result.generated_admin_password = password.lock().expect("password").take();
        Ok(result)
    }
}
