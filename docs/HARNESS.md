# Harness

The project goal is to provide a reusable operating harness that lets humans and
agents turn a future product spec into safe, validated work.

The app is what users touch. The harness is what agents touch.

## Mental Model

```text
------------------+
| Human intent    |
+------------------+
         |
         v
+------------------+
| Feature intake   |
+------------------+
         |
         v
+------------------+
| Story packet     |
+------------------+
         |
         v
+------------------+
| Agent work loop  |
+------------------+
         |
         v
+------------------+
| Product delta    |
+------------------+
         |
         v
+------------------+
| Validation proof |
+------------------+
         |
         v
+------------------+
| Harness delta    |
+------------------+
         |
         v
+------------------+
| Next intent      |
+------------------+
```

Every task has two possible outputs:

1. Product delta: app code, tests, API shape, data model, or product docs.
2. Harness delta: docs, templates, validation expectations, backlog items, or
   decision records that make the next task easier.

## Product scope (this repo)

This repository **is** the harness product (`npm-harness` / bin `harness`).

Includes:

- Agent entrypoint and collaboration docs.
- Feature intake and risk lanes.
- Story / decision / validation templates.
- Product contracts under `docs/product/` (including **roadmap**).
- Implementation story packets US-001+ under `docs/stories/`.
- TypeScript CLI (v0.5: SQLite MVP; pivot 0011: markdown SoT + global registry).

Tracking map: **`docs/product/roadmap.md`**.

## Durable Layer

Policy documents describe how to work. The durable layer records what happened.

### Direction (decision 0011)

| Kind | Storage | Git |
| --- | --- | --- |
| Stories, decisions, intakes, backlog, reports | Markdown entities in the project | **Yes** |
| Derived search index | `.5harness/index/` | No |
| Traces | Machine-local | No |
| Multi-project pointers | `HARNESS_HOME` / `~/.5harness` | No |

**Agents must only mutate operational durable state through the harness CLI** —
never by hand-editing entity markdown.

**Hard-fail (decision 0017):** if the harness CLI or MCP fails for a required
step, agents **HARD STOP** that path, run recovery (`harness doctor`,
`harness link`, `harness reindex` as needed), and retry the tool. They must
not bypass failures by editing story / decision / intake / backlog / report files by
hand. See the harness block in `AGENTS.md` and
`docs/decisions/0017-agent-hard-fail-contract.md`.

Collaborator workflow:

```bash
git clone <repo>
npm i -g 5harness
harness link          # register path + reindex committed history
harness query matrix
```

### Project Link (opt-in)

Project Link connects related repositories without making every linked project
visible to every agent. Configure the local role and direct peers explicitly:

```bash
harness project role set frontend --stack supabase
harness project peer add <backend-project-id-or-path> --role backend
harness project peer list
harness peer context US-088 --role backend --max-chars 8000
harness report add --to backend --summary "Login response contract mismatch"
```

Role, stack, and peer ids live in the managed `AGENTS.md` block. Opting in also
injects a short role-aware workflow there; plain projects keep the original
agent instructions. Peer paths remain machine-local. After cloning, run
`harness link` in each repository so the durable ids can resolve through the
same `~/.5harness` registry (or `HARNESS_HOME`).

Peer reads are limited to configured, direct peers and bounded
`search` / `get` / `context` / `links` results. They never accept an arbitrary
filesystem path or traverse a peer's peers. After explicit peer configuration,
the only cross-project operational-entity mutation is a sanitized `report`
created in the configured target project; peer-management commands may also
attempt reverse AGENTS markers.
Reports live under the target's `docs/reports/` and must be created or updated
through `harness report`, never by hand. Do not put credentials, tokens, secrets,
or unnecessary personal data in a report.

For tighter machine-local write boundaries, set `HARNESS_PEER_WRITE_ROOTS` to
existing absolute directories separated by the operating system's path-list
delimiter (`;` on Windows, `:` on macOS/Linux). Report creation through both
CLI and MCP canonicalizes the target and fails closed unless it is inside one
of those roots. An invalid policy also fails closed. The variable is optional;
unset behavior continues to trust direct, explicitly configured peers.

For a backend project, `harness report list --status open` is the report inbox;
acknowledge and resolve each item with `harness report update`. `doctor` warns
about unresolved peers, missing peer indexes, or report targets outside the
configured write roots; `status` shows role/stack/peer
and open-report counts, and `next` places open reports after in-progress work
but before planned stories.

