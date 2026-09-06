use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::domain::paths::{is_loopback_bind_host, is_valid_public_https_url};
use crate::domain::project_id::extract_project_id;
use crate::error::{Error, Result};
use crate::VERSION;

use super::dashboard::RunningServer;
use super::durable::{
    add_backlog, add_decision, add_intake, add_report, add_story, get_entity, update_report,
    update_story, StoryUpdate,
};
use super::index::{ensure_index, format_search_hits, search_index};
use super::project_link;
use super::query::{query_matrix, query_stats, query_view_json};
use super::status::{doctor_json, next_items, status_json};
use crate::infra::entities::MutationLock;

const MAX_MCP_BODY_BYTES: usize = 1_048_576;
const MAX_MCP_HEADER_VALUE_BYTES: usize = 16 * 1024;
const MAX_MCP_HEADERS: usize = 64;
const MAX_MCP_HEADER_BYTES: usize = 64 * 1024;
const MAX_MCP_STRING_BYTES: usize = 64 * 1024;
const MAX_MCP_DEPTH: usize = 32;
const MAX_MCP_COLLECTION_ITEMS: usize = 1_000;
const DEFAULT_MCP_TOKEN_TTL_SECS: u64 = 86_400;
const DEFAULT_MCP_RATE_LIMIT_PER_MINUTE: u32 = 120;
const MAX_MCP_RATE_LIMIT_BUCKETS: usize = 4_096;
const MCP_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

struct RateLimiter {
    limit: u32,
    buckets: HashMap<String, (Instant, u32)>,
}

impl RateLimiter {
    fn new(limit: u32) -> Self {
        Self {
            limit: limit.max(1),
            buckets: HashMap::new(),
        }
    }

    fn allow(&mut self, remote: Option<&SocketAddr>) -> bool {
        let now = Instant::now();
        self.buckets
            .retain(|_, (started, _)| now.duration_since(*started) < MCP_RATE_LIMIT_WINDOW);
        let key = remote
            .map(|address| address.ip().to_string())
            .unwrap_or_else(|| "<unknown>".into());
        if !self.buckets.contains_key(&key) && self.buckets.len() >= MAX_MCP_RATE_LIMIT_BUCKETS {
            return false;
        }
        let entry = self.buckets.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= MCP_RATE_LIMIT_WINDOW {
            *entry = (now, 0);
        }
        entry.1 = entry.1.saturating_add(1);
        entry.1 <= self.limit
    }
}

