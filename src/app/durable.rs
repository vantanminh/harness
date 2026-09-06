use std::path::Path;

use crate::domain::entities::{entity_relative_path, parse_links_csv, sanitize_entity_id};
use crate::domain::enums::{
    parse_backlog_status, parse_decision_status, parse_input_type, parse_proof_flag,
    parse_risk_lane, parse_story_status,
};
use crate::domain::frontmatter::{
    as_string, as_string_array, insert_arr, insert_int, insert_null, insert_str, FmValue,
    Frontmatter,
};
use crate::error::{Error, Result};
use crate::infra::entities::{
    ensure_entity_dirs, list_entity_files, next_numeric_entity_id, read_entity_by_id,
    read_entity_file, write_entity_file, EntityFile, MutationLock,
};

use super::index::write_project_index;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

const MAX_VERIFY_COMMAND_BYTES: usize = 8 * 1024;

/// Verify commands are intentionally shell-backed, but their persisted shape
/// must stay unambiguous.  A single line also keeps frontmatter parsing and
/// operator review deterministic; execution still requires an explicit trust
/// flag in the CLI.
fn validate_verify_command(command: &str) -> Result<()> {
    if command.trim().is_empty() {
        return Err(Error::new("verify command must not be empty"));
    }
    if command.as_bytes().contains(&0) {
        return Err(Error::new("verify command must not contain NUL bytes"));
    }
    if command.contains(['\n', '\r']) {
        return Err(Error::new(
            "verify command must be a single line; split complex checks into a project script",
        ));
    }
    if command.len() > MAX_VERIFY_COMMAND_BYTES {
        return Err(Error::new(format!(
            "verify command exceeds the {}-byte limit",
            MAX_VERIFY_COMMAND_BYTES
        )));
    }
    Ok(())
}

pub(crate) fn validate_verify_command_for_cli(command: &str) -> Result<()> {
    validate_verify_command(command)
}

fn with_links(mut data: Frontmatter, links_csv: Option<&str>) -> Frontmatter {
    if let Some(links) = parse_links_csv(links_csv) {
        insert_arr(&mut data, "links", links);
    }
    data
}

pub fn maybe_reindex(project_root: &Path) -> Result<()> {
    write_project_index(project_root).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub fn add_story(
    project_root: &Path,
    id: &str,
    title: &str,
    lane: &str,
    contract: Option<&str>,
    verify: Option<&str>,
    notes: Option<&str>,
    links: Option<&str>,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let id = sanitize_entity_id(id)?;
    let lane = parse_risk_lane(lane)?;
    ensure_entity_dirs(project_root)?;
    if read_entity_by_id(project_root, "story", &id)?.is_some() {
        return Err(Error::new(format!(
            "Story {id} already exists. Use story update."
        )));
    }
    let relative = entity_relative_path("story", &id, None)?;
    let mut data = Frontmatter::new();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "story");
    insert_str(&mut data, "title", title);
    insert_str(&mut data, "status", "planned");
    insert_str(&mut data, "lane", lane);
    insert_int(&mut data, "unit", 0);
    insert_int(&mut data, "integration", 0);
    insert_int(&mut data, "e2e", 0);
    insert_int(&mut data, "platform", 0);
    set_opt(&mut data, "contract", contract);
    if let Some(verify) = verify {
        validate_verify_command(verify)?;
    }
    set_opt(&mut data, "verify", verify);
    insert_null(&mut data, "evidence");
    set_opt(&mut data, "notes", notes);
    insert_str(&mut data, "created_at", now_iso());
    insert_str(&mut data, "updated_at", now_iso());
    data = with_links(data, links);
    let body = format!("# {title}\n\n");
    let file = write_entity_file(project_root, &relative, &data, &body)?;
    maybe_reindex(project_root)?;
    Ok(file)
}

pub struct StoryUpdate {
    pub id: String,
    pub status: Option<String>,
    pub evidence: Option<String>,
    pub unit: Option<String>,
    pub integration: Option<String>,
    pub e2e: Option<String>,
    pub platform: Option<String>,
    pub verify: Option<String>,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub contract: Option<String>,
    pub links: Option<String>,
}

