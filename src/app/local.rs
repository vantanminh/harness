use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::infra::entities::{atomic_write, ensure_directory_no_symlink};

fn local_dir(project_root: &Path) -> PathBuf {
    project_root.join(".5harness").join("local")
}

fn safe_local_dir(project_root: &Path) -> Result<PathBuf> {
    let root = project_root.canonicalize()?;
    let dir = local_dir(project_root);
    ensure_directory_no_symlink(&dir)?;
    let canonical = dir.canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(Error::new(format!(
            "local state path escapes project root: {}",
            dir.display()
        )));
    }
    Ok(canonical)
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn append(project_root: &Path, kind: &str, mut value: Value) -> Result<Value> {
    let dir = safe_local_dir(project_root)?;
    let id = format!(
        "{}-{}",
        kind.to_ascii_uppercase(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    if let Value::Object(ref mut map) = value {
        map.entry("id").or_insert_with(|| Value::String(id));
        map.entry("created_at")
            .or_insert_with(|| Value::String(now()));
    } else {
        return Err(Error::new("local record must be a JSON object"));
    }
    let path = dir.join(format!("{kind}.jsonl"));
    if path
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::new(format!(
            "refusing to append through symlinked local state: {}",
            path.display()
        )));
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(&value)?)?;
    file.sync_data()?;
    Ok(value)
}

pub fn append_trace(project_root: &Path, value: Value) -> Result<Value> {
    append(project_root, "traces", value)
}

pub fn append_worklog(project_root: &Path, value: Value) -> Result<Value> {
    append(project_root, "worklog", value)
}

pub fn append_mcp_call(project_root: &Path, value: Value) -> Result<Value> {
    append(project_root, "mcp-calls", value)
}

pub fn upsert_tool(project_root: &Path, value: Value) -> Result<Value> {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::new("tool name is required"))?;
    let mut records = read_records(project_root, "tools")?;
    records.retain(|item| item.get("name").and_then(|v| v.as_str()) != Some(name));
    let mut next = value;
    if let Value::Object(ref mut map) = next {
        map.insert("updated_at".into(), Value::String(now()));
    }
    records.push(next.clone());
    write_records(project_root, "tools", &records)?;
    Ok(next)
}

pub fn remove_tool(project_root: &Path, name: &str) -> Result<bool> {
    let records = read_records(project_root, "tools")?;
    let mut next = Vec::new();
    let mut removed = false;
    for item in records {
        if item.get("name").and_then(|v| v.as_str()) == Some(name) {
            removed = true;
        } else {
            next.push(item);
        }
    }
    write_records(project_root, "tools", &next)?;
    Ok(removed)
}

pub fn read_records(project_root: &Path, kind: &str) -> Result<Vec<Value>> {
    let path = safe_local_dir(project_root)?.join(format!("{kind}.jsonl"));
    if !path.exists() {
        return Ok(Vec::new());
    }
    if !path
        .canonicalize()?
        .starts_with(project_root.canonicalize()?)
    {
        return Err(Error::new(format!(
            "local state path escapes project root: {}",
            path.display()
        )));
    }
    let file = fs::File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(&line)?);
    }
    Ok(out)
}

fn write_records(project_root: &Path, kind: &str, records: &[Value]) -> Result<()> {
    let dir = safe_local_dir(project_root)?;
    let payload = records
        .iter()
        .map(serde_json::to_string)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    atomic_write(
        &dir.join(format!("{kind}.jsonl")),
        &if payload.is_empty() {
            String::new()
        } else {
            format!("{payload}\n")
        },
    )
}

pub fn score_trace(record: &Value) -> Value {
    let mut score = 0u8;
    let mut missing = Vec::new();
    let summary_ok = record
        .get("task_summary")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.trim().chars().count() >= 10);
    if summary_ok {
        score += 1
    } else {
        missing.push("task_summary")
    }
    if record
        .get("outcome")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty())
    {
        score += 1
    } else {
        missing.push("outcome")
    }
    for field in [
        "agent",
        "actions_taken",
        "files_read",
        "files_changed",
        "harness_friction",
    ] {
        if record.get(field).is_some() {
            score += 1;
        } else {
            missing.push(field);
        }
    }
    json!({"score": score, "tier": if score >= 7 { "detailed" } else if score >= 4 { "standard" } else { "minimal" }, "missing": missing})
}

pub fn latest_trace(project_root: &Path, id: Option<&str>) -> Result<Option<Value>> {
    let records = read_records(project_root, "traces")?;
    Ok(match id {
        Some(id) => records
            .into_iter()
            .find(|v| v.get("id").and_then(|x| x.as_str()) == Some(id)),
        None => records.into_iter().last(),
    })
}

pub fn git_commits(project_root: &Path, limit: usize) -> Result<Vec<Value>> {
    let output = Command::new("git")
        .args([
            "-C",
            &project_root.to_string_lossy(),
            "log",
            &format!("-{limit}"),
            "--pretty=format:%H%x09%s",
        ])
        .output()?;
    if !output.status.success() {
        return Err(Error::new("git log failed while creating worklog"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (hash, subject) = line.split_once('\t')?;
            Some(json!({"commit":hash,"summary":subject}))
        })
        .collect())
}