MCP always binds the caller first. Peer read and report tools are advertised
dynamically only when the calling project has configured peers. With an
MCP bearer authentication plus `X-Harness-Project` selects the calling project;
the peer id is resolved afterward and never acts as the project selector.

### Product CLI

The shipped CLI uses Git-backed markdown as the durable source of truth. The
local index, traces, and registry are derived or machine-local state.

Prefer the product CLI:

```bash
npm run harness -- --help
# or after build: node dist/cli.js …
# or global: harness …
```

Common product commands:

```bash
harness init
harness intake  --type <type> --summary <text> --lane <lane>
harness story   add --id <id> --title <text> --lane <lane>
harness story   update --id <id> --status <status>
harness story   update --id <id> --unit 1 --integration 1 --e2e 0 --platform 0
harness decision add --id <id> --title <text> --doc docs/decisions/<file>.md
harness report add --to <role-or-id> --summary <text>
harness query   matrix
harness query   stats
harness audit
harness propose
```

Read, registry, and dashboard commands:

```bash
harness link | unlink | projects
harness reindex | get | search | links
harness dashboard
```

## Source Hierarchy

```text
User-provided spec or prompt
  input material for first buildout or future changes

docs/product/*
  current product contract derived from accepted input

docs/stories/*
  story-sized work packets and historical evidence

harness query matrix
  behavior-to-proof control panel backed by the durable layer

docs/decisions/*
  why the contract changed
```

Before implementation, product docs describe intent. After implementation,
product docs plus executable tests become the living contract.

## Spec Lifecycle

Harness v0 starts without a tracked project spec. When the human provides a
specification, treat it as input material, not as a permanent operating manual.
Use it to populate product docs, story packets, architecture decisions, and
validation expectations during the first buildout.

After the specification has been decomposed, do not keep extending it as the
living product plan. Ongoing work should update the smaller product docs,
stories, durable proof records, and decision records.

Ongoing work should enter the harness as one of these input types:

- New spec: a project specification that needs to become product docs and
  initial story candidates.
- Spec slice: a selected behavior from the provided spec.
- Change request: a bounded behavior change, bug fix, or product refinement.
- New initiative: a larger product area that needs multiple stories.
- Maintenance request: dependency, architecture, performance, security, or
  operational work.
- Harness improvement: a process, template, proof, or agent-instruction change.

The spec-to-work loop is:

```text
human intent or supplied spec
  -> classify input type
  -> update or create product contract
  -> create story packet or initiative notes when needed
  -> define validation proof
  -> implement or document the blocker
  -> update product docs, stories, durable proof records, and decisions
  -> capture harness friction
```

Large product areas should use scoped initiative notes instead of a second
monolithic specification. An initiative should explain the goal, affected
product docs, candidate stories, validation shape, open decisions, and exit
criteria. If initiative work becomes a repeated pattern, add a template or
record the proposal with `harness backlog add`.

## Growth Rule

The harness grows from friction.

When an agent is confused, repeats manual reasoning, needs a new validation
command, discovers a missing rule, or sees a recurring failure pattern, it must
either improve the harness directly or record the friction:

```bash
harness backlog add --title "<short name>" --pain "<what was hard>"
```

Use the backlog outcome loop for improvements that are expected to change agent
behavior or validation results:

1. When creating the backlog item, fill `--predicted` with the measurable
   impact expected from the improvement.
2. When closing the item, fill `--outcome` with the actual measured result or
   review evidence.
3. Use `harness query backlog --open` to review proposed and accepted
   items, and `harness query backlog --closed` to compare predictions
   with outcomes after implementation.

The `harness_friction` field on traces also captures per-task friction so
later audits and `harness propose` can see repeated patterns. Review traces
with `harness query traces`. There is no separate `query friction` command
(decision 0023).

Backlog risk uses the same lane vocabulary as intake and stories:
`tiny`, `normal`, or `high-risk`. Use `--risk tiny` for low-risk follow-up
items; `low` is not a valid lane.

## Task Loop

For every task:

1. Classify the request with `docs/FEATURE_INTAKE.md`.
2. Record the classification with `harness intake`.
3. Locate the affected product docs and story files.
4. Check proof status with `harness query matrix`.
5. Work only inside the selected lane: tiny, normal, or high-risk.
6. Before finishing, ask whether product truth, validation expectations,
   architecture rules, repeated failure patterns, or next-agent instructions
   changed.
