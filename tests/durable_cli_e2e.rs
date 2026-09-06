use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness"))
}

fn run(args: &[&str], cwd: Option<&Path>) -> std::process::Output {
    let mut cmd = Command::new(bin());
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.env("HARNESS_NO_UPDATE_CHECK", "1");
    let home = std::env::temp_dir().join(format!("harness-home-{}", std::process::id()));
    let _ = fs::create_dir_all(&home);
    cmd.env("HARNESS_HOME", &home);
    cmd.output().expect("spawn harness")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn tmp(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "{prefix}{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn init_then_intake_story_decision_backlog_query_search_get() {
    let dir = tmp("harness-dur-cli-");
    let d = dir.to_str().unwrap();

    let init = run(&["init", d], None);
    assert!(init.status.success(), "{}", stderr(&init) + &stdout(&init));

    let intake = run(
        &[
            "intake",
            "--dir",
            d,
            "--type",
            "spec-slice",
            "--summary",
            "phase b",
            "--lane",
            "normal",
            "--story",
            "US-100",
        ],
        None,
    );
    assert!(
        intake.status.success(),
        "{}",
        stderr(&intake) + &stdout(&intake)
    );
    assert!(
        stdout(&intake).contains("Intake IN-001"),
        "{}",
        stdout(&intake)
    );

    let story_add = run(
        &[
            "story",
            "add",
            "--dir",
            d,
            "--id",
            "US-100",
            "--title",
            "Phase B story",
            "--lane",
            "normal",
        ],
        None,
    );
    assert!(
        story_add.status.success(),
        "{}",
        stderr(&story_add) + &stdout(&story_add)
    );

    let story_update = run(
        &[
            "story",
            "update",
            "--dir",
            d,
            "--id",
            "US-100",
            "--status",
            "implemented",
            "--unit",
            "1",
            "--integration",
            "1",
            "--e2e",
            "0",
            "--platform",
            "0",
        ],
        None,
    );
    assert!(
        story_update.status.success(),
        "{}",
        stderr(&story_update) + &stdout(&story_update)
    );

    let decision = run(
        &[
            "decision",
            "add",
            "--dir",
            d,
            "--id",
            "0100-test",
            "--title",
            "Test decision",
        ],
        None,
    );
    assert!(
        decision.status.success(),
        "{}",
        stderr(&decision) + &stdout(&decision)
    );

    let backlog = run(
        &[
            "backlog",
            "add",
            "--dir",
            d,
            "--title",
            "Polish help text",
            "--risk",
            "tiny",
        ],
        None,
    );
    assert!(
        backlog.status.success(),
        "{}",
        stderr(&backlog) + &stdout(&backlog)
    );

    let matrix = run(&["query", "matrix", "--dir", d], None);
    assert!(
        matrix.status.success(),
        "{}",
        stderr(&matrix) + &stdout(&matrix)
    );
    assert!(stdout(&matrix).contains("US-100"), "{}", stdout(&matrix));

    let stats = run(&["query", "stats", "--dir", d], None);
    assert!(
        stats.status.success(),
        "{}",
        stderr(&stats) + &stdout(&stats)
    );
    assert!(
        stdout(&stats).contains("Harness Stats"),
        "{}",
        stdout(&stats)
    );

    let search = run(&["search", "Phase B", "--dir", d], None);
    assert!(
        search.status.success(),
        "{}",
        stderr(&search) + &stdout(&search)
    );
    assert!(
        stdout(&search).contains("US-100") || stdout(&search).contains("Phase B"),
        "{}",
        stdout(&search)
    );

    let get = run(&["get", "US-100", "--dir", d], None);
    assert!(get.status.success(), "{}", stderr(&get) + &stdout(&get));
    assert!(stdout(&get).contains("US-100"), "{}", stdout(&get));

    let empty = tmp("harness-empty-q-");
    let empty_matrix = run(&["query", "matrix", "--dir", empty.to_str().unwrap()], None);
    assert!(
        empty_matrix.status.success(),
        "{}",
        stderr(&empty_matrix) + &stdout(&empty_matrix)
    );
}

#[test]
fn project_verify_commands_require_explicit_trust() {
    let dir = tmp("harness-verify-trust-");
    let d = dir.to_str().unwrap();
    let init = run(&["init", d], None);
    assert!(init.status.success(), "{}", stderr(&init) + &stdout(&init));

    let add = run(
        &[
            "story",
            "add",
            "--dir",
            d,
            "--id",
            "US-VERIFY",
            "--title",
            "Trust gate",
            "--lane",
            "normal",
            "--verify",
            "echo verify-ok",
        ],
        None,
    );
    assert!(add.status.success(), "{}", stderr(&add) + &stdout(&add));

    let refused = run(&["story", "verify", "US-VERIFY", "--dir", d], None);
    assert!(!refused.status.success());
    let refused_text = stderr(&refused) + &stdout(&refused);
    assert!(
        refused_text.contains("--allow-project-command"),
        "{refused_text}"
    );

    let allowed = run(
        &[
            "story",
            "verify",
            "US-VERIFY",
            "--dir",
            d,
            "--allow-project-command",
        ],
        None,
    );
    assert!(
        allowed.status.success(),
        "{}",
        stderr(&allowed) + &stdout(&allowed)
    );
    assert!(stdout(&allowed).contains("verification: passed"));
}

#[test]
fn verify_all_validates_every_command_before_execution() {
    let dir = tmp("harness-verify-all-validation-");
    let d = dir.to_str().unwrap();
    let init = run(&["init", d], None);
    assert!(init.status.success(), "{}", stderr(&init));
    let add = run(
        &[
            "story",
            "add",
            "--dir",
            d,
            "--id",
            "US-GOOD",
            "--title",
            "Good command",
            "--lane",
            "normal",
            "--verify",
            "echo good",
        ],
        None,
    );
    assert!(add.status.success(), "{}", stderr(&add));

    let oversized = "x".repeat(8 * 1024 + 1);
    fs::write(
        dir.join("docs/stories/US-ZZZ.md"),
        format!(
            "---\nid: US-ZZZ\ntype: story\ntitle: Bad command\nstatus: planned\nverify: \"{oversized}\"\n---\n\n# Bad command\n"
        ),
    )
    .unwrap();
    let result = run(
        &["story", "verify-all", "--dir", d, "--allow-project-command"],
        None,
    );
    assert!(!result.status.success());
    let text = stderr(&result) + &stdout(&result);
    assert!(text.contains("8192-byte limit"), "{text}");
    let good = fs::read_to_string(dir.join("docs/stories/US-GOOD.md")).unwrap();
    assert!(!good.contains("last_verified_result"));
}

#[test]
fn tool_check_requires_explicit_project_command_trust() {
    let dir = tmp("harness-tool-trust-");
    let d = dir.to_str().unwrap();
    assert!(run(&["init", d], None).status.success());
    let register = run(
        &[
            "tool",
            "register",
            "--dir",
            d,
            "--name",
            "project-check",
            "--command",
            "echo tool-ok",
            "--description",
            "Project check",
            "--responsibility",
            "Verification",
        ],
        None,
    );
    assert!(register.status.success(), "{}", stderr(&register));
    let refused = run(&["tool", "check", "--dir", d], None);
    assert!(!refused.status.success());
    let refused_text = stderr(&refused) + &stdout(&refused);
    assert!(
        refused_text.contains("--allow-project-command"),
        "{refused_text}"
    );
    let allowed = run(
        &["tool", "check", "--dir", d, "--allow-project-command"],
        None,
    );
    assert!(allowed.status.success(), "{}", stderr(&allowed));
    assert!(stdout(&allowed).contains("project-check"));
}

#[cfg(unix)]
#[test]
fn symlinked_entity_and_local_state_paths_fail_closed() {
    use std::os::unix::fs::symlink;

    let dir = tmp("harness-symlink-boundary-");
    let d = dir.to_str().unwrap();
    let init = run(&["init", d], None);
    assert!(init.status.success(), "{}", stderr(&init) + &stdout(&init));

    let outside = dir
        .parent()
        .unwrap()
        .join(format!("harness-symlink-outside-{}", std::process::id()));
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("US-ESCAPE.md"), "# escaped\n").unwrap();
    symlink(
        outside.join("US-ESCAPE.md"),
        dir.join("docs/stories/US-ESCAPE.md"),
    )
    .unwrap();

    let query = run(&["query", "matrix", "--dir", d], None);
    assert!(!query.status.success());
    let query_text = stderr(&query) + &stdout(&query);
    assert!(
        query_text.to_lowercase().contains("escapes project root"),
        "{query_text}"
    );

    let local = dir.join(".5harness/local");
    fs::create_dir_all(&local).unwrap();
    let outside_log = outside.join("traces.jsonl");
    fs::write(&outside_log, "").unwrap();
    let local_link = local.join("traces.jsonl");
    symlink(&outside_log, &local_link).unwrap();
    let trace = run(&["trace", "--dir", d, "--summary", "symlink test"], None);
    assert!(!trace.status.success());
    let trace_text = stderr(&trace) + &stdout(&trace);
    assert!(
        trace_text.to_lowercase().contains("symlink"),
        "{trace_text}"
    );
    assert!(fs::read_to_string(outside_log).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_global_registry_is_ignored() {
    use std::os::unix::fs::symlink;

    let root = tmp("harness-registry-symlink-");
    let outside = tmp("harness-registry-outside-");
    fs::write(
        outside.join("registry.json"),
        r#"{"version":1,"projects":[{"id":"escape","path":"/outside","name":"outside-secret","linked_at":"now","updated_at":"now","remote":null}]}"#,
    )
    .unwrap();
    symlink(outside.join("registry.json"), root.join("registry.json")).unwrap();
    let output = Command::new(bin())
        .args(["projects"])
        .env("HARNESS_HOME", &root)
        .env("HARNESS_NO_UPDATE_CHECK", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(!text.contains("outside-secret"), "{text}");
    assert!(!text.contains("/outside"), "{text}");
}

#[test]
fn mutations_are_serialized_and_paths_are_contained() {
    let dir = tmp("harness-hardening-");
    let d = dir.to_str().unwrap();
    let init = run(&["init", d], None);
    assert!(init.status.success(), "{}", stderr(&init) + &stdout(&init));

    let mut workers = Vec::new();
    for n in 0..8 {
        let path = dir.clone();
        workers.push(std::thread::spawn(move || {
            run(
                &[
                    "intake",
                    "--dir",
                    path.to_str().unwrap(),
                    "--type",
                    "spec_slice",
                    "--summary",
                    &format!("parallel intake {n}"),
                    "--lane",
                    "normal",
                ],
                None,
            )
        }));
    }
    for worker in workers {
        let result = worker.join().unwrap();
        assert!(
            result.status.success(),
            "{}",
            stderr(&result) + &stdout(&result)
        );
    }
    let files = fs::read_dir(dir.join("docs/intakes"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("IN-"))
        .count();
    assert_eq!(files, 8);

    let secret_name = format!("{}-secret.md", dir.file_name().unwrap().to_string_lossy());
    let outside = dir.parent().unwrap().join(format!(
        "{}-outside.md",
        dir.file_name().unwrap().to_string_lossy()
    ));
    let secret = dir.parent().unwrap().join(&secret_name);
    fs::write(&secret, "secret").unwrap();
    let relative = format!("../{secret_name}");
    let read = run(&["get", relative.as_str(), "--dir", d], None);
    assert!(!read.status.success());
    let write = run(
        &[
            "decision",
            "add",
            "--dir",
            d,
            "--id",
            "ESC",
            "--title",
            "escape",
            "--doc",
            outside.to_str().unwrap(),
        ],
        None,
    );
    assert!(!write.status.success());
    assert!(!outside.exists());
    let _ = fs::remove_file(secret);
}

#[test]
fn agent_json_views_are_structured_and_errors_are_machine_readable() {
    let dir = tmp("harness-json-hardening-");
    let d = dir.to_str().unwrap();
    assert!(run(&["init", d], None).status.success());
    assert!(run(
        &[
            "story",
            "add",
            "--dir",
            d,
            "--id",
            "US-JSON",
            "--title",
            "JSON story",
            "--lane",
            "normal"
        ],
        None
    )
    .status
    .success());
    let stories = run(&["query", "stories", "--dir", d, "--json"], None);
    let parsed: serde_json::Value = serde_json::from_slice(&stories.stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    let next = run(&["next", "--dir", d, "--limit", "1", "--json"], None);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&next.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let context = run(
        &[
            "context",
            "US-JSON",
            "--dir",
            d,
            "--max-chars",
            "8",
            "--json",
        ],
        None,
    );
    let context_json: serde_json::Value = serde_json::from_slice(&context.stdout).unwrap();
    assert!(context_json.get("links").is_some());
    let mut cmd = std::process::Command::new(bin());
    cmd.args(["get", "NOPE", "--dir", d, "--json"])
        .env("HARNESS_JSON_ERRORS", "1")
        .env(
            "HARNESS_HOME",
            std::env::temp_dir().join(format!("harness-home-json-{}", std::process::id())),
        );
    let error = cmd.output().unwrap();
    assert!(!error.status.success());
    let value: serde_json::Value = serde_json::from_slice(&error.stderr).unwrap();
    assert_eq!(value["ok"], false);
    assert!(value["code"].as_str().unwrap().starts_with("HARNESS_E_"));
}

#[test]
fn shipped_binary_is_not_typescript_cli() {
    let path = bin();
    let name = path.file_name().unwrap().to_string_lossy();
    assert!(
        name.starts_with("harness"),
        "expected harness binary, got {name}"
    );
    assert!(
        !path.to_string_lossy().contains("cli.ts"),
        "{}",
        path.display()
    );
    let bytes = fs::read(&path).unwrap();
    // PE or ELF/Mach-O magic — not a UTF-8 TypeScript source file.
    let is_pe = bytes.len() > 2 && bytes[0] == b'M' && bytes[1] == b'Z';
    let is_elf = bytes.len() > 4 && bytes[0] == 0x7f && bytes[1] == b'E';
    let is_macho = bytes.len() > 4 && (bytes[0] == 0xcf || bytes[0] == 0xca);
    assert!(
        is_pe || is_elf || is_macho,
        "shipped binary is not a native executable: {}",
        path.display()
    );
}
