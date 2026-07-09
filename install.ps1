$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

Write-Host "[app-call] Starting auto install..."

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoEnvPs1 = Join-Path $HOME ".cargo\env.ps1"
    if (Test-Path $cargoEnvPs1) {
        . $cargoEnvPs1
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[app-call] Rust toolchain not found. Installing with rustup..."
    $tempScript = Join-Path $env:TEMP "rustup-init.ps1"
    Invoke-WebRequest -Uri "https://win.rustup.rs/" -OutFile $tempScript
    & $tempScript -y

    $cargoEnvPs1 = Join-Path $HOME ".cargo\env.ps1"
    if (Test-Path $cargoEnvPs1) {
        . $cargoEnvPs1
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "[app-call] cargo still not available after install."
    exit 1
}

Write-Host "[app-call] Updating Rust toolchain (requires 1.85+)..."
rustup update stable

Write-Host "[app-call] Fetching dependencies..."
cargo fetch

Write-Host "[app-call] Building desktop app..."
cargo build -p desktop

Write-Host "[app-call] Install complete. Start with: .\\start.ps1"
