# Automatic Windows install for 5harness (native CLI).
# Documented command:
#   irm https://raw.githubusercontent.com/vantanminh/5harness/v0.26.2/install/windows.ps1 -OutFile install.ps1
#   powershell -File install.ps1
# Local artifact (tests / offline):
#   $env:HARNESS_INSTALL_FROM = "D:\path\to\artifact-dir-or-exe-or-zip"
#   powershell -File install/windows.ps1
#
# HARNESS_INSTALL_PREFIX overrides the install root (default:
# %LOCALAPPDATA%\5harness). HARNESS_INSTALL_SKIP_PATH=1 is useful in CI.
$ErrorActionPreference = "Stop"

function Fail([string]$Message) {
  throw "5harness install: $Message"
}

function Get-Prefix {
  if ($env:HARNESS_INSTALL_PREFIX -and $env:HARNESS_INSTALL_PREFIX.Trim()) {
    return $env:HARNESS_INSTALL_PREFIX.Trim()
  }
  $base = if ($env:LOCALAPPDATA -and $env:LOCALAPPDATA.Trim()) {
    $env:LOCALAPPDATA
  } elseif ($env:USERPROFILE -and $env:USERPROFILE.Trim()) {
    $env:USERPROFILE
  } else {
    Fail "LOCALAPPDATA or USERPROFILE is required"
  }
  return Join-Path $base "5harness"
}

function Get-Target {
  $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
  switch ($architecture) {
    "X64" { return "x86_64-pc-windows-msvc" }
    "Arm64" { return "aarch64-pc-windows-msvc" }
    default { Fail "unsupported Windows architecture: $architecture (supported: X64, Arm64)" }
  }
}

$script:TemporaryRoots = New-Object System.Collections.Generic.List[string]

function Get-ExpectedChecksum([string]$Source) {
  if ($env:HARNESS_INSTALL_EXPECTED_SHA256 -and $env:HARNESS_INSTALL_EXPECTED_SHA256.Trim()) {
    return $env:HARNESS_INSTALL_EXPECTED_SHA256.Trim().ToLowerInvariant()
  }
  $manifest = $env:HARNESS_INSTALL_CHECKSUM_FILE
  if (-not $manifest -and $env:HARNESS_INSTALL_FROM -and (Test-Path -LiteralPath $env:HARNESS_INSTALL_FROM -PathType Container)) {
    foreach ($candidate in @(
      (Join-Path $env:HARNESS_INSTALL_FROM "SHA256SUMS"),
      (Join-Path $env:HARNESS_INSTALL_FROM "sha256sums.txt")
    )) {
      if (Test-Path -LiteralPath $candidate -PathType Leaf) { $manifest = $candidate; break }
    }
  }
  if (-not $manifest -or -not (Test-Path -LiteralPath $manifest -PathType Leaf)) { return $null }
  $name = if ($env:HARNESS_INSTALL_CHECKSUM_NAME -and $env:HARNESS_INSTALL_CHECKSUM_NAME.Trim()) {
    $env:HARNESS_INSTALL_CHECKSUM_NAME.Trim()
  } else {
    [System.IO.Path]::GetFileName($Source)
  }
  foreach ($line in Get-Content -LiteralPath $manifest) {
    if ($line -match '^\s*([0-9A-Fa-f]{64})\s+\*?(.+?)\s*$') {
      $candidate = [System.IO.Path]::GetFileName($Matches[2].Trim())
      if ($candidate -ieq $name) { return $Matches[1].ToLowerInvariant() }
    }
  }
  return $null
}

