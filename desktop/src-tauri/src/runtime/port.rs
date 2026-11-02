use std::process::Command;

use crate::error::{AppError, AppResult, ErrorCategory};

pub fn parse_lsof_pids(text: &str) -> Vec<i32> {
    let mut pids: Vec<i32> = text.lines().filter_map(|line| line.strip_prefix('p')?.parse().ok()).collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Processes of this user listening on a TCP port, via lsof's field output.
pub fn listening_pids(port: u16) -> AppResult<Vec<i32>> {
    let output = Command::new("/usr/sbin/lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fp"])
        .output()
        .map_err(|error| AppError::new(ErrorCategory::Io, format!("Could not run lsof: {error}")))?;
    Ok(parse_lsof_pids(&String::from_utf8_lossy(&output.stdout)))
}

pub fn is_listening_locally(port: u16) -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(250)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_field_output_and_dedupes() {
        assert_eq!(parse_lsof_pids("p123\nf5\np123\nf6\np99\n"), vec![99, 123]);
        assert!(parse_lsof_pids("").is_empty());
    }

    #[test]
    fn finds_this_process_listening() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(listening_pids(port).unwrap().contains(&(std::process::id() as i32)));
    }
}