pub fn update_story(project_root: &Path, input: StoryUpdate) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let id = sanitize_entity_id(&input.id)?;
    ensure_entity_dirs(project_root)?;
    let file = read_entity_by_id(project_root, "story", &id)?
        .ok_or_else(|| Error::new(format!("Story {id} not found")))?;
    let mut data = file.data.clone();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "story");
    let mut changed = false;
    if let Some(status) = &input.status {
        insert_str(&mut data, "status", parse_story_status(status)?);
        changed = true;
    }
    if let Some(evidence) = &input.evidence {
        insert_str(&mut data, "evidence", evidence);
        changed = true;
    }
    if let Some(unit) = &input.unit {
        insert_int(&mut data, "unit", parse_proof_flag(unit, "unit")?);
        changed = true;
    }
    if let Some(integration) = &input.integration {
        insert_int(
            &mut data,
            "integration",
            parse_proof_flag(integration, "integration")?,
        );
        changed = true;
    }
    if let Some(e2e) = &input.e2e {
        insert_int(&mut data, "e2e", parse_proof_flag(e2e, "e2e")?);
        changed = true;
    }
    if let Some(platform) = &input.platform {
        insert_int(
            &mut data,
            "platform",
            parse_proof_flag(platform, "platform")?,
        );
        changed = true;
    }
    if let Some(verify) = &input.verify {
        validate_verify_command(verify)?;
        insert_str(&mut data, "verify", verify);
        changed = true;
    }
    if let Some(title) = &input.title {
        insert_str(&mut data, "title", title);
        changed = true;
    }
    if let Some(notes) = &input.notes {
        insert_str(&mut data, "notes", notes);
        changed = true;
    }
    if let Some(contract) = &input.contract {
        insert_str(&mut data, "contract", contract);
        changed = true;
    }
    if let Some(links) = &input.links {
        insert_arr(
            &mut data,
            "links",
            parse_links_csv(Some(links)).unwrap_or_default(),
        );
        changed = true;
    }
    if !changed {
        return Err(Error::new(
            "story update requires at least one field to change",
        ));
    }
    insert_str(&mut data, "updated_at", now_iso());
    let written = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    if as_string(&data, "status").as_deref() == Some("implemented") {
        auto_complete_eligible_intakes(project_root)?;
    }
    maybe_reindex(project_root)?;
    Ok(written)
}

pub fn record_story_verification(
    project_root: &Path,
    id: &str,
    passed: bool,
    output: &str,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let id = sanitize_entity_id(id)?;
    let file = read_entity_by_id(project_root, "story", &id)?
        .ok_or_else(|| Error::new(format!("Story {id} not found")))?;
    let mut data = file.data.clone();
    insert_str(&mut data, "last_verified_at", now_iso());
    insert_str(
        &mut data,
        "last_verified_result",
        if passed { "passed" } else { "failed" },
    );
    insert_str(&mut data, "last_verified_output", output);
    insert_str(&mut data, "updated_at", now_iso());
    let written = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    maybe_reindex(project_root)?;
    Ok(written)
}