function Verify-Checksum([string]$Source) {
  $expected = Get-ExpectedChecksum $Source
  if (-not $expected -or $expected -notmatch '^[0-9a-f]{64}$') {
    Fail "no valid SHA-256 checksum found for $([System.IO.Path]::GetFileName($Source)); provide SHA256SUMS or HARNESS_INSTALL_EXPECTED_SHA256"
  }
  $actual = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash.ToLowerInvariant()
  $different = 0
  for ($index = 0; $index -lt 64; $index++) {
    $different = $different -bor ([int][char]$expected[$index] -bxor [int][char]$actual[$index])
  }
  if ($different -ne 0) {
    Fail "SHA-256 mismatch for $([System.IO.Path]::GetFileName($Source)): expected $expected, got $actual"
  }
  Write-Host "Verified SHA-256 for $([System.IO.Path]::GetFileName($Source))"
}

function Find-LocalBinary([string]$From, [string]$Target) {
  if (-not $From -or -not $From.Trim()) { return $null }
  $path = [System.IO.Path]::GetFullPath($From.Trim())
  if (-not (Test-Path -LiteralPath $path)) {
    Fail "HARNESS_INSTALL_FROM does not exist: $From"
  }

  # Check archives before the generic leaf path branch. A .zip is an input
  # bundle, never a native executable to copy to harness.exe.
  if ((Test-Path -LiteralPath $path -PathType Leaf) -and ($path -match '(?i)\.zip$')) {
    $extract = Join-Path ([System.IO.Path]::GetTempPath()) ("5harness-unpack-" + [guid]::NewGuid().ToString("n"))
    New-Item -ItemType Directory -Path $extract -Force | Out-Null
    Expand-Archive -LiteralPath $path -DestinationPath $extract -Force
    $script:TemporaryRoots.Add($extract)
    return Find-LocalBinary $extract $Target
  }

  if (Test-Path -LiteralPath $path -PathType Leaf) {
    if ($path -notmatch '(?i)\.exe$') {
      Fail "HARNESS_INSTALL_FROM must point to a .exe, directory, or .zip: $From"
    }
    return (Resolve-Path -LiteralPath $path).Path
  }

  $names = @("harness-$Target.exe", "harness.exe")
  foreach ($name in $names) {
    $candidate = Join-Path $path $name
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
      return (Resolve-Path -LiteralPath $candidate).Path
    }
  }
  foreach ($name in $names) {
    $nested = Get-ChildItem -LiteralPath $path -Recurse -File -Filter $name -ErrorAction SilentlyContinue |
      Select-Object -First 1
    if ($nested) { return $nested.FullName }
  }
  Fail "HARNESS_INSTALL_FROM did not contain a harness Windows binary for ${Target}: $From"
}

