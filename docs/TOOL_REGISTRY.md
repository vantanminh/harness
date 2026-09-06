# Tool Registry

The harness deals with two distinct kinds of "tool". Keep them separate.

| | Capability manifest (outbound) | Inbound tool registry |
| --- | --- | --- |
| Direction | harness offers it to the agent | a project equips it for the harness to use |
| Examples | the `harness-cli` subcommands below | gitnexus, c3, a linter, a deploy check |
| Presence | always compiled in | optional; may be absent on any machine |
| If missing | n/a (it is the harness) | clean skip; never blocks the main process |

This document describes both. The **inbound registry** is the extension base:
it is where the harness learns what extra capability is equipped, what purpose
it serves, and whether it is actually present right now, so a workflow step can
adapt to what is installed without the core ever depending on it.

## Inbound Registry: Register A Tool

```bash
harness tool register \
  --name deploy-check \
  --kind cli \
  --capability deploy-verification \
  --command ./scripts/deploy-check.sh \
  --description "Verify deploy health before release" \
  --responsibility Verification \
  --args "env:enum:required:staging,production"
```

Fields specific to inbound tools:

- `--kind` — how the tool is reached and probed. One of `cli`, `binary`, `mcp`,
  `skill`, `http`. Defaults to `cli`. The kind tells each agent runtime what it
  can orchestrate (a non-Claude agent simply treats a `skill` it cannot run as
  absent) and tells `tool check` which probe to use.
- `--capability` — the workflow purpose a step looks the tool up by. Free-text
  but normalized to kebab-case, so `Impact Analysis`, `impact_analysis`, and
  `impact-analysis` all register as `impact-analysis`. This is the only coupling
  between a step and a tool; steps reference the capability, never the tool name.
- `--scan` — for `mcp`/`skill`/`http`, a declarative path or URL that
  `tool check` resolves to decide presence (e.g. `.c3`, `~/.claude/skills/c3`,
  `https://localhost:8080/health`). `cli`/`binary` are probed via their command.

`--force` is only needed for `cli`/`binary` whose command is intentionally
absent on the current machine. `mcp`/`skill`/`http` are not on `PATH` by nature,
so they register without `--force`; their presence is resolved later by
`tool check`.

Registering an MCP server or a Claude skill (examples):

```bash
harness tool register --name gitnexus --kind mcp \
  --capability impact-analysis --scan ".gitnexus" --command "mcp:gitnexus" \
  --description "Code-graph blast radius" --responsibility Verification
harness tool register --name c3 --kind skill \
  --capability impact-analysis --scan ".c3" --command "skill:c3" \
  --description "Component model and drift audit (Claude skill)" \
  --responsibility Verification
```

Remove a tool with:

```bash
harness tool remove --name deploy-check
```

## Inbound Registry: Check Presence

Registration records intent. `tool check` reconciles intent with reality by
scanning each registered tool and persisting the verdict (`status` and
`checked_at`). Run it at intake start so status reflects current reality.

```bash
harness tool check --allow-project-command            # scan all registered tools
harness tool check --name c3 --allow-project-command  # scan one
harness tool check --allow-project-command --json     # machine-readable for agents
```

The native runtime treats each persisted `command` field as a project-authored
shell check, regardless of the record's `kind`. It refuses to execute any such
command unless `--allow-project-command` is present; review the command in the
project record first. Commands are bounded to one non-empty 8 KiB line and run
in the project directory. The older `--scan`/kind-specific probe metadata is
not executed by this runtime and never bypasses the approval gate; use a
trusted wrapper command when a probe is required.

The intended probe metadata (for runtimes that implement it) is:

| Kind | Probe | `present` means |
| --- | --- | --- |
| `cli`, `binary` | command resolves on `PATH` or as a path | installed and runnable |
| `mcp`, `skill` | `scan_target` path resolves (`~` expands) | equipped/configured on disk |
| `http` | `scan_target` reachable over TCP (2s), else path | endpoint answers |

Without the explicit approval flag, a command-backed check exits non-zero and
does not execute. With approval, command success is recorded as `ok` and
failure as `failed`; a missing extension is a fact to report, not a CLI failure.
Only an explicitly trusted command can establish presence in the native
runtime. Metadata without a command is not an authorization or execution
path.

## Inbound Registry: Look Up By Capability

A workflow step asks "what is present for this purpose?" rather than naming a
tool:

```bash
harness query tools --capability impact-analysis
harness query tools --capability impact-analysis --status present
```

The result is the set of providers. Multiple tools may provide one capability
(gitnexus and c3 both serve `impact-analysis` and are complementary), so a step
reads the set and degrades on how much of it is present.

### Degrade Ladder

The CLI reports facts (`status`); the agent applies policy. The generic rule,
keyed on the present-provider count for a capability:

| Providers present | Posture | Agent behavior |
| --- | --- | --- |
| none registered | Inactive | clean skip; note `capability X: inactive` in the trace. Not drift. |
| registered but none/some present | Degraded | run with what resolves; set the `Weak proof` flag; note the gap. |
| all present | Full | normal operation. |

A registered tool that scans as `missing` is a failed validity gate, not a skip.
A capability with no registered providers is simply inactive and is skipped
without penalty — this is what keeps the core seamless on a fresh install.

