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

/// Listener addresses from lsof's `n` fields, such as `*:7004` or `127.0.0.1:7004`.
pub fn parse_lsof_addresses(text: &str) -> Vec<String> {
    text.lines().filter_map(|line| line.strip_prefix('n')).map(str::to_string).collect()
}

fn is_wildcard(address: &str) -> bool {
    address.starts_with("*:") || address.starts_with("[::]:") || address.starts_with("0.0.0.0:")
}

/// True when something on this port accepts connections from other devices, not only from this Mac.
pub fn is_exposed_to_network(port: u16) -> bool {
    let output = Command::new("/usr/sbin/lsof").args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fn"]).output();
    let Ok(output) = output else { return false };
    parse_lsof_addresses(&String::from_utf8_lossy(&output.stdout)).iter().any(|address| is_wildcard(address))
}

pub fn is_listening_locally(port: u16) -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(250)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wildcard_listeners_are_reachable_from_the_network() {
        let names = parse_lsof_addresses("p1\nf5\nn*:7004\nf6\nn127.0.0.1:7004\nn[::1]:7004\n");
        assert_eq!(names, ["*:7004", "127.0.0.1:7004", "[::1]:7004"]);
        assert!(is_wildcard("*:7004") && is_wildcard("[::]:7004") && is_wildcard("0.0.0.0:7004"));
        assert!(!is_wildcard("127.0.0.1:7004") && !is_wildcard("[::1]:7004"));
    }

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
