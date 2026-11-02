use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, LazyLock};

use regex::Regex;

use super::logs::LogBuffer;

static SETUP_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://(localhost|127\.0\.0\.1):(\d+)/\?sid=(\S+)").expect("setup link pattern"));
static GENERATED_PASSWORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)generated admin password:\s*(\S+)").expect("password pattern"));

/// A setup link is trusted only when it points at this bench's own loopback admin port.
pub fn extract_setup_url(line: &str, admin_port: u16) -> Option<String> {
    let captures = SETUP_LINK.captures(line)?;
    let port: u16 = captures[2].parse().ok()?;
    (port == admin_port).then(|| format!("http://127.0.0.1:{port}/?sid={}", &captures[3]))
}

pub fn extract_generated_password(line: &str) -> Option<String> {
    GENERATED_PASSWORD.captures(line).map(|captures| captures[1].to_string())
}

/// Reads raw lines in memory. `inspect` sees the raw text; the log buffer only ever stores the redacted form.
pub fn pump<R: Read>(reader: R, bench: &str, stream: &str, logs: &Arc<LogBuffer>, mut inspect: impl FnMut(&str)) {
    let mut reader = BufReader::new(reader);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                let raw = String::from_utf8_lossy(&bytes);
                inspect(&raw);
                logs.push(bench, stream, &raw);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_this_benchs_loopback_setup_link() {
        let line = "  Open http://localhost:7000/?sid=tok.en-1 in your browser";
        assert_eq!(extract_setup_url(line, 7000).as_deref(), Some("http://127.0.0.1:7000/?sid=tok.en-1"));
        assert_eq!(extract_setup_url(line, 7001), None);
        assert_eq!(extract_setup_url("Open http://evil.example:7000/?sid=abc", 7000), None);
        assert_eq!(extract_setup_url("Open http://localhost.evil.example:7000/?sid=abc", 7000), None);
    }

    #[test]
    fn finds_the_generated_password() {
        assert_eq!(extract_generated_password("\nGenerated admin password: Ab1!xyz"), Some("Ab1!xyz".into()));
        assert_eq!(extract_generated_password("nothing here"), None);
    }
}