7. Record a trace with `harness trace`, using
   `docs/TRACE_SPEC.md` for the expected trace tier and field depth.
8. Review the trace score printed by `harness trace`; use
   `harness score-trace --id <id>` only when re-checking a
   specific historical trace.
9. If harness friction was found, either fix it directly or record it with
   `harness backlog add`.

## Story Verification

Stories may carry a mechanical proof command:

```bash
harness story add --id US-012 --title "Story verification" --lane normal --verify "npm test"
harness story update --id US-012 --verify "npm test"
harness story verify US-012 --allow-project-command
```

`story verify` runs the command from the repository root, records
`last_verified_at` and `last_verified_result`, and exits 0 on pass or 1 on fail.
When `trace --story <id>` links to a story whose verification command has never
passed, the trace still records but prints an advisory warning before close.

Use `story verify-all --allow-project-command` before merges, maturity claims,
and benchmark runs. It preflights every configured story verification command,
prints one result per story, skips stories without `verify`, and exits 1
if any configured story fails. Without the explicit flag it refuses before
executing any project-authored command.

`story verify` accepts the story id plus the explicit trust flag. Configure the
command with `story add --verify` or `story update --verify`. Record proof booleans with
`story update`, using numeric values: `1` means yes and `0` means no. The CLI
rejects text values such as `yes` and `no`.

Use `harness query matrix --numeric` when copying proof values
back into `story update`. The default matrix output is human-readable
`yes`/`no`; the numeric output mirrors CLI input.

## Evolution Commands

Tool discovery:

```bash
harness query tools --summary
harness query tools --json
harness tool register --name <name> --command <cmd> --description <text> --responsibility Verification
```

Drift checks:

```bash
harness audit
```

`audit` reports drift categories and an entropy score documented in
`docs/HARNESS_AUDIT.md`. Human, reviewer, CI, or agent corrections belong in
the story evidence, worklog, or a new trace — there is no separate
intervention store (decision 0023).

Improvement proposals:

```bash
harness propose
harness propose --commit
```

`propose` prints deterministic proposals from audit findings and recorded
trace friction. `--commit` creates proposed backlog items only; it does not
edit policy docs or approve the proposal.

## Decision Records

High-risk work needs durable decisions when it changes behavior or architecture.
For auth, authorization, data ownership, API shape, audit/security, or
validation changes, record the decision in both places:

1. Add a markdown file under `docs/decisions/` from
   `docs/templates/decision.md`.
2. Add or refresh the durable record:

```bash
harness decision add \
  --id 0008-auth-boundary \
  --title "Auth Boundary" \
  --doc docs/decisions/0008-auth-boundary.md \
  --notes "Accepted during T4 authentication work."
```

The trace `--decisions` field is useful evidence, but it is not the decision
log. Do not treat decision text in a trace as satisfying the durable decision
record requirement.

## Harness Change Policy

Agents may update directly:

- Story status and evidence via `harness story update`.
- Test matrix rows via `harness story add` and
  `harness story update`.
- Links from story packets to product docs.
- Validation notes and reports.
- Small clarifications tied to the current task.
- Intake records, traces, and backlog items via `harness`.

Agents should ask for human confirmation before:

- Changing architecture direction.
- Removing validation requirements.
- Changing the source-of-truth hierarchy.
- Changing risk classification rules.
- Replacing the feature workflow.

## Done Definition

A task is done only when:

- The requested change is completed or the blocker is documented.
- Relevant docs, stories, and test matrix entries remain current.
- Validation commands were run when they exist.
- A trace has been recorded with `harness trace`.
- Missing harness capabilities were recorded with
  `harness backlog add`.
- The final response says what changed and what was not attempted.

## Validation Ladder (this product)

This repository's mechanical proof is the pinned Rust + npm release gate:

```text
cargo fmt --check
  formatting

cargo clippy --all-targets -- -D warnings
  linting

cargo test --all-targets
  unit, integration, and CLI e2e suites

cargo audit && cargo deny check
  Rust advisory/license/source policy

npm run pack:check
  npm pack dry-run and published-file assertions

npm run release:check
  security check + fmt + clippy + test + pack:check
```

Target projects may set their own `verify` commands on stories
(`npm test`, `cargo test`, `go test`, …). Agents must not claim a command
passes until it exists and has been run.
