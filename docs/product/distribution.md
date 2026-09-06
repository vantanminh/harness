# Distribution

## Package

| Field | Value |
| --- | --- |
| npm name | `5harness` |
| bin | `harness` / `5harness` / `5hn` → `dist/cli.js` (fixed-path shim → native Rust binary) |
| GitHub | [vantanminh/5harness](https://github.com/vantanminh/5harness) |
| **Preferred install** | `npm i -g 5harness` |
| Windows auto-install | Download `install/windows.ps1` from a versioned `vX.Y.Z` tag, inspect it, then run it |
| macOS auto-install | Download `install/macos.sh` from a versioned `vX.Y.Z` tag, inspect it, then run it |
| Linux auto-install | Download `install/linux.sh` from a versioned `vX.Y.Z` tag, inspect it, then run it |
| Alternate install | `npm i -D 5harness` + `npx harness …` |
| Node | `>=22.5.0` (packaging/publish glue; CLI runtime is native) |
| License | MIT |
| Former name | `@vantanminh/harness` — see [DEPRECATION.md](../DEPRECATION.md) |

## Install story (product)

```bash
npm i -g 5harness
cd /path/to/project
harness init          # new project: scaffold + register
# or after git clone of an already-harnessed repo:
harness link          # register path + reindex committed history
```

Global install matches multi-project use and a future local dashboard. Project
files (markdown) remain in the repo for GitHub backup and collaborator clones.

### Native installers

The direct installers are useful on a machine that does not have Node.js. They
download the native binary for the current OS and CPU from the matching GitHub
Release asset, install it under `~/.5harness/bin` (macOS/Linux) or
`%LOCALAPPDATA%\5harness\bin` (Windows), and run `harness --version` before
returning. Supported release targets are Linux `x86_64`/`aarch64`, macOS
`x86_64`/`arm64`, and Windows `x86_64`/`arm64`; the release workflows build
and publish each of these six target assets.

```bash
# Linux or macOS
VERSION=0.26.2
curl --proto '=https' --tlsv1.2 -fsSL "https://raw.githubusercontent.com/vantanminh/5harness/v${VERSION}/install/linux.sh" -o install-5harness.sh
less install-5harness.sh
bash install-5harness.sh

# Pin a release or install an offline/local artifact
export HARNESS_INSTALL_VERSION="$VERSION"
HARNESS_INSTALL_FROM=/path/to/artifacts ./install/linux.sh
```

```powershell
# Windows PowerShell
$version = "0.26.2"
Invoke-WebRequest "https://raw.githubusercontent.com/vantanminh/5harness/v$version/install/windows.ps1" -OutFile install-5harness.ps1
Get-Content .\install-5harness.ps1
powershell -ExecutionPolicy Bypass -File .\install-5harness.ps1
$env:HARNESS_INSTALL_VERSION = $version
$env:HARNESS_INSTALL_FROM = "D:\path\to\harness-x86_64-pc-windows-msvc.exe"
powershell -File install/windows.ps1
```

Set `HARNESS_INSTALL_PREFIX` to choose another install root and
`HARNESS_INSTALL_SKIP_PATH=1` when a CI job should not edit the user's PATH.
The installer scripts are shipped in the npm tarball as well as attached to
GitHub Releases, so the same commands can be tested offline with
`HARNESS_INSTALL_FROM`.

## Published artifacts

The npm tarball **must** include:

- `dist/cli.js` (thin Node shim with shebang that launches the target native binary)
- `bin/harness-*` platform binaries (Rust CLI)
- `templates/**` (init payload + `manifest.json`)
- `install/windows.ps1` and `install/macos.sh`
- `install/linux.sh`
- schema/templates needed for entity writes (as implemented)
- `package.json`, `README.md`, `LICENSE`

> Note: `migrations/**` remain only for legacy `harness import-sqlite`.
> Operational SoT is markdown under `docs/`.

### npm launcher security boundary

The npm entrypoint is intentionally a small Node bridge because one npm
package contains native binaries for multiple OS/CPU targets. It chooses only
package-relative `bin/harness-*` paths, passes command-line arguments as an
array, and sets `shell: false`. It does not read an environment-provided
executable path and does not probe the filesystem before launching.

The `child_process` capability reported by package scanners is therefore an
intentional, bounded delegation to the Rust CLI; it is not a shell command
interpreter. The native CLI itself necessarily reads and writes project files
and selected configuration environment variables as part of its documented
functionality.

## Release checklist

### Default (automatic)

1. Merge / push to `main` (do **not** hand-bump the version).
2. CI runs `release:check` on **ubuntu / windows / macos × Node 22.19.0 + 24.19.0**
   and runs native installer smoke tests on all three OSes.
3. On success, **Auto-release** (ubuntu + Node 24 only):
   - Detects bump kind from commits since last `v*` tag
     (`feat:` → minor, `BREAKING CHANGE` / `type!:` → major, else patch).
   - Override with commit markers: `[release: major]`, `[release: minor]`,
     `[release: patch]`.
   - Skip with `[skip release]` in the commit message.
   - **Release plan** (`scripts/release-plan.mjs`): if `v{version}` tag already
     exists → skip; if `package.json` is already ahead of the last tag →
     **tag-only** (no second bump); else bump as usual. Prevents duplicate
     `chore(release)` commits that diverge developer clones.
   - Serialized with concurrency group `harness-auto-release-main`.
   - **Push** via `scripts/git-push-release.mjs`: `fetch` + `pull --rebase` +
     retry so concurrent main updates do not leave a bare non-fast-forward.
   - Runs `npm run bump` when needed; keeps version files + CHANGELOG promote
     (US-038) in sync.
   - Commits `chore(release): X.Y.Z` when files change, tags `vX.Y.Z`.
   - Resolves and tags the release commit **before** native builds; every
     binary, npm tarball, and GitHub asset is produced from that same immutable
     ref (build once, publish everywhere).
   - **npm publish** via **OIDC trusted publishing** with **`--provenance`**
     (green provenance check on npm when configured).
   - Creates a **GitHub Release** with notes from `CHANGELOG.md`, all six
     native target binaries, `SHA256SUMS`, GitHub artifact attestations, an
     optional `SHA256SUMS.sig`, and an **SPDX SBOM** (`sbom.spdx.json`).

### Pushing from a local clone (avoid non-fast-forward)

CI may land a `chore(release): …` commit on `main` while you work. Always
rebase before push:

```bash
npm run push          # fetch + pull --rebase + push (scripts/safe-push.mjs)
# equivalent:
git fetch origin && git pull --rebase origin main && git push
```

Do **not** `git push --force` to `main`.

`safe-push` correlates local commits before and after rebase by stable patch id
and atomically refreshes matching machine-local worklog commit references. It
preserves short/full hash length and leaves unmatched references unchanged with
a warning. If rebase conflicts, resolve them and rerun `npm run push`; the
pre-rebase mapping snapshot is retained under `.git/` until reconciliation
succeeds.

### Authentication (US-036 / decision 0018)

| Method | Role |
| --- | --- |
| **npm Trusted Publisher (OIDC)** | **Preferred** for CI publishes — short-lived tokens, automatic provenance |
| **`NPM_TOKEN` secret** | Not read by release workflows; configure npm Trusted Publishing |

**One-time setup on [npmjs.com](https://www.npmjs.com)** for package
**`5harness`** → Settings → **Trusted Publisher** (required for green
provenance on CI; after the package exists from a first publish):

| Field | Value |
| --- | --- |
| Provider | GitHub Actions |
| Organization or user | `vantanminh` |
| Repository | `5harness` |
| Workflow filename | **`ci.yml`** (primary auto-release) |
| Environment name | leave empty unless you use GitHub Environments |

Day-to-day: push to `main` (or `npm run push`) — do **not** `npm publish`
from a laptop for production releases. CI uses `npm publish --provenance`.

Notes:

- Configure the trusted publisher for the workflow you use (`ci.yml` for
  automatic releases or `release.yml` for tag/workflow-dispatch releases).
- Requires **npm CLI ≥ 11.5.1** on the runner (workflows install pinned
  `npm@11.17.0`)
  and job permission **`id-token: write`**.
- After OIDC works, consider restricting token-based publish on npm
  (Settings → Publishing access) and revoking long-lived automation tokens.
- `package.json` `repository.url` must match the GitHub repo used for OIDC.

### Manual

- **GitHub UI:** Actions → **Release** → Run workflow → choose patch/minor/major.
- **Local tag (no auto-bump):** ensure version files already match, then
  `git tag vX.Y.Z && git push origin vX.Y.Z` (tag must equal `package.json`).
- **Local publish fallback** (no provenance — OIDC provenance only works on CI):

  ```bash
  npm run release:check
  npm publish --access public
  # Do NOT use --provenance on a laptop: npm error
  # "Automatic provenance generation not supported for provider: null"
  ```

  Prefer re-running the **Auto-release** / **Release** GitHub Action after
  Trusted Publisher is configured for package `5harness`.

### Local version bump (optional)

```bash
npm run bump          # patch
npm run bump -- minor
npm run bump -- major
npm run bump -- 1.0.0
```

### Release notes helper

```bash
node scripts/release-notes.mjs            # package.json version → stdout
node scripts/release-notes.mjs 1.2.3 -o release-notes.md
# Include durable-history assist (stories/decisions) after CHANGELOG body:
node scripts/release-notes.mjs 1.2.3 --with-export -o release-notes.md
```

### CHANGELOG discipline (US-038)

- **Source of truth:** human-edited `CHANGELOG.md` (Keep a Changelog + semver).
- **On bump:** `scripts/bump-version.mjs` promotes non-empty `[Unreleased]` into
  `## [X.Y.Z] - YYYY-MM-DD` and leaves an empty Unreleased section. Release
  commits include `CHANGELOG.md`.
- **Assist only:** `harness export changelog` / `--with-export` on release notes
  append implemented stories/decisions; they do **not** replace human judgment.
- **Drafting:** run `harness export changelog [--since <date>]` when preparing
  Unreleased notes, then edit into Added/Changed/Fixed/Security sections.

## CI / CD

| Workflow | Trigger | What it does |
| --- | --- | --- |
| [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) | push/PR → `main` | `release:check` + npm tarball smoke on **ubuntu + windows + macos × Node 22.19.0 + 24.19.0**, native installer smoke on all three OSes, and six-target native artifact build; release prep tags the commit before build, then publishes the same artifacts with **OIDC npm provenance**, checksums, attestations, Release, and SBOM |
| [`.github/workflows/codeql.yml`](../../.github/workflows/codeql.yml) | push/PR + weekly | Pinned CodeQL scans JavaScript/TypeScript and Rust with `security-events: write` limited to the analysis job |
| [`.github/workflows/release.yml`](../../.github/workflows/release.yml) | tag `v*` **or** workflow_dispatch | Resolves/creates the version tag first, builds six targets once, then publishes the same binaries to npm and GitHub Release with checksums, attestations, and SBOM |

Actions are pinned to Node-24-ready major versions (`actions/checkout@v6`,
`actions/setup-node@v6`) and set `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` per
GitHub’s Node 20 → Node 24 Actions migration.

Build jobs default to read-only permissions. Only release preparation/publish
jobs receive `contents: write`, `id-token: write`, and `attestations: write`.

## Consumer: verifying provenance

After a trusted publish, the package page on npm shows a provenance attestation
(“Built and signed on GitHub Actions”). Consumers can also use:

```bash
npm audit signatures
# or inspect the package on https://www.npmjs.com/package/5harness
```

GitHub Releases for each `vX.Y.Z` include release notes, all six native target
assets, `SHA256SUMS`, GitHub attestations, and `sbom.spdx.json` (plus
`SHA256SUMS.sig` when the maintainer signing key is configured).

## Native engine packaging

The current package ships the supported native artifacts under `bin/` and uses
the fixed-path npm bridge above. Release jobs build each OS target
independently, stage the binaries, attach the same files to the GitHub Release,
and publish with npm provenance. Do not add a postinstall downloader or an
environment-controlled executable override; use a separately reviewed
platform-package design if the target matrix grows beyond the current
artifacts.
