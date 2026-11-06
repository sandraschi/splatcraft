@echo off
setlocal
REM SplatCraft: double-click entry point. Delegates to start.ps1.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0start.ps1" %*
if errorlevel 1 (
  echo.
  echo [splatcraft] start.ps1 exited with error - window held open
  pause
  exit /b 1
)
endlocal
exit /b 0
