@echo off
setlocal EnableExtensions EnableDelayedExpansion
title OPENVIDEOMIX - local verify and repair
cd /d "%~dp0"
if errorlevel 1 goto :fail
if not exist "AGENTS.md" goto :fail
if not exist "scripts\local_alpha_loop.py" goto :fail

echo.
echo === OPENVIDEOMIX LOCAL VERIFY AND REPAIR ===
echo Windows 11 / Ollama local / no paid coding API
echo All logs remain private in local-results.
echo Stop with Ctrl+C. Do not run as Administrator.
echo.

set "VCVARS=%ProgramFiles(x86)%\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if not exist "%VCVARS%" goto :fail
call "%VCVARS%" >nul 2>&1
if errorlevel 1 goto :fail

set "FIRST_LINK="
for /f "delims=" %%L in ('where link.exe 2^>nul') do if not defined FIRST_LINK set "FIRST_LINK=%%L"
if not defined FIRST_LINK goto :fail
echo MSVC linker: !FIRST_LINK!
echo !FIRST_LINK! | findstr /i /c:"Microsoft Visual Studio" >nul
if errorlevel 1 goto :fail

set "NPMROOT="
for /f "delims=" %%P in ('npm.cmd prefix -g 2^>nul') do set "NPMROOT=%%P"
if not defined NPMROOT goto :fail
set "OPENVIDEOMIX_OPENCODE=%NPMROOT%\node_modules\@opencode\cli\bin\opencode.exe"
if not exist "!OPENVIDEOMIX_OPENCODE!" goto :fail
where python.exe >nul 2>&1
if errorlevel 1 goto :fail
python -c "import sys; sys.exit(sys.version_info < (3, 10))"
if errorlevel 1 goto :fail

rem Model may be selected per run; Devstral remains the default.
set "LOCAL_MODEL=ollama/devstral-small-2:24b-instruct-2512-q4_K_M"
if defined OPENVIDEOMIX_MODEL set "LOCAL_MODEL=!OPENVIDEOMIX_MODEL!"
echo Model: !LOCAL_MODEL!
echo.

python scripts\local_alpha_loop.py --model "!LOCAL_MODEL!" --rounds 3
set "RC=!errorlevel!"
echo.
echo Run exit code: !RC!
echo See local-results\WINDOWS_ALPHA_REPORT.md and WINDOWS_ALPHA_RESULTS.json
if not "!RC!"=="0" (
  echo Errors or external blockers require attention; no changes were pushed.
)
goto :end

:fail
echo ERROR: prerequisites missing or an unsafe linker was detected.
echo The existing Windows installation and Ollama were not modified.
:end
echo.
pause
endlocal
