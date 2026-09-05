use std::fmt;
use std::sync::OnceLock;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: String,
    pub exit_code: i32,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            code: classify_code(&message).into(),
            message,
            exit_code: 1,
        }
    }

    pub fn with_code(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            exit_code: 1,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

fn classify_code(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("not found") {
        "HARNESS_E_NOT_FOUND"
    } else if lower.contains("path escapes") || lower.contains("project-relative") {
        "HARNESS_E_PATH"
    } else if lower.contains("lock") || lower.contains("busy") {
        "HARNESS_E_LOCK"
    } else if lower.contains("requires") || lower.contains("invalid") || lower.contains("unknown") {
        "HARNESS_E_USAGE"
    } else {
        "HARNESS_E_OPERATION"
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Remove common credential forms before an error is written to a terminal,
/// JSON response, or diagnostic log. This is defense in depth; callers should
/// still avoid putting secrets in durable payloads or command arguments.
pub fn redact_sensitive(input: &str) -> String {
    static CREDENTIALS: OnceLock<regex::Regex> = OnceLock::new();
    static PREFIXES: OnceLock<regex::Regex> = OnceLock::new();
    let credentials = CREDENTIALS.get_or_init(|| {
        regex::Regex::new(
            r"(?ix)(authorization\s*:\s*bearer\s+|bearer\s+|(?:token|password|secret|npm_token|github_token)\s*[=:]\s*)([^\s,;]+)",
        )
        .expect("credential redaction regex")
    });
    let prefixes = PREFIXES.get_or_init(|| {
        regex::Regex::new(r"(?i)(?:npm_[a-z0-9]{10,}|gh[pousr]_[a-z0-9]{10,}|sk-[a-z0-9_-]{10,})")
            .expect("token prefix redaction regex")
    });
    let replaced = credentials.replace_all(input, "$1[REDACTED]");
    prefixes.replace_all(&replaced, "[REDACTED]").into_owned()
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::new(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::new(value.to_string())
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Error::new(value)
    }
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Error::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive;

    #[test]
    fn redacts_credentials_without_echoing_values() {
        let value = redact_sensitive(
            "Authorization: Bearer super-secret-token token=abc123 password:letmein npm_1234567890",
        );
        assert!(!value.contains("super-secret-token"));
        assert!(!value.contains("abc123"));
        assert!(!value.contains("letmein"));
        assert!(!value.contains("npm_1234567890"));
        assert!(value.contains("[REDACTED]"));
    }
}
