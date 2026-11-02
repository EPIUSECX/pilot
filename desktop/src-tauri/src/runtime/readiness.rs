use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::port::listening_pids;
use super::process::ProcessTable;
use crate::contracts::BenchTarget;
use crate::error::AppResult;

/// The Admin API lives under this prefix. Other paths serve the dashboard page, which answers 200 to everything.
const API_PREFIX: &str = "/api/v1";
const WIZARD_MODULE: &str = "admin.backend.run_server";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpProbe {
    pub health_ok: bool,
    pub bootstrap_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Free,
    Starting { pid: i32 },
    Running { anchor: Option<i32> },
    Conflict(String),
}

pub struct ProbeInputs<'a> {
    pub name: &'a str,
    pub path: &'a Path,
    pub port: u16,
    pub listeners: &'a [i32],
    pub http: &'a HttpProbe,
    pub table: &'a ProcessTable,
    pub supervisor: Option<i32>,
    pub spawned: Option<i32>,
}

fn is_alive(table: &ProcessTable, pid: Option<i32>) -> Option<i32> {
    pid.filter(|pid| table.get(*pid).is_some())
}

fn is_wizard_of(command: &str, path: &Path) -> bool {
    command.contains(WIZARD_MODULE) && command.contains(&path.display().to_string())
}

/// Port ownership, process ancestry and HTTP identity must all agree. Anything else is a conflict.
pub fn classify(inputs: &ProbeInputs) -> Verdict {
    let supervisor = is_alive(inputs.table, inputs.supervisor);
    let spawned = is_alive(inputs.table, inputs.spawned);
    let anchors: Vec<i32> = [spawned, supervisor].into_iter().flatten().collect();

    if inputs.listeners.is_empty() {
        return match anchors.first() {
            Some(pid) => Verdict::Starting { pid: *pid },
            None => Verdict::Free,
        };
    }
    let mut anchor = None;
    for listener in inputs.listeners {
        let lineage = anchors.iter().copied().find(|a| inputs.table.is_self_or_descendant(*listener, *a));
        let wizard = inputs.table.get(*listener).is_some_and(|entry| is_wizard_of(&entry.command, inputs.path));
        if lineage.is_some() || wizard {
            anchor = lineage.or(Some(*listener));
        }
    }
    let Some(anchor) = anchor else {
        let command = inputs.table.get(inputs.listeners[0]).map(|e| e.command.as_str()).unwrap_or("unknown process");
        return Verdict::Conflict(format!("Port {} is already used by another process ({}).", inputs.port, short(command)));
    };
    if !inputs.http.health_ok {
        return Verdict::Starting { pid: anchor };
    }
    match &inputs.http.bootstrap_name {
        Some(name) if name == inputs.name => Verdict::Running { anchor: Some(anchor) },
        Some(other) => Verdict::Conflict(format!("Port {} answers as bench '{other}', not '{}'.", inputs.port, inputs.name)),
        None => Verdict::Starting { pid: anchor },
    }
}

fn short(command: &str) -> String {
    command.chars().take(80).collect()
}

pub fn probe_http(port: u16) -> HttpProbe {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(1500)))
        .http_status_as_error(false)
        .build()
        .into();
    let fetch = |route: &str| -> Option<Value> {
        let mut response = agent.get(&format!("http://127.0.0.1:{port}{API_PREFIX}{route}")).call().ok()?;
        if response.status().as_u16() != 200 {
            return None;
        }
        serde_json::from_str(&response.body_mut().read_to_string().ok()?).ok()
    };
    let health_ok = fetch("/health").is_some_and(|body| body["status"] == "ok");
    let bootstrap_name = if health_ok {
        fetch("/bootstrap").and_then(|body| body["name"].as_str().map(str::to_string))
    } else {
        None
    };
    HttpProbe { health_ok, bootstrap_name }
}

pub fn read_supervisor_pid(bench_path: &Path, table: &ProcessTable) -> Option<i32> {
    let pid: i32 = fs::read_to_string(bench_path.join("pids").join("bench.pid")).ok()?.trim().parse().ok()?;
    let command = &table.get(pid)?.command;
    (command.contains("pilot") && command.contains("start")).then_some(pid)
}

pub fn probe(target: &BenchTarget, spawned: Option<i32>) -> AppResult<Verdict> {
    probe_in(target, spawned, &ProcessTable::snapshot()?)
}

