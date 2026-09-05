use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domain::conflicts::{blocking_conflicts, classify_file_plan, PlannedWrite};
use crate::domain::entities::ENTITY_TYPES;
use crate::domain::paths::{
    project_backup_root, resolve_db_path, resolve_target_dir, PROJECT_STATE_DIRNAME,
    SQLITE_DB_BASENAME,
};
use crate::domain::project_id::{extract_project_id, insert_project_id_marker};
use crate::domain::upgrade::{extract_harness_block, replace_harness_block};
use crate::error::{Error, Result};
use crate::infra::entities::{atomic_write, ensure_directory_no_symlink, ensure_entity_dirs};
use crate::infra::package_root::resolve_package_root;

use super::link::link_project;

pub const GITIGNORE_RULES: &[&str] = &[
    "# 5harness local / derived (not SoT)",
    ".5harness/index/",
    ".5harness/local/",
    ".5harness/mutation.lock",
    "# Optional SQLite import residue (not SoT)",
    SQLITE_DB_BASENAME,
    "harness.db-wal",
    "harness.db-shm",
];

#[derive(Deserialize)]
struct Manifest {
    files: Vec<String>,
}

pub struct InitResult {
    pub target_dir: PathBuf,
    pub created: Vec<String>,
    pub overwritten: Vec<String>,
    pub skipped: Vec<String>,
    pub dry_run: bool,
    pub registered: bool,
    pub registry_path: Option<PathBuf>,
    pub logs: Vec<String>,
}

pub fn run_init(
    directory: Option<&str>,
    force: bool,
    dry_run: bool,
    cwd: &Path,
    skip_register: bool,
) -> Result<InitResult> {
    let package_root = resolve_package_root()?;
    let target_dir = resolve_target_dir(directory, cwd);
    let db_path = resolve_db_path(&target_dir);
    let manifest_path = package_root.join("templates").join("manifest.json");
    let manifest: Manifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    if manifest.files.is_empty() {
        return Err(Error::new(
            "templates/manifest.json must list at least one file",
        ));
    }

    let agents_path = target_dir.join("AGENTS.md");
    let existing_agents_text = fs::read_to_string(&agents_path).ok();
    let existing_project_id = existing_agents_text.as_deref().and_then(extract_project_id);

    let mut plans: Vec<PlannedWrite> = manifest
        .files
        .iter()
        .map(|rel| classify_file_plan(&target_dir, rel, force))
        .collect();
    plans.push(plan_gitignore(&target_dir));

    let blockers = blocking_conflicts(&plans, force);
    if !blockers.is_empty() {
        return Err(Error::new(format!(
            "Refusing to overwrite protected paths without --force: {}",
            blockers.join(", ")
        )));
    }

    let mut logs = Vec::new();
    let mut created = Vec::new();
    let mut overwritten = Vec::new();
    let mut skipped = Vec::new();

    for plan in &plans {
        match plan {
            PlannedWrite::Create { relative } => {
                logs.push(format!("create   {relative}"));
                if !dry_run {
                    write_template_file(&package_root, &target_dir, relative)?;
                }
                created.push(relative.clone());
            }
            PlannedWrite::Overwrite { relative } => {
                logs.push(format!("overwrite {relative}"));
                if !dry_run {
                    let backup = backup_file(&target_dir, relative)?;
                    logs.push(format!("  backup  {backup}"));
                    write_template_file(&package_root, &target_dir, relative)?;
                }
                overwritten.push(relative.clone());
            }
            PlannedWrite::Skip { relative, reason } => {
                logs.push(format!("skip     {relative} ({reason})"));
                skipped.push(relative.clone());
            }
            PlannedWrite::Gitignore { action } => {
                logs.push(format!("gitignore {action}"));
                if !dry_run {
                    apply_gitignore(&target_dir, action)?;
                }
            }
            PlannedWrite::Db { action, path } => {
                logs.push(format!("db       {action} {path}"));
            }
        }
    }

    let mut registered = false;
    let mut registry_path = None;

    if !dry_run {
        ensure_directory_no_symlink(&target_dir)?;
        ensure_entity_dirs(&target_dir)?;
        write_entity_dir_readmes(&target_dir)?;
        ensure_project_id(&target_dir, existing_project_id.as_deref())?;
        if let Some(existing) = existing_agents_text {
            let initialized = fs::read_to_string(&agents_path)?;
            if let Some(block) = extract_harness_block(&initialized) {
                // Preserve existing project-id if we rewrote AGENTS.md.
                let restored = if let Some(id) = extract_project_id(&existing) {
                    if extract_project_id(&initialized).is_none() {
                        insert_project_id_marker(&initialized, &id).unwrap_or(initialized.clone())
                    } else {
                        initialized.clone()
                    }
                } else {
                    initialized.clone()
                };
                let _ = (block, replace_harness_block);
                if restored != initialized {
                    atomic_write(&agents_path, &restored)?;
                }
            }
        }
        logs.push("dirs     entity markdown directories ready".into());
        let _ = db_path;
        let _ = PROJECT_STATE_DIRNAME;
        if !skip_register {
            match link_project(Some(&target_dir.to_string_lossy()), cwd) {
                Ok(link) => {
                    registered = true;
                    registry_path = Some(link.registry_path.clone());
                    logs.push(format!(
                        "register {} → {}",
                        if link.created { "linked" } else { "updated" },
                        link.registry_path.display()
                    ));
                }
                Err(err) => {
                    return Err(Error::new(format!(
                        "project initialized but registry registration failed: {err}"
                    )))
                }
            }
        }
    } else {
        logs.push("dry-run  no files, database, or registry written".into());
        logs.push("plan     would ensure entity dirs + register project".into());
    }

    Ok(InitResult {
        target_dir,
        created,
        overwritten,
        skipped,
        dry_run,
        registered,
        registry_path,
        logs,
    })
}