pub fn mcp_tools() -> Value {
    json!([
        {"name":"harness_get","description":"Get a durable entity by ID or path.","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}},
        {"name":"harness_search","description":"Search entity catalog with ranked hits and snippets.","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}},
        {"name":"harness_query_matrix","description":"Story matrix: all stories with status, proof, evidence.","inputSchema":{"type":"object","properties":{}}},
        {"name":"harness_query_stats","description":"Summary counts by category.","inputSchema":{"type":"object","properties":{}}},
        {"name":"harness_next","description":"Ranked next work items.","inputSchema":{"type":"object","properties":{"limit":{"type":"integer"}}}},
        {"name":"harness_context","description":"Read bounded context for one local entity.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"max_chars":{"type":"integer"}},"required":["id"]}},
        {"name":"harness_links","description":"Read outbound links and backlinks.","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}},
        {"name":"harness_doctor","description":"Run structured workspace health checks.","inputSchema":{"type":"object","properties":{}}},
        {"name":"harness_status","description":"Project snapshot: work counts, Project Link role/peers/reports, version, index.","inputSchema":{"type":"object","properties":{}}},
        {"name":"harness_intake","description":"Record a feature intake. Mutates durable markdown.","inputSchema":{"type":"object","properties":{"type":{"type":"string"},"summary":{"type":"string"},"lane":{"type":"string"}},"required":["type","summary","lane"]}},
        {"name":"harness_story_add","description":"Add a story. Mutates durable markdown.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"title":{"type":"string"},"lane":{"type":"string"}},"required":["id","title","lane"]}},
        {"name":"harness_story_update","description":"Update a story. Mutates durable markdown.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"status":{"type":"string"},"evidence":{"type":"string"},"unit":{"type":"string"},"integration":{"type":"string"},"e2e":{"type":"string"},"platform":{"type":"string"}},"required":["id"]}},
        {"name":"harness_decision_add","description":"Add a decision. Mutates durable markdown.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"title":{"type":"string"}},"required":["id","title"]}},
        {"name":"harness_backlog_add","description":"Add a backlog item. Mutates durable markdown.","inputSchema":{"type":"object","properties":{"title":{"type":"string"},"risk":{"type":"string"}},"required":["title"]}},
        {"name":"harness_reindex","description":"Rebuild the derived project index.","inputSchema":{"type":"object","properties":{}}},
        {"name":"harness_project_role","description":"Read local Project Link role and stack.","inputSchema":{"type":"object","properties":{}}},
        {"name":"harness_project_peers","description":"List configured Project Link peers.","inputSchema":{"type":"object","properties":{}}}
    ])
}

fn mcp_tools_for_root(root: Option<&PathBuf>) -> Value {
    let mut tools = mcp_tools().as_array().cloned().unwrap_or_default();
    if let Some(root) = root {
        if project_link::peers(root)
            .map(|p| !p.is_empty())
            .unwrap_or(false)
        {
            tools.extend([
                json!({"name":"harness_peer_search","description":"Search one configured peer.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"peer_id":{"type":"string"},"role":{"type":"string"}},"required":["query"]}}),
                json!({"name":"harness_peer_get","description":"Get one entity from a configured peer.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"peer_id":{"type":"string"},"role":{"type":"string"}},"required":["id"]}}),
                json!({"name":"harness_peer_context","description":"Read bounded context from a configured peer.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"peer_id":{"type":"string"},"role":{"type":"string"},"max_chars":{"type":"integer"}},"required":["id"]}}),
                json!({"name":"harness_peer_links","description":"Read links for one configured peer entity.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"peer_id":{"type":"string"},"role":{"type":"string"}},"required":["id"]}}),
                json!({"name":"harness_report_add","description":"Create a target-owned report on one configured peer.","inputSchema":{"type":"object","properties":{"to":{"type":"string"},"summary":{"type":"string"}},"required":["to","summary"]}}),
                json!({"name":"harness_report_list","description":"List local reports.","inputSchema":{"type":"object","properties":{"status":{"type":"string"}}}}),
                json!({"name":"harness_report_get","description":"Get one local report.","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}}),
                json!({"name":"harness_report_update","description":"Update a local report.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"status":{"type":"string"},"resolution":{"type":"string"}},"required":["id","status"]}}),
            ]);
        }
    }
    Value::Array(tools)
}

pub fn handle_mcp_request(root: Option<&PathBuf>, body: &str) -> Value {
    handle_mcp_request_with_auth(root, body, false)
}

fn json_within_limits(value: &Value, depth: usize) -> bool {
    if depth > MAX_MCP_DEPTH {
        return false;
    }
    match value {
        Value::String(text) => text.len() <= MAX_MCP_STRING_BYTES,
        Value::Array(items) => {
            items.len() <= MAX_MCP_COLLECTION_ITEMS
                && items.iter().all(|item| json_within_limits(item, depth + 1))
        }
        Value::Object(items) => {
            items.len() <= MAX_MCP_COLLECTION_ITEMS
                && items.iter().all(|(key, item)| {
                    key.len() <= MAX_MCP_STRING_BYTES && json_within_limits(item, depth + 1)
                })
        }
        _ => true,
    }
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut diff = left.len() ^ right.len();
    let max = left.len().max(right.len());
    for index in 0..max {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        diff |= usize::from(a ^ b);
    }
    diff == 0
}

fn handle_mcp_request_with_auth(root: Option<&PathBuf>, body: &str, authenticated: bool) -> Value {
    let parsed: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return json!({
                "jsonrpc":"2.0",
                "id": Value::Null,
                "error":{"code":-32700,"message":"Parse error"}
            });
        }
    };
    let id = parsed.get("id").cloned().unwrap_or(Value::Null);
    let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
    match method {
        "initialize" => json!({
            "jsonrpc":"2.0",
            "id": id,
            "result": {
                "protocolVersion":"2024-11-05",
                "serverInfo": {"name":"5harness","version": VERSION},
                "capabilities": {"tools": {}}
            }
        }),
        "tools/list" => json!({
            "jsonrpc":"2.0",
            "id": id,
            "result": {"tools": mcp_tools_for_root(root)}
        }),
        "tools/call" => {
            if !authenticated {
                return json!({
                    "jsonrpc":"2.0",
                    "id":id,
                    "error":{"code":-32001,"message":"MCP bearer token required."}
                });
            }
            let Some(root) = root else {
                return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32001,"message":"MCP project is unbound."}});
            };
            let params = parsed.get("params").cloned().unwrap_or(json!({}));
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(root, name, &args) {
                Ok(text) => json!({
                    "jsonrpc":"2.0",
                    "id": id,
                    "result": {"content":[{"type":"text","text": text}]}
                }),
                Err(err) => json!({
                    "jsonrpc":"2.0",
                    "id": id,
                    "error":{"code":-32000,"message":crate::error::redact_sensitive(&err.to_string())}
                }),
            }
        }
        "ping" => json!({"jsonrpc":"2.0","id":id,"result":{}}),
        _ => {
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("Method not found: {method}")}})
        }
    }
}

fn call_tool(root: &Path, name: &str, args: &Value) -> Result<String> {
    match name {
        "harness_get" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_get requires id"))?;
            match get_entity(root, id) {
                Ok(Some(file)) => Ok(format!(
                    "# {} ({})\npath: {}\n",
                    id, "entity", file.relative_path
                )),
                Ok(None) => Err(Error::new(format!("Entity not found: {id}"))),
                Err(e) => Err(e),
            }
        }
        "harness_search" => {
            let q = args
                .get("query")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_search requires query"))?;
            match ensure_index(root) {
                Ok(idx) => Ok(format_search_hits(&search_index(&idx, q, 20, None))),
                Err(e) => Err(e),
            }
        }
        "harness_query_matrix" => query_matrix(root, false),
        "harness_query_stats" => query_stats(root),
        "harness_status" => Ok(serde_json::to_string(&status_json(root)?)?),
        "harness_next" => {
            let items = next_items(
                root,
                args.get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
            )?;
            Ok(serde_json::to_string(&items)?)
        }
        "harness_context" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_context requires id"))?;
            let file = get_entity(root, id)?
                .ok_or_else(|| Error::new(format!("Entity not found: {id}")))?;
            let index = ensure_index(root)?;
            let max_chars = args
                .get("max_chars")
                .and_then(|v| v.as_u64())
                .unwrap_or(12_000) as usize;
            let body: String = file.body.chars().take(max_chars.min(100_000)).collect();
            Ok(serde_json::to_string(
                &json!({"id":id,"path":file.relative_path,"frontmatter":super::durable::fm_json(&file.data),"body":body,"links":super::index::links_for(&index,id)}),
            )?)
        }
        "harness_links" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_links requires id"))?;
            let index = ensure_index(root)?;
            Ok(serde_json::to_string(&super::index::links_for(&index, id))?)
        }
        "harness_doctor" => Ok(serde_json::to_string(&doctor_json(root)?)?),
        "harness_project_role" => Ok(serde_json::to_string(&project_link::role(root)?)?),
        "harness_project_peers" => Ok(serde_json::to_string(&project_link::peers(root)?)?),
        "harness_intake" => {
            let ty = args
                .get("type")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_intake requires type"))?;
            let summary = args
                .get("summary")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_intake requires summary"))?;
            let lane = args
                .get("lane")
                .and_then(|v| v.as_str())
                .unwrap_or("normal");
            match add_intake(root, ty, summary, lane, None, None, None, None, None, None) {
                Ok((_, id)) => Ok(format!("Intake {id} recorded.")),
                Err(e) => Err(e),
            }
        }
        "harness_story_add" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_story_add requires id"))?;
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_story_add requires title"))?;
            let lane = args
                .get("lane")
                .and_then(|v| v.as_str())
                .unwrap_or("normal");
            match add_story(root, id, title, lane, None, None, None, None) {
                Ok(_) => Ok(format!("Story {id} added.")),
                Err(e) => Err(e),
            }
        }
        "harness_story_update" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_story_update requires id"))?;
            update_story(
                root,
                StoryUpdate {
                    id: id.into(),
                    status: args
                        .get("status")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    evidence: args
                        .get("evidence")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    unit: args
                        .get("unit")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    integration: args
                        .get("integration")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    e2e: args.get("e2e").and_then(|v| v.as_str()).map(str::to_string),
                    platform: args
                        .get("platform")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    verify: None,
                    title: None,
                    notes: None,
                    contract: None,
                    links: None,
                },
            )?;
            Ok(format!("Story {id} updated."))
        }
        "harness_decision_add" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_decision_add requires id"))?;
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| Error::new("harness_decision_add requires title"))?;
            match add_decision(root, id, title, None, None, None, None, None, false) {
                Ok(_) => Ok(format!("Decision {id} added.")),
                Err(e) => Err(e),
            }
        }
        "harness_backlog_add" => {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_backlog_add requires title"))?;
            let (_, id) = add_backlog(
                root,
                title,
                None,
                None,
                None,
                args.get("risk").and_then(|v| v.as_str()),
                None,
                None,
                None,
            )?;
            Ok(format!("Backlog {id} added."))
        }
        "harness_reindex" => {
            let _lock = MutationLock::acquire(root)?;
            let (_, entities, edges) = super::index::write_project_index(root)?;
            Ok(format!("Reindexed {entities} entities, {edges} edges."))
        }
        "harness_peer_search" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_peer_search requires query"))?;
            let peer = args.get("peer_id").and_then(|v| v.as_str());
            let role = args.get("role").and_then(|v| v.as_str());
            let peer_root = project_link::resolve_peer(root, peer, role)?;
            let index = ensure_index(&peer_root)?;
            Ok(serde_json::to_string(&search_index(
                &index, query, 20, None,
            ))?)
        }
        "harness_peer_get" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_peer_get requires id"))?;
            let peer_root = project_link::resolve_peer(
                root,
                args.get("peer_id").and_then(|v| v.as_str()),
                args.get("role").and_then(|v| v.as_str()),
            )?;
            let file = get_entity(&peer_root, id)?
                .ok_or_else(|| Error::new(format!("Peer entity not found: {id}")))?;
            Ok(serde_json::to_string(
                &json!({"id":id,"path":file.relative_path,"frontmatter":super::durable::fm_json(&file.data),"body":file.body}),
            )?)
        }
        "harness_peer_context" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_peer_context requires id"))?;
            let peer_root = project_link::resolve_peer(
                root,
                args.get("peer_id").and_then(|v| v.as_str()),
                args.get("role").and_then(|v| v.as_str()),
            )?;
            let file = get_entity(&peer_root, id)?
                .ok_or_else(|| Error::new(format!("Peer entity not found: {id}")))?;
            let max_chars = args
                .get("max_chars")
                .and_then(|v| v.as_u64())
                .unwrap_or(12_000) as usize;
            let body: String = file.body.chars().take(max_chars.min(100_000)).collect();
            Ok(serde_json::to_string(
                &json!({"id":id,"path":file.relative_path,"body":body}),
            )?)
        }
        "harness_peer_links" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_peer_links requires id"))?;
            let peer_root = project_link::resolve_peer(
                root,
                args.get("peer_id").and_then(|v| v.as_str()),
                args.get("role").and_then(|v| v.as_str()),
            )?;
            let index = ensure_index(&peer_root)?;
            Ok(serde_json::to_string(&super::index::links_for(&index, id))?)
        }
        "harness_report_add" => {
            let to = args
                .get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_report_add requires to"))?;
            let summary = args
                .get("summary")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_report_add requires summary"))?;
            let target = project_link::resolve_peer(root, Some(to), None)?;
            project_link::ensure_peer_write_allowed(&target)?;
            let from = std::fs::read_to_string(root.join("AGENTS.md"))
                .ok()
                .and_then(|t| crate::domain::project_id::extract_project_id(&t));
            let (_, id) = add_report(&target, summary, None, from.as_deref(), None)?;
            Ok(format!("Report {id} added."))
        }
        "harness_report_list" => {
            let status = args.get("status").and_then(|v| v.as_str());
            let mut rows = Vec::new();
            for file in crate::infra::entities::list_entity_files(root, "report")? {
                let current =
                    crate::domain::frontmatter::as_string(&file.data, "status").unwrap_or_default();
                if status.is_some_and(|wanted| wanted != current) {
                    continue;
                }
                rows.push(json!({"id":crate::domain::frontmatter::as_string(&file.data,"id"),"status":current,"summary":crate::domain::frontmatter::as_string(&file.data,"summary"),"updated_at":crate::domain::frontmatter::as_string(&file.data,"updated_at")}));
            }
            Ok(serde_json::to_string(&rows)?)
        }
        "harness_report_get" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_report_get requires id"))?;
            let file = get_entity(root, id)?
                .ok_or_else(|| Error::new(format!("Report {id} not found")))?;
            Ok(serde_json::to_string(
                &json!({"id":id,"path":file.relative_path,"frontmatter":super::durable::fm_json(&file.data),"body":file.body}),
            )?)
        }
        "harness_report_update" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_report_update requires id"))?;
            let status = args
                .get("status")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::new("harness_report_update requires status"))?;
            update_report(
                root,
                id,
                status,
                args.get("resolution").and_then(|v| v.as_str()),
                args.get("related").and_then(|v| v.as_str()),
            )?;
            Ok(format!("Report {id} updated."))
        }
        _ => Err(Error::new(format!("Unknown tool {name}"))),
    }
}

