param(
    [string]$Target = "",
    [string]$Profile = "release",
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    @'
Usage: build-harness-cli-release.ps1 [-Target <triple>] [-Profile <name>] [-OutDir <path>]

Build a prebuilt Harness Rust CLI artifact and checksum.

Options:
  -Target <triple>   Cargo target triple. Defaults to the host target.
  -Profile <name>    Cargo profile. Defaults to release.
  -OutDir <path>     Artifact directory. Defaults to dist.

Produced files:
  dist/harness-cli-<platform>
  dist/harness-cli-<platform>.sha256
  dist/harness-cli-windows-x64.exe
  dist/harness-cli-windows-x64.exe.sha256

Supported platform labels:
  aarch64-apple-darwin      -> macos-arm64
  x86_64-apple-darwin       -> macos-x64
  x86_64-unknown-linux-gnu  -> linux-x64
  aarch64-unknown-linux-gnu -> linux-arm64
  x86_64-pc-windows-msvc    -> windows-x64
'@
}

function Get-RepoRoot {
    return [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
}

function Get-HostTriple {
    $rustc = & rustc -vV
    $hostLine = $rustc | Where-Object { $_ -like "host:*" } | Select-Object -First 1
    if (-not $hostLine) {
        throw "Could not determine host target from rustc -vV."
    }
    return ($hostLine -replace '^host:\s*', '').Trim()
}

function Get-PlatformLabel([string]$Triple) {
    switch ($Triple) {
        "aarch64-apple-darwin" { return "macos-arm64" }
        "x86_64-apple-darwin" { return "macos-x64" }
        "x86_64-unknown-linux-gnu" { return "linux-x64" }
        "aarch64-unknown-linux-gnu" { return "linux-arm64" }
        "x86_64-pc-windows-msvc" { return "windows-x64" }
        default { throw "Unsupported release target: $Triple" }
    }
}

if ($args -contains "-h" -or $args -contains "--help") {
    Show-Usage
    exit 0
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $repoRoot "dist"
} elseif (-not [System.IO.Path]::IsPathRooted($OutDir)) {
    $OutDir = [System.IO.Path]::GetFullPath((Join-Path (Get-Location).Path $OutDir))
}

$triple = if ([string]::IsNullOrWhiteSpace($Target)) { Get-HostTriple } else { $Target }
$platform = Get-PlatformLabel $triple

$cargoArgs = @("build", "--package", "harness-cli", "--profile", $Profile)
if (-not [string]::IsNullOrWhiteSpace($Target)) {
    $cargoArgs += @("--target", $Target)
}

$binaryName = "harness-cli"
$artifactName = "harness-cli-$platform"
if ($platform -eq "windows-x64") {
    $binaryName = "harness-cli.exe"
    $artifactName = "$artifactName.exe"
}

$binaryPath = if ([string]::IsNullOrWhiteSpace($Target)) {
    Join-Path $repoRoot "target\$Profile\$binaryName"
} else {
    Join-Path $repoRoot "target\$Target\$Profile\$binaryName"
}

Push-Location $repoRoot
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed."
    }
} finally {
    Pop-Location
}

if (-not (Test-Path $binaryPath)) {
    throw "Expected compiled binary missing: $binaryPath"
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$artifactPath = Join-Path $OutDir $artifactName
Copy-Item -LiteralPath $binaryPath -Destination $artifactPath -Force

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifactPath).Hash.ToLowerInvariant()
$checksumPath = "$artifactPath.sha256"
[System.IO.File]::WriteAllText($checksumPath, "$hash  $artifactName`n")

Write-Host "Built $artifactPath"
Write-Host "Wrote $checksumPath"
