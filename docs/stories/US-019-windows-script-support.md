# US-019 Windows Script Support

## Status

implemented

## Lane

normal

## Product Contract

Harness source-repo scripts can be run from a Windows environment without
requiring macOS/Linux-only shell entrypoints for the main local workflows.

## Relevant Product Docs

- `README.md`
- `scripts/README.md`
- `docs/decisions/0005-prebuilt-rust-harness-cli.md`

## Acceptance Criteria

- Windows users have a native PowerShell release-build command for the Harness
  CLI source repo.
- Release-build script options stay aligned across Bash and PowerShell entrypoints.
- Windows usage is documented for both installer and source-repo release
  packaging workflows.
- The local Windows workflow can produce `scripts/bin/harness-cli.exe` and run
  `query matrix`.

## Design Notes

- Commands: `scripts/build-harness-cli-release.ps1`, `scripts/install-harness.ps1`,
  `.\scripts\bin\harness-cli.exe query matrix`
- Queries: `query matrix`
- API: none
- Tables: `story`, `intake`, `trace`
- Domain rules: keep the repo-local Harness CLI entrypoint stable while adding
  Windows-native source-repo tooling.
- UI surfaces: terminal output only

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id <id> --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | `cargo test --package harness-cli` passes on Windows. |
| Integration | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-harness-cli-release.ps1` writes a Windows artifact and checksum. |
| E2E | Copy the built CLI to `scripts/bin/harness-cli.exe`, run `init` and `query matrix` in the repo root. |
| Platform | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-harness.ps1 -DryRun -Yes` works on Windows. |
| Release | `cargo build --package harness-cli` succeeds on Windows. |

## Harness Delta

Adds a Windows-native build script and updates repo docs so Windows is treated
as a first-class source-repo workflow, not only as an installed-project target.

## Evidence

- `cargo test --package harness-cli`
- `cargo build --package harness-cli`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-harness-cli-release.ps1`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-harness.ps1 -Directory . -DryRun -Yes -Merge`
- `Copy-Item dist\harness-cli-windows-x64.exe scripts\bin\harness-cli.exe`
- `.\scripts\bin\harness-cli.exe --version`
- `.\scripts\bin\harness-cli.exe init`
- `.\scripts\bin\harness-cli.exe import brownfield`
- `.\scripts\bin\harness-cli.exe query matrix`
