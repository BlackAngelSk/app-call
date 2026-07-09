$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir
$logPath = Join-Path $scriptDir "app-call-launch.log"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoEnvPs1 = Join-Path $HOME ".cargo\env.ps1"
    if (Test-Path $cargoEnvPs1) {
        . $cargoEnvPs1
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[app-call] cargo was not found. Install Rust from https://rustup.rs/ and reopen your shell." -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

# Require Rust 1.85+ (edition 2024 support needed by dependencies)
$cargoVersion = (cargo --version) -replace 'cargo (\d+\.\d+)\..*','$1'
$major, $minor = $cargoVersion -split '\.'
if ([int]$minor -lt 85) {
    Write-Host "[app-call] Rust 1.85 or newer is required (found $cargoVersion). Run: rustup update stable" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

# Network port (default 9000). Override with: $env:APP_CALL_PORT=9001; .\start.ps1
if (-not $env:APP_CALL_PORT) { $env:APP_CALL_PORT = "9000" }

# Use console mode by default on VMs or when explicitly requested.
# Set $env:APP_CALL_CONSOLE="0" to force GUI mode.
if (-not $env:APP_CALL_CONSOLE) { $env:APP_CALL_CONSOLE = "1" }

$env:RUST_BACKTRACE = "1"
"[$(Get-Date -Format s)] starting app-call port=$($env:APP_CALL_PORT) console=$($env:APP_CALL_CONSOLE)" | Out-File -FilePath $logPath -Encoding utf8
"[$(Get-Date -Format s)] cwd=$scriptDir" | Out-File -FilePath $logPath -Append -Encoding utf8

Write-Host "[app-call] Building and starting (log: $logPath)..." -ForegroundColor Cyan
Write-Host ""

cargo run -p desktop @args *>&1 | Tee-Object -FilePath $logPath -Append
$exitCode = $LASTEXITCODE

if ($exitCode -ne 0) {
    Write-Host ""
    Write-Host "[app-call] Process exited with error code $exitCode." -ForegroundColor Red
    Write-Host "[app-call] Full log:" -ForegroundColor Yellow
    Write-Host "────────────────────────────────────────"
    Get-Content $logPath | Write-Host
    Write-Host "────────────────────────────────────────"
    Write-Host ""
    Write-Host "[app-call] Log saved to: $logPath" -ForegroundColor Yellow
    Write-Host ""
    Read-Host "Press Enter to exit"
}

exit $exitCode

