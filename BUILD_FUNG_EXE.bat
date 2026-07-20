@echo off
setlocal

REM ============================================================
REM  Script: BUILD_FUNG_EXE.bat
REM  Purpose: Build the FUNG Tauri desktop executable/installer.
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
call :stage_gpu_runtime || goto :fail

echo.
echo Building FUNG desktop release...
echo Project: %CD%
echo Command: npm run tauri -- build
echo.
npm run tauri -- build
set "_EXIT=%ERRORLEVEL%"
if not "%_EXIT%"=="0" goto :fail_with_code

echo.
echo Build finished. Check:
echo   %_ROOT%src-tauri\target\release\
echo   %_ROOT%src-tauri\target\release\bundle\
echo.
pause
popd >NUL
exit /b 0

:check_only
call :check_tools || goto :fail
echo OK: build checks passed.
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

:stage_gpu_runtime
echo Staging FUNG-owned CUDA runtime...
powershell -NoProfile -ExecutionPolicy Bypass -File "%_ROOT%scripts\stage_gpu_runtime.ps1" || exit /b 1
exit /b 0

:fail_with_code
echo.
echo ERROR: Build failed with code %_EXIT%. 1>&2
pause
popd >NUL
exit /b %_EXIT%

:fail
echo.
echo ERROR: Could not build FUNG. 1>&2
pause
popd >NUL
exit /b 1

:usage
echo Usage:
echo   BUILD_FUNG_EXE.bat          Build FUNG desktop release
echo   BUILD_FUNG_EXE.bat --check  Check required tools only
echo.
echo Output:
echo   src-tauri\target\release\
echo   src-tauri\target\release\bundle\
popd >NUL
exit /b 0
