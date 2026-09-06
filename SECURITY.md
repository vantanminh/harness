# Security Policy

This document is the **public security policy** for
[`5harness`](https://www.npmjs.com/package/5harness)
([source](https://github.com/vantanminh/5harness)).

Operational trust model (verify commands, MCP, registry, secrets, provenance):
see **[docs/SECURITY.md](docs/SECURITY.md)**.

## Supported versions

| Version | Supported |
| --- | --- |
| Latest release on npm (`5harness`) | Yes — security fixes land here first |
| Previous minor on the same major (best effort) | Yes, for critical issues when practical |
| Older majors / unreleased `main` only | No guarantee; upgrade to latest |

Use a current Node.js LTS/current line that satisfies `engines` (`>=22.5.0`).

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Prefer one of:

1. **GitHub Security Advisories** (private):  
   [Report a vulnerability](https://github.com/vantanminh/5harness/security/advisories/new)
   on this repository (if enabled for the org/user).
2. **Maintainer contact** via the GitHub profile linked from the repository
   owner (`vantanminh`), with subject prefix `[SECURITY] harness`.

Include, when possible:

- Affected package version (`npm list -g 5harness` or `harness --version`)
- Environment (OS, Node version)
- Clear reproduction steps or proof-of-concept
- Impact assessment (data disclosure, local code execution, supply chain, etc.)

### Response expectations

| Stage | Target |
| --- | --- |
| Initial acknowledgement | Within **7 days** |
| Triage / severity | As soon as practical after acknowledgement |
| Fix or mitigation advisory | Coordinated; critical issues prioritized |

We may ask for more detail or a minimal repro. Please allow time for a fix
before public disclosure when the issue is not already widely known.

## Out of scope (typical)

- Issues that require an attacker to already control the local project’s Git
  history or machine user account (same trust class as “can edit the repo”)
- Denial of service against intentional local tools (CLI, dashboard, MCP on
  loopback) without a realistic multi-tenant exposure
- Vulnerabilities only in **devDependencies** that do not ship in the published
  tarball, unless they affect CI release integrity in a demonstrated way
- Social engineering of npm/GitHub account credentials (report to those vendors)

## Supply chain

- Releases use **npm trusted publishing (OIDC)** with **provenance** and build
  native binaries once from an immutable tag — see
  [docs/product/distribution.md](docs/product/distribution.md) and
  [docs/SECURITY.md](docs/SECURITY.md#release-provenance).
- GitHub Releases include `SHA256SUMS`, GitHub artifact attestations, and an
  **SPDX SBOM** asset (`sbom.spdx.json`); `SHA256SUMS.sig` is added when the
  maintainer signing key is configured.
- Dependency updates and gates cover npm, Cargo, and GitHub Actions:
  Dependabot, `cargo audit`, and `cargo deny check` are configured under
  `.github/` and `deny.toml`.
- Pinned CodeQL analysis covers the JavaScript/TypeScript and Rust surfaces on
  pushes, pull requests, and a weekly scheduled run.

## Safe use (summary)

- Run `harness` as a normal developer user; treat **verify** scripts as
  project-trusted shell (like CI `run:` steps). Execution requires the
  explicit `--allow-project-command` flag and is never implicitly enabled by
  MCP.
- Treat command-backed `harness tool check` probes the same way: review the
  project-authored command and pass `--allow-project-command` explicitly.
- Keep **dashboard** and **MCP** bound to **localhost** unless you understand
  the exposure of local project data.
- Project Link resolves only explicitly configured peers through the
  machine-local registry. Peer reads are bounded and direct; its only
  cross-project operational-entity mutation is a target-owned `report`.
  Explicit peer-management commands may also attempt reverse AGENTS markers.
- On shared or multi-user development machines, set `HARNESS_PEER_WRITE_ROOTS`
  to an OS-path-delimited list of existing absolute directories. Cross-project
  report creation canonicalizes the target and fails closed when it is outside
  every allowed root or the policy is invalid. The policy applies equally to
  CLI and MCP calls; `harness doctor` shows the effective peer-policy health.
  Leaving it unset preserves the explicit-configured-peer trust model.
- Sanitize Git-backed report payloads. Never include credentials, tokens,
  secrets, or unnecessary personal data in `docs/reports/` files.
- MCP exposes peer/report tools dynamically after binding the calling project.
  A peer id cannot replace the `X-Harness-Project` selector.
- Do not commit tokens; release workflows do not read `NPM_TOKEN` and require
  npm Trusted Publishing/OIDC for publication.

Thank you for helping keep harness and its users safe.
