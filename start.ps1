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
    Write-Error "cargo was not found. Install Rust from https://rustup.rs/ and reopen your shell."
    exit 1
}

# Require Rust 1.85+ (edition 2024 support needed by dependencies)
$rustVersion = (rustup show active-toolchain 2>$null) -replace '^(\d+\.\d+\.\d+).*','$1'
$cargoVersion = (cargo --version) -replace 'cargo (\d+\.\d+)\..*','$1'
$major, $minor = $cargoVersion -split '\.'
if ([int]$minor -lt 85) {
    Write-Error "[app-call] Rust 1.85 or newer is required (found $cargoVersion). Run: rustup update stable"
    exit 1
}

# Network port (default 9000). Override with: $env:APP_CALL_PORT=9001; .\start.ps1
if (-not $env:APP_CALL_PORT) { $env:APP_CALL_PORT = "9000" }

$env:RUST_BACKTRACE = "1"
"[$(Get-Date -Format s)] starting app-call port=$($env:APP_CALL_PORT)" | Out-File -FilePath $logPath -Encoding utf8
"[$(Get-Date -Format s)] cwd=$scriptDir" | Out-File -FilePath $logPath -Append -Encoding utf8

cargo run -p desktop @args *>&1 | Tee-Object -FilePath $logPath -Append
exit $LASTEXITCODE

