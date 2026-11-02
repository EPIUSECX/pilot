mod common;

use std::time::Duration;

use common::*;
use pilot_desktop_lib::contracts::{CreateBenchRequest, Ownership, Status, TaskActivity};
use pilot_desktop_lib::error::ErrorCategory;
use pilot_desktop_lib::runtime::manager::Timeouts;

#[test]
fn starts_an_owned_bench_in_a_path_with_spaces_and_stops_it() {
    let harness = Harness::fast();
    let bench = harness.add_bench("main");
    let started = harness.manager.start("main").unwrap();
    let summary = started.bench.unwrap();
    assert_eq!((summary.status, summary.ownership), (Status::Running, Ownership::Owned));
    assert_eq!(summary.dashboard_url.as_deref(), Some(format!("http://127.0.0.1:{}", bench.admin_port).as_str()));
    assert!(bench.path.display().to_string().contains(' '));

    let stopped = harness.manager.stop("main", false).unwrap();
    assert!(stopped.notices.is_empty());
    wait_for(|| !listening(bench.admin_port), "port to close");
    assert_eq!(harness.manager.summary_for("main").unwrap().status, Status::Stopped);
}

#[test]
fn start_attaches_to_a_running_bench_without_a_duplicate_launch() {
    let mut harness = Harness::fast();
    let bench = harness.add_bench("main");
    harness.start_externally(&bench);
    assert_eq!(harness.spawn_count(&bench), 1);

    let summary = harness.manager.start("main").unwrap().bench.unwrap();
    assert_eq!((summary.status, summary.ownership), (Status::Running, Ownership::Attached));
    harness.manager.start("main").unwrap();
    assert_eq!(harness.spawn_count(&bench), 1, "no second launch");
}

#[test]
fn an_unrelated_service_on_the_port_is_a_conflict_and_nothing_is_launched() {
    let mut harness = Harness::fast();
    let bench = harness.add_bench("main");
    harness.start_unrelated(bench.admin_port);

    let error = harness.manager.start("main").unwrap_err();
    assert_eq!(error.category, ErrorCategory::Conflict);
    assert_eq!(harness.spawn_count(&bench), 0);
    let summary = harness.manager.summary_for("main").unwrap();
    assert_eq!(summary.status, Status::Failed);
    assert!(harness.external_alive(0), "the unrelated service keeps running");
}

#[test]
fn stopping_an_attached_bench_needs_confirmation_and_leaves_others_running() {
    let mut harness = Harness::fast();
    let target = harness.add_bench("attached");
    let other = harness.add_bench("other");
    harness.start_externally(&target);
    harness.start_externally(&other);

    let refused = harness.manager.stop("attached", false).unwrap_err();
    assert_eq!(refused.category, ErrorCategory::ConfirmationRequired);
    assert!(listening(target.admin_port));

    harness.manager.stop("attached", true).unwrap();
    wait_for(|| !listening(target.admin_port), "attached bench to stop");
    assert!(listening(other.admin_port), "the second runtime is untouched");
}

#[test]
fn quit_cleanup_stops_owned_benches_and_spares_attached_and_unrelated_ones() {
    let mut harness = Harness::fast();
    let owned = harness.add_bench("owned");
    let attached = harness.add_bench("attached");
    harness.start_externally(&attached);
    harness.start_unrelated(free_port());
    harness.manager.start("owned").unwrap();
    assert_eq!(harness.manager.owned_benches(), vec!["owned".to_string()]);

    harness.manager.shutdown_owned().unwrap();
    wait_for(|| !listening(owned.admin_port), "owned bench to stop");
    assert!(listening(attached.admin_port));
    assert!(harness.external_alive(1));
    assert!(harness.manager.owned_benches().is_empty());
}

#[test]
fn active_work_blocks_stop_and_restart() {
    let harness = Harness::fast();
    let bench = harness.add_bench("main");
    harness.manager.start("main").unwrap();
    *harness.adapter.activity.lock().unwrap() = TaskActivity { active: true, uncertain: false, queued_tasks: 1, running_tasks: 1 };

    let error = harness.manager.stop("main", false).unwrap_err();
    assert_eq!(error.category, ErrorCategory::UnsafeToStop);
    assert!(listening(bench.admin_port));
    assert_eq!(harness.manager.restart("main", false).unwrap_err().category, ErrorCategory::UnsafeToStop);
    assert_eq!(harness.spawn_count(&bench), 1);

    *harness.adapter.activity.lock().unwrap() = TaskActivity::default();
    harness.manager.restart("main", false).unwrap();
    assert_eq!(harness.spawn_count(&bench), 2);
}

