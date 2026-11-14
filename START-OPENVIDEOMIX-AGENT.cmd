@echo off
setlocal EnableExtensions EnableDelayedExpansion
title OPENVIDEOMIX - Windows 11 local agent
cd /d "%~dp0"
if errorlevel 1 goto :startup_error
if not exist "AGENTS.md" goto :startup_error

echo.
echo === OPENVIDEOMIX AUTONOMOUS LOCAL AGENT ===
echo Target: Windows 11 / local Ollama / no Codex quota
echo Working directory: %CD%
echo.

set "VCVARS=%ProgramFiles(x86)%\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if not exist "%VCVARS%" (
  echo ERROR: MSVC vcvars64.bat is missing.
  goto :startup_error
)
call "%VCVARS%" >nul 2>&1
if errorlevel 1 (
  echo ERROR: MSVC environment failed to initialize.
  goto :startup_error
)
set "FIRST_LINK="
for /f "delims=" %%L in ('where link.exe 2^>nul') do if not defined FIRST_LINK set "FIRST_LINK=%%L"
if not defined FIRST_LINK goto :startup_error
echo MSVC linker: !FIRST_LINK!
echo !FIRST_LINK! | findstr /i /c:"Microsoft Visual Studio" >nul
if errorlevel 1 (
  echo ERROR: Windows is still using a non-MSVC linker.
  goto :startup_error
)

set "NPMROOT="
for /f "delims=" %%P in ('npm.cmd prefix -g 2^>nul') do set "NPMROOT=%%P"
if not defined NPMROOT goto :startup_error
set "OPENCODE_EXE=%NPMROOT%\node_modules\@opencode\cli\bin\opencode.exe"
if not exist "!OPENCODE_EXE!" (
  echo ERROR: OpenCode executable not found.
  goto :startup_error
)
if not exist "docs\LOCAL_WINDOWS_ALPHA_AGENT.md" goto :startup_error

if not exist "local-results" mkdir "local-results"
if errorlevel 1 goto :startup_error

rem All model calls are local through the owner's existing Ollama.
rem --auto accepts OpenCode permission prompts. Never run this as Administrator.
set "MODEL=ollama/devstral-small-2:24b-instruct-2512-q4_K_M"
set "STATEFILE=local-results\agent-state.txt"
set /a ROUND=0

:loop
if !ROUND! GEQ 4 goto :limit
set /a ROUND+=1
echo.
echo === AGENT SESSION !ROUND! / 4 ===
> "!STATEFILE!" echo RUNNING

"!OPENCODE_EXE!" run --standalone --auto --agent build --model "!MODEL!" "Read docs/LOCAL_WINDOWS_ALPHA_AGENT.md and AGENTS.md. Execute the current P0 Windows Alpha task, not a planning-only response. Continue from local-results/WINDOWS_ALPHA_REPORT.md when it exists. Build, test, refine, package and measure as far as possible using only Ollama local. Write local-results/WINDOWS_ALPHA_REPORT.md and exactly DONE, BLOCKED or CONTINUE to local-results/agent-state.txt before exiting. Never use paid AI APIs, elevation, destructive operations or git push."
set "RC=!errorlevel!"
if not "!RC!"=="0" (
  echo ERROR: OpenCode exited with code !RC!.
  goto :finish_error
)
set "STATE="
set /p STATE=<"!STATEFILE!"
echo Session status: !STATE!
if /i "!STATE!"=="CONTINUE" goto :loop
if /i "!STATE!"=="DONE" goto :done
if /i "!STATE!"=="BLOCKED" goto :blocked

echo ERROR: Agent did not write a valid final state.
goto :finish_error

:limit
echo.
echo Completed four agent sessions. Work is checkpointed for the next run.
goto :finish

:blocked
echo.
echo Agent needs a real human decision or permission. See local-results\WINDOWS_ALPHA_REPORT.md
goto :finish

:done
echo.
echo Automated scope reached its checkpoint. Check the report and test evidence.
goto :finish

:startup_error
echo.
echo Startup failed. No code was intentionally modified by this launcher.
goto :finish_error

:finish_error
echo See details above. Report, if created: local-results\WINDOWS_ALPHA_REPORT.md
:finish
echo.
echo Report: %CD%\local-results\WINDOWS_ALPHA_REPORT.md
echo You can stop with Ctrl+C at any time.
pause
endlocal
