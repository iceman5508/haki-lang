# install.ps1 — Haki Language Windows installer
# Usage (PowerShell, run as user — no admin required):
#   irm https://raw.githubusercontent.com/iceman5508/haki-lang/main/install.ps1 | iex
#
# What it does:
#   1. Downloads the latest haki-windows.zip release from GitHub
#   2. Extracts to %LOCALAPPDATA%\haki\bin
#   3. Adds that directory to the user's PATH (HKCU, no admin needed)
#   4. Prints next-steps instructions

$ErrorActionPreference = 'Stop'
$ProgressPreference    = 'SilentlyContinue'   # faster Invoke-WebRequest

# ── Config ────────────────────────────────────────────────────────────────────
$Repo       = 'iceman5508/haki-lang'
$AssetName  = 'haki-windows.zip'
$InstallDir = Join-Path $env:LOCALAPPDATA 'haki\bin'

# ── Helpers ───────────────────────────────────────────────────────────────────
function Write-Step([string]$msg) {
    Write-Host "  $msg" -ForegroundColor Cyan
}
function Write-Ok([string]$msg) {
    Write-Host "  $msg" -ForegroundColor Green
}
function Write-Err([string]$msg) {
    Write-Host "ERROR: $msg" -ForegroundColor Red
    exit 1
}

# ── Resolve latest release ────────────────────────────────────────────────────
Write-Host ""
Write-Host "Haki Language Installer for Windows" -ForegroundColor White
Write-Host "=====================================" -ForegroundColor DarkGray
Write-Host ""

Write-Step "Fetching latest release from github.com/$Repo ..."

$apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
try {
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ 'User-Agent' = 'haki-installer' }
} catch {
    Write-Err "Could not reach GitHub API. Check your internet connection and try again."
}

$version = $release.tag_name
$asset   = $release.assets | Where-Object { $_.name -eq $AssetName } | Select-Object -First 1
if (-not $asset) {
    Write-Err "Release $version does not contain '$AssetName'. Check https://github.com/$Repo/releases"
}

Write-Ok "Found Haki $version"

# ── Download ──────────────────────────────────────────────────────────────────
$tmpZip = Join-Path $env:TEMP "haki-windows.zip"
Write-Step "Downloading $AssetName ..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tmpZip

# ── Extract ───────────────────────────────────────────────────────────────────
Write-Step "Installing to $InstallDir ..."
if (Test-Path $InstallDir) {
    Remove-Item -Recurse -Force $InstallDir
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Expand-Archive -Path $tmpZip -DestinationPath $InstallDir -Force
Remove-Item $tmpZip

# ── PATH ──────────────────────────────────────────────────────────────────────
$userPath = [System.Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notlike "*$InstallDir*") {
    Write-Step "Adding $InstallDir to user PATH ..."
    $newPath = "$InstallDir;$userPath"
    [System.Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')
    # Also update for this session
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Ok "PATH updated (restart any open terminals to pick up the change)"
} else {
    Write-Ok "$InstallDir already in PATH"
}

# ── Verify ────────────────────────────────────────────────────────────────────
Write-Step "Verifying installation ..."
$hakiExe = Join-Path $InstallDir 'haki.exe'
if (Test-Path $hakiExe) {
    $ver = & $hakiExe --version 2>&1 | Select-Object -First 1
    Write-Ok "haki installed: $ver"
} else {
    Write-Err "haki.exe not found in $InstallDir after extraction. The zip may be malformed."
}

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Haki $version installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "Quick start:" -ForegroundColor White
Write-Host "  haki --version"
Write-Host "  haki-desktop myapp.haki     # native Win32 desktop app"
Write-Host "  haki-server  myapi.haki     # HTTP server module"
Write-Host "  haki-browser myui.haki      # WebAssembly"
Write-Host ""
Write-Host "Docs: https://github.com/$Repo#readme" -ForegroundColor DarkGray
Write-Host ""
