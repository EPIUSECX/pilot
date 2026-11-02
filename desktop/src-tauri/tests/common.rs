#![allow(dead_code)]

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pilot_desktop_lib::contracts::{BenchTarget, TaskActivity};
use pilot_desktop_lib::error::AppResult;
use pilot_desktop_lib::runtime::adapter::PilotAdapter;
use pilot_desktop_lib::runtime::logs::LogBuffer;
use pilot_desktop_lib::runtime::manager::{AdapterSource, Manager, Timeouts};

/// Lifecycle tests probe the shared process table and loopback ports, so they run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

pub struct FakeAdapter {
    pub root: PathBuf,
    pub benches: PathBuf,
    pub targets: Mutex<Vec<BenchTarget>>,
    pub activity: Mutex<TaskActivity>,
    pub mode: Mutex<String>,
}

impl FakeAdapter {
    fn command(&self, target: Option<&BenchTarget>, arguments: &[&str]) -> Command {
        let mut command = Command::new("/usr/bin/python3");
        command.arg(self.root.join("bin").join("pilot")).args(arguments).current_dir(&self.root);
        command.env("FAKE_BENCHES", &self.benches).env("FAKE_MODE", self.mode.lock().unwrap().as_str());
        if let Some(target) = target {
            command.env("FAKE_PATH", &target.path).env("FAKE_PORT", target.admin_port.to_string());
        }
        command
    }
}

impl PilotAdapter for FakeAdapter {
    fn list(&self) -> AppResult<Vec<BenchTarget>> {
        Ok(self.targets.lock().unwrap().clone())
    }
    fn activity(&self, _: &BenchTarget) -> AppResult<TaskActivity> {
        Ok(*self.activity.lock().unwrap())
    }
    fn start_command(&self, bench: &BenchTarget) -> Command {
        self.command(Some(bench), &["-b", &bench.name, "start"])
    }
    fn stop_command(&self, bench: &BenchTarget) -> Command {
        self.command(Some(bench), &["-b", &bench.name, "stop"])
    }
    fn create_command(&self, name: &str, _branch: &str) -> Command {
        self.command(None, &["new", name])
    }
}

struct Fixed(Arc<FakeAdapter>);
impl AdapterSource for Fixed {
    fn current(&self) -> AppResult<Arc<dyn PilotAdapter>> {
        Ok(self.0.clone())
    }
}

pub struct Harness {
    _serial: std::sync::MutexGuard<'static, ()>,
    pub directory: tempfile::TempDir,
    pub adapter: Arc<FakeAdapter>,
    pub manager: Arc<Manager>,
    external: Vec<Child>,
}

/// Ports come from a range below the OS ephemeral range, so another test's outgoing probe can never
/// take a port that has been handed out but not bound yet. Each port is issued once per process.
pub fn free_port() -> u16 {
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let base = 20_000 + (std::process::id() % 400) as u16 * 40;
    loop {
        let port = base + NEXT.fetch_add(1, Ordering::SeqCst);
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}

impl Harness {
    pub fn new(timeouts: Timeouts) -> Self {
        let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("Pilot Root");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_pilot.py"), root.join("bin/pilot")).unwrap();
        let benches = root.join("My Benches");
        fs::create_dir_all(&benches).unwrap();
        let adapter = Arc::new(FakeAdapter {
            root,
            benches,
            targets: Mutex::new(Vec::new()),
            activity: Mutex::new(TaskActivity::default()),
            mode: Mutex::new("normal".into()),
        });
        let logs = Arc::new(LogBuffer::new(directory.path().join("logs")));
        let manager = Manager::new(Arc::new(Fixed(adapter.clone())), logs, directory.path().join("owned.json"), timeouts);
        Self { _serial, directory, adapter, manager, external: Vec::new() }
    }

    pub fn fast() -> Self {
        Self::new(Timeouts { start: Duration::from_secs(20), stop_grace: Duration::from_secs(3), poll: Duration::from_millis(100) })
    }

    pub fn add_bench(&self, name: &str) -> BenchTarget {
        let path = self.adapter.benches.join(name);
        fs::create_dir_all(&path).unwrap();
        let target = BenchTarget {
            name: name.into(),
            path,
            is_production: false,
            admin_enabled: true,
            admin_port: free_port(),
            is_initialized: true,
            frappe_branch: Some("version-16".into()),
            read_error: None,
        };
        self.adapter.targets.lock().unwrap().push(target.clone());
        target
    }

    pub fn spawn_count(&self, target: &BenchTarget) -> usize {
        fs::read_to_string(target.path.join("spawns.log")).map(|text| text.lines().count()).unwrap_or(0)
    }

    /// A bench started outside the manager, as from a terminal.
    pub fn start_externally(&mut self, target: &BenchTarget) {
        let mut command = self.adapter.start_command(target);
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0);
        self.external.push(command.spawn().unwrap());
        wait_for(|| listening(target.admin_port), "external bench to listen");
    }

    pub fn start_unrelated(&mut self, port: u16) {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unrelated.py");
        let child = Command::new("/usr/bin/python3").arg(script).arg(port.to_string()).stdout(Stdio::null()).spawn().unwrap();
        self.external.push(child);
        wait_for(|| listening(port), "unrelated service to listen");
    }

    pub fn external_alive(&mut self, index: usize) -> bool {
        self.external[index].try_wait().unwrap().is_none()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for target in self.adapter.targets.lock().unwrap().iter() {
            for file in ["pids/bench.pid", "child.pid"] {
                if let Ok(text) = fs::read_to_string(target.path.join(file)) {
                    if let Ok(pid) = text.trim().parse::<i32>() {
                        unsafe { libc::kill(pid, libc::SIGKILL) };
                    }
                }
            }
        }
        for child in &mut self.external {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_millis(200)).is_ok()
}

pub fn wait_for(mut condition: impl FnMut() -> bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

pub fn pid_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

pub fn read_pid(path: &Path) -> i32 {
    fs::read_to_string(path).unwrap().trim().parse().unwrap()
}