#[allow(clippy::too_many_arguments)]
pub fn add_decision(
    project_root: &Path,
    id: &str,
    title: &str,
    status: Option<&str>,
    doc: Option<&str>,
    verify: Option<&str>,
    notes: Option<&str>,
    links: Option<&str>,
    force: bool,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let id = sanitize_entity_id(id)?;
    let status = match status {
        Some(s) => parse_decision_status(s)?,
        None => "accepted".to_string(),
    };
    ensure_entity_dirs(project_root)?;
    let relative = entity_relative_path("decision", &id, doc)?;
    if let Some(existing) = read_entity_file(project_root, &relative)? {
        if !force {
            return Err(Error::new(format!(
                "Decision {id} already exists. Use force=true to overwrite."
            )));
        }
        let created = as_string(&existing.data, "created_at").unwrap_or_else(now_iso);
        let mut data = Frontmatter::new();
        insert_str(&mut data, "id", &id);
        insert_str(&mut data, "type", "decision");
        insert_str(&mut data, "title", title);
        insert_str(&mut data, "status", status);
        insert_str(&mut data, "doc", &relative);
        if let Some(verify) = verify {
            validate_verify_command(verify)?;
        }
        set_opt(&mut data, "verify", verify);
        set_opt(&mut data, "notes", notes);
        insert_str(&mut data, "created_at", created);
        insert_str(&mut data, "updated_at", now_iso());
        data = with_links(data, links);
        let body = format!("# {title}\n\n");
        let file = write_entity_file(project_root, &relative, &data, &body)?;
        maybe_reindex(project_root)?;
        return Ok(file);
    }
    if read_entity_by_id(project_root, "decision", &id)?.is_some() && !force {
        return Err(Error::new(format!(
            "Decision {id} already exists. Use force=true to overwrite."
        )));
    }
    let mut data = Frontmatter::new();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "decision");
    insert_str(&mut data, "title", title);
    insert_str(&mut data, "status", status);
    insert_str(&mut data, "doc", &relative);
    if let Some(verify) = verify {
        validate_verify_command(verify)?;
    }
    set_opt(&mut data, "verify", verify);
    set_opt(&mut data, "notes", notes);
    insert_str(&mut data, "created_at", now_iso());
    insert_str(&mut data, "updated_at", now_iso());
    data = with_links(data, links);
    let body = format!("# {title}\n\n");
    let file = write_entity_file(project_root, &relative, &data, &body)?;
    maybe_reindex(project_root)?;
    Ok(file)
}

#[allow(clippy::too_many_arguments)]
pub fn update_decision(
    project_root: &Path,
    id: &str,
    title: Option<&str>,
    status: Option<&str>,
    doc: Option<&str>,
    verify: Option<&str>,
    notes: Option<&str>,
    links: Option<&str>,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let id = sanitize_entity_id(id)?;
    let file = read_entity_by_id(project_root, "decision", &id)?
        .ok_or_else(|| Error::new(format!("Decision {id} not found. Use decision add.")))?;
    let mut data = file.data.clone();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "decision");
    let mut changed = false;
    if let Some(title) = title {
        insert_str(&mut data, "title", title);
        changed = true;
    }
    if let Some(status) = status {
        insert_str(&mut data, "status", parse_decision_status(status)?);
        changed = true;
    }
    if let Some(doc) = doc {
        insert_str(&mut data, "doc", doc);
        changed = true;
    }
    if let Some(verify) = verify {
        validate_verify_command(verify)?;
        insert_str(&mut data, "verify", verify);
        changed = true;
    }
    if let Some(notes) = notes {
        insert_str(&mut data, "notes", notes);
        changed = true;
    }
    if let Some(links) = links {
        insert_arr(
            &mut data,
            "links",
            parse_links_csv(Some(links)).unwrap_or_default(),
        );
        changed = true;
    }
    if !changed {
        return Err(Error::new(
            "decision update requires at least one field to change",
        ));
    }
    insert_str(&mut data, "updated_at", now_iso());
    let file = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    maybe_reindex(project_root)?;
    Ok(file)
}

pub fn record_decision_verification(
    project_root: &Path,
    id: &str,
    passed: bool,
    output: &str,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let id = sanitize_entity_id(id)?;
    let file = read_entity_by_id(project_root, "decision", &id)?
        .ok_or_else(|| Error::new(format!("Decision {id} not found")))?;
    let mut data = file.data.clone();
    insert_str(&mut data, "last_verified_at", now_iso());
    insert_str(
        &mut data,
        "last_verified_result",
        if passed { "passed" } else { "failed" },
    );
    insert_str(&mut data, "last_verified_output", output);
    insert_str(&mut data, "updated_at", now_iso());
    let written = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    maybe_reindex(project_root)?;
    Ok(written)
}

