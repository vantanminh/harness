# CLI Contract

User-facing bin name: `harness`.

Install (**preferred** — decision 0011):

```bash
npm i -g 5harness
harness <command>
```

Alternate: `npm i -D 5harness` + `npx harness <command>`.

Package name: **`5harness`**. Bin name: **`harness`**.

## Commands in scope for Phase A (US-001) — shipped MVP

| Command | Behavior |
| --- | --- |
| `harness --version` / `-V` | Print CLI version from package |
| `harness --help` / `-h` | Top-level help |
| `harness update` | Install the npm `latest` dist-tag globally; normal commands recheck that tag hourly and retry failed checks after five minutes |
| `harness init [options]` | Scaffold operating files + (MVP: SQLite; **target 0011:** markdown + registry) |
| `harness migrate` | MVP only: apply SQL migrations (retired when markdown SoT ships) |

## `harness init` options (Phase A)

| Option | Meaning |
| --- | --- |
| `[directory]` or `--dir <path>` | Target project root (default: cwd) |
| `--yes` / `-y` | Non-interactive; do not prompt |
| `--dry-run` | Print planned writes; write nothing |
| `--force` | Overwrite conflicting non-protected files after backup (define precisely in story) |

Merge/override policy for existing `AGENTS.md` / `docs/` may be a follow-up story if Phase A only supports empty or non-conflicting targets. **Phase A minimum:** refuse to clobber protected paths unless `--force`, with a clear error.

## Commands in scope for Phase B (US-002)

| Command | Behavior |
| --- | --- |
| `harness intake` | Record feature intake (`--type`, `--summary`, `--lane`, …) |
| `harness story add` | Add a story matrix row |
| `harness story update` | Update status, proof flags (`0\|1`), evidence, verify command |
| `harness decision add` | Add a decision row |
| `harness decision update` | Update status, notes, verify command, or links (US-076) |
| `harness backlog add` | Add a backlog item |
| `harness backlog close` | Close backlog (`implemented` / `rejected`) |
| `harness query matrix` | Story matrix (`--numeric` for 0/1) |
| `harness query stats` | Summary counts |
| `harness query intakes` | Recent intakes |
| `harness query decisions` | Decisions |
| `harness query stories` | Story list |
| `harness query backlog` | Backlog (`--open` / `--closed`) |
| `harness query reports` | Target-owned reports (US-076) |
| `harness query * --json` | Structured JSON for agent reads (US-073) |

Most durable commands accept `-d, --dir <path>` for the target project (default: cwd).
They auto-migrate an existing DB; if the DB is missing, run `harness init` first.

## Commands in scope for Phase C (US-003)

| Command | Behavior |
| --- | --- |
| `harness story verify <id> --allow-project-command` | Run one project-authored story `verify_command` after explicit operator approval; record pass/fail |
| `harness story verify-all --allow-project-command` | Preflight and run all configured story commands after explicit approval |
| `harness decision verify <id> --allow-project-command` | Run one project-authored decision verify command after explicit approval |
| `harness trace` | Record execution trace (`--summary`, `--outcome`, …); scores by default |
| `harness score-trace [--id]` | Score latest or specific trace tiers |
| `harness query traces` | List recent traces |
| `harness audit` | Drift findings + entropy score 0–100 |

## Commands in scope for Phase E (US-005)

| Command | Behavior |
| --- | --- |
| `harness propose` | Print improvement proposals from audit findings |
| `harness propose --commit` | Also insert new proposals into backlog (dedupe open titles) |
| `harness query tools` | Built-in tool registry (`--capability`, `--status`) |

## Commands in scope for Phase F (0011 store pivot)

| Command | Behavior |
| --- | --- |
| `harness init` | Scaffold markdown + **register** project in global registry |
| `harness link [path]` | Register existing harness project (clone workflow) + reindex |
| `harness unlink [path]` | Drop the registry entry for an accessible path only |
| `harness unlink --id <project-id>` | Drop one missing-path registry entry by exact durable id |
| `harness unlink --missing` | Drop every registry entry whose path is missing |
| `harness projects` | List linked projects |
| `harness reindex` | Rebuild derived index from markdown |
| `harness get <id\|path>` | Load one entity (optional summary) |
| `harness search <query>` | Ranked hits with snippets |
| `harness links <id>` | Outbound + backlinks |
| `harness intake update --id <id> [--status <status>] [--stories <csv>]` | Update intake lifecycle metadata |
| `harness intake close <id>` | Mark an intake completed. `--id <id>` is an alias. |
| `harness intake dismiss <id>` | Dismiss an intake without implementation. `--id <id>` is an alias. |

