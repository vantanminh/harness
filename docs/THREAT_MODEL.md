# 5harness threat model

This document describes the security boundary for the public developer CLI.
5harness is a single-user, local-first repository harness; it is not a
multi-tenant authorization boundary and it does not make an untrusted
repository safe to execute.

## Assets

- Source files and Git-backed durable history (stories, decisions, intakes,
  backlog, and reports).
- Machine-local registry, indexes, traces, worklogs, MCP call records, and
  dashboard credentials under `$HARNESS_HOME` / `.5harness`.
- Project environment, filesystem permissions, SSH credentials, and other
  secrets available to the operating-system user running the CLI.
- Release artifacts, npm package contents, checksums, SBOM, and provenance.
- Project names, paths, peer relationships, and report contents exposed by the
  dashboard or MCP.

## Trust boundaries

```text
Git-authored repository ──┐
                          ▼
                    5harness CLI
                 ┌────────┼────────┐
                 ▼        ▼        ▼
             project   local      verify
             files     state      shell
                 │        │        │
                 └────────┼────────┘
                          ▼
                  OS user / network

GitHub source → protected CI → immutable tag → one native build matrix
                                      ├→ npm (OIDC + provenance)
                                      └→ Release (SHA256 + SBOM + attestation)
```

The repository author controls durable markdown and source. The operator
controls the local machine, environment variables, registry, and whether a
project-authored command is trusted. A non-loopback HTTP client is outside the
local boundary and must use a TLS reverse proxy plus authentication.

## Threats and mitigations

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| Tampered binary downloaded by an installer | Versioned release URL; release `SHA256SUMS`; installers verify SHA-256 before copy or `--version`; optional detached signature and GitHub attestation | A compromised release account/key can still publish a matching checksum; verify provenance/signature and protect maintainer accounts |
| Release artifact differs between npm and GitHub | Release preparation tags first; matrix builds once; publish jobs verify tag commit and reuse the same uploaded artifacts | CI/GitHub/npm account compromise remains out of process scope |
| Mutable CI action/toolchain/dependency | Full-SHA Actions pins, pinned Rust toolchain, lockfiles, Dependabot Cargo/npm/actions, `cargo audit`, `cargo deny` | Pin updates still require review; runner compromise is not eliminated |
| Public dashboard exposes project data | Loopback default; non-loopback requires valid HTTPS URL and modern Argon2id password; security headers and no-store responses | Reverse-proxy/TLS configuration is operator responsibility |
| MCP unauthorized mutation or project confusion | Bearer token with TTL, constant-time comparison, required durable project id, conflicting selectors rejected, default-deny tool calls, request/depth/collection limits | Token theft by the local OS user or an exposed reverse proxy remains possible |
| Project-authored verify command executes attacker code | Single-line and length/null/newline validation; explicit `--allow-project-command`; verify-all preflight; no MCP verify tool | An operator who approves a hostile repository still grants shell execution; use an external sandbox for untrusted repos |
| Project-authored tool probe executes attacker code | `tool check` requires the same explicit approval and bounded command shape; command output/time are bounded; MCP does not expose tool-check execution | An approved probe still has the local OS user's permissions; use a trusted project script or external sandbox |
| Path traversal or symlink escape | Canonical containment checks for entity, local, peer, index, and report paths; symlink targets rejected; atomic owner-only writes | Filesystem races outside the process cannot be fully prevented without OS sandboxing |
| Secret leakage through diagnostics | Central redaction for bearer/password/token shapes in CLI and MCP errors; no tokens in durable payloads | Redaction is best effort and cannot identify every custom secret format |
| Malformed input causes excessive work | Bounded MCP body/header/string/depth/collection sizes; bounded request/response payloads; bounded entity context output; public per-source rate limit; parser errors are safe responses | Slow-client read timeouts and process-level isolation remain deployment concerns |
| XSS/clickjacking/cache disclosure in dashboard | Escaped project fields, CSP, `nosniff`, `Referrer-Policy`, `X-Frame-Options`, and `Cache-Control: no-store` | A browser extension or compromised local browser is outside the boundary |

## Security assumptions

1. The operating-system account running `harness` is trusted to access the
   selected project and its local credentials.
2. Git authors and reviewers decide whether repository-authored commands and
   mutations are trusted.
3. Release maintainers protect GitHub/npm accounts with passkeys or hardware
   keys, 2FA, minimal permissions, and reviewed applications.
4. Public dashboard/MCP deployments terminate TLS correctly and keep bearer
   tokens/passwords secret.
5. GitHub branch/ruleset protection and required CI are enabled by the
   repository owner. Dependabot vulnerability alerts and automated security
   fixes are enabled for this repository; secret scanning and push protection
   depend on GitHub plan/account settings and must be verified by the owner.
   These account/repository settings cannot be enforced from a checked-out
   worktree.

## Out of scope

- Multi-user or multi-tenant authorization, shared-host isolation, or hostile
  local users with the same OS privileges.
- Making arbitrary build/test/verify commands safe without an external
  sandbox.
- Protecting GitHub, npm, DNS, TLS reverse proxies, or maintainer hardware
  from account takeover.
- Availability against a deliberately overloaded local process or network.

## Security review checklist for new capabilities

Every feature or PR that adds a capability should answer:

1. What untrusted input is accepted, and are length, encoding, nesting, and
   schema limits explicit?
2. What can it read or write? Is the target canonicalized and contained under
   the project/approved peer root, including symlinks?
3. Does it execute a process, invoke a shell, access the network, open a port,
   or read environment variables/secrets?
4. Is it exposed through MCP or dashboard? What authentication, project
   binding, authorization, rate/resource limit, and audit behavior applies?
5. Can errors, logs, traces, HTML, or durable markdown echo credentials or
   attacker-controlled markup?
6. Are unit/integration/security tests included, and are docs and threat-model
   assumptions updated?

Changes that cross a trust boundary require a security review and a focused
regression test before release.