function Add-UserPath([string]$BinDir) {
  if ($env:HARNESS_INSTALL_SKIP_PATH -eq "1") {
    Write-Host "Skipping user PATH update (HARNESS_INSTALL_SKIP_PATH=1)"
    return
  }
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if (-not $userPath) { $userPath = "" }
  $normalized = ([System.IO.Path]::GetFullPath($BinDir)).TrimEnd([char[]]@('\', '/'))
  $parts = @($userPath -split ';' | Where-Object { $_ -and $_.Trim() })
  $alreadyPresent = $false
  foreach ($part in $parts) {
    try {
      $candidate = ([System.IO.Path]::GetFullPath($part.Trim())).TrimEnd([char[]]@('\', '/'))
      if ($candidate -ieq $normalized) { $alreadyPresent = $true; break }
    } catch {
      # Keep unrelated malformed PATH entries untouched.
    }
  }
  if (-not $alreadyPresent) {
    $newPath = if ($userPath.Trim()) { "$userPath;$BinDir" } else { $BinDir }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Added $BinDir to user PATH"
  }
}

function Install-Binary([string]$Source, [string]$Prefix) {
  if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
    Fail "native binary not found: $Source"
  }
  $sourceItem = Get-Item -LiteralPath $Source -Force
  if ($sourceItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
    Fail "refusing to verify a symlinked native binary: $Source"
  }
  Verify-Checksum $Source
  $binDir = Join-Path $Prefix "bin"
  New-Item -ItemType Directory -Path $binDir -Force | Out-Null
  $destination = Join-Path $binDir "harness.exe"
  if (Test-Path -LiteralPath $destination) {
    $destinationItem = Get-Item -LiteralPath $destination -Force
    if ($destinationItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
      Fail "refusing to replace symlinked installed binary: $destination"
    }
  }
  Copy-Item -LiteralPath $Source -Destination $destination -Force
  Verify-Checksum $destination
  Write-Host "Installed $destination"
  Add-UserPath $binDir
  $env:Path = "$binDir;$env:Path"
  & $destination --version
  if ($LASTEXITCODE -ne 0) {
    Fail "harness --version failed after install"
  }
}

$prefix = Get-Prefix
$target = Get-Target
$from = $env:HARNESS_INSTALL_FROM

try {
  if ($from -and $from.Trim()) {
    $source = Find-LocalBinary $from $target
    Install-Binary $source $prefix
    exit 0
  }

  $repo = if ($env:HARNESS_INSTALL_REPO -and $env:HARNESS_INSTALL_REPO.Trim()) {
    $env:HARNESS_INSTALL_REPO.Trim()
  } else {
    "vantanminh/5harness"
  }
  $version = if ($env:HARNESS_INSTALL_VERSION -and $env:HARNESS_INSTALL_VERSION.Trim()) {
    $env:HARNESS_INSTALL_VERSION.Trim()
  } else {
    "latest"
  }
  if ($version -eq "latest") {
    $api = "https://api.github.com/repos/$repo/releases/latest"
    Write-Host "Resolving latest 5harness release from GitHub ($repo)..."
    $release = Invoke-RestMethod -Uri $api -Headers @{ "User-Agent" = "5harness-install" }
    $tag = [string]$release.tag_name
    if (-not $tag) { Fail "latest GitHub release did not contain a tag" }
    $asset = $release.assets | Where-Object { $_.name -eq "harness-$target.exe" } | Select-Object -First 1
    if (-not $asset) { Fail "release $tag has no harness-$target.exe asset" }
    $url = [string]$asset.browser_download_url
  } else {
    if ($version -notmatch '^v?[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$') {
      Fail "HARNESS_INSTALL_VERSION must be semver (for example 0.25.3 or v0.25.3)"
    }
    $tag = "v" + $version.TrimStart('v')
    $url = "https://github.com/$repo/releases/download/$tag/harness-$target.exe"
  }

  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("5harness-$target-" + [guid]::NewGuid().ToString("n") + ".exe")
  $script:TemporaryRoots.Add($tmp)
  $checksum = Join-Path ([System.IO.Path]::GetTempPath()) ("5harness-checksums-" + [guid]::NewGuid().ToString("n") + ".txt")
  $script:TemporaryRoots.Add($checksum)
  Write-Host "Downloading 5harness $tag ($target) from GitHub ($repo)..."
  Invoke-WebRequest -Uri $url -OutFile $tmp -Headers @{ "User-Agent" = "5harness-install" }
  $checksumUrl = "https://github.com/$repo/releases/download/$tag/SHA256SUMS"
  try {
    Invoke-WebRequest -Uri $checksumUrl -OutFile $checksum -Headers @{ "User-Agent" = "5harness-install" }
  } catch {
    Fail "release $tag does not provide SHA256SUMS; refusing to execute an unverified binary"
  }
  $env:HARNESS_INSTALL_CHECKSUM_FILE = $checksum
  $env:HARNESS_INSTALL_CHECKSUM_NAME = "harness-$target.exe"
  Install-Binary $tmp $prefix
} finally {
  foreach ($temporary in $script:TemporaryRoots) {
    try {
      if (Test-Path -LiteralPath $temporary -PathType Container) {
        Remove-Item -LiteralPath $temporary -Recurse -Force -ErrorAction SilentlyContinue
      } elseif (Test-Path -LiteralPath $temporary -PathType Leaf) {
        Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
      }
    } catch {
      # Temporary cleanup must not hide a successful installation.
    }
  }
}