pub fn probe_in(target: &BenchTarget, spawned: Option<i32>, table: &ProcessTable) -> AppResult<Verdict> {
    let listeners = listening_pids(target.admin_port)?;
    let http = if listeners.is_empty() { HttpProbe::default() } else { probe_http(target.admin_port) };
    let supervisor = read_supervisor_pid(&target.path, table);
    let inputs = ProbeInputs {
        name: &target.name,
        path: &target.path,
        port: target.admin_port,
        listeners: &listeners,
        http: &http,
        table,
        supervisor,
        spawned,
    };
    Ok(classify(&inputs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::process::ProcessEntry;

    fn entry(pid: i32, parent: i32, command: &str) -> ProcessEntry {
        ProcessEntry { pid, parent, started: "Sun Nov 1 21:00:00 2026".into(), command: command.into() }
    }

    fn table() -> ProcessTable {
        ProcessTable::from_entries(vec![
            entry(100, 1, "python /Users/a b/pilot/bin/pilot -b main start"),
            entry(101, 100, "python -m admin.backend.run_server --bench-root /Users/a b/pilot/benches/main --port 7000"),
            entry(300, 1, "node some-other-server.js"),
            entry(400, 1, "python -m admin.backend.run_server --bench-root /Users/a b/pilot/benches/wizard --port 7001 --wizard"),
        ])
    }

    fn inputs<'a>(listeners: &'a [i32], http: &'a HttpProbe, table: &'a ProcessTable, path: &'a Path, supervisor: Option<i32>) -> ProbeInputs<'a> {
        ProbeInputs { name: "main", path, port: 7000, listeners, http, table, supervisor, spawned: None }
    }

    fn ok(name: &str) -> HttpProbe {
        HttpProbe { health_ok: true, bootstrap_name: Some(name.into()) }
    }

    #[test]
    fn no_listener_and_no_supervisor_is_free() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/main");
        assert_eq!(classify(&inputs(&[], &HttpProbe::default(), &table, path, None)), Verdict::Free);
        assert_eq!(classify(&inputs(&[], &HttpProbe::default(), &table, path, Some(100))), Verdict::Starting { pid: 100 });
    }

    #[test]
    fn matching_supervisor_lineage_and_name_is_running() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/main");
        let verdict = classify(&inputs(&[101], &ok("main"), &table, path, Some(100)));
        assert_eq!(verdict, Verdict::Running { anchor: Some(100) });
    }

    #[test]
    fn an_unrelated_service_on_the_port_is_a_conflict_even_if_it_answers_health() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/main");
        let verdict = classify(&inputs(&[300], &ok("main"), &table, path, Some(100)));
        assert!(matches!(verdict, Verdict::Conflict(message) if message.contains("another process")));
    }

    #[test]
    fn another_benchs_identity_is_a_conflict() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/main");
        let verdict = classify(&inputs(&[101], &ok("other"), &table, path, Some(100)));
        assert!(matches!(verdict, Verdict::Conflict(message) if message.contains("'other'")));
    }

    #[test]
    fn a_wizard_server_is_recognised_by_its_bench_root() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/wizard");
        let wizard_http = ok("wizard");
        let mut probe_inputs = inputs(&[400], &wizard_http, &table, path, None);
        probe_inputs.name = "wizard";
        assert_eq!(classify(&probe_inputs), Verdict::Running { anchor: Some(400) });
        let wrong_bench = Path::new("/Users/a b/pilot/benches/main");
        let main_http = ok("main");
        let mut other = inputs(&[400], &main_http, &table, wrong_bench, None);
        other.name = "main";
        assert!(matches!(classify(&other), Verdict::Conflict(_)));
    }

    #[test]
    fn a_listener_that_does_not_answer_yet_is_starting() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/main");
        let verdict = classify(&inputs(&[101], &HttpProbe::default(), &table, path, Some(100)));
        assert_eq!(verdict, Verdict::Starting { pid: 100 });
    }

    #[test]
    fn a_stale_pid_file_for_a_dead_process_is_ignored() {
        let table = table();
        let path = Path::new("/Users/a b/pilot/benches/main");
        assert_eq!(classify(&inputs(&[], &HttpProbe::default(), &table, path, Some(999))), Verdict::Free);
    }

    #[test]
    fn a_page_that_answers_200_to_every_path_is_not_a_healthy_admin() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = "<!doctype html><html><body><div id=\"app\"></div></body></html>";
                let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = std::io::Write::write_all(&mut stream, reply.as_bytes());
            }
        });
        let probe = probe_http(port);
        assert!(!probe.health_ok && probe.bootstrap_name.is_none());
    }
}