#[allow(clippy::too_many_arguments)]
pub fn add_intake(
    project_root: &Path,
    input_type: &str,
    summary: &str,
    lane: &str,
    flags: Option<&str>,
    docs: Option<&str>,
    story: Option<&str>,
    stories: Option<&str>,
    notes: Option<&str>,
    links: Option<&str>,
) -> Result<(EntityFile, String)> {
    let _lock = MutationLock::acquire(project_root)?;
    let input_type = parse_input_type(input_type)?;
    let lane = parse_risk_lane(lane)?;
    ensure_entity_dirs(project_root)?;
    let id = next_numeric_entity_id(project_root, "intake", "IN-")?;
    let relative = entity_relative_path("intake", &id, None)?;
    let mut story_ids = Vec::new();
    if let Some(s) = story {
        story_ids.push(sanitize_entity_id(s)?);
    }
    if let Some(extra) = parse_links_csv(stories) {
        for s in extra {
            let sid = sanitize_entity_id(&s)?;
            if !story_ids.contains(&sid) {
                story_ids.push(sid);
            }
        }
    }
    let mut all_links = parse_links_csv(links).unwrap_or_default();
    for s in &story_ids {
        if !all_links.contains(s) {
            all_links.push(s.clone());
        }
    }
    let mut data = Frontmatter::new();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "intake");
    insert_str(&mut data, "status", "pending");
    insert_str(&mut data, "input_type", input_type);
    insert_str(&mut data, "summary", summary);
    insert_str(&mut data, "lane", lane);
    set_opt(&mut data, "flags", flags);
    set_opt(&mut data, "docs", docs);
    if let Some(s) = story {
        insert_str(&mut data, "story", sanitize_entity_id(s)?);
    } else {
        insert_null(&mut data, "story");
    }
    insert_arr(&mut data, "stories", story_ids);
    set_opt(&mut data, "notes", notes);
    insert_str(&mut data, "created_at", now_iso());
    insert_str(&mut data, "updated_at", now_iso());
    insert_arr(&mut data, "links", all_links);
    let body = format!("# Intake {id}\n\n{summary}\n");
    let file = write_entity_file(project_root, &relative, &data, &body)?;
    maybe_reindex(project_root)?;
    Ok((file, id))
}

pub fn update_intake(
    project_root: &Path,
    id: &str,
    status: Option<&str>,
    stories: Option<&str>,
    notes: Option<&str>,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    update_intake_inner(project_root, id, status, stories, notes)
}

fn update_intake_inner(
    project_root: &Path,
    id: &str,
    status: Option<&str>,
    stories: Option<&str>,
    notes: Option<&str>,
) -> Result<EntityFile> {
    let id = sanitize_entity_id(id)?;
    ensure_entity_dirs(project_root)?;
    let file = read_entity_by_id(project_root, "intake", &id)?
        .ok_or_else(|| Error::new(format!("Intake {id} not found")))?;
    let mut data = file.data.clone();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "intake");
    let mut changed = false;
    if let Some(status) = status {
        insert_str(
            &mut data,
            "status",
            crate::domain::enums::parse_intake_status(status)?,
        );
        changed = true;
    }
    if let Some(stories_csv) = stories {
        let previous = as_string_array(&data, "stories").unwrap_or_default();
        let stories: Vec<String> = parse_links_csv(Some(stories_csv))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| sanitize_entity_id(&s).ok())
            .collect();
        insert_arr(&mut data, "stories", stories.clone());
        let mut links = as_string_array(&data, "links").unwrap_or_default();
        links.retain(|l| !previous.contains(l));
        for s in stories {
            if !links.contains(&s) {
                links.push(s);
            }
        }
        insert_arr(&mut data, "links", links);
        changed = true;
    }
    if let Some(notes) = notes {
        insert_str(&mut data, "notes", notes);
        changed = true;
    }
    if !changed {
        return Err(Error::new(
            "intake update requires status, stories, or notes",
        ));
    }
    insert_str(&mut data, "updated_at", now_iso());
    let file = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    maybe_reindex(project_root)?;
    Ok(file)
}

