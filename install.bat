@echo off
setlocal

cd /d "%~dp0"

echo [app-call] Starting auto install...

where cargo >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
  )
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo [app-call] Rust toolchain not found. Installing with rustup...
  powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://win.rustup.rs/ -OutFile rustup-init.exe"
  if errorlevel 1 exit /b 1
  rustup-init.exe -y
  if errorlevel 1 exit /b 1
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo [app-call] cargo still not available after install.
  exit /b 1
)

echo [app-call] Updating Rust toolchain (requires 1.85+)...
rustup update stable
if errorlevel 1 exit /b 1

echo [app-call] Fetching dependencies...
cargo fetch
if errorlevel 1 exit /b 1

echo [app-call] Building desktop app...
cargo build -p desktop
if errorlevel 1 exit /b 1

echo [app-call] Install complete. Start with: start.bat