#[test]
fn a_stubborn_owned_bench_is_force_stopped_with_an_explanation() {
    let harness = Harness::new(Timeouts { start: Duration::from_secs(20), stop_grace: Duration::from_millis(800), poll: Duration::from_millis(100) });
    *harness.adapter.mode.lock().unwrap() = "stubborn".into();
    let bench = harness.add_bench("main");
    harness.manager.start("main").unwrap();
    let sleeper = read_pid(&bench.path.join("child.pid"));

    let result = harness.manager.stop("main", false).unwrap();
    assert!(result.notices.iter().any(|notice| notice.contains("force-stopped")));
    wait_for(|| !listening(bench.admin_port) && !pid_alive(sleeper), "owned processes to die");
}

#[test]
fn a_start_that_never_listens_times_out_cleans_up_and_can_be_retried() {
    let harness = Harness::new(Timeouts { start: Duration::from_millis(1500), stop_grace: Duration::from_secs(2), poll: Duration::from_millis(100) });
    *harness.adapter.mode.lock().unwrap() = "slow".into();
    let bench = harness.add_bench("main");

    let error = harness.manager.start("main").unwrap_err();
    assert_eq!(error.category, ErrorCategory::Timeout);
    assert_eq!(error.stage.as_deref(), Some("readiness"));
    let sleeper = read_pid(&bench.path.join("child.pid"));
    wait_for(|| !pid_alive(sleeper), "timed-out bench processes to be stopped");
    assert!(harness.manager.owned_benches().is_empty());

    let summary = harness.manager.summary_for("main").unwrap();
    assert_eq!((summary.status, summary.ownership), (Status::Failed, Ownership::None));
    *harness.adapter.mode.lock().unwrap() = "normal".into();
    assert_eq!(harness.manager.start("main").unwrap().bench.unwrap().status, Status::Running);
}

#[test]
fn a_crash_during_startup_reports_the_stage_and_sanitized_details() {
    let harness = Harness::fast();
    *harness.adapter.mode.lock().unwrap() = "crash".into();
    harness.add_bench("main");

    let error = harness.manager.start("main").unwrap_err();
    assert_eq!(error.category, ErrorCategory::StartupFailed);
    assert_eq!(error.stage.as_deref(), Some("startup"));
    let details = error.details.unwrap_or_default();
    assert!(details.contains("boom") && !details.contains("hunter2"));
    assert!(harness.manager.owned_benches().is_empty());
}

#[test]
fn setup_link_tokens_are_kept_in_memory_and_never_logged() {
    let harness = Harness::fast();
    harness.add_bench("main");
    harness.manager.start("main").unwrap();
    wait_for(|| harness.manager.logs.recent("main").iter().any(|line| line.text.contains("sid=[redacted]")), "redacted setup line");

    let url = harness.manager.take_setup_url("main").unwrap();
    assert!(url.starts_with("http://127.0.0.1:") && url.ends_with("sid=SECRETTOKEN"));
    assert!(harness.manager.take_setup_url("main").is_none(), "consumed once");
    assert!(!format!("{:?}", harness.manager.logs.recent("main")).contains("SECRETTOKEN"));
    harness.manager.stop("main", false).unwrap();
}

#[test]
fn a_second_concurrent_operation_is_refused_not_queued() {
    let harness = Harness::new(Timeouts { start: Duration::from_secs(5), stop_grace: Duration::from_secs(1), poll: Duration::from_millis(100) });
    *harness.adapter.mode.lock().unwrap() = "slow".into();
    harness.add_bench("main");
    let manager = harness.manager.clone();
    let first = std::thread::spawn(move || manager.start("main"));
    wait_for(|| harness.manager.summary_for("main").map(|s| s.status == Status::Starting).unwrap_or(false), "starting state");
    assert_eq!(harness.manager.start("main").unwrap_err().category, ErrorCategory::Busy);
    harness.manager.cancel_start("main");
    assert!(first.join().unwrap().is_err());
}

#[test]
fn create_validates_input_rejects_duplicates_and_returns_the_password_once() {
    let harness = Harness::fast();
    harness.add_bench("existing");
    let request = |name: &str| CreateBenchRequest { name: name.into(), frappe_branch: "version-16".into() };

    for bad in ["", "Bad Name", "evil; rm -rf ~", "-x", "UPPER"] {
        assert_eq!(harness.manager.create(&request(bad)).unwrap_err().category, ErrorCategory::InvalidInput, "{bad}");
    }
    let bad_branch = CreateBenchRequest { name: "ok".into(), frappe_branch: "version-9; ls".into() };
    assert_eq!(harness.manager.create(&bad_branch).unwrap_err().category, ErrorCategory::InvalidInput);
    assert_eq!(harness.manager.create(&request("existing")).unwrap_err().category, ErrorCategory::Conflict);

    let created = harness.manager.create(&request("fresh")).unwrap();
    assert_eq!(created.generated_admin_password.as_deref(), Some("Gen3rated!Pass"));
    assert!(!format!("{:?}", harness.manager.logs.recent("fresh")).contains("Gen3rated"));
    assert!(harness.adapter.benches.join("fresh").is_dir() && harness.adapter.benches.join("existing").is_dir());
}
