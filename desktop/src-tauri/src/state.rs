use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::contracts::EnvironmentReport;
use crate::environment::{detect, pilot_check, python_check};
use crate::error::{AppError, AppResult, ErrorCategory};
use crate::preferences::Preferences;
use crate::runtime::adapter::{InstalledPilot, PilotAdapter};
use crate::runtime::manager::{AdapterSource, Manager};
use crate::setup::SetupSession;

const CACHE_LIFETIME: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct AppPaths {
    pub support: PathBuf,
    pub logs: PathBuf,
    pub preferences: PathBuf,
    pub owned: PathBuf,
}

impl AppPaths {
    pub fn new(support: PathBuf, logs: PathBuf) -> Self {
        Self { preferences: support.join("preferences.json"), owned: support.join("owned-processes.json"), support, logs }
    }
}

/// Resolves the Pilot installation and interpreter from preferences, caching briefly.
pub struct Resolver {
    pub home: PathBuf,
    pub bridge: PathBuf,
    pub preferences: Arc<Mutex<Preferences>>,
    cache: Mutex<Option<(Instant, Arc<InstalledPilot>)>>,
}

impl Resolver {
    pub fn new(home: PathBuf, bridge: PathBuf, preferences: Arc<Mutex<Preferences>>) -> Self {
        Self { home, bridge, preferences, cache: Mutex::new(None) }
    }

    pub fn detect(&self) -> EnvironmentReport {
        let preferences = self.preferences.lock().expect("preferences").clone();
        let preferred = preferences.pilot_root.map(PathBuf::from);
        detect(preferred.as_deref(), preferences.editor, &self.home, |root, python| {
            InstalledPilot { root: root.to_path_buf(), python: python.to_path_buf(), bridge: self.bridge.clone(), home: self.home.clone() }
                .pilot_version()
        })
    }

    pub fn invalidate(&self) {
        *self.cache.lock().expect("cache") = None;
    }

    pub fn installed(&self) -> AppResult<Arc<InstalledPilot>> {
        if let Some((created, pilot)) = self.cache.lock().expect("cache").as_ref() {
            if created.elapsed() < CACHE_LIFETIME {
                return Ok(pilot.clone());
            }
        }
        let preferred = self.preferences.lock().expect("preferences").pilot_root.clone().map(PathBuf::from);
        let (_, root) = pilot_check(preferred.as_deref(), &self.home);
        let (_, python) = python_check(&self.home);
        let (Some(root), Some(python)) = (root, python) else {
            return Err(AppError::new(ErrorCategory::NotConfigured, "Pilot or a compatible Python was not found.")
                .recovery("Open Environment and follow the next action for each failed check."));
        };
        let pilot = Arc::new(InstalledPilot { root, python, bridge: self.bridge.clone(), home: self.home.clone() });
        *self.cache.lock().expect("cache") = Some((Instant::now(), pilot.clone()));
        Ok(pilot)
    }
}

impl AdapterSource for Resolver {
    fn current(&self) -> AppResult<Arc<dyn PilotAdapter>> {
        Ok(self.installed()? as Arc<dyn PilotAdapter>)
    }
}

pub struct AppState {
    pub manager: Arc<Manager>,
    pub preferences: Arc<Mutex<Preferences>>,
    pub paths: AppPaths,
    pub resolver: Arc<Resolver>,
    pub setup: Arc<SetupSession>,
    pub quit_confirmed: AtomicBool,
}