Write commands (`intake`, `story`, `decision`, `backlog`) keep the same *intent*
but persist to markdown entities. Agents **must** use these tools; they must not
hand-edit operational markdown.

## Commands in scope for Phase H (E12–E14, post-G agent loop)

### E12 — Agent Loop Tier 1

| Command | Behavior |
| --- | --- |
| `harness doctor [--json]` | Workspace health checks (store, registry, index, Node, logs); Project Link adds non-fatal unresolved-peer and unreadable peer index warnings |
| `harness status [--json]` | Project snapshot: stories, intakes, backlog, Project Link role/stack/peer count/open local reports, version, index age |
| `harness next [--limit <n>] [--json]` | Recommend next work item; open reports after in-progress, then blocked, then planned (any role) |
| `harness context <id> [--depth 0\|1] [--max-chars N] [--json]` | Budgeted entity context pack |
| `harness tool register [--name] [--command] ...` | Register external project tool |
| `harness tool check [--name] [--allow-project-command] [--json]` | Scan registered tools; command-backed probes require explicit project-command approval |
| `harness tool remove [--name] [--json]` | Remove a registered tool |

### E13 — Agent Loop Tier 2

| Command | Behavior |
| --- | --- |
| `harness story start <id>` | Mark story as in_progress. `--id <id>` is an alias. |
| `harness story done <id>` | Mark story as implemented. `--id <id>` is an alias. |
| `harness story block <id> [--reason]` | Mark story as blocked. `--id <id>` is an alias. |
| `harness worklog add --story <id> --summary <text> [--pr] [--commit]` | Add worklog entry |
| `harness worklog list [--json]` | List worklog entries |
| `harness worklog from-git --story <id> [--since]` | Link recent git commits to a story |
| `harness intake-run [--prompt]` | Analyze prompt and suggest intake classification |
| `harness dashboard` | Start local dashboard (supports mutations via same CLI code paths) |

### E14 — Agent Loop Tier 3

| Command | Behavior |
| --- | --- |
| `harness project id [--ensure] [--json]` | Print the cwd/`--dir` project's durable random id. `--ensure` creates the managed `AGENTS.md` marker if missing; `--json` returns id, path, and name. Init/link/upgrade ensure identity automatically. |
| `harness mcp` | Start an authenticated streamable HTTP MCP server. Discovery is public; every `tools/call` requires `Authorization: Bearer <startup-token>` and `X-Harness-Project: <id>` (or `?project=<id>`). The token is supplied with `--token`/`HARNESS_MCP_TOKEN` or generated per process (24-hour default TTL). Requests and serialized responses are bounded to 1 MiB bodies/responses, 16 KiB individual headers (64 headers / 64 KiB total), 64 KiB strings, 32 nesting levels, and 1,000 collection entries; non-loopback binds are rate-limited to 120 requests/minute per source by default. Project Link peer/report tools are added dynamically when peers exist. Non-loopback requires `--public-url https://...`. |
| `harness export changelog [--since <tag\|date>] [--json]` | Derive changelog notes from implemented stories/decisions (assist only) |
| `harness watch` | Watch entity directories and auto-reindex on markdown changes (debounced 500ms) |
| `harness handoff [--story <id>] [--json]` | Emit concise session summary: recent traces, worklog, status, next steps |

## Commands in scope for Phase I (E16, Project Link) — shipped (v0.21+)

Project Link is opt-in. Project ids and peer intent are durable markers in the
Harness-managed `AGENTS.md` block; peer paths resolve only through the
same-machine registry. Registry `harness link` keeps its existing meaning.

| Command | Behavior |
| --- | --- |
| `harness project role set <role> [--stack <tags>] [--json]` | Set `frontend\|backend\|mobile\|service\|shared\|other` and up to four unique lowercase stack tags (`[a-z0-9_-]+`, max 32 characters each) |
| `harness project role show [--json]` | Show the local role and stack tags |
| `harness project peer add <id\|path> [--role <role>]` | Add a registered harness project as a peer and attempt the reverse marker; save the forward marker with a warning if reverse write fails |
| `harness project peer remove <id>` | Remove the local edge and attempt the reverse unlink |
| `harness project peer list [--json]` | List configured ids, roles, names, and machine-local resolved paths/status |
| `harness peer search <query> (--peer <id>\|--role <role>) [--limit <n>]` | Search one configured peer's derived index with bounded ranked snippets |
| `harness peer get <id\|path> (--peer <id>\|--role <role>) [--summary]` | Get one peer entity; `--summary` returns frontmatter only |
| `harness peer context <id> (--peer <id>\|--role <role>) [--depth 0\|1] [--max-chars <n>] [--json]` | Build a bounded peer context pack |
| `harness peer links <id> (--peer <id>\|--role <role>)` | Show one peer entity's outbound links and backlinks |
| `harness report add --to <role\|id> --summary <text> [options]` | Create an `open`, target-owned report in one configured peer and reindex that target |
| `harness report list [--status <status>] [--json]` | List bounded local report rows (`open\|acked\|fixed\|wontfix\|needs_info`) |
| `harness report get <id> [--from <role\|id>]` | Get one local report, or read it from a configured peer |
| `harness report update --id <id> --status <status> [--resolution <text>] [--related <csv>]` | Update a report owned by the local project and reindex locally; `fixed` requires a resolution |

