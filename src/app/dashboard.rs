use std::collections::HashMap;
use std::io::Cursor;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use argon2::{
    password_hash::{PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use sha2::{Digest, Sha256};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::domain::paths::{
    is_loopback_bind_host, is_valid_public_https_url, resolve_harness_home,
};
use crate::error::{Error, Result};
use crate::VERSION;

use super::catalog::{build_catalog, by_type};
use super::link::list_projects;
use super::query::{query_matrix, query_stats};

const DASHBOARD_AUTH_FAILURE_LIMIT: u32 = 30;
const DASHBOARD_AUTH_FAILURE_WINDOW: Duration = Duration::from_secs(60);
const MAX_DASHBOARD_AUTH_FAILURE_BUCKETS: usize = 4_096;
const MAX_DASHBOARD_HEADER_VALUE_BYTES: usize = 16 * 1024;
const MAX_DASHBOARD_HEADERS: usize = 64;
const MAX_DASHBOARD_HEADER_BYTES: usize = 64 * 1024;
const MAX_DASHBOARD_PASSWORD_BYTES: usize = 4 * 1024;

fn dashboard_password_input_allowed(password: &str) -> bool {
    password.len() <= MAX_DASHBOARD_PASSWORD_BYTES
}

fn dashboard_headers_oversized(headers: &[Header]) -> bool {
    headers
        .iter()
        .any(|header| header.value.as_bytes().len() > MAX_DASHBOARD_HEADER_VALUE_BYTES)
        || headers.len() > MAX_DASHBOARD_HEADERS
        || headers
            .iter()
            .map(|header| header.field.as_str().as_bytes().len() + header.value.as_bytes().len())
            .sum::<usize>()
            > MAX_DASHBOARD_HEADER_BYTES
}

struct AuthFailureLimiter {
    buckets: HashMap<String, (Instant, u32)>,
}

impl AuthFailureLimiter {
    fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    fn key(remote: Option<&SocketAddr>) -> String {
        remote
            .map(|address| address.ip().to_string())
            .unwrap_or_else(|| "<unknown>".into())
    }

    fn cleanup(&mut self) {
        let now = Instant::now();
        self.buckets
            .retain(|_, (started, _)| now.duration_since(*started) < DASHBOARD_AUTH_FAILURE_WINDOW);
    }

    fn can_attempt(&mut self, remote: Option<&SocketAddr>) -> bool {
        self.cleanup();
        let key = Self::key(remote);
        match self.buckets.get(&key) {
            Some((_, count)) => *count < DASHBOARD_AUTH_FAILURE_LIMIT,
            None => self.buckets.len() < MAX_DASHBOARD_AUTH_FAILURE_BUCKETS,
        }
    }

    fn record_failure(&mut self, remote: Option<&SocketAddr>) {
        self.cleanup();
        let key = Self::key(remote);
        if !self.buckets.contains_key(&key)
            && self.buckets.len() >= MAX_DASHBOARD_AUTH_FAILURE_BUCKETS
        {
            return;
        }
        let now = Instant::now();
        let entry = self.buckets.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= DASHBOARD_AUTH_FAILURE_WINDOW {
            *entry = (now, 0);
        }
        entry.1 = entry.1.saturating_add(1);
    }

    fn clear(&mut self, remote: Option<&SocketAddr>) {
        self.buckets.remove(&Self::key(remote));
    }
}

pub struct RunningServer {
    pub url: String,
    pub port: u16,
    pub auth_token: Option<String>,
    pub shutdown: Arc<AtomicBool>,
    pub handle: Option<thread::JoinHandle<()>>,
}

impl RunningServer {
    pub fn shutdown(mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Wake the listener by connecting once.
        let _ = std::net::TcpStream::connect(format!("127.0.0.1:{}", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn set_dashboard_password(password: &str) -> Result<std::path::PathBuf> {
    if password.trim().len() < 12 {
        return Err(Error::new(
            "dashboard password must be at least 12 characters",
        ));
    }
    if !dashboard_password_input_allowed(password) {
        return Err(Error::new("dashboard password must be at most 4096 bytes"));
    }
    let home = resolve_harness_home();
    crate::infra::entities::ensure_directory_no_symlink(&home)?;
    let path = dashboard_password_path();
    let mut salt_bytes = [0u8; 16];
    getrandom::getrandom(&mut salt_bytes)
        .map_err(|err| Error::new(format!("generate dashboard password salt: {err}")))?;
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|err| Error::new(format!("encode dashboard password salt: {err}")))?;
    let digest = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| Error::new(format!("hash dashboard password: {err}")))?
        .to_string();
    crate::infra::entities::atomic_write(&path, &format!("{digest}\n"))?;
    // A pre-0.27 SHA-256 record cannot be upgraded without the plaintext.  It
    // is safe to remove it after writing the Argon2id record; authentication
    // will use the memory-hard hash from now on.
    let _ = std::fs::remove_file(legacy_dashboard_password_path());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

fn dashboard_password_path() -> std::path::PathBuf {
    resolve_harness_home().join("dashboard-password.argon2")
}

fn legacy_dashboard_password_path() -> std::path::PathBuf {
    resolve_harness_home().join("dashboard-password.sha256")
}

fn dashboard_password_hash() -> Option<String> {
    fn read_record(path: std::path::PathBuf) -> Option<String> {
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return None;
        }
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    read_record(dashboard_password_path()).or_else(|| read_record(legacy_dashboard_password_path()))
}

pub fn dashboard_password_configured() -> bool {
    dashboard_password_hash().is_some_and(|hash| hash.starts_with("$argon2id$"))
}

fn dashboard_authorized(headers: &[Header]) -> bool {
    let Some(expected) = dashboard_password_hash() else {
        return true;
    };
    let supplied = headers
        .iter()
        .find(|h| h.field.equiv("X-Harness-Password"))
        .map(|h| h.value.as_str())
        .or_else(|| {
            headers
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
        });
    let Some(supplied) = supplied else {
        return false;
    };
    if !dashboard_password_input_allowed(supplied) {
        return false;
    }
    if expected.starts_with("$argon2") {
        let Ok(parsed) = PasswordHash::new(&expected) else {
            return false;
        };
        return Argon2::default()
            .verify_password(supplied.as_bytes(), &parsed)
            .is_ok();
    }

    // Legacy SHA-256 records are accepted only long enough for an operator to
    // replace them with `set-password`; compare the complete digest without an
    // early-return equality check.
    let mut hasher = Sha256::new();
    hasher.update(supplied.as_bytes());
    let actual = hex::encode(hasher.finalize());
    constant_time_equal(actual.as_bytes(), expected.as_bytes())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    let max = left.len().max(right.len());
    for index in 0..max {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        diff |= usize::from(a ^ b);
    }
    diff == 0
}

pub fn start_dashboard(
    host: &str,
    port: u16,
    serve_forever: bool,
    public_url: Option<&str>,
) -> Result<RunningServer> {
    if !is_loopback_bind_host(host) {
        let url = public_url.ok_or_else(|| {
            Error::new("refusing non-loopback dashboard bind without --public-url https://...")
        })?;
        if !is_valid_public_https_url(url) {
            return Err(Error::new(
                "--public-url must be a valid https URL without credentials, query, or fragment for non-loopback dashboard",
            ));
        }
        if !dashboard_password_configured() {
            return Err(Error::new(
                "refusing non-loopback dashboard bind without a configured password; run `harness dashboard set-password` first",
            ));
        }
    }
    let listener = TcpListener::bind((host, port))
        .map_err(|e| Error::new(format!("dashboard bind {host}:{port} failed: {e}")))?;
    let actual = listener.local_addr()?.port();
    let server = Server::from_listener(listener, None)
        .map_err(|e| Error::new(format!("dashboard server: {e}")))?;
    let local_url = format!("http://{host}:{actual}/");
    let url = public_url
        .map(|value| format!("{}/", value.trim_end_matches('/')))
        .unwrap_or(local_url);
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let public_bind = !is_loopback_bind_host(host);
    let handle = thread::spawn(move || dashboard_loop(server, flag, public_bind));
    if serve_forever {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
    Ok(RunningServer {
        url,
        port: actual,
        auth_token: None,
        shutdown,
        handle: Some(handle),
    })
}

fn dashboard_loop(server: Server, shutdown: Arc<AtomicBool>, public_bind: bool) {
    let mut auth_failures = AuthFailureLimiter::new();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(request)) => {
                let url = request.url().to_string();
                let method = request.method().clone();
                let path = url.split('?').next().unwrap_or("/");
                let remote = request.remote_addr();
                let public_endpoint =
                    path == "/api/health" || (path == "/mcp" && method == Method::Get);
                let oversized_headers = dashboard_headers_oversized(request.headers());
                let throttled = public_bind
                    && !public_endpoint
                    && !oversized_headers
                    && !auth_failures.can_attempt(remote);
                let authorized = public_endpoint
                    || (!oversized_headers
                        && !throttled
                        && (!public_bind || dashboard_password_configured())
                        && dashboard_authorized(request.headers()));
                if public_bind && !public_endpoint {
                    if authorized {
                        auth_failures.clear(remote);
                    } else if !throttled {
                        auth_failures.record_failure(remote);
                    }
                }
                let (status, content_type, body) = if oversized_headers {
                    (
                        431,
                        "application/json; charset=utf-8".into(),
                        r#"{"error":"request header exceeds 16 KiB limit"}"#.into(),
                    )
                } else if throttled {
                    (
                        429,
                        "application/json; charset=utf-8".into(),
                        r#"{"error":"too many dashboard authentication failures"}"#.into(),
                    )
                } else if authorized {
                    route(&method, &url)
                } else {
                    (
                        401,
                        "application/json; charset=utf-8".into(),
                        r#"{"error":"dashboard password required"}"#.into(),
                    )
                };
                let mut response = Response::new(
                    StatusCode(status),
                    vec![
                        Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                            .unwrap_or_else(|_| {
                                Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..])
                                    .unwrap()
                            }),
                    ],
                    Cursor::new(body.into_bytes()),
                    None,
                    None,
                );
                response.add_header(
                    Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap(),
                );
                for (name, value) in [
                    (
                        "Content-Security-Policy",
                        "default-src 'self'; base-uri 'none'; frame-ancestors 'none'",
                    ),
                    ("X-Content-Type-Options", "nosniff"),
                    ("Referrer-Policy", "no-referrer"),
                    ("X-Frame-Options", "DENY"),
                ] {
                    response
                        .add_header(Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap());
                }
                if status == 401 {
                    response.add_header(
                        Header::from_bytes(&b"WWW-Authenticate"[..], &b"Bearer"[..]).unwrap(),
                    );
                }
                if status == 429 {
                    response
                        .add_header(Header::from_bytes(&b"Retry-After"[..], &b"60"[..]).unwrap());
                }
                let _ = request.respond(response);
            }
            Ok(None) => continue,
            Err(_) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }
}

fn route(method: &Method, url: &str) -> (u16, String, String) {
    let path = url.split('?').next().unwrap_or("/");
    match (method, path) {
        (Method::Get, "/") | (Method::Get, "/index.html") => {
            (200, "text/html; charset=utf-8".into(), render_home())
        }
        (Method::Get, "/api/projects") => (
            200,
            "application/json; charset=utf-8".into(),
            serde_json::to_string_pretty(&projects_json()).unwrap_or_else(|_| "[]".into()),
        ),
        (Method::Get, "/api/health") => (
            200,
            "application/json; charset=utf-8".into(),
            format!(r#"{{"ok":true,"product":"5harness","version":"{VERSION}"}}"#),
        ),
        (Method::Get, "/mcp") => (
            200,
            "application/json; charset=utf-8".into(),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "5harness",
                "version": VERSION,
                "protocolVersion": "2024-11-05",
                "transport": "streamable-http",
                "tools": super::mcp::mcp_tools(),
                "message": "Use `harness mcp` for authenticated project-bound MCP calls."
            })).unwrap_or_else(|_| "{}".into()),
        ),
        (Method::Post, "/mcp") => (
            501,
            "application/json; charset=utf-8".into(),
            r#"{"error":"dashboard MCP transport is discovery-only; start `harness mcp` for authenticated calls"}"#.into(),
        ),
        _ => (
            404,
            "text/plain; charset=utf-8".into(),
            "not found".into(),
        ),
    }
}