### Recommended Capability Vocabulary

Capability is open (no code change to add one), but a step and its providers
must agree on the exact string. Reuse these where they fit before coining a new
one; coin new ones in kebab-case:

```
impact-analysis · deploy-verification · coverage · security-scan
performance-benchmark · documentation-lookup
```

## Inspecting The Registry

```bash
harness query tools --summary
harness query tools --json
harness query tools --responsibility Verification
```

JSON records carry `kind`, `capability`, `scan_target`, `status`, and
`checked_at` alongside the existing fields, so any agent can read the registry
without parsing the human table.

## Compiled Harness Commands (Outbound Manifest)

| Command | Responsibility | Purpose | Arguments |
| --- | --- | --- | --- |
| `init` | Task state | Scaffold markdown state and register the project. | optional directory / force flags |
| `migrate` | Task state | Migrate a legacy `harness.db` when present. | optional directory |
| `import-sqlite` | Project memory | Import legacy SQLite rows into markdown entities. | optional `--db`, `--force` |
| `link` / `unlink` / `projects` | Task state | Manage machine-local registry pointers. | optional project path |
| `project id` | Project memory | Read or ensure durable project identity. | optional `--ensure`, `--json` |
| `project role` | Project memory | Configure or inspect Project Link role and stack tags. | `set` or `show` |
| `project peer` | Tool access | Add, remove, or list explicitly configured peers. | peer id/path and optional role |
| `peer search\|get\|context\|links` | Tool access | Run bounded reads against one configured peer. | `--peer` or unique `--role` |
| `report add\|list\|get\|update` | Project memory | Manage target-owned cross-project reports. | command-specific report fields |
| `intake` | Task specification | Record a feature intake classification. | `--type`, `--summary`, `--lane` |
| `story add` | Task state | Create a durable story record. | `--id`, `--title`, `--lane`, optional `--verify` |
| `story update` | Task state | Update story status, proof flags, evidence, or verification command. | `--id`, optional proof/status fields |
| `story verify` | Verification | Run one project-authored story `verify` command after explicit `--allow-project-command` approval and record pass/fail. | story id + approval flag |
| `story verify-all` | Verification | Preflight and run all configured story verification commands after explicit `--allow-project-command` approval. | approval flag |
| `decision add` | Project memory | Create a durable decision record. | `--id`, `--title`, optional `--doc`, `--verify` |
| `decision verify` | Verification | Run one project-authored decision verification command after explicit `--allow-project-command` approval. | decision id + approval flag |
| `backlog add` | Entropy auditing | Record a harness improvement proposal. | `--title`, optional pain/suggestion/risk/predicted fields |
| `backlog close` | Entropy auditing | Close a backlog item with outcome evidence. | `--id`, optional `--status`, `--outcome` |
| `tool register` | Tool access | Register an external project tool. | `--name`, `--command`, `--description`, `--responsibility`, optional `--kind`, `--capability`, `--scan`, `--args`, `--force` |
| `tool check` | Tool access | Scan registered tools and persist present/missing/unknown status; command-backed checks require explicit project-command approval. | optional `--name`, `--allow-project-command`, `--json` |
| `tool remove` | Tool access | Remove a registered external tool. | `--name` |
| `trace` | Observability | Record an agent execution trace and print trace quality. | `--summary`, optional trace fields |
| `score-trace` | Observability | Score trace detail against lane requirements. | optional `--id` |
| `audit` | Entropy auditing | Run drift checks and compute entropy score. | none |
| `propose` | Entropy auditing | Generate improvement proposals from audit findings and recorded trace friction. | optional `--commit` |
| `query matrix` | Task state | Show durable story proof matrix. | optional `--numeric` |
| `query backlog` | Entropy auditing | Show harness improvement backlog. | optional `--open`, `--closed` |
| `query decisions` | Project memory | Show durable decision records. | none |
| `query intakes` | Task specification | Show recent intake records. | none |
| `query traces` | Observability | Show recent trace records. | none |
| `query tools` | Tool access | Show compiled and registered tool entries. | optional `--json`, `--summary`, `--responsibility`, `--capability`, `--status` |
| `query stats` | Task state | Show durable record counts. | none |

Project Link MCP tools are advertised only when the calling project has
configured peers. Their peer selector is a capability boundary, not an OAuth
project selector: reads stay bounded to one configured peer and never traverse
peer-of-peer links; cross-project operational-entity writes are report-only
after the explicit peer-management configuration step.

## Validation Rules

- Tool names must be unique among registered tools.
- Descriptions must be 10-200 characters.
- Responsibilities must match the Runtime Substrate responsibility list.
- `--kind` must be one of `cli`, `binary`, `mcp`, `skill`, `http`.
- `--capability` must be kebab-case (lowercase letters, digits, single hyphens);
  spaces and underscores are normalized to hyphens.
- `--args` entries must use `name:type:required` or
  `name:type:required:help`, with `required` or `optional` as the third field.
- For `cli`/`binary`, the command must exist as a path or on `PATH`, unless
  `--force` is supplied. `mcp`/`skill`/`http` skip this check.