Peer selectors are capability selectors, not arbitrary paths or MCP project routing.
`--role` must identify exactly one configured peer; `--peer`, `--to`, and
`--from` must name a configured peer id/role. There is no peer-of-peer traversal.
Cross-project mutation is limited to `report add`; report lifecycle updates are
local to the target project. Report payloads must be sanitized and must not
contain credentials, tokens, secrets, or unnecessary personal data.

MCP exposes `harness_project_role` and `harness_project_peers` for the calling
project. When that project has peers it additionally exposes
`harness_peer_search`, `harness_peer_get`, `harness_peer_context`,
`harness_peer_links`, `harness_report_add`, `harness_report_list`,
`harness_report_get`, and `harness_report_update`. `X-Harness-Project`/`?project=`
selects the calling project; MCP `peer_id`, `role`, `to`, and `from` never
replace that selection.

## Commands deferred (later)

Changesets; cloud registry (BL-003). Custom tool registration is shipped
(`harness tool register|check|remove`). `score-context` and a dedicated
intervention store are **not** in the product (decision 0023).

## Exit codes

Aligned with [decision 0017](../decisions/0017-agent-hard-fail-contract.md)
(agent hard-fail contract):

| Code | Meaning | Agent expectation |
| --- | --- | --- |
| 0 | Success | Continue |
| 1 | Usage / validation / operational error | **HARD STOP** for the failed step; run recovery (`doctor`, `link`, `reindex`) then retry. Never hand-edit durable entities as a fallback. |
| 2 | Reserved (optional: partial success / soft fail) | Treat as non-success unless the specific command documents otherwise |

`harness doctor` exits non-zero only for hard-fail modes; soft issues are
warnings with exit 0 (US-018).

## Environment

| Variable | Meaning |
| --- | --- |
| `HARNESS_HOME` | Override global harness dir (default `~/.5harness`) |
| `HARNESS_DB_PATH` | **Legacy only** — path to SQLite DB for `import-sqlite` / old DB ops |
| `HARNESS_DEBUG` | When `1`/`true`, emit debug lines to stderr and append to the log file |
| `HARNESS_LOG_FILE` | Override log file path (default: project `.5harness/logs/5harness.log` when a project context exists, else `$HARNESS_HOME/logs/5harness.log`) |
| `HARNESS_JSON_ERRORS` | When `1`/`true`, print failures as a single JSON object on stderr (`ok`, `code`, `message`, `exitCode`) |

## Structured errors (US-033)

CLI failures go through a shared `fail()` path:

| Field | Meaning |
| --- | --- |
| `code` | Stable `HARNESS_E_*` identifier (`USAGE`, `VALIDATION`, `NOT_FOUND`, `STATE`, `IO`, `INTERNAL`) |
| `message` | Human-readable explanation (no secrets) |
| `exitCode` | Process exit code (usually `1`) |

Human stderr form:

```text
error: HARNESS_E_NOT_FOUND: Entity not found: US-999
```

JSON form (`HARNESS_JSON_ERRORS=1`):

```json
{"ok":false,"code":"HARNESS_E_NOT_FOUND","message":"Entity not found: US-999","exitCode":1}
```

Logs never intentionally write secrets (tokens, API keys, passwords are redacted).
`harness doctor` reports the active log file path under the `logs` check.

## Index integrity (US-034)

| Mechanism | Behavior |
| --- | --- |
| Atomic write | `index.json` written via temp file + rename |
| Mutation lock | `.5harness/mutation.lock` held across ID allocation, entity write, and index write; contention fails after a bounded wait and is never silently reclaimed |
| Checksum | SHA-256 over stable index payload; stored as `checksum` on the index |
| Recovery | `harness reindex` rebuilds a valid index; never hand-edit `index.json` |

`harness doctor` includes an `index-integrity` check (corrupt JSON, schema mismatch,
checksum failure, missing entity files → fail; unresolved links / missing checksum → warn).
