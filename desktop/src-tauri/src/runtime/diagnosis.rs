use std::path::Path;

/// What a bench's own output says went wrong, in words a person can act on.
#[derive(Debug, PartialEq, Eq)]
pub struct Diagnosis {
    pub message: String,
    pub recovery: String,
}

fn missing_module(line: &str) -> Option<&str> {
    let rest = line.split("No module named '").nth(1)?;
    rest.split('\'').next()
}

pub fn diagnose(lines: &[&str], bench: &Path) -> Option<Diagnosis> {
    let environment = bench.join("env/bin/python").display().to_string();
    for line in lines.iter().rev() {
        if line.contains("needs watchdog") {
            return Some(Diagnosis {
                message: "This bench's web server is missing a Python package (watchdog).".into(),
                recovery: format!("Frappe's development extras did not install. Run: uv pip install --python \"{environment}\" \"watchdog~=6.0.0\", then start the bench again."),
            });
        }
        if line.contains("No space left on device") {
            return Some(Diagnosis {
                message: "Your Mac ran out of disk space.".into(),
                recovery: "Free some space, then start the bench again.".into(),
            });
        }
        if line.contains("Address already in use") {
            return Some(Diagnosis {
                message: "A port this bench needs is already in use by another program.".into(),
                recovery: "Stop the other program or change this bench's ports, then start it again.".into(),
            });
        }
        if let Some(module) = missing_module(line) {
            return Some(Diagnosis {
                message: format!("A Python package is missing from this bench ({module})."),
                recovery: format!("Install it into the bench environment at {environment}, then start the bench again."),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench() -> &'static Path {
        Path::new("/Users/a b/pilot/benches/main")
    }

    #[test]
    fn the_watchdog_failure_from_a_real_bench_is_named_with_its_fix() {
        let lines = ["[web] INFO frappe.realtime.packets Server initialized", "[web] --dev needs watchdog: pip install 'frappe[dev]'", "[web] exited with code 1"];
        let found = diagnose(&lines, bench()).unwrap();
        assert!(found.message.contains("watchdog"));
        assert!(found.recovery.contains("\"/Users/a b/pilot/benches/main/env/bin/python\""));
    }

    #[test]
    fn a_full_disk_and_a_busy_port_are_recognised() {
        assert!(diagnose(&["OSError: [Errno 28] No space left on device"], bench()).unwrap().message.contains("disk space"));
        assert!(diagnose(&["OSError: [Errno 48] Address already in use"], bench()).unwrap().message.contains("port"));
    }

    #[test]
    fn any_missing_module_is_named() {
        let found = diagnose(&["ModuleNotFoundError: No module named 'redis'"], bench()).unwrap();
        assert!(found.message.contains("(redis)"));
    }

    #[test]
    fn ordinary_output_gives_no_diagnosis() {
        assert_eq!(diagnose(&["GET /api/v1/health 200", "Pilot exited with code 0."], bench()), None);
    }
}
