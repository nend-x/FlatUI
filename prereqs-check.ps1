# prereqs-check.ps1 - verify the three build prerequisites are installed.
# Exits with a non-zero code (and a friendly message) if anything is missing.
#
# Usage:
#   .\prereqs-check.ps1
#
# This script does NOT modify anything - it only checks.
#
# NOTE: keep this file pure ASCII. Windows PowerShell 5.1 reads .ps1 files
# without a BOM in the system's legacy codepage, and a UTF-8 em-dash inside
# a string literal decodes to a quote byte that breaks the parser entirely.

$ErrorActionPreference = "Stop"

$missing = @()

# --- 1. Node.js >= 18 ---
try {
    $nodeVersion = (node --version 2>$null)
    if (-not $nodeVersion) { throw "not installed" }
    $nodeMajor = [int]($nodeVersion -replace '^v(\d+).*', '$1')
    if ($nodeMajor -lt 18) {
        Write-Host "FAIL  Node.js $nodeVersion is too old (need >= 18)" -ForegroundColor Red
        $missing += "node"
    } else {
        Write-Host "OK    Node.js $nodeVersion" -ForegroundColor Green
    }
} catch {
    Write-Host "FAIL  Node.js is not installed (need >= 18; get it from https://nodejs.org)" -ForegroundColor Red
    $missing += "node"
}

# --- 2. npm (any version is fine) ---
try {
    $npmVersion = (npm --version 2>$null)
    if (-not $npmVersion) { throw "not installed" }
    Write-Host "OK    npm $npmVersion" -ForegroundColor Green
} catch {
    Write-Host "FAIL  npm is not installed (comes with Node.js)" -ForegroundColor Red
    $missing += "npm"
}

# --- 3. Rust (stable, >= 1.85) with the x86_64-pc-windows-msvc target ---
try {
    $rustcVersion = (rustc --version 2>$null)
    if (-not $rustcVersion) { throw "not installed" }
    $rustcVer = [version]($rustcVersion -replace '^rustc (\d+\.\d+\.\d+).*', '$1')
    $minRustc = [version]"1.85.0"
    if ($rustcVer -lt $minRustc) {
        Write-Host "FAIL  rustc $rustcVer is too old (need >= 1.85.0 - prevent-alt-win-menu uses edition 2024). Run: rustup update stable" -ForegroundColor Red
        $missing += "rustc"
    } else {
        Write-Host "OK    rustc $rustcVer" -ForegroundColor Green
    }
} catch {
    Write-Host "FAIL  rustc is not installed (get it from https://rustup.rs)" -ForegroundColor Red
    $missing += "rustc"
}

# Check the windows-msvc target is installed
try {
    $targets = (rustup target list --installed 2>$null)
    if ($targets -match "x86_64-pc-windows-msvc") {
        Write-Host "OK    rust target x86_64-pc-windows-msvc" -ForegroundColor Green
    } else {
        Write-Host "FAIL  rust target x86_64-pc-windows-msvc is not installed. Run: rustup target add x86_64-pc-windows-msvc" -ForegroundColor Red
        $missing += "msvc-target"
    }
} catch {
    Write-Host "WARN  could not query rustup targets (rustup may not be on PATH)" -ForegroundColor Yellow
}

# --- 4. MSVC C++ Build Tools ---
# We look for vcvarsall.bat / cl.exe via vswhere, which is shipped with VS.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    if ($vsPath) {
        Write-Host "OK    MSVC C++ Build Tools at $vsPath" -ForegroundColor Green
    } else {
        Write-Host "FAIL  VS Build Tools installed but the C++ workload (Microsoft.VisualStudio.Component.VC.Tools.x86.x64) is missing." -ForegroundColor Red
        Write-Host "      Re-run the VS installer and check 'Desktop development with C++'." -ForegroundColor Red
        $missing += "msvc-tools"
    }
} else {
    # vswhere not present - fall back to a cl.exe / link.exe search on PATH.
    $cl = (Get-Command cl.exe -ErrorAction SilentlyContinue)
    if ($cl) {
        Write-Host "OK    cl.exe on PATH at $($cl.Source)" -ForegroundColor Green
    } else {
        Write-Host "FAIL  No Visual Studio C++ Build Tools detected." -ForegroundColor Red
        Write-Host "      Install from https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio" -ForegroundColor Red
        Write-Host "      and select 'Desktop development with C++'." -ForegroundColor Red
        $missing += "msvc-tools"
    }
}

Write-Host ""
if ($missing.Count -gt 0) {
    Write-Host "Missing $($missing.Count) prerequisite(s): $($missing -join ', ')" -ForegroundColor Red
    Write-Host "Build will fail until these are installed. See BUILD.md for details." -ForegroundColor Red
    exit 1
} else {
    Write-Host "All prerequisites are installed." -ForegroundColor Green
    exit 0
}