fn projects_json() -> serde_json::Value {
    let projects: Vec<serde_json::Value> = list_projects()
        .into_iter()
        .map(|(p, missing)| {
            let stats = if missing {
                serde_json::Value::Null
            } else {
                count_stats(Path::new(&p.path))
            };
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "path": p.path,
                "missing": missing,
                "linked_at": p.linked_at,
                "stats": stats,
            })
        })
        .collect();
    serde_json::Value::Array(projects)
}

fn count_stats(path: &Path) -> serde_json::Value {
    match build_catalog(path) {
        Ok(cat) => serde_json::json!({
            "stories": by_type(&cat, "story").len(),
            "decisions": by_type(&cat, "decision").len(),
            "intakes": by_type(&cat, "intake").len(),
            "backlog": by_type(&cat, "backlog").len(),
        }),
        Err(_) => serde_json::Value::Null,
    }
}

fn render_home() -> String {
    let projects = list_projects();
    let mut rows = String::new();
    let mut total_stories = 0usize;
    let mut total_decisions = 0usize;
    let mut total_intakes = 0usize;
    for (p, missing) in &projects {
        let (status, stats) = if *missing {
            ("missing".to_string(), "path missing on disk".to_string())
        } else if let Ok(cat) = build_catalog(Path::new(&p.path)) {
            let s = by_type(&cat, "story").len();
            let d = by_type(&cat, "decision").len();
            let i = by_type(&cat, "intake").len();
            total_stories += s;
            total_decisions += d;
            total_intakes += i;
            (
                "ok".to_string(),
                format!("{s} stories · {d} decisions · {i} intakes"),
            )
        } else {
            ("error".to_string(), "catalog error".to_string())
        };
        rows.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td class=\"status-{status}\">{status}</td><td>{stats}</td></tr>",
            html_escape(&p.name),
            html_escape(&p.path),
        ));
    }
    if rows.is_empty() {
        rows = "<tr><td colspan=\"4\">No linked projects. Run <code>harness link</code>.</td></tr>"
            .into();
    }
    let _ = (query_matrix, query_stats);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Harness Dashboard</title>
  <style>
    :root {{ --bg:#0f1419; --fg:#e7ecf3; --muted:#9aa7b8; --border:#243042; --link:#6cb6ff; --ok:#3dd68c; --missing:#ffb020; --error:#ff6b6b; }}
    body {{ font-family: ui-sans-serif, system-ui, sans-serif; background: var(--bg); color: var(--fg); margin: 0; padding: 1.5rem 2rem; }}
    a {{ color: var(--link); }}
    h1 {{ margin: 0 0 0.4rem; }}
    .muted {{ color: var(--muted); }}
    table {{ width: 100%; border-collapse: collapse; margin-top: 1rem; }}
    th, td {{ text-align: left; padding: 0.5rem 0.6rem; border-bottom: 1px solid var(--border); }}
    code {{ font-size: 0.85em; }}
    .status-ok {{ color: var(--ok); }}
    .status-missing {{ color: var(--missing); }}
    .status-error {{ color: var(--error); }}
    .footer {{ margin-top: 2rem; color: var(--muted); font-size: 0.85rem; }}
  </style>
</head>
<body>
  <h1>Harness Dashboard</h1>
  <p class="muted">Local cockpit for linked projects — next work, matrix, reports, MCP.</p>
  <p><a href="/api/projects">JSON /api/projects</a></p>
  <table>
    <thead><tr><th>Name</th><th>Path</th><th>Status</th><th>Summary</th></tr></thead>
    <tbody>
      {rows}
    </tbody>
  </table>
  <p class="muted">{count} project(s) — totals: {total_stories} stories, {total_decisions} decisions, {total_intakes} intakes</p>
  <div class="footer"><p>Harness v{VERSION} — github.com/vantanminh/5harness</p></div>
</body>
</html>"#,
        count = projects.len(),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::{
        dashboard_headers_oversized, dashboard_password_input_allowed, AuthFailureLimiter,
        DASHBOARD_AUTH_FAILURE_LIMIT, MAX_DASHBOARD_HEADER_VALUE_BYTES,
    };
    use std::net::SocketAddr;
    use tiny_http::Header;

    #[test]
    fn public_auth_failures_are_bounded_per_source() {
        let mut limiter = AuthFailureLimiter::new();
        let remote: SocketAddr = "127.0.0.1:3927".parse().unwrap();
        for _ in 0..DASHBOARD_AUTH_FAILURE_LIMIT {
            assert!(limiter.can_attempt(Some(&remote)));
            limiter.record_failure(Some(&remote));
        }
        assert!(!limiter.can_attempt(Some(&remote)));
        limiter.clear(Some(&remote));
        assert!(limiter.can_attempt(Some(&remote)));
    }

    #[test]
    fn dashboard_auth_inputs_have_bounded_sizes() {
        assert!(dashboard_password_input_allowed(&"x".repeat(4 * 1024)));
        assert!(!dashboard_password_input_allowed(&"x".repeat(4 * 1024 + 1)));
        let header = Header::from_bytes(
            &b"X-Harness-Password"[..],
            vec![b'x'; MAX_DASHBOARD_HEADER_VALUE_BYTES + 1],
        )
        .expect("header");
        assert!(dashboard_headers_oversized(&[header]));
    }
}
