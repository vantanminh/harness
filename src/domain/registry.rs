use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REGISTRY_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryProject {
    pub id: String,
    pub path: String,
    pub name: String,
    pub linked_at: String,
    pub updated_at: String,
    pub remote: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectRegistry {
    pub version: u32,
    pub projects: Vec<RegistryProject>,
}

pub fn empty_registry() -> ProjectRegistry {
    ProjectRegistry {
        version: REGISTRY_VERSION,
        projects: Vec::new(),
    }
}

pub fn normalize_project_path(absolute_path: &Path) -> PathBuf {
    let resolved = if absolute_path.is_absolute() {
        absolute_path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(absolute_path)
    };
    #[cfg(windows)]
    {
        let s = resolved.to_string_lossy();
        if s.len() >= 2 && s.as_bytes()[1] == b':' {
            let mut chars: Vec<char> = s.chars().collect();
            chars[0] = chars[0].to_ascii_uppercase();
            return PathBuf::from(chars.into_iter().collect::<String>());
        }
    }
    resolved
}

pub fn project_id_from_path(absolute_path: &Path) -> String {
    let normalized = normalize_project_path(absolute_path);
    let mut hasher = Sha256::new();
    hasher.update(normalized.to_string_lossy().as_bytes());
    hex::encode(&hasher.finalize()[..8])
}

pub fn default_project_name(absolute_path: &Path) -> String {
    normalize_project_path(absolute_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| absolute_path.display().to_string())
}

pub fn paths_equal(a: &Path, b: &Path) -> bool {
    let na = normalize_project_path(a);
    let nb = normalize_project_path(b);
    #[cfg(windows)]
    {
        na.to_string_lossy()
            .eq_ignore_ascii_case(&nb.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        na == nb
    }
}

pub fn find_project_by_path<'a>(
    registry: &'a ProjectRegistry,
    absolute_path: &Path,
) -> Option<&'a RegistryProject> {
    registry
        .projects
        .iter()
        .find(|p| paths_equal(Path::new(&p.path), absolute_path))
}

pub fn upsert_project(
    registry: &ProjectRegistry,
    id: Option<String>,
    path: &Path,
    name: &str,
    remote: Option<String>,
    now: &str,
) -> Result<(ProjectRegistry, RegistryProject, bool), String> {
    let absolute_path = normalize_project_path(path);
    let existing_by_path = find_project_by_path(registry, &absolute_path).cloned();
    let existing_by_id = id
        .as_ref()
        .and_then(|wanted| registry.projects.iter().find(|p| p.id == *wanted).cloned());
    if let (Some(by_path), Some(by_id)) = (&existing_by_path, &existing_by_id) {
        if by_path.id != by_id.id {
            return Err(format!(
                "Registry conflict: {} and project id {} identify different entries.",
                absolute_path.display(),
                by_id.id
            ));
        }
    }
    let mut next = registry.clone();
    if let Some(existing) = existing_by_path.or(existing_by_id) {
        let updated = RegistryProject {
            id: id.unwrap_or(existing.id.clone()),
            path: absolute_path.to_string_lossy().into_owned(),
            name: if name.is_empty() {
                existing.name.clone()
            } else {
                name.to_string()
            },
            linked_at: existing.linked_at.clone(),
            updated_at: now.to_string(),
            remote: remote.or(existing.remote),
        };
        for p in &mut next.projects {
            if p.id == existing.id {
                *p = updated.clone();
            }
        }
        return Ok((next, updated, false));
    }
    let created = RegistryProject {
        id: id.unwrap_or_else(|| project_id_from_path(&absolute_path)),
        path: absolute_path.to_string_lossy().into_owned(),
        name: if name.is_empty() {
            default_project_name(&absolute_path)
        } else {
            name.to_string()
        },
        linked_at: now.to_string(),
        updated_at: now.to_string(),
        remote,
    };
    next.projects.push(created.clone());
    Ok((next, created, true))
}

pub fn remove_project_by_path(
    registry: &ProjectRegistry,
    absolute_path: &Path,
) -> (ProjectRegistry, Option<RegistryProject>) {
    let existing = find_project_by_path(registry, absolute_path).cloned();
    let Some(existing) = existing else {
        return (registry.clone(), None);
    };
    let projects = registry
        .projects
        .iter()
        .filter(|p| p.id != existing.id)
        .cloned()
        .collect();
    (
        ProjectRegistry {
            version: registry.version,
            projects,
        },
        Some(existing),
    )
}

pub fn parse_registry_json(raw: &str) -> ProjectRegistry {
    serde_json::from_str(raw).unwrap_or_else(|_| empty_registry())
}
