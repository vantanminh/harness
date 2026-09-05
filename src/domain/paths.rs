use std::env;
use std::path::{Path, PathBuf};

pub const GLOBAL_HOME_DIRNAME: &str = ".5harness";
pub const PROJECT_STATE_DIRNAME: &str = ".5harness";
pub const BACKUP_DIRNAME: &str = ".5harness-backup";
pub const SQLITE_DB_BASENAME: &str = "harness.db";

pub fn resolve_target_dir(input: Option<&str>, cwd: &Path) -> PathBuf {
    let raw = input.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    if raw.is_empty() {
        return cwd.to_path_buf();
    }
    if raw.starts_with('~') {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| cwd.display().to_string());
        let rest = raw.trim_start_matches('~').trim_start_matches(['/', '\\']);
        return PathBuf::from(home).join(rest);
    }
    if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        cwd.join(raw)
    }
}

pub fn resolve_harness_home() -> PathBuf {
    if let Ok(override_home) = env::var("HARNESS_HOME") {
        let trimmed = override_home.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with('~') {
                let home = env::var("HOME")
                    .or_else(|_| env::var("USERPROFILE"))
                    .unwrap_or_default();
                let rest = trimmed
                    .trim_start_matches('~')
                    .trim_start_matches(['/', '\\']);
                return PathBuf::from(home).join(rest);
            }
            return PathBuf::from(trimmed);
        }
    }
    dirs_home().join(GLOBAL_HOME_DIRNAME)
}

fn dirs_home() -> PathBuf {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn registry_file_path(harness_home: &Path) -> PathBuf {
    harness_home.join("registry.json")
}

pub fn resolve_project_state_root(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_STATE_DIRNAME)
}

pub fn project_index_dir(project_root: &Path) -> PathBuf {
    resolve_project_state_root(project_root).join("index")
}

pub fn project_local_dir(project_root: &Path) -> PathBuf {
    resolve_project_state_root(project_root).join("local")
}

pub fn resolve_db_path(target_dir: &Path) -> PathBuf {
    if let Ok(override_path) = env::var("HARNESS_DB_PATH") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            return if p.is_absolute() {
                p
            } else {
                target_dir.join(p)
            };
        }
    }
    target_dir.join(SQLITE_DB_BASENAME)
}

pub fn project_backup_root(target_dir: &Path, stamp: &str) -> PathBuf {
    target_dir.join(BACKUP_DIRNAME).join(stamp)
}

pub fn is_protected_relative(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    normalized == "AGENTS.md" || normalized == "docs" || normalized.starts_with("docs/")
}

pub fn is_loopback_bind_host(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "::1" | "[::1]"
    )
}

/// Validate a public base URL used for non-loopback HTTP services.
///
/// A prefix check such as `starts_with("https://")` accepts malformed URLs,
/// embedded credentials, and query/fragment values that can confuse clients.
/// Parse the URL structurally and fail closed instead.
pub fn is_valid_public_https_url(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw.trim()) else {
        return false;
    };
    url.scheme().eq_ignore_ascii_case("https")
        && url.host_str().is_some_and(|host| !host.trim().is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

#[cfg(test)]
mod tests {
    use super::is_valid_public_https_url;

    #[test]
    fn public_url_validation_is_structural() {
        assert!(is_valid_public_https_url("https://example.test/mcp"));
        assert!(!is_valid_public_https_url("http://example.test"));
        assert!(!is_valid_public_https_url("https://user:pass@example.test"));
        assert!(!is_valid_public_https_url(
            "https://example.test?token=secret"
        ));
        assert!(!is_valid_public_https_url("https://"));
    }
}
