# Harness

**Agent-ready repository harness** — a global npm CLI that turns any software
repo into a structured workspace for humans and coding agents.

> The app is what users touch. The harness is what agents touch.

[![CI](https://github.com/vantanminh/5harness/actions/workflows/ci.yml/badge.svg)](https://github.com/vantanminh/5harness/actions/workflows/ci.yml)
[![npm version](https://img.shields.io/npm/v/5harness.svg)](https://www.npmjs.com/package/5harness)
[![Node.js](https://img.shields.io/node/v/5harness.svg)](https://www.npmjs.com/package/5harness)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Provenance](https://img.shields.io/badge/npm-provenance-green)](https://docs.npmjs.com/generating-provenance-statements)

| | |
| --- | --- |
| **Package** | [`5harness`](https://www.npmjs.com/package/5harness) |
| **Bin** | `harness` |
| **Source** | [github.com/vantanminh/5harness](https://github.com/vantanminh/5harness) |
| **Node** | ≥ 22.5 |
| **License** | [MIT](LICENSE) |

## Install

Preferred (global — multi-project + dashboard):

```bash
npm i -g 5harness
harness --version
```

Automatic native install (no compile from source):

**Windows (PowerShell):**

```powershell
$version = "0.26.2"
Invoke-WebRequest "https://raw.githubusercontent.com/vantanminh/5harness/v$version/install/windows.ps1" -OutFile install-5harness.ps1
Get-Content .\install-5harness.ps1
powershell -ExecutionPolicy Bypass -File .\install-5harness.ps1
```

**macOS:**

```bash
VERSION=0.26.2
curl --proto '=https' --tlsv1.2 -fsSL "https://raw.githubusercontent.com/vantanminh/5harness/v${VERSION}/install/macos.sh" -o install-5harness.sh
less install-5harness.sh
bash install-5harness.sh
```

**Linux:**

```bash
VERSION=0.26.2
curl --proto '=https' --tlsv1.2 -fsSL "https://raw.githubusercontent.com/vantanminh/5harness/v${VERSION}/install/linux.sh" -o install-5harness.sh
less install-5harness.sh
bash install-5harness.sh
```

Point any native installer at a local build with `HARNESS_INSTALL_FROM`
(directory or binary path). Pin a release with `HARNESS_INSTALL_VERSION=<version>`
and use `HARNESS_INSTALL_SKIP_PATH=1` in automation. Release installers fetch
the matching `SHA256SUMS` manifest and refuse to execute a binary whose
SHA-256 does not match. npm install remains the
preferred option and works on all three supported operating systems.

Project-local (optional):

```bash
npm i -D 5harness
npx harness --help
```

Releases use **npm trusted publishing (OIDC)** with **provenance** when
configured. See [docs/product/distribution.md](docs/product/distribution.md).

## Quick start

```bash
cd /path/to/project
harness init                 # scaffold markdown + register this project
# after git clone of a harnessed repo on another machine:
harness link                 # register path + reindex committed history

harness intake --type spec_slice --summary "Add export API" --lane normal
harness intake update --id IN-001 --stories US-001
harness intake close IN-001
harness story add --id US-001 --title "Export API" --lane normal
harness story update --id US-001 --status implemented --unit 1 --integration 1 --e2e 0 --platform 0
harness decision add --id 0001 --title "Use markdown SoT" --doc docs/decisions/0001.md
harness query matrix
harness search "export"
harness get US-001
harness links US-001
harness doctor
harness next
harness dashboard            # or bare: harness
```

## Features

| Feature | What it does |
| --- | --- |
| **Init / link** | Scaffold agent docs + markdown entities; register project in `~/.5harness` |
| **Durable history** | Stories, decisions, intakes, backlog, reports as **Git-backed** markdown |
| **Agent index** | `search` / `get` / `links` / `reindex` — no whole-vault dumps |
| **Tools-only mutation** | Agents change operational entities **only** via CLI (or MCP tools) |
| **Project Link** | Opt-in roles and configured peers with bounded reads + target-owned reports |
| **Quality loop** | `verify`, `trace`, `audit`, `propose` |
| **Agent loop** | `doctor`, `status`, `next`, `context`, `handoff`, `watch` |
| **MCP** | Local `harness mcp` / dashboard MCP — reads + durable mutations (US-041) |
| **Dashboard** | Localhost multi-project UI + optional MCP monitoring |
| **Releases** | CI multi-OS matrix, OIDC publish, GitHub Releases, SBOM |

### Project Link (opt-in)

Project Link lets related repositories declare roles, resolve explicitly
configured peers, read bounded peer context, and exchange durable reports:

```bash
# frontend project
harness project role set frontend --stack supabase
harness project peer add <backend-project-id-or-path> --role backend
harness peer search "auth contract" --role backend
harness report add --to backend --summary "Login response contract mismatch"

# backend project
harness report list --status open
harness report get RP-001
harness report update --id RP-001 --status fixed --resolution "Updated response schema"
```

Role, stack, and peer ids are Git-backed markers in the managed `AGENTS.md`
block. Peer paths are not committed: both repositories must be registered in
the same machine-local `~/.5harness` registry (or `HARNESS_HOME`). Reads are
limited to direct configured peers and bounded `search` / `get` / `context` /
`links` results; peer-of-peer traversal and arbitrary paths are rejected.

Reports are target-owned Git-backed entities under `docs/reports/`. Create and
update them only through `harness report` or its MCP tools, never by hand. Keep
payloads sanitized: do not include credentials, tokens, secrets, or unnecessary
personal data. Multi-user machines can restrict report targets with
`HARNESS_PEER_WRITE_ROOTS`, an OS-path-delimited list of existing absolute
directories. When set, CLI and MCP report creation fail closed for targets
outside those canonical roots. `doctor` reports policy mismatches in addition
to unresolved peers and missing peer indexes;
`status` summarizes Project Link state; `next` surfaces open reports before
planned backend work.

### MCP authentication

`harness mcp` is an authenticated, project-bound streamable HTTP endpoint. The
server prints a per-process bearer token when it starts (or accepts `--token` /
`HARNESS_MCP_TOKEN`). Discovery and protocol negotiation are public; every
`tools/call` requires both the bearer token and the calling project's
`X-Harness-Project` id. Requests are bounded (1 MiB body, 16 KiB individual
headers, 64 headers / 64 KiB total header bytes, 64 KiB strings, 32 nesting
levels, 1,000 collection entries) and generated tokens expire after 24 hours by
default. Serialized responses are capped at 1 MiB. Non-loopback MCP binds are
rate-limited to 120 requests/minute per source by default. The dashboard `/mcp` endpoint is discovery-only; use
`harness mcp` for authenticated tool calls.

```bash
harness dashboard              # dashboard + discovery metadata
harness mcp                    # authenticated MCP resource
harness dashboard set-password --password '<12+ character password>'
```

Every project-bound tool call requires the target project's durable id:

```bash
cd /path/to/project
harness project id             # also stored as harness-project-id in AGENTS.md

# Preferred all-projects request selector:
X-Harness-Project: <project-id>
# Compatibility selector: append ?project=<project-id> to the MCP resource URI
```

Missing, conflicting, unknown, or unavailable project ids fail closed. Agents
must not infer authorization from cwd. See the [security model](docs/SECURITY.md).

After binding the calling project, MCP tool discovery dynamically exposes peer
read and report tools only when that project has configured peers. In an
`X-Harness-Project` still selects the **calling** project; a peer id never
substitutes for that project binding. Cross-project
operational-entity mutation is restricted to sanitized reports owned by the
configured target project; explicit peer-management commands may also attempt
reverse configuration markers.

Plain HTTP is accepted only on loopback. A non-loopback dashboard or MCP bind
requires an HTTPS reverse proxy and its canonical URL; dashboards also require
a configured Argon2id password. For example:
`harness mcp --host 0.0.0.0 --public-url https://mcp.example.com`.

Product pivot: [decision 0011](docs/decisions/0011-global-tool-markdown-durable-index.md).

## Agent rules (summary)

1. **First commands:** `harness doctor --json`, `harness next --json`. Then
   the harness block in `AGENTS.md` or `.agents/skills/harness/SKILL.md`.
   Do not dump the whole docs tree.
2. **Mutate only via tools:** `harness intake` / `story` / `decision` /
   `backlog` / `report` — **never** hand-edit operational entity markdown.
3. **Hard-fail (decision 0017):** if harness CLI or MCP fails for a required
   step → **HARD STOP**. Recover with `doctor` / `link` / `reindex`, then
   retry. Do not bypass with hand-edits.
4. **Prefer queries:** `search`, `get`, `links`, `query matrix|stats`, `next`
   with `--json`.
5. **Commit after each completed slice.** Do not push unless asked.

Full contract: [AGENTS.md](AGENTS.md) · [docs/CONTEXT_RULES.md](docs/CONTEXT_RULES.md).

## Security

- **Report vulnerabilities** privately — [SECURITY.md](SECURITY.md)
- **Trust model** (verify commands, MCP local-only, registry paths, secrets,
  provenance for consumers): [docs/SECURITY.md](docs/SECURITY.md)
- **Threat model** (assets, boundaries, mitigations, residual risk):
  [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)

`harness story verify --allow-project-command` runs the **project-authored** `verify` command from story
markdown only with explicit `--allow-project-command` approval (intentional
local proof, like CI scripts). Commands are bounded to one non-empty 8 KiB line,
and verification output is redacted/truncated before it is persisted. MCP never
enables this approval automatically.

## Changelog

Notable changes: [CHANGELOG.md](CHANGELOG.md) (Keep a Changelog + semver).

Draft assist from durable history:

```bash
harness export changelog [--since 2026-07-01]
```

## Current status

| Area | Status |
| --- | --- |
| npm package | **`5harness`** — bin `harness` (was `@vantanminh/harness`; see [docs/DEPRECATION.md](docs/DEPRECATION.md)) |
| `init` / `link` / registry | Shipped |
| Markdown durable SoT | Shipped |
| Query + agent index | Shipped |
| Quality (verify / trace / audit / propose) | Shipped |
| Agent-loop tools (doctor / status / next / …) | Shipped |
| Project Link (roles / peers / reports) | Shipped (v0.21+) |
| MCP core (reads + intake/story/decision/backlog mutations) | Shipped |
| Local dashboard + MCP monitor | Shipped |
| CI multi-OS + OIDC provenance releases | Shipped |
| Legacy SQLite import | Optional (`harness import-sqlite`) |

## Local development

```bash
npm install
npm run build
npm test
npm run pack:check          # tarball + version sync
# full gate:
npm run release:check
node dist/cli.js --help
# or: npm run harness -- --help
```

## CI / CD

- **CI** (push/PR): `release:check` + npm tarball smoke on **ubuntu / windows /
  macos × Node 22.19.0 + 24.19.0**, plus native installer smoke on each OS
- **Auto-release** (push to `main`): tag the release commit first, build native
  artifacts once, then publish the same files with **OIDC npm provenance**,
  SHA-256 checksums, attestations, GitHub Release, and SBOM (skip with
  `[skip release]`)
- **Manual:** Actions → Release, or matching `v*` tag

Details: [docs/product/distribution.md](docs/product/distribution.md).

## Read first (agents & contributors)

1. [AGENTS.md](AGENTS.md) — identity, mutation rules, hard-fail  
2. [docs/product/overview.md](docs/product/overview.md) — product contract  
3. [docs/HARNESS.md](docs/HARNESS.md) — collaboration loop  
4. [docs/FEATURE_INTAKE.md](docs/FEATURE_INTAKE.md) — lanes before code  
5. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — stack and layering  
6. [docs/decisions/](docs/decisions/) — locked choices  
7. [docs/product/roadmap.md](docs/product/roadmap.md) — implementation tracking  

## Update notices

The CLI may print a one-line notice on stderr when a newer npm version exists.
Successful checks are cached for one hour under `~/.5harness/`; transient npm
errors retry after five minutes. Disable with
`HARNESS_NO_UPDATE_CHECK=1` (also auto-disabled when `CI=true`).

```bash
harness update    # reinstall latest with detected package manager
harness upgrade   # refresh harness block in project AGENTS.md
```

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgments

Thanks to the authors of
[repository-harness](https://github.com/hoangnb24/repository-harness) for ideas
we studied while designing this product.
