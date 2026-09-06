# Agent Instructions

## What This Repo Is

This repository is **5harness** — an **npm-native** agent-ready
repository harness. Users install a global CLI and operate durable history as
**Git-backed markdown** in each project.

**Target user UX (decision 0011):**

```bash
npm i -g 5harness
cd /path/to/project
harness init             # scaffold markdown + register project on this machine
# after someone else clones a harnessed repo:
harness link             # register clone + reindex committed history

harness intake --type spec_slice --summary "..." --lane normal
harness story add --id US-001 --title "..." --lane normal
harness query matrix
harness search "verify story"
harness get US-001
```

Users install via **npm** (`-g` preferred).

**Agent mutation rule (mandatory):** agents change operational durable state
**only** through harness CLI tools — never by hand-editing story/decision/intake
/backlog markdown.

**Hard-fail (decision 0017):** if harness CLI or MCP fails for a required step,
**HARD STOP** — do not hand-edit durable entities; run `harness doctor` /
`link` / `reindex` as needed, then retry.

**Implemented CLI surface:** run `harness --help`. Do not copy a remembered
list — the shipped CLI is the contract (decision 0023).

**Tracking:** `docs/product/roadmap.md` and `docs/stories/README.md`.

## Product Direction (locked — decision 0011)

1. **Distribution:** npm package **`5harness`** with bins `harness` / `5harness`; preferred `npm i -g 5harness`.
2. **Init + link:** `init` scaffolds project markdown and registers the path in
   `~/.5harness`; `link` registers an existing clone for dashboard/query.
3. **Durable SoT:** markdown entities in the project (Git-backed). Derived index
   is local/rebuildable. Traces are machine-local by default.
4. **Agents:** mutate durable state only via CLI tools; use get/search/links/
   query for reads (no whole-vault dumps).
5. **Dashboard:** browser UI over machine-local registry + project paths.
6. **Out of near-term scope:** cloud registry, vector RAG as primary search,
   project SQLite as SoT (superseded).

## Project Skills

If `.agents/skills/harness/SKILL.md` exists, use it when a request needs
discussion, feature intake, docs, or story shaping. The skill is
project-scoped; `harness init` installs it. Do not use a global copy as the
source of truth.

<!-- HARNESS:BEGIN -->
<!-- harness-version: 0.27.0 -->
<!-- harness-project-id: 2155089a1e379d9ebae4b4ac654e7360 -->
## Harness

**5harness** (bin `harness`) is this repo's operating system for **coding
agents**. Durable work — stories, decisions, intakes, backlog, reports — is
Git-backed markdown. **You** (the agent) read and write that history through
the CLI. You do not invent commands. You do not hand-edit those files.

### First commands

```bash
npm i -g 5harness          # if `harness` is missing
harness link               # after cloning a harnessed repo
harness doctor --json
harness status --json
harness next --json
```

Do **not** start by dumping `docs/HARNESS.md`, `ARCHITECTURE.md`, or
`CONTEXT_RULES.md`. Open those only when the task changes harness or product
rules. Prefer `.agents/skills/harness/SKILL.md` when present.

### Work loop

```bash
harness intake --type <type> --summary "…" --lane tiny|normal|high-risk
harness story add --id US-… --title "…" --lane normal
harness story start US-…                 # also: --id US-…
# implement the slice
git add <slice files> && git commit -m "feat: …"
harness story done US-…                  # or: story update --id … --status implemented
harness next --json
```

`story add` / `update` use `--id`. `story start` / `done` / `block` take a
positional id (`--id` also works).

### Mutation rule (mandatory)

**Do not** create or edit operational durable markdown by hand
(stories / decisions / intakes / backlog / reports).

```bash
harness intake --type … --summary "…" --lane normal
harness story add --id US-… --title "…" --lane normal
harness story update --id US-… --status implemented --unit 1 --integration 1 --e2e 0 --platform 0
harness decision add --id … --title "…" --doc docs/decisions/….md
harness query matrix --json
```

All mutation commands auto-reindex after writing. You do NOT need to call
`harness reindex` manually after mutations.

### Commit after each completed slice (mandatory)

When a small task is actually done (code + tests/docs for that slice, or a
durable write that should travel with the repo):

1. `git add` only the files for that slice.
2. `git commit` with a conventional message (`feat:`, `fix:`, `docs:`, `chore:`).
3. Do **not** wait for the whole epic.
4. Do **not** `git push` unless the user asked.
5. Never commit `.5harness/`, secrets, `node_modules`, or unrelated dirty files.

Skip if there is nothing to commit, or the user forbade commits.

### HARD STOP — harness failure contract (decision 0017)

If the harness **CLI** or **MCP** fails, is missing, or returns a non-zero /
error result for a step you need:

1. **HARD STOP** that durable-write path. Do **not** continue as if it succeeded.
2. **Never** fall back to hand-editing story / decision / intake / backlog
   markdown to “fix” or bypass the failure.
3. **Recover**, then retry the harness command:

| Order | Command | Why |
| --- | --- | --- |
| 1 | `harness --version` | Confirm install / PATH |
| 2 | `harness doctor` or `harness doctor --json` | Workspace health |
| 3 | `harness link` | Register clone / registry pointer |
| 4 | `harness reindex` | Rebuild derived index from markdown |
| 5 | `harness status` / `harness next` | Confirm the project is usable |

**Exit codes:** `0` = success; `1` = usage / validation / operational error
(**stop**, fix, retry); `2` = reserved — treat as non-success unless that
command’s docs say otherwise. Non-zero exit is never success.

### Read with tools (prefer `--json`)

```bash
harness search "…" --json
harness get <id> --json
harness links <id> --json
harness context <id> --json
harness query matrix --json
harness query stats --json
harness query reports --json
```

Classify work with feature intake before large edits. Record durable decisions
when architecture or product rules change.

### MCP (optional)

Prefer the CLI. If MCP is connected, use JSON tools (`harness_next`,
`harness_get`, `harness_query_*`). Discover the project id with
`harness project id` or the `<!-- harness-project-id: … -->` comment.
For an all-projects grant, send `X-Harness-Project: <id>` (preferred) or
`?project=<id>` on every call. Never rely on cwd to select a project.

### Upgrade

When a newer harness CLI version is installed (`npm i -g 5harness`),
run `harness upgrade` to update the harness block in this AGENTS.md.
Only the harness-managed section (markers HARNESS:BEGIN through HARNESS:END)
is modified — all other content is preserved.
<!-- HARNESS:END -->

## Security Audit Log

<!-- SECURITY-AUDIT-LOG:BEGIN -->
<!-- One row per unique audit fingerprint; keep detailed evidence in the linked report. -->
| audit_id | utc | mode | revision | fingerprint | coverage | findings | disposition | report | trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SA-20260906-0157Z-df6bdc686ef2b589 | 2026-09-06T02:00Z | system | HEAD@756ec520b541ab3a7a236000c3e29a1df47ecb1d | sha256:df6bdc686ef2b589499385b56df6d3c155f81cbc158ff1615643181f4d5117a6 | npm launcher/installers/dashboard/MCP/verify/filesystem/CI/dependencies/governance | 1 confirmed info/low installer authenticity gap; 1 likely low slow-client availability gap; controls otherwise not-a-finding | follow-up | docs/security/audits/SA-20260906-0157Z-df6bdc686ef2b589.md | TRACES-1788660005041396008 |
<!-- SECURITY-AUDIT-LOG:END -->
