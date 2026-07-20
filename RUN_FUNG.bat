@echo off
setlocal

REM ============================================================
REM  Script: RUN_FUNG.bat
REM  Purpose: Run the FUNG Tauri desktop app in development mode.
REM ============================================================

set "_ROOT=%~dp0"
pushd "%_ROOT%" >NUL || (
  echo ERROR: Cannot enter project directory "%_ROOT%". 1>&2
  pause
  exit /b 1
)

if /i "%~1"=="--help" goto :usage
if /i "%~1"=="--check" goto :check_only

call :check_tools || goto :fail
call :ensure_deps || goto :fail

echo.
echo Starting FUNG desktop app...
echo Project: %CD%
echo Command: npm run desktop
echo.
npm run desktop
set "_EXIT=%ERRORLEVEL%"
if not "%_EXIT%"=="0" goto :fail_with_code

popd >NUL
exit /b 0

:check_only
call :check_tools || goto :fail
echo OK: launcher checks passed.
popd >NUL
exit /b 0

:check_tools
where npm >NUL 2>&1 || (
  echo ERROR: npm was not found in PATH. Install Node.js or open a shell with npm available. 1>&2
  exit /b 1
)
where cargo >NUL 2>&1 || (
  echo ERROR: cargo was not found in PATH. Install Rust or open a shell with cargo available. 1>&2
  exit /b 1
)
exit /b 0

:ensure_deps
if exist "%_ROOT%node_modules\" exit /b 0
echo node_modules not found. Installing npm dependencies...
npm install || exit /b 1
exit /b 0

:fail_with_code
echo.
echo ERROR: FUNG exited with code %_EXIT%. 1>&2
pause
popd >NUL
exit /b %_EXIT%

:fail
echo.
echo ERROR: Could not start FUNG. 1>&2
pause
popd >NUL
exit /b 1

:usage
echo Usage:
echo   RUN_FUNG.bat          Run FUNG desktop app with Tauri dev
echo   RUN_FUNG.bat --check  Check required tools only
echo.
echo Notes:
echo   This uses npm run desktop, which starts the Vite dev server and Tauri window.
popd >NUL
exit /b 0
