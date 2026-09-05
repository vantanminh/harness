use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness"))
}

fn http_get(addr: &str, path: &str) -> (u16, String) {
    http_get_with_headers(addr, path, &[])
}

fn http_get_with_headers(addr: &str, path: &str, headers: &[(&str, &str)]) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let extra = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    stream
        .write_all(
            format!("GET {path} HTTP/1.0\r\nHost: {addr}\r\n{extra}Connection: close\r\n\r\n")
                .as_bytes(),
        )
        .unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, body.to_string())
}

#[test]
fn dashboard_password_is_argon2id_and_public_bind_fails_closed() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let home = std::env::temp_dir().join(format!("harness-home-dashboard-auth-{nonce}"));
    std::fs::create_dir_all(&home).unwrap();

    let refused = Command::new(bin())
        .args([
            "dashboard",
            "--host",
            "0.0.0.0",
            "--port",
            "3954",
            "--public-url",
            "https://dashboard.example.test",
        ])
        .env("HARNESS_HOME", &home)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("configured password"));

    let set = Command::new(bin())
        .args([
            "dashboard",
            "set-password",
            "--password",
            "correct horse battery staple",
        ])
        .env("HARNESS_HOME", &home)
        .output()
        .unwrap();
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );
    let record = std::fs::read_to_string(home.join("dashboard-password.argon2")).unwrap();
    assert!(record.starts_with("$argon2id$"), "{record}");
    assert!(!home.join("dashboard-password.sha256").exists());

    let mut dashboard = Command::new(bin())
        .args([
            "dashboard",
            "--host",
            "0.0.0.0",
            "--port",
            "3954",
            "--public-url",
            "https://dashboard.example.test",
        ])
        .env("HARNESS_HOME", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_port("127.0.0.1:3954");
    let (status, _) = http_get("127.0.0.1:3954", "/");
    assert_eq!(status, 401);
    let (status, body) = http_get_with_headers(
        "127.0.0.1:3954",
        "/",
        &[("X-Harness-Password", "correct horse battery staple")],
    );
    assert_eq!(status, 200);
    assert!(body.contains("Harness Dashboard"));
    let _ = dashboard.kill();
    let _ = dashboard.wait();
}

fn http_post(addr: &str, path: &str, json: &str) -> (u16, String) {
    http_post_with_headers(addr, path, json, &[])
}

fn http_post_with_headers(
    addr: &str,
    path: &str,
    json: &str,
    headers: &[(&str, &str)],
) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let extra = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    let req = format!(
        "POST {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len(),
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, body.to_string())
}

#[test]
fn mcp_mutations_require_token_and_project_binding() {
    let home = std::env::temp_dir().join(format!("harness-home-mcp-auth-{}", std::process::id()));
    let tmp = std::env::temp_dir().join(format!("harness-mcp-auth-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();
    let init = Command::new(bin())
        .args(["init", tmp.to_str().unwrap()])
        .env("HARNESS_HOME", &home)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let agents = std::fs::read_to_string(tmp.join("AGENTS.md")).unwrap();
    let project_id = agents
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("<!-- harness-project-id: ")
                .and_then(|s| s.strip_suffix(" -->"))
        })
        .unwrap()
        .to_string();
    let mut mcp = Command::new(bin())
        .args([
            "mcp",
            "--host",
            "127.0.0.1",
            "--port",
            "3943",
            "--dir",
            tmp.to_str().unwrap(),
            "--token",
            "test-token",
        ])
        .env("HARNESS_HOME", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_port("127.0.0.1:3943");
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"harness_intake","arguments":{"type":"spec_slice","summary":"auth test","lane":"normal"}}}"#;
    let (status, _) = http_post("127.0.0.1:3943", "/mcp", body);
    assert_eq!(status, 401);
    let (status, _) = http_post_with_headers(
        "127.0.0.1:3943",
        "/mcp",
        body,
        &[("Authorization", "Bearer test-token")],
    );
    assert_eq!(status, 403);
    let (status, response) = http_post_with_headers(
        "127.0.0.1:3943",
        "/mcp",
        body,
        &[
            ("Authorization", "Bearer test-token"),
            ("X-Harness-Project", &project_id),
        ],
    );
    assert_eq!(status, 200);
    assert!(response.contains("Intake IN-001"), "{response}");
    let _ = mcp.kill();
    let _ = mcp.wait();
}

fn wait_port(addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if let Ok(s) =
            TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(200))
        {
            drop(s);
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("server did not bind {addr}");
}

#[test]
fn dashboard_html_and_mcp_protocol_bodies() {
    let home = std::env::temp_dir().join(format!("harness-home-srv-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();

    let mut dash = Command::new(bin())
        .args(["dashboard", "--host", "127.0.0.1", "--port", "3941"])
        .env("HARNESS_HOME", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_port("127.0.0.1:3941");
    let (st, body) = http_get("127.0.0.1:3941", "/");
    assert_eq!(st, 200);
    assert!(body.contains("Harness Dashboard"), "{body}");
    assert!(body.contains("<!DOCTYPE html>"), "{body}");
    assert!(body.contains("/api/projects"), "{body}");
    let (st2, api) = http_get("127.0.0.1:3941", "/api/projects");
    assert_eq!(st2, 200);
    let trimmed = api.trim_start();
    assert!(trimmed.starts_with('['), "{api}");
    let _ = dash.kill();
    let _ = dash.wait();

    let tmp = std::env::temp_dir().join(format!("harness-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut mcp = Command::new(bin())
        .args([
            "mcp",
            "--host",
            "127.0.0.1",
            "--port",
            "3942",
            "--dir",
            tmp.to_str().unwrap(),
        ])
        .env("HARNESS_HOME", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_port("127.0.0.1:3942");
    let (st, disc) = http_get("127.0.0.1:3942", "/.well-known/oauth-protected-resource");
    assert_eq!(st, 200);
    assert!(disc.contains("authorization_servers"), "{disc}");
    assert!(disc.contains("/mcp"), "{disc}");
    let (st, init_body) = http_post(
        "127.0.0.1:3942",
        "/mcp",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    );
    assert_eq!(st, 200);
    assert!(init_body.contains("protocolVersion"), "{init_body}");
    assert!(init_body.contains("2024-11-05"), "{init_body}");
    assert!(init_body.contains("5harness"), "{init_body}");
    let (st, tools) = http_post(
        "127.0.0.1:3942",
        "/mcp",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    );
    assert_eq!(st, 200);
    assert!(tools.contains("harness_get"), "{tools}");
    let _ = mcp.kill();
    let _ = mcp.wait();
}
