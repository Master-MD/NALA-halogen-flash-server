$ErrorActionPreference = "Stop"

# NALA-halogen-rs Zero-Python Native Rust Installer (Windows)
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Rust toolchain (cargo) not found. Install from https://rustup.rs"
    exit 1
}

Write-Host "Building nala-halogen-rs (100% Pure Safe Rust, Edition 2024, 0 Crates)..."
cargo build --release --manifest-path (Join-Path $Root "Cargo.toml")

$Dest = Join-Path $env:LOCALAPPDATA "NALA\bin"
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
Copy-Item (Join-Path $Root "target\release\nala-halogen-rs.exe") (Join-Path $Dest "nala-halogen-rs.exe") -Force
& (Join-Path $Dest "nala-halogen-rs.exe") --version
Write-Host "Installed to $Dest\nala-halogen-rs.exe"
