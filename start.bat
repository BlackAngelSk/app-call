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
  echo [app-call] cargo was not found. Install Rust from https://rustup.rs/ and reopen your shell.
  echo.
  pause
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
  echo.
  pause
  exit /b 1
)

if not defined APP_CALL_PORT set "APP_CALL_PORT=9000"

rem Use console mode by default on VMs or when explicitly requested.
rem Set APP_CALL_CONSOLE=0 to force GUI mode.
if not defined APP_CALL_CONSOLE set "APP_CALL_CONSOLE=1"

set "RUST_BACKTRACE=1"
echo [%date% %time%] starting app-call port=%APP_CALL_PORT% console=%APP_CALL_CONSOLE%> "%LOG_PATH%"
echo [%date% %time%] cwd=%CD%>> "%LOG_PATH%"

echo [app-call] Building and starting (log: %LOG_PATH%)...
echo.

cargo run -p desktop %* 2>>"%LOG_PATH%"
set "APP_CALL_EXIT=%ERRORLEVEL%"

if %APP_CALL_EXIT% NEQ 0 (
  echo.
  echo [app-call] Process exited with error code %APP_CALL_EXIT%.
  echo [app-call] Error log saved to: %LOG_PATH%
  echo.
  pause
  exit /b %APP_CALL_EXIT%
)

exit /b 0