fn auto_complete_eligible_intakes(project_root: &Path) -> Result<Vec<EntityFile>> {
    let story_statuses: Vec<(String, String)> = list_entity_files(project_root, "story")?
        .into_iter()
        .map(|s| {
            (
                as_string(&s.data, "id").unwrap_or_default(),
                as_string(&s.data, "status").unwrap_or_default(),
            )
        })
        .collect();
    let mut completed = Vec::new();
    for intake in list_entity_files(project_root, "intake")? {
        let status = as_string(&intake.data, "status").unwrap_or_default();
        if !status.is_empty() && status != "pending" {
            continue;
        }
        let mut stories = as_string_array(&intake.data, "stories").unwrap_or_default();
        if stories.is_empty() {
            if let Some(legacy) = as_string(&intake.data, "story") {
                stories.push(legacy);
            }
        }
        if stories.is_empty() {
            continue;
        }
        if !stories.iter().all(|sid| {
            story_statuses
                .iter()
                .any(|(id, st)| id == sid && st == "implemented")
        }) {
            continue;
        }
        if let Some(id) = as_string(&intake.data, "id") {
            completed.push(update_intake_inner(
                project_root,
                &id,
                Some("completed"),
                None,
                None,
            )?);
        }
    }
    Ok(completed)
}

#[allow(clippy::too_many_arguments)]
pub fn add_backlog(
    project_root: &Path,
    title: &str,
    while_text: Option<&str>,
    pain: Option<&str>,
    suggestion: Option<&str>,
    risk: Option<&str>,
    predicted: Option<&str>,
    notes: Option<&str>,
    links: Option<&str>,
) -> Result<(EntityFile, String)> {
    let _lock = MutationLock::acquire(project_root)?;
    let risk = match risk {
        Some(r) => Some(parse_risk_lane(r)?),
        None => None,
    };
    ensure_entity_dirs(project_root)?;
    let id = next_numeric_entity_id(project_root, "backlog", "BL-")?;
    let relative = entity_relative_path("backlog", &id, None)?;
    let mut data = Frontmatter::new();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "backlog");
    insert_str(&mut data, "title", title);
    insert_str(&mut data, "status", "proposed");
    if let Some(risk) = risk {
        insert_str(&mut data, "risk", risk);
    } else {
        insert_null(&mut data, "risk");
    }
    set_opt(&mut data, "discovered_while", while_text);
    set_opt(&mut data, "pain", pain);
    set_opt(&mut data, "suggestion", suggestion);
    set_opt(&mut data, "predicted", predicted);
    insert_null(&mut data, "outcome");
    set_opt(&mut data, "notes", notes);
    insert_str(&mut data, "created_at", now_iso());
    insert_str(&mut data, "updated_at", now_iso());
    data = with_links(data, links);
    let body = format!("# {title}\n\n");
    let file = write_entity_file(project_root, &relative, &data, &body)?;
    maybe_reindex(project_root)?;
    Ok((file, id))
}

pub fn close_backlog(
    project_root: &Path,
    id: &str,
    status: Option<&str>,
    outcome: Option<&str>,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    ensure_entity_dirs(project_root)?;
    let file = read_entity_by_id(project_root, "backlog", id)?
        .ok_or_else(|| Error::new(format!("Backlog item {id} not found")))?;
    let status = match status {
        Some(s) => parse_backlog_status(s)?,
        None => "implemented".to_string(),
    };
    if status != "implemented" && status != "rejected" {
        return Err(Error::new(format!(
            "backlog close status must be implemented or rejected (got {status})"
        )));
    }
    let mut data = file.data.clone();
    insert_str(&mut data, "type", "backlog");
    insert_str(&mut data, "status", status);
    if let Some(outcome) = outcome {
        insert_str(&mut data, "outcome", outcome);
    } else {
        insert_null(&mut data, "outcome");
    }
    insert_str(&mut data, "updated_at", now_iso());
    let file = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    maybe_reindex(project_root)?;
    Ok(file)
}

pub fn add_report(
    project_root: &Path,
    summary: &str,
    severity: Option<&str>,
    from_project: Option<&str>,
    related: Option<&str>,
) -> Result<(EntityFile, String)> {
    let _lock = MutationLock::acquire(project_root)?;
    if summary.trim().is_empty() {
        return Err(Error::new("report summary must not be empty"));
    }
    ensure_entity_dirs(project_root)?;
    let id = next_numeric_entity_id(project_root, "report", "RP-")?;
    let relative = entity_relative_path("report", &id, None)?;
    let mut data = Frontmatter::new();
    insert_str(&mut data, "id", &id);
    insert_str(&mut data, "type", "report");
    insert_str(&mut data, "status", "open");
    insert_str(&mut data, "summary", summary);
    set_opt(&mut data, "severity", severity);
    set_opt(&mut data, "from_project", from_project);
    set_opt(&mut data, "resolution", None);
    insert_arr(
        &mut data,
        "related",
        parse_links_csv(related).unwrap_or_default(),
    );
    insert_str(&mut data, "created_at", now_iso());
    insert_str(&mut data, "updated_at", now_iso());
    let body = format!("# Report {id}\n\n{summary}\n");
    let file = write_entity_file(project_root, &relative, &data, &body)?;
    maybe_reindex(project_root)?;
    Ok((file, id))
}