pub fn oauth_protected_resource(issuer: &str) -> Value {
    json!({
        "resource": format!("{}/mcp", issuer.trim_end_matches('/')),
        "authorization_servers": [],
        "authorization_endpoint": null,
        "bearer_methods_supported": ["header"],
        "resource_name": "5harness MCP",
        "resource_documentation": "https://github.com/vantanminh/5harness"
    })
}

pub fn start_mcp(
    host: &str,
    port: u16,
    project_root: PathBuf,
    serve_forever: bool,
    public_url: Option<&str>,
    token: Option<String>,
) -> Result<RunningServer> {
    if !is_loopback_bind_host(host) {
        let url = public_url.ok_or_else(|| {
            Error::new("refusing non-loopback MCP bind without --public-url https://...")
        })?;
        if !is_valid_public_https_url(url) {
            return Err(Error::new(
                "--public-url must be a valid https URL without credentials, query, or fragment for non-loopback MCP",
            ));
        }
    }
    let token = match token.filter(|v| !v.trim().is_empty()).or_else(|| {
        std::env::var("HARNESS_MCP_TOKEN")
            .ok()
            .filter(|v| !v.trim().is_empty())
    }) {
        Some(token) => token,
        None => {
            let mut bytes = [0u8; 32];
            getrandom::getrandom(&mut bytes)
                .map_err(|e| Error::new(format!("generate MCP token: {e}")))?;
            hex::encode(bytes)
        }
    };
    let ttl_secs = std::env::var("HARNESS_MCP_TOKEN_TTL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MCP_TOKEN_TTL_SECS);
    let rate_limit_per_minute = std::env::var("HARNESS_MCP_RATE_LIMIT_PER_MINUTE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MCP_RATE_LIMIT_PER_MINUTE);
    let public_bind = !is_loopback_bind_host(host);
    let token_expires_at = std::time::Instant::now() + Duration::from_secs(ttl_secs);
    let project_id = std::fs::read_to_string(project_root.join("AGENTS.md"))
        .ok()
        .and_then(|text| extract_project_id(&text));
    let listener = TcpListener::bind((host, port))
        .map_err(|e| Error::new(format!("mcp bind {host}:{port} failed: {e}")))?;
    let actual = listener.local_addr()?.port();
    let server = Server::from_listener(listener, None)
        .map_err(|e| Error::new(format!("mcp server: {e}")))?;
    let local_url = format!("http://{host}:{actual}/");
    let url = public_url
        .map(|value| format!("{}/", value.trim_end_matches('/')))
        .unwrap_or(local_url);
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let auth_token = token.clone();
    let issuer = public_url
        .map(|url| url.trim_end_matches('/').to_string())
        .unwrap_or_else(|| format!("http://{host}:{actual}"));
    let handle = thread::spawn(move || {
        mcp_loop(
            server,
            flag,
            project_root,
            project_id,
            issuer,
            token,
            token_expires_at,
            public_bind,
            rate_limit_per_minute,
        )
    });
    if serve_forever {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
    Ok(RunningServer {
        url,
        port: actual,
        auth_token: Some(auth_token),
        shutdown,
        handle: Some(handle),
    })
}

#[allow(clippy::too_many_arguments)]
fn mcp_loop(
    server: Server,
    shutdown: Arc<AtomicBool>,
    project_root: PathBuf,
    project_id: Option<String>,
    issuer: String,
    token: String,
    token_expires_at: std::time::Instant,
    public_bind: bool,
    rate_limit_per_minute: u32,
) {
    let mut rate_limiter = RateLimiter::new(rate_limit_per_minute);
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(mut request)) => {
                let url = request.url().to_string();
                let method = request.method().clone();
                let rate_limited = public_bind && !rate_limiter.allow(request.remote_addr());
                let oversized_headers = request
                    .headers()
                    .iter()
                    .any(|header| header.value.as_bytes().len() > MAX_MCP_HEADER_VALUE_BYTES)
                    || request.headers().len() > MAX_MCP_HEADERS
                    || request
                        .headers()
                        .iter()
                        .map(|header| {
                            header.field.as_str().as_bytes().len() + header.value.as_bytes().len()
                        })
                        .sum::<usize>()
                        > MAX_MCP_HEADER_BYTES;
                let oversized = request
                    .headers()
                    .iter()
                    .find(|header| header.field.equiv("Content-Length"))
                    .and_then(|header| header.value.as_str().parse::<usize>().ok())
                    .is_some_and(|length| length > MAX_MCP_BODY_BYTES);
                let mut body = String::new();
                if !rate_limited && !oversized {
                    let mut limited = request.as_reader().take((MAX_MCP_BODY_BYTES + 1) as u64);
                    let _ = limited.read_to_string(&mut body);
                }
                let oversized = oversized || body.len() > MAX_MCP_BODY_BYTES;
                let json_limits_exceeded = if oversized {
                    false
                } else {
                    serde_json::from_str::<Value>(&body)
                        .ok()
                        .is_some_and(|value| !json_within_limits(&value, 0))
                };
                let path = url.split('?').next().unwrap_or("/");
                let authorization = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Authorization"))
                    .map(|h| h.value.as_str());
                let authenticated = authorization
                    .and_then(|value| value.strip_prefix("Bearer "))
                    .is_some_and(|value| {
                        std::time::Instant::now() < token_expires_at
                            && constant_time_equal(value, &token)
                    });
                let needs_auth = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| {
                        v.get("method")
                            .and_then(|m| m.as_str())
                            .map(|m| m == "tools/call")
                    })
                    .unwrap_or(true);
                let header_project = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("X-Harness-Project"))
                    .map(|h| h.value.as_str());
                let query_project = url.split_once('?').and_then(|(_, query)| {
                    query
                        .split('&')
                        .find_map(|item| item.strip_prefix("project="))
                });
                let selector_conflict = header_project.is_some()
                    && query_project.is_some()
                    && header_project != query_project;
                let supplied_project = header_project.or(query_project);
                let project_bound = project_id
                    .as_deref()
                    .is_some_and(|id| supplied_project == Some(id));
                let (status, ctype, payload, www_authenticate) = if rate_limited {
                    (
                        429,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&json!({"error":"rate limit exceeded"}))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    )
                } else {
                    match (method, path) {
                    (Method::Get, "/.well-known/oauth-protected-resource") => (
                        200,
                        "application/json; charset=utf-8",
                        serde_json::to_string_pretty(&oauth_protected_resource(&issuer))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Get, "/mcp") => (
                        200,
                        "application/json; charset=utf-8",
                        serde_json::to_string_pretty(&json!({
                            "name": "5harness",
                            "version": VERSION,
                            "protocolVersion": "2024-11-05",
                            "transport": "streamable-http",
                            "tools": mcp_tools_for_root(Some(&project_root))
                        }))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Post, "/mcp") | (Method::Post, "/") if oversized_headers => (
                        431,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&json!({"error":"request header exceeds 16 KiB limit"}))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Post, "/mcp") | (Method::Post, "/")
                        if oversized || json_limits_exceeded =>
                    (
                        413,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&json!({"error":"request body exceeds 1 MiB limit"}))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Post, "/mcp") | (Method::Post, "/") if selector_conflict => (
                        400,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&json!({"error":"conflicting project selectors"}))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Post, "/mcp") | (Method::Post, "/") if needs_auth && !authenticated => (
                        401,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&json!({
                            "jsonrpc":"2.0",
                            "id": Value::Null,
                            "error":{"code":-32001,"message":"Bearer token required"}
                        })).unwrap_or_else(|_| "{}".into()),
                        true,
                    ),
                    (Method::Post, "/mcp") | (Method::Post, "/") if needs_auth && !project_bound => (
                        403,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&json!({
                            "jsonrpc":"2.0",
                            "id": Value::Null,
                            "error":{"code":-32002,"message":"X-Harness-Project must select the authorized project"}
                        })).unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Post, "/mcp") | (Method::Post, "/") => (
                        200,
                        "application/json; charset=utf-8",
                        serde_json::to_string(&handle_mcp_request_with_auth(Some(&project_root), &body, true))
                            .unwrap_or_else(|_| "{}".into()),
                        false,
                    ),
                    (Method::Options, _) => (204, "text/plain", String::new(), false),
                        _ => (404, "text/plain; charset=utf-8", "not found".into(), false),
                    }
                };
                let mut response = Response::new(
                    StatusCode(status),
                    vec![
                        Header::from_bytes(&b"Content-Type"[..], ctype.as_bytes()).unwrap(),
                        Header::from_bytes(
                            &b"Access-Control-Allow-Headers"[..],
                            &b"Authorization, Content-Type, X-Harness-Project"[..],
                        )
                        .unwrap(),
                        Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap(),
                        Header::from_bytes(&b"X-Content-Type-Options"[..], &b"nosniff"[..])
                            .unwrap(),
                        Header::from_bytes(&b"Referrer-Policy"[..], &b"no-referrer"[..]).unwrap(),
                        Header::from_bytes(&b"X-Frame-Options"[..], &b"DENY"[..]).unwrap(),
                    ],
                    Cursor::new(payload.into_bytes()),
                    None,
                    None,
                );
                if www_authenticate {
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

pub fn query_view_json_pub(root: &Path, view: &str) -> Result<Value> {
    query_view_json(root, view)
}

#[cfg(test)]
mod tests {
    use super::RateLimiter;

    #[test]
    fn rate_limiter_bounds_requests_per_source() {
        let mut limiter = RateLimiter::new(2);
        assert!(limiter.allow(None));
        assert!(limiter.allow(None));
        assert!(!limiter.allow(None));
    }
}
