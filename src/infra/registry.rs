use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::domain::paths::{registry_file_path, resolve_harness_home};
use crate::domain::registry::{
    default_project_name, empty_registry, parse_registry_json, ProjectRegistry, RegistryProject,
};
use crate::error::Result;

use super::entities::{atomic_write, ensure_directory_no_symlink};

pub fn get_harness_home() -> PathBuf {
    resolve_harness_home()
}

pub fn get_registry_path() -> PathBuf {
    registry_file_path(&get_harness_home())
}

pub fn read_registry() -> ProjectRegistry {
    let file = get_registry_path();
    if !file.exists() {
        return empty_registry();
    }
    match fs::read_to_string(&file) {
        Ok(raw) if raw.trim().is_empty() => empty_registry(),
        Ok(raw) => parse_registry_json(&raw),
        Err(_) => empty_registry(),
    }
}

pub fn write_registry(registry: &ProjectRegistry) -> Result<PathBuf> {
    let home = get_harness_home();
    ensure_directory_no_symlink(&home)?;
    let file = registry_file_path(&home);
    let payload = format!("{}\n", serde_json::to_string_pretty(registry)?);
    atomic_write(&file, &payload)?;
    Ok(file)
}

pub fn detect_project_name(absolute_path: &Path) -> String {
    let pkg_path = absolute_path.join("package.json");
    if pkg_path.exists() {
        if let Ok(raw) = fs::read_to_string(&pkg_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
        }
    }
    default_project_name(absolute_path)
}

pub fn detect_git_remote(absolute_path: &Path) -> Option<String> {
    if !absolute_path.join(".git").exists() {
        return None;
    }
    let out = Command::new("git")
        .args([
            "-C",
            &absolute_path.to_string_lossy(),
            "config",
            "--get",
            "remote.origin.url",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

pub fn list_projects_with_status() -> Vec<(RegistryProject, bool)> {
    read_registry()
        .projects
        .into_iter()
        .map(|p| {
            let missing = !Path::new(&p.path).is_dir();
            (p, missing)
        })
        .collect()
}