pub fn update_report(
    project_root: &Path,
    id: &str,
    status: &str,
    resolution: Option<&str>,
    related: Option<&str>,
) -> Result<EntityFile> {
    let _lock = MutationLock::acquire(project_root)?;
    let allowed = ["open", "acked", "fixed", "wontfix", "needs_info"];
    if !allowed.contains(&status) {
        return Err(Error::new(format!("invalid report status {status}")));
    }
    if status == "fixed"
        && resolution
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .is_none()
    {
        return Err(Error::new("fixed reports require --resolution"));
    }
    let id = sanitize_entity_id(id)?;
    let file = read_entity_by_id(project_root, "report", &id)?
        .ok_or_else(|| Error::new(format!("Report {id} not found")))?;
    let mut data = file.data.clone();
    insert_str(&mut data, "type", "report");
    insert_str(&mut data, "status", status);
    if let Some(resolution) = resolution {
        insert_str(&mut data, "resolution", resolution);
    }
    if let Some(related) = related {
        insert_arr(
            &mut data,
            "related",
            parse_links_csv(Some(related)).unwrap_or_default(),
        );
    }
    insert_str(&mut data, "updated_at", now_iso());
    let written = write_entity_file(project_root, &file.relative_path, &data, &file.body)?;
    maybe_reindex(project_root)?;
    Ok(written)
}

fn set_opt(data: &mut Frontmatter, key: &str, value: Option<&str>) {
    match value {
        Some(v) => insert_str(data, key, v),
        None => insert_null(data, key),
    }
}

pub fn get_entity(project_root: &Path, id_or_path: &str) -> Result<Option<EntityFile>> {
    let catalog = super::catalog::build_catalog(project_root)?;
    if let Some(entry) = catalog.entries.iter().find(|e| {
        e.id == id_or_path
            || e.path == id_or_path
            || Path::new(&e.path).file_stem().and_then(|s| s.to_str()) == Some(id_or_path)
    }) {
        return read_entity_file(project_root, &entry.path);
    }
    if id_or_path.ends_with(".md") || id_or_path.contains('/') || id_or_path.contains('\\') {
        return read_entity_file(project_root, id_or_path);
    }
    Ok(None)
}

pub fn fm_to_yaml(data: &Frontmatter) -> String {
    crate::domain::frontmatter::serialize_entity_file(data, "")
        .replace("---\n", "")
        .trim_end_matches("---\n")
        .to_string()
}

pub fn fm_json(data: &Frontmatter) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in data {
        map.insert(
            k.clone(),
            match v {
                FmValue::Null => serde_json::Value::Null,
                FmValue::Bool(b) => serde_json::Value::Bool(*b),
                FmValue::Int(n) => serde_json::json!(*n),
                FmValue::Float(n) => serde_json::json!(*n),
                FmValue::Str(s) => serde_json::Value::String(s.clone()),
                FmValue::Arr(a) => serde_json::Value::Array(
                    a.iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            },
        );
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::validate_verify_command;

    #[test]
    fn verify_commands_are_single_line_and_bounded() {
        assert!(validate_verify_command("cargo test --all-targets").is_ok());
        assert!(validate_verify_command("").is_err());
        assert!(validate_verify_command("echo first\necho second").is_err());
        assert!(validate_verify_command("echo\r\nnext").is_err());
        assert!(validate_verify_command("echo\0secret").is_err());
        assert!(validate_verify_command(&"x".repeat(8 * 1024 + 1)).is_err());
    }
}
