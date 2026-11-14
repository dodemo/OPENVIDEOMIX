@echo off
setlocal EnableExtensions EnableDelayedExpansion
title OPENVIDEOMIX - first local Alpha bootstrap
set "ORIGIN=%USERPROFILE%\source\OPENVIDEOMIX"
set "SANDBOX=%USERPROFILE%\source\OPENVIDEOMIX-ALPHA-EXECUTOR"
set "REMOTE=feat/windows-agent-verification-loop"
echo.
echo === ONE-TIME OPENVIDEOMIX LOCAL ALPHA BOOTSTRAP ===
echo No administrator, no cloud AI, no reset of prior local work.
echo.

if not exist "%ORIGIN%\.git" (
 echo ERROR: Existing OPENVIDEOMIX source checkout not found.
 goto :stop
)
set "URL="
for /f "delims=" %%U in ('git -C "%ORIGIN%" remote get-url origin 2^>nul') do set "URL=%%U"
if not defined URL goto :stop
echo !URL! | findstr /i /c:"dodemo/OPENVIDEOMIX" >nul
if errorlevel 1 (
 echo ERROR: Unexpected Git origin. Not running untrusted code.
 goto :stop
)

if exist "%SANDBOX%\.git" goto :reopen
if exist "%SANDBOX%" (
 echo ERROR: Target directory exists but is not an existing Git worktree.
 echo Nothing overwritten.
 goto :stop
)

echo Downloading only the verified local runner branch...
git -C "%ORIGIN%" fetch --no-tags origin "refs/heads/%REMOTE%:refs/remotes/origin/%REMOTE%"
if errorlevel 1 (
 echo ERROR: Cannot fetch runner branch.
 goto :stop
)
echo Creating a separate working tree, leaving existing changes intact...
git -C "%ORIGIN%" worktree add -b local/windows-alpha-base "%SANDBOX%" "origin/%REMOTE%"
if errorlevel 1 (
 echo ERROR: Cannot create a separate worktree. Existing files untouched.
 goto :stop
)

:reopen
if not exist "%SANDBOX%\START-OPENVIDEOMIX-AGENT.cmd" (
 echo ERROR: Runner is missing from worktree.
 goto :stop
)
echo.
echo Starting deterministic tests and local Ollama agent...
call "%SANDBOX%\START-OPENVIDEOMIX-AGENT.cmd"
goto :end

:stop
echo.
echo Bootstrap stopped safely; no existing branches or files were reset.
pause
:end
endlocal