fn plan_gitignore(target_dir: &Path) -> PlannedWrite {
    let gitignore_path = target_dir.join(".gitignore");
    if !gitignore_path.exists() {
        return PlannedWrite::Gitignore {
            action: "create".into(),
        };
    }
    let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();
    let missing: Vec<_> = GITIGNORE_RULES
        .iter()
        .filter(|rule| !existing.contains(*rule))
        .collect();
    if missing.is_empty() {
        PlannedWrite::Gitignore {
            action: "skip".into(),
        }
    } else {
        PlannedWrite::Gitignore {
            action: "append".into(),
        }
    }
}

fn write_template_file(package_root: &Path, target_dir: &Path, relative: &str) -> Result<()> {
    let source = package_root.join("templates").join(relative);
    if !source.exists() {
        return Err(Error::new(format!("Template missing: {relative}")));
    }
    let dest = target_dir.join(relative);
    if let Some(parent) = dest.parent() {
        ensure_directory_no_symlink(parent)?;
    }
    if dest
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::new(format!(
            "refusing to overwrite symlinked template path: {}",
            dest.display()
        )));
    }
    fs::copy(source, dest)?;
    Ok(())
}

fn backup_file(target_dir: &Path, relative: &str) -> Result<String> {
    let stamp = chrono::Utc::now().to_rfc3339().replace([':', '.'], "-");
    let backup_root = project_backup_root(target_dir, &stamp);
    let dest = backup_root.join(relative);
    if let Some(parent) = dest.parent() {
        ensure_directory_no_symlink(parent)?;
    }
    let source = target_dir.join(relative);
    if let Some(parent) = source.parent() {
        ensure_directory_no_symlink(parent)?;
    }
    if source
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::new(format!(
            "refusing to back up symlinked path: {}",
            source.display()
        )));
    }
    fs::copy(source, &dest)?;
    Ok(dest
        .strip_prefix(target_dir)
        .unwrap_or(&dest)
        .to_string_lossy()
        .replace('\\', "/"))
}

fn apply_gitignore(target_dir: &Path, action: &str) -> Result<()> {
    if action == "skip" {
        return Ok(());
    }
    let gitignore_path = target_dir.join(".gitignore");
    ensure_directory_no_symlink(target_dir)?;
    if gitignore_path
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::new(format!(
            "refusing to update symlinked .gitignore: {}",
            gitignore_path.display()
        )));
    }
    if action == "create" {
        fs::write(&gitignore_path, format!("{}\n", GITIGNORE_RULES.join("\n")))?;
        return Ok(());
    }
    let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();
    let missing: Vec<_> = GITIGNORE_RULES
        .iter()
        .filter(|rule| !existing.contains(*rule))
        .copied()
        .collect();
    let prefix = if !existing.is_empty() && !existing.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let mut f = existing;
    f.push_str(prefix);
    if !f.is_empty() && !f.ends_with('\n') {
        f.push('\n');
    }
    f.push_str(&missing.join("\n"));
    f.push('\n');
    fs::write(gitignore_path, f)?;
    Ok(())
}

fn write_entity_dir_readmes(target_dir: &Path) -> Result<()> {
    let readmes = [
        (
            "docs/stories",
            "# Stories\n\nOperational story entities (`US-*.md`) are managed via `harness story`.\n",
        ),
        (
            "docs/decisions",
            "# Decisions\n\nDecision entities are managed via `harness decision`.\n",
        ),
        (
            "docs/intakes",
            "# Intakes\n\nIntake entities (`IN-*.md`) are managed via `harness intake`.\n",
        ),
        (
            "docs/backlog",
            "# Backlog\n\nBacklog entities (`BL-*.md`) are managed via `harness backlog`.\n",
        ),
    ];
    for (rel, content) in readmes {
        let dir = target_dir.join(rel);
        ensure_directory_no_symlink(&dir)?;
        let readme = dir.join("README.md");
        if readme.symlink_metadata().is_err() {
            fs::write(readme, content)?;
        } else if readme
            .symlink_metadata()
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(Error::new(format!(
                "refusing to write symlinked entity README: {}",
                readme.display()
            )));
        }
    }
    let _ = ENTITY_TYPES;
    Ok(())
}

pub fn ensure_project_id(project_root: &Path, preferred: Option<&str>) -> Result<String> {
    let agents_path = project_root.join("AGENTS.md");
    if !agents_path.exists() {
        return Err(Error::new(format!(
            "AGENTS.md not found in {}. Run `harness init` first.",
            project_root.display()
        )));
    }
    let current = fs::read_to_string(&agents_path)?;
    if let Some(existing) = extract_project_id(&current) {
        return Ok(existing);
    }
    let id = preferred
        .map(|s| s.to_string())
        .unwrap_or_else(crate::domain::project_id::generate_project_id);
    let updated = insert_project_id_marker(&current, &id)?;
    atomic_write(&agents_path, &updated)?;
    Ok(id)
}

pub fn run_migrate(target_dir: &Path) -> String {
    let db = resolve_db_path(target_dir);
    if db.exists() {
        format!(
            "Found legacy {}. Markdown is SoT; nothing to migrate. Use import-sqlite to convert rows.",
            db.display()
        )
    } else {
        "No harness.db present — markdown is SoT, nothing to migrate.".to_string()
    }
}
