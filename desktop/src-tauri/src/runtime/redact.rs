use regex::Regex;
use std::sync::LazyLock;

const MAXIMUM_LINE_LENGTH: usize = 2000;

static RULES: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    let rule = |pattern: &str, replacement: &str| {
        (Regex::new(pattern).expect("valid redaction pattern"), replacement.to_string())
    };
    vec![
        rule(r"(?i)\bsid=[^\s&#]+", "sid=[redacted]"),
        rule(r"(?i)(generated admin password:\s*)\S+", "${1}[redacted]"),
        rule(r"(?i)((?:set-)?cookie\s*:)[^\n]*", "${1} [redacted]"),
        rule(r"(?i)(authorization\s*:)[^\n]*", "${1} [redacted]"),
        rule(r"(?i)(--(?:admin-)?password(?:=|\s+))\S+", "${1}[redacted]"),
        rule(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*", "[redacted-jwt]"),
        rule(r"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@]+):[^\s@/]+@", "${1}:[redacted]@"),
        rule(
            r#"(?i)\b((?:[a-z0-9_]*(?:password|passwd|secret|token|api[_-]?key|jwt_secret)[a-z0-9_]*))(\s*[:=]\s*)("[^"]*"|'[^']*'|[^\s,;]+)"#,
            "${1}${2}[redacted]",
        ),
    ]
});

pub fn redact(text: &str) -> String {
    let mut output = text.to_string();
    for (pattern, replacement) in RULES.iter() {
        output = pattern.replace_all(&output, replacement.as_str()).into_owned();
    }
    if output.len() > MAXIMUM_LINE_LENGTH {
        let mut end = MAXIMUM_LINE_LENGTH;
        while !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str(" …");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_setup_link_tokens_and_passwords() {
        let line = "  Open http://localhost:7000/?sid=abc.DEF-123_xyz in your browser";
        assert!(!redact(line).contains("abc.DEF"));
        assert!(redact("Generated admin password: Zx9!secretValue").ends_with("[redacted]"));
        assert!(!redact("mariadb_password = \"hunter2\"").contains("hunter2"));
        assert!(!redact("jwt_secret: s3cr3t").contains("s3cr3t"));
    }

    #[test]
    fn hides_credentials_in_headers_cookies_urls_and_arguments() {
        for secret in [
            "Authorization: Bearer topsecrettoken",
            "Cookie: sid=cookievalue; other=1",
            "mysql://root:dbpass@127.0.0.1:3310/db",
            "pilot new x --admin-password Passw0rd!",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhIn0.signaturepart",
        ] {
            let cleaned = redact(secret);
            for leaked in ["topsecrettoken", "cookievalue", "dbpass", "Passw0rd", "signaturepart"] {
                assert!(!cleaned.contains(leaked), "{cleaned}");
            }
        }
    }

    #[test]
    fn leaves_ordinary_output_alone_and_bounds_length() {
        assert_eq!(redact("[web] Listening on port 8000"), "[web] Listening on port 8000");
        assert!(redact(&"x".repeat(5000)).len() < 2100);
    }
}
