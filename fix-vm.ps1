# fix-vm.ps1 — Run this on the Windows 10 VM to restore missing module exports
$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

Write-Host "[app-call] Fixing crates/app-core/src/lib.rs ..." -ForegroundColor Cyan

$libRsPath = "crates\app-core\src\lib.rs"
$expectedContent = @'
pub mod bootstrap;
pub mod identity;
pub mod model;
pub mod network;
pub mod privacy;
pub mod seed;
pub mod user_settings;

pub use bootstrap::{load_or_create_app, update_display_name};
pub use identity::{DeviceKey, IdentityId, LocalIdentity, PublicIdentity};
pub use model::{AppModel, ChannelKind, ChannelSummary, PeerAddress, ProtocolMessage, SpaceSummary};
pub use network::{IncomingChat, NetworkEvent, NetworkState};
pub use privacy::PrivacyMode;
pub use seed::demo_app;
pub use user_settings::{load_or_create_user_settings, save_user_settings, UserSettings};
'@

$expectedContent | Out-File -FilePath $libRsPath -Encoding utf8 -NoNewline
Write-Host "[app-call] $libRsPath updated." -ForegroundColor Green

# Verify network.rs exists
$networkRsPath = "crates\app-core\src\network.rs"
if (-not (Test-Path $networkRsPath)) {
    Write-Host "[app-call] ERROR: $networkRsPath is missing. You must run: git checkout HEAD -- $networkRsPath" -ForegroundColor Red
    exit 1
} else {
    Write-Host "[app-call] $networkRsPath exists. OK." -ForegroundColor Green
}

# Verify model.rs has PeerAddress and ProtocolMessage
$modelPath = "crates\app-core\src\model.rs"
$modelContent = Get-Content $modelPath -Raw
if ($modelContent -notmatch "PeerAddress") {
    Write-Host "[app-call] ERROR: $modelPath is missing 'PeerAddress'. Please update this file from the repo." -ForegroundColor Red
    exit 1
}
if ($modelContent -notmatch "ProtocolMessage") {
    Write-Host "[app-call] ERROR: $modelPath is missing 'ProtocolMessage'. Please update this file from the repo." -ForegroundColor Red
    exit 1
}
Write-Host "[app-call] $modelPath has required types. OK." -ForegroundColor Green

# Clean build
Write-Host "[app-call] Cleaning build..." -ForegroundColor Cyan
cargo clean 2>&1 | Out-Null
Write-Host "[app-call] Building..." -ForegroundColor Cyan
& cargo build -p desktop 2>&1 | Write-Host
$buildExit = $LASTEXITCODE
if ($buildExit -eq 0) {
    Write-Host ""
    Write-Host "[app-call] Build successful! Run: .\start.bat" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "[app-call] Build failed. Please share the error output above." -ForegroundColor Red
}