@echo off
setlocal

cd /d "%~dp0"
set "LOG_PATH=%~dp0app-call-launch.log"

where cargo >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
  )
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo cargo was not found. Install Rust from https://rustup.rs/ and reopen your shell.
  exit /b 1
)

rem Check Rust version is >= 1.85 (edition 2024 support required by dependencies)
for /f "tokens=2 delims= " %%v in ('cargo --version') do set CARGO_VER=%%v
for /f "tokens=1,2 delims=." %%a in ("%CARGO_VER%") do (
  set CARGO_MAJOR=%%a
  set CARGO_MINOR=%%b
)
if %CARGO_MINOR% LSS 85 (
  echo [app-call] Rust 1.85 or newer is required (found %CARGO_VER%). Run: rustup update stable
  exit /b 1
)

if not defined APP_CALL_PORT set "APP_CALL_PORT=9000"

set "RUST_BACKTRACE=1"
echo [%date% %time%] starting app-call port=%APP_CALL_PORT%> "%LOG_PATH%"
echo [%date% %time%] cwd=%CD%>> "%LOG_PATH%"

cargo run -p desktop %* >> "%LOG_PATH%" 2>&1
set "APP_CALL_EXIT=%ERRORLEVEL%"
type "%LOG_PATH%"
exit /b %APP_CALL_EXIT%
